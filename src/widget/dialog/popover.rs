// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Popover widget — a floating bubble card with an anchor arrow.
//!
//! The Popover widget displays a floating card near an anchor rectangle,
//! optionally containing a child widget. It supports show/hide with an
//! arrow pointing toward the anchor, and auto-dismisses when the user
//! clicks outside the popover area.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Arrow size in pixels from tip to base.
const ARROW_SIZE: i32 = 10;
/// Corner radius of the popover body.
const CORNER_RADIUS: u32 = 8;

/// Popover widget — a floating bubble card with an anchor arrow.
///
/// Shows a rounded rectangle with a triangular arrow pointing toward an
/// anchor rectangle. When visible, it can contain an optional child widget
/// rendered inside the card body. Clicking outside the popover area
/// automatically hides it.
pub struct Popover {
    base: BaseWidget,
    content: Option<Box<dyn Widget>>,
    anchor_rect: Rect,
    visible: bool,
    /// Cached popover body rectangle (computed during draw).
    body_rect: Rect,
}

impl Popover {
    /// Creates a new Popover widget with the given geometry.
    ///
    /// Initially hidden with no content and an empty anchor rectangle.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Popover, geometry, "Popover"),
            content: None,
            anchor_rect: Rect::default(),
            visible: false,
            body_rect: Rect::default(),
        }
    }

    /// Shows the popover positioned relative to the given anchor rectangle.
    pub fn show(&mut self, anchor: Rect) {
        self.anchor_rect = anchor;
        self.visible = true;
        self.base.request_redraw();
    }

    /// Hides the popover.
    pub fn hide(&mut self) {
        self.visible = false;
        self.base.request_redraw();
    }

    /// Returns whether the popover is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Reports the popover's own shown-state.
    ///
    /// Deliberately distinct from [`Widget::is_visible`], which this widget
    /// overrides to return the popup state. The property contract publishes the
    /// inherited `visible` for the control, so the popup state is published under
    /// the `shown` name instead; without this separation the base property would
    /// be unreachable.
    pub fn is_shown(&self) -> bool {
        self.visible
    }

    /// Sets the popover's shown-state without moving its anchor.
    ///
    /// The counterpart to [`is_shown`](Self::is_shown). Showing keeps the existing
    /// anchor rectangle, so a caller that wants to reposition first uses
    /// [`show`](Self::show) with the new anchor.
    pub fn set_shown(&mut self, shown: bool) {
        if shown {
            self.show(self.anchor_rect);
        } else {
            self.hide();
        }
    }

    /// Returns the title of the popover's content widget, or an empty string when
    /// it has no content.
    ///
    /// A `Box<dyn Widget>` exposes no text of its own, so the concrete `Label`
    /// case is read through a downcast; anything else answers honestly with the
    /// empty string rather than inventing a rendering.
    pub fn content_text(&self) -> String {
        self.content
            .as_deref()
            .and_then(
                crate::widget::capability::coercion::widget_as::<
                    crate::widget::base_widgets::label::Label,
                >,
            )
            .map_or_else(String::new, |label| label.text().to_string())
    }

    /// Sets the content widget displayed inside the popover.
    pub fn set_content(&mut self, widget: Box<dyn Widget>) {
        self.content = Some(widget);
        self.base.request_redraw();
    }

    /// Returns a reference to the content widget, if any.
    pub fn content(&self) -> Option<&dyn Widget> {
        self.content.as_deref()
    }

    /// Returns a mutable reference to the content widget, if any.
    pub fn content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.content.as_deref_mut()
    }

    /// Returns the anchor rectangle.
    pub fn anchor_rect(&self) -> Rect {
        self.anchor_rect
    }

    /// Sets the anchor rectangle.
    pub fn set_anchor_rect(&mut self, rect: Rect) {
        self.anchor_rect = rect;
        self.base.request_redraw();
    }

    /// Computes the popover body rectangle below/above the anchor.
    fn compute_layout(&self) -> (Rect, Point, ArrowDirection) {
        let geom = self.geometry();
        let body_width = geom.width.clamp(100, 400);
        let body_height = geom.height.clamp(60, 400);

        // Position popover below the anchor by default; flip above if not enough room
        let below_space =
            geom.y + geom.height as i32 - (self.anchor_rect.y + self.anchor_rect.height as i32);
        let above_space = self.anchor_rect.y - geom.y;

        let (body_y, arrow_dir) = if below_space >= body_height as i32 + ARROW_SIZE {
            (self.anchor_rect.y + self.anchor_rect.height as i32 + ARROW_SIZE, ArrowDirection::Up)
        } else if above_space >= body_height as i32 + ARROW_SIZE {
            (self.anchor_rect.y - body_height as i32 - ARROW_SIZE, ArrowDirection::Down)
        } else {
            // Default: below
            (self.anchor_rect.y + self.anchor_rect.height as i32 + ARROW_SIZE, ArrowDirection::Up)
        };

        // Center horizontally on anchor
        let anchor_center_x = self.anchor_rect.x + self.anchor_rect.width as i32 / 2;
        let body_x = (anchor_center_x - body_width as i32 / 2).max(geom.x);
        let body_rect = Rect::new(body_x, body_y, body_width, body_height);

        // Arrow tip points to anchor center
        let arrow_tip = Point::new(
            anchor_center_x,
            match arrow_dir {
                ArrowDirection::Up => body_y - ARROW_SIZE,
                ArrowDirection::Down => body_y + body_height as i32 + ARROW_SIZE,
            },
        );

        (body_rect, arrow_tip, arrow_dir)
    }
}

