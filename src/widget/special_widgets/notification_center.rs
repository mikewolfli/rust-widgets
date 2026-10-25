// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! NotificationCenter widget.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::menu_toolbar::popup_reveal::PopupReveal;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use core::cell::{Ref, RefCell};

/// Notification severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    /// Neutral informational notification.
    Info,
    /// Non-fatal problem the user should be aware of.
    Warning,
    /// Failure that needs attention.
    Error,
}

/// One notification entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationItem {
    /// Stable notification id.
    pub id: String,
    /// Notification title.
    pub title: String,
    /// Notification body.
    pub message: String,
    /// Severity level.
    pub level: NotificationLevel,
    /// Whether this item has been read.
    pub read: bool,
    /// Wall-clock time the notification arrived, in milliseconds since the Unix epoch.
    ///
    /// Zero means "no timestamp": the row then omits its time and is grouped with the
    /// previous row when day grouping is on.
    pub timestamp_ms: u64,
}

impl NotificationItem {
    /// Creates a notification item.
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
        level: NotificationLevel,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            message: message.into(),
            level,
            read: false,
            timestamp_ms: 0,
        }
    }

    /// Sets the arrival time, in milliseconds since the Unix epoch.
    pub fn with_timestamp(mut self, timestamp_ms: u64) -> Self {
        self.timestamp_ms = timestamp_ms;
        self
    }

    /// The calendar day (days since the epoch) the notification arrived on, if it has a time.
    pub fn day_index(&self) -> Option<u64> {
        (self.timestamp_ms > 0).then_some(self.timestamp_ms / 86_400_000)
    }
}

/// Notification list center with unread management.
pub struct NotificationCenter {
    base: BaseWidget,
    items: Vec<NotificationItem>,
    /// Bumped whenever something the row layout depends on changes.
    layout_revision: u64,
    /// The laid-out rows and their day headers, recomputed only when the revision or the
    /// geometry moves.
    layout_cache: LayoutCache,
    selected_index: Option<usize>,
    row_height: u32,
    /// The newest notification's slide-in, so an arriving item is seen to arrive.
    ///
    /// # Why one reveal and not one per row
    ///
    /// Same reasoning as `toast::ToastStack`: notifications are queued, so at most one is ever
    /// *arriving*. A driver per row would be N state machines for one visible movement plus the
    /// bookkeeping to retire them; one driver re-aimed on `push` says the same thing.
    reveal: PopupReveal,
    /// Rows scrolled out of view at the top.
    scroll_offset: usize,
    /// Whether each row shows its arrival time.
    show_timestamps: bool,
    /// Whether the header paints a "Clear all" button.
    show_clear_all: bool,
    /// Whether the header paints a "Mark all read" button.
    show_mark_all_read: bool,
    /// Whether the caller set [`Self::show_mark_all_read`] explicitly, which stops it from
    /// following `show_clear_all`.
    mark_all_read_explicit: bool,
    /// Whether rows are preceded by a day header.
    group_by_day: bool,
    /// Only rows whose level matches are shown; `None` shows every level.
    level_filter: Option<NotificationLevel>,
    /// Maximum retained notifications; zero keeps every item.
    max_items: usize,
    /// Emitted when selected notification changes. Payload is id.
    pub notification_selected: Signal1<String>,
    /// Emitted when notification is activated. Payload is id.
    pub notification_activated: Signal1<String>,
    /// Emitted when unread count changes.
    pub unread_count_changed: Signal1<usize>,
    /// Emitted with the id of a notification removed by the user.
    pub notification_dismissed: Signal1<String>,
    /// Emitted when the "Clear all" affordance is activated.
    pub clear_all_requested: Signal1<()>,
    /// Emitted when the "Mark all read" affordance is activated.
    pub all_read_requested: Signal1<()>,
}

/// One laid-out notification row, with the day header it follows.
struct RowLayout {
    /// Index into [`NotificationCenter::items`].
    item: usize,
    /// The row's rectangle.
    rect: Rect,
    /// The day-header text drawn above this row, if it starts a day group.
    ///
    /// The label used to be `format!`-ed on every frame for every header (SP-P68); it is built
    /// with the row layout instead and therefore once per item change.
    day_label: Option<String>,
    /// The row's `HH:MM` stamp, when timestamps are on and the item has a time.
    stamp: Option<String>,
}

/// The laid-out rows and day headers, with the key they were built for.
type LayoutCache = RefCell<Option<((u64, Rect, u32, usize, bool), Vec<RowLayout>)>>;

