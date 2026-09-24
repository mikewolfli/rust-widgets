// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Frame widget.
use crate::compat::{Rc, RefCell, ToString};
use crate::core::{Color, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;

use crate::render::{Bevel, BevelDirection};
use crate::widget::capability::coercion::{expect_f32, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Frame widget.
pub struct Frame {
    base: BaseWidget,
    frame_shape: FrameShape,
    frame_shadow: FrameShadow,
    line_width: f32,
    mid_line_width: f32,
    widget: Option<ObjectId>,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}
/// Frame shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameShape {
    /// No frame
    NoFrame,
    /// Box frame
    #[default]
    Box,
    /// Panel frame
    Panel,
    /// Styled panel frame
    StyledPanel,
    /// HLine frame
    HLine,
    /// VLine frame
    VLine,
    /// WinPanel frame
    WinPanel,
}

impl FrameShape {
    /// The factory spelling of this shape, matching `FRAME_PROPERTIES`.
    pub fn as_str(self) -> &'static str {
        match self {
            FrameShape::NoFrame => "no_frame",
            FrameShape::Box => "box",
            FrameShape::Panel => "panel",
            FrameShape::StyledPanel => "styled_panel",
            FrameShape::HLine => "hline",
            FrameShape::VLine => "vline",
            FrameShape::WinPanel => "win_panel",
        }
    }

    /// Parses a factory spelling into a shape.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "no_frame" => FrameShape::NoFrame,
            "box" => FrameShape::Box,
            "panel" => FrameShape::Panel,
            "styled_panel" => FrameShape::StyledPanel,
            "hline" => FrameShape::HLine,
            "vline" => FrameShape::VLine,
            "win_panel" => FrameShape::WinPanel,
            _ => return None,
        })
    }
}
/// Frame shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameShadow {
    /// Plain shadow
    #[default]
    Plain,
    /// Raised shadow
    Raised,
    /// Sunken shadow
    Sunken,
}

