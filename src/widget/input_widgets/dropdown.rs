//! Dropdown widget — standalone dropdown list selector (BLUE13 R2.4).
//!
//! A button-like label showing the current selection. When clicked, it
//! expands to show a scrollable list of options. Selecting an item emits
//! a `changed` signal and collapses the list.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Height of each item row in the expanded dropdown list (pixels).
const ITEM_HEIGHT: u32 = 20;
/// Padding between the button text and the widget edges.
const PADDING: i32 = 4;

/// Dropdown widget for selecting from a list of options.
///
/// # Behaviour
/// - Displays the currently selected item (or a placeholder) inside a button-like
///   rectangle with a ▼ arrow.
/// - On click the list expands below the button; clicking an item selects it and
///   collapses the list.
/// - Losing focus also collapses the list.
pub struct Dropdown {
    base: BaseWidget,
    /// List of option strings.
    items: Vec<String>,
    /// Currently selected index. Defaults to `0` when items is non-empty.
    selected_index: usize,
    /// Whether the dropdown list is expanded.
    expanded: bool,
    /// Signal emitted when the selection changes.
    pub changed: GenericSignal,
}

impl Dropdown {
    /// Creates a new `Dropdown` with the given items and geometry.
    ///
    /// The first item is automatically selected when `items` is non-empty.
    /// The widget's height should accommodate the collapsed button area.
    pub fn new(items: Vec<String>, rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Dropdown, rect, "Dropdown"),
            selected_index: 0,
            expanded: false,
            changed: GenericSignal::new(),
            items,
        }
    }

    /// Returns the full list of option strings.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// Replaces all option strings and resets the selection to the first item
    /// (or index 0 if the list is empty).
    pub fn set_items(&mut self, new_items: Vec<String>) {
        self.items = new_items;
        self.selected_index = 0;
        self.changed.emit();
    }

    /// Returns the currently-selected index (may be out of range if the list
    /// was shrunk after selection).
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Sets the selected index, clamping it to the list bounds.
    ///
    /// Emits the `changed` signal whenever the index actually changes or is
    /// clamped to a different value.
    pub fn set_selected_index(&mut self, index: usize) {
        let clamped = if self.items.is_empty() { 0 } else { index.min(self.items.len() - 1) };
        if self.selected_index != clamped {
            self.selected_index = clamped;
            self.changed.emit();
        }
    }

    /// Returns the text of the currently-selected item, or `None` if the list
    /// is empty.
    pub fn selected_text(&self) -> Option<&str> {
        if self.selected_index < self.items.len() {
            Some(self.items[self.selected_index].as_str())
        } else {
            None
        }
    }

    /// Returns `true` when the dropdown list is currently expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Expands or collapses the dropdown list.
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    /// Toggles the expanded / collapsed state.
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }

    // ─── helpers ────────────────────────────────────────────────────────

    /// Compute the bounding `Rect` of the n-th list item in screen coordinates.
    /// Only meaningful when `expanded == true`.
    fn item_rect(&self, index: usize) -> Rect {
        let geo = self.geometry();
        let item_y = geo.y + geo.height as i32 + (index as u32 * ITEM_HEIGHT) as i32;
        Rect::new(geo.x, item_y, geo.width, ITEM_HEIGHT)
    }

    /// Determine which list item (if any) a point falls on.
    fn hit_test_item(&self, pos: Point) -> Option<usize> {
        if !self.expanded {
            return None;
        }
        (0..self.items.len()).position(|i| self.item_rect(i).contains_point(pos))
    }
}

impl Widget for Dropdown {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        let max_text_w = self.items.iter().map(|s| s.len() as u32).max().unwrap_or(6) * 8 + 30; // + dropdown arrow area
        crate::core::Size::new(max_text_w.max(80), 24)
    }
}

impl EventHandler for Dropdown {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                if self.expanded {
                    // Clicked on one of the list items?
                    if let Some(idx) = self.hit_test_item(*pos) {
                        self.set_selected_index(idx);
                        self.expanded = false;
                        return;
                    }

                    // Clicked back on the button header — collapse
                    let geo = self.geometry();
                    if geo.contains_point(*pos) {
                        self.expanded = false;
                        return;
                    }

                    // Clicked outside the entire widget while expanded – collapse
                    self.expanded = false;
                } else {
                    // Collapsed: toggle expand
                    let geo = self.geometry();
                    if geo.contains_point(*pos) {
                        self.expanded = true;
                    }
                }
            }

            Event::FocusLost => {
                self.expanded = false;
            }

            Event::KeyPress { key, modifiers: _ } => {
                if !self.expanded {
                    // Up / Down arrow keys open the list
                    if (*key == 40 || *key == 38) && !self.items.is_empty() {
                        self.expanded = true;
                    }
                    return;
                }

                match *key {
                    38 => {
                        // Up arrow — previous item
                        if self.selected_index > 0 {
                            self.set_selected_index(self.selected_index - 1);
                        }
                    }
                    40 => {
                        // Down arrow — next item
                        if self.selected_index + 1 < self.items.len() {
                            self.set_selected_index(self.selected_index + 1);
                        }
                    }
                    13 => {
                        // Enter — commit selection, collapse
                        self.expanded = false;
                    }
                    27 => {
                        // Escape — cancel, collapse
                        self.expanded = false;
                    }
                    _ => {}
                }
            }

            _ => {}
        }
    }
}

