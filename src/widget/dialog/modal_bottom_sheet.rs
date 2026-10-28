// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ModalBottomSheet widget — Material-style modal bottom sheet with drag-to-dismiss.
//!
//! Displays a semi-transparent overlay behind a rounded top sheet containing a
//! drag handle, title, and optional content. Supports show/hide, drag-to-dismiss,
//! and overlay-click-to-dismiss. Emits a `dismissed` signal when closed.

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_bool, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Material-style modal bottom sheet widget.
///
/// When visible, a semi-transparent overlay covers the widget geometry and a
/// rounded sheet appears at the bottom with a drag handle, title, and optional
/// child content. The user can tap the overlay or drag downward to dismiss.
pub struct ModalBottomSheet {
    base: BaseWidget,
    title: String,
    content: Option<Box<dyn Widget>>,
    is_visible: bool,
    drag_offset: f32,
    is_dragging: bool,
    /// Pointer `y` where the current drag began, in the same space `Event` carries.
    ///
    /// Kept so `drag_to` can turn an absolute position into the delta `drag_offset`
    /// accumulates from. `None` outside a drag.
    drag_origin_y: Option<i32>,
    /// Explicit sheet height in logical pixels, or `None` to derive it.
    ///
    /// The height used to be a fixed `40 + 24 + 80` sum, so a caller could not make a
    /// peek-only sheet or a tall list sheet without replacing the control. `None` keeps
    /// that derived height, which is why the default frame is unchanged.
    sheet_height: Option<u32>,
    /// Whether `show`/`hide` slide the panel instead of switching it.
    ///
    /// Opt-in: the default control paints the same frames it always did, and a caller that
    /// wants the rise/slide transition asks for it. The driver still starts at its hidden
    /// end so enabling the flag while the sheet is up cannot replay the entrance.
    animated: bool,
    /// How far the panel has risen: `0.0` fully off-screen, `1.0` at rest.
    reveal: PropertyDriver,
    /// Emitted when the sheet is dismissed by user interaction.
    pub dismissed: GenericSignal,
}

