// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TabView widget — iOS-style segmented tab page view.
//!
//! Displays a horizontal segmented tab bar at the top and a content area
//! below showing the selected tab's content. Supports add/remove/clear
//! operations on tabs and emits a `tab_changed` signal on selection.

use crate::core::{Color, Font, HorizontalAlignment, Rect};
use crate::event::{Event, EventHandler};
#[cfg(full_widgets)]
use crate::layout::{
    AlignItems, FlexDirection, FlexLayout, FlexWrap, JustifyContent, LayoutParams,
};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::estimate_text_width;
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

    /// Each tab's own width, derived from its own label.
    ///
    /// # The defect this replaces
    ///
    /// The strip used to be divided **equally**: `tab_width = rect.width / tab_count`. That
    /// makes a tab's width a fact about its *neighbours' count* rather than about its own
    /// label, with two visible consequences. A short title got a cell far wider than it needed,
    /// and a long one got the same cell as every other — so it was elided by
    /// `draw_text_fitted` even when the strip had room to spare. Adding a fourth tab also
    /// silently narrowed the first three.
    ///
    /// A tab's width is `label_width + TAB_TEXT_PADDING`, clamped into
    /// `TAB_MIN_WIDTH..TAB_MAX_WIDTH` so a one-character tab is still tappable and a long one
    /// does not crowd out its siblings. This is the same derivation `TabWidget::tab_widths`
    /// uses, with the same constants, so the two tab strips in this crate cannot disagree.
    fn tab_widths(&self) -> crate::compat::Vec<i32> {
        let font = Font::default();
        self.tabs
            .iter()
            .map(|tab| {
                // The label's width comes from the shared estimate -- the same model the renderer
                // draws with -- rather than from `chars().count() * TAB_CHAR_WIDTH`. The hand-rolled
                // form was a second copy of the advance arithmetic and one that counts *clusters*
                // at a fixed 8 px: a CJK caption measured half its drawn width, so a Chinese tab
                // label overflowed the strip its own width had reserved.
                (estimate_text_width(&tab.title, &font, 1.0) as i32 + TAB_TEXT_PADDING)
                    .clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH)
            })
            .collect()
    }

    /// The tab boxes, in strip order, positioned by a layout rather than by an accumulator.
    ///
    /// # Why the run is assembled and not summed
    ///
    /// The draw path used to compute each tab's x as `rect.x + i * tab_width`, and the hit test
    /// re-derived the same expression from a separate bookkeeping run — two accumulators of one
    /// fact. Handing the measured widths to a [`FlexLayout`] through
    /// [`CompositeBuilder`](crate::widget::composite::CompositeBuilder) makes the run the
    /// *layout's* answer, so the paint, the hit test and any future caller read one result. It
    /// also means the strip picks up the layout's device scaling for free.
    ///
    /// The returned boxes are in **strip coordinates** (origin at the strip's own top-left), so
    /// the caller has one translation to apply and the geometry is independent of where the
    /// control was placed.
    fn tab_run(&self, strip: Rect) -> crate::compat::Vec<Rect> {
        let widths = self.tab_widths();
        if widths.is_empty() {
            return crate::compat::Vec::new();
        }
        let height = strip.height;
        let total: i32 = widths.iter().sum();
        // # Why the stripped profiles take the direct route
        //
        // `full_widgets` is "a device profile *and* an unstripped widget set" (principle #47),
        // and a `mobile-api` build has no `WidgetFactory` here. Both arms read the same widths,
        // so the fallback is the same run written the only way that profile can express it.
        #[cfg(not(full_widgets))]
        {
            let mut placed: crate::compat::Vec<Rect> = crate::compat::Vec::new();
            let mut cursor = 0i32;
            for width in widths.iter() {
                placed.push(Rect::new(cursor, 0, *width as u32, height));
                cursor += *width + TAB_SPACING;
            }
            return placed;
        }
        #[cfg(full_widgets)]
        {
            use crate::compat::Box as _Box;
            let run =
                Rect::new(0, 0, (total + TAB_SPACING * widths.len() as i32).max(0) as u32, height);
            let factory = crate::widget::WidgetFactory::new_with_defaults();
            let mut row = crate::widget::composite::CompositeBuilder::new(
                _Box::new(FlexLayout::with_params(
                    FlexDirection::Row,
                    FlexWrap::NoWrap,
                    JustifyContent::FlexStart,
                    AlignItems::Stretch,
                    TAB_SPACING,
                    0,
                )),
                EdgeOffsets::all(0),
                crate::core::Size::new(0, 0),
            );
            for (index, tab) in self.tabs.iter().enumerate() {
                let along = widths.get(index).copied().unwrap_or(TAB_MIN_WIDTH) as u32;
                let created = row.add_sized(
                    &factory,
                    "label",
                    &tab.title,
                    crate::core::Size::new(along, height),
                    LayoutParams::new(),
                );
                debug_assert!(created.is_some(), "a tab is a core control");
            }
            let mut placed: crate::compat::Vec<Rect> = crate::compat::Vec::new();
            row.arrange(run, &mut |_, rect| placed.push(rect));
            while placed.len() < self.tabs.len() {
                placed.push(Rect::new(0, 0, 0, 0));
            }
            placed
        }
    }
}

