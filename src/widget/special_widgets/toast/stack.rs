// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `ToastStack` — the container that queues toasts and lays them out as rows.

use super::item::{ToastItem, ToastLevel};
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::menu_toolbar::popup_reveal::PopupReveal;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use core::cell::RefCell;

/// Where a toast stack anchors its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastAnchor {
    /// Bottom-left, the default.
    #[default]
    BottomLeft,
    /// Top-right, the placement a notification banner uses.
    TopRight,
    /// Top-left.
    TopLeft,
    /// Bottom-right.
    BottomRight,
}

/// Which end of the stack receives the newest toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastStackingDirection {
    /// The newest toast is the lowest row, the historical behaviour.
    #[default]
    NewestAtBottom,
    /// The newest toast is the highest row.
    NewestAtTop,
}

/// Toast stack with keyboard/mouse activation and dismiss.
pub struct ToastStack {
    base: BaseWidget,
    toasts: Vec<ToastItem>,
    selected_index: Option<usize>,
    row_height: u32,
    /// The newest toast's rise-in reveal, so a toast arrives from the bottom edge instead of
    /// appearing on the frame it was pushed.
    ///
    /// # Why one reveal and not one per toast
    ///
    /// Toasts are queued, so at most one is ever *arriving* — the others have already settled. A
    /// per-toast driver would be N state machines for one visible movement, plus the lifetime
    /// bookkeeping to retire them. One driver, re-aimed on every `push`, says the same thing: the
    /// newest row rises, the rest sit still.
    reveal: PopupReveal,
    /// The exit reveal of a row being dismissed. Dismissal is deferred until this settles, so a
    /// toast leaves the same way it arrives instead of vanishing between frames.
    exit: PopupReveal,
    /// The index of the row playing its exit animation.
    closing: Option<usize>,
    /// Whether `dismiss_selected` animates the row out before removing it.
    ///
    /// Off by default: a caller that removes a toast expects the item to be gone, and the
    /// caller that asked for motion opts in with [`ToastStack::set_animate_exit`].
    animate_exit: bool,
    anchor: ToastAnchor,
    /// Which end of the stack receives the newest toast.
    stacking_direction: ToastStackingDirection,
    /// The maximum number of toasts kept at once; the oldest is expired beyond it.
    max_visible: usize,
    /// The stack's own monotonic millisecond clock, advanced by `tick`.
    now_ms: u64,
    /// When each toast was pushed, aligned with `toasts`.
    shown_at_ms: Vec<u64>,
    /// How many of the oldest rows are scrolled out of view.
    scroll_offset: usize,
    /// Whether rows paint a translucent (vibrant) fill.
    translucent: bool,
    /// Whether each row paints a trailing close button (SP-72).
    row_close_visible: bool,
    /// Measured action-label widths, keyed by label and font size.
    ///
    /// `estimate_text_width` ran for every row's action on every frame for a string and a font
    /// that had not changed (SP-P76).
    action_widths: RefCell<crate::compat::HashMap<(String, u32), u32>>,
    /// Emitted when toast is activated.
    pub toast_activated: Signal1<String>,
    /// Emitted when toast is dismissed.
    pub toast_dismissed: Signal1<String>,
    /// Emitted with the toast id when its action button is pressed.
    pub toast_action: Signal1<String>,
    /// Emitted with the id of a toast removed by its ttl or by the visible cap.
    pub toast_expired: Signal1<String>,
}

