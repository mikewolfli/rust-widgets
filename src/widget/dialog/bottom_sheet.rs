// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BottomSheet widget — a modal panel that slides up from the bottom edge of the
//! screen, similar to Android/Material Design bottom sheets.
//!
//! The BottomSheet displays a semi-transparent overlay behind a rounded-rect panel
//! at the bottom of its geometry. It supports open/close state, a drag handle at
//! the top of the sheet, and emits a `dismissed` signal when the user taps outside
//! or directly on the sheet area.

use crate::core::{Color, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::capability::coercion::{expect_bool, expect_f32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// BottomSheet widget — a modal panel that slides up from the bottom edge.
///
/// The sheet occupies the bottom `content_height` pixels of its geometry rect.
/// When `open` is true, a semi-transparent gray overlay fills the area above the
/// sheet and a rounded panel is drawn at the bottom with a small drag handle.
/// Any click dismisses the sheet and fires the `dismissed` signal.
pub struct BottomSheet {
    base: BaseWidget,
    open: bool,
    content_height: u32,
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
            dismissed: GenericSignal::new(),
        }
    }

    /// Opens the bottom sheet, making it visible.
    pub fn open(&mut self) {
        if !self.open {
            self.open = true;
            self.base.request_redraw();
        }
    }

    /// Dismisses (closes) the bottom sheet without emitting the dismissed signal.
    pub fn dismiss(&mut self) {
        if self.open {
            self.open = false;
            self.base.request_redraw();
        }
    }

    /// Sets the content height of the sheet panel (in pixels).
    ///
    /// The sheet is drawn at the bottom of the geometry rect using this height.
    /// Clamped to the geometry height.
    pub fn set_content_height(&mut self, height: u32) {
        self.content_height = height.min(self.base.geometry.height);
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
}

impl Widget for BottomSheet {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
        // The theme reads take and release the global manager's lock internally, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("bottom_sheet");
        // `bottom_sheet` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background` — the window's own fill. A
        // pane painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from
        // it, the same distinction `Colors::input_background` draws for a field.
        //
        // Read as its own lock acquisition and copied out as a value, so the guard is
        // dropped before anything else touches the theme.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(40, 40, 40));
        let sheet_color = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != sheet_color)
            .unwrap_or_else(|| sheet_color.blend(&ink, 0.35));
        let handle_color = ink.blend(&sheet_color, 0.35);

        let sheet_height = self.content_height.min(rect.height);

        // Sheet panel rect at the bottom of the geometry
        let sheet_y = rect.y + rect.height as i32 - sheet_height as i32;
        let sheet_rect = Rect::new(rect.x, sheet_y, rect.width, sheet_height);

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
        // `Colors.black54`, UIKit, Qt and SwiftUI all dim toward black); none blends toward
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
        const SCRIM_DARKEN: f32 = 0.32;
        // Drawn as an explicit darkened fill rather than as a translucent overlay, which
        // is how this control already painted it; `blend` carries the window fill's own
        // alpha through, so the scrim stays as opaque as the surface it stands in for.
        let scrim_color = window_fill.blend(&Color::BLACK, SCRIM_DARKEN);
        if overlay_height > 0 {
            let overlay_rect = Rect::new(rect.x, rect.y, rect.width, overlay_height);
            context.fill_rect(overlay_rect, scrim_color);
        }

        // A closed sheet paints only the scrim; the panel and its handle belong to the
        // open state alone.
        if !self.open {
            return;
        }

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
    }
}

impl EventHandler for BottomSheet {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos: _, button } => {
                if *button == 1 && self.open {
                    self.open = false;
                    self.dismissed.emit();
                    self.base.request_redraw();
                }
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

        sheet.handle_event(&Event::MousePress { pos: Point::new(200, 300), button: 1 });
        assert!(!sheet.is_open());
        assert!(dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_sheet_mouse_press_dismisses_when_open() {
        let mut sheet = make_sheet();
        sheet.open();
        assert!(sheet.is_open());

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1 });
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

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1 });
        assert!(!dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_sheet_other_button_noop() {
        let mut sheet = make_sheet();
        sheet.open();

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 2 });
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

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1 });
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

        sheet.handle_event(&Event::MousePress { pos: Point::new(100, 50), button: 1 });
        assert!(!sheet.is_open());
        assert!(dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_sheet_dismiss_via_sheet_click() {
        let mut sheet = make_sheet();
        sheet.open();

        // Click in the sheet area (bottom half)
        let dismissed = Arc::new(AtomicBool::new(false));
        let d = dismissed.clone();
        sheet.dismissed.connect(move || {
            d.store(true, Ordering::SeqCst);
        });

        sheet.handle_event(&Event::MousePress { pos: Point::new(200, 450), button: 1 });
        assert!(!sheet.is_open());
        assert!(dismissed.load(Ordering::SeqCst));
    }
}
