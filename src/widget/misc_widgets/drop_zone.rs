// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! DropZone widget — a named drop target for drag-and-drop payloads.
//!
//! A [`DropZone`] accepts [`crate::event::dnd::DragPayload`]s whose `type_id`
//! matches an accepted MIME type. It renders a dashed border and a highlight when
//! a compatible drag hovers over it, and emits a `payload_dropped` signal when a
//! payload is committed onto it. It implements [`crate::event::dnd::DropTarget`]
//! so it can be handed directly to the drag session without an adapter.

use crate::core::{Color, Point, Rect};
use crate::event::dnd::{DragPayload, DropEffect, DropTarget};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A named accept region for drag-and-drop payloads.
///
/// The zone accepts exactly one MIME type (its `accepted_type`); a payload of any
/// other type is refused. A compatible drag hovering over it lights the zone up,
/// and committing the drop emits [`DropZone::payload_dropped`] with the payload.
pub struct DropZone {
    base: BaseWidget,
    /// The MIME type this zone accepts, e.g. `"text/plain"` or `"card"`.
    accepted_type: String,
    /// Whether a compatible drag is currently hovering over this zone.
    hovered: bool,
    /// Emitted, with the accepted payload, when a drop is committed onto the zone.
    pub payload_dropped: Signal1<DragPayload>,
}

impl DropZone {
    /// Creates an empty drop zone that accepts the given MIME type.
    pub fn new(geometry: Rect, accepted_type: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DropZone, geometry, "DropZone"),
            accepted_type: accepted_type.into(),
            hovered: false,
            payload_dropped: Signal1::new(),
        }
    }

    /// Returns the MIME type this zone accepts.
    pub fn accepted_type(&self) -> &str {
        &self.accepted_type
    }

    /// Sets the MIME type this zone accepts and clears any hover highlight.
    pub fn set_accepted_type(&mut self, accepted_type: impl Into<String>) {
        self.accepted_type = accepted_type.into();
        if self.hovered {
            self.hovered = false;
            self.base.request_redraw();
        }
    }

    /// Returns whether a compatible drag is currently hovering over the zone.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Clears any pending hover highlight, e.g. when a drag is cancelled.
    pub fn clear_hover(&mut self) {
        if self.hovered {
            self.hovered = false;
            self.base.request_redraw();
        }
    }

    /// Returns whether `payload` is of a type this zone accepts.
    pub fn accepts(&self, payload: &DragPayload) -> bool {
        payload.is_type(&self.accepted_type)
    }
}

impl DropTarget for DropZone {
    fn can_accept(&self, payload: &DragPayload) -> bool {
        self.accepts(payload)
    }

    fn on_drop(&mut self, payload: &DragPayload, _pos: Point) -> DropEffect {
        // `can_accept` already gated the type; a zone cannot refuse at commit time
        // because acceptance is purely a type match.
        self.payload_dropped.emit(payload.clone());
        self.hovered = false;
        self.base.request_redraw();
        DropEffect::Copy
    }

    fn preview_rect(&self, _payload: &DragPayload, _pos: Point) -> Option<Rect> {
        Some(self.geometry())
    }
}

