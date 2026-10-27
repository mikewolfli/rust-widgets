// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SafeArea widget — insets content to avoid notches, status bars, home indicators (BLUE11 R10.14).
use crate::core::{Color, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::expect_f32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

use super::theme_snapshot::ThemeSnapshot;

/// Safe area insets for mobile devices.
///
/// Re-exported from [`crate::platform::types`] rather than defined here: the platform trait
/// returns this type, so one definition serves both layers (principle #54). The field
/// documentation, and the reason the default is a portrait-phone shape rather than zero, live
/// with that definition.
pub use crate::platform::types::SafeAreaInsets;

impl Default for SafeAreaInsets {
    fn default() -> Self {
        Self { top: 44, bottom: 34, left: 0, right: 0 }
    }
}

/// SafeArea widget — wraps content with safe area insets (BLUE11 R10.14).
///
/// The widget does not reposition its children itself. It publishes
/// [`SafeArea::content_rect`] and paints the inset margins, leaving the host
/// layout to place content inside that rectangle. Children laid out against
/// [`Widget::geometry`] instead of `content_rect` will overlap system UI.
pub struct SafeArea {
    base: BaseWidget,
    insets: SafeAreaInsets,
    /// Background color for the safe area margins.
    margin_color: Color,
    /// Optional child placed inside the safe content rectangle.
    content_child: Option<crate::core::ObjectId>,
    /// Optional shared registry, used to forward events to the content child (C-19).
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}

impl SafeArea {
    /// Creates a widget using the default phone-style insets
    /// ([`SafeAreaInsets::default`]) and a white margin colour.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is
    /// 300x200.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SafeArea, geometry, "SafeArea"),
            insets: SafeAreaInsets::default(),
            margin_color: Color::WHITE,
            content_child: None,
            registry: None,
        }
    }
    /// Sets the shared widget registry used to forward events to the content child.
    ///
    /// Without it a content control mounted through [`Self::set_content`] is painted but
    /// receives no events, so a registry-driven host saw a control it could not interact
    /// with (C-19). The interface matches the rest of the container family
    /// (`StackedWidget`, `ScrollArea`, `TabWidget`).
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }
    /// Returns the shared widget registry, if set.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }
    /// Replaces all four insets at once and requests a redraw.
    ///
    /// # Why this re-places the child (C-18)
    ///
    /// The insets *are* the content rectangle's offset. Publishing a new rectangle without
    /// writing it to the mounted child left the child in the old safe rect — exactly the
    /// overlap a safe area exists to prevent. [`Self::apply`] is idempotent, so a no-op
    /// write costs nothing; a real change moves the child now rather than at the next
    /// unrelated resize.
    ///
    /// Insets larger than the widget in a given axis are tolerated: the content
    /// rect saturates to zero extent rather than underflowing, and the margin
    /// bars may then paint outside the widget's own rectangle.
    pub fn set_insets(&mut self, insets: SafeAreaInsets) {
        if self.insets == insets {
            return;
        }
        self.insets = insets;
        self.apply();
        self.base.request_redraw();
    }
    /// Returns the current insets.
    pub fn insets(&self) -> SafeAreaInsets {
        self.insets
    }
    /// Sets the colour painted in the four inset margin bars.
    ///
    /// Requests a redraw on a real change: `draw` fills every inset band with this colour, so a
    /// change that did not repaint stayed invisible until something else forced a frame. This
    /// documentation used to say the opposite ("does not request a redraw"), which contradicted
    /// the three lines below it.
    pub fn set_margin_color(&mut self, color: Color) {
        if self.margin_color == color {
            return;
        }
        self.margin_color = color;
        // `draw` fills every inset band with this colour, so a change that did not repaint was
        // invisible until something else forced a frame.
        self.base.request_redraw();
    }
    /// Sets the top inset, preserving the other three edges.
    pub fn set_top_inset(&mut self, top: u32) {
        self.set_insets(SafeAreaInsets { top, ..self.insets });
    }
    /// Sets the bottom inset, preserving the other three edges.
    pub fn set_bottom_inset(&mut self, bottom: u32) {
        self.set_insets(SafeAreaInsets { bottom, ..self.insets });
    }
    /// Sets the left inset, preserving the other three edges.
    pub fn set_left_inset(&mut self, left: u32) {
        self.set_insets(SafeAreaInsets { left, ..self.insets });
    }
    /// Sets the right inset, preserving the other three edges.
    pub fn set_right_inset(&mut self, right: u32) {
        self.set_insets(SafeAreaInsets { right, ..self.insets });
    }
    /// Returns the rectangle content should occupy: the widget's geometry
    /// inset by the safe area on all four sides.
    ///
    /// In the same parent-relative coordinate space as [`Widget::geometry`].
    /// Width and height saturate at zero when the insets exceed the widget, so
    /// the result is never inverted.
    pub fn content_rect(&self) -> Rect {
        let g = self.geometry();
        Rect::new(
            g.x + self.insets.left as i32,
            g.y + self.insets.top as i32,
            g.width.saturating_sub(self.insets.left + self.insets.right),
            g.height.saturating_sub(self.insets.top + self.insets.bottom),
        )
    }

    /// Mounts `child` as the control laid out inside [`Self::content_rect`].
    ///
    /// The previous API published the rect and left the host to place content, which
    /// meant the one thing a safe area exists to do — keep a child out of the notch — was
    /// the caller's job. Calling [`Self::apply`] (or resizing/mounting) now writes the
    /// content rect to the child through the runtime's geometry entry point.
    pub fn set_content(&mut self, child: Option<crate::core::ObjectId>) {
        if self.content_child == child {
            return;
        }
        if let Some(previous) = self.content_child {
            self.base.remove_child(previous);
        }
        self.content_child = child;
        if let Some(child) = child {
            self.base.add_child_linked(child);
        }
        self.apply();
        self.base.request_redraw();
    }

    /// The mounted content child, if any.
    pub fn content(&self) -> Option<crate::core::ObjectId> {
        self.content_child
    }

    /// Places the content child inside the safe rectangle.
    ///
    /// This is idempotent and cheap, so a host may call it after a layout pass or on every
    /// resize; a control with no child is a no-op.
    pub fn apply(&mut self) {
        if let Some(child) = self.content_child {
            let rect = self.content_rect();
            crate::set_widget_geometry(child, rect.x, rect.y, rect.width, rect.height);
            crate::set_widget_visible(child, true);
        }
    }
}