impl FrameShadow {
    /// The factory spelling of this shadow, matching `FRAME_PROPERTIES`.
    pub fn as_str(self) -> &'static str {
        match self {
            FrameShadow::Plain => "plain",
            FrameShadow::Raised => "raised",
            FrameShadow::Sunken => "sunken",
        }
    }

    /// Parses a factory spelling into a shadow.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "plain" => FrameShadow::Plain,
            "raised" => FrameShadow::Raised,
            "sunken" => FrameShadow::Sunken,
            _ => return None,
        })
    }
}
impl Frame {
    /// Creates a frame.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Frame, geometry, "Frame"),
            frame_shape: FrameShape::Box,
            frame_shadow: FrameShadow::Plain,
            line_width: 1.0,
            mid_line_width: 0.0,
            widget: None,
            registry: None,
        }
    }
    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
        self.base.request_redraw();
    }
    /// Returns the shared widget registry, if set.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }
    /// Returns frame shape.
    pub fn frame_shape(&self) -> FrameShape {
        self.frame_shape
    }
    /// Sets frame shape.
    pub fn set_frame_shape(&mut self, shape: FrameShape) {
        self.frame_shape = shape;
        self.base.request_redraw();
    }
    /// Returns frame shadow.
    pub fn frame_shadow(&self) -> FrameShadow {
        self.frame_shadow
    }
    /// Sets frame shadow.
    pub fn set_frame_shadow(&mut self, shadow: FrameShadow) {
        self.frame_shadow = shadow;
        self.base.request_redraw();
    }
    /// Returns line width.
    pub fn line_width(&self) -> f32 {
        self.line_width
    }
    /// Sets line width.
    pub fn set_line_width(&mut self, width: f32) {
        self.line_width = width.max(0.0);
        self.base.request_redraw();
    }
    /// Returns mid line width.
    pub fn mid_line_width(&self) -> f32 {
        self.mid_line_width
    }
    /// Sets mid line width.
    pub fn set_mid_line_width(&mut self, width: f32) {
        self.mid_line_width = width.max(0.0);
        self.base.request_redraw();
    }
    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        // Remove old child from widget tree before replacing
        if let Some(old) = self.widget {
            self.base.request_redraw();
            self.base.remove_child(old);
        }
        self.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
        self.base.request_redraw();
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }
    /// Draws frame border.
    fn draw_frame(&self, context: &mut RenderContext) {
        let rect = self.geometry();
        match self.frame_shape {
            FrameShape::NoFrame => {}
            FrameShape::Box => self.draw_box_frame(context, rect),
            FrameShape::Panel => self.draw_panel_frame(context, rect),
            FrameShape::StyledPanel => self.draw_styled_panel_frame(context, rect),
            FrameShape::HLine => self.draw_hline_frame(context, rect),
            FrameShape::VLine => self.draw_vline_frame(context, rect),
            FrameShape::WinPanel => self.draw_win_panel_frame(context, rect),
        }
    }
    /// Draws box frame.
    fn draw_box_frame(&self, context: &mut RenderContext, rect: Rect) {
        // A frame's whole appearance is its outline, so every literal below is the
        // fallback for `border_color`. Two of them are *derived* rather than dropped:
        //
        //  * The raised/sunken bevel needs two tones that read as "lit" and "shaded".
        //    They are made by lightening/darkening the border colour, which is what
        //    keeps the 3D effect readable on a dark surface instead of hardcoding a
        //    white highlight that would glow.
        //  * Explicit black/white border colours are recognised and the bevel is
        //    suppressed, because that is the only nonzero `border_width` a theme can
        //    express today; inventing a white highlight on top of a theme that asked
        //    for a plain black rule would be inventing chrome the caller did not ask
        //    for. An unset or grey border keeps the historical light/dark bevel.
        let style = self.style();
        let border = style.border_color.unwrap_or(Color::rgb(0, 0, 0));
        let themed = style.border_color.is_some();
        let plain =
            themed && (border == Color::rgb(0, 0, 0) || border == Color::rgb(255, 255, 255));
        match self.frame_shadow {
            FrameShadow::Plain => {
                // Draw single border
                context.draw_rect(rect, border);
            }
            FrameShadow::Raised => {
                // Draw raised border
                //
                // # Why this is now the shared primitive
                //
                // The four lines below and their two tones were spelled out here, and the same
                // relationship was spelled out again in the `Sunken` arm (with the tones exchanged)
                // and a third time in `draw_win_panel_frame`. "Which edges are lit" was therefore
                // carried by the *order of two nearly identical blocks* — so reversing an inset was a
                // copy-paste edit that nothing could check.
                //
                // `Bevel` states that relationship once: a direction decides which pair of edges
                // gets which tone. The **derivation is unchanged** (base blended 0.5 toward white and
                // black), which is why this is a refactor rather than a visual change — the
                // byte-identical snapshot is the evidence, and `Bevel`'s own tests pin the two
                // expressions.
                //
                // The plain-border case still suppresses the bevel by handing `Bevel` the same tone
                // for both sides: a theme that asked for a plain black rule must not get a white
                // highlight invented on top of it.
                let bevel = if plain {
                    Bevel::from_tones(border, border, border)
                } else if themed {
                    Bevel::from_base(border)
                } else {
                    // No theme: the historical hard-coded pair, which is deliberately *not* derived
                    // from the fallback black border — `rgb(0,0,0)` blended would be `rgb(0,0,0)`,
                    // and the bevel would vanish on exactly the build that has no palette to fall
                    // back on.
                    Bevel::from_tones(
                        Color::rgb(128, 128, 128),
                        Color::rgb(255, 255, 255),
                        Color::rgb(128, 128, 128),
                    )
                }
                .with_direction(BevelDirection::Raised);
                // One pixel, because that is what the `draw_line` calls this replaces painted; the
                // primitive's width support is for callers that state a `line_width`.
                bevel.stroke(context, rect, 1);

                // The mid lines are a **four-line groove**, not the inner half of a two-line bevel:
                // a softer highlight and a *deeper* shade, which is why they keep their own weights
                // rather than `Bevel::stroke_inner`. See `BEVEL_INNER_SHADE_WEIGHT`'s note — the
                // numbers are right for this shape and wrong for the other one.
                let mid_light = if themed {
                    border.blend(&Color::rgb(255, 255, 255), 0.25)
                } else {
                    Color::rgb(192, 192, 192)
                };
                let mid_dark = if themed {
                    border.blend(&Color::rgb(0, 0, 0), 0.75)
                } else {
                    Color::rgb(64, 64, 64)
                };
                self.draw_mid_lines(context, rect, mid_light, mid_dark);
            }
            FrameShadow::Sunken => {
                // Draw sunken border
                let light_color = if plain {
                    border
                } else if themed {
                    border.blend(&Color::rgb(0, 0, 0), 0.5)
                } else {
                    Color::rgb(128, 128, 128)
                };
                let dark_color = if plain {
                    border
                } else if themed {
                    border.blend(&Color::rgb(255, 255, 255), 0.5)
                } else {
                    Color::rgb(255, 255, 255)
                };
                // Top and left (dark)
                context.draw_line(
                    Point::from_f32(rect.x as f32, rect.y as f32),
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
                    light_color,
                );
                context.draw_line(
                    Point::from_f32(rect.x as f32, rect.y as f32),
                    Point::from_f32(rect.x as f32, rect.y as f32 + rect.height as f32),
                    light_color,
                );
                // Bottom and right (light)
                context.draw_line(
                    Point::from_f32(rect.x as f32, rect.y as f32 + rect.height as f32),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32,
                        rect.y as f32 + rect.height as f32,
                    ),
                    dark_color,
                );
                context.draw_line(
                    Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
                    Point::from_f32(
                        rect.x as f32 + rect.width as f32,
                        rect.y as f32 + rect.height as f32,
                    ),
                    dark_color,
                );
                // Draw mid line if needed
                let mid_light = if themed {
                    border.blend(&Color::rgb(0, 0, 0), 0.75)
                } else {
                    Color::rgb(64, 64, 64)
                };
                let mid_dark = if themed {
                    border.blend(&Color::rgb(255, 255, 255), 0.25)
                } else {
                    Color::rgb(192, 192, 192)
                };
                self.draw_mid_lines(context, rect, mid_light, mid_dark);
            }
        }
    }
    /// Draws the mid-line section shared by Raised/Sunken box frames.
    fn draw_mid_lines(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        mid_light: Color,
        mid_dark: Color,
    ) {
        let line_width = self.line_width;
        let mid_line_width = self.mid_line_width;
        if mid_line_width <= 0.0 {
            return;
        }
        context.draw_line(
            Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
            Point::from_f32(
                rect.x as f32 + rect.width as f32 - line_width,
                rect.y as f32 + line_width,
            ),
            mid_light,
        );
        context.draw_line(
            Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
            Point::from_f32(
                rect.x as f32 + line_width,
                rect.y as f32 + rect.height as f32 - line_width,
            ),
            mid_light,
        );
        context.draw_line(
            Point::from_f32(
                rect.x as f32 + line_width,
                rect.y as f32 + rect.height as f32 - line_width,
            ),
            Point::from_f32(
                rect.x as f32 + rect.width as f32 - line_width,
                rect.y as f32 + rect.height as f32 - line_width,
            ),
            mid_dark,
        );
        context.draw_line(
            Point::from_f32(
                rect.x as f32 + rect.width as f32 - line_width,
                rect.y as f32 + line_width,
            ),
            Point::from_f32(
                rect.x as f32 + rect.width as f32 - line_width,
                rect.y as f32 + rect.height as f32 - line_width,
            ),
            mid_dark,
        );
    }
    /// Draws panel frame with a subtle background fill and simple border.
    fn draw_panel_frame(&self, context: &mut RenderContext, rect: Rect) {
        let style = self.style();
        let bg_color = style.background_color.unwrap_or(Color::rgb(236, 233, 216));
        context.fill_rect(rect, bg_color);
        context.draw_rect(rect, style.border_color.unwrap_or(Color::rgb(64, 64, 64)));
    }
    /// Draws styled panel frame.
    fn draw_styled_panel_frame(&self, context: &mut RenderContext, rect: Rect) {
        // More sophisticated panel with gradient
        let bg_color = self.style().background_color.unwrap_or(Color::rgb(240, 240, 240));
        context.fill_rect(rect, bg_color);
        self.draw_box_frame(context, rect);
    }
    /// Draws horizontal line frame.
    fn draw_hline_frame(&self, context: &mut RenderContext, rect: Rect) {
        let y = rect.y + (rect.height as i32) / 2;
        context.draw_line_stroke(
            Point::new(rect.x, y),
            Point::new(rect.x + rect.width as i32, y),
            self.style().border_color.unwrap_or(Color::rgb(0, 0, 0)),
            self.line_width as u32,
        );
    }
    /// Draws vertical line frame.
    fn draw_vline_frame(&self, context: &mut RenderContext, rect: Rect) {
        let x = rect.x + (rect.width as i32) / 2;
        context.draw_line_stroke(
            Point::new(x, rect.y),
            Point::new(x, rect.y + rect.height as i32),
            self.style().border_color.unwrap_or(Color::rgb(0, 0, 0)),
            self.line_width as u32,
        );
    }
    /// Draws Windows panel frame.
    fn draw_win_panel_frame(&self, context: &mut RenderContext, rect: Rect) {
        // Windows-style panel
        //
        // The fill is themed, but the white/grey bevel is not: it is the illusion of
        // a raised edge, so unlike `draw_box_frame` it is left alone rather than
        // derived from the border colour. A panel that has asked for a dark background
        // still wants its highlight to read as a highlight.
        let style = self.style();
        let bg_color = style.background_color.unwrap_or(Color::rgb(240, 240, 240));
        context.fill_rect(rect, bg_color);
        // Draw 3D border
        let light_color = Color::rgb(255, 255, 255);
        let dark_color = Color::rgb(128, 128, 128);
        // Outer border (sunken)
        context.draw_line(
            Point::from_f32(rect.x as f32, rect.y as f32),
            Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
            dark_color,
        );
        context.draw_line(
            Point::from_f32(rect.x as f32, rect.y as f32),
            Point::from_f32(rect.x as f32, rect.y as f32 + rect.height as f32),
            dark_color,
        );
        context.draw_line(
            Point::from_f32(rect.x as f32, rect.y as f32 + rect.height as f32),
            Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32 + rect.height as f32),
            light_color,
        );
        context.draw_line(
            Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32),
            Point::from_f32(rect.x as f32 + rect.width as f32, rect.y as f32 + rect.height as f32),
            light_color,
        );
        // Inner border (raised)
        let inner_rect = Rect::new(
            rect.x + 1,
            rect.y + 1,
            rect.width.saturating_sub(2),
            rect.height.saturating_sub(2),
        );
        context.draw_line(
            Point::new(inner_rect.x, inner_rect.y),
            Point::new(inner_rect.x + inner_rect.width as i32, inner_rect.y),
            light_color,
        );
        context.draw_line(
            Point::new(inner_rect.x, inner_rect.y),
            Point::new(inner_rect.x, inner_rect.y + inner_rect.height as i32),
            light_color,
        );
        context.draw_line(
            Point::new(inner_rect.x, inner_rect.y + inner_rect.height as i32),
            Point::new(
                inner_rect.x + inner_rect.width as i32,
                inner_rect.y + inner_rect.height as i32,
            ),
            dark_color,
        );
        context.draw_line(
            Point::new(inner_rect.x + inner_rect.width as i32, inner_rect.y),
            Point::new(
                inner_rect.x + inner_rect.width as i32,
                inner_rect.y + inner_rect.height as i32,
            ),
            dark_color,
        );
    }
}
// Implement Widget trait
impl Widget for Frame {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(200, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Frame`'s property contract.
///
/// # Why this is not empty
///
/// The old dispatch grouped `WidgetKind::Panel | WidgetKind::Frame` into the
/// breadcrumb arm, which only ever matched `Breadcrumb` instances and answered
/// `UnsupportedOnWidget` for a `Frame`. Declaring nothing was honest about *that*
/// bug but wrong about the control: `Frame` has four real, independently settable
/// fields (`frame_shape`, `frame_shadow`, `line_width`, `mid_line_width`), and back
/// when it had no capability the factory could not construct one at all —
/// `factory_name_for_kind(WidgetKind::Frame)` returned `""`, so
/// `create_frame(..)` silently produced id `0`.
///
/// The names match `FRAME_PROPERTIES` one for one; a token that appears in the
/// schema but not in the `set` match below is a property that answers
/// `UnknownProperty` on write, which
/// `published_enum_tokens_are_accepted_by_their_control` rejects.
impl WidgetProperties for Frame {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "frame_shape" => Ok(CapabilityValue::String(self.frame_shape.as_str().to_string())),
            "frame_shadow" => Ok(CapabilityValue::String(self.frame_shadow.as_str().to_string())),
            "line_width" => Ok(CapabilityValue::Float(self.line_width as f64)),
            "mid_line_width" => Ok(CapabilityValue::Float(self.mid_line_width as f64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "frame_shape" => {
                let text = expect_string(value)?;
                let Some(shape) = FrameShape::from_name(&text) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_frame_shape(shape);
                Ok(())
            }
            "frame_shadow" => {
                let text = expect_string(value)?;
                let Some(shadow) = FrameShadow::from_name(&text) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_frame_shadow(shadow);
                Ok(())
            }
            "line_width" => {
                self.set_line_width(expect_f32(value)?);
                Ok(())
            }
            "mid_line_width" => {
                self.set_mid_line_width(expect_f32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `FRAME_PROPERTIES`.
        property_names_of![
            "frame_shape",
            "frame_shadow",
            "line_width",
            "mid_line_width",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for Frame {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Skip event forwarding when disabled
        if !self.base.is_enabled() {
            return;
        }
        // Forward events to widget via registry
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().forward_event(widget_id, event);
            }
        }
    }
}
impl Draw for Frame {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw frame
        self.draw_frame(context);
        // Draw widget via registry
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().draw_widget(widget_id, context);
            }
        }
        // Dim content when disabled.
        //
        // A **recession toward the surface**, not a grey wash: the fixed
        // `rgba(128,128,128,80)` this used to paint has no direction, so it lightened the contents
        // on a dark appearance and darkened them on a light one — "disabled" reading as "more
        // contrast" exactly where legibility was worst. Same defect and same fix as the modal
        // scrim (BLUE21 B23); see [`dimensions::DISABLED_VEIL_ALPHA`].
        if !self.base.is_enabled() {
            let rect = self.geometry();
            let surface = self.style().background_color.or_else(|| {
                crate::style::theme_manager().current_theme().map(|active| active.colors.background)
            });
            match surface {
                Some(surface) => {
                    context.fill_rect(rect, surface.with_alpha(dimensions::DISABLED_VEIL_ALPHA))
                }
                // No surface to fade toward: the historical grey is the honest fallback, and it is
                // the one case where a direction cannot be derived.
                None => context.fill_rect(rect, Color::rgba(128, 128, 128, 80)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{MiniToString, String};
    use crate::core::{Color, ObjectId, Rect};
    use crate::style::WidgetStyle;

    #[test]
    fn frame_creation_defaults() {
        let f = Frame::new(Rect::new(0, 0, 200, 100));
        assert_eq!(f.frame_shape(), FrameShape::Box);
        assert_eq!(f.frame_shadow(), FrameShadow::Plain);
        assert!((f.line_width() - 1.0).abs() < f32::EPSILON);
        assert!((f.mid_line_width() - 0.0).abs() < f32::EPSILON);
        assert!(f.widget().is_none());
        assert!(f.registry().is_none());
    }

    #[test]
    fn frame_shape_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_frame_shape(FrameShape::Panel);
        assert_eq!(f.frame_shape(), FrameShape::Panel);
        f.set_frame_shape(FrameShape::NoFrame);
        assert_eq!(f.frame_shape(), FrameShape::NoFrame);
        f.set_frame_shape(FrameShape::StyledPanel);
        assert_eq!(f.frame_shape(), FrameShape::StyledPanel);
        f.set_frame_shape(FrameShape::HLine);
        assert_eq!(f.frame_shape(), FrameShape::HLine);
        f.set_frame_shape(FrameShape::VLine);
        assert_eq!(f.frame_shape(), FrameShape::VLine);
        f.set_frame_shape(FrameShape::WinPanel);
        assert_eq!(f.frame_shape(), FrameShape::WinPanel);
        f.set_frame_shape(FrameShape::Box);
        assert_eq!(f.frame_shape(), FrameShape::Box);
    }

    #[test]
    fn frame_shadow_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_frame_shadow(FrameShadow::Raised);
        assert_eq!(f.frame_shadow(), FrameShadow::Raised);
        f.set_frame_shadow(FrameShadow::Sunken);
        assert_eq!(f.frame_shadow(), FrameShadow::Sunken);
        f.set_frame_shadow(FrameShadow::Plain);
        assert_eq!(f.frame_shadow(), FrameShadow::Plain);
    }

    #[test]
    fn frame_line_width_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_line_width(3.0);
        assert!((f.line_width() - 3.0).abs() < f32::EPSILON);
        f.set_line_width(0.5);
        assert!((f.line_width() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn frame_mid_line_width_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_mid_line_width(2.0);
        assert!((f.mid_line_width() - 2.0).abs() < f32::EPSILON);
        f.set_mid_line_width(0.0);
        assert!((f.mid_line_width() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn frame_widget_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.widget().is_none());
        let wid: ObjectId = 42;
        f.set_widget(Some(wid));
        assert_eq!(f.widget(), Some(wid));
        f.set_widget(None);
        assert!(f.widget().is_none());
    }

    #[test]
    fn frame_geometry_delegation() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_geometry(Rect::new(10, 20, 300, 150));
        assert_eq!(f.geometry(), Rect::new(10, 20, 300, 150));
    }

    #[test]
    fn frame_visibility() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.is_visible());
        f.hide();
        assert!(!f.is_visible());
        f.show();
        assert!(f.is_visible());
    }

    #[test]
    fn frame_enabled() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.is_enabled());
        f.set_enabled(false);
        assert!(!f.is_enabled());
        f.set_enabled(true);
        assert!(f.is_enabled());
    }

    #[test]
    fn frame_parent_children() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.parent().is_none());
        let pid: ObjectId = 10;
        f.set_parent(Some(pid));
        assert_eq!(f.parent(), Some(pid));
        f.set_parent(None);
        assert!(f.parent().is_none());

        let cid: ObjectId = 20;
        f.add_child(cid);
        assert_eq!(f.children().len(), 1);
        assert_eq!(f.children()[0], cid);
        f.remove_child(cid);
        assert!(f.children().is_empty());
    }

    #[test]
    fn frame_tooltip_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.tooltip().is_empty());
        f.set_tooltip("Frame info".to_string());
        assert_eq!(f.tooltip(), "Frame info");
        f.set_tooltip(String::new());
        assert!(f.tooltip().is_empty());
    }

    #[test]
    fn frame_style_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert_eq!(*f.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::rgb(240, 240, 240));
        f.set_style(custom.clone());
        assert_eq!(*f.style(), custom);
    }

    #[test]
    fn frame_id_kind() {
        let f_a = Frame::new(Rect::new(0, 0, 100, 50));
        let f_b = Frame::new(Rect::new(0, 0, 100, 50));
        assert_ne!(f_a.id(), f_b.id());
        assert_eq!(f_a.kind(), WidgetKind::Frame);
        assert_eq!(f_b.kind(), WidgetKind::Frame);
    }

    /// A raised box frame paints the **two tones the hand-written code painted**, on the same edges.
    ///
    /// # Why this is a pixel assertion and not a snapshot assertion
    ///
    /// The exported `frame` uses the **default** `FrameShadow::Plain`, which paints a single border
    /// and no bevel at all — so `snapshots/svg/frame.svg` never exercises this branch, and "the
    /// snapshot is byte-identical" would be evidence of nothing. This test drives the branch
    /// directly and reads the rendered pixels.
    ///
    /// # What it pins about the migration
    ///
    /// `draw_box_frame`'s `Raised` arm used to spell out four `draw_line` calls and two
    /// `border.blend(...)` expressions; it now hands a `Bevel` a direction. The *derivation* is
    /// required to be unchanged, or the migration is a visual edit wearing the word "refactor":
    ///
    /// * the top and left edges carry `border.blend(WHITE, 0.5)`,
    /// * the bottom and right edges carry `border.blend(BLACK, 0.5)`,
    /// * each edge is **one pixel** wide, which is what `draw_line` painted.
    ///
    /// The tones are computed here from `Color` rather than through `Bevel`, so a mutation inside
    /// the primitive cannot make the expectation move with it.
    #[test]
    fn a_raised_box_frame_paints_the_hand_written_tones() {
        use crate::core::{Color, Font, Size};
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        // Silence an unused-import warning in the profiles that gate `Font` out of this path.
        let _ = Font::default();

        let border = Color::rgb(120, 140, 160);
        let rect = Rect::new(4, 4, 12, 8);
        let mut frame = Frame::new(rect);
        frame.set_frame_shadow(FrameShadow::Raised);
        frame.set_style(WidgetStyle::default().with_border(border, 1, 0));

        let mut backend = SoftwarePaintBackend::new(Size::new(24, 20), 1.0);
        backend.begin_frame(Color::rgba(0, 0, 0, 0));
        {
            let mut context = RenderContext::new(&mut backend);
            frame.draw(&mut context);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        let pixel = |x: i32, y: i32| -> Color {
            let at = ((y as u32 * 24 + x as u32) * 4) as usize;
            Color::rgba(rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3])
        };

        let lit = border.blend(&Color::WHITE, 0.5);
        let shaded = border.blend(&Color::BLACK, 0.5);
        let mid_x = rect.x + rect.width as i32 / 2;
        let mid_y = rect.y + rect.height as i32 / 2;

        assert_eq!(pixel(mid_x, rect.y), lit, "the top edge must be the highlight");
        assert_eq!(pixel(rect.x, mid_y), lit, "the left edge must be the highlight");
        assert_eq!(
            pixel(mid_x, rect.y + rect.height as i32),
            shaded,
            "the bottom edge must be the shade"
        );
        assert_eq!(
            pixel(rect.x + rect.width as i32, mid_y),
            shaded,
            "the right edge must be the shade"
        );
        // One pixel wide: the row *inside* the top edge is not part of the bevel.
        assert_ne!(
            pixel(mid_x, rect.y + 1),
            lit,
            "a one-pixel bevel must not bleed into the second row"
        );
    }

    /// A raised frame given a **plain black** border gets no invented highlight.
    ///
    /// The rule the hand-written arm documented: the only nonzero `border_width` a theme can express
    /// today is an explicit black or white rule, and inventing a white bevel on top of a theme that
    /// asked for a plain black line would be adding chrome the caller did not ask for.
    #[test]
    fn a_plain_border_suppresses_the_bevel_tones() {
        use crate::core::{Color, Size};
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

        let rect = Rect::new(4, 4, 12, 8);
        let mut frame = Frame::new(rect);
        frame.set_frame_shadow(FrameShadow::Raised);
        frame.set_style(WidgetStyle::default().with_border(Color::rgb(0, 0, 0), 1, 0));

        let mut backend = SoftwarePaintBackend::new(Size::new(24, 20), 1.0);
        backend.begin_frame(Color::rgba(0, 0, 0, 0));
        {
            let mut context = RenderContext::new(&mut backend);
            frame.draw(&mut context);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        let pixel = |x: i32, y: i32| -> Color {
            let at = ((y as u32 * 24 + x as u32) * 4) as usize;
            Color::rgba(rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3])
        };
        let mid_x = rect.x + rect.width as i32 / 2;
        let top = pixel(mid_x, rect.y);
        let bottom = pixel(mid_x, rect.y + rect.height as i32);
        assert_eq!(top, bottom, "a plain border must not light one edge and shade the other");
        assert_eq!(top, Color::rgb(0, 0, 0), "the border's own colour, nothing invented");
    }

    #[test]
    fn frame_signal_accessors() {
        let f = Frame::new(Rect::new(0, 0, 100, 50));
        let _hover = f.base().hover_signal();
        let _mouse_down = f.base().mouse_down_signal();
        let _mouse_up = f.base().mouse_up_signal();
    }

    /// A panel-shaped frame's fill follows the appearance.
    ///
    /// # What is actually being pinned, and what is **not**
    ///
    /// `draw_panel_frame`, `draw_styled_panel_frame` and `draw_win_panel_frame` each fall back to
    /// an opaque literal — `rgb(236,233,216)`, `rgb(240,240,240)`, `rgb(240,240,240)` — when no
    /// colour was set. Those literals look like the defect this appendix is full of (a constant that
    /// does not move with the theme), so this test was first written to pin them.
    ///
    /// It **does not**. Measured: `apply_active_theme` classifies a `frame` into a role and fills
    /// `style.background_color` with `surface_container`, so the literal arm is never reached in a
    /// themed build — and the reverse injection (putting the literal back) **still passed**. The
    /// mutation test is what caught that; a re-derivation that cannot fail is not a test.
    ///
    /// What it pins instead is the part that *is* uncovered: the exported `frame` uses the
    /// **default** `Box` shape, which paints no fill at all, so the three panel arms are reachable
    /// only through a caller that asks for a shape and **no snapshot ever does**. This is the only
    /// coverage those arms have, and it is what the appendix's `frame` row should say.
    #[test]
    #[cfg(device_profile)]
    fn a_panels_fill_moves_with_the_appearance() {
        let _guard = crate::theme::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let rect = Rect::new(0, 0, 100, 50);

        let fill_of = |appearance| {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut frame = Frame::new(rect);
            frame.set_frame_shape(FrameShape::Panel);
            crate::theme::apply_theme_to_widget(&mut frame);
            let surface = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let svg = crate::widget::svg::render_widget_to_svg_on(&mut frame, rect, surface);
            // The panel's fill is the first `fill="rgba(` after the backdrop.
            let key = "fill=\"rgba(";
            let backdrop = svg.find(key).expect("a backdrop") + key.len();
            let face = svg[backdrop..].find(key).expect("the panel's fill") + backdrop + key.len();
            let end = svg[face..].find(')').expect("the fill's close") + face;
            let mut parts = svg[face..end].split(',');
            let r = parts.next().and_then(|v| v.trim().parse().ok()).expect("r");
            let g = parts.next().and_then(|v| v.trim().parse().ok()).expect("g");
            let b = parts.next().and_then(|v| v.trim().parse().ok()).expect("b");
            (Color::rgb(r, g, b), surface)
        };

        let dark = fill_of(crate::theme::AppearanceMode::Dark);
        let light = fill_of(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark.0, light.0,
            "a panel's fill must follow the appearance; both were {:?}",
            dark.0
        );
        // Each must be the **surface role** rather than any literal — the panel reads
        // `surface_container`, which is a step off the page in that appearance, so the fill must
        // differ from the window it sits on. A literal that happened to equal the page would make
        // a panel invisible; the literals this replaced did exactly that in one appearance each.
        for (label, (fill, surface)) in [("dark", dark), ("light", light)] {
            assert_ne!(
                fill, surface,
                "the {label} panel fill {fill:?} must be a step off the page {surface:?}, or the \
                 panel has no edge to see"
            );
        }
    }
}
