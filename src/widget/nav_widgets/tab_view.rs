// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TabView widget — iOS-style segmented tab page view.
//!
//! Displays a horizontal segmented tab bar at the top and a content area
//! below showing the selected tab's content. Supports add/remove/clear
//! operations on tabs and emits a `tab_changed` signal on selection.

use crate::core::{Color, Font, HorizontalAlignment, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A single tab page with a title, optional content, and optional icon name.
pub struct TabPage {
    /// Display title shown in the tab bar.
    pub title: String,
    /// Optional content widget displayed when this tab is selected.
    pub content: Option<Box<dyn Widget>>,
    /// Optional icon identifier for the tab.
    pub icon: Option<String>,
}

/// iOS-style segmented tab page view.
///
/// Manages a vector of `TabPage` instances and draws a top segmented bar
/// plus the content of the currently selected tab below.
pub struct TabView {
    base: BaseWidget,
    tabs: Vec<TabPage>,
    selected_index: usize,
    /// Emitted when the selected tab index changes.
    pub tab_changed: Signal1<usize>,
}

impl TabView {
    /// Creates a new empty TabView widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TabView, geometry, "TabView"),
            tabs: Vec::new(),
            selected_index: 0,
            tab_changed: Signal1::new(),
        }
    }

    /// Adds a new tab page at the end of the tab list.
    /// If this is the first tab, it becomes the selected tab.
    pub fn add_tab(
        &mut self,
        title: impl Into<String>,
        content: Option<Box<dyn Widget>>,
        icon: Option<impl Into<String>>,
    ) {
        let was_empty = self.tabs.is_empty();
        self.tabs.push(TabPage { title: title.into(), content, icon: icon.map(|i| i.into()) });
        if was_empty {
            self.set_current_index(0);
        }
        self.base.request_redraw();
    }

    /// Removes the tab at the given index.
    /// Adjusts selection if the removed tab was selected.
    pub fn remove_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.tabs.len() {
            self.selected_index = self.tabs.len() - 1;
        }
        self.base.request_redraw();
    }

    /// Returns the number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Removes all tabs and resets the selection.
    pub fn clear_tabs(&mut self) {
        self.tabs.clear();
        self.selected_index = 0;
        self.base.request_redraw();
    }

    /// Sets the current tab index. Clamped to valid range.
    /// Emits `tab_changed` if the index actually changed.
    pub fn set_current_index(&mut self, index: usize) {
        if self.tabs.is_empty() {
            return;
        }
        let clamped = index.min(self.tabs.len() - 1);
        if self.selected_index != clamped {
            self.selected_index = clamped;
            self.tab_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected tab index.
    pub fn current_index(&self) -> usize {
        self.selected_index
    }

    /// Returns a reference to the tabs vector.
    pub fn tabs(&self) -> &[TabPage] {
        &self.tabs
    }

    /// Returns a mutable reference to the tabs vector.
    pub fn tabs_mut(&mut self) -> &mut Vec<TabPage> {
        &mut self.tabs
    }

    /// The tab strip's height: 40 (Material's `Tab` height, which is a 14 px line plus the
    /// 48 px touch floor this crate uses for a primary target).
    ///
    /// It is a constant rather than the two `let tab_bar_height: u32 = 40;` declarations it
    /// used to be — one in `draw` and one in `handle_event`. Two copies of one layout fact is
    /// the drift shape this crate keeps paying for: the hit test would keep accepting presses
    /// in a strip the renderer had stopped drawing whenever one of them was edited.
    pub const TAB_BAR_HEIGHT: u32 = 40;

    /// The strip band at the top of the control's own rectangle.
    ///
    /// The single derivation the strip's fill, the indicator, the hit test and the content
    /// area below all read.
    pub fn tab_bar_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x, rect.y, rect.width, Self::TAB_BAR_HEIGHT.min(rect.height))
    }

    /// The rectangle the selected tab's content occupies: what the strip leaves.
    pub fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let strip = self.tab_bar_rect();
        let top = strip.y + strip.height as i32;
        Rect::new(rect.x, top, rect.width, (rect.y + rect.height as i32 - top).max(0) as u32)
    }
}

impl Widget for TabView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `TabView`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch: the published
/// name is `selected_index`, backed by the `current_index` accessors.
impl WidgetProperties for TabView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_index" => Ok(CapabilityValue::UInt(self.current_index() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => {
                self.set_current_index(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["selected_index", BASE_PROPERTY_NAMES]
    }
}