impl Widget for DropZone {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 120)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `DropZone`'s property contract. `accepted_type` is readable/writable; the hover
/// flag and the dropped signal are behaviour, not data, so they are read-only
/// (hover) or absent (the signal is subscribed through the event bridge).
impl WidgetProperties for DropZone {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "accepted_type" => Ok(CapabilityValue::String(self.accepted_type().to_string())),
            "hovered" => Ok(CapabilityValue::Bool(self.is_hovered())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "accepted_type" => {
                self.set_accepted_type(expect_string(value)?);
                Ok(())
            }
            "hovered" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["accepted_type", "hovered", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for DropZone {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // The drop itself is driven by `DropTarget` through the drag session; here
        // the zone only needs to clear its hover state when the pointer leaves.
        match event {
            Event::MouseLeave { .. } => self.clear_hover(),
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for DropZone {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch would change nothing on screen, because the fills below
        // were previously hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("drop_zone");
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(36, 107, 201));
        // `drop_zone` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A well
        // painted in that colour would be byte-identical to the frame behind it, so the drop
        // target had no visible face at rest: `drop_zone.svg` carried `rgba(18,18,18)` twice and
        // only the dashed outline said a control was there. A resolved surface equal to the
        // window fill is therefore re-derived a visible step toward the ink, the same guard
        // `dial.rs` and `slider.rs` apply to their own faces. A drop zone has to show a well it
        // drops into, so the fallback steps *away* from the window rather than landing on it.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let surface = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.10),
        };
        let outline = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.45));

        // Base fill: the theme's surface, lifted toward its ink while hovering so the
        // drop target reads as active.
        let fill = if self.hovered { surface.blend(&ink, 0.12) } else { surface };
        context.fill_rect(rect, fill);

        // A dashed border drawn as a series of short strokes. Hovering draws it in the
        // control's own ink; at rest it is the resolved outline colour.
        let border = if self.hovered { ink } else { outline };
        let dash = 8u32;
        let mut x = rect.x;
        while x + dash as i32 <= rect.x + rect.width as i32 {
            context.draw_line_stroke(
                Point::new(x, rect.y),
                Point::new(x + dash as i32, rect.y),
                border,
                1,
            );
            context.draw_line_stroke(
                Point::new(x, rect.y + rect.height as i32 - 1),
                Point::new(x + dash as i32, rect.y + rect.height as i32 - 1),
                border,
                1,
            );
            x += (dash * 2) as i32;
        }
        let mut y = rect.y;
        while y + dash as i32 <= rect.y + rect.height as i32 {
            context.draw_line_stroke(
                Point::new(rect.x, y),
                Point::new(rect.x, y + dash as i32),
                border,
                1,
            );
            context.draw_line_stroke(
                Point::new(rect.x + rect.width as i32 - 1, y),
                Point::new(rect.x + rect.width as i32 - 1, y + dash as i32),
                border,
                1,
            );
            y += (dash * 2) as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone() -> DropZone {
        DropZone::new(Rect::new(0, 0, 200, 120), "text/plain")
    }

    #[test]
    fn new_zone_records_its_accepted_type() {
        let z = zone();
        assert_eq!(z.accepted_type(), "text/plain");
        assert!(!z.is_hovered());
    }

    #[test]
    fn accepts_only_matching_type_id() {
        let z = zone();
        assert!(z.accepts(&DragPayload::new("text/plain", "doc1")));
        assert!(!z.accepts(&DragPayload::new("image/png", "img1")));
    }

    #[test]
    fn on_drop_emits_payload_and_clears_hover() {
        let mut z = zone();
        let payload = DragPayload::new("text/plain", "doc1").with_label("doc1");

        let got = std::sync::Arc::new(std::sync::Mutex::new(None::<DragPayload>));
        let sink = got.clone();
        z.payload_dropped.connect(move |p| {
            *sink.lock().unwrap() = Some(p.as_ref().clone());
        });

        // Simulate a hover then a drop through the DropTarget trait directly.
        z.hovered = true;
        let effect = z.on_drop(&payload, Point::new(10, 10));

        assert_eq!(effect, DropEffect::Copy);
        assert!(!z.is_hovered());
        let recorded = got.lock().unwrap().clone();
        assert_eq!(recorded, Some(payload));
    }

    #[test]
    fn set_accepted_type_clears_hover() {
        let mut z = zone();
        z.hovered = true;
        z.set_accepted_type("application/json");
        assert!(!z.is_hovered());
        assert_eq!(z.accepted_type(), "application/json");
    }

    #[test]
    fn preview_rect_is_the_zone_geometry() {
        let z = zone();
        let payload = DragPayload::new("text/plain", "doc");
        assert_eq!(z.preview_rect(&payload, Point::new(0, 0)), Some(Rect::new(0, 0, 200, 120)));
    }
}