impl ModalBottomSheet {
    /// Creates a new ModalBottomSheet widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ModalBottomSheet, geometry, "ModalBottomSheet"),
            title: String::new(),
            content: None,
            is_visible: false,
            drag_offset: 0.0,
            is_dragging: false,
            drag_origin_y: None,
            sheet_height: None,
            animated: false,
            reveal: PropertyDriver::at(0.0, MotionSlot::Normal),
            dismissed: GenericSignal::new(),
        }
    }

    /// Shows the bottom sheet.
    ///
    /// With [`set_animated`](Self::set_animated) on, the panel rises from below over the
    /// following frames; off, it appears on this frame, which is the historical behaviour.
    pub fn show(&mut self) {
        if !self.is_visible {
            self.is_visible = true;
            self.drag_offset = 0.0;
            self.is_dragging = false;
            self.aim_reveal(1.0);
            self.base.request_redraw();
        }
    }

    /// Hides the bottom sheet without emitting the dismissed signal.
    ///
    /// With animation on, the panel slides back down and stops being painted once it has
    /// left the control; the logical state is already hidden, so it no longer takes input.
    pub fn hide(&mut self) {
        if self.is_visible {
            self.is_visible = false;
            self.drag_offset = 0.0;
            self.is_dragging = false;
            self.aim_reveal(0.0);
            self.base.request_redraw();
        }
    }

    /// Dismisses the sheet as a user action: hides it and emits `dismissed`.
    ///
    /// The overlay click, the Escape key and [`command`](WidgetProperties::command)'s
    /// `dismiss` route all end here, so a sheet has one definition of "the user closed
    /// it" and a hidden sheet never emits the signal.
    pub fn dismiss(&mut self) {
        if !self.is_visible {
            return;
        }
        self.hide();
        self.dismissed.emit();
    }

    /// Aims the reveal driver at `target`, snapping it when animation is off.
    fn aim_reveal(&mut self, target: f32) {
        if self.animated {
            self.reveal.set_target(target);
        } else {
            self.reveal.jump_to(target);
        }
    }

    /// Returns how far the panel has risen, `0.0` hidden and `1.0` at rest.
    pub fn reveal_progress(&self) -> f32 {
        self.reveal.value()
    }

    /// Returns whether show/hide animate the panel.
    pub fn is_animated(&self) -> bool {
        self.animated
    }

    /// Enables or disables the rise/slide transition.
    ///
    /// Turning it off snaps the panel to its current logical position, so the next frame
    /// is the settled frame rather than a half-transitioned one.
    pub fn set_animated(&mut self, animated: bool) {
        if self.animated == animated {
            return;
        }
        self.animated = animated;
        let target = if self.is_visible { 1.0 } else { 0.0 };
        self.reveal.jump_to(target);
        self.base.request_redraw();
    }

    /// Returns the sheet panel's height in logical pixels.
    ///
    /// This is the explicit override when one was set, otherwise the height derived from
    /// the control's own rectangle (the historical `40 + 24 + 80` figure).
    pub fn sheet_height(&self) -> u32 {
        self.compute_sheet_height()
    }

    /// Overrides the sheet panel's height, or restores the derived height with `None`.
    ///
    /// The value is clamped to the control's rectangle when painted, so a height taller
    /// than the sheet's own control cannot paint outside it.
    pub fn set_sheet_height(&mut self, height: Option<u32>) {
        if self.sheet_height == height {
            return;
        }
        self.sheet_height = height;
        self.base.request_redraw();
    }

    /// Returns whether the sheet is currently visible.
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Reports the sheet's own visibility flag.
    ///
    /// Deliberately distinct from [`Widget::is_visible`], which this widget
    /// overrides to return the same flag. Keeping the two entry points separate
    /// lets the property contract call the inherent one unambiguously.
    pub fn is_sheet_visible(&self) -> bool {
        self.is_visible
    }

    /// Sets the sheet title text.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }

    /// Returns the sheet title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the child content widget displayed in the sheet body.
    ///
    /// The child has never been laid out, so the first layout request is issued here rather
    /// than deferred to the first paint (DLG-008). The sheet places the child in its content
    /// well and paints it in the same pass that paints the chrome.
    pub fn set_content(&mut self, widget: Box<dyn Widget>) {
        self.content = Some(widget);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The rectangle the content well occupies, or `None` when the sheet has no room for it.
    ///
    /// A host that draws its own child reads this instead of reconstructing the title and
    /// handle arithmetic (DLG-17).
    pub fn content_rect(&self) -> Option<Rect> {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        let sheet_height = self.compute_sheet_height();
        let title_font = effective_font(self.base.style());
        // The title row is derived from the font the paint will use, so the well's top edge is
        // the one the drawn title leaves behind.
        let title_height = title_font.size().ceil() as i32 + 4;
        let content_top =
            rect.y + rect.height as i32 - sheet_height as i32 + 10 + 5 + 12 + title_height + 8;
        let content_bottom = rect.y + rect.height as i32 + self.drag_offset as i32;
        let height = (content_bottom - content_top).max(0) as u32;
        (height > 20)
            .then(|| Rect::new(rect.x + 8, content_top, rect.width.saturating_sub(16), height))
    }

    /// Returns a reference to the child content, if any.
    pub fn content(&self) -> Option<&dyn Widget> {
        self.content.as_deref()
    }

    /// Returns a mutable reference to the child content, if any.
    pub fn content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.content.as_deref_mut()
    }

    /// Returns the current drag offset.
    pub fn drag_offset(&self) -> f32 {
        self.drag_offset
    }

    /// Returns whether the user is currently dragging the sheet.
    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }

    /// Called when the user starts dragging the sheet.
    pub fn start_drag(&mut self) {
        if self.is_visible {
            self.is_dragging = true;
            self.base.request_redraw();
        }
    }

    /// Called to update the drag offset.
    /// Positive values indicate dragging downward.
    pub fn update_drag(&mut self, delta: f32) {
        if self.is_dragging {
            self.drag_offset = (self.drag_offset + delta).max(0.0);
            self.base.request_redraw();
        }
    }

    /// Tracks a pointer position during a drag, relative to where the press began.
    ///
    /// This is what the `MouseMove` event arm uses. `update_drag` takes a *delta*,
    /// but an event carries an absolute position, so the press origin has to be
    /// remembered to derive it. Without that the `MouseMove` arm had nothing to
    /// pass, `drag_offset` stayed `0.0`, and `end_drag` could never cross the
    /// one-third threshold — so drag-to-dismiss was unreachable through the event
    /// API even though the methods that implement it were correct and tested.
    ///
    /// The offset is recomputed from the origin rather than accumulated, so moving the
    /// pointer back up restores the original position exactly.
    pub fn drag_to(&mut self, y: i32) {
        let Some(origin_y) = self.drag_origin_y else { return };
        if !self.is_dragging {
            return;
        }
        self.drag_offset = (y - origin_y).max(0) as f32;
        self.base.request_redraw();
    }

    /// Ends a drag, dismissing the sheet when it was pulled far enough.
    ///
    /// Dismisses the sheet if drag offset exceeds one third of the sheet height.
    pub fn end_drag(&mut self) {
        if self.is_dragging {
            self.is_dragging = false;
            self.drag_origin_y = None;
            let sheet_height = self.compute_sheet_height();
            if self.drag_offset > sheet_height as f32 / 3.0 {
                self.is_visible = false;
                self.dismissed.emit();
            }
            self.drag_offset = 0.0;
            self.base.request_redraw();
        }
    }

    /// Cancels a drag without dismissing, restoring the sheet's resting position.
    ///
    /// Used when the pointer leaves the sheet mid-drag: a release outside the
    /// control is never delivered, so without this the sheet would stay in a
    /// half-dragged state with `is_dragging` set.
    pub fn cancel_drag(&mut self) {
        if self.is_dragging {
            self.is_dragging = false;
            self.drag_origin_y = None;
            self.drag_offset = 0.0;
            self.base.request_redraw();
        }
    }

    /// Computes the sheet panel height: the caller's override, or the derived default.
    ///
    /// The derived figure is the historical `40 (title) + 24 (handle) + 80 (body)` sum,
    /// clamped to the control. An explicit height is used as given (still clamped to the
    /// control's height), so a caller can ask for a short peek sheet.
    fn compute_sheet_height(&self) -> u32 {
        let rect = self.geometry();
        if let Some(explicit) = self.sheet_height {
            return explicit.min(rect.height);
        }
        let title_height: u32 = 40;
        let handle_area: u32 = 24;
        let min_sheet = 120;
        let max_sheet = rect.height.saturating_sub(40);
        (title_height + handle_area + 80).min(max_sheet).max(min_sheet)
    }
}