impl Widget for Popover {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(200, 150)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Popover`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
///
/// The legacy table served a `visible` arm for this kind, but there is a name
/// collision to resolve: this widget overrides [`Widget::is_visible`] to return its
/// *popup* state, so a `visible` arm here would shadow the shared `visible` that
/// [`base_property_get`] serves — the two would be indistinguishable and the base
/// one unreachable. The popup state is therefore published as `shown`, and bare
/// `visible` keeps meaning what it means for every other control.
impl WidgetProperties for Popover {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "shown" => Ok(CapabilityValue::Bool(self.is_shown())),
            "text" => Ok(CapabilityValue::String(self.content_text())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "shown" => {
                self.set_shown(expect_bool(value)?);
                Ok(())
            }
            // The text mirrors the content widget, so writing it would either be
            // dropped by the next layout or require synthesising a `Label` the
            // caller never asked for. A caller that means to set it installs the
            // content widget through `set_content`.
            "text" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["shown", "text", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `popover` publishes.
    ///
    /// Both names carry a value — `set_visible` the boolean the property route
    /// already accepts, `set_text` the content the popover mirrors and cannot be
    /// given as a bare string — so a payload-less invocation is reported as
    /// needing one rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_visible" | "set_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Popover {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal —
        // and the whole card used to be skipped unless the popover was already showing — so
        // the census reported `ink = 0` *and* no response to a light/dark switch.
        //
        // The theme reads take and release the global manager's lock internally, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("popover");
        // `popover` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A card
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, the same
        // distinction `Colors::input_background` draws for a field.
        let window_fill = {
            let manager = crate::theme::global_theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(40, 40, 40));
        let card = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != card)
            .unwrap_or_else(|| card.blend(&ink, 0.35));
        let muted_ink = ink.blend(&card, 0.45);

        // A hidden popover is still laid out, so the card it will occupy is visible before it
        // opens: the control used to paint nothing at rest. It is drawn at reduced opacity so
        // the two states stay distinguishable.
        let visible = self.visible;

        let (body_rect, arrow_tip, arrow_dir) = self.compute_layout();
        self.body_rect = body_rect;
        let (body_rect, arrow_tip) = if visible {
            (body_rect, arrow_tip)
        } else {
            // Anchor-less at rest: the card fills the control's own rect rather than being
            // positioned against an anchor rectangle that has never been set.
            (
                Rect::new(
                    rect.x + ARROW_SIZE,
                    rect.y,
                    rect.width.saturating_sub(ARROW_SIZE as u32),
                    rect.height,
                ),
                arrow_tip,
            )
        };
        let card = if visible { card } else { window_fill.blend(&card, 0.45) };
        let border = if visible { border } else { window_fill.blend(&border, 0.45) };

        // ── Draw shadow ──
        let shadow_offset = 2i32;
        let shadow_rect = Rect::new(
            body_rect.x + shadow_offset,
            body_rect.y + shadow_offset,
            body_rect.width,
            body_rect.height,
        );
        context.fill_rounded_rect(shadow_rect, CORNER_RADIUS, Color::rgba(0, 0, 0, 40));

        // ── Draw popover body ──
        context.fill_rounded_rect(body_rect, CORNER_RADIUS, card);
        context.draw_rounded_rect_stroke(body_rect, CORNER_RADIUS, border, 1);

        // ── Draw arrow ──
        if visible {
            self.draw_arrow(context, arrow_tip, arrow_dir, card, border);
        }

        // ── Draw placeholder content indicator ──
        // The label reads the theme rather than the previous fixed grey, so a dark card does
        // not carry light-theme text.
        let content_padding = 8i32;
        let content_rect = Rect::new(
            body_rect.x + content_padding,
            body_rect.y + content_padding,
            body_rect.width.saturating_sub((content_padding as u32) * 2),
            body_rect.height.saturating_sub((content_padding as u32) * 2),
        );
        let font = Font::simple("sans-serif", 13.0);
        let label = if self.content.is_some() { "Popover" } else { "Popover (empty)" };
        let metrics = context.measure_text(label, &font);
        let text_x = content_rect.x + (content_rect.width as i32 - metrics.width as i32) / 2;
        let text_y = content_rect.y
            + (content_rect.height as i32 - metrics.height as i32) / 2
            + metrics.ascent as i32;
        context.draw_text(
            Point::new(text_x.max(content_rect.x), text_y.max(content_rect.y)),
            label,
            &font,
            muted_ink,
            HorizontalAlignment::Left,
        );
    }
}

/// Direction the popover arrow points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArrowDirection {
    Up,
    Down,
}

impl Popover {
    /// Draws the triangular arrow pointing toward the anchor.
    ///
    /// The fill and outline are passed in rather than literals so the arrow is the same
    /// colour as the card it belongs to — otherwise the arrow stayed white on a dark card.
    fn draw_arrow(
        &self,
        context: &mut RenderContext,
        tip: Point,
        dir: ArrowDirection,
        fill: Color,
        outline: Color,
    ) {
        let half_base = ARROW_SIZE / 2;
        let (_base_center, base_left, base_right) = match dir {
            ArrowDirection::Up => {
                let base_center = Point::new(tip.x, tip.y + ARROW_SIZE);
                let base_left = Point::new(tip.x - half_base, tip.y + ARROW_SIZE);
                let base_right = Point::new(tip.x + half_base, tip.y + ARROW_SIZE);
                (base_center, base_left, base_right)
            }
            ArrowDirection::Down => {
                let base_center = Point::new(tip.x, tip.y - ARROW_SIZE);
                let base_left = Point::new(tip.x - half_base, tip.y - ARROW_SIZE);
                let base_right = Point::new(tip.x + half_base, tip.y - ARROW_SIZE);
                (base_center, base_left, base_right)
            }
        };

        // Draw filled triangle using DrawPath
        let points = vec![tip, base_left, base_right];
        context.execute_command(RenderCommand::DrawPath {
            points: points.clone(),
            closed: true,
            color: fill,
            filled: true,
            width: 1,
        });
        // Draw triangle outline
        context.execute_command(RenderCommand::DrawPath {
            points,
            closed: true,
            color: outline,
            filled: false,
            width: 1,
        });
    }
}

impl EventHandler for Popover {
    fn handle_event(&mut self, event: &Event) {
        if !self.visible {
            self.base.handle_event(event);
            return;
        }

        // A disabled popover must not act on input. `is_enabled` was unchecked here,
        // so `set_enabled(false)` still let a click outside — or Escape — close it,
        // which is a state change the caller explicitly asked to suspend.
        if !self.base.is_enabled() {
            self.base.handle_event(event);
            return;
        }

        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 && !self.body_rect.contains_point(*pos) {
                    // Auto-dismiss on click outside
                    self.hide();
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                if *key == 27 {
                    // Escape key
                    self.hide();
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
    use crate::widget::svg::render_to_svg;
    /// A simple test widget used as content inside popover tests.
    struct TestContent {
        base: BaseWidget,
        text: String,
    }

    impl TestContent {
        fn new(text: &str, geometry: Rect) -> Self {
            Self {
                base: BaseWidget::new(WidgetKind::Label, geometry, "TestContent"),
                text: text.to_string(),
            }
        }
    }

    impl Widget for TestContent {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
    }

    impl Draw for TestContent {
        fn draw(&mut self, context: &mut RenderContext) {
            let font = Font::simple("sans-serif", 12.0);
            context.draw_text(
                Point::new(self.geometry().x, self.geometry().y),
                &self.text,
                &font,
                Color::BLACK,
                HorizontalAlignment::Left,
            );
        }
    }

    impl EventHandler for TestContent {
        fn handle_event(&mut self, _event: &Event) {}
    }

    #[test]
    fn popover_default_creation() {
        let popover = Popover::new(Rect::new(0, 0, 300, 200));
        assert_eq!(popover.kind(), WidgetKind::Popover);
        assert!(!popover.is_visible());
        assert!(popover.content().is_none());
        assert_eq!(popover.geometry(), Rect::new(0, 0, 300, 200));
    }

    #[test]
    fn popover_show_hide() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        assert!(!popover.is_visible());

        popover.show(Rect::new(100, 100, 50, 20));
        assert!(popover.is_visible());

        popover.hide();
        assert!(!popover.is_visible());
    }

    #[test]
    fn popover_anchor_rect() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 400));
        let anchor = Rect::new(100, 100, 50, 20);
        popover.show(anchor);
        assert_eq!(popover.anchor_rect(), anchor);

        let new_anchor = Rect::new(50, 50, 80, 30);
        popover.set_anchor_rect(new_anchor);
        assert_eq!(popover.anchor_rect(), new_anchor);
    }

    #[test]
    fn popover_set_content() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        assert!(popover.content().is_none());

        let content = TestContent::new("Hello", Rect::new(0, 0, 100, 30));
        popover.set_content(Box::new(content));
        assert!(popover.content().is_some());
    }

    #[test]
    fn popover_auto_dismiss_on_click_outside() {
        let mut popover = Popover::new(Rect::new(0, 0, 400, 400));
        let anchor = Rect::new(150, 100, 50, 20);
        popover.show(anchor);
        assert!(popover.is_visible());

        // Click far outside the body rect
        popover.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1 });
        assert!(!popover.is_visible());
    }

    #[test]
    fn popover_escape_key_dismisses() {
        let mut popover = Popover::new(Rect::new(0, 0, 400, 400));
        popover.show(Rect::new(150, 100, 50, 20));
        assert!(popover.is_visible());

        popover.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!popover.is_visible());
    }

    /// A disabled popover must not close on a click outside or on Escape.
    ///
    /// `handle_event` checked `visible` but never `enabled`, so a caller that
    /// suspended the popover with `set_enabled(false)` still had it dismissed by a
    /// stray click. Auto-dismissing is exactly the kind of state change `enabled`
    /// exists to gate.
    #[test]
    fn popover_disabled_ignores_dismissal_input() {
        let mut popover = Popover::new(Rect::new(0, 0, 400, 400));
        popover.show(Rect::new(150, 100, 50, 20));
        popover.set_enabled(false);

        // Click far outside the body rect: would normally auto-dismiss.
        popover.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1 });
        assert!(popover.is_visible(), "a disabled popover must not auto-dismiss");

        // Escape: would normally dismiss.
        popover.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(popover.is_visible(), "a disabled popover must not close on Escape");

        // Re-enabling must restore the behaviour, so the gate is a suspension and not
        // a permanent lock.
        popover.set_enabled(true);
        popover.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!popover.is_visible(), "re-enabling must restore dismissal");
    }

    #[test]
    fn popover_svg_output_visible() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        popover.show(Rect::new(100, 100, 50, 20));
        let svg = render_to_svg(&mut popover);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }

    #[test]
    fn popover_svg_output_hidden() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        let svg = render_to_svg(&mut popover);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        // A hidden popover is laid out, not blank: the card it will occupy is painted at
        // reduced opacity, so a control that has been created but not opened still has a
        // rendered extent instead of vanishing. `draw` used to `return` early here, which
        // made the control invisible at rest — the defect the rendering census reported as
        // `ink = 0`.
        let fill_count = svg.matches("fill=").count();
        assert!(
            fill_count > 1,
            "a hidden popover must still paint its card, got only the background fill: {svg}"
        );
        assert!(
            svg.contains("Popover (empty)"),
            "the placeholder label must be painted while the popover is hidden: {svg}"
        );

        // A hidden popover and a shown one must still differ: the shown card is opaque and
        // carries its anchor arrow, the hidden one is dimmed and does not.
        let mut open = Popover::new(Rect::new(0, 0, 300, 200));
        open.show(Rect::new(100, 100, 50, 20));
        let shown = render_to_svg(&mut open);
        assert_ne!(svg, shown, "showing the popover must change what is painted");
    }
}