impl Draw for TabView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // The strip and the content area come from the control's own band derivation, so the
        // strip's height, the hit test and the content's top edge cannot disagree.
        let tab_bar_rect = self.tab_bar_rect();
        let tab_bar_height = tab_bar_rect.height;
        let content_rect = self.content_rect();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("tab_view");
        // `tab_view` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The content area's fill is therefore a step toward
        // the foreground, so the page reads as a surface of its own.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let content_background = resolved.blend(&text_color, 0.08);
        // The strip behind the tabs is a further step away, so selected and
        // unselected tabs read as different states of the same chrome.
        let strip_background = content_background.blend(&text_color, 0.08);
        let inactive_tab = content_background.blend(&text_color, 0.05);
        // The selected indicator is a *selection* state, so it reads the theme's
        // primary token rather than a literal blue.
        let indicator = crate::style::resolved_theme_style("button")
            .and_then(|button| button.background_color)
            .unwrap_or_else(|| content_background.blend(&text_color, 0.6));
        let selected_text = indicator;
        let inactive_text = text_color.blend(&content_background, 0.3);
        // The separator under the strip is secondary chrome, derived from the same pair.
        let separator = content_background.blend(&text_color, 0.2);

        // Draw tab bar background
        context.fill_rect(tab_bar_rect, strip_background);

        if self.tabs.is_empty() {
            // Draw empty content area
            context.fill_rect(content_rect, content_background);
            return;
        }

        // Draw each tab header
        let tab_count = self.tabs.len() as u32;
        let tab_width = rect.width / tab_count.max(1);
        let font = Font::simple("sans-serif", 12.0);

        for i in 0..self.tabs.len() {
            let tab_x = rect.x + (i as u32 * tab_width) as i32;
            let tab_rect = Rect::new(tab_x, rect.y, tab_width, tab_bar_height);
            let is_selected = i == self.selected_index;

            // Background
            let bg_color = if is_selected { content_background } else { inactive_tab };
            context.fill_rect(tab_rect, bg_color);

            // Selected tab indicator line
            if is_selected {
                let indicator_rect =
                    Rect::new(tab_x, rect.y + tab_bar_height as i32 - 3, tab_width, 3);
                context.fill_rect(indicator_rect, indicator);
            }

            // Draw tab title (with icon prefix if available)
            let tab = &self.tabs[i];
            let display_text = if let Some(ref icon_name) = tab.icon {
                format!("{} {}", icon_name, tab.title)
            } else {
                tab.title.clone()
            };

            let text_color = if is_selected { selected_text } else { inactive_text };

            // The caption is centred on **both** axes through the shared line box. The previous
            // form computed `rect.y + (tab_bar_height - metrics.height) / 2` by hand and passed
            // it as the glyph box's top edge — and `metrics.height` is the *measurement* height,
            // which is the same number as the line height only for a single-line ASCII label.
            // The line box is what `text_line` derives from the same font, so the caption cannot
            // sit half a line off; `draw_text_fitted` with `Center` then bounds it to the tab, so
            // a long title is elided rather than running over its neighbour.
            context.draw_text_fitted(
                context.text_line(tab_rect, &font),
                &display_text,
                &font,
                text_color,
                HorizontalAlignment::Center,
            );
        }

        // Draw separator line below tab bar
        let separator_rect = Rect::new(rect.x, rect.y + tab_bar_height as i32 - 1, rect.width, 1);
        context.fill_rect(separator_rect, separator);

        // Draw selected tab content area (child widget rendering is delegated)
        context.fill_rect(content_rect, content_background);
    }
}

impl EventHandler for TabView {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 && !self.tabs.is_empty() {
                    // Check if click is in the tab bar area. The band comes from the control's
                    // own derivation, so the region that accepts a press is by construction the
                    // region the renderer drew.
                    let strip = self.tab_bar_rect();
                    let rect = self.geometry();
                    if pos.y >= strip.y && pos.y < strip.y + strip.height as i32 {
                        let tab_count = self.tabs.len() as u32;
                        let tab_width = rect.width / tab_count.max(1);
                        let relative_x = (pos.x - rect.x) as u32;
                        let clicked_index = (relative_x / tab_width) as usize;
                        if clicked_index < self.tabs.len() {
                            self.set_current_index(clicked_index);
                        }
                    }
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn make_tab_view() -> TabView {
        TabView::new(Rect::new(0, 0, 300, 400))
    }