impl NotificationCenter {
    /// Creates empty notification center.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListView, geometry, "NotificationCenter"),
            items: Vec::new(),
            layout_revision: 1,
            layout_cache: RefCell::new(None),
            selected_index: None,
            row_height: 36,
            reveal: PopupReveal::new(true),
            scroll_offset: 0,
            show_timestamps: false,
            show_clear_all: false,
            show_mark_all_read: false,
            mark_all_read_explicit: false,
            group_by_day: false,
            level_filter: None,
            max_items: 0,
            notification_selected: Signal1::new(),
            notification_activated: Signal1::new(),
            unread_count_changed: Signal1::new(),
            notification_dismissed: Signal1::new(),
            clear_all_requested: Signal1::new(),
            all_read_requested: Signal1::new(),
        }
    }

    /// Enables the time shown at the trailing edge of each row.
    pub fn set_show_timestamps(&mut self, show: bool) {
        if self.show_timestamps == show {
            return;
        }
        self.show_timestamps = show;
        self.invalidate_layout();
        self.base.request_redraw();
    }

    /// Returns whether row timestamps are shown.
    pub fn show_timestamps(&self) -> bool {
        self.show_timestamps
    }

    /// Enables the header's "Clear all" button.
    pub fn set_show_clear_all(&mut self, show: bool) {
        if self.show_clear_all == show {
            return;
        }
        self.show_clear_all = show;
        // The mark-all action is the same header row's sibling, so it follows the clear-all
        // switch unless a caller has set it itself (SP-59).
        if !self.mark_all_read_explicit {
            self.show_mark_all_read = show;
        }
        self.invalidate_layout();
        self.base.request_redraw();
    }

    /// Returns whether the "Clear all" button is shown.
    pub fn show_clear_all(&self) -> bool {
        self.show_clear_all
    }

    /// Enables the header's "Mark all read" button (SP-59).
    ///
    /// The action existed as `mark_all_read()` and as a published command but had no affordance
    /// in the panel, so a user could not mark the list read without host code. It defaults to the
    /// same value as [`Self::set_show_clear_all`] so the two header actions appear together.
    pub fn set_show_mark_all_read(&mut self, show: bool) {
        self.mark_all_read_explicit = true;
        if self.show_mark_all_read == show {
            return;
        }
        self.show_mark_all_read = show;
        self.invalidate_layout();
        self.base.request_redraw();
    }

    /// Returns whether the "Mark all read" button is shown.
    pub fn show_mark_all_read(&self) -> bool {
        self.show_mark_all_read
    }

    /// Enables a day header before the first row of each day.
    pub fn set_group_by_day(&mut self, group: bool) {
        if self.group_by_day == group {
            return;
        }
        self.group_by_day = group;
        self.invalidate_layout();
        self.base.request_redraw();
    }

    /// Returns whether rows are grouped by day.
    pub fn group_by_day(&self) -> bool {
        self.group_by_day
    }

    /// Returns how many rows are scrolled out of view.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Scrolls the list by `delta` rows.
    pub fn scroll_by(&mut self, delta: isize) {
        let capacity = (self.geometry().height / self.row_height.max(1)) as usize;
        let max = self.visible_count().saturating_sub(capacity.max(1));
        let next = (self.scroll_offset as isize + delta).clamp(0, max as isize) as usize;
        if self.scroll_offset == next {
            return;
        }
        self.scroll_offset = next;
        self.invalidate_layout();
        self.base.request_redraw();
    }

    /// Removes the notification owning `id`, publishing it.
    pub fn remove(&mut self, id: &str) -> bool {
        let Some(index) = self.items.iter().position(|item| item.id == id) else {
            return false;
        };
        let removed = self.items.remove(index);
        if self.items.is_empty() {
            self.selected_index = None;
        } else if self.selected_index.is_some_and(|selected| selected >= self.items.len()) {
            self.selected_index = Some(self.items.len() - 1);
        }
        let capacity = (self.geometry().height / self.row_height.max(1)) as usize;
        self.scroll_offset =
            self.scroll_offset.min(self.items.len().saturating_sub(capacity.max(1)));
        self.notification_dismissed.emit(removed.id);
        if !removed.read {
            self.unread_count_changed.emit(self.unread_count());
        }
        self.invalidate_layout();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Marks the row layout stale; the next paint or hit test rebuilds it.
    fn invalidate_layout(&mut self) {
        self.layout_revision = self.layout_revision.wrapping_add(1);
    }

    /// The header height reserved for the header buttons, or zero when none is shown.
    fn header_height(&self) -> i32 {
        let has_action = (self.show_clear_all || self.show_mark_all_read) && !self.items.is_empty();
        if has_action {
            18
        } else {
            0
        }
    }

    /// The "Clear all" button rectangle.
    fn clear_all_rect(&self) -> Option<Rect> {
        if !self.show_clear_all || self.items.is_empty() {
            return None;
        }
        self.header_button_rect(0)
    }

    /// The "Mark all read" button rectangle, placed beside "Clear all".
    fn mark_all_read_rect(&self) -> Option<Rect> {
        if !self.show_mark_all_read || self.items.is_empty() {
            return None;
        }
        let slot = i32::from(self.show_clear_all);
        self.header_button_rect(slot)
    }

    /// One header button, counted from the trailing edge.
    fn header_button_rect(&self, slot: i32) -> Option<Rect> {
        let rect = self.geometry();
        let width = 64.min(rect.width);
        let gap = 4;
        Some(Rect::new(
            rect.x + rect.width as i32 - width as i32 - 4 - slot * (width as i32 + gap),
            rect.y + 2,
            width,
            14,
        ))
    }

    /// The laid-out rows and day headers, cached until something they depend on changes.
    ///
    /// Both the paint and the hit test read this; they used to build the whole row vector
    /// independently, so a click frame paid for it twice (SP-P67). The day labels are built
    /// here too, which is what stops them from being `format!`-ed on every frame (SP-P68).
    fn layout_rows(&self) -> Ref<'_, Vec<RowLayout>> {
        let rect = self.geometry();
        let key =
            (self.layout_revision, rect, self.row_height, self.scroll_offset, self.group_by_day);
        let fresh = self.layout_cache.borrow().as_ref().is_some_and(|(cached, _)| *cached == key);
        if !fresh {
            let rows = self.build_rows(rect);
            *self.layout_cache.borrow_mut() = Some((key, rows));
        }
        Ref::map(self.layout_cache.borrow(), |cache| {
            &cache.as_ref().expect("the row layout was just built").1
        })
    }

    /// Builds the row layout and the day-header labels in one pass.
    fn build_rows(&self, rect: Rect) -> Vec<RowLayout> {
        let mut y = rect.y + self.header_height();
        let mut rows = Vec::new();
        let mut previous_day = None;
        for (index, item) in self.items.iter().enumerate().skip(self.scroll_offset) {
            if !self.item_matches_filter(item) {
                continue;
            }
            let mut day_label = None;
            if self.group_by_day && item.day_index().is_some() && item.day_index() != previous_day {
                y += 14;
                day_label = Self::day_label(item);
            }
            if item.day_index().is_some() {
                previous_day = item.day_index();
            }
            if y >= rect.y + rect.height as i32 {
                break;
            }
            rows.push(RowLayout {
                item: index,
                rect: Rect::new(rect.x, y, rect.width, self.row_height),
                day_label,
                stamp: (self.show_timestamps && item.timestamp_ms > 0)
                    .then(|| Self::stamp_label(item.timestamp_ms)),
            });
            y += self.row_height as i32;
        }
        rows
    }

    /// The `HH:MM` label for a timestamp, in the row's own unit.
    fn stamp_label(timestamp_ms: u64) -> String {
        let seconds = timestamp_ms / 1000;
        let hour = (seconds / 3600) % 24;
        let minute = (seconds / 60) % 60;
        format!("{hour:02}:{minute:02}")
    }

    /// The day-header label for `item`, or `None` when grouping is off or the item has no time.
    fn day_label(item: &NotificationItem) -> Option<String> {
        let day = item.day_index()?;
        let seconds = item.timestamp_ms / 1000;
        let hour = (seconds / 3600) % 24;
        let minute = (seconds / 60) % 60;
        Some(format!("Day {day}  {hour:02}:{minute:02}"))
    }

    /// Returns all notifications.
    pub fn items(&self) -> &[NotificationItem] {
        &self.items
    }

    /// Adds a notification.
    pub fn push(&mut self, item: NotificationItem) {
        self.items.push(item);
        if self.selected_index.is_none() {
            self.selected_index = Some(0);
        }
        // The arriving item slides in from the trailing edge. `jump_to(false)` first, so the first
        // painted frame is the start of the movement rather than a snap into its middle.
        self.reveal.jump_to(false);
        self.reveal.set_open(true);
        self.enforce_item_cap();
        self.clamp_scroll_to_visible();
        self.invalidate_layout();
        self.unread_count_changed.emit(self.unread_count());
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears all notifications.
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_index = None;
        self.invalidate_layout();
        self.unread_count_changed.emit(0);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Sets the level filter, or `None` to show every level.
    pub fn set_level_filter(&mut self, level: Option<NotificationLevel>) {
        if self.level_filter == level {
            return;
        }
        self.level_filter = level;
        self.clamp_scroll_to_visible();
        self.invalidate_layout();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The level filter, or `None` when every level is shown.
    pub fn level_filter(&self) -> Option<NotificationLevel> {
        self.level_filter
    }

    /// Sets how many notifications are retained; zero keeps every item.
    ///
    /// Lowering the cap evicts the oldest items immediately, publishing each through
    /// [`Self::notification_dismissed`] so a mirroring host sees the same removals.
    pub fn set_max_items(&mut self, max_items: usize) {
        if self.max_items == max_items {
            return;
        }
        self.max_items = max_items;
        self.enforce_item_cap();
        self.invalidate_layout();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// How many notifications are retained; zero means unbounded.
    pub fn max_items(&self) -> usize {
        self.max_items
    }

    /// Whether `item` passes the current level filter.
    fn item_matches_filter(&self, item: &NotificationItem) -> bool {
        self.level_filter.is_none_or(|level| item.level == level)
    }

    /// The number of rows the filter lets through.
    fn visible_count(&self) -> usize {
        self.items.iter().filter(|item| self.item_matches_filter(item)).count()
    }

    /// Evicts the oldest items until the cap holds. Removal goes through [`Self::remove`], so
    /// the dismissal signal and selection repair are the same as a user dismissal.
    fn enforce_item_cap(&mut self) {
        if self.max_items == 0 {
            return;
        }
        while self.items.len() > self.max_items {
            let Some(id) = self.items.first().map(|item| item.id.clone()) else {
                break;
            };
            if !self.remove(&id) {
                break;
            }
        }
    }

    /// Keeps the scroll offset inside the rows the filter actually shows.
    fn clamp_scroll_to_visible(&mut self) {
        let capacity = (self.geometry().height / self.row_height.max(1)) as usize;
        let max = self.visible_count().saturating_sub(capacity.max(1));
        self.scroll_offset = self.scroll_offset.min(max);
    }

    /// Returns unread count.
    pub fn unread_count(&self) -> usize {
        self.items.iter().filter(|item| !item.read).count()
    }

    /// Marks a notification read/unread by id.
    pub fn set_read(&mut self, id: &str, read: bool) -> bool {
        let before = self.unread_count();
        for item in &mut self.items {
            if item.id == id {
                item.read = read;
                let after = self.unread_count();
                if before != after {
                    self.unread_count_changed.emit(after);
                }
                self.base.request_redraw();
                return true;
            }
        }
        false
    }

    /// Marks all notifications as read.
    pub fn mark_all_read(&mut self) {
        let mut changed = false;
        for item in &mut self.items {
            if !item.read {
                item.read = true;
                changed = true;
            }
        }
        if changed {
            self.unread_count_changed.emit(0);
            self.base.request_redraw();
        }
    }

    /// Returns selected notification id.
    pub fn selected_id(&self) -> Option<&str> {
        let index = self.selected_index?;
        self.items.get(index).map(|item| item.id.as_str())
    }

    /// Selects notification index.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.selected_index == Some(index) {
            return true;
        }
        self.selected_index = Some(index);
        if let Some(item) = self.items.get(index) {
            self.notification_selected.emit(item.id.clone());
        }
        self.base.request_redraw();
        true
    }

    /// Activates current selected notification.
    pub fn activate_selected(&mut self) -> bool {
        let Some(index) = self.selected_index else {
            return false;
        };
        if index >= self.items.len() {
            return false;
        }

        let id = self.items[index].id.clone();
        let changed = !self.items[index].read;
        self.items[index].read = true;
        self.notification_activated.emit(id);

        if changed {
            self.unread_count_changed.emit(self.unread_count());
        }
        self.base.request_redraw();
        true
    }

    fn row_at(&self, pos: Point) -> Option<usize> {
        let rows = self.layout_rows();
        rows.iter().find(|row| row.rect.contains_point(pos)).map(|row| row.item)
    }

    /// Returns the height of one notification row, in logical pixels.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets the height of one notification row, in logical pixels.
    ///
    /// Clamped to at least one pixel so a row can never collapse to zero height
    /// and become unclickable.
    pub fn set_row_height(&mut self, row_height: u32) {
        let row_height = row_height.max(1);
        if self.row_height == row_height {
            return;
        }
        self.row_height = row_height;
        self.invalidate_layout();
        self.base.request_layout();
        self.base.request_redraw();
    }
}