/// Padding added to a tab's measured label width.
pub(crate) const TAB_TEXT_PADDING: i32 = 24;
/// Narrowest a tab may be drawn, so a one-character caption is still a tap target.
pub(crate) const TAB_MIN_WIDTH: i32 = 40;
/// Widest a tab may be drawn, so one long caption cannot crowd out its siblings.
pub(crate) const TAB_MAX_WIDTH: i32 = 200;
/// Gap between adjacent tabs.
pub(crate) const TAB_SPACING: i32 = 2;

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
        //
        // The boxes come from `tab_run`, the *same* derivation the hit test reads, so the tab a
        // press selects is the tab that was painted. They are in strip coordinates, so one
        // translation places them.
        let font = Font::simple("sans-serif", 12.0);
        for (i, local) in self.tab_run(tab_bar_rect).into_iter().enumerate() {
            let tab_rect = Rect::new(
                tab_bar_rect.x + local.x,
                tab_bar_rect.y + local.y,
                local.width,
                local.height,
            );
            let is_selected = i == self.selected_index;

            // Background
            let bg_color = if is_selected { content_background } else { inactive_tab };
            context.fill_rect(tab_rect, bg_color);

            // Selected tab indicator line
            if is_selected {
                let indicator_rect = Rect::new(
                    tab_rect.x,
                    tab_rect.y + tab_rect.height as i32 - 3,
                    tab_rect.width,
                    3,
                );
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
                    // The band comes from the control's own derivation, so the region that accepts
                    // a press is by construction the region the renderer drew.
                    let strip = self.tab_bar_rect();
                    if pos.y >= strip.y && pos.y < strip.y + strip.height as i32 {
                        // The tab boxes are the layout's answer, the *same* boxes the draw pass
                        // painted. Deriving the index from `relative_x / tab_width` (the old
                        // form) meant the hit test divided the strip equally while the paint
                        // path used each tab's own width — so once tabs stopped being equal, a
                        // press on a tab selected its neighbour. Reading the boxes removes the
                        // second derivation rather than trying to keep the two in step.
                        let local_x = pos.x - strip.x;
                        let clicked = self.tab_run(strip).into_iter().position(|box_rect| {
                            let left = box_rect.x;
                            let right = left + box_rect.width as i32;
                            local_x >= left && local_x < right
                        });
                        if let Some(index) = clicked {
                            self.set_current_index(index);
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

        // The boxes come from the control's own derivation, so the test asserts against the
        // *same* geometry the control painted rather than re-deriving a second expectation.
        // `tv.geometry().width / 2` used to stand here, and it pinned the equal division the
        // control has since stopped doing — a test that restated the implementation could not
        // notice the implementation was wrong.
        let strip = tv.tab_bar_rect();
        let boxes = tv.tab_run(strip);
        let runs = text_run_boxes(&svg);
        let caption_of = |index: usize| -> (i32, i32, i32, i32) {
            let local =
                boxes.get(index).copied().unwrap_or_else(|| panic!("tab {index} has no box"));
            let left = strip.x + local.x;
            let right = left + local.width as i32;
            runs.iter()
                .find(|(l, _, r, _)| {
                    let centre = (l + r) / 2;
                    centre >= left && centre < right
                })
                .copied()
                .unwrap_or_else(|| panic!("tab {index} painted no caption in {left}..{right}"))
        };
        let alpha = caption_of(0);
        let beta = caption_of(1);
        assert_eq!(alpha.1, expected_y, "a caption belongs on the strip's line box");
        assert_eq!(beta.1, expected_y, "and every caption shares it");
        // The caption is bounded by its own tab, which the attribute assertion could not see: a
        // run centred on the strip but not on its tab would still have had the right `y`.
        for (index, run) in [alpha, beta].into_iter().enumerate() {
            let local = boxes[index];
            let left = strip.x + local.x;
            let right = left + local.width as i32;
            assert!(
                run.0 >= left && run.2 <= right,
                "tab {index}: caption {run:?} must stay inside {left}..{right}"
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

        // The click point is derived from the second tab's *own* box rather than from the
        // old equal division (`x=150..299`). Tabs are sized by their captions now, so a
        // hardcoded coordinate would be asserting the geometry was divided equally — the
        // thing this control stopped doing — instead of asserting that a press on a tab
        // selects it.
        let strip = tv.tab_bar_rect();
        let boxes = tv.tab_run(strip);
        let second = boxes.get(1).copied().expect("two tabs were added");
        let centre = Point::new(
            strip.x + second.x + second.width as i32 / 2,
            strip.y + second.height as i32 / 2,
        );
        tv.handle_event(&Event::MousePress { pos: centre, button: 1 });
        assert_eq!(tv.current_index(), 1, "a press on the second tab selects it");

        // And the first tab's own box still selects the first, so the mapping is not merely
        // "anything selects index 1".
        let first = boxes.first().copied().expect("two tabs were added");
        let centre = Point::new(
            strip.x + first.x + first.width as i32 / 2,
            strip.y + first.height as i32 / 2,
        );
        tv.handle_event(&Event::MousePress { pos: centre, button: 1 });
        assert_eq!(tv.current_index(), 0, "a press on the first tab selects it");
    }

    #[test]
    fn a_tab_is_as_wide_as_its_own_caption_needs() {
        // The defect this pins: the strip used to be divided equally by the tab count, so a
        // short caption got a cell far wider than it needed and its neighbour's width changed
        // when a tab was added. A tab's width is now a fact about its own label.
        let mut tv = make_tab_view();
        tv.add_tab("I", None, None::<&str>);
        tv.add_tab("A much longer caption", None, None::<&str>);
        let boxes = tv.tab_run(tv.tab_bar_rect());

        let short = boxes[0].width;
        let long = boxes[1].width;
        assert!(long > short, "a longer caption must ask for a wider tab: {short} vs {long}");
        assert_eq!(short, TAB_MIN_WIDTH as u32, "a one-character tab floors at the minimum");
        // The expected width is read from the same estimate the control uses, over a caption long
        // enough to stay under the 200 px ceiling -- so this asserts the *measured* width, and the
        // ceiling is asserted separately below with a caption long enough to reach it.
        //
        // The literal this replaced (`21 * TAB_CHAR_WIDTH + TAB_TEXT_PADDING`) was the old
        // `chars().count() * 8` arithmetic restated in the test. That is a second copy of the
        // measurement, and it is exactly what went stale when the shared estimate took over: the
        // real advance for 21 Latin clusters at the default 14 px font is 21 x 8.4, not 21 x 8.
        let caption = "A much longer caption";
        assert_eq!(caption.chars().count(), 21, "the arithmetic below is stated for 21 clusters");
        let measured = estimate_text_width(caption, &Font::default(), 1.0);
        assert!(
            measured + TAB_TEXT_PADDING as u32 <= TAB_MAX_WIDTH as u32,
            "this caption must stay under the ceiling for the assertion to be about measurement"
        );
        assert_eq!(long, measured + TAB_TEXT_PADDING as u32);

        tv.add_tab("A caption far longer than any tab could ever need to be", None, None::<&str>);
        let boxes = tv.tab_run(tv.tab_bar_rect());
        assert_eq!(
            boxes[2].width, TAB_MAX_WIDTH as u32,
            "an over-long caption ceilings at the maximum so it cannot crowd out its siblings"
        );

        // Adding a third tab must not narrow the first two: under equal division this was
        // exactly the silent regression that made a long caption elide.
        assert_eq!(boxes[0].width, short, "an added sibling must not resize tab 0");
        assert_eq!(boxes[1].width, long, "nor tab 1");
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