impl ToastStack {
    /// The tint a severity badge is painted in, derived from the theme's semantic token.
    fn badge_color(level: ToastLevel, background: Color, text_color: Color) -> Color {
        crate::style::semantic_color(level.into())
        .map(|token| token.blend(&background, 0.15))
        .unwrap_or_else(|| background.blend(&text_color, 0.6))
    }
    /// Creates empty toast stack.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PopupWindow, geometry, "ToastStack"),
            toasts: Vec::new(),
            selected_index: None,
            row_height: 30,
            reveal: PopupReveal::new(true),
            exit: PopupReveal::new(true),
            closing: None,
            animate_exit: false,
            anchor: ToastAnchor::BottomLeft,
            stacking_direction: ToastStackingDirection::default(),
            max_visible: 5,
            now_ms: 0,
            shown_at_ms: Vec::new(),
            scroll_offset: 0,
            translucent: false,
            row_close_visible: true,
            action_widths: RefCell::new(crate::compat::HashMap::new()),
            toast_activated: Signal1::new(),
            toast_dismissed: Signal1::new(),
            toast_action: Signal1::new(),
            toast_expired: Signal1::new(),
        }
    }

    /// Enables the animated exit.
    pub fn set_animate_exit(&mut self, animate: bool) {
        if self.animate_exit == animate {
            return;
        }
        self.animate_exit = animate;
        if !animate {
            self.flush_pending_dismissal();
        }
    }

    /// Returns whether dismissal animates.
    pub fn animate_exit(&self) -> bool {
        self.animate_exit
    }

    /// Sets where the stack anchors its rows.
    pub fn set_anchor(&mut self, anchor: ToastAnchor) {
        if self.anchor == anchor {
            return;
        }
        self.anchor = anchor;
        self.base.request_redraw();
    }

    /// Returns the stack's anchor.
    pub fn anchor(&self) -> ToastAnchor {
        self.anchor
    }

    /// Which end of the stack receives the newest toast.
    pub fn stacking_direction(&self) -> ToastStackingDirection {
        self.stacking_direction
    }

    /// Sets which end of the stack receives the newest toast.
    pub fn set_stacking_direction(&mut self, direction: ToastStackingDirection) {
        if self.stacking_direction == direction {
            return;
        }
        self.stacking_direction = direction;
        self.base.request_redraw();
    }

    /// The maximum number of toasts kept at once.
    pub fn max_visible(&self) -> usize {
        self.max_visible
    }

    /// Caps the stack; pushing past the cap expires the oldest toast.
    pub fn set_max_visible(&mut self, max_visible: usize) {
        let max_visible = max_visible.max(1);
        if self.max_visible == max_visible {
            return;
        }
        self.max_visible = max_visible;
        while self.toasts.len() > self.max_visible {
            self.remove_oldest(true);
        }
        self.base.request_redraw();
    }

    /// Makes the rows translucent, so the window shows through them.
    ///
    /// A rendering layer without backdrop blur can only approximate vibrancy with alpha; that
    /// is what this opt-in flag does, and it is why the opaque default is unchanged.
    pub fn set_translucent(&mut self, translucent: bool) {
        if self.translucent == translucent {
            return;
        }
        self.translucent = translucent;
        self.base.request_redraw();
    }

    /// Returns whether rows are translucent.
    pub fn is_translucent(&self) -> bool {
        self.translucent
    }

    /// The number of oldest rows scrolled out of view.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Sets how many oldest rows are scrolled out of view.
    pub fn set_scroll_offset(&mut self, offset: usize) {
        let max = self.toasts.len().saturating_sub(self.visible_row_count());
        let offset = offset.min(max);
        if self.scroll_offset == offset {
            return;
        }
        self.scroll_offset = offset;
        self.base.request_redraw();
    }

    /// The number of rows the band can show at once.
    fn visible_row_count(&self) -> usize {
        let capacity = self.geometry().height as usize / self.row_height.max(1) as usize;
        capacity.max(1)
    }

    /// Returns toast items.
    pub fn toasts(&self) -> &[ToastItem] {
        &self.toasts
    }

    /// Adds a toast to the stack.
    pub fn push(&mut self, item: ToastItem) {
        self.push_at(item, self.now_ms);
    }

    /// Adds a toast stamped with `now_ms` on the stack's own clock.
    ///
    /// Use this when the host feeds `expire_toasts` an absolute time: the timestamp the
    /// ttl counts from is then the same domain the expiry check reads.
    pub fn push_at(&mut self, item: ToastItem, now_ms: u64) {
        self.now_ms = self.now_ms.max(now_ms);
        self.toasts.push(item);
        self.shown_at_ms.push(self.now_ms);
        self.selected_index = Some(self.toasts.len() - 1);
        // The new row starts fully below its seat and rises into it. `jump_to(false)` before
        // aiming at open, so the first painted frame is the start of the movement rather than a
        // snap to the middle of it.
        self.reveal.jump_to(false);
        self.reveal.set_open(true);
        while self.toasts.len() > self.max_visible {
            self.remove_oldest(true);
        }
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Removes the oldest toast, reporting it as expired when `emit` is set.
    fn remove_oldest(&mut self, emit: bool) {
        if self.toasts.is_empty() {
            return;
        }
        let item = self.toasts.remove(0);
        if !self.shown_at_ms.is_empty() {
            self.shown_at_ms.remove(0);
        }
        if let Some(index) = self.selected_index.as_mut() {
            *index = index.saturating_sub(1);
        }
        if emit {
            self.toast_expired.emit(item.id);
        }
    }

    /// Removes every toast whose ttl has elapsed by `now_ms`, returning how many left.
    ///
    /// `now_ms` is read on the stack's own millisecond clock — the one `tick` advances and
    /// `push_at` stamps against. A toast whose `ttl_ms` is zero never expires.
    pub fn expire_toasts(&mut self, now_ms: u64) -> usize {
        self.now_ms = self.now_ms.max(now_ms);
        let mut removed = 0usize;
        let mut index = 0usize;
        while index < self.toasts.len() {
            let ttl = self.toasts[index].ttl_ms as u64;
            let shown = self.shown_at_ms.get(index).copied().unwrap_or(0);
            if ttl > 0 && self.now_ms >= shown.saturating_add(ttl) {
                let item = self.toasts.remove(index);
                if index < self.shown_at_ms.len() {
                    self.shown_at_ms.remove(index);
                }
                if let Some(selected) = self.selected_index.as_mut() {
                    if *selected > index {
                        *selected -= 1;
                    } else if *selected == index {
                        *selected = self.toasts.len().saturating_sub(1);
                    }
                }
                if self.closing == Some(index) {
                    self.closing = None;
                }
                self.toast_expired.emit(item.id);
                removed += 1;
            } else {
                index += 1;
            }
        }
        if self.toasts.is_empty() {
            self.selected_index = None;
        }
        self.clamp_scroll();
        if removed > 0 {
            self.base.request_layout();
            self.base.request_redraw();
        }
        removed
    }

    fn clamp_scroll(&mut self) {
        let max = self.toasts.len().saturating_sub(self.visible_row_count());
        self.scroll_offset = self.scroll_offset.min(max);
    }

    /// Clears all toasts.
    pub fn clear(&mut self) {
        self.toasts.clear();
        self.shown_at_ms.clear();
        self.selected_index = None;
        self.scroll_offset = 0;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns selected toast id.
    pub fn selected_id(&self) -> Option<&str> {
        let index = self.selected_index?;
        self.toasts.get(index).map(|item| item.id.as_str())
    }

    /// Selects toast by index.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.toasts.len() {
            return false;
        }
        self.selected_index = Some(index);
        self.base.request_redraw();
        true
    }

    /// Activates selected toast.
    pub fn activate_selected(&mut self) -> bool {
        let Some(index) = self.selected_index else {
            return false;
        };
        let Some(item) = self.toasts.get(index) else {
            return false;
        };
        self.toast_activated.emit(item.id.clone());
        true
    }

    /// Dismisses selected toast.
    ///
    /// The row is not removed on this frame: it starts its exit animation, and [`Widget::tick`]
    /// finishes the removal once the reveal has settled. A caller that wants the row gone at once
    /// can call [`Self::flush_pending_dismissal`] after the animation, or push a new toast — the
    /// removal is the same code either way.
    pub fn dismiss_selected(&mut self) -> bool {
        let Some(index) = self.selected_index else {
            return false;
        };
        if index >= self.toasts.len() || self.closing.is_some() {
            return false;
        }
        if !self.animate_exit {
            // The immediate path is the historical contract: the row is gone on this call.
            self.closing = Some(index);
            return self.flush_pending_dismissal();
        }
        self.closing = Some(index);
        self.exit.jump_to(true);
        self.exit.set_open(false);
        self.base.request_redraw();
        true
    }

    /// Removes the row whose exit animation is in flight, if any.
    pub fn flush_pending_dismissal(&mut self) -> bool {
        let Some(index) = self.closing.take() else {
            return false;
        };
        if index >= self.toasts.len() {
            return false;
        }
        let id = self.toasts[index].id.clone();
        self.toasts.remove(index);
        if index < self.shown_at_ms.len() {
            self.shown_at_ms.remove(index);
        }
        self.toast_dismissed.emit(id);
        if self.toasts.is_empty() {
            self.selected_index = None;
        } else if index >= self.toasts.len() {
            self.selected_index = Some(self.toasts.len() - 1);
        }
        let max = self.toasts.len().saturating_sub(self.visible_row_count());
        self.scroll_offset = self.scroll_offset.min(max);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// The measured trailing band an action's label occupies inside `row`.
    fn action_rect_for(
        &self,
        row: Rect,
        label: &str,
        font: &crate::core::Font,
        right: i32,
    ) -> Rect {
        let width = self.action_width(label, font).min(row.width);
        Rect::new(right - width as i32 - 4, row.y, width, row.height)
    }

    /// The measured width of one action label, cached per label and font size.
    fn action_width(&self, label: &str, font: &crate::core::Font) -> u32 {
        let key = (label.to_string(), font.size().to_bits());
        if let Some(width) = self.action_widths.borrow().get(&key) {
            return *width;
        }
        let width = estimate_text_width(label, font, 1.0) + 12;
        self.action_widths.borrow_mut().insert(key, width);
        width
    }

    /// Whether each row paints a trailing close button.
    pub fn row_close_visible(&self) -> bool {
        self.row_close_visible
    }

    /// Shows or hides the per-row close button.
    ///
    /// The row was removable only through the keyboard or the host API, which left a mouse user
    /// unable to dismiss a single toast; the button routes through the same dismissal path
    /// `Delete` uses, so the exit animation and the signal are identical (SP-72).
    pub fn set_row_close_visible(&mut self, visible: bool) {
        if self.row_close_visible == visible {
            return;
        }
        self.row_close_visible = visible;
        self.base.request_redraw();
    }

    /// The close button's rectangle inside `row`, when it is shown.
    fn close_rect_for(&self, row: Rect) -> Option<Rect> {
        if !self.row_close_visible {
            return None;
        }
        let side = 14u32.min(row.height).min(row.width);
        Some(Rect::new(
            row.x + row.width as i32 - side as i32 - 4,
            row.y + (row.height.saturating_sub(side) / 2) as i32,
            side,
            side,
        ))
    }

    /// The region the stack actually occupies: the bottom of the control's rectangle, as many
    /// rows tall as there are toasts, clamped to the rectangle.
    ///
    /// # Why the stack is not the rectangle
    ///
    /// A toast stack is a column of fixed-height bars. Filling the whole rectangle made a
    /// 240x120 census cell a 120 px panel behind two 26 px rows — a surface much larger than
    /// the toasts it holds, whose top edge was painted but never occupied. Anchoring the band to
    /// the bottom and sizing it from the rows is the same derivation the draw and the hit test
    /// read, so the panel and the rows it contains cannot disagree.
    ///
    /// An empty stack still claims **one row**: the control is a drop target that has to show
    /// where a toast will land, and an empty container that paints nothing is indistinguishable
    /// from a control that was never laid out (the census's P1 defect).
    fn stack_band(&self) -> Rect {
        let rows = (self.toasts.len() as u32).max(1);
        let height = rows.saturating_mul(self.row_height);
        let rect = self.geometry();
        // A left/right anchor is only meaningful when the band is narrower than the control;
        // the cap keeps a banner from spanning an ultrawide window.
        let width = rect.width.min(360);
        let x = match self.anchor {
            ToastAnchor::BottomLeft | ToastAnchor::TopLeft => rect.x,
            ToastAnchor::BottomRight | ToastAnchor::TopRight => {
                rect.x + rect.width.saturating_sub(width) as i32
            }
        };
        let y = match self.anchor {
            ToastAnchor::BottomLeft | ToastAnchor::BottomRight => {
                rect.y + rect.height.saturating_sub(height.min(rect.height)) as i32
            }
            ToastAnchor::TopLeft | ToastAnchor::TopRight => rect.y,
        };
        Rect::new(x, y, width, height.min(rect.height))
    }

    fn row_at(&self, pos: Point) -> Option<usize> {
        (self.scroll_offset..self.toasts.len())
            .find(|&index| self.row_rect(index).contains_point(pos))
    }

    /// The rectangle a toast row is painted at.
    fn row_rect(&self, index: usize) -> Rect {
        let band = self.stack_band();
        let visual_order = self.toasts.len().saturating_sub(1 + index);
        let y = match self.stacking_direction {
            ToastStackingDirection::NewestAtBottom => {
                let bottom = band.y + band.height as i32;
                bottom - ((visual_order + 1) as i32 * self.row_height as i32)
            }
            ToastStackingDirection::NewestAtTop => {
                band.y + visual_order as i32 * self.row_height as i32
            }
        };
        Rect::new(
            band.x + 4,
            y + 2,
            band.width.saturating_sub(8),
            self.row_height.saturating_sub(4),
        )
    }
}

