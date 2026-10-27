//! `ToastHost` — a bounded toast queue with expiry and overflow policy.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::{expect_u32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::special_widgets::toast::{Toast, ToastItem, ToastLevel};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{expect_token, surface_ink};

// ---------------------------------------------------------------------------
// COMP-12 ToastHost
// ---------------------------------------------------------------------------

/// Where a toast host anchors its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastAnchor {
    /// Top-left.
    TopLeft,
    /// Top-centre.
    TopCenter,
    /// Top-right.
    TopRight,
    /// Bottom-left, the default.
    #[default]
    BottomLeft,
    /// Bottom-centre.
    BottomCenter,
    /// Bottom-right.
    BottomRight,
}

impl ToastAnchor {
    /// The published token.
    pub fn as_str(self) -> &'static str {
        match self {
            ToastAnchor::TopLeft => "top_left",
            ToastAnchor::TopCenter => "top_center",
            ToastAnchor::TopRight => "top_right",
            ToastAnchor::BottomLeft => "bottom_left",
            ToastAnchor::BottomCenter => "bottom_center",
            ToastAnchor::BottomRight => "bottom_right",
        }
    }
    /// Parses the published token.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "top_left" => Some(ToastAnchor::TopLeft),
            "top_center" => Some(ToastAnchor::TopCenter),
            "top_right" => Some(ToastAnchor::TopRight),
            "bottom_left" => Some(ToastAnchor::BottomLeft),
            "bottom_center" => Some(ToastAnchor::BottomCenter),
            "bottom_right" => Some(ToastAnchor::BottomRight),
            _ => None,
        }
    }
    fn is_top(self) -> bool {
        matches!(self, ToastAnchor::TopLeft | ToastAnchor::TopCenter | ToastAnchor::TopRight)
    }
}

/// Which end of a toast host receives the newest row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastStackingDirection {
    /// Newest toast at the top of the stack.
    #[default]
    NewestAtTop,
    /// Newest toast at the bottom of the stack.
    NewestAtBottom,
}

impl ToastStackingDirection {
    /// The published token.
    pub fn as_str(self) -> &'static str {
        match self {
            ToastStackingDirection::NewestAtTop => "newest_at_top",
            ToastStackingDirection::NewestAtBottom => "newest_at_bottom",
        }
    }
    /// Parses the published token.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "newest_at_top" => Some(ToastStackingDirection::NewestAtTop),
            "newest_at_bottom" => Some(ToastStackingDirection::NewestAtBottom),
            _ => None,
        }
    }
}

/// What a toast host does when a push arrives and every slot is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastOverflowPolicy {
    /// Discard the oldest visible toast and show the new one.
    #[default]
    DropOldest,
    /// Reject the new toast.
    DropNewest,
    /// Hold the new toast until a visible slot frees up.
    Queue,
}

impl ToastOverflowPolicy {
    /// The published token.
    pub fn as_str(self) -> &'static str {
        match self {
            ToastOverflowPolicy::DropOldest => "drop_oldest",
            ToastOverflowPolicy::DropNewest => "drop_newest",
            ToastOverflowPolicy::Queue => "queue",
        }
    }
    /// Parses the published token.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "drop_oldest" => Some(ToastOverflowPolicy::DropOldest),
            "drop_newest" => Some(ToastOverflowPolicy::DropNewest),
            "queue" => Some(ToastOverflowPolicy::Queue),
            _ => None,
        }
    }
}

/// A toast owned by a [`ToastHost`]: the message plus its host-assigned id and deadline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedToast {
    id: u64,
    message: String,
    level: ToastLevel,
    duration_ms: u32,
    deadline_ms: u64,
}

impl HostedToast {
    /// The host-assigned id.
    pub fn id(&self) -> u64 {
        self.id
    }
    /// The message.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// The severity level.
    pub fn level(&self) -> ToastLevel {
        self.level
    }
    /// The duration this toast was admitted with.
    pub fn duration_ms(&self) -> u32 {
        self.duration_ms
    }
    /// The monotonic time at which it expires.
    pub fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }
}