    /// The band the renderer draws and the band the hit test accepts are one derivation.
    ///
    /// The strip's height used to be two separate `let tab_bar_height: u32 = 40;` declarations
    /// — one in `draw`, one in `handle_event` — so editing one would leave the control
    /// accepting presses in a strip it had stopped drawing. Both now read
    /// [`TabView::tab_bar_rect`].
    #[test]
    fn the_drawn_strip_and_the_hit_region_are_one_derivation() {
        let tv = make_tab_view();
        let strip = tv.tab_bar_rect();
        let content = tv.content_rect();
        assert_eq!(strip.height, TabView::TAB_BAR_HEIGHT);
        assert_eq!(strip.y, tv.geometry().y);
        // The content area begins exactly where the strip ends — no gap, no overlap.
        assert_eq!(
            content.y,
            strip.y + strip.height as i32,
            "the content area starts at the strip's bottom edge"
        );
        // And together they tile the control's own rectangle.
        assert_eq!(content.height as i32 + strip.height as i32, tv.geometry().height as i32);
    }

    /// One ink box per text `<path>` in document order, as `(left, top, right, bottom)`.
    ///
    /// # Why the ink and not the string
    ///
    /// Text leaves the SVG backend as the `font8x8` rectangles the rasteriser fills — one
    /// axis-aligned subpath per set bitmap bit — so a caption is not in the document in any form
    /// and a test has to locate a run by *where* it is. That is the stronger check: the old form
    /// matched `>Alpha</text>` and read the element's `y`, so a caption drawn on the wrong line
    /// with a correct attribute would have passed it.
    ///
    /// One element is one `draw_text`, so this is one box per caption. Subpaths are not
    /// deduplicated: a glyph box wider than the 8 bitmap columns maps two columns to one pixel
    /// and emits the same rectangle twice, exactly as the rasteriser fills it twice.
    fn text_run_boxes(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        let mut boxes = Vec::new();
        for line in svg.lines() {
            let Some(path_at) = line.find("<path ") else { continue };
            let Some(d_at) = line[path_at..].find("d=\"") else { continue };
            let start = path_at + d_at + 3;
            let Some(end) = line[start..].find('"') else { continue };
            let mut bounds: Option<(i32, i32, i32, i32)> = None;
            for subpath in line[start..start + end].split('M').skip(1) {
                let numbers: Vec<i32> = subpath
                    .split(|c: char| !c.is_ascii_digit() && c != '-')
                    .filter(|part| !part.is_empty())
                    .filter_map(|part| part.parse().ok())
                    .collect();
                if numbers.len() < 4 {
                    continue;
                }
                let (x, y, w, h) = (numbers[0], numbers[1], numbers[2], numbers[3]);
                let bit = (x, y, x + w, y + h);
                bounds = Some(match bounds {
                    None => bit,
                    Some((l, t, r, b)) => (l.min(bit.0), t.min(bit.1), r.max(bit.2), b.max(bit.3)),
                });
            }
            if let Some(union) = bounds {
                boxes.push(union);
            }
        }
        boxes
    }

    /// A tab caption is centred on its tab's own line box, not on a hand-computed pair of axes.
    ///
    /// The previous form derived `text_x` and `text_y` inline from `measure_text(..).height` and
    /// handed both to `draw_text`. The line box is the shared primitive for exactly this, and it
    /// is what makes a caption with a descender ("/g/j") sit the same as one without.
    ///
    /// The captions are located by the **tab they lie in** rather than by the string they spell:
    /// the string is no longer in the document, and a geometric lookup is what the assertion is
    /// about anyway. Both captions start with a bitmap row that is lit in its first row, so a
    /// run's ink top is its glyph box's top edge.
    #[test]
    fn a_tab_caption_sits_on_its_tabs_line_box() {
        let mut tv = make_tab_view();
        tv.add_tab("Alpha", None, None::<&str>);
        tv.add_tab("Beta", None, None::<&str>);
        let svg = render_to_svg(&mut tv);
        let strip = tv.tab_bar_rect();

        let font = Font::simple("sans-serif", 12.0);
        let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(300, 400));
        let context = crate::render::RenderContext::new(&mut backend);
        let line_h = context.measure_text("M", &font).height as i32;
        let expected_y = strip.y + (strip.height as i32 - line_h) / 2;

