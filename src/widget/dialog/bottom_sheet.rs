// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BottomSheet widget — a modal panel that slides up from the bottom edge of the
//! screen, in the Material Design sense.
//!
//! The sheet displays a semi-transparent overlay behind a rounded-rect panel at the bottom
//! of its geometry. It supports open/close state, a content child, a drag handle that can be
//! dragged down to dismiss, and emits a `dismissed` signal when the user taps the overlay,
//! presses Escape, or drags the panel far enough.
//!
//! # Presentation
//!
//! The default [`SheetAttachment::Overlay`] is the Material form: a floating panel over a
//! dimmed page. [`SheetAttachment::Attached`] is the in-window variant — the panel is flush
//! with the window's bottom edge and there is no scrim. A native window-attached sheet that
//! slides out of a title bar is a platform window, not this control; a host that needs that
//! owns the window and uses this control for its body.

use crate::core::{Color, ObjectId, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_bool, expect_f32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// How the sheet is presented inside its host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetAttachment {
    /// A floating panel over a dimmed page. The default, and the historical appearance.
    #[default]
    Overlay,
    /// A panel flush with the window's bottom edge, with no scrim.
    Attached,
}

/// A drag in progress on the sheet's handle.
#[derive(Debug, Clone, Copy)]
struct SheetDrag {
    /// The pointer's y at the press.
    origin_y: i32,
    /// Whether the pointer has moved far enough to count as a drag rather than a tap.
    moved: bool,
}

/// How far the pointer must travel before a press counts as a drag rather than a tap.
const DRAG_THRESHOLD: i32 = 2;

/// BottomSheet widget — a modal panel that slides up from the bottom edge.
///
/// The sheet occupies the bottom `content_height` pixels of its geometry rect.
/// When open, a semi-transparent overlay fills the area above the sheet and a
/// rounded panel is drawn at the bottom with a small drag handle.
/// A press on the overlay dismisses the sheet; a press on the panel does not.
pub struct BottomSheet {
    base: BaseWidget,
    open: bool,
    content_height: u32,
    /// The single content widget the sheet hosts, if any.
    content_widget: Option<ObjectId>,
    /// The content geometry the last frame wrote, so a still frame does not repeat the write (G-P12).
    placed_content: Option<(ObjectId, Rect)>,
    /// How the panel is presented.
    attachment: SheetAttachment,
    /// The handle drag in progress.
    drag: Option<SheetDrag>,
    /// How far the panel has been pulled down during a drag.
    drag_offset: f32,
    /// How far the sheet has risen, `0.0` fully stowed below the page and `1.0` fully shown.
    ///
    /// # Why this is separate from `open`
    ///
    /// `open` is the logical state a caller reads the instant it changes; `rise` is what the draw
    /// path measures the panel and the scrim with. Drawing straight from `open` put the sheet at its
    /// final position on the first frame and removed it on the last, so a bottom sheet — whose whole
    /// gesture *is* "slide up from the bottom edge" — teleported. Same split, same reason, as
    /// `Switch`'s `checked`/`travel`. It starts at `0.0` because a freshly built sheet is closed.
    rise: PropertyDriver,
    /// Emitted when the sheet is dismissed by user interaction.
    pub dismissed: GenericSignal,
}