impl Widget for NotificationCenter {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(320, 400)
    }

    /// Advances the newest notification's slide-in; `true` while it still owes frames.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.reveal.tick(delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.reveal.is_animating()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "notification_selected" => Some(EventSignalRef::mapped(
                "notification_selected",
                &self.notification_selected,
                |v| CapabilityValue::String(v.clone()),
            )),
            "notification_activated" => Some(EventSignalRef::mapped(
                "notification_activated",
                &self.notification_activated,
                |v| CapabilityValue::String(v.clone()),
            )),
            "unread_count_changed" => Some(EventSignalRef::mapped(
                "unread_count_changed",
                &self.unread_count_changed,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            "clear_all_requested" => Some(EventSignalRef::mapped(
                "clear_all_requested",
                &self.clear_all_requested,
                |_| crate::widget::capability::CapabilityValue::Null,
            )),
            "notification_dismissed" => Some(EventSignalRef::mapped(
                "notification_dismissed",
                &self.notification_dismissed,
                |v| crate::widget::capability::CapabilityValue::String(v.clone()),
            )),
            _ => None,
        }
    }
}

/// `NotificationCenter`'s property contract.
///
/// `unread_count` and `item_count` are derived from the live list, and
/// `selected_index` is read-only because selection emits `notification_selected`
/// — the counts would go stale if the list were mutated behind the signals.
impl WidgetProperties for NotificationCenter {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            "unread_count" => Ok(CapabilityValue::UInt(self.unread_count() as u64)),
            "selected_index" => {
                let index = self
                    .items()
                    .iter()
                    .position(|item| Some(item.id.as_str()) == self.selected_id());
                match index {
                    Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                    None => Ok(CapabilityValue::Null),
                }
            }
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "level_filter" => Ok(CapabilityValue::String(
                match self.level_filter() {
                    None => "all",
                    Some(NotificationLevel::Info) => "info",
                    Some(NotificationLevel::Warning) => "warning",
                    Some(NotificationLevel::Error) => "error",
                }
                .to_string(),
            )),
            "max_items" => Ok(CapabilityValue::UInt(self.max_items() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "row_height" => match value {
                CapabilityValue::UInt(height) => {
                    let height =
                        u32::try_from(height).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                    self.set_row_height(height);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "level_filter" => {
                let token = crate::widget::capability::coercion::expect_string(value)?;
                let level = match token.to_ascii_lowercase().as_str() {
                    "all" => None,
                    "info" => Some(NotificationLevel::Info),
                    "warning" => Some(NotificationLevel::Warning),
                    "error" => Some(NotificationLevel::Error),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_level_filter(level);
                Ok(())
            }
            "max_items" => {
                let max = crate::widget::capability::coercion::expect_usize(value)?;
                self.set_max_items(max);
                Ok(())
            }
            "item_count" | "unread_count" | "selected_index" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "item_count",
            "unread_count",
            "selected_index",
            "row_height",
            "level_filter",
            "max_items",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `notification_center` publishes.
    ///
    /// `clear` and `mark_all_read` are payload-free and map onto the widget's
    /// real methods. `activate_selected` reports `OutOfRange` when nothing is
    /// selected, because then there is no notification to activate. `push` needs
    /// a whole `NotificationItem`, `set_read` an id and a flag, and
    /// `select_index` an index, so those are answered as needing a payload.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "mark_all_read" => {
                self.mark_all_read();
                Ok(())
            }
            "activate_selected" => {
                if self.activate_selected() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "push" | "set_read" | "select_index" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for NotificationCenter {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if self.clear_all_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    self.clear_all_requested.emit(());
                    return;
                }
                if self.mark_all_read_rect().is_some_and(|rect| rect.contains_point(*pos)) {
                    self.all_read_requested.emit(());
                    self.mark_all_read();
                    return;
                }
                let hit = {
                    let rows = self.layout_rows();
                    rows.iter()
                        .find(|row| row.rect.contains_point(*pos))
                        .map(|row| (row.item, row.rect))
                };
                if let Some((index, rect)) = hit {
                    // The trailing band is the row's remove affordance.
                    if pos.x >= rect.x + rect.width as i32 - 22 {
                        let id = self.items[index].id.clone();
                        let _ = self.remove(&id);
                        return;
                    }
                    let _ = self.select_index(index);
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some(index) = self.row_at(*pos) {
                    let _ = self.select_index(index);
                    let _ = self.activate_selected();
                }
            }
            Event::Wheel { delta, .. } => {
                if delta.y != 0 {
                    self.scroll_by(if delta.y > 0 { 1 } else { -1 });
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                38 => {
                    if let Some(index) = self.selected_index {
                        if index > 0 {
                            let _ = self.select_index(index - 1);
                        }
                    } else if !self.items.is_empty() {
                        let _ = self.select_index(0);
                    }
                }
                40 => {
                    if let Some(index) = self.selected_index {
                        if index + 1 < self.items.len() {
                            let _ = self.select_index(index + 1);
                        }
                    } else if !self.items.is_empty() {
                        let _ = self.select_index(0);
                    }
                }
                13 => {
                    let _ = self.activate_selected();
                }
                46 => {
                    if let Some(id) = self.selected_id().map(str::to_owned) {
                        let _ = self.remove(&id);
                    }
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for NotificationCenter {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("notification_center");
        // `notification_center` is not a control kind in the role table, so it
        // classifies as `Surface`, whose background is `theme.colors.background` —
        // byte-identical to the window behind it. The panel is therefore a step
        // toward the foreground, so it reads as a surface of its own.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(resolved));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = resolved.blend(&text_color, 0.08);
        // Selection and unread row tints are chrome states, so they are derived from
        // the resolved colours rather than from literals: the selected row reads as
        // tinted toward the text colour, the unread one more faintly.
        let selected_background = background.blend(&text_color, 0.14);
        let unread_background = background.blend(&text_color, 0.05);
        // The row separator is secondary chrome, derived from the same pair.
        let separator = background.blend(&text_color, 0.12);

        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        if let Some(button) = self.clear_all_rect() {
            context.fill_rect(button, background.blend(&text_color, 0.10));
            context.draw_rect(button, separator);
            context.draw_text_line_cow(
                button,
                "Clear all",
                effective_font(style),
                text_color,
                HorizontalAlignment::Center,
            );
        }
        if let Some(button) = self.mark_all_read_rect() {
            context.fill_rect(button, background.blend(&text_color, 0.10));
            context.draw_rect(button, separator);
            context.draw_text_line_cow(
                button,
                "Mark all read",
                effective_font(style),
                text_color,
                HorizontalAlignment::Center,
            );
        }

        let rows = self.layout_rows();
        for row in rows.iter() {
            let index = row.item;
            let row_rect = row.rect;
            let Some(item) = self.items.get(index) else {
                continue;
            };
            // A day header is drawn in the strip `build_rows` reserved above the first row of
            // that day. Grouping is a reading aid, so it is opt-in. The label itself was built
            // with the row layout, so a still frame does not format it again (SP-P68).
            if let Some(label) = row.day_label.as_deref() {
                let header = Rect::new(row_rect.x + 4, row_rect.y - 14, row_rect.width, 12);
                context.draw_text_line_cow(
                    header,
                    label,
                    effective_font(style),
                    text_color.blend(&background, 0.2),
                    HorizontalAlignment::Left,
                );
            }
            let y = row_rect.y;
            let rect = self.geometry();
            // # Why only the newest row moves
            //
            // The reveal is re-aimed on every `push`, so applying it to every row would make the
            // whole list slide whenever one item is added — the settled items would look like they
            // are re-arriving, which says the opposite of what happened. Items are appended, so the
            // one at `len - 1` is the only one still on its way in.
            let is_arriving = index + 1 == self.items.len();
            let row_rect = if is_arriving && self.reveal.is_animating() {
                // Slides in from the trailing edge, so it reads as the list *growing* rather than
                // as a row fading in place. Proportional to the row so a taller row travels further.
                let offset = (self.row_height as f32 * (1.0 - self.reveal.value())) as i32;
                Rect::new(row_rect.x + offset, row_rect.y, row_rect.width, row_rect.height)
            } else {
                row_rect
            };

            let bg = if self.selected_index == Some(index) {
                selected_background
            } else if !item.read {
                unread_background
            } else {
                background
            };
            context.fill_rect(row_rect, bg);

            // The badge is a *state* indicator, so it reads the theme's semantic
            // tokens rather than a literal pair of its own.
            let badge_color = crate::style::semantic_color(match item.level {
                NotificationLevel::Info => crate::style::SemanticColor::Info,
                NotificationLevel::Warning => crate::style::SemanticColor::Warning,
                NotificationLevel::Error => crate::style::SemanticColor::Error,
            })
            .map(|token| token.blend(&background, 0.25))
            .unwrap_or_else(|| background.blend(&text_color, 0.5));

            // The row's leading indicator and the two text lines are placed from the row's own
            // box rather than from the literals `+ 8`/`+ 14`/`+ 22`/`+ 28`, which described a
            // 36 px row: at any other `row_height` the badge and both labels drifted out of
            // the row they belong to (a 20 px row drew its message 8 px past its own bottom
            // edge). `row_height` is a published property, so it can be any value a caller
            // sets.
            let indent = (self.row_height / 3).max(8) as i32;
            let badge_size = 8u32.min(self.row_height);
            let badge_y = y + (self.row_height.saturating_sub(badge_size) / 2) as i32;
            context.fill_rect(
                Rect::new(rect.x + indent, badge_y, badge_size, badge_size),
                badge_color,
            );
            let text_x = rect.x + indent + badge_size as i32 + 6;
            // Trailing chrome: the remove affordance always, the timestamp when asked for.
            // Both are reserved out of the title band so a long title cannot run under them.
            let trailing = 22 + if self.show_timestamps && item.timestamp_ms > 0 { 40 } else { 0 };
            // Two stacked lines: the title on the row's upper half, the message on the lower,
            // each fitted to the room the row actually has.
            let line_height = (self.row_height / 2).max(1);
            let title_band = Rect::new(
                text_x,
                y,
                (rect.width as i32 - (text_x - rect.x) - trailing).max(0) as u32,
                line_height,
            );
            context.draw_text_fitted_cow(
                title_band,
                &item.title,
                effective_font(style),
                text_color,
                HorizontalAlignment::Left,
            );
            if !item.message.is_empty() {
                let message_band = Rect::new(
                    text_x,
                    y + line_height as i32,
                    (rect.width as i32 - (text_x - rect.x) - trailing).max(0) as u32,
                    line_height,
                );
                context.draw_text_fitted_cow(
                    message_band,
                    &item.message,
                    effective_font(style),
                    // The message is secondary text, so it is a tint of the resolved
                    // foreground rather than a second literal.
                    text_color.blend(&background, 0.25),
                    HorizontalAlignment::Left,
                );
            }

            context.draw_line(
                Point::new(rect.x, y + self.row_height as i32),
                Point::new(rect.x + rect.width as i32, y + self.row_height as i32),
                separator,
            );

            // Trailing remove ×, in the band the press handler reserves.
            let close_x = row_rect.x + row_rect.width as i32 - 14;
            let close_y = row_rect.y + row_rect.height as i32 / 2;
            let close_ink = text_color.with_alpha(150);
            context.draw_line_stroke(
                Point::new(close_x - 3, close_y - 3),
                Point::new(close_x + 3, close_y + 3),
                close_ink,
                1,
            );
            context.draw_line_stroke(
                Point::new(close_x + 3, close_y - 3),
                Point::new(close_x - 3, close_y + 3),
                close_ink,
                1,
            );
            if let Some(stamp) = row.stamp.as_deref() {
                let band = Rect::new(
                    row_rect.x + row_rect.width as i32 - 56,
                    row_rect.y,
                    36,
                    row_rect.height,
                );
                context.draw_text_line_cow(
                    band,
                    stamp,
                    effective_font(style),
                    text_color.blend(&background, 0.25),
                    HorizontalAlignment::Right,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_center() -> NotificationCenter {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 420, 160));
        center.push(NotificationItem::new(
            "n1",
            "Build finished",
            "All checks passed",
            NotificationLevel::Info,
        ));
        center.push(NotificationItem::new(
            "n2",
            "Deploy warning",
            "One region has latency spike",
            NotificationLevel::Warning,
        ));
        center.push(NotificationItem::new(
            "n3",
            "Runtime error",
            "Worker crashed",
            NotificationLevel::Error,
        ));
        center
    }

    #[test]
    fn unread_count_and_mark_read_work() {
        let mut center = sample_center();
        assert_eq!(center.unread_count(), 3);

        assert!(center.set_read("n2", true));
        assert_eq!(center.unread_count(), 2);

        center.mark_all_read();
        assert_eq!(center.unread_count(), 0);
    }

    #[test]
    fn level_filter_and_item_cap_bound_the_list() {
        let mut center = sample_center();
        center.set_level_filter(Some(NotificationLevel::Warning));
        {
            // Scoped: `layout_rows` hands out a borrow of the layout cache, which must be
            // released before the setters below take `&mut center`.
            let rows = center.layout_rows();
            assert_eq!(rows.len(), 1, "only the warning row is laid out");
            assert_eq!(center.items()[rows[0].item].id, "n2");
        }

        center.set_level_filter(None);
        let dismissed = Arc::new(Mutex::new(Vec::<String>::new()));
        center.notification_dismissed.connect({
            let dismissed = Arc::clone(&dismissed);
            move |id: Arc<String>| dismissed.lock().unwrap().push((*id).clone())
        });
        center.set_max_items(2);
        assert_eq!(center.items().len(), 2, "the cap evicts down to two rows");
        assert_eq!(*dismissed.lock().unwrap(), vec!["n1".to_string()]);
        assert_eq!(center.items()[0].id, "n2");
    }

    #[test]
    fn activate_selected_marks_read_and_emits() {
        let mut center = sample_center();
        let activated = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = activated.clone();
        center.notification_activated.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(center.select_index(1));
        assert!(center.activate_selected());
        assert_eq!(center.selected_id(), Some("n2"));
        assert_eq!(center.unread_count(), 2);

        let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["n2".to_string()]);
    }

    #[test]
    fn keyboard_navigation_changes_selection() {
        let mut center = sample_center();

        center.handle_event(&Event::key_press(40, 0));
        assert_eq!(center.selected_id(), Some("n2"));

        center.handle_event(&Event::key_press(40, 0));
        assert_eq!(center.selected_id(), Some("n3"));

        center.handle_event(&Event::key_press(38, 0));
        assert_eq!(center.selected_id(), Some("n2"));
    }

    #[test]
    fn new_creates_default_state() {
        let center = NotificationCenter::new(Rect::new(0, 0, 800, 600));
        assert!(center.items().is_empty());
        assert_eq!(center.selected_id(), None);
        assert_eq!(center.unread_count(), 0);
    }

    #[test]
    fn items_returns_items() {
        let center = sample_center();
        assert_eq!(center.items().len(), 3);
        assert_eq!(center.items()[0].id, "n1");
        assert_eq!(center.items()[1].id, "n2");
        assert_eq!(center.items()[2].id, "n3");
    }

    #[test]
    fn push_sets_selection_to_first() {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 800, 600));
        center.push(NotificationItem::new("n1", "Title", "Body", NotificationLevel::Info));
        assert_eq!(center.selected_id(), Some("n1"));
    }

    #[test]
    fn clear_removes_all_and_resets_state() {
        let mut center = sample_center();
        center.clear();
        assert!(center.items().is_empty());
        assert_eq!(center.selected_id(), None);
        assert_eq!(center.unread_count(), 0);
    }

    #[test]
    fn select_index_invalid_returns_false() {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 800, 600));
        assert!(!center.select_index(0));

        center.push(NotificationItem::new("n1", "Title", "Body", NotificationLevel::Info));
        assert!(!center.select_index(5));
        assert_eq!(center.selected_id(), Some("n1"));
    }

    #[test]
    fn unread_count_changed_signal_on_push() {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 800, 600));
        let counts = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = counts.clone();
        center.unread_count_changed.connect(move |count| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*count);
            }
        });

        center.push(NotificationItem::new("n1", "Title", "Body", NotificationLevel::Info));
        let got = counts.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(got, vec![1]);
    }

