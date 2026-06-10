//! NavigationDrawer widget — an Android-style slide-out side navigation drawer.
//!
//! The NavigationDrawer presents a semi-transparent overlay behind a side panel
//! that lists navigation items (icon + label). It supports open/close state,
//! item selection, and emits signals for opened, closed, and item_selected events.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A single item in the navigation drawer, consisting of an icon and a label.
#[derive(Clone, Debug)]
pub struct DrawerItem {
    /// Icon displayed as text (emoji or character).
    pub icon: String,
    /// Display label text.
    pub label: String,
}

/// Navigation Drawer widget — an Android-style slide-out side navigation drawer.
///
/// The drawer can be opened and closed programmatically or by user interaction.
/// When open, it renders a semi-transparent overlay covering the full geometry
/// with a side panel on the left containing navigation items. Clicking an item
/// selects it and emits `item_selected`. Clicking the overlay closes the drawer.
pub struct NavigationDrawer {
    base: BaseWidget,
    /// Whether the drawer is currently open.
    open: bool,
    /// List of drawer items to display.
    items: Vec<DrawerItem>,
    /// Index of the currently selected item.
    selected_index: usize,
    /// Panel width in pixels.
    panel_width: u32,
    /// Emitted when the drawer opens.
    pub opened: GenericSignal,
    /// Emitted when the drawer closes.
    pub closed: GenericSignal,
    /// Emitted when an item is selected, with the index of the selected item.
    pub item_selected: Signal1<usize>,
}