/// Height of one host-owned toast row.
const TOAST_ROW_HEIGHT: u32 = 38;
/// Gap between toast rows.
const TOAST_ROW_GAP: u32 = 6;
/// Inset from the anchored edge.
const TOAST_INSET: u32 = 8;

/// A toast queue with visible/queued caps, expiry and a bounded overflow policy.
///
/// `Toast` and `ToastItem` provide the payload; this control provides the lifecycle their own
/// documents assign to the host: automatic expiry, a queue cap, and an explicit overflow
/// policy. It paints its visible rows and dismisses the row clicked.
///
/// A host that shares a screen edge with a `Snackbar` or a `ToastStack` joins their `OverlayStack`
/// through [`Self::set_overlay_offset`], so the controls stack instead of painting over each other
/// (SP-124). With no stack the offset stays zero and the picture is unchanged.
pub struct ToastHost {
    base: BaseWidget,
    visible: Vec<HostedToast>,
    queued: Vec<HostedToast>,
    max_visible: usize,
    max_queued: usize,
    default_duration_ms: u32,
    anchor: ToastAnchor,
    stacking_direction: ToastStackingDirection,
    overflow_policy: ToastOverflowPolicy,
    next_id: u64,
    now_ms: u64,
    /// A host-assigned offset that clears the controls reserved before it (SP-124).
    ///
    /// Zero is the unstacked picture, so a host that never joins an `OverlayStack` paints
    /// exactly the same rows as before.
    overlay_offset: i32,
    /// Emitted with the id of a toast removed by its deadline.
    pub toast_expired: Signal1<u64>,
    /// Emitted with the id of a toast removed by [`Self::dismiss`], a policy drop or
    /// [`Self::dismiss_all`].
    pub toast_dismissed: Signal1<u64>,
    /// Emitted with the number of toasts already waiting when a push was rejected.
    pub queue_overflow: Signal1<usize>,
}