        let tab_width = tv.geometry().width / 2;
        let runs = text_run_boxes(&svg);
        let caption_of = |index: usize| -> (i32, i32, i32, i32) {
            let left = tv.geometry().x + (index as u32 * tab_width) as i32;
            runs.iter()
                .find(|(l, _, r, _)| {
                    let centre = (l + r) / 2;
                    centre >= left && centre < left + tab_width as i32
                })
                .copied()
                .unwrap_or_else(|| panic!("tab {index} painted no caption in {left}.."))
        };
        let alpha = caption_of(0);
        let beta = caption_of(1);
        assert_eq!(alpha.1, expected_y, "a caption belongs on the strip's line box");
        assert_eq!(beta.1, expected_y, "and every caption shares it");
        // The caption is bounded by its own tab, which the attribute assertion could not see: a
        // run centred on the strip but not on its tab would still have had the right `y`.
        for (index, run) in [alpha, beta].into_iter().enumerate() {
            let left = tv.geometry().x + (index as u32 * tab_width) as i32;
            assert!(
                run.0 >= left && run.2 <= left + tab_width as i32,
                "tab {index}: caption {run:?} must stay inside {left}..{}",
                left + tab_width as i32
            );
            assert!(run.2 > run.0, "tab {index}: the caption laid down ink: {run:?}");
        }
        assert!(alpha.0 < beta.0, "the tabs read left to right: {alpha:?} then {beta:?}");
    }

    #[test]
    fn tab_view_default_state() {
        let tv = make_tab_view();
        assert_eq!(tv.tab_count(), 0);
        assert_eq!(tv.current_index(), 0);
        assert_eq!(tv.kind(), WidgetKind::TabView);
    }

    #[test]
    fn tab_view_add_and_select() {
        let mut tv = make_tab_view();
        tv.add_tab("Tab 1", None, None::<&str>);
        tv.add_tab("Tab 2", None, None::<&str>);
        assert_eq!(tv.tab_count(), 2);
        assert_eq!(tv.current_index(), 0);

        tv.set_current_index(1);
        assert_eq!(tv.current_index(), 1);
    }

    #[test]
    fn tab_view_signal_emits() {
        let mut tv = make_tab_view();
        tv.add_tab("First", None, None::<&str>);
        tv.add_tab("Second", None, None::<&str>);

        let captured = Arc::new(AtomicUsize::new(usize::MAX));
        tv.tab_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                captured.store(*val, Ordering::SeqCst);
            }
        });

        tv.set_current_index(1);
        assert_eq!(captured.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn tab_view_remove_tab() {
        let mut tv = make_tab_view();
        tv.add_tab("A", None, None::<&str>);
        tv.add_tab("B", None, None::<&str>);
        tv.add_tab("C", None, None::<&str>);
        tv.set_current_index(2);
        tv.remove_tab(2);
        assert_eq!(tv.tab_count(), 2);
        assert_eq!(tv.current_index(), 1);
    }

    #[test]
    fn tab_view_clear_tabs() {
        let mut tv = make_tab_view();
        tv.add_tab("X", None, None::<&str>);
        tv.add_tab("Y", None, None::<&str>);
        tv.clear_tabs();
        assert_eq!(tv.tab_count(), 0);
        assert_eq!(tv.current_index(), 0);
    }

    #[test]
    fn tab_view_mouse_click_switches_tab() {
        let mut tv = make_tab_view();
        tv.add_tab("Foo", None, None::<&str>);
        tv.add_tab("Bar", None, None::<&str>);

        // Click on second tab header (x=150..299, y=0..40)
        tv.handle_event(&Event::MousePress { pos: Point::new(160, 20), button: 1 });
        assert_eq!(tv.current_index(), 1);
    }

    #[test]
    fn tab_view_svg_output() {
        let mut tv = make_tab_view();
        tv.add_tab("One", None, None::<&str>);
        tv.add_tab("Two", None, None::<&str>);
        let svg = render_to_svg(&mut tv);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn tab_view_set_current_index_noop_when_empty() {
        let mut tv = make_tab_view();
        tv.set_current_index(5); // no tabs, should not panic
        assert_eq!(tv.current_index(), 0);
    }
}
