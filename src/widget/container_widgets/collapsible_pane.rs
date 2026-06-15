//! CollapsiblePane — a container widget that can be collapsed/expanded.
use crate::core::{HorizontalAlignment, Color, Font, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use std::cell::RefCell;
use std::rc::Rc;

/// A container widget that can be collapsed/expanded by clicking its header bar.
///
/// The header displays a title and an arrow indicator (▶ when collapsed, ▼ when expanded).
/// When collapsed, the content area is hidden; when expanded, the content area is visible
/// below the header bar.
pub struct CollapsiblePane {
    base: BaseWidget,
    title: String,
    collapsed: bool,
    content_child: Option<ObjectId>,
    header_height: u32,
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
            header_height: 24,
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
    }

    /// Returns whether the pane is currently collapsed.
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Sets the collapsed state, emits the `toggled` signal, and requests a redraw.
    pub fn set_collapsed(&mut self, collapsed: bool) {
        if self.collapsed == collapsed {
            return;
        }
        self.collapsed = collapsed;
        self.toggled.emit(collapsed);
        self.base.request_redraw();
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
    }

    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }

    /// Returns the geometry of the header area.
    fn header_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x, rect.y, rect.width, self.header_height)
    }

    /// Returns the geometry of the content area (below the header).
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let y_offset = rect.y + self.header_height as i32;
        let height = rect.height.saturating_sub(self.header_height);
        Rect::new(rect.x, y_offset, rect.width, height)
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

    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
        if self.content_child == Some(child) {
            self.content_child = None;
        }
    }
}

impl EventHandler for CollapsiblePane {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button } = event {
            if *button == 1 {
                // Check if click is within the header area.
                let hdr = self.header_rect();
                if hdr.contains(*pos) {
                    self.toggle();
                }
            }
        }
        // Forward events to content child
        if self.base.is_enabled() {
            if let Some(content) = self.content_child {
                if let Some(ref reg) = self.registry {
                    let _ = reg.borrow_mut().forward_event(content, event);
                }
            }
        }
    }
}

impl Draw for CollapsiblePane {
    fn draw(&mut self, context: &mut RenderContext) {
        let hdr = self.header_rect();

        // --- Draw header background ---
        let header_bg = if self.base.is_enabled() {
            Color::rgb(220, 220, 220)
        } else {
            Color::rgb(240, 240, 240)
        };
        context.fill_rect(hdr, header_bg);

        // --- Draw header bottom border ---
        let border_color = Color::rgb(180, 180, 180);
        context.draw_line(
            Point::from_f32(hdr.x as f32, (hdr.y + hdr.height as i32) as f32),
            Point::from_f32((hdr.x + hdr.width as i32) as f32, (hdr.y + hdr.height as i32) as f32),
            border_color,
        );

        // --- Draw expand/collapse arrow (▶ collapsed, ▼ expanded) ---
        let arrow_x = hdr.x + 6;
        let arrow_y = hdr.y + (hdr.height as i32 / 2) - 4;
        let arrow_color = if self.base.is_enabled() {
            Color::rgb(80, 80, 80)
        } else {
            Color::rgb(180, 180, 180)
        };
        let arrow_char = if self.collapsed { "▶" } else { "▼" };
        context.draw_text(
            Point::from_f32(arrow_x as f32, arrow_y as f32),
            arrow_char,
            &Font::default(),
            arrow_color,
            HorizontalAlignment::Left,
        );

        // --- Draw title text ---
        if !self.title.is_empty() {
            let text_x = hdr.x + 20;
            let text_y = hdr.y + (hdr.height as i32 / 2) - 6;
            let text_color = if self.base.is_enabled() {
                Color::rgb(0, 0, 0)
            } else {
                Color::rgb(150, 150, 150)
            };
            context.draw_text(
                Point::from_f32(text_x as f32, text_y as f32),
                &self.title,
                &Font::default(),
                text_color,
                HorizontalAlignment::Left,
            );
        }

        // --- Draw content area (only when expanded) ---
        if !self.collapsed {
            let content_rect = self.content_rect();
            // Draw a subtle inner background for the content area.
            context.fill_rect(content_rect, Color::rgb(248, 248, 248));
            // Draw border around the content area (left, right, bottom).
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
        assert_eq!(cp.header_height(), 24, "default header height should be 24");
        assert!(cp.content().is_none(), "no content child by default");
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
        assert_eq!(cp.header_height(), 24);

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

    #[test]
    fn collapsible_pane_draw_produces_svg() {
        let mut cp = make_pane();

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
        assert!(svg_output.contains("248,248,248"), "should contain content area background");
        // Header background should be present.
        assert!(svg_output.contains("220,220,220"), "should contain header background");

        // Now collapsed.
        let mut cp2 = CollapsiblePane::new(Rect::new(0, 0, 200, 100), "Collapsed".to_string());
        cp2.set_collapsed(true);

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
            !svg_collapsed.contains("248,248,248"),
            "collapsed pane must not draw content area"
        );
        // But the header should still be visible.
        assert!(svg_collapsed.contains("220,220,220"), "collapsed pane must still draw header");
    }

    // ── 11. Mouse click toggles ─────────────────────────────────────────

    #[test]
    fn collapsible_pane_mouse_click_toggles() {
        let mut cp = make_pane();
        assert!(!cp.is_collapsed());

        // MousePress on the header area (button 1 = left click).
        // Header rect is at (0,0,200,24) — y=12 is well inside.
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
            pos: Point { x: 10, y: 50 }, // content area, y=50 > header_height=24
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
}
