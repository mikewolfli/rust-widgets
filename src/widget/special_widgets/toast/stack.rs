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
use crate::widget::metrics::{effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Where a toast stack anchors its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastAnchor {
    /// Bottom-left, the default.
    #[default]
    BottomLeft,
    /// Top-right, the placement a notification banner uses.
    TopRight,
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
    /// How many of the oldest rows are scrolled out of view.
    scroll_offset: usize,
    /// Whether rows paint a translucent (vibrant) fill.
    translucent: bool,
    /// Emitted when toast is activated.
    pub toast_activated: Signal1<String>,
    /// Emitted when toast is dismissed.
    pub toast_dismissed: Signal1<String>,
    /// Emitted with the toast id when its action button is pressed.
    pub toast_action: Signal1<String>,
}

impl ToastStack {
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
            scroll_offset: 0,
            translucent: false,
            toast_activated: Signal1::new(),
            toast_dismissed: Signal1::new(),
            toast_action: Signal1::new(),
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
        self.toasts.push(item);
        self.selected_index = Some(self.toasts.len() - 1);
        // The new row starts fully below its seat and rises into it. `jump_to(false)` before
        // aiming at open, so the first painted frame is the start of the movement rather than a
        // snap to the middle of it.
        self.reveal.jump_to(false);
        self.reveal.set_open(true);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears all toasts.
    pub fn clear(&mut self) {
        self.toasts.clear();
        self.selected_index = None;
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
        match self.anchor {
            ToastAnchor::BottomLeft => ControlMetrics::bottom_band(self.geometry(), height),
            ToastAnchor::TopRight => {
                let rect = self.geometry();
                let width = rect.width;
                let height = height.min(rect.height);
                Rect::new(rect.x, rect.y, width, height)
            }
        }
    }

    fn row_at(&self, pos: Point) -> Option<usize> {
        let band = self.stack_band();
        if pos.x < band.x
            || pos.x >= band.x + band.width as i32
            || pos.y < band.y
            || pos.y >= band.y + band.height as i32
        {
            return None;
        }

        let bottom = band.y + band.height as i32;
        for index in 0..self.toasts.len() {
            let top = bottom - ((index + 1) as i32 * self.row_height as i32);
            if pos.y >= top && pos.y < top + self.row_height as i32 {
                return Some(self.toasts.len() - 1 - index);
            }
        }
        None
    }

    /// The rectangle a toast row is painted at.
    fn row_rect(&self, index: usize) -> Rect {
        let band = self.stack_band();
        let bottom = band.y + band.height as i32;
        let visual_order = self.toasts.len().saturating_sub(1 + index);
        let y = bottom - ((visual_order + 1) as i32 * self.row_height as i32);
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
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["toast_count", "selected_id", "row_height", BASE_PROPERTY_NAMES]
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
                    // An action button occupies the trailing band of a row, so a press there
                    // runs the action instead of merely activating the toast.
                    let has_action =
                        self.toasts.get(index).is_some_and(|item| item.action_label.is_some());
                    if has_action {
                        let row = self.row_rect(index);
                        if pos.x >= row.x + row.width as i32 - 72 {
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
        let bottom = band.y + band.height as i32;
        for (index, item) in self.toasts.iter().enumerate() {
            if index < self.scroll_offset {
                continue;
            }
            let visual_order = self.toasts.len() - 1 - index;
            let y = bottom - ((visual_order + 1) as i32 * self.row_height as i32);
            if y < band.y {
                continue;
            }

            let row = Rect::new(
                band.x + 4,
                y + 2,
                band.width.saturating_sub(8),
                self.row_height.saturating_sub(4),
            );
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
            let badge = crate::style::semantic_color(match item.level {
                ToastLevel::Info => crate::style::SemanticColor::Info,
                ToastLevel::Success => crate::style::SemanticColor::Success,
                ToastLevel::Warning => crate::style::SemanticColor::Warning,
                ToastLevel::Error => crate::style::SemanticColor::Error,
            })
            .map(|token| token.blend(&bg, 0.15))
            .unwrap_or_else(|| bg.blend(&text_color, 0.6));
            context.fill_rect(Rect::new(row.x + 6, row.y + 9, 8, 8), badge);
            context.draw_text(
                Point::new(row.x + 20, row.y + 17),
                &item.message,
                effective_font(style),
                text_color,
                HorizontalAlignment::Left,
            );
            if let Some(label) = item.action_label.as_deref() {
                // The action sits in the trailing band the hit test reserves, right-aligned so
                // its label ends where the row's padding does.
                let action = Rect::new(row.x + row.width as i32 - 72, row.y, 68, row.height);
                context.draw_text_line_cow(
                    action,
                    label,
                    effective_font(style),
                    text_color,
                    HorizontalAlignment::Right,
                );
            }
        }
    }
}