impl NavigationDrawer {
    /// Creates a new NavigationDrawer widget with the given geometry.
    ///
    /// The geometry should cover the full screen/overlay area. The side panel
    /// defaults to 280px wide.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::NavigationDrawer, geometry, "NavigationDrawer"),
            open: false,
            items: Vec::new(),
            selected_index: 0,
            panel_width: 280,
            opened: GenericSignal::new(),
            closed: GenericSignal::new(),
            item_selected: Signal1::new(),
        }
    }

    /// Opens the navigation drawer. Emits `opened` signal.
    pub fn open(&mut self) {
        if !self.open {
            self.open = true;
            self.opened.emit();
            self.base.request_redraw();
        }
    }

    /// Closes the navigation drawer. Emits `closed` signal.
    pub fn close(&mut self) {
        if self.open {
            self.open = false;
            self.closed.emit();
            self.base.request_redraw();
        }
    }

    /// Toggles the open/close state of the drawer.
    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Returns whether the drawer is currently open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Adds a new item to the navigation drawer.
    ///
    /// `icon` is a text/emoji character displayed to the left of the label.
    pub fn add_item(&mut self, icon: &str, label: &str) {
        self.items.push(DrawerItem { icon: icon.to_string(), label: label.to_string() });
        self.base.request_redraw();
    }

    /// Sets the selected item index. Emits `item_selected` if changed.
    pub fn set_selected_index(&mut self, index: usize) {
        if index < self.items.len() && self.selected_index != index {
            self.selected_index = index;
            self.item_selected.emit(index);
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected item index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Returns a reference to the items list.
    pub fn items(&self) -> &[DrawerItem] {
        &self.items
    }

    /// Returns the panel width in pixels.
    pub fn panel_width(&self) -> u32 {
        self.panel_width
    }

    /// Sets the panel width in pixels.
    pub fn set_panel_width(&mut self, width: u32) {
        self.panel_width = width;
        self.base.request_redraw();
    }
}

impl Widget for NavigationDrawer {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for NavigationDrawer {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.open {
            return;
        }

        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Draw semi-transparent overlay covering the full geometry
        let overlay_color = Color::rgba(0, 0, 0, 100);
        context.fill_rect(rect, overlay_color);

        // Draw side panel on the left
        let panel_width = self.panel_width.min(rect.width);
        let panel_rect = Rect::new(rect.x, rect.y, panel_width, rect.height);
        let panel_color = if !is_enabled {
            Color::rgba(245, 245, 245, 240)
        } else {
            Color::rgba(255, 255, 255, 250)
        };
        context.fill_rect(panel_rect, panel_color);

        // Draw a subtle right border on the panel
        context.draw_rect_stroke(panel_rect, Color::rgba(0, 0, 0, 20), 1);

        // Draw header area
        let header_height: u32 = 60;
        let header_rect = Rect::new(rect.x, rect.y, panel_width, header_height);
        let header_color = Color::rgba(240, 245, 250, 255);
        context.fill_rect(header_rect, header_color);

        // Draw header title text
        let font = Font::new("sans-serif", 16.0, true, false);
        let header_text = "Navigation";
        let metrics = context.measure_text(header_text, &font);
        let header_text_x = rect.x + 16;
        let header_text_y = rect.y + (header_height as i32 / 2) + (metrics.ascent as i32 / 2);
        let header_color_text = Color::rgba(30, 30, 30, 255);
        context.draw_text(
            Point::new(header_text_x, header_text_y),
            header_text,
            &font,
            header_color_text,
        );

        // Draw items vertically
        let item_height: u32 = 48;
        let item_font = Font::new("sans-serif", 14.0, false, false);
        let icon_font = Font::new("sans-serif", 16.0, false, false);
        let item_metrics = context.measure_text("A", &item_font);
        let start_y = rect.y + header_height as i32;

        for (i, item) in self.items.iter().enumerate() {
            let y_offset = start_y + (i as i32 * item_height as i32);
            let item_rect = Rect::new(rect.x, y_offset, panel_width, item_height);

            // Highlight selected item
            if i == self.selected_index {
                let highlight_color = Color::rgba(220, 230, 245, 255);
                context.fill_rect(item_rect, highlight_color);
            } else {
                // Draw hover-like subtle background
                let item_bg = Color::rgba(255, 255, 255, 255);
                context.fill_rect(item_rect, item_bg);
            }

            // Vertical center position for text
            let text_center_y =
                y_offset + (item_height as i32 / 2) + (item_metrics.ascent as i32 / 2);

            // Draw icon
            let icon_x = rect.x + 16;
            let icon_color = if i == self.selected_index {
                Color::rgba(30, 100, 200, 255)
            } else {
                Color::rgba(80, 80, 80, 255)
            };
            context.draw_text(
                Point::new(icon_x, text_center_y),
                &item.icon,
                &icon_font,
                icon_color,
            );

            // Draw label
            let icon_width: u32 = 24;
            let label_x = rect.x + 16 + icon_width as i32 + 8;
            let label_color = if i == self.selected_index {
                Color::rgba(30, 100, 200, 255)
            } else {
                Color::rgba(50, 50, 50, 255)
            };
            context.draw_text(
                Point::new(label_x, text_center_y),
                &item.label,
                &item_font,
                label_color,
            );

            // Draw divider line between items
            if i > 0 {
                let divider_y = y_offset;
                let divider_color = Color::rgba(0, 0, 0, 10);
                context.fill_rect(
                    Rect::new(rect.x + 16, divider_y, panel_width - 32, 1),
                    divider_color,
                );
            }
        }
    }
}

impl EventHandler for NavigationDrawer {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() || !self.open {
            self.base.handle_event(event);
            return;
        }

        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }

                let rect = self.geometry();
                let panel_width = self.panel_width.min(rect.width);

                // Check if click is on the overlay (outside the side panel)
                if pos.x > rect.x + panel_width as i32 {
                    if let Event::MouseRelease { .. } = event {
                        self.close();
                    }
                    return;
                }

                // Check if click is inside the panel on an item
                let header_height: u32 = 60;
                let item_height: u32 = 48;
                let start_y = rect.y + header_height as i32;

                if pos.x >= rect.x && pos.x <= rect.x + panel_width as i32 {
                    let relative_y = pos.y - start_y;
                    if relative_y >= 0 {
                        let item_index = (relative_y as u32) / item_height;
                        if (item_index as usize) < self.items.len() {
                            if let Event::MouseRelease { .. } = event {
                                self.set_selected_index(item_index as usize);
                            }
                            return;
                        }
                    }
                }

                // Fall through to base handler for other events
                self.base.handle_event(event);
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
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };

    fn make_drawer() -> NavigationDrawer {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        drawer.add_item("🏠", "Home");
        drawer.add_item("🔍", "Search");
        drawer.add_item("⚙️", "Settings");
        drawer.add_item("👤", "Profile");
        drawer
    }

    #[test]
    fn drawer_default_is_closed() {
        let drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        assert!(!drawer.is_open());
        assert_eq!(drawer.kind(), WidgetKind::NavigationDrawer);
        assert_eq!(drawer.selected_index(), 0);
        assert!(drawer.items().is_empty());
    }

    #[test]
    fn drawer_open_signal_emits() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        let opened = Arc::new(AtomicBool::new(false));
        let o = opened.clone();
        drawer.opened.connect(move || {
            o.store(true, Ordering::SeqCst);
        });

        drawer.open();
        assert!(drawer.is_open());
        assert!(opened.load(Ordering::SeqCst));
    }

    #[test]
    fn drawer_close_signal_emits() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        drawer.open();
        assert!(drawer.is_open());

        let closed = Arc::new(AtomicBool::new(false));
        let c = closed.clone();
        drawer.closed.connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        drawer.close();
        assert!(!drawer.is_open());
        assert!(closed.load(Ordering::SeqCst));
    }

    #[test]
    fn drawer_toggle_flips_state() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        assert!(!drawer.is_open());

        drawer.toggle();
        assert!(drawer.is_open());

        drawer.toggle();
        assert!(!drawer.is_open());
    }

    #[test]
    fn drawer_add_item() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        assert_eq!(drawer.items().len(), 0);

        drawer.add_item("📁", "Files");
        assert_eq!(drawer.items().len(), 1);
        assert_eq!(drawer.items()[0].icon, "📁");
        assert_eq!(drawer.items()[0].label, "Files");

        drawer.add_item("📧", "Mail");
        assert_eq!(drawer.items().len(), 2);
    }

    #[test]
    fn drawer_set_selected_index_emits_signal() {
        let mut drawer = make_drawer();
        let selected = Arc::new(AtomicUsize::new(usize::MAX));
        let s = selected.clone();
        drawer.item_selected.connect(move |index: Arc<usize>| {
            s.store(*index, Ordering::SeqCst);
        });

        drawer.set_selected_index(2);
        assert_eq!(drawer.selected_index(), 2);
        assert_eq!(selected.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn drawer_set_selected_index_out_of_bounds_noop() {
        let mut drawer = make_drawer();
        drawer.set_selected_index(99);
        // Should not change because index is out of bounds
        assert_eq!(drawer.selected_index(), 0);
    }

    #[test]
    fn drawer_set_selected_index_same_value_no_reemit() {
        let mut drawer = make_drawer();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        drawer.item_selected.connect(move |_: Arc<usize>| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        drawer.set_selected_index(0);
        // Should not emit because it's already 0
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn drawer_mouse_press_on_overlay_closes() {
        let mut drawer = make_drawer();
        drawer.open();
        assert!(drawer.is_open());

        // Click on overlay (outside panel, at x=350)
        drawer.handle_event(&Event::MousePress { pos: Point::new(350, 300), button: 1 });
        // Press alone does not close; release does
        assert!(drawer.is_open());

        drawer.handle_event(&Event::MouseRelease { pos: Point::new(350, 300), button: 1 });
        assert!(!drawer.is_open());
    }

    #[test]
    fn drawer_mouse_release_on_item_selects() {
        let mut drawer = make_drawer();
        drawer.open();

        // Click on item at index 1 ("Search") - header=60, item_height=48, item 1 starts at y=108
        let target_y = 60 + 1 * 48 + 24;
        drawer.handle_event(&Event::MousePress { pos: Point::new(20, target_y), button: 1 });
        // Not yet selected on press
        assert_eq!(drawer.selected_index(), 0);

        drawer.handle_event(&Event::MouseRelease { pos: Point::new(20, target_y), button: 1 });
        assert_eq!(drawer.selected_index(), 1);
    }

    #[test]
    fn drawer_mouse_click_other_button_noop() {
        let mut drawer = make_drawer();
        drawer.open();

        // Right-click on overlay
        drawer.handle_event(&Event::MousePress { pos: Point::new(350, 300), button: 2 });
        drawer.handle_event(&Event::MouseRelease { pos: Point::new(350, 300), button: 2 });
        assert!(drawer.is_open());
        assert_eq!(drawer.selected_index(), 0);
    }

    #[test]
    fn drawer_disabled_blocks_events() {
        let mut drawer = make_drawer();
        drawer.set_enabled(false);
        drawer.open();

        // Click release on overlay - should NOT close because disabled
        drawer.handle_event(&Event::MousePress { pos: Point::new(350, 300), button: 1 });
        drawer.handle_event(&Event::MouseRelease { pos: Point::new(350, 300), button: 1 });
        assert!(drawer.is_open());
    }

    #[test]
    fn drawer_closed_ignores_events() {
        let mut drawer = make_drawer();
        assert!(!drawer.is_open());

        // Events while closed should be ignored for drawer-specific behavior
        drawer.handle_event(&Event::MousePress { pos: Point::new(20, 100), button: 1 });
        drawer.handle_event(&Event::MouseRelease { pos: Point::new(20, 100), button: 1 });
        assert_eq!(drawer.selected_index(), 0);
    }

    #[test]
    fn drawer_panel_width_customization() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        assert_eq!(drawer.panel_width(), 280);

        drawer.set_panel_width(320);
        assert_eq!(drawer.panel_width(), 320);
    }

    #[test]
    fn drawer_svg_output_closed() {
        let mut drawer = make_drawer();
        // Closed drawer produces an empty SVG (just background / no content)
        let svg = render_to_svg(&mut drawer);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn drawer_svg_output_open() {
        let mut drawer = make_drawer();
        drawer.open();
        let svg = render_to_svg(&mut drawer);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"400\""));
        assert!(svg.contains("height=\"600\""));
    }

    #[test]
    fn drawer_open_close_signals_not_reemitted() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        let open_count = Arc::new(AtomicUsize::new(0));
        let close_count = Arc::new(AtomicUsize::new(0));
        let oc = open_count.clone();
        let cc = close_count.clone();
        drawer.opened.connect(move || {
            oc.fetch_add(1, Ordering::SeqCst);
        });
        drawer.closed.connect(move || {
            cc.fetch_add(1, Ordering::SeqCst);
        });

        // Open twice - should only emit once
        drawer.open();
        drawer.open();
        assert_eq!(open_count.load(Ordering::SeqCst), 1);

        // Close twice - should only emit once
        drawer.close();
        drawer.close();
        assert_eq!(close_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn drawer_item_selected_signal_is_typed() {
        let mut drawer = make_drawer();
        let captured = Arc::new(std::sync::Mutex::new(None));
        let c = captured.clone();
        drawer.item_selected.connect(move |val: Arc<usize>| {
            *c.lock().unwrap() = Some(*val);
        });

        drawer.set_selected_index(3);
        assert_eq!(*captured.lock().unwrap(), Some(3));
    }

    #[test]
    fn drawer_drawer_item_struct() {
        let item = DrawerItem { icon: "📁".to_string(), label: "Documents".to_string() };
        assert_eq!(item.icon, "📁");
        assert_eq!(item.label, "Documents");
    }
}