    #[test]
    fn keyboard_navigation_on_empty_does_nothing() {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 800, 600));

        // Should not panic
        center.handle_event(&Event::key_press(40, 0));
        assert_eq!(center.selected_id(), None);

        center.handle_event(&Event::key_press(38, 0));
        assert_eq!(center.selected_id(), None);
    }

    #[test]
    fn activate_selected_marks_read_and_updates_unread_count() {
        let mut center = NotificationCenter::new(Rect::new(0, 0, 800, 600));
        center.push(NotificationItem::new("n1", "Title", "Body", NotificationLevel::Info));
        assert_eq!(center.unread_count(), 1);

        // activate_selected on index 0 (first/only item)
        assert!(center.activate_selected());
        assert_eq!(center.unread_count(), 0);
    }

    #[test]
    fn set_read_nonexistent_returns_false() {
        let mut center = sample_center();
        assert!(!center.set_read("nonexistent", true));
        assert_eq!(center.unread_count(), 3);
    }

    #[test]
    fn select_index_duplicate_guard_returns_true() {
        let mut center = sample_center();
        // First select a different index
        assert!(center.select_index(1));

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        center.notification_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        // Select index 0 - emits "n1"
        assert!(center.select_index(0));
        // Second call with same index returns true but doesn't emit
        assert!(center.select_index(0));
        let got = emitted.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn enter_key_activates_selected() {
        let mut center = sample_center();
        let activated = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = activated.clone();
        center.notification_activated.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        // Navigate down to n2, then activate
        center.handle_event(&Event::key_press(40, 0));
        center.handle_event(&Event::key_press(13, 0));

        let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["n2".to_string()]);
    }
}