impl Draw for Dropdown {
    fn draw(&mut self, context: &mut RenderContext) {
        let geo = self.geometry();

        // ── style-derived colours ───────────────────────────────────────
        let bg = self.style().background_color.unwrap_or(Color::from_rgb(255, 255, 255));
        let border = self.style().border_color.unwrap_or(Color::from_rgb(180, 180, 180));
        let text_color = self.style().text_color.unwrap_or(Color::from_rgb(0, 0, 0));
        let placeholder_color = Color::from_rgb(160, 160, 160);
        let highlight_bg = Color::from_rgb(200, 220, 255);
        let highlight_text = Color::from_rgb(0, 0, 0);
        let list_border = Color::from_rgb(150, 150, 150);

        // ── Collapsed / button area ─────────────────────────────────────
        // Background
        context.fill_rect(geo, bg);
        // Border
        context.draw_rect(geo, border);

        // Arrow indicator (▼)
        let arrow = "▼";
        let arrow_x = geo.x + geo.width as i32 - PADDING - 12;

        // Text (selected item or placeholder)
        let label_x = geo.x + PADDING;
        let label_cy = geo.y as f32 + geo.height as f32 / 2.0;

        if let Some(text) = self.selected_text() {
            context.draw_text(
                Point::new(label_x, label_cy as i32),
                text,
                &Font::default(),
                text_color,
            );
            context.draw_text(
                Point::new(arrow_x, label_cy as i32),
                arrow,
                &Font::default(),
                text_color,
            );
        } else {
            context.draw_text(
                Point::new(label_x, label_cy as i32),
                "(Select)",
                &Font::default(),
                placeholder_color,
            );
            context.draw_text(
                Point::new(arrow_x, label_cy as i32),
                arrow,
                &Font::default(),
                placeholder_color,
            );
        }

        // ── Expanded list ───────────────────────────────────────────────
        if !self.expanded || self.items.is_empty() {
            return;
        }

        for i in 0..self.items.len() {
            let item_geo = self.item_rect(i);

            // Background
            let is_selected = i == self.selected_index;
            if is_selected {
                context.fill_rect(item_geo, highlight_bg);
            } else {
                context.fill_rect(item_geo, Color::from_rgb(248, 248, 248));
            }

            // Border (bottom line)
            context.draw_rect(item_geo, list_border);

            // Item text
            let item_color = if is_selected { highlight_text } else { text_color };
            let item_x = item_geo.x + PADDING;
            let item_cy = item_geo.y as f32 + item_geo.height as f32 / 2.0;
            context.draw_text(
                Point::new(item_x, item_cy as i32),
                &self.items[i],
                &Font::default(),
                item_color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    fn make_dropdown() -> Dropdown {
        Dropdown::new(
            vec!["Option A".to_string(), "Option B".to_string(), "Option C".to_string()],
            Rect::new(10, 10, 160, 24),
        )
    }

    #[test]
    fn dropdown_creation_defaults() {
        let dd = make_dropdown();
        assert_eq!(dd.items().len(), 3);
        assert_eq!(dd.items()[0], "Option A");
        assert_eq!(dd.items()[1], "Option B");
        assert_eq!(dd.items()[2], "Option C");
        assert_eq!(dd.selected_index(), 0);
        assert_eq!(dd.selected_text(), Some("Option A"));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_set_items() {
        let mut dd = make_dropdown();
        dd.set_items(vec!["X".to_string(), "Y".to_string()]);
        assert_eq!(dd.items().len(), 2);
        assert_eq!(dd.items()[0], "X");
        assert_eq!(dd.items()[1], "Y");
        assert_eq!(dd.selected_index(), 0);
    }

    #[test]
    fn dropdown_set_items_empty_resets_selection() {
        let mut dd = make_dropdown();
        dd.set_selected_index(2);
        dd.set_items(Vec::new());
        assert!(dd.items().is_empty());
        assert_eq!(dd.selected_index(), 0);
        assert!(dd.selected_text().is_none());
    }

    #[test]
    fn dropdown_select_item() {
        let mut dd = make_dropdown();
        dd.set_selected_index(1);
        assert_eq!(dd.selected_index(), 1);
        assert_eq!(dd.selected_text(), Some("Option B"));
    }

    #[test]
    fn dropdown_select_item_clamps_out_of_range() {
        let mut dd = make_dropdown();
        dd.set_selected_index(100);
        assert_eq!(dd.selected_index(), 2);
        assert_eq!(dd.selected_text(), Some("Option C"));
    }

    #[test]
    fn dropdown_toggle_expand() {
        let mut dd = make_dropdown();
        assert!(!dd.is_expanded());
        dd.toggle();
        assert!(dd.is_expanded());
        dd.toggle();
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_set_expanded() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        assert!(dd.is_expanded());
        dd.set_expanded(false);
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_focus_lost_collapses() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        dd.handle_event(&Event::FocusLost);
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_mouse_press_toggles() {
        let mut dd = make_dropdown();
        // Click inside the widget geometry to expand
        dd.handle_event(&Event::mouse_press(20, 20, 1));
        assert!(dd.is_expanded());

        // Click inside the widget geometry again to collapse
        dd.handle_event(&Event::mouse_press(20, 20, 1));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_mouse_press_outside_collapses_when_expanded() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        // Click far outside
        dd.handle_event(&Event::mouse_press(999, 999, 1));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_mouse_select_item() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);

        // The first item is at y = 10 + 24 + 0*20 = 34
        dd.handle_event(&Event::mouse_press(15, 35, 1));
        assert_eq!(dd.selected_index(), 0);
        assert!(!dd.is_expanded()); // selection collapses

        // Expand again and select the 3rd item (index 2)
        dd.set_expanded(true);
        dd.handle_event(&Event::mouse_press(15, 35 + 2 * 20, 1));
        assert_eq!(dd.selected_index(), 2);
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_keyboard_navigation() {
        let mut dd = make_dropdown();
        // Down arrow opens the expanded list
        dd.handle_event(&Event::key_press(40, 0));
        assert!(dd.is_expanded());

        // Down arrow moves to next item
        dd.handle_event(&Event::key_press(40, 0));
        assert_eq!(dd.selected_index(), 1);

        dd.handle_event(&Event::key_press(40, 0));
        assert_eq!(dd.selected_index(), 2);

        // Already at last — should stay
        dd.handle_event(&Event::key_press(40, 0));
        assert_eq!(dd.selected_index(), 2);

        // Up arrow
        dd.handle_event(&Event::key_press(38, 0));
        assert_eq!(dd.selected_index(), 1);

        dd.handle_event(&Event::key_press(38, 0));
        assert_eq!(dd.selected_index(), 0);

        // Already at first — should stay
        dd.handle_event(&Event::key_press(38, 0));
        assert_eq!(dd.selected_index(), 0);

        // Enter commits and collapses
        dd.handle_event(&Event::key_press(13, 0));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_escape_collapses() {
        let mut dd = make_dropdown();
        dd.set_expanded(true);
        dd.handle_event(&Event::key_press(27, 0));
        assert!(!dd.is_expanded());
    }

    #[test]
    fn dropdown_disabled_ignores_events() {
        let mut dd = make_dropdown();
        dd.set_expanded(false);
        dd.set_enabled(false);
        dd.handle_event(&Event::mouse_press(20, 20, 1));
        assert!(!dd.is_expanded(), "disabled widget should not expand");
    }

    #[test]
    fn dropdown_draw_does_not_panic() {
        // Minimal smoke test — we only verify that the draw method runs
        // without panicking. A real render backend would be needed for
        // meaningful pixel tests.
        let mut dd = make_dropdown();

        // Create a minimal software render backend for testing
        use crate::render::PaintBackend;
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(200, 200), 1.0);
        backend.begin_frame(crate::core::Color::WHITE);
        let mut ctx = crate::render::RenderContext::new(&mut backend);

        // Collapsed draw
        dd.draw(&mut ctx);

        // Expanded draw
        dd.set_expanded(true);
        dd.draw(&mut ctx);
    }

    #[test]
    fn dropdown_geometry_delegation() {
        let mut dd = make_dropdown();
        dd.set_geometry(Rect::new(0, 0, 200, 30));
        assert_eq!(dd.geometry(), Rect::new(0, 0, 200, 30));
    }

    #[test]
    fn dropdown_visibility() {
        let mut dd = make_dropdown();
        assert!(dd.is_visible());
        dd.hide();
        assert!(!dd.is_visible());
        dd.show();
        assert!(dd.is_visible());
    }

    #[test]
    fn dropdown_enabled() {
        let mut dd = make_dropdown();
        assert!(dd.is_enabled());
        dd.set_enabled(false);
        assert!(!dd.is_enabled());
        dd.set_enabled(true);
        assert!(dd.is_enabled());
    }

    #[test]
    fn dropdown_id_kind() {
        let dd_a = make_dropdown();
        let dd_b = make_dropdown();
        assert_ne!(dd_a.id(), dd_b.id());
        assert_eq!(dd_a.kind(), WidgetKind::Dropdown);
        assert_eq!(dd_b.kind(), WidgetKind::Dropdown);
    }

    #[test]
    fn dropdown_signal_accessors() {
        let dd = make_dropdown();
        let _ = &dd.changed;
        let _ = dd.changed_signal();
        let _ = dd.clicked_signal();
    }

    #[test]
    fn dropdown_item_rect_computation() {
        let dd = make_dropdown(); // Rect(10, 10, 160, 24)
        let r = dd.item_rect(0);
        assert_eq!(r, Rect::new(10, 34, 160, 20));
        let r = dd.item_rect(2);
        assert_eq!(r, Rect::new(10, 74, 160, 20));
    }

    #[test]
    fn dropdown_selected_text_none_when_empty() {
        let dd = Dropdown::new(Vec::new(), Rect::new(0, 0, 100, 24));
        assert!(dd.selected_text().is_none());
    }
}