impl ToastHost {
    /// Creates an empty host with five visible rows and a ten-entry queue.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToastHost, geometry, "ToastHost"),
            visible: Vec::new(),
            queued: Vec::new(),
            max_visible: 5,
            max_queued: 10,
            default_duration_ms: 3000,
            anchor: ToastAnchor::BottomLeft,
            stacking_direction: ToastStackingDirection::NewestAtTop,
            overflow_policy: ToastOverflowPolicy::DropOldest,
            next_id: 1,
            now_ms: 0,
            overlay_offset: 0,
            toast_expired: Signal1::new(),
            toast_dismissed: Signal1::new(),
            queue_overflow: Signal1::new(),
        }
    }

    /// Pushes a `Toast` using the host's last observed clock.
    ///
    /// Call [`Self::expire_toasts`] with the host clock once per frame, or use
    /// [`Self::push_at`] to supply the timestamp explicitly.
    pub fn push(&mut self, toast: Toast) -> Option<u64> {
        self.push_at(toast, self.now_ms)
    }

    /// Pushes a `Toast` at an explicit monotonic time.
    pub fn push_at(&mut self, toast: Toast, now_ms: u64) -> Option<u64> {
        self.now_ms = self.now_ms.max(now_ms);
        let duration = if toast.ttl_ms() == 0 { self.default_duration_ms } else { toast.ttl_ms() };
        let hosted = self.hosted(toast.message().to_string(), toast.level(), duration);
        self.admit(hosted)
    }

    /// Pushes an existing `ToastItem` at the host's last observed clock.
    ///
    /// The item's string `id` is not used as the host id: the queue's signals carry the
    /// stable numeric id the host assigns, so `toast_expired` and `toast_dismissed` identify
    /// the same entry they admitted.
    pub fn push_item(&mut self, item: ToastItem) -> Option<u64> {
        let duration = if item.ttl_ms == 0 { self.default_duration_ms } else { item.ttl_ms };
        let hosted = self.hosted(item.message, item.level, duration);
        self.admit(hosted)
    }

    /// Expires every visible toast whose deadline has passed, then promotes queued entries.
    ///
    /// Returns the number expired.
    pub fn expire_toasts(&mut self, now_ms: u64) -> usize {
        self.now_ms = self.now_ms.max(now_ms);
        let now = self.now_ms;
        let mut expired = Vec::new();
        self.visible.retain(|toast| {
            if toast.deadline_ms <= now {
                expired.push(toast.id);
                false
            } else {
                true
            }
        });
        for id in &expired {
            self.toast_expired.emit(*id);
        }
        self.promote();
        self.base.request_redraw();
        expired.len()
    }

    /// Dismisses one toast (visible or queued) by id.
    pub fn dismiss(&mut self, id: u64) -> bool {
        let removed = self.remove_id(id);
        if removed {
            self.toast_dismissed.emit(id);
            self.promote();
            self.base.request_redraw();
        }
        removed
    }

    /// Dismisses every visible and queued toast. Returns how many were removed.
    pub fn dismiss_all(&mut self) -> usize {
        let mut ids = Vec::new();
        for toast in self.visible.drain(..) {
            ids.push(toast.id);
        }
        for toast in self.queued.drain(..) {
            ids.push(toast.id);
        }
        for id in &ids {
            self.toast_dismissed.emit(*id);
        }
        self.base.request_redraw();
        ids.len()
    }

    /// Returns the visible rows, oldest first.
    pub fn visible_toasts(&self) -> &[HostedToast] {
        &self.visible
    }

    /// Returns the waiting rows, oldest first.
    pub fn queued_toasts(&self) -> &[HostedToast] {
        &self.queued
    }

    /// Returns the host's last observed clock.
    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    /// Sets the visible-row cap. Extras move to the queue, or overflow if it is full.
    pub fn set_max_visible(&mut self, max_visible: usize) {
        let max_visible = max_visible.max(1);
        self.max_visible = max_visible;
        while self.visible.len() > self.max_visible {
            let extra = self.visible.remove(0);
            if self.queued.len() < self.max_queued {
                self.queued.insert(0, extra);
            } else {
                self.toast_dismissed.emit(extra.id);
                self.queue_overflow.emit(self.queued.len() + 1);
            }
        }
        self.base.request_redraw();
    }

    /// Returns the visible-row cap.
    pub fn max_visible(&self) -> usize {
        self.max_visible
    }

    /// Sets the waiting-row cap; the new limit applies to future pushes.
    pub fn set_max_queued(&mut self, max_queued: usize) {
        self.max_queued = max_queued;
    }

    /// Returns the waiting-row cap.
    pub fn max_queued(&self) -> usize {
        self.max_queued
    }

    /// Sets the duration used when a pushed toast has no ttl of its own.
    pub fn set_default_duration_ms(&mut self, duration_ms: u32) {
        self.default_duration_ms = duration_ms.max(100);
    }

    /// Returns the default duration.
    pub fn default_duration_ms(&self) -> u32 {
        self.default_duration_ms
    }

    /// Sets the anchor corner.
    pub fn set_anchor(&mut self, anchor: ToastAnchor) {
        if self.anchor != anchor {
            self.anchor = anchor;
            self.base.request_redraw();
        }
    }

    /// Returns the anchor.
    pub fn anchor(&self) -> ToastAnchor {
        self.anchor
    }

    /// Sets the stacking direction.
    pub fn set_stacking_direction(&mut self, direction: ToastStackingDirection) {
        if self.stacking_direction != direction {
            self.stacking_direction = direction;
            self.base.request_redraw();
        }
    }

    /// Returns the stacking direction.
    pub fn stacking_direction(&self) -> ToastStackingDirection {
        self.stacking_direction
    }

    /// Sets the overflow policy.
    pub fn set_overflow_policy(&mut self, policy: ToastOverflowPolicy) {
        self.overflow_policy = policy;
    }

    /// Returns the overflow policy.
    pub fn overflow_policy(&self) -> ToastOverflowPolicy {
        self.overflow_policy
    }

    /// The host-assigned offset that clears another overlay control (SP-124).
    pub fn overlay_offset(&self) -> i32 {
        self.overlay_offset
    }

    /// Moves the host's rows clear of the controls stacked before it (SP-124).
    ///
    /// The sign follows the anchor: a bottom-anchored host rises by `offset`, a top-anchored one
    /// descends. Zero leaves every row where it was, so a single host is unaffected.
    pub fn set_overlay_offset(&mut self, offset: i32) {
        if self.overlay_offset == offset {
            return;
        }
        self.overlay_offset = offset;
        self.base.request_redraw();
    }

    /// The vertical space the visible rows currently claim in a host's stack.
    ///
    /// An empty host claims nothing: unlike `ToastStack` it paints no resting row, so reserving
    /// space for it would push a neighbouring snackbar up for an invisible control.
    pub fn overlay_height(&self) -> u32 {
        let rows = self.visible.len() as u32;
        if rows == 0 {
            return 0;
        }
        rows.saturating_mul(TOAST_ROW_HEIGHT)
            .saturating_add(rows.saturating_sub(1).saturating_mul(TOAST_ROW_GAP))
    }

    /// Builds the host's copy of a toast with the next id and deadline.
    fn hosted(&mut self, message: String, level: ToastLevel, duration_ms: u32) -> HostedToast {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let duration_ms = duration_ms.max(100);
        HostedToast {
            id,
            message,
            level,
            duration_ms,
            deadline_ms: self.now_ms.saturating_add(u64::from(duration_ms)),
        }
    }

    /// Applies the overflow policy and admits or rejects `toast`.
    fn admit(&mut self, toast: HostedToast) -> Option<u64> {
        let id = toast.id;
        if self.visible.len() < self.max_visible {
            self.visible.push(toast);
            self.base.request_redraw();
            return Some(id);
        }
        match self.overflow_policy {
            ToastOverflowPolicy::Queue => {
                if self.queued.len() < self.max_queued {
                    self.queued.push(toast);
                    Some(id)
                } else {
                    self.queue_overflow.emit(self.queued.len() + 1);
                    None
                }
            }
            ToastOverflowPolicy::DropNewest => {
                self.queue_overflow.emit(self.visible.len() + self.queued.len() + 1);
                None
            }
            ToastOverflowPolicy::DropOldest => {
                if self.visible.is_empty() {
                    self.queue_overflow.emit(1);
                    return None;
                }
                let dropped = self.visible.remove(0);
                self.toast_dismissed.emit(dropped.id);
                self.visible.push(toast);
                self.base.request_redraw();
                Some(id)
            }
        }
    }

    /// Removes `id` from whichever list holds it.
    fn remove_id(&mut self, id: u64) -> bool {
        if let Some(position) = self.visible.iter().position(|toast| toast.id == id) {
            self.visible.remove(position);
            return true;
        }
        if let Some(position) = self.queued.iter().position(|toast| toast.id == id) {
            self.queued.remove(position);
            return true;
        }
        false
    }

    /// Moves queued toasts into free visible slots, restarting their clock.
    fn promote(&mut self) {
        while self.visible.len() < self.max_visible && !self.queued.is_empty() {
            let mut toast = self.queued.remove(0);
            toast.deadline_ms = self.now_ms.saturating_add(u64::from(toast.duration_ms));
            self.visible.push(toast);
        }
    }

    /// The rectangle of visible row `index`.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.visible.len() {
            return None;
        }
        let rect = self.geometry();
        let count = self.visible.len();
        // `stack_index` is measured from the visual top of the stack, so the same stacking
        // direction reads the same way whether the host is anchored at the top or the bottom.
        let stack_index = match self.stacking_direction {
            ToastStackingDirection::NewestAtTop => count - 1 - index,
            ToastStackingDirection::NewestAtBottom => index,
        };
        let width = rect.width.saturating_sub(TOAST_INSET * 2).max(1);
        let x = match self.anchor {
            ToastAnchor::TopLeft | ToastAnchor::BottomLeft => rect.x + TOAST_INSET as i32,
            ToastAnchor::TopCenter | ToastAnchor::BottomCenter => {
                rect.center().x - width as i32 / 2
            }
            ToastAnchor::TopRight | ToastAnchor::BottomRight => {
                rect.right() - TOAST_INSET as i32 - width as i32
            }
        };
        let pitch = TOAST_ROW_HEIGHT + TOAST_ROW_GAP;
        let y = if self.anchor.is_top() {
            rect.y + TOAST_INSET as i32 + (stack_index as u32 * pitch) as i32
        } else {
            let below = (count - 1 - stack_index) as u32;
            rect.bottom() - TOAST_INSET as i32 - (TOAST_ROW_HEIGHT + below * pitch) as i32
        };
        // A host-stacked overlay moves away from the edge it is anchored to; at rest the offset
        // is zero and this is the same row the unstacked host painted.
        let y = if self.anchor.is_top() {
            y.saturating_add(self.overlay_offset)
        } else {
            y.saturating_sub(self.overlay_offset)
        };
        Some(Rect::new(x, y, width, TOAST_ROW_HEIGHT))
    }
}

