// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CollapsiblePane — a container widget that can be collapsed/expanded.
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// A container widget that can be collapsed/expanded by clicking its header bar.
///
/// The header displays a title and an arrow indicator (▶ when collapsed, ▼ when expanded).
/// When collapsed, the content area is hidden; when expanded, the content area is visible
/// below the header bar. The disclosure is **animated**: `open` runs from `0.0` (collapsed)
/// to `1.0` (expanded) and the content band is a function of it, so the pane opens rather
/// than hard-cutting between two frames.
pub struct CollapsiblePane {
    base: BaseWidget,
    title: String,
    collapsed: bool,
    content_child: Option<ObjectId>,
    header_height: u32,
    /// How far the pane is open, `0.0` fully collapsed and `1.0` fully expanded.
    ///
    /// # Why this is separate from `collapsed`
    ///
    /// `collapsed` is the logical state a caller reads the instant it changes; `open` is what
    /// the draw path measures the content band with. Drawing the band straight from `collapsed`
    /// made the content appear and disappear between two adjacent frames, which is the
    /// hard-cut this field exists to remove -- the same split, for the same reason, as
    /// `Switch`'s `checked` versus `travel`. It starts at `1.0` because a freshly built pane is
    /// **expanded**, so a pane that has never been toggled draws at rest rather than fading open
    /// on its first frame.
    open: PropertyDriver,
    /// Emitted when the collapsed state changes (parameter: new collapsed state).
    pub toggled: Signal1<bool>,
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}

impl CollapsiblePane {
    /// Creates a new collapsible pane with the specified geometry and title.
    pub fn new(geometry: Rect, title: String) -> Self {
        let base = BaseWidget::new(WidgetKind::CollapsiblePane, geometry, "CollapsiblePane");
        Self {
            base,
            title,
            collapsed: false,
            content_child: None,
            header_height: dimensions::COLLAPSIBLE_HEADER_HEIGHT,
            // Expanded at rest, so an untouched pane pays no frames and shows its content.
            open: PropertyDriver::at(1.0, MotionSlot::Normal),
            toggled: Signal1::new(),
            registry: None,
        }
    }

    /// Returns the title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the title text.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.base.request_redraw();
    }

    /// Returns whether the pane is currently collapsed.
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Sets the collapsed state, emits the `toggled` signal, and requests a redraw.
    ///
    /// The drawn band follows `open`, which the frame bus advances; this only re-aims it. The
    /// logical state is answered immediately, so a caller reading `is_collapsed()` after a click
    /// sees the new value the moment the click happened — the same split as `Switch::set_checked`.
    /// No frame is requested here, because it would not be the thing that makes the motion
    /// visible: `tick_animations` reports `true` while any control is in flight, and that is what
    /// keeps the loop painting.
    pub fn set_collapsed(&mut self, collapsed: bool) {
        if self.collapsed == collapsed {
            return;
        }
        self.collapsed = collapsed;
        self.open.set_target(self.open_target());
        self.toggled.emit(collapsed);
        self.base.request_redraw();
    }

    /// How far open the logical `collapsed` flag calls for.
    ///
    /// One place states which end `collapsed` means, so the tick and the "am I moving?" query
    /// cannot disagree about it (the same reason `Switch::travel_target` exists).
    fn open_target(&self) -> f32 {
        if self.collapsed {
            0.0
        } else {
            1.0
        }
    }

    /// How far the pane is currently open, `0.0` collapsed and `1.0` expanded.
    ///
    /// This is the *drawn* fraction: the content band's height is a function of it, so a test
    /// can assert the disclosure slid rather than jumped by sampling it at successive frames.
    pub fn open_progress(&self) -> f32 {
        self.open.value()
    }

    /// Advances the open/close animation by `delta_ms`; `true` while it is still moving.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.open.set_target(self.open_target());
        self.open.tick(delta_ms)
    }

    /// Whether the pane is between two open fractions -- answers only, never advances.
    pub fn is_animating(&self) -> bool {
        self.open.is_moving()
    }

    /// Toggles the collapsed state.
    pub fn toggle(&mut self) {
        self.set_collapsed(!self.collapsed);
    }

    /// Sets the child widget to display in the content area.
    pub fn set_content(&mut self, child: ObjectId) {
        // Remove existing content child from children list if present.
        if let Some(existing) = self.content_child {
            self.base.remove_child(existing);
        }
        self.content_child = Some(child);
        self.base.add_child(child);
        self.base.request_redraw();
    }

    /// Returns the content child widget ID, if any.
    pub fn content(&self) -> Option<ObjectId> {
        self.content_child
    }

    /// Returns the height of the header bar in pixels.
    pub fn header_height(&self) -> u32 {
        self.header_height
    }

    /// Sets the height of the header bar in pixels.
    pub fn set_header_height(&mut self, height: u32) {
        self.header_height = height;
        self.base.request_redraw();
    }

    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
        self.base.request_redraw();
    }

    /// Returns the geometry of the header area.
    fn header_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x, rect.y, rect.width, self.header_height)
    }

    /// Returns the geometry of the content area, as far open as the pane currently is.
    ///
    /// The band is `header_height + fullContentHeight * open`, so the content is revealed by
    /// growing rather than by appearing. A fully open pane therefore reports exactly the same
    /// rectangle it always did (`open == 1.0`), which is what keeps the resting appearance and
    /// every existing geometry assertion unchanged.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let y_offset = rect.y + self.header_height as i32;
        let full_height = rect.height.saturating_sub(self.header_height);
        // A closed pane reserves nothing; a half-open one reserves half. Rounding toward zero
        // rather than nearest keeps `open == 0.0` at exactly zero, which is the value the
        // "collapsed reserves no band" assertion reads.
        let height = (full_height as f32 * self.open.value()) as u32;
        Rect::new(rect.x, y_offset, rect.width, height)
    }

    fn sync_content_geometry(&mut self) {
        // A band with no height has nothing to lay a child out in, and a zero-height rectangle
        // is not "a hidden child" — it is a child that gets drawn into a sliver. The child's
        // geometry is left at the last open band instead, which is also the rectangle it will
        // animate back into as the pane reopens.
        if self.content_rect().height == 0 {
            return;
        }
        if let Some(content) = self.content_child {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().set_widget_geometry(content, self.content_rect());
            }
        }
    }
}