impl Widget for ToastStack {
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
        crate::core::Size::new(320, 200)
    }

    /// Advances the newest toast's rise-in; `true` while it still owes frames.
    fn tick(&mut self, delta_ms: u32) -> bool {
        let mut moving = self.reveal.tick(delta_ms);
        self.now_ms = self.now_ms.saturating_add(delta_ms as u64);
        let expired = self.expire_toasts(self.now_ms);
        moving |= expired > 0;
        if self.closing.is_some() {
            moving |= self.exit.tick(delta_ms);
            if !self.exit.is_animating() {
                self.flush_pending_dismissal();
                return true;
            }
        }
        moving
    }

    fn is_animating(&self) -> bool {
        self.reveal.is_animating() || self.exit.is_animating()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "toast_activated" => {
                Some(EventSignalRef::mapped("toast_activated", &self.toast_activated, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "toast_dismissed" => {
                Some(EventSignalRef::mapped("toast_dismissed", &self.toast_dismissed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "toast_action" => {
                Some(EventSignalRef::mapped("toast_action", &self.toast_action, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            "toast_expired" => {
                Some(EventSignalRef::mapped("toast_expired", &self.toast_expired, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `ToastStack`'s property contract.
///
/// `toast_count` and `selected_id` describe the live stack. `row_height` is the
/// one writable name here: the stack lays its toasts out bottom-up from the
/// geometry it was given, so its row height must stay adjustable when a host
/// changes density.
impl WidgetProperties for ToastStack {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "toast_count" => Ok(CapabilityValue::UInt(self.toasts().len() as u64)),
            "selected_id" => match self.selected_id() {
                Some(id) => Ok(CapabilityValue::String(id.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            "row_height" => Ok(CapabilityValue::UInt(self.row_height as u64)),
            "max_visible" => Ok(CapabilityValue::UInt(self.max_visible as u64)),
            "anchor" => Ok(CapabilityValue::String(
                match self.anchor {
                    ToastAnchor::BottomLeft => "bottom_left",
                    ToastAnchor::BottomRight => "bottom_right",
                    ToastAnchor::TopLeft => "top_left",
                    ToastAnchor::TopRight => "top_right",
                }
                .to_string(),
            )),
            "stacking_direction" => Ok(CapabilityValue::String(
                match self.stacking_direction {
                    ToastStackingDirection::NewestAtBottom => "newest_at_bottom",
                    ToastStackingDirection::NewestAtTop => "newest_at_top",
                }
                .to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "row_height" => match value {
                CapabilityValue::UInt(height) => {
                    let height =
                        u32::try_from(height).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                    self.row_height = height.max(1);
                    self.base.request_layout();
                    self.base.request_redraw();
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "toast_count" | "selected_id" => Err(CapabilityAccessError::ReadOnlyProperty),
            "max_visible" => match value {
                CapabilityValue::UInt(max) => {
                    self.set_max_visible(usize::try_from(max).unwrap_or(usize::MAX));
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "anchor" => match value {
                CapabilityValue::String(token) => {
                    let anchor = match token.as_str() {
                        "bottom_left" => ToastAnchor::BottomLeft,
                        "bottom_right" => ToastAnchor::BottomRight,
                        "top_left" => ToastAnchor::TopLeft,
                        "top_right" => ToastAnchor::TopRight,
                        _ => return Err(CapabilityAccessError::OutOfRange),
                    };
                    self.set_anchor(anchor);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "stacking_direction" => match value {
                CapabilityValue::String(token) => {
                    let direction = match token.as_str() {
                        "newest_at_bottom" => ToastStackingDirection::NewestAtBottom,
                        "newest_at_top" => ToastStackingDirection::NewestAtTop,
                        _ => return Err(CapabilityAccessError::OutOfRange),
                    };
                    self.set_stacking_direction(direction);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "toast_count",
            "selected_id",
            "row_height",
            "max_visible",
            "anchor",
            "stacking_direction",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `toast_stack` publishes.
    ///
    /// `clear`, `activate_selected` and `dismiss_selected` are payload-free and
    /// map onto the widget's real methods; the two `*_selected` actions report
    /// `OutOfRange` when no toast is selected. `push` needs a whole `ToastItem`,
    /// and `select_index` an index, so those are answered as needing a payload.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "activate_selected" => {
                if self.activate_selected() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "dismiss_selected" => {
                if self.dismiss_selected() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "push" | "select_index" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ToastStack {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.row_at(*pos) {
                    let row = self.row_rect(index);
                    // The close button occupies the row's trailing corner and runs the same
                    // removal path as the Delete key (SP-72).
                    if self.close_rect_for(row).is_some_and(|close| close.contains_point(*pos)) {
                        let _ = self.select_index(index);
                        let _ = self.dismiss_selected();
                        return;
                    }
                    // An action button occupies the trailing band of a row, so a press there
                    // runs the action instead of merely activating the toast.
                    let has_action =
                        self.toasts.get(index).is_some_and(|item| item.action_label.is_some());
                    if has_action {
                        let label = self
                            .toasts
                            .get(index)
                            .and_then(|item| item.action_label.as_deref())
                            .unwrap_or("");
                        let font = effective_font(self.base.style());
                        let right = self
                            .close_rect_for(row)
                            .map(|close| close.x - 4)
                            .unwrap_or(row.x + row.width as i32);
                        let action = self.action_rect_for(row, label, font, right);
                        if action.contains_point(*pos) {
                            let id = self.toasts[index].id.clone();
                            self.toast_action.emit(id);
                            return;
                        }
                    }
                    let _ = self.select_index(index);
                    let _ = self.activate_selected();
                }
            }
            Event::Wheel { delta, .. } => {
                if delta.y != 0 {
                    let step = if delta.y > 0 { 1isize } else { -1 };
                    let next = (self.scroll_offset as isize + step).max(0) as usize;
                    self.set_scroll_offset(next);
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                38 => {
                    if let Some(index) = self.selected_index {
                        if index > 0 {
                            let _ = self.select_index(index - 1);
                        }
                    }
                }
                40 => {
                    if let Some(index) = self.selected_index {
                        if index + 1 < self.toasts.len() {
                            let _ = self.select_index(index + 1);
                        }
                    }
                }
                13 => {
                    let _ = self.activate_selected();
                }
                46 => {
                    let _ = self.dismiss_selected();
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for ToastStack {
    fn draw(&mut self, context: &mut RenderContext) {
        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("toast_stack");
        // `toast_stack` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The stack's own fill is therefore a step toward
        // the foreground, so it reads as a surface of its own.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(250, 251, 253));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(resolved));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let background = resolved.blend(&text_color, 0.08);
        // Row chrome is derived from the resolved pair: the selected row reads as
        // tinted toward the text colour, an ordinary row more faintly.
        let selected_background = background.blend(&text_color, 0.14);
        let row_background = background.blend(&text_color, 0.05);
        let row_border = background.blend(&text_color, 0.2);

        // ── The stack actually painted ──
        //
        // The panel is the bottom-anchored region the rows occupy, not the whole rectangle: a
        // 240x120 census cell was a 120 px panel behind two 26 px rows. An empty stack paints
        // nothing, because a stack with no toasts has no bar to show.
        let band = self.stack_band();
        if band.height == 0 {
            return;
        }
        context.fill_rect(band, background);
        context.draw_rect(band, border);

        // Bottom-up: the newest toast is the lowest row, which is the end the stack grows from.
        let font = effective_font(style);
        // The four severity tints are derived once for the frame; each row used to take its own
        // semantic-token lookup and blend (SP-P76).
        let badges = [
            Self::badge_color(ToastLevel::Info, background, text_color),
            Self::badge_color(ToastLevel::Success, background, text_color),
            Self::badge_color(ToastLevel::Warning, background, text_color),
            Self::badge_color(ToastLevel::Error, background, text_color),
        ];
        let close_ink = text_color.with_alpha(150);
        for (index, item) in self.toasts.iter().enumerate() {
            if index < self.scroll_offset {
                continue;
            }
            let row = self.row_rect(index);
            if row.y + row.height as i32 <= band.y || row.y >= band.y + band.height as i32 {
                continue;
            }
            // # Why only the newest row moves
            //
            // The reveal is re-aimed on every `push`, so applying it to every row would make the
            // whole stack rise whenever one is added — the settled toasts would appear to be
            // re-arriving, which says the opposite of what happened. Only the row that was just
            // pushed is still on its way in, so only that one is offset; the rest are at rest by
            // definition.
            let is_arriving = index + 1 == self.toasts.len();
            let row = if is_arriving && self.reveal.is_animating() {
                // Rises from `row_height` below its seat to its seat, so a tall row travels
                // proportionally further than a short one rather than a fixed number of pixels.
                let offset = (self.row_height as f32 * (1.0 - self.reveal.value())) as i32;
                Rect::new(row.x, row.y + offset, row.width, row.height)
            } else if self.closing == Some(index) {
                // Falls back out of its seat along the same path it arrived on.
                let offset = (self.row_height as f32 * (1.0 - self.exit.value())) as i32;
                Rect::new(row.x, row.y + offset, row.width, row.height)
            } else {
                row
            };
            let bg = if self.selected_index == Some(index) {
                selected_background
            } else {
                row_background
            };
            let bg = if self.translucent { bg.with_alpha(220) } else { bg };
            context.fill_rect(row, bg);
            context.draw_rect(row, row_border);

            // The badge is a *state* indicator, so it reads the theme's semantic
            // tokens rather than a literal colour per level.
            let badge = match item.level {
                ToastLevel::Info => badges[0],
                ToastLevel::Success => badges[1],
                ToastLevel::Warning => badges[2],
                ToastLevel::Error => badges[3],
            };
            context.fill_rect(Rect::new(row.x + 6, row.y + 9, 8, 8), badge);
            // The action band is measured from its own label (cached) and sits left of the row's
            // close button; the message is fitted into what is left. The action used to be a fixed
            // 72 px tail regardless of its label, and the message used a plain `draw_text`, so a
            // long message ran under the action (and into the next row's frame) while a short
            // action wasted the reserved tail.
            let close = self.close_rect_for(row);
            let trailing_right = close.map(|close| close.x - 4).unwrap_or(row.x + row.width as i32);
            let action = item
                .action_label
                .as_deref()
                .map(|label| self.action_rect_for(row, label, font, trailing_right));
            let message_right =
                action.map(|rect| rect.x - row.x - 4).unwrap_or(trailing_right - row.x - 4).max(0);
            context.draw_text_fitted_cow(
                Rect::new(row.x + 20, row.y, message_right as u32, row.height),
                &item.message,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
            if let (Some(label), Some(action)) = (item.action_label.as_deref(), action) {
                // The action sits in the measured trailing band, right-aligned so its label
                // ends where the row's padding does.
                context.draw_text_line_cow(
                    action,
                    label,
                    font,
                    text_color,
                    HorizontalAlignment::Right,
                );
            }
            if let Some(close) = close {
                // A pair of strokes, the same affordance the single-toast control paints.
                context.draw_line_stroke(
                    Point::new(close.x + 3, close.y + 3),
                    Point::new(close.x + close.width as i32 - 3, close.y + close.height as i32 - 3),
                    close_ink,
                    1,
                );
                context.draw_line_stroke(
                    Point::new(close.x + close.width as i32 - 3, close.y + 3),
                    Point::new(close.x + 3, close.y + close.height as i32 - 3),
                    close_ink,
                    1,
                );
            }
        }
    }
}