/// The theme colour for a severity stripe.
fn toast_level_color(level: ToastLevel) -> Color {
    match level {
        ToastLevel::Info => Color::PRIMARY,
        ToastLevel::Success => Color::SUCCESS,
        ToastLevel::Warning => Color::WARNING,
        ToastLevel::Error => Color::ERROR,
    }
}

impl EventHandler for ToastHost {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            let target = (0..self.visible.len())
                .find(|&index| self.row_rect(index).is_some_and(|rect| rect.contains(*pos)))
                .map(|index| self.visible[index].id);
            if let Some(id) = target {
                self.dismiss(id);
            }
        }
    }
}

impl Draw for ToastHost {
    fn draw(&mut self, context: &mut RenderContext) {
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let font = effective_font(style);
        if self.visible.is_empty() {
            // The empty queue still paints its resting slot, so it is visible and theme-aware.
            let rect = self.geometry();
            context.fill_rect(rect, surface.with_alpha(32));
            context.draw_rect(rect, surface.blend(&ink, 0.18));
            return;
        }
        for index in 0..self.visible.len() {
            let Some(row) = self.row_rect(index) else {
                continue;
            };
            let toast = &self.visible[index];
            context.fill_rect(row, surface.blend(&ink, 0.10));
            context.draw_rect(row, surface.blend(&ink, 0.30));
            let stripe = Rect::new(row.x, row.y, 4.min(row.width), row.height);
            context.fill_rect(stripe, toast_level_color(toast.level()));
            let text = Rect::new(
                stripe.x + stripe.width as i32 + 8,
                row.y,
                row.width.saturating_sub(stripe.width + 16),
                row.height,
            );
            context.draw_text_fitted_cow(
                text,
                &toast.message,
                font,
                ink,
                crate::core::HorizontalAlignment::Left,
            );
        }
    }
}