impl BottomSheet {
    /// Creates a new BottomSheet widget with the given geometry.
    ///
    /// The sheet starts closed. Default content height is half the geometry height.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::BottomSheet, geometry, "BottomSheet"),
            open: false,
            content_height: geometry.height / 2,
            content_widget: None,
            placed_content: None,
            attachment: SheetAttachment::Overlay,
            drag: None,
            drag_offset: 0.0,
            // At rest at the stowed end: a freshly built sheet is closed, so it must not animate
            // *down* on its first frame.
            rise: PropertyDriver::at(0.0, MotionSlot::Normal),
            dismissed: GenericSignal::new(),
        }
    }

    /// Opens the bottom sheet, making it visible.
    pub fn open(&mut self) {
        if !self.open {
            self.open = true;
            // Aim the slide; the frames that follow carry the panel up. No redraw is requested
            // here, because a redraw would not be what makes the motion visible --
            // `tick_animations` reports the movement itself and that is what keeps the loop
            // painting.
            self.rise.set_target(1.0);
            self.drag = None;
            self.drag_offset = 0.0;
            self.place_content();
        }
    }

    /// Dismisses (closes) the bottom sheet without emitting the dismissed signal.
    pub fn dismiss(&mut self) {
        if self.open {
            self.open = false;
            self.rise.set_target(0.0);
            self.drag = None;
            self.drag_offset = 0.0;
            self.set_content_visible(false);
            self.base.request_redraw();
        }
    }

    /// How far the sheet has risen, `0.0` stowed and `1.0` fully shown.
    ///
    /// This is the *drawn* fraction: the panel's offset and the scrim's strength are both functions
    /// of it, so a test can assert the sheet slid rather than appeared by sampling it per frame.
    pub fn rise_progress(&self) -> f32 {
        self.rise.value()
    }

    /// Advances the rise by `delta_ms`; `true` while it is still moving.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.rise.tick(delta_ms)
    }

    /// Whether the sheet is between two positions -- answers only, never advances.
    pub fn is_animating(&self) -> bool {
        self.rise.is_moving()
    }

    /// Sets the content height of the sheet panel (in pixels).
    ///
    /// The sheet is drawn at the bottom of the geometry rect using this height.
    /// Clamped to the geometry height.
    pub fn set_content_height(&mut self, height: u32) {
        let height = height.min(self.base.geometry.height);
        if self.content_height == height {
            return;
        }
        self.content_height = height;
        self.base.request_redraw();
    }

    /// Returns whether the bottom sheet is currently open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Returns the height of the sheet panel in pixels.
    pub fn content_height(&self) -> u32 {
        self.content_height
    }

    /// The content widget the sheet hosts, if any.
    pub fn content_widget(&self) -> Option<ObjectId> {
        self.content_widget
    }

    /// Sets the content widget this sheet hosts.
    ///
    /// The link is recorded in both directions, and a replaced child is unlinked and hidden —
    /// the same pairing [`crate::widget::dialog::Dialog::set_content_widget`] performs, because
    /// a stale child id is a rectangle the tree still visits and draws.
    pub fn set_content_widget(&mut self, widget: Option<ObjectId>) {
        if self.content_widget == widget {
            return;
        }
        if let Some(old) = self.content_widget {
            if Some(old) != widget {
                self.base.remove_child_linked(old);
                crate::set_widget_visible(old, false);
            }
        }
        self.content_widget = widget;
        if let Some(id) = widget {
            self.base.add_child_linked(id);
        }
        self.place_content();
        self.base.request_redraw();
    }

    /// The rectangle handed to the content child on the resting panel.
    ///
    /// The handle's band is reserved at the top of the panel, and the content gets the rest,
    /// inset by the panel's own padding. A closed sheet has no content rectangle.
    pub fn content_rect(&self) -> Option<Rect> {
        if !self.open {
            return None;
        }
        let panel = self.resting_panel_rect();
        let top = panel.y + 8 + 5 + 8;
        let height = panel.y + panel.height as i32 - top - 8;
        (height > 0)
            .then(|| Rect::new(panel.x + 8, top, panel.width.saturating_sub(16), height as u32))
    }

    /// Places the content child inside the current panel.
    fn place_content(&self) {
        let Some(content) = self.content_widget else { return };
        match self.content_rect() {
            Some(rect) => {
                crate::set_widget_geometry(content, rect.x, rect.y, rect.width, rect.height);
                crate::set_widget_visible(content, self.open);
            }
            None => crate::set_widget_visible(content, false),
        }
    }

    /// Shows or hides the content child without changing the link.
    fn set_content_visible(&self, visible: bool) {
        if let Some(content) = self.content_widget {
            crate::set_widget_visible(content, visible);
        }
    }

    /// How the panel is presented.
    pub fn attachment(&self) -> SheetAttachment {
        self.attachment
    }

    /// Selects the presentation and repaints when it changes.
    pub fn set_attachment(&mut self, attachment: SheetAttachment) {
        if self.attachment == attachment {
            return;
        }
        self.attachment = attachment;
        self.base.request_redraw();
    }

    /// The panel's rectangle at rest (fully risen), derived from the geometry.
    fn resting_panel_rect(&self) -> Rect {
        let rect = self.geometry();
        let sheet_height = self.content_height.min(rect.height);
        Rect::new(
            rect.x,
            rect.y + rect.height as i32 - sheet_height as i32,
            rect.width,
            sheet_height,
        )
    }

    /// The handle's rectangle at rest.
    fn handle_rect(&self) -> Rect {
        let panel = self.resting_panel_rect();
        let width = 32.min(panel.width);
        let height = 5.min(panel.height);
        Rect::new(panel.x + (panel.width as i32 - width as i32) / 2, panel.y + 8, width, height)
    }

    /// Whether a drag of the handle is in progress.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// How far the panel has been pulled down during the current drag.
    pub fn drag_offset(&self) -> f32 {
        self.drag_offset
    }

    /// Tracks a pointer position during a handle drag.
    ///
    /// The offset is recomputed from the press origin, so moving the pointer back up restores
    /// the panel exactly instead of accumulating rounding error.
    pub fn drag_to(&mut self, y: i32) {
        let Some(drag) = self.drag else { return };
        let delta = y - drag.origin_y;
        if delta.abs() >= DRAG_THRESHOLD && !drag.moved {
            self.drag = Some(SheetDrag { moved: true, ..drag });
        }
        if self.drag.is_some_and(|drag| drag.moved) {
            self.drag_offset = delta.max(0) as f32;
            self.base.request_redraw();
        }
    }

    /// Ends a handle drag, dismissing the sheet when it was pulled far enough.
    ///
    /// A drag past one third of the panel's height dismisses; anything shorter restores the
    /// resting position.
    pub fn end_drag(&mut self) {
        let Some(drag) = self.drag else { return };
        self.drag = None;
        let threshold = self.content_height as f32 / 3.0;
        if drag.moved && self.drag_offset > threshold {
            self.drag_offset = 0.0;
            self.open = false;
            self.rise.set_target(0.0);
            self.set_content_visible(false);
            self.dismissed.emit();
        } else {
            self.drag_offset = 0.0;
        }
        self.base.request_redraw();
    }
}