// Implement Widget trait
impl Widget for CollapsiblePane {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(200, 100)
    }

    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
        if self.content_child == Some(child) {
            self.content_child = None;
        }
    }

    // The disclosure animation is the control's own; the trait spelling is what the bus reaches
    // through `&mut dyn Widget`, which is the only way the opening actually happens.
    fn tick(&mut self, delta_ms: u32) -> bool {
        CollapsiblePane::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.open.is_moving()
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `CollapsiblePane`'s property contract.
impl WidgetProperties for CollapsiblePane {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "collapsed" => Ok(CapabilityValue::Bool(self.is_collapsed())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "collapsed" => {
                self.set_collapsed(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `COLLAPSIBLE_PANE_PROPERTIES`.
        property_names_of!["title", "collapsed", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `collapsible_pane` publishes.
    ///
    /// `toggle` is payload-free and maps onto the widget's real `toggle`, which
    /// flips the collapsed state and emits `toggled`. `set_title` and
    /// `set_collapsed` carry the text and the state, so a bare invocation is
    /// reported as needing one rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle" => {
                self.toggle();
                Ok(())
            }
            "set_title" | "set_collapsed" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for CollapsiblePane {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                let hdr = self.header_rect();
                if hdr.contains(*pos) {
                    self.toggle();
                    return;
                }
            }
            Event::KeyPress { key, .. } if *key == 32 || *key == 13 => {
                self.toggle();
                return;
            }
            _ => {}
        }

        // Events reach the content only while there is a band to reach into. A closing pane
        // shrinks that band, so the moment it reaches nothing the content stops receiving
        // pointer events — which is the same rule the draw path applies, read from the same
        // `content_rect` rather than from the logical flag, so the two cannot disagree.
        let content_rect = self.content_rect();
        if content_rect.height == 0 {
            return;
        }

        if let Some(content) = self.content_child {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().set_widget_geometry(content, content_rect);
                match event {
                    Event::MousePress { pos, .. }
                    | Event::MouseRelease { pos, .. }
                    | Event::MouseMove { pos }
                    | Event::MouseDoubleClick { pos, .. } => {
                        if content_rect.contains(*pos) {
                            let _ = reg.borrow_mut().forward_event(content, event);
                        }
                    }
                    Event::Wheel { .. } => {
                        let fallback = Point::new(content_rect.x + 1, content_rect.y + 1);
                        if content_rect.contains(fallback) {
                            let _ = reg.borrow_mut().forward_event(content, event);
                        }
                    }
                    #[cfg(feature = "touch")]
                    Event::TouchBegin { pos, .. }
                    | Event::TouchEnd { pos, .. }
                    | Event::TouchMove { pos, .. }
                    | Event::Tap { pos }
                    | Event::DoubleTap { pos }
                    | Event::LongPress { pos }
                    | Event::Swipe { start: _, end: pos, .. }
                    | Event::Drag { pos, .. }
                    | Event::TwoFingerTap { pos }
                    | Event::TwoFingerSwipe { centroid_start: _, centroid_end: pos, .. }
                    | Event::Fling { pos, .. } => {
                        if content_rect.contains(*pos) {
                            let _ = reg.borrow_mut().forward_event(content, event);
                        }
                    }
                    _ => {
                        let _ = reg.borrow_mut().forward_event(content, event);
                    }
                }
            }
        }
    }
}

impl Draw for CollapsiblePane {
    fn draw(&mut self, context: &mut RenderContext) {
        let hdr = self.header_rect();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the header, the content area, the border and both text colours used
        // to be hardcoded literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("collapsible_pane");
        // `collapsible_pane` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. A header painted in that colour would be byte-identical to the frame
        // behind it, so a resolved surface equal to the window fill is re-derived a visible
        // step away from it, the same distinction `Colors::input_background` draws for a field.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| window_fill.blend(&text_color, 0.25));
        let header_bg = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&text_color, 0.08),
        };
        // The content area is one step more inset than the header, so the two read as
        // separate regions in either appearance.
        //
        // # Why the raised step is read rather than blended
        //
        // This is what `Colors::surface_container_high` exists for: "a raised step above
        // `surface_container`". The blend produced the same *intent* but could not express it as a
        // choice — a theme that wants its nested region flat, or more strongly raised, had no way to
        // say so, and the token was declared for exactly that purpose and read by nothing (BLUE22 ·
        // F-9). The blend survives as the fallback for a theme that predates the role.
        let content_bg = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.surface_container_high)
            .unwrap_or_else(|| header_bg.blend(&text_color, 0.03));

        // --- Draw header background ---
        // A disabled pane dims toward its own ink rather than to a fixed light grey, which
        // is what previously made the disabled state ignore the appearance entirely.
        let header_bg =
            if self.base.is_enabled() { header_bg } else { header_bg.blend(&text_color, 0.5) };
        context.fill_rect(hdr, header_bg);

        // --- Draw header bottom border ---
        context.draw_line(
            Point::from_f32(hdr.x as f32, (hdr.y + hdr.height as i32) as f32),
            Point::from_f32((hdr.x + hdr.width as i32) as f32, (hdr.y + hdr.height as i32) as f32),
            border_color,
        );

        // --- Draw expand/collapse arrow (▶ collapsed, ▼ expanded) ---
        // The arrow and the title share one line box centred in the header, so the two read
        // as a single row. Centring through the shared primitive replaces the previous
        // `hdr.y + hdr.height / 2 - k`: the glyph origin is the box's top-left edge, so the
        // hand-tuned constant was compensating for a defect rather than expressing the row.
        let header_font = Font::default();
        let header_line = context.text_line(hdr, &header_font);
        let arrow_x = hdr.x + 6;
        let arrow_y = header_line.y;
        let arrow_color =
            if self.base.is_enabled() { text_color } else { text_color.blend(&header_bg, 0.5) };
        let arrow_char = if self.collapsed { "▶" } else { "▼" };
        context.draw_text(
            Point::from_f32(arrow_x as f32, arrow_y as f32),
            arrow_char,
            &header_font,
            arrow_color,
            HorizontalAlignment::Left,
        );

        // --- Draw title text ---
        if !self.title.is_empty() {
            let text_x = hdr.x + 20;
            let text_y = header_line.y;
            let title_color =
                if self.base.is_enabled() { text_color } else { text_color.blend(&header_bg, 0.5) };
            context.draw_text(
                Point::from_f32(text_x as f32, text_y as f32),
                &self.title,
                &header_font,
                title_color,
                HorizontalAlignment::Left,
            );
        }

        // --- Draw content area (only while the pane is at all open) ---
        //
        // Guarded on the *band*, not on `collapsed`: the band shrinks to nothing as the pane
        // closes, so a frame mid-close draws a shorter content area rather than snapping it away
        // on the first frame of the animation. A band at zero draws nothing, which is what a
        // fully collapsed pane is.
        let content_rect = self.content_rect();
        if content_rect.height > 0 {
            self.sync_content_geometry();
            context.fill_rect(content_rect, content_bg);
            context.draw_line(
                Point::from_f32(content_rect.x as f32, content_rect.y as f32),
                Point::from_f32(
                    content_rect.x as f32,
                    (content_rect.y + content_rect.height as i32) as f32,
                ),
                border_color,
            );
            context.draw_line(
                Point::from_f32(
                    (content_rect.x + content_rect.width as i32) as f32,
                    content_rect.y as f32,
                ),
                Point::from_f32(
                    (content_rect.x + content_rect.width as i32) as f32,
                    (content_rect.y + content_rect.height as i32) as f32,
                ),
                border_color,
            );
            context.draw_line(
                Point::from_f32(
                    content_rect.x as f32,
                    (content_rect.y + content_rect.height as i32) as f32,
                ),
                Point::from_f32(
                    (content_rect.x + content_rect.width as i32) as f32,
                    (content_rect.y + content_rect.height as i32) as f32,
                ),
                border_color,
            );

            if let Some(child_id) = self.content_child {
                if let Some(ref reg) = self.registry {
                    context.push_clip(
                        content_rect.x,
                        content_rect.y,
                        content_rect.width,
                        content_rect.height,
                    );
                    reg.borrow_mut().draw_widget(child_id, context);
                    context.pop_clip();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect, Size};
    use crate::render::{PaintBackend, RenderContext, SvgPaintBackend};
    use std::sync::Arc;

    /// Helper to construct a default collapsible pane for tests.
    fn make_pane() -> CollapsiblePane {
        CollapsiblePane::new(Rect::new(0, 0, 200, 100), "Test".to_string())
    }

    // ── 1. Creation defaults ────────────────────────────────────────────

    #[test]
    fn collapsible_pane_creation_defaults() {
        let cp = CollapsiblePane::new(Rect::new(0, 0, 200, 100), "Title".to_string());
        assert_eq!(cp.title(), "Title");
        assert!(!cp.is_collapsed(), "pane should start expanded");
        assert_eq!(
            cp.header_height(),
            dimensions::COLLAPSIBLE_HEADER_HEIGHT,
            "the header is the touch target, so it takes Material's 44 floor"
        );
        assert!(cp.content().is_none(), "no content child by default");
        // A freshly built pane is expanded *and* drawn expanded: the driver starts at its
        // resting end, so an untouched pane owes no frames and its content is visible.
        assert_eq!(cp.open_progress(), 1.0, "a new pane is open, not mid-reveal");
        assert!(!cp.is_animating(), "and therefore owes no frames");
    }

    // ── 2. Title get / set ──────────────────────────────────────────────

    #[test]
    fn collapsible_pane_set_title() {
        let mut cp = make_pane();
        assert_eq!(cp.title(), "Test");
        cp.set_title("Updated Title".to_string());
        assert_eq!(cp.title(), "Updated Title");
        cp.set_title(String::new());
        assert_eq!(cp.title(), "");
    }

    // ── 3. Toggle collapsed ─────────────────────────────────────────────

    #[test]
    fn collapsible_pane_toggle_collapsed() {
        let mut cp = make_pane();
        assert!(!cp.is_collapsed());

        cp.set_collapsed(true);
        assert!(cp.is_collapsed());

        cp.set_collapsed(false);
        assert!(!cp.is_collapsed());

        // No-op calls should not change state.
        cp.set_collapsed(false);
        assert!(!cp.is_collapsed());
        cp.set_collapsed(true);
        assert!(cp.is_collapsed());
        cp.set_collapsed(true);
        assert!(cp.is_collapsed());

        // toggle()
        cp.toggle();
        assert!(!cp.is_collapsed());
        cp.toggle();
        assert!(cp.is_collapsed());
    }

    // ── 4. Signal emission ──────────────────────────────────────────────

    #[test]
    fn collapsible_pane_toggle_emits_signal() {
        let mut cp = make_pane();
        let emitted = Arc::new(std::sync::Mutex::new(Vec::new()));
        let e = Arc::clone(&emitted);
        cp.toggled.connect(move |v| {
            e.lock().unwrap().push(*v);
        });

        cp.set_collapsed(true);
        assert_eq!(emitted.lock().unwrap().as_slice(), &[true], "should emit true when collapsing");

        cp.set_collapsed(false);
        assert_eq!(
            emitted.lock().unwrap().as_slice(),
            &[true, false],
            "should emit false when expanding"
        );

        // No-op: set_collapsed again with same value should NOT emit.
        cp.set_collapsed(false);
        assert_eq!(
            emitted.lock().unwrap().as_slice(),
            &[true, false],
            "no-op must not emit signal"
        );

        // toggle() should also emit.
        cp.toggle();
        assert_eq!(emitted.lock().unwrap().as_slice(), &[true, false, true], "toggle should emit");
    }

    // ── 5. Header height ────────────────────────────────────────────────

    #[test]
    fn collapsible_pane_header_height() {
        let mut cp = make_pane();
        assert_eq!(cp.header_height(), dimensions::COLLAPSIBLE_HEADER_HEIGHT);

        cp.set_header_height(32);
        assert_eq!(cp.header_height(), 32);

        cp.set_header_height(0);
        assert_eq!(cp.header_height(), 0);
    }

    // ── 6. Content child ────────────────────────────────────────────────

    #[test]
    fn collapsible_pane_content_child() {
        let mut cp = make_pane();
        assert!(cp.content().is_none());

        cp.set_content(42);
        assert_eq!(cp.content(), Some(42));
        // The child should also appear in the base children list.
        assert!(cp.base().children().contains(&42), "content child must be in base children");

        // Replacing content child should remove the old one.
        cp.set_content(99);
        assert_eq!(cp.content(), Some(99));
        assert!(!cp.base().children().contains(&42), "old child must be removed");
        assert!(cp.base().children().contains(&99), "new child must be in base children");

        // Removing via remove_child should also clear content_child.
        cp.remove_child(99);
        assert!(cp.content().is_none(), "content should be cleared after remove_child");
        assert!(!cp.base().children().contains(&99));
    }

    // ── 7. Geometry delegation ──────────────────────────────────────────

    #[test]
    fn collapsible_pane_geometry_delegation() {
        let mut cp = CollapsiblePane::new(Rect::new(10, 20, 300, 150), "Geo".to_string());

        // Base geometry matches what we passed.
        assert_eq!(cp.geometry(), Rect::new(10, 20, 300, 150));

        // Modify geometry through base_mut().
        cp.base_mut().set_geometry(Rect::new(0, 0, 400, 200));
        assert_eq!(cp.geometry(), Rect::new(0, 0, 400, 200));

        // Verify Widget trait delegation returns the same geometry.
        assert_eq!(cp.base().kind(), WidgetKind::CollapsiblePane);
    }

    // ── 8. Visibility ───────────────────────────────────────────────────

    #[test]
    fn collapsible_pane_visibility() {
        let mut cp = make_pane();
        assert!(cp.base().is_visible(), "should be visible by default");

        cp.base_mut().hide();
        assert!(!cp.base().is_visible());

        cp.base_mut().show();
        assert!(cp.base().is_visible());
    }

    // ── 9. ObjectId and WidgetKind ──────────────────────────────────────

    #[test]
    fn collapsible_pane_id_kind() {
        let cp1 = make_pane();
        let cp2 = make_pane();

        // Each instance gets a unique ObjectId.
        assert_ne!(cp1.base().id(), cp2.base().id(), "each pane must have a unique ObjectId");

        // WidgetKind must be CollapsiblePane.
        assert_eq!(cp1.base().kind(), WidgetKind::CollapsiblePane);
        assert_eq!(cp2.base().kind(), WidgetKind::CollapsiblePane);

        // ObjectId is a u64 type.
        let id: ObjectId = cp1.base().id();
        // It should be non-zero (first allocation starts at 1).
        assert!(id > 0, "ObjectId should be positive");
    }

    // ── 10. SVG draw output ─────────────────────────────────────────────

    /// The pane paints a header region and, when expanded, a distinct content region.
    ///
    /// The assertions are about that *structure*, not about literal RGB triples: the two
    /// fills come from the active theme now, so a colour literal here would pin the light
    /// preset and fail the moment the appearance changed — which is exactly the
    /// theme-blindness the resolved colours exist to remove.
    #[test]
    fn collapsible_pane_draw_produces_svg() {
        // The expectation below is derived from the active theme, so the test has to pin the
        // appearance and hold the registry guard: otherwise a concurrently-running test can switch
        // the theme between the derivation and the draw, and the two halves of this one assertion
        // would be comparing different appearances.
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
        let mut cp = make_pane();

        // The colour each region resolves to under the active theme, computed from the same
        // inputs the draw uses rather than read back out of the SVG.
        //
        // # Why the derivation reads `surface_container` rather than `background`
        //
        // The pane is a container, so it classifies as `WidgetRole::Surface`, and that role now
        // resolves to the theme's `surface_container` token instead of to the window's own
        // `background`. The draw reads it the same way — through the resolved theme style — so the
        // expectation has to follow the same route or the test is asserting the old role's fill.
        // The fallback mirrors the draw's own: a theme with no container role falls back to a step
        // away from the window fill, which is one of that role's defaults.
        let theme = crate::style::resolved_theme_style("collapsible_pane");
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        let text_color = theme.as_ref().and_then(|t| t.text_color).unwrap_or(Color::rgb(0, 0, 0));
        let header_expected = theme
            .as_ref()
            .and_then(|t| t.background_color)
            .filter(|resolved| *resolved != window_fill)
            .unwrap_or_else(|| window_fill.blend(&text_color, 0.08));
        let content_expected = header_expected.blend(&text_color, 0.03);
        let rgb = |c: Color| format!("{},{},{}", c.r, c.g, c.b);

        let mut svg_backend = SvgPaintBackend::new(Size::new(200, 100));
        svg_backend.begin_frame(Color::rgb(255, 255, 255));
        {
            let mut ctx = RenderContext::new(&mut svg_backend);
            cp.draw(&mut ctx);
        }
        svg_backend.end_frame();
        let svg_output = svg_backend.finish();

        // SVG must be well-formed.
        assert!(svg_output.starts_with("<svg"), "output must start with <svg");
        assert!(svg_output.ends_with("</svg>"), "output must end with </svg>");

        // Expanded state should draw the content area.
        assert!(
            svg_output.contains(&rgb(content_expected)),
            "should contain content area background"
        );
        // Header background should be present.
        assert!(svg_output.contains(&rgb(header_expected)), "should contain header background");

        // Now collapsed.
        let mut cp2 = CollapsiblePane::new(Rect::new(0, 0, 200, 100), "Collapsed".to_string());
        cp2.set_collapsed(true);
        // The band is a function of `open`, and `set_collapsed` only *re-aims* it: a pane that has
        // just been told to collapse is still drawn open until the frame bus advances it. That is
        // the point of the animation rather than a gap in it, so the test drives the slide the way
        // the bus would before reading the settled frame — the same thing `Switch`'s travel tests
        // do. What a real user sees is the band shrinking over these frames, not the two endpoints.
        while cp2.tick(1000) {}

        let mut svg_backend2 = SvgPaintBackend::new(Size::new(200, 100));
        svg_backend2.begin_frame(Color::rgb(255, 255, 255));
        {
            let mut ctx = RenderContext::new(&mut svg_backend2);
            cp2.draw(&mut ctx);
        }
        svg_backend2.end_frame();
        let svg_collapsed = svg_backend2.finish();

        // Collapsed state should NOT contain content area background.
        assert!(
            !svg_collapsed.contains(&rgb(content_expected)),
            "a settled collapsed pane must not draw the content area"
        );
        // But the header should still be visible.
        assert!(
            svg_collapsed.contains(&rgb(header_expected)),
            "collapsed pane must still draw header"
        );
    }

    // ── 11. Mouse click toggles ─────────────────────────────────────────

    #[test]
    fn collapsible_pane_mouse_click_toggles() {
        let mut cp = make_pane();
        assert!(!cp.is_collapsed());

        // MousePress on the header area (button 1 = left click).
        // Header rect is at (0,0,200,44) — y=12 is well inside.
        let click_event = Event::MousePress { pos: Point { x: 10, y: 12 }, button: 1 };
        cp.handle_event(&click_event);
        assert!(cp.is_collapsed(), "click on header should collapse the pane");

        // Click again to expand.
        cp.handle_event(&click_event);
        assert!(!cp.is_collapsed(), "second click on header should expand the pane");

        // Click outside the header area (below the header) should NOT toggle.
        cp.handle_event(&click_event); // collapse again
        assert!(cp.is_collapsed());
        let outside_event = Event::MousePress {
            pos: Point { x: 10, y: 80 }, // content area, well below header_height
            button: 1,
        };
        cp.handle_event(&outside_event);
        assert!(cp.is_collapsed(), "click outside header must not toggle");

        // Click with non-left button should NOT toggle.
        let right_click = Event::MousePress { pos: Point { x: 10, y: 12 }, button: 2 };
        // State is currently collapsed (true).
        cp.handle_event(&right_click);
        assert!(cp.is_collapsed(), "right-click on header must not toggle");
    }

    // ── 14. Keyboard toggle (Space/Enter) ───────────────────────────────

    #[test]
    fn collapsible_pane_keyboard_toggle() {
        let mut cp = make_pane();
        assert!(!cp.is_collapsed());

        // Space key toggles
        cp.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        assert!(cp.is_collapsed(), "Space should collapse the pane");

        // Enter key toggles
        cp.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(!cp.is_collapsed(), "Enter should expand the pane");

        // Other keys should not toggle
        cp.handle_event(&Event::KeyPress { key: 65, modifiers: 0 }); // 'A'
        assert!(!cp.is_collapsed(), "'A' key must not toggle");
    }

    // ── 15. The disclosure is an animation, not a hard cut (§0.3 three-frame judgement) ──

    /// Collapsing and expanding runs the content band between two heights over frames.
    ///
    /// # The defect this pins
    ///
    /// The pane used to draw its content band straight from `collapsed`, so the band was either
    /// the full height or absent, on adjacent frames, with nothing in between: the content
    /// popped out of existence rather than sliding shut. The assertion is the same shape every
    /// animation in this crate uses (BLUE23 §0.3): sample the drawn geometry at three times and
    /// require the middle one to lie **strictly between** the two ends, and require the whole
    /// sequence to be **monotonic**. A hard cut fails both halves — the middle sample equals one
    /// end, and the sequence never takes an interior value.
    #[test]
    fn the_disclosure_slides_rather_than_cutting() {
        let mut cp = make_pane();
        assert!(!cp.tick(0), "an open pane at rest owes no frame");
        let open_height = cp.content_rect().height;
        assert!(open_height > 0, "an expanded pane reserves a content band");

        cp.set_collapsed(true);
        assert!(cp.is_animating(), "collapsing owes frames, which is what the bus reads");

        // Frame 0 is still the open band: the slide has not happened yet, which is what makes
        // the first frame of the animation continuous with the last frame before it.
        assert_eq!(cp.content_rect().height, open_height, "the slide starts where the pane was");

        // Two half-step frames, then settle. `MotionSlot::Normal` is well under 2 ticks of
        // 60 ms, so the midpoint is a genuine interior sample rather than a snapped endpoint.
        assert!(cp.tick(60), "still moving after the first step");
        let mid = cp.content_rect().height;
        assert!(
            mid < open_height && mid > 0,
            "the band must take an interior height, not jump to either end (got {mid})"
        );
        while cp.tick(60) {}
        assert_eq!(cp.content_rect().height, 0, "and settle at exactly zero");
        assert!(!cp.is_animating(), "a settled pane owes no more frames");

        // Reopening takes the same interior values in the other direction.
        cp.set_collapsed(false);
        assert!(cp.is_animating());
        assert!(cp.tick(60));
        let mid_reopen = cp.content_rect().height;
        assert!(
            mid_reopen > 0 && mid_reopen < open_height,
            "reopening must also pass through interior heights (got {mid_reopen})"
        );
        while cp.tick(60) {}
        assert_eq!(cp.content_rect().height, open_height, "and return to the full band");
    }
}