impl Widget for ToastHost {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        Size::new(320, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "toast_expired" => {
                Some(EventSignalRef::mapped("toast_expired", &self.toast_expired, |value| {
                    CapabilityValue::UInt(*value)
                }))
            }
            "toast_dismissed" => {
                Some(EventSignalRef::mapped("toast_dismissed", &self.toast_dismissed, |value| {
                    CapabilityValue::UInt(*value)
                }))
            }
            "queue_overflow" => {
                Some(EventSignalRef::mapped("queue_overflow", &self.queue_overflow, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for ToastHost {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "max_visible" => Ok(CapabilityValue::UInt(self.max_visible as u64)),
            "max_queued" => Ok(CapabilityValue::UInt(self.max_queued as u64)),
            "default_duration_ms" => Ok(CapabilityValue::UInt(u64::from(self.default_duration_ms))),
            "anchor" => Ok(CapabilityValue::String(self.anchor.as_str().to_string())),
            "stacking_direction" => {
                Ok(CapabilityValue::String(self.stacking_direction.as_str().to_string()))
            }
            "overflow_policy" => {
                Ok(CapabilityValue::String(self.overflow_policy.as_str().to_string()))
            }
            _ => base_property_get(self, name),
        }
    }
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "max_visible" => {
                self.set_max_visible(expect_usize(value)?);
                Ok(())
            }
            "max_queued" => {
                self.set_max_queued(expect_usize(value)?);
                Ok(())
            }
            "default_duration_ms" => {
                self.set_default_duration_ms(expect_u32(value)?);
                Ok(())
            }
            "anchor" => {
                self.set_anchor(expect_token(value, ToastAnchor::from_name)?);
                Ok(())
            }
            "stacking_direction" => {
                self.set_stacking_direction(expect_token(
                    value,
                    ToastStackingDirection::from_name,
                )?);
                Ok(())
            }
            "overflow_policy" => {
                self.set_overflow_policy(expect_token(value, ToastOverflowPolicy::from_name)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "max_visible",
            "max_queued",
            "default_duration_ms",
            "anchor",
            "stacking_direction",
            "overflow_policy",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::special_widgets::snackbar::{OverlayKind, OverlayStack};

    #[test]
    fn a_host_stack_offset_moves_the_rows_without_changing_the_anchor() {
        let mut host = ToastHost::new(Rect::new(0, 0, 320, 200));
        host.push_item(ToastItem::new("a", "first", ToastLevel::Info, 3_000));
        let unstacked = host.row_rect(0).expect("one visible row");
        assert_eq!(host.overlay_height(), TOAST_ROW_HEIGHT);

        host.set_overlay_offset(24);
        assert_eq!(host.overlay_offset(), 24);
        let shifted = host.row_rect(0).expect("one visible row");
        assert_eq!(shifted.y, unstacked.y - 24, "a bottom anchor rises by the offset");

        host.set_anchor(ToastAnchor::TopLeft);
        host.set_overlay_offset(0);
        let top_unstacked = host.row_rect(0).expect("one visible row");
        host.set_overlay_offset(24);
        assert_eq!(
            host.row_rect(0).expect("one visible row").y,
            top_unstacked.y + 24,
            "a top anchor descends by the offset"
        );
    }

    /// The shared stack seats a toast host clear of a snackbar (SP-124).
    #[test]
    fn the_overlay_stack_seats_a_toast_host_clear_of_a_snackbar() {
        let mut host = ToastHost::new(Rect::new(0, 0, 320, 200));
        host.push_item(ToastItem::new("a", "first", ToastLevel::Info, 3_000));

        let mut stack = OverlayStack::new();
        stack.reserve(OverlayKind::Snackbar, 48);
        stack.reserve(OverlayKind::ToastHost, host.overlay_height());
        assert_eq!(stack.offset(OverlayKind::Snackbar), 0);
        assert_eq!(stack.offset(OverlayKind::ToastHost), 54, "height plus the gap");

        let unstacked = host.row_rect(0).expect("one visible row");
        host.set_overlay_offset(stack.offset(OverlayKind::ToastHost));
        assert_eq!(host.row_rect(0).expect("one visible row").y, unstacked.y - 54);
    }

    #[test]
    fn a_toast_leaves_the_visible_queue_on_its_deadline() {
        let mut host = ToastHost::new(Rect::new(0, 0, 320, 200));
        let mut toast = Toast::new(Rect::new(0, 0, 200, 48), "Saved");
        toast.set_ttl_ms(1_000);
        let id = host.push_at(toast, 10).expect("the queue admits a toast");
        assert_eq!(host.visible_toasts().len(), 1);
        // The deadline is one second after admission, so half a second changes nothing.
        assert_eq!(host.expire_toasts(500), 0);
        assert_eq!(host.expire_toasts(1_010), 1);
        assert!(host.visible_toasts().is_empty());
        // An expired toast cannot be dismissed a second time.
        assert!(!host.dismiss(id));
    }
}