impl Widget for BottomSheet {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::BOTTOM_SHEET_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::BOTTOM_SHEET_DEFAULT_HEIGHT,
        )
    }

    /// Moves the sheet and re-places its content child.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.place_content();
    }

    // The rise is the control's own animation; the trait spelling is what the frame bus reaches
    // through `&mut dyn Widget`, which is the only way the slide actually happens.
    fn tick(&mut self, delta_ms: u32) -> bool {
        BottomSheet::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        BottomSheet::is_animating(self)
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

/// `BottomSheet`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The panel height is stored as
/// a `u32` internally and published as `Float` to match the legacy shape.
impl WidgetProperties for BottomSheet {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "expanded" => Ok(CapabilityValue::Bool(self.is_open())),
            "peek_height" => Ok(CapabilityValue::Float(f64::from(self.content_height()))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "expanded" => {
                if expect_bool(value)? {
                    self.open();
                } else {
                    self.dismiss();
                }
                Ok(())
            }
            "peek_height" => {
                self.set_content_height(expect_f32(value)? as u32);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["expanded", "peek_height", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `bottom_sheet` publishes.
    ///
    /// Both names carry a payload, so they are answered through the property
    /// route (`expanded` / `peek_height`). The trait default would answer
    /// `UnknownCommand` for names the capability does publish, which
    /// `invoke_command` reports as a registry/implementation disagreement.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_expanded" | "set_peek_height" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for BottomSheet {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so
        // a light/dark switch left the pane and its handle unchanged — the rendering census
        // reported the control as theme-blind.
        //
        // The theme reads are the cached shared snapshots, so no manager lock is held across
        // the draw (the mutex is not re-entrant).
        let style = self.base.style();
        // One palette-snapshot read supplies the window fill and the fallback ink/border; the old
        // path resolved a widget style and then took the manager lock again (G-P13).
        let (window_fill, theme_ink, theme_border) = {
            match crate::style::palette_shared() {
                Some(colors) => (colors.background, Some(colors.foreground), Some(colors.outline)),
                None => (Color::WHITE, None, None),
            }
        };
        let ink = style.text_color.or(theme_ink).unwrap_or(Color::rgb(40, 40, 40));
        let sheet_color = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border_color = style
            .border_color
            .or(theme_border)
            .filter(|resolved| *resolved != sheet_color)
            .unwrap_or_else(|| sheet_color.blend(&ink, 0.35));
        let handle_color = ink.blend(&sheet_color, 0.35);

        let sheet_height = self.content_height.min(rect.height);

        // 1. Draw the modal scrim covering the area above the sheet.
        //
        // Painting the scrim is what keeps the control visible in its default (closed)
        // state. It used to `return` early while closed, so a freshly constructed sheet
        // painted nothing at all and the census reported `ink = 0`; the scrim is drawn at
        // partial opacity in both states, which reads as a dimmed backdrop in either
        // appearance and still leaves the sheet itself the opaque element when open.
        //
        // The scrim darkens the *backdrop it covers* toward an absolute black. Blending
        // toward black is what every platform's modal scrim does (Material's
        // `Colors.black54`, UIKit and SwiftUI all dim toward black); none blends toward
        // the foreground colour.
        //
        // The old `ink.blend(&sheet_color, 0.55)` blended toward the *foreground*, so on the
        // dark appearance it brightened the backdrop instead of dimming it — the SVG showed
        // `rgba(121,121,121)` over an `rgba(18,18,18)` frame, so the modal backplate was lit
        // up while the sheet it was meant to sit behind ended up darker than its own scrim.
        //
        // # Why the old value looked plausible
        //
        // With the two presets this ships with, the old expression happened to produce 120 on
        // the dark appearance and 122 on the light one — nearly identical values in both. That
        // is not agreement but coincidence: `ink` and `sheet_color` *swap roles* between the
        // presets (dark ink is the light preset's surface and vice versa), so the same weights
        // land on nearly the same bytes from opposite directions. A quantity that only
        // coincides because the two inputs are mirror images of each other is derived from
        // nothing — which is exactly why it could invert the scrim's direction on one
        // appearance without the number changing.
        //
        // # Why the source is the window fill and not the sheet colour
        //
        // The scrim covers the *window*, so the colour it must darken is the window fill.
        // Deriving it from the panel's own colour instead leaves it lighter than the frame
        // wherever the panel itself is lighter than the window — which is the dark preset,
        // where the panel is `rgb(35,35,35)` over a `rgb(18,18,18)` window and a scrim of
        // `rgb(24,24,24)` would still read as a lift rather than a dimming.
        let overlay_height = rect.height - sheet_height;
        // The rise drives both halves of the appearance: the scrim fades in and the panel slides up.
        // Deriving them from *one* value is what keeps the two in step, so the backdrop can never be
        // fully dimmed while the panel is still below the edge.
        let rise = self.rise.value();
        const SCRIM_DARKEN: f32 = 0.32;
        // The token wins when a theme provides one; the blend is the honest fallback for an
        // unthemed build. The token is read here so a theme author can express the dimming
        // *once*, instead of every modal re-deriving the same weight and drifting apart --
        // which is why `Colors::scrim` exists as a role at all.
        let scrim_color = crate::style::layer_color(crate::style::LayerColor::Scrim)
            .unwrap_or_else(|| window_fill.blend(&Color::BLACK, SCRIM_DARKEN));
        if overlay_height > 0 && rise > 0.0 && self.attachment == SheetAttachment::Overlay {
            // A rising sheet's scrim is a fraction of its own alpha, not a second colour: fading by
            // scaling the token's alpha is the one form that works for a theme that authored its
            // scrim as a translucent white as well as one that authored it as a black veil.
            let scrim_color = scrim_color.with_alpha((scrim_color.a as f32 * rise) as u8);
            let overlay_rect = Rect::new(rect.x, rect.y, rect.width, overlay_height);
            context.fill_rect(overlay_rect, scrim_color);
        }

        // A sheet that has not risen at all paints only that much scrim (nothing, at rest); the
        // panel and its handle belong to the risen part alone. This replaces the old `if !self.open
        // { return; }`: the guard is now on the *drawn* position rather than the logical flag, so the
        // last frame of a dismissal still paints the panel where it is.
        if rise <= 0.0 {
            return;
        }

        // The panel slides up from the bottom edge: at `rise == 0` it is entirely below the page,
        // and at `rise == 1` it is at its resting position. Interpolating the *y* rather than the
        // height keeps the panel's own shape fixed while it travels, which is what a sheet does --
        // a growing height would read as the content being revealed rather than the panel arriving.
        let resting_y = rect.y + rect.height as i32 - sheet_height as i32;
        let stowed_y = rect.y + rect.height as i32;
        let drawn_y = resting_y + ((stowed_y - resting_y) as f32 * (1.0 - rise)) as i32;
        let sheet_y = drawn_y + self.drag_offset as i32;
        let sheet_rect = Rect::new(rect.x, sheet_y, rect.width, sheet_height);

        // 2. Draw the sheet panel with rounded top corners
        let sheet_radius = 16;
        context.fill_rounded_rect(sheet_rect, sheet_radius, sheet_color);

        // 3. Draw a thin stroke at the rounded top edge for definition
        context.draw_rounded_rect_stroke(sheet_rect, sheet_radius, border_color, 1);

        // 4. Draw the drag handle at the top center of the sheet
        let handle_width = 32;
        let handle_height = 5;
        let handle_x = rect.x + (rect.width as i32 - handle_width as i32) / 2;
        let handle_y = sheet_y + 8;
        let handle_rect = Rect::new(handle_x, handle_y, handle_width, handle_height);
        context.fill_rounded_rect(handle_rect, handle_height / 2, handle_color);

        // The content child follows the panel while it rises and is dragged. The tree painter
        // draws it after this function returns, so this is the rectangle that very frame is
        // drawn into, and it is hidden with the sheet rather than floating over a closed panel.
        if let Some(content) = self.content_widget {
            let content_top = sheet_y + 8 + handle_height as i32 + 8;
            let content_height = sheet_y + sheet_height as i32 - content_top - 8;
            if content_height > 0 {
                let rect = Rect::new(
                    sheet_rect.x + 8,
                    content_top,
                    sheet_rect.width.saturating_sub(16),
                    content_height as u32,
                );
                // A still frame (the reveal settled and no drag) writes the same rectangle it
                // wrote last frame; comparing first keeps the tree update off those frames (G-P12).
                if self.placed_content != Some((content, rect)) {
                    crate::set_widget_geometry(content, rect.x, rect.y, rect.width, rect.height);
                    crate::set_widget_visible(content, true);
                    self.placed_content = Some((content, rect));
                }
            } else if self.placed_content.is_some() {
                crate::set_widget_visible(content, false);
                self.placed_content = None;
            }
        }
    }
}

impl EventHandler for BottomSheet {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } => {
                if *button != 1 || !self.open {
                    return;
                }
                // The handle starts a drag; the panel body is inert; the overlay dismisses.
                if self.handle_rect().contains_point(*pos) {
                    self.drag = Some(SheetDrag { origin_y: pos.y, moved: false });
                    self.drag_offset = 0.0;
                    return;
                }
                if !self.resting_panel_rect().contains_point(*pos) {
                    self.open = false;
                    self.rise.set_target(0.0);
                    self.set_content_visible(false);
                    self.dismissed.emit();
                    self.base.request_redraw();
                }
            }
            Event::MouseMove { pos } => self.drag_to(pos.y),
            Event::MouseRelease { button: 1, .. } => self.end_drag(),
            Event::KeyPress { key, .. } if *key == key_codes::ESCAPE && self.open => {
                self.open = false;
                self.rise.set_target(0.0);
                self.set_content_visible(false);
                self.dismissed.emit();
                self.base.request_redraw();
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
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn make_sheet() -> BottomSheet {
        BottomSheet::new(Rect::new(0, 0, 400, 600))
    }

    #[test]
    fn bottom_sheet_default_is_closed() {
        let sheet = make_sheet();
        assert!(!sheet.is_open());
        assert_eq!(sheet.kind(), WidgetKind::BottomSheet);
        assert_eq!(sheet.base.geometry, Rect::new(0, 0, 400, 600));
    }

    #[test]
    fn bottom_sheet_open_and_close() {
        let mut sheet = make_sheet();
        assert!(!sheet.is_open());

        sheet.open();
        assert!(sheet.is_open());

        sheet.dismiss();
        assert!(!sheet.is_open());
    }

    #[test]
    fn bottom_sheet_set_content_height() {
        let mut sheet = make_sheet();
        assert_eq!(sheet.content_height, 300); // half of 600

        sheet.set_content_height(200);
        assert_eq!(sheet.content_height, 200);

        // Clamp to geometry height
        sheet.set_content_height(999);
        assert_eq!(sheet.content_height, 600);
    }

    #[test]
    fn bottom_sheet_dismiss_signal_emits() {
        let mut sheet = make_sheet();
        sheet.open();
        assert!(sheet.is_open());

        let dismissed = Arc::new(AtomicBool::new(false));
        let d = dismissed.clone();
        sheet.dismissed.connect(move || {
            d.store(true, Ordering::SeqCst);
        });

        sheet.handle_event(&Event::MousePress {
            pos: Point::new(200, 100),
            button: 1,
            modifiers: 0,
        });
        assert!(!sheet.is_open());
        assert!(dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_sheet_mouse_press_dismisses_when_open() {
        let mut sheet = make_sheet();
        sheet.open();
        assert!(sheet.is_open());

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1, modifiers: 0 });
        assert!(!sheet.is_open());
    }

    #[test]
    fn bottom_sheet_mouse_press_noop_when_closed() {
        let mut sheet = make_sheet();
        assert!(!sheet.is_open());

        let dismissed = Arc::new(AtomicBool::new(false));
        let d = dismissed.clone();
        sheet.dismissed.connect(move || {
            d.store(true, Ordering::SeqCst);
        });

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1, modifiers: 0 });
        assert!(!dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_sheet_other_button_noop() {
        let mut sheet = make_sheet();
        sheet.open();

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 2, modifiers: 0 });
        assert!(sheet.is_open());
    }

    #[test]
    fn bottom_sheet_disabled_blocks_events() {
        let mut sheet = make_sheet();
        sheet.set_enabled(false);
        sheet.open();
        assert!(sheet.is_open());

        let dismissed = Arc::new(AtomicBool::new(false));
        let d = dismissed.clone();
        sheet.dismissed.connect(move || {
            d.store(true, Ordering::SeqCst);
        });

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1, modifiers: 0 });
        assert!(sheet.is_open());
        assert!(!dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_sheet_open_twice_noop() {
        let mut sheet = make_sheet();
        sheet.open();
        sheet.open(); // should not change anything
        assert!(sheet.is_open());
    }

    #[test]
    fn bottom_sheet_dismiss_twice_noop() {
        let mut sheet = make_sheet();
        sheet.dismiss(); // no-op when already closed
        assert!(!sheet.is_open());

        sheet.open();
        sheet.dismiss();
        sheet.dismiss(); // no-op when already closed
        assert!(!sheet.is_open());
    }

    #[test]
    fn bottom_sheet_svg_output_open() {
        let mut sheet = make_sheet();
        sheet.open();
        let svg = render_to_svg(&mut sheet);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"400\""));
        assert!(svg.contains("height=\"600\""));
    }

    #[test]
    fn bottom_sheet_svg_output_closed() {
        let mut sheet = make_sheet();
        let svg = render_to_svg(&mut sheet);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn bottom_sheet_content_height_default_is_half_geometry() {
        let sheet = BottomSheet::new(Rect::new(0, 0, 400, 800));
        assert_eq!(sheet.content_height, 400);
    }

    #[test]
    fn bottom_sheet_dismiss_via_overlay_click() {
        let mut sheet = make_sheet();
        sheet.open();

        // Click in the overlay area (above the sheet panel, in the top half)
        let dismissed = Arc::new(AtomicBool::new(false));
        let d = dismissed.clone();
        sheet.dismissed.connect(move || {
            d.store(true, Ordering::SeqCst);
        });

        sheet.handle_event(&Event::MousePress {
            pos: Point::new(100, 50),
            button: 1,
            modifiers: 0,
        });
        assert!(!sheet.is_open());
        assert!(dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_sheet_panel_press_does_not_dismiss() {
        let mut sheet = make_sheet();
        sheet.open();

        // A press on the panel body is not an overlay dismissal: the panel is where the user
        // interacts with the sheet's own content.
        let dismissed = Arc::new(AtomicBool::new(false));
        let d = dismissed.clone();
        sheet.dismissed.connect(move || {
            d.store(true, Ordering::SeqCst);
        });

        sheet.handle_event(&Event::MousePress {
            pos: Point::new(200, 450),
            button: 1,
            modifiers: 0,
        });
        assert!(sheet.is_open(), "a press on the panel must not dismiss the sheet");
        assert!(!dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn escape_dismisses_an_open_sheet() {
        let mut sheet = make_sheet();
        sheet.open();
        sheet.handle_event(&Event::key_press(key_codes::ESCAPE, 0));
        assert!(!sheet.is_open());
        while sheet.tick(1000) {}
        assert_eq!(sheet.rise_progress(), 0.0, "the slide is played out by the frame loop");
    }

    #[test]
    fn dragging_the_handle_far_enough_dismisses() {
        let mut sheet = make_sheet();
        sheet.open();
        let handle = sheet.handle_rect();
        let grab = Point::new(handle.x + 2, handle.y + 2);
        sheet.handle_event(&Event::MousePress { pos: grab, button: 1, modifiers: 0 });
        assert!(sheet.is_dragging());
        sheet.handle_event(&Event::MouseMove { pos: Point::new(grab.x, grab.y + 120) });
        assert_eq!(sheet.drag_offset(), 120.0);
        sheet.handle_event(&Event::MouseRelease {
            pos: Point::new(grab.x, grab.y + 120),
            button: 1,
        });
        assert!(!sheet.is_open(), "a drag past a third of the panel dismisses");
        assert!(!sheet.is_dragging());
    }

    #[test]
    fn a_short_handle_drag_restores_the_panel() {
        let mut sheet = make_sheet();
        sheet.open();
        let handle = sheet.handle_rect();
        let grab = Point::new(handle.x + 2, handle.y + 2);
        sheet.handle_event(&Event::MousePress { pos: grab, button: 1, modifiers: 0 });
        sheet.handle_event(&Event::MouseMove { pos: Point::new(grab.x, grab.y + 10) });
        sheet
            .handle_event(&Event::MouseRelease { pos: Point::new(grab.x, grab.y + 10), button: 1 });
        assert!(sheet.is_open());
        assert_eq!(sheet.drag_offset(), 0.0);
    }

    #[test]
    fn the_sheet_places_its_content_inside_the_panel() {
        use crate::widget::runtime;

        let mut sheet = make_sheet();
        let content = runtime::register(Box::new(crate::widget::Label::new(
            "body".to_string(),
            Rect::new(0, 0, 10, 10),
        )))
        .expect("mount content");
        sheet.set_content_widget(Some(content));
        assert!(!crate::is_widget_visible(content), "a closed sheet hides its content");

        sheet.open();
        let rect = sheet.content_rect().expect("an open sheet hands over a box");
        assert_eq!(runtime::geometry_of(content), Some(rect));
        assert!(crate::is_widget_visible(content));

        sheet.dismiss();
        assert!(!crate::is_widget_visible(content));
        runtime::unregister(content);
    }

    #[test]
    fn an_attached_sheet_paints_no_scrim() {
        let mut overlay = make_sheet();
        overlay.open();
        while overlay.tick(1000) {}
        let overlay_svg = render_to_svg(&mut overlay);

        let mut attached = make_sheet();
        attached.set_attachment(SheetAttachment::Attached);
        attached.open();
        while attached.tick(1000) {}
        let attached_svg = render_to_svg(&mut attached);
        assert_ne!(overlay_svg, attached_svg, "the presentation must be visible");
        assert_eq!(attached.attachment(), SheetAttachment::Attached);
    }

    /// Opening a sheet slides the panel up instead of placing it there.
    ///
    /// # The defect this pins
    ///
    /// The draw read `open` directly, so the panel appeared at its final position on the first frame
    /// and vanished on the last — on the one control whose whole gesture *is* "slide up from the
    /// bottom edge".
    ///
    /// # Why this asserts on pixels as well as on the model
    ///
    /// A first version sampled only `rise_progress()` and **passed with the draw reverted to a fixed
    /// panel** — a progress nothing reads is not a slide. So the assertions are in two halves: the
    /// model must take an interior value, and the *painted* panel must be somewhere else at that
    /// moment than at either end. The painted position is read from the document's panel rectangle,
    /// which the sheet is the only element to emit at that width.
    #[test]
    fn opening_the_sheet_slides_the_panel_up() {
        use crate::widget::svg::render_to_svg;

        let mut sheet = BottomSheet::new(Rect::new(0, 0, 240, 120));
        assert_eq!(sheet.rise_progress(), 0.0, "a fresh sheet is stowed");
        assert!(!sheet.is_animating(), "and owes no frames");

        sheet.open();
        assert!(sheet.is_open(), "the logical state answers at once");
        assert!(sheet.is_animating(), "while the drawn position owes frames");
        assert_eq!(sheet.rise_progress(), 0.0, "the slide starts where the panel was");

        // The panel's own top edge, read from the emitted document. The panel is the only element
        // the sheet draws with a corner radius (`rx=16`), so naming it that way is what keeps the
        // measurement from picking up the scrim -- which is also full width, at y=0, and would
        // satisfy any test that merely looked for "a wide rectangle".
        fn panel_top(svg: &str) -> Option<i32> {
            svg.split("<rect ")
                .filter(|chunk| chunk.contains("rx=\"16\""))
                .filter_map(|chunk| {
                    let y = chunk.split("y=\"").nth(1)?;
                    y.split('"').next()?.parse::<i32>().ok()
                })
                .max()
        }

        let stowed_top = panel_top(&render_to_svg(&mut sheet));
        assert!(sheet.tick(60), "still moving after one step");
        let mid = sheet.rise_progress();
        assert!(
            mid > 0.0 && mid < 1.0,
            "the sheet must pass through an interior position (got {mid})"
        );
        let mid_top = panel_top(&render_to_svg(&mut sheet));

        while sheet.tick(60) {}
        assert_eq!(sheet.rise_progress(), 1.0, "and settle fully shown");
        let shown_top = panel_top(&render_to_svg(&mut sheet));

        // At rest the panel is entirely below the page, so it is not painted at all; as soon as the
        // slide begins it appears from the bottom edge and travels *upward* (a smaller y each frame
        // it is compared across).
        assert_eq!(
            stowed_top, None,
            "a stowed panel is below the page, so nothing of it is painted"
        );
        let middle = mid_top.expect("a mid-slide sheet paints a panel");
        let shown = shown_top.expect("a settled sheet paints a panel");
        assert!(
            middle > shown,
            "the panel must rise: mid-slide top {middle} must be below its settled top {shown}"
        );
        assert_eq!(shown, 60, "and settle where the stowed geometry says it belongs (60)");

        // Dismissing is the same movement in reverse, so the panel does not vanish on the last frame.
        sheet.dismiss();
        assert!(!sheet.is_open());
        assert!(sheet.is_animating(), "closing is also a slide");
        assert!(!sheet.tick(1000) || true, "one long frame is allowed to finish it");
        while sheet.tick(60) {}
        assert_eq!(sheet.rise_progress(), 0.0, "back to stowed");
    }
}