impl Widget for ModalBottomSheet {
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
        crate::core::Size::new(300, 200)
    }

    /// Advances the rise/slide transition, returning whether another frame is owed.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.reveal.tick(delta_ms)
    }

    /// Whether the panel is mid-transition and owes the frame bus another frame.
    fn is_animating(&self) -> bool {
        self.reveal.is_moving()
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
            "dismissed" => Some(EventSignalRef::unit("dismissed", &self.dismissed)),
            _ => None,
        }
    }
}

/// `ModalBottomSheet`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `visible` is served by the
/// inherent accessor rather than the base fallthrough, so a modal sheet's own
/// show/hide state is what the name reports.
impl WidgetProperties for ModalBottomSheet {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "visible" => Ok(CapabilityValue::Bool(self.is_sheet_visible())),
            "sheet_height" => Ok(CapabilityValue::UInt(u64::from(self.sheet_height.unwrap_or(0)))),
            "animated" => Ok(CapabilityValue::Bool(self.is_animated())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "visible" => {
                if expect_bool(value)? {
                    self.show();
                } else {
                    self.hide();
                }
                Ok(())
            }
            // `0` means "derive the height from the control", which is what makes the
            // override clearable over the property route as well as through the setter.
            "sheet_height" => {
                let height = expect_u32(value)?;
                self.set_sheet_height(if height == 0 { None } else { Some(height) });
                Ok(())
            }
            "animated" => {
                self.set_animated(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["visible", "sheet_height", "animated", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `modal_bottom_sheet` publishes.
    ///
    /// `show` and `dismiss` are payload-free and map onto the widget's real
    /// methods. `dismiss` takes the same path every other close route takes —
    /// [`ModalBottomSheet::hide`], which the property route's `visible = false`
    /// and the overlay click also use — so the sheet ends up with no more than one
    /// way to become hidden. `set_visible` carries the boolean the property route
    /// already accepts, so a bare invocation is reported as needing one rather than
    /// being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "show" => {
                self.show();
                Ok(())
            }
            "dismiss" => {
                self.dismiss();
                Ok(())
            }
            "set_visible" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for ModalBottomSheet {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal —
        // and the whole sheet used to be skipped unless it was already showing — so the
        // census reported `ink = 0` *and* no response to a light/dark switch.
        //
        // The theme reads are the cached shared snapshots, so no manager lock is held across
        // the draw (the mutex is not re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("modal_bottom_sheet");
        // `modal_bottom_sheet` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. A pane painted in that colour would be byte-identical to the dark scrim
        // over the frame behind it, so a resolved surface equal to the window fill is
        // re-derived a visible step away from it, the same distinction
        // `Colors::input_background` draws for a field.
        let window_fill = {
            crate::style::palette_shared().map(|colors| colors.background).unwrap_or(Color::WHITE)
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(40, 40, 40));
        let sheet_fill = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != sheet_fill)
            .unwrap_or_else(|| sheet_fill.blend(&ink, 0.35));

        let sheet_height = self.compute_sheet_height();
        let drag_offset_px = self.drag_offset as i32;

        let reveal = self.reveal.value();
        // A sheet that is hidden and has finished sliding out paints nothing at all. The
        // previous order filled the whole page with the scrim and only then returned, which
        // cost a full-page fill on every still frame of a closed sheet (G-P11).
        if !self.is_visible && reveal <= 0.0 {
            return;
        }

        // The scrim is drawn in both states and fades with the reveal, so a freshly constructed
        // sheet still has a rendered extent at rest while the open state keeps the sheet itself
        // as the opaque element.
        let scrim = crate::style::layer_color(crate::style::LayerColor::Scrim)
            .unwrap_or_else(|| ink.blend(&sheet_fill, 0.55));
        if reveal < 1.0 {
            let scrim = scrim.with_alpha((scrim.a as f32 * reveal) as u8);
            let overlay_rect = Rect::new(rect.x, rect.y, rect.width, rect.height);
            context.fill_rect(overlay_rect, scrim);
        } else {
            let overlay_rect = Rect::new(rect.x, rect.y, rect.width, rect.height);
            context.fill_rect(overlay_rect, scrim);
        }

        // 2. Sheet panel at the bottom, shifted by the drag offset and by however far the
        // panel has still to rise. At `reveal = 1.0` the slide term is zero, so the settled
        // frame is byte-identical to the pre-animation one.
        let slide = ((1.0 - reveal) * sheet_height as f32) as i32;
        let sheet_y = rect.y + rect.height as i32 - sheet_height as i32 + drag_offset_px + slide;
        let sheet_rect_panel = Rect::new(rect.x, sheet_y, rect.width, sheet_height);
        let corner_radius: u32 = 16;

        context.fill_rounded_rect(sheet_rect_panel, corner_radius, sheet_fill);
        context.draw_rounded_rect_stroke(sheet_rect_panel, corner_radius, border, 1);

        // 3. Drag handle
        let handle_width: u32 = 36;
        let handle_height: u32 = 5;
        let handle_x = rect.x + (rect.width as i32 - handle_width as i32) / 2;
        let handle_y = sheet_y + 10;
        let handle_rect = Rect::new(handle_x, handle_y, handle_width, handle_height);
        context.fill_rounded_rect(handle_rect, handle_height / 2, ink.blend(&sheet_fill, 0.35));

        // 4. Title. `title_y` already names the top of the title row, and the glyph origin is
        // the box's top edge, so no ascent belongs here — the one that used to be added began
        // the title half a line below its own row, and the content area below then started
        // from `title_y + height`, which only looked right because both ends shared the error.
        let title_y = handle_y + handle_height as i32 + 12;
        // The **effective font** — the resolved theme/caller font — rather than the fixed 16 px
        // sans-serif this used to build (D09-STYLE-01).
        let title_font = effective_font(style);
        let title_metrics = context.measure_text(&self.title, title_font);
        if !self.title.is_empty() {
            let title_x = rect.x + (rect.width as i32 - title_metrics.width as i32) / 2;
            context.draw_text(
                Point::new(title_x.max(rect.x), title_y),
                &self.title,
                title_font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        // 5. Content area (child widget rendering is delegated)
        let content_top = title_y + title_metrics.height as i32 + 8;
        let content_bottom = rect.y + rect.height as i32 + drag_offset_px;
        let content_available = (content_bottom - content_top) as u32;

        if content_available > 20 {
            let content_rect = Rect::new(
                rect.x + 8,
                content_top,
                rect.width.saturating_sub(16),
                content_available,
            );
            // The content well is one step away from the sheet it sits in, so the two read as
            // separate regions in either appearance.
            context.fill_rect(content_rect, sheet_fill.blend(&ink, 0.04));
            // Place the child in the well it is drawn into. Writing the geometry every frame
            // is idempotent; the child owns its own internal layout, and this control owns only
            // the box handed to it (DLG-17).
            //
            // DLG-008: placing the child was all this did — the paint path returned without
            // asking it to draw, so a mounted child left the content well as an empty tinted
            // rectangle. The child is drawn here, through the same `as_draw_mut` bridge the
            // mounted tree uses, because the sheet is the one object that knows the well.
            if let Some(content) = self.content.as_mut() {
                content.set_geometry(content_rect);
                content.show();
                if let Some(drawable) = content.as_draw_mut() {
                    drawable.draw(context);
                }
            }
        } else if let Some(content) = self.content.as_mut() {
            content.hide();
        }
    }
}

impl EventHandler for ModalBottomSheet {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() || !self.is_visible {
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == 1 {
                    let rect = self.geometry();
                    let sheet_height = self.compute_sheet_height();
                    let drag_offset_px = self.drag_offset as i32;
                    let sheet_y =
                        rect.y + rect.height as i32 - sheet_height as i32 + drag_offset_px;

                    // Check if click is in the sheet area (including drag handle area)
                    let in_sheet = pos.y >= sheet_y;

                    if in_sheet {
                        // Start a drag, recording where the pointer went down so
                        // `drag_to` can measure from it.
                        self.start_drag();
                        self.drag_origin_y = Some(pos.y);
                    } else {
                        // Click on overlay — dismiss
                        self.dismiss();
                    }
                }
            }
            Event::MouseMove { pos } => {
                // Driven through `drag_to`, which needs the press origin to turn an
                // absolute pointer position into the offset `end_drag` tests. The arm
                // used to be empty with a comment saying the origin "would be tracked",
                // so `drag_offset` never left `0.0` and the one-third threshold in
                // `end_drag` was unreachable from a real event sequence.
                self.drag_to(pos.y);
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 && self.is_dragging {
                    self.end_drag();
                }
            }
            // A release outside the sheet is never delivered (the runtime's hit-test
            // answers `None` outside every control), so the drag is cancelled here
            // rather than left half-finished.
            Event::MouseLeave { .. } if self.is_dragging => {
                self.cancel_drag();
            }
            // Escape is the keyboard counterpart of the overlay click, and it goes through the
            // same `dismiss` entry point so the signal and the hidden state agree.
            Event::KeyPress { key, .. } if *key == key_codes::ESCAPE => {
                self.dismiss();
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::widget::capability::types::CapabilityValue;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    fn make_sheet() -> ModalBottomSheet {
        ModalBottomSheet::new(Rect::new(0, 0, 400, 600))
    }

    #[test]
    fn modal_bottom_sheet_default_state() {
        let sheet = make_sheet();
        assert!(!sheet.is_visible());
        assert_eq!(sheet.title(), "");
        assert_eq!(sheet.kind(), WidgetKind::ModalBottomSheet);
    }

    #[test]
    fn modal_bottom_sheet_show_and_hide() {
        let mut sheet = make_sheet();
        assert!(!sheet.is_visible());

        sheet.show();
        assert!(sheet.is_visible());

        sheet.hide();
        assert!(!sheet.is_visible());
    }

    /// A content stand-in that records the box it was painted into and lays down one colour.
    struct PaintedContent {
        base: BaseWidget,
        painted: std::rc::Rc<std::cell::Cell<Option<Rect>>>,
    }

    impl PaintedContent {
        fn new(painted: std::rc::Rc<std::cell::Cell<Option<Rect>>>) -> Self {
            Self {
                base: BaseWidget::new(WidgetKind::Label, Rect::new(0, 0, 0, 0), "PaintedContent"),
                painted,
            }
        }
    }

    impl Widget for PaintedContent {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
        fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
            Some(self)
        }
    }

    impl Draw for PaintedContent {
        fn draw(&mut self, context: &mut RenderContext) {
            self.painted.set(Some(self.geometry()));
            context.fill_rect(self.geometry(), Color::rgb(3, 4, 5));
        }
    }

    impl EventHandler for PaintedContent {
        fn handle_event(&mut self, _event: &Event) {}
    }

    /// The sheet paints its content child inside the content well (DLG-008).
    ///
    /// # The defect this pins
    ///
    /// `draw` placed the child and then returned without asking it to paint, so a mounted
    /// child left the content well as an empty tinted rectangle — the same defect as the
    /// popover's (DLG-007).
    #[test]
    fn the_sheet_paints_its_content_child_inside_the_well() {
        let painted = std::rc::Rc::new(std::cell::Cell::new(None));
        let mut sheet = ModalBottomSheet::new(Rect::new(0, 0, 400, 600));
        sheet.set_content(Box::new(PaintedContent::new(std::rc::Rc::clone(&painted))));
        sheet.show();

        let svg = render_to_svg(&mut sheet);
        assert!(
            svg.contains("rgba(3,4,5,1.00)"),
            "the installed content must reach the pixels: {svg}"
        );
        let inner = painted.get().expect("the content child was painted");
        let sheet_rect = sheet.geometry();
        assert!(inner.width > 0 && inner.height > 0, "the child must get a real box: {inner:?}");
        assert!(
            inner.x >= sheet_rect.x
                && inner.y >= sheet_rect.y
                && inner.x + inner.width as i32 <= sheet_rect.x + sheet_rect.width as i32
                && inner.y + inner.height as i32 <= sheet_rect.y + sheet_rect.height as i32,
            "the child is placed inside the sheet: {inner:?} vs {sheet_rect:?}"
        );
    }

    /// Installing content asks for its first layout (DLG-008).
    #[test]
    fn installing_sheet_content_requests_its_first_layout() {
        let mut sheet = make_sheet();
        let asked = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&asked);
        sheet.base.layout_requested.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        sheet.set_content(Box::new(PaintedContent::new(std::rc::Rc::new(std::cell::Cell::new(
            None,
        )))));
        assert_eq!(asked.load(Ordering::SeqCst), 1, "the new child has never been laid out");
    }

    #[test]
    fn modal_bottom_sheet_dismiss_signal_on_overlay_click() {
        let mut sheet = make_sheet();
        sheet.show();
        assert!(sheet.is_visible());

        let dismissed = Arc::new(AtomicBool::new(false));
        sheet.dismissed.connect({
            let d = Arc::clone(&dismissed);
            move || {
                d.store(true, Ordering::SeqCst);
            }
        });

        // Click above the sheet area (overlay region)
        // Sheet height is computed, but geometry is 600 tall so overlay is top portion
        sheet.handle_event(&Event::MousePress {
            pos: Point::new(200, 50),
            button: 1,
            modifiers: 0,
        });
        assert!(!sheet.is_visible());
        assert!(dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn modal_bottom_sheet_set_title() {
        let mut sheet = make_sheet();
        assert_eq!(sheet.title(), "");
        sheet.set_title("Options");
        assert_eq!(sheet.title(), "Options");
    }

    #[test]
    fn modal_bottom_sheet_drag_end_dismisses() {
        let mut sheet = make_sheet();
        sheet.show();
        assert!(sheet.is_visible());

        let dismissed = Arc::new(AtomicBool::new(false));
        sheet.dismissed.connect({
            let d = Arc::clone(&dismissed);
            move || {
                d.store(true, Ordering::SeqCst);
            }
        });

        // Start drag, push it past threshold, then release
        sheet.start_drag();
        assert!(sheet.is_dragging());

        // Push past 1/3 of sheet height (sheet is at least 120, so > 40)
        sheet.update_drag(80.0);
        assert_eq!(sheet.drag_offset(), 80.0);

        sheet.end_drag();
        assert!(!sheet.is_visible());
        assert!(dismissed.load(Ordering::SeqCst));
    }

    /// The same dismissal, driven through real `Event`s rather than the methods.
    ///
    /// # Why this test exists
    ///
    /// The test above calls `start_drag`/`update_drag`/`end_drag` directly, so it
    /// passed even while the `MouseMove` event arm was an empty body with a comment
    /// saying the origin "would be tracked". Through the event API `drag_offset`
    /// therefore never left `0.0`, the one-third threshold in `end_drag` was
    /// unreachable, and a user dragging the sheet down got nothing — with every unit
    /// test green.
    #[test]
    fn a_real_drag_gesture_dismisses_the_sheet() {
        let mut sheet = make_sheet();
        sheet.show();
        let rect = sheet.geometry();

        let dismissed = Arc::new(AtomicBool::new(false));
        sheet.dismissed.connect({
            let d = Arc::clone(&dismissed);
            move || {
                d.store(true, Ordering::SeqCst);
            }
        });

        // Press inside the sheet panel, near its bottom edge, as a user would.
        let press_y = rect.y + rect.height as i32 - 20;
        sheet.handle_event(&Event::MousePress {
            pos: Point::new(rect.x + 50, press_y),
            button: 1,
            modifiers: 0,
        });
        assert!(sheet.is_dragging(), "a press inside the sheet must start a drag");

        // Drag downward well past a third of the sheet height.
        for step in 1..=8 {
            sheet.handle_event(&Event::MouseMove {
                pos: Point::new(rect.x + 50, press_y + step * 20),
            });
        }
        assert!(
            sheet.drag_offset() > 40.0,
            "dragging down must accumulate an offset, got {}",
            sheet.drag_offset()
        );

        sheet.handle_event(&Event::MouseRelease {
            pos: Point::new(rect.x + 50, press_y + 160),
            button: 1,
        });
        assert!(!sheet.is_visible(), "a long downward drag must dismiss the sheet");
        assert!(dismissed.load(Ordering::SeqCst));
    }

    /// A short drag through real events must not dismiss, and must reset the offset.
    #[test]
    fn a_real_short_drag_leaves_the_sheet_visible_and_resets_the_offset() {
        let mut sheet = make_sheet();
        sheet.show();
        let rect = sheet.geometry();
        let press_y = rect.y + rect.height as i32 - 20;

        sheet.handle_event(&Event::MousePress {
            pos: Point::new(rect.x + 50, press_y),
            button: 1,
            modifiers: 0,
        });
        sheet.handle_event(&Event::MouseMove { pos: Point::new(rect.x + 50, press_y + 10) });
        sheet.handle_event(&Event::MouseRelease {
            pos: Point::new(rect.x + 50, press_y + 10),
            button: 1,
        });

        assert!(sheet.is_visible(), "a 10px drag must not dismiss");
        assert_eq!(sheet.drag_offset(), 0.0, "the offset must reset after the drag ends");
        assert!(!sheet.is_dragging());
    }

    /// A drag whose pointer leaves the sheet is cancelled, not left half-finished.
    #[test]
    fn a_drag_that_leaves_the_sheet_is_cancelled() {
        let mut sheet = make_sheet();
        sheet.show();
        let rect = sheet.geometry();
        let press_y = rect.y + rect.height as i32 - 20;

        sheet.handle_event(&Event::MousePress {
            pos: Point::new(rect.x + 50, press_y),
            button: 1,
            modifiers: 0,
        });
        sheet.handle_event(&Event::MouseMove { pos: Point::new(rect.x + 50, press_y + 60) });
        assert!(sheet.drag_offset() > 0.0);

        sheet.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
        assert!(!sheet.is_dragging(), "leaving the sheet must end the drag");
        assert_eq!(sheet.drag_offset(), 0.0, "the sheet must return to its resting position");
        assert!(sheet.is_visible(), "leaving is not a dismissal");
    }

    #[test]
    fn modal_bottom_sheet_drag_small_offset_no_dismiss() {
        let mut sheet = make_sheet();
        sheet.show();

        sheet.start_drag();
        sheet.update_drag(20.0);
        sheet.end_drag();

        // Small drag should not dismiss
        assert!(sheet.is_visible());
    }

    #[test]
    fn modal_bottom_sheet_svg_output_visible() {
        let mut sheet = make_sheet();
        sheet.set_title("Example");
        sheet.show();
        let svg = render_to_svg(&mut sheet);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn modal_bottom_sheet_svg_output_hidden() {
        let mut sheet = make_sheet();
        let svg = render_to_svg(&mut sheet);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    /// BLUE23 §5.1, judgement 15 — the modal scrim is **darker** than the surface it covers.
    ///
    /// # What this pins, and why the previous assertion did not
    ///
    /// The defect is BLUE21 B23's: a scrim derived from the *foreground* came out **brighter**
    /// than the surface it covered, so the "dimming" layer lifted the backdrop. BLUE21 B23
    /// introduced the `scrim` role to fix it — but the dark preset then authored that role as a
    /// white veil (`rgba(255,255,255,38)`), which put the defect straight back: over the dark
    /// window it composites to `rgb(54,54,54)` against a `rgb(18,18,18)` page.
    ///
    /// The assertion that catches that is **composited** luminance, not `scrim != window` and not
    /// the blend fallback on its own: the veil *did* differ from the page, and its fallback was
    /// fine — the comparison that matters is the one the user sees. Off the plan's own wording,
    /// "scrim luminance in dark mode < surface it covers".
    #[test]
    #[cfg(device_profile)]
    fn the_scrim_composites_darker_than_the_backdrop() {
        let _guard = crate::style::theme_test_guard();
        for appearance in [crate::theme::AppearanceMode::Dark, crate::theme::AppearanceMode::Light]
        {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let window = crate::style::palette_shared()
                .map(|colors| colors.background)
                .expect("a preset is active");
            let scrim = crate::style::layer_color(crate::style::LayerColor::Scrim)
                .expect("the preset defines a scrim role");
            // What the user sees: the scrim laid over the page, at the scrim's own alpha.
            let composited = window.blend(&scrim, scrim.a as f32 / 255.0);
            assert!(
                composited.luminance() < window.luminance(),
                "the {appearance:?} scrim must dim the page: {window:?} -> {composited:?} \
                 (scrim {scrim:?}) is a lift, which is BLUE21 B23"
            );
        }
    }

    /// BLUE23 §5.1: the two appearances' scrims must not be a numerical coincidence.
    ///
    /// BLUE21 B23's second finding was that the old foreground-derived blend landed on 121 (dark)
    /// and 122 (light) — "almost the same number", which is the signature of an expression that
    /// happened to agree under these two presets rather than of a derived quantity. A scrim that
    /// reads the same on both appearances is the same defect wearing a token.
    #[test]
    #[cfg(device_profile)]
    fn the_two_appearances_do_not_share_one_scrim_number() {
        let _guard = crate::style::theme_test_guard();
        let scrim_of = |appearance| {
            crate::theme::global_theme_manager().set_appearance(appearance);
            crate::style::palette_shared().map(|colors| colors.scrim).expect("a preset is active")
        };
        let dark = scrim_of(crate::theme::AppearanceMode::Dark);
        let light = scrim_of(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark, light,
            "both appearances resolve one scrim value; that is a constant, not a theme role"
        );
        // Stated as the property, not as a hex value: the dark backdrop is darker, so its veil has
        // to carry more weight to separate the modal from the page.
        assert!(
            dark.a > light.a,
            "a near-black backdrop needs a heavier veil than a light one (dark {dark:?}, light \
             {light:?})"
        );
    }

    /// Escape dismisses the sheet through the same path as an overlay click.
    ///
    /// A modal sheet that only answers the pointer strands a keyboard user: there was no
    /// key arm at all, so the only exit was a click on the backdrop.
    #[test]
    fn escape_dismisses_the_sheet_once() {
        let mut sheet = make_sheet();
        sheet.show();
        let dismissed = Arc::new(AtomicUsize::new(0));
        sheet.dismissed.connect({
            let d = Arc::clone(&dismissed);
            move || {
                d.fetch_add(1, Ordering::SeqCst);
            }
        });

        sheet.handle_event(&Event::key_press(key_codes::ESCAPE, 0));
        assert!(!sheet.is_visible(), "Escape must close the sheet");
        assert_eq!(dismissed.load(Ordering::SeqCst), 1, "one Escape is one dismissal");

        sheet.handle_event(&Event::key_press(key_codes::ESCAPE, 0));
        assert_eq!(
            dismissed.load(Ordering::SeqCst),
            1,
            "a hidden sheet must not keep emitting dismissals"
        );
    }

    /// The panel height is parameterised, and the default is still the derived figure.
    #[test]
    fn the_sheet_height_is_parameterised() {
        let mut sheet = make_sheet();
        assert_eq!(sheet.sheet_height(), 144, "the derived height is unchanged");

        sheet.set_sheet_height(Some(220));
        assert_eq!(sheet.sheet_height(), 220);
        // Clamped to the control, so a taller request cannot paint outside it.
        sheet.set_sheet_height(Some(10_000));
        assert_eq!(sheet.sheet_height(), 600);

        sheet.set_sheet_height(None);
        assert_eq!(sheet.sheet_height(), 144, "None restores the derived height");

        // The property route publishes the same value and accepts 0 as "derive".
        sheet.set("sheet_height", CapabilityValue::UInt(180)).expect("writable");
        assert_eq!(sheet.sheet_height(), 180);
        assert_eq!(sheet.get("sheet_height"), Ok(CapabilityValue::UInt(180)));
        sheet.set("sheet_height", CapabilityValue::UInt(0)).expect("writable");
        assert_eq!(sheet.sheet_height(), 144);
        assert_eq!(
            sheet.get("sheet_height"),
            Ok(CapabilityValue::UInt(0)),
            "the property reports the override, not the effective height"
        );
    }

    /// The slide transition is opt-in, and while it runs it paints an intermediate panel.
    #[test]
    fn the_slide_transition_is_opt_in() {
        let mut sheet = make_sheet();
        sheet.show();
        assert_eq!(sheet.reveal_progress(), 1.0, "a static show is settled at once");
        assert!(!sheet.is_animating());

        sheet.set_animated(true);
        assert_eq!(
            sheet.reveal_progress(),
            1.0,
            "enabling the flag snaps to the current position, not a replay"
        );
        assert!(!sheet.is_animating());

        sheet.hide();
        assert!(sheet.is_animating(), "an animated hide owes frames");
        assert_eq!(sheet.reveal_progress(), 1.0, "the exit starts from the shown frame");
        while sheet.tick(1000) {}
        assert_eq!(sheet.reveal_progress(), 0.0, "the panel settles fully slid out");

        sheet.show();
        assert!(sheet.is_animating(), "an animated show owes frames");
        assert!(sheet.tick(16), "the first step still owes frames");
        let mid = sheet.reveal_progress();
        assert!(mid > 0.0 && mid < 1.0, "the panel must pass through an interior rise, got {mid}");
        while sheet.tick(1000) {}
        assert_eq!(sheet.reveal_progress(), 1.0);
        assert!(!sheet.is_animating());
    }
}