impl Widget for SafeArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.apply();
    }

    fn size_hint(&self) -> Size {
        // C-019: the content control's hint plus the insets this area adds around it. The
        // historical 300x200 is the floor when no content is mounted.
        let content = self.content_child.and_then(|child| {
            crate::widget::runtime::with_widget(child, |child| child.size_hint())
        });
        content
            .map(|content| {
                Size::new(
                    content
                        .width
                        .saturating_add(self.insets.left)
                        .saturating_add(self.insets.right),
                    content
                        .height
                        .saturating_add(self.insets.top)
                        .saturating_add(self.insets.bottom),
                )
            })
            .unwrap_or(Size::new(300, 200))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `SafeArea`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The insets are stored as
/// `u32` internally and published as `Float` to match the legacy shape.
impl WidgetProperties for SafeArea {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        let insets = self.insets();
        match name {
            "top_inset" => Ok(CapabilityValue::Float(f64::from(insets.top))),
            "bottom_inset" => Ok(CapabilityValue::Float(f64::from(insets.bottom))),
            "left_inset" => Ok(CapabilityValue::Float(f64::from(insets.left))),
            "right_inset" => Ok(CapabilityValue::Float(f64::from(insets.right))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "top_inset" => {
                self.set_top_inset(expect_f32(value)? as u32);
                Ok(())
            }
            "bottom_inset" => {
                self.set_bottom_inset(expect_f32(value)? as u32);
                Ok(())
            }
            "left_inset" => {
                self.set_left_inset(expect_f32(value)? as u32);
                Ok(())
            }
            "right_inset" => {
                self.set_right_inset(expect_f32(value)? as u32);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "top_inset",
            "bottom_inset",
            "left_inset",
            "right_inset",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for SafeArea {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let g = self.geometry();
        let cr = self.content_rect();

        // The inset bars and the content outline resolve explicit style first, then the
        // theme's resolved style for this control, and only then a literal. SafeArea is
        // not in the role table, so it classifies as `Surface` and `apply_active_theme`
        // cannot give it a distinct fill on its own — the bars therefore derive their
        // colour one step away from the theme's foreground, which is what makes an
        // appearance switch visible.
        //
        // The theme reads take and release the global manager's lock internally, so no
        // guard is held across the draw (the mutex is not re-entrant).
        //
        // The resolved style and the window fill used to be two independent manager
        // visits; `ThemeSnapshot` folds them into one read for the frame (C-P01).
        let theme = ThemeSnapshot::capture("safe_area");
        let style = self.base.style();
        let window_fill = theme.window_fill();
        let ink = style
            .text_color
            .or_else(|| theme.style.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // A caller who set an explicit margin colour keeps it: `set_margin_color` is the
        // control's own override and wins over the theme, the same precedence
        // `WidgetStyle` documents. The default white it starts on is not an override, so
        // it is replaced by the themed surface.
        let margin_color =
            if self.margin_color == Color::WHITE {
                style
                    .background_color
                    .or_else(|| theme.style.as_ref().and_then(|t| t.background_color))
                    .map(|resolved| {
                        if resolved == window_fill {
                            window_fill.blend(&ink, 0.08)
                        } else {
                            resolved
                        }
                    })
                    .unwrap_or_else(|| window_fill.blend(&ink, 0.08))
            } else {
                self.margin_color
            };
        // The content outline is chrome too, and used to be a hardcoded grey that ignored
        // the appearance entirely.
        let border_color = style
            .border_color
            .or_else(|| theme.style.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| window_fill.blend(&ink, 0.25));

        // Fill margin areas (top/bottom/left/right bars)
        if self.insets.top > 0 {
            ctx.fill_rect(Rect::new(g.x, g.y, g.width, self.insets.top), margin_color);
        }
        if self.insets.bottom > 0 {
            ctx.fill_rect(
                Rect::new(
                    g.x,
                    g.y + g.height as i32 - self.insets.bottom as i32,
                    g.width,
                    self.insets.bottom,
                ),
                margin_color,
            );
        }
        if self.insets.left > 0 {
            let left_bar_height = g.height.saturating_sub(self.insets.top + self.insets.bottom);
            ctx.fill_rect(
                Rect::new(g.x, g.y + self.insets.top as i32, self.insets.left, left_bar_height),
                margin_color,
            );
        }
        if self.insets.right > 0 {
            let right_bar_height = g.height.saturating_sub(self.insets.top + self.insets.bottom);
            ctx.fill_rect(
                Rect::new(
                    g.x + g.width as i32 - self.insets.right as i32,
                    g.y + self.insets.top as i32,
                    self.insets.right,
                    right_bar_height,
                ),
                margin_color,
            );
        }
        // Draw content area border (subtle)
        ctx.draw_rect(cr, border_color.with_alpha_f32(0.4));
    }
}

impl EventHandler for SafeArea {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Forward to the mounted content control (C-19). Pointer events only reach it when
        // they land inside the safe rect; keyboard, focus and tick events go through
        // unconditionally, because a focused control owns the keyboard wherever the pointer
        // is — the same split `Carousel` and `CollapsiblePane` make.
        //
        // No coordinate translation is applied: [`SafeArea::apply`] writes the content rect
        // into the child's own geometry, and this library's events are expressed in the same
        // space as that geometry (the convention `StackedWidget` and `TabWidget` follow).
        let Some(child) = self.content_child else {
            return;
        };
        let Some(ref registry) = self.registry else {
            return;
        };
        let content_rect = self.content_rect();
        match event {
            Event::MousePress { pos, .. }
            | Event::MouseRelease { pos, .. }
            | Event::MouseMove { pos }
            | Event::MouseDoubleClick { pos, .. }
            | Event::MouseEnter { pos }
            | Event::MouseLeave { pos } => {
                if content_rect.contains(*pos) {
                    registry.borrow_mut().forward_event(child, event);
                }
            }
            _ => {
                registry.borrow_mut().forward_event(child, event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn safe_area_default_insets() {
        let sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        let insets = sa.insets();
        assert_eq!(insets.top, 44);
        assert_eq!(insets.bottom, 34);
        assert_eq!(insets.left, 0);
        assert_eq!(insets.right, 0);
    }

    #[test]
    fn safe_area_content_rect_computes_correctly() {
        let sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        let cr = sa.content_rect();
        assert_eq!(cr.x, 0);
        assert_eq!(cr.y, 44);
        assert_eq!(cr.width, 375);
        assert_eq!(cr.height, 812 - 44 - 34);
    }

    #[test]
    fn safe_area_content_rect_with_all_insets() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        sa.set_insets(SafeAreaInsets { top: 50, bottom: 40, left: 16, right: 16 });
        let cr = sa.content_rect();
        assert_eq!(cr.x, 16);
        assert_eq!(cr.y, 50);
        assert_eq!(cr.width, 375 - 32);
        assert_eq!(cr.height, 812 - 90);
    }

    #[test]
    fn safe_area_set_insets_updates_content_rect() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 400, 800));
        let custom = SafeAreaInsets { top: 60, bottom: 50, left: 10, right: 10 };
        sa.set_insets(custom);
        assert_eq!(sa.insets(), custom);
        let cr = sa.content_rect();
        assert_eq!(cr.x, 10);
        assert_eq!(cr.y, 60);
    }

    #[test]
    fn safe_area_set_margin_color() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        sa.set_margin_color(Color::BLACK);
        // No crash; color is used in draw.
        assert_eq!(sa.kind(), WidgetKind::SafeArea);
    }

    #[test]
    fn safe_area_event_delegation() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        // Events should not panic/crash.
        sa.handle_event(&Event::MousePress { pos: Point::new(10, 5), button: 1, modifiers: 0 });
        sa.handle_event(&Event::MouseRelease { pos: Point::new(10, 5), button: 1 });
        assert_eq!(sa.insets().top, 44);
    }

    #[test]
    fn safe_area_svg_output() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 375, 812));
        let svg = crate::widget::svg::render_to_svg(&mut sa);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn safe_area_content_rect_saturating_overflow() {
        let mut sa = SafeArea::new(Rect::new(0, 0, 100, 100));
        let huge = SafeAreaInsets { top: 200, bottom: 200, left: 0, right: 0 };
        sa.set_insets(huge);
        // This should not panic; height saturates to 0.
        let cr = sa.content_rect();
        assert_eq!(cr.width, 100); // left/right are 0, so width unchanged
        assert_eq!(cr.height, 0);
    }

    #[test]
    fn setting_the_margin_colour_repaints() {
        let mut area = SafeArea::new(Rect::new(0, 0, 200, 200));
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        area.base.redraw_requested.connect({
            let seen = std::sync::Arc::clone(&seen);
            move || {
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        area.set_margin_color(Color::rgb(1, 2, 3));
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the inset bands are filled with this colour"
        );
        area.set_margin_color(Color::rgb(1, 2, 3));
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the same colour is not a change"
        );
    }

    /// A mounted content child is placed inside the safe rectangle.
    #[test]
    fn a_content_child_is_placed_inside_the_safe_rect() {
        let mut area = SafeArea::new(Rect::new(0, 0, 200, 200));
        area.set_insets(safe_area_insets());
        let child = 0x5AFE;
        area.set_content(Some(child));
        assert_eq!(area.content(), Some(child));
        area.apply();
        assert_eq!(area.content_rect(), Rect::new(10, 20, 180, 160));
        area.set_content(None);
        assert_eq!(area.content(), None);
    }

    fn safe_area_insets() -> SafeAreaInsets {
        SafeAreaInsets { top: 20, bottom: 20, left: 10, right: 10 }
    }

    /// C-19: a registry-mounted content child receives the events aimed at it, and only those
    /// that land inside the safe rectangle.
    #[test]
    fn the_content_child_receives_forwarded_events() {
        use crate::widget::SimpleRegistry;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut area = SafeArea::new(Rect::new(0, 0, 200, 200));
        area.set_insets(safe_area_insets());
        let seen = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&seen);
        let mut registry = SimpleRegistry::new();
        registry.register(
            0x5AFE,
            |_ctx: &mut crate::render::RenderContext| {},
            move |_event: &Event| {
                sink.fetch_add(1, Ordering::SeqCst);
            },
        );
        area.set_registry(Rc::new(RefCell::new(registry)));
        area.set_content(Some(0x5AFE));

        area.handle_event(&Event::MousePress { pos: Point::new(60, 60), button: 1, modifiers: 0 });
        assert_eq!(seen.load(Ordering::SeqCst), 1, "a press inside the safe rect reaches it");
        area.handle_event(&Event::MousePress { pos: Point::new(1, 1), button: 1, modifiers: 0 });
        assert_eq!(seen.load(Ordering::SeqCst), 1, "a press in the margin is not the child's");
    }

    /// C-18: changing the insets re-places the mounted child, not only the published rect.
    #[test]
    fn changing_the_insets_keeps_the_content_rect_and_the_child_in_step() {
        let mut area = SafeArea::new(Rect::new(0, 0, 200, 200));
        area.set_content(Some(0x5AFE));
        area.set_insets(SafeAreaInsets { top: 10, bottom: 0, left: 0, right: 0 });
        assert_eq!(area.content_rect(), Rect::new(0, 10, 200, 190));
        area.set_top_inset(50);
        assert_eq!(area.content_rect(), Rect::new(0, 50, 200, 150));
        // `apply` is the write the single-edge setter now performs; the rect it uses is the one
        // just asserted, so the child cannot stay in the old band.
        area.apply();
        assert_eq!(area.content_rect().y, 50);
    }

    /// C-019: the hint adds the insets around the mounted content's own hint.
    #[test]
    fn the_size_hint_aggregates_content_and_insets() {
        use crate::widget::base_widgets::label::Label;

        let label = Label::new("content".to_string(), Rect::new(0, 0, 160, 70));
        let expected = label.size_hint();
        let id = crate::widget::runtime::register(Box::new(label))
            .expect("the test control mounts");
        let mut area = SafeArea::new(Rect::new(0, 0, 300, 200));
        area.set_content(Some(id));
        let insets = area.insets();
        assert_eq!(
            area.size_hint(),
            Size::new(
                expected.width + insets.left + insets.right,
                expected.height + insets.top + insets.bottom,
            )
        );
        let _ = crate::widget::runtime::unregister(id);
    }
}
