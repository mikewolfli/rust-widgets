// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Standalone TabBar widget — decoupled from TabWidget, draws a row/column of tabs.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::{expect_bool, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::container_widgets::tabwidget::{TabPosition, TabShape};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

const TAB_HEIGHT: i32 = 24;
const TAB_MIN_WIDTH: u32 = 40;
const TAB_MAX_WIDTH: u32 = 200;
const TAB_SPACING: i32 = 2;
const CLOSE_SIZE: i32 = 12;
const CLOSE_PADDING: i32 = 5;

/// Font size every tab label is drawn at, in points (see [`TabBar::draw_tab`]).
const TAB_FONT_SIZE: f32 = 14.0;

/// Space reserved either side of a tab's label inside its own box.
///
/// The label starts [`TAB_TEXT_INSET`] from the tab's left edge and the same distance is
/// kept free on the right; the width reservation has to cover both, or the last glyph
/// would sit against the border.
const TAB_TEXT_INSET: u32 = 6;
const TAB_TEXT_PADDING: u32 = TAB_TEXT_INSET * 2;

/// The advance the renderer will give a label before the first `draw` has measured it.
///
/// Mirrors `RenderContext`'s own clustered-advance heuristic — one cluster per `char`,
/// each advancing by the font size — so layout and painting agree even on the very first
/// frame. Counting bytes here instead is what made a CJK title measure four times its
/// drawn width.
fn estimate_text_width(text: &str, font_size: f32) -> u32 {
    (text.chars().count() as f32 * font_size).round() as u32
}

/// A single tab in a `TabBar`.
pub struct TabBarTab {
    title: String,
    icon: Option<Image>,
    tooltip: String,
    enabled: bool,
}

impl TabBarTab {
    /// Creates a new tab with the given title.
    pub fn new(title: String) -> Self {
        Self { title, icon: None, tooltip: String::new(), enabled: true }
    }

    /// Returns the tab title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the tab title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Returns the tab icon, if any.
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }

    /// Sets the tab icon.
    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon;
    }

    /// Returns the tab tooltip.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }

    /// Sets the tab tooltip.
    pub fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }

    /// Returns whether the tab is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sets the enabled state of the tab.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// Standalone tab bar widget that displays a row (or column) of tabs.
pub struct TabBar {
    base: BaseWidget,
    tabs: Vec<TabBarTab>,
    current_index: Option<usize>,
    hovered_index: Option<usize>,
    tab_position: TabPosition,
    tab_shape: TabShape,
    closable: bool,
    movable: bool,
    /// Index of the tab a drag started on, when a move gesture is in progress.
    ///
    /// Tracked so a release can complete a move that began with a press: without
    /// it a drag has no origin, and `tab_moved` has nothing to report.
    dragging_from: Option<usize>,
    tab_min_width: u32,
    tab_max_width: u32,
    /// Per-tab label widths, measured by the renderer on the last `draw`.
    ///
    /// `RenderContext` is the only component that knows the font metrics the backend
    /// will actually use, so the width a tab reserves has to come from there rather
    /// than from a private estimate. Caching it in `draw` (the same shape `GroupBox`
    /// uses for its title) makes hit testing, layout and painting agree *after* the
    /// first frame, and [`estimate_text_width`] only has to carry the first one.
    measured_title_widths: Vec<u32>,
    /// Emitted when the current tab index changes.
    pub current_changed: Signal1<usize>,
    /// Emitted when a tab close is requested (closable tabs only).
    pub tab_close_requested: Signal1<usize>,
    /// Emitted when a tab is moved; payload is `(old_index, new_index)`.
    pub tab_moved: Signal1<(usize, usize)>,
}

impl TabBar {
    /// Creates a new `TabBar` with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TabBar, geometry, "TabBar"),
            tabs: Vec::new(),
            current_index: None,
            hovered_index: None,
            tab_position: TabPosition::North,
            tab_shape: TabShape::Rounded,
            closable: false,
            movable: false,
            dragging_from: None,
            tab_min_width: TAB_MIN_WIDTH,
            tab_max_width: TAB_MAX_WIDTH,
            measured_title_widths: Vec::new(),
            current_changed: Signal1::new(),
            tab_close_requested: Signal1::new(),
            tab_moved: Signal1::new(),
        }
    }

    // ---------------------------------------------------------------------------
    // Tab management
    // ---------------------------------------------------------------------------

    /// Adds a tab with the given title and returns the index of the new tab.
    pub fn add_tab(&mut self, title: String) -> usize {
        self.tabs.push(TabBarTab::new(title));
        let idx = self.tabs.len().saturating_sub(1);
        if self.current_index.is_none() {
            self.current_index = Some(idx);
        }
        idx
    }

    /// Inserts a tab at the given index.
    pub fn insert_tab(&mut self, index: usize, title: String) {
        let idx = index.min(self.tabs.len());
        self.tabs.insert(idx, TabBarTab::new(title));
        // Adjust current_index if needed.
        if let Some(cur) = self.current_index {
            if cur >= idx {
                self.current_index = Some(cur + 1);
            }
        } else {
            self.current_index = Some(idx);
        }
    }

    /// Removes the tab at the given index.
    pub fn remove_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        // Adjust current_index.
        match self.current_index {
            Some(cur) if cur == index => {
                if self.tabs.is_empty() {
                    self.current_index = None;
                } else if cur >= self.tabs.len() {
                    self.current_index = Some(self.tabs.len() - 1);
                }
                // Otherwise stays the same (next tab slides into same position).
            }
            Some(cur) if cur > index => {
                self.current_index = Some(cur - 1);
            }
            // No action needed for this transition
            _ => {}
        }
    }

    /// Removes all tabs.
    pub fn clear(&mut self) {
        self.tabs.clear();
        self.current_index = None;
        self.hovered_index = None;
    }

    /// Returns the number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Returns the number of tabs.
    pub fn count(&self) -> usize {
        self.tabs.len()
    }

    /// Returns `true` if there are no tabs.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    // ---------------------------------------------------------------------------
    // Tab text
    // ---------------------------------------------------------------------------

    /// Returns the text of the tab at the given index.
    pub fn tab_text(&self, index: usize) -> Option<&str> {
        self.tabs.get(index).map(|t| t.title.as_str())
    }

    /// Sets the text of the tab at the given index.
    pub fn set_tab_text(&mut self, index: usize, text: String) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.title = text;
        }
    }

    // ---------------------------------------------------------------------------
    // Tab enabled state
    // ---------------------------------------------------------------------------

    /// Returns whether the tab at the given index is enabled.
    pub fn tab_enabled(&self, index: usize) -> Option<bool> {
        self.tabs.get(index).map(|t| t.enabled)
    }

    /// Sets the enabled state of the tab at the given index.
    pub fn set_tab_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.enabled = enabled;
        }
    }

    // ---------------------------------------------------------------------------
    // Current index
    // ---------------------------------------------------------------------------

    /// Returns the current tab index.
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Sets the current tab index.
    ///
    /// An index outside the tab list is ignored; use [`Self::clear_current_index`]
    /// to deliberately select nothing.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.tabs.len() {
            let changed = self.current_index != Some(index);
            self.current_index = Some(index);
            if changed {
                self.current_changed.emit(index);
                self.base.request_redraw();
            }
        }
    }

    /// Clears the selection so no tab is current.
    ///
    /// The property contract needs this: `current_index` reads as `Null` when
    /// nothing is selected, so a writer must be able to restore that state or the
    /// read → write → read round-trip would not close.
    pub fn clear_current_index(&mut self) {
        if self.current_index.take().is_some() {
            self.base.changed.emit();
            self.base.request_redraw();
        }
    }

    // ---------------------------------------------------------------------------
    // Tab position
    // ---------------------------------------------------------------------------

    /// Returns the tab position.
    pub fn tab_position(&self) -> TabPosition {
        self.tab_position
    }

    /// Sets the tab position. Requests redraw if the position changes.
    pub fn set_tab_position(&mut self, position: TabPosition) {
        if self.tab_position != position {
            self.tab_position = position;
            self.base.request_redraw();
        }
    }

    // ---------------------------------------------------------------------------
    // Tab shape
    // ---------------------------------------------------------------------------

    /// Returns the tab shape.
    pub fn tab_shape(&self) -> TabShape {
        self.tab_shape
    }

    /// Sets the tab shape. Requests redraw.
    pub fn set_tab_shape(&mut self, shape: TabShape) {
        if self.tab_shape != shape {
            self.tab_shape = shape;
            self.base.request_redraw();
        }
    }

    // ---------------------------------------------------------------------------
    // Closable / Movable
    // ---------------------------------------------------------------------------

    /// Returns whether tabs are closable.
    pub fn closable(&self) -> bool {
        self.closable
    }

    /// Sets whether tabs should show a close button.
    pub fn set_closable(&mut self, closable: bool) {
        self.closable = closable;
    }

    /// Returns whether tabs are movable.
    pub fn movable(&self) -> bool {
        self.movable
    }

    /// Sets whether tabs can be moved via drag-and-drop.
    pub fn set_movable(&mut self, movable: bool) {
        self.movable = movable;
    }

    /// Moves the tab at `from` so that it sits at `to`, and emits
    /// [`Self::tab_moved`] with `(from, to)`.
    ///
    /// # Why this is the only way to reorder
    ///
    /// `tab_moved` was declared but nothing emitted it, so a caller could see the
    /// signal, connect a handler, and wait forever. The fix is not to emit from
    /// somewhere convenient but to give reordering a real entry point: this is that
    /// entry point, and the drag gesture calls it.
    ///
    /// # Why `to` is clamped rather than rejected
    ///
    /// A drag reports positions from pointer coordinates, which can land one past
    /// the last tab. Clamping to `tabs.len() - 1` treats "dropped past the end" as
    /// "moved to the end" — the intent — while an out-of-range `from` is a caller
    /// bug and is rejected without emitting.
    ///
    /// Returns `true` when a move happened.
    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tabs.len() {
            return false;
        }
        let to = to.min(self.tabs.len() - 1);
        if to == from {
            return false;
        }

        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);

        // The selection follows the *tab*, not the slot: dragging the current tab
        // must not silently move the selection to whichever tab took its index.
        if let Some(current) = self.current_index {
            self.current_index = Some(match current {
                c if c == from => to,
                c if from < c && c <= to => c - 1,
                c if to <= c && c < from => c + 1,
                c => c,
            });
        }

        self.tab_moved.emit((from, to));
        true
    }

    // ---------------------------------------------------------------------------
    // Tab dimensions
    // ---------------------------------------------------------------------------

    /// Returns the minimum tab width.
    pub fn tab_min_width(&self) -> u32 {
        self.tab_min_width
    }

    /// Sets the minimum tab width.
    ///
    /// Raises the maximum to match if the new minimum would exceed it, mirroring
    /// [`Self::set_tab_max_width`]. Without that the pair could be left inverted, and
    /// [`Self::compute_tab_width`]`s `clamp(min, max)` **panics** on `min > max` — reaching it via
    /// `set_tab_min_width(5000)` on a bar whose maximum was still the 200 default. The two setters
    /// were asymmetric: `set_tab_max_width` already pulled the maximum up to the minimum, this one
    /// left the maximum behind.
    pub fn set_tab_min_width(&mut self, width: u32) {
        self.tab_min_width = width.max(1);
        self.tab_max_width = self.tab_max_width.max(self.tab_min_width);
    }

    /// Returns the maximum tab width.
    pub fn tab_max_width(&self) -> u32 {
        self.tab_max_width
    }

    /// Sets the maximum tab width.
    ///
    /// Floors at the current minimum, so the pair the clamp is applied to is never inverted.
    pub fn set_tab_max_width(&mut self, width: u32) {
        self.tab_max_width = width.max(self.tab_min_width);
    }

    // ---------------------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------------------

    /// Returns the rect for the tab at the given index.
    fn tab_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() {
            return None;
        }
        let rect = self.base.geometry();
        let spacing = TAB_SPACING;
        match self.tab_position {
            TabPosition::North | TabPosition::South => {
                let tab_width = self.compute_tab_width(index);
                let x = rect.x + (tab_width as i32 + spacing) * index as i32;
                let y = if self.tab_position == TabPosition::North {
                    rect.y
                } else {
                    rect.y + rect.height as i32 - TAB_HEIGHT
                };
                Some(Rect::new(x, y, tab_width, TAB_HEIGHT as u32))
            }
            TabPosition::West | TabPosition::East => {
                let tab_height = TAB_HEIGHT as u32;
                let y = rect.y + (TAB_HEIGHT + spacing) * index as i32;
                let x = if self.tab_position == TabPosition::West {
                    rect.x
                } else {
                    rect.x + rect.width as i32 - TAB_HEIGHT
                };
                Some(Rect::new(x, y, TAB_HEIGHT as u32, tab_height))
            }
        }
    }

    /// Computes the width of a tab, taking into account whether a close button
    /// is rendered and clamping to the configured min/max.
    ///
    /// # Why the estimate cannot be "characters x 8"
    ///
    /// The estimate used to be `text.len() * 8`, which is wrong twice over: `len()`
    /// counts **bytes**, so a two-character CJK title measured 48 px; and the advance
    /// it should predict is the one the *renderer* will use, which is 12 px per glyph
    /// (`Font::simple("Arial", 14.0)` in [`Self::draw_tab`]). A title that measured
    /// wider here than it drew there left the label running past its own tab; one that
    /// measured narrower wasted the tab.
    ///
    /// The fallback therefore mirrors `RenderContext`'s own heuristic — one cluster per
    /// `char`, each advancing by the font size — rather than inventing a second one. It
    /// is only reached before the first `draw`, because `draw_tab` records the measured
    /// width in [`Self::measured_title_widths`] from then on.
    fn compute_tab_width(&self, index: usize) -> u32 {
        let text = self.tabs[index].title.as_str();
        let measured = self.measured_title_widths.get(index).copied();
        let text_width = measured.unwrap_or_else(|| estimate_text_width(text, TAB_FONT_SIZE));
        let mut w = text_width + TAB_TEXT_PADDING; // horizontal padding
        if self.closable {
            w += (CLOSE_SIZE + CLOSE_PADDING) as u32;
        }
        w.clamp(self.tab_min_width, self.tab_max_width)
    }

    /// Roughly measure text width (fallback if no RenderContext handy).
    /// The index of the tab at the given point, or None.
    fn tab_at_position(&self, pos: Point) -> Option<usize> {
        for i in 0..self.tabs.len() {
            if let Some(r) = self.tab_rect(i) {
                if r.contains(pos) {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Returns the close-button rect for the tab at the given index, if closable.
    fn close_rect(&self, index: usize) -> Option<Rect> {
        if !self.closable {
            return None;
        }
        self.tab_rect(index).map(|r| {
            let cx = r.x + r.width as i32 - CLOSE_SIZE - CLOSE_PADDING;
            let cy = r.y + (r.height as i32 - CLOSE_SIZE) / 2;
            Rect::new(cx, cy, CLOSE_SIZE as u32, CLOSE_SIZE as u32)
        })
    }

    /// Draws a single tab with the given shape.
    fn draw_tab(&self, context: &mut RenderContext, index: usize, tab_rect: Rect) {
        let tab = &self.tabs[index];
        let is_current = self.current_index == Some(index);
        let is_hovered = self.hovered_index == Some(index);
        let is_enabled = tab.enabled;

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("tab_bar");
        // `tab_bar` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The current tab's fill is therefore a step toward
        // the foreground, so the strip reads as a surface of its own.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(255, 255, 255));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgb(180, 180, 180));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let current_tab = resolved.blend(&text_color, 0.08);
        // Tab chrome is derived from that pair: a disabled tab is pressed toward the
        // foreground, an inactive one less so, and a hovered tab reads as a light
        // tint of the selection colour.
        let disabled_tab = current_tab.blend(&text_color, 0.14);
        let inactive_tab = current_tab.blend(&text_color, 0.06);
        let hovered_tab = current_tab.blend(
            &crate::style::resolved_theme_style("button")
                .and_then(|button| button.background_color)
                .unwrap_or(text_color),
            0.08,
        );

        // Background color.
        let bg = if !is_enabled {
            disabled_tab
        } else if is_current {
            current_tab
        } else if is_hovered {
            hovered_tab
        } else {
            inactive_tab
        };

        // Border color.
        let border =
            if !is_enabled || is_current { border } else { border.blend(&inactive_tab, 0.5) };

        // Draw tab shape.
        match self.tab_shape {
            TabShape::Rounded => {
                // For simplicity, fill the rect then draw a border.
                context.fill_rect(tab_rect, bg);
                context.draw_rect(tab_rect, border);
                // Top-left and top-right rounded corner hints (visual only via fill).
                if is_current {
                    // Overdraw bottom edge so it blends with content area.
                    let bottom_rect = Rect::new(
                        tab_rect.x,
                        tab_rect.y + tab_rect.height as i32 - 1,
                        tab_rect.width,
                        2,
                    );
                    context.fill_rect(bottom_rect, bg);
                }
            }
            TabShape::Triangular => {
                // Draw a trapezoid-ish shape: fill a rect then clip top corners.
                context.fill_rect(tab_rect, bg);
                context.draw_rect(tab_rect, border);
            }
            TabShape::Rectangular => {
                context.fill_rect(tab_rect, bg);
                context.draw_rect(tab_rect, border);
            }
        }

        // Text color. A disabled tab's label is muted toward its own fill rather than
        // the previous literal grey.
        let text_color =
            if !is_enabled { text_color.blend(&disabled_tab, 0.5) } else { text_color };

        // Draw tab title. The origin is the glyph's **top** edge, so the vertical centre is
        // half the difference between the tab and the line box. Passing the tab's midline
        // (which is what `tab_rect.height / 2` is) put the glyph box's *top* at the centre,
        // so a 14 px label in a 24 px tab spanned 12..26 and crossed the tab's bottom edge.
        let text_x = tab_rect.x + TAB_TEXT_INSET as i32;
        let title_height = context.measure_text("M", &Font::simple("Arial", TAB_FONT_SIZE)).height;
        let text_y = tab_rect.y + (tab_rect.height as i32 - title_height as i32) / 2;
        context.draw_text(
            Point::new(text_x, text_y),
            &tab.title,
            &Font::simple("Arial", TAB_FONT_SIZE),
            text_color,
            HorizontalAlignment::Left,
        );

        // Draw close button if closable.
        if let Some(close_rect) = self.close_rect(index) {
            // Secondary chrome: a tint of the resolved foreground, muted further when
            // the tab is disabled.
            let close_color = if !is_enabled {
                text_color.blend(&disabled_tab, 0.4)
            } else {
                text_color.blend(&current_tab, 0.4)
            };
            context.draw_line(
                Point::new(close_rect.x, close_rect.y),
                Point::new(
                    close_rect.x + close_rect.width as i32,
                    close_rect.y + close_rect.height as i32,
                ),
                close_color,
            );
            context.draw_line(
                Point::new(close_rect.x + close_rect.width as i32, close_rect.y),
                Point::new(close_rect.x, close_rect.y + close_rect.height as i32),
                close_color,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Widget trait
// ---------------------------------------------------------------------------
impl Widget for TabBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(400, 30)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `TabBar`'s property contract.
///
/// `tab_count` is derived from the tab vector and `current_index` is optional,
/// so the read path publishes `Null` for "no tab selected" while the write path
/// narrows through `expect_usize` before storing a `usize`.
impl WidgetProperties for TabBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "tab_count" => Ok(CapabilityValue::UInt(self.tab_count() as u64)),
            "current_index" => match self.current_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "closable" => Ok(CapabilityValue::Bool(self.closable())),
            "movable" => Ok(CapabilityValue::Bool(self.movable())),
            "tab_min_width" => Ok(CapabilityValue::UInt(self.tab_min_width() as u64)),
            "tab_max_width" => Ok(CapabilityValue::UInt(self.tab_max_width() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => match value {
                // `Null` mirrors what `get` publishes when no tab is selected, so
                // the two directions agree on the meaning of an absent index.
                CapabilityValue::Null => {
                    self.clear_current_index();
                    Ok(())
                }
                other => {
                    self.set_current_index(expect_usize(other)?);
                    Ok(())
                }
            },
            "closable" => {
                self.set_closable(expect_bool(value)?);
                Ok(())
            }
            "movable" => {
                self.set_movable(expect_bool(value)?);
                Ok(())
            }
            "tab_min_width" => {
                self.set_tab_min_width(expect_usize(value)? as u32);
                Ok(())
            }
            "tab_max_width" => {
                self.set_tab_max_width(expect_usize(value)? as u32);
                Ok(())
            }
            "tab_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "tab_count",
            "current_index",
            "closable",
            "movable",
            "tab_min_width",
            "tab_max_width",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `tab_bar` publishes.
    ///
    /// `add_tab` takes the new tab's title and `remove_tab` takes the index to
    /// remove, so neither can complete without a payload: they are refused as
    /// [`CapabilityAccessError::OutOfRange`], meaning the name is valid and the
    /// argument is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_tab" | "remove_tab" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

// ---------------------------------------------------------------------------
// EventHandler trait
// ---------------------------------------------------------------------------
impl EventHandler for TabBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let prev = self.hovered_index;
                self.hovered_index = self.tab_at_position(*pos);
                if prev != self.hovered_index {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button } if *button == 1 => {
                // Check close button first.
                if self.closable {
                    for i in 0..self.tabs.len() {
                        if let Some(close_rect) = self.close_rect(i) {
                            if close_rect.contains(*pos) && self.tabs[i].enabled {
                                self.tab_close_requested.emit(i);
                                return;
                            }
                        }
                    }
                }
                // Otherwise, select tab.
                if let Some(index) = self.tab_at_position(*pos) {
                    if self.tabs[index].enabled {
                        self.set_current_index(index);
                        // A press on a movable tab arms a drag; the release decides
                        // whether it moved. Arming on press rather than on the first
                        // move means a click that happens to jitter by a pixel still
                        // behaves as a click.
                        if self.movable {
                            self.dragging_from = Some(index);
                        }
                    }
                }
            }
            Event::MouseRelease { pos, .. } => {
                // Completing the move on release, and only on release, is what makes
                // `movable` mean "drag to reorder" rather than "reorder on click".
                if let Some(from) = self.dragging_from.take() {
                    if let Some(to) = self.tab_at_position(*pos) {
                        self.move_tab(from, to);
                    }
                }
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                // Check close button first.
                if self.closable {
                    for i in 0..self.tabs.len() {
                        if let Some(close_rect) = self.close_rect(i) {
                            if close_rect.contains(*pos) && self.tabs[i].enabled {
                                self.tab_close_requested.emit(i);
                                return;
                            }
                        }
                    }
                }
                // Otherwise, select tab.
                if let Some(index) = self.tab_at_position(*pos) {
                    if self.tabs[index].enabled {
                        self.set_current_index(index);
                    }
                }
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Draw trait
// ---------------------------------------------------------------------------
impl Draw for TabBar {
    fn draw(&mut self, context: &mut RenderContext) {
        // Measure every label once, up front, for the whole strip. A tab's width depends
        // on its own label, so measuring while drawing would leave tab `n` laid out from
        // the estimate and tab `n+1` from a real metric — a strip whose later tabs are a
        // different size from its first.
        self.measured_title_widths.clear();
        for tab in &self.tabs {
            let width =
                context.measure_text(&tab.title, &Font::simple("Arial", TAB_FONT_SIZE)).width;
            self.measured_title_widths.push(width);
        }
        for i in 0..self.tabs.len() {
            if let Some(tr) = self.tab_rect(i) {
                self.draw_tab(context, i, tr);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn tabbar_creation_defaults() {
        let tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert!(tb.is_empty());
        assert_eq!(tb.count(), 0);
        assert_eq!(tb.current_index(), None);
        assert!(!tb.closable());
        assert!(!tb.movable());
        assert_eq!(tb.tab_min_width(), 40);
        assert_eq!(tb.tab_max_width(), 200);
    }

    #[test]
    fn tabbar_add_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        let idx = tb.add_tab("Tab 1".to_string());
        assert_eq!(idx, 0);
        assert_eq!(tb.count(), 1);
        assert_eq!(tb.current_index(), Some(0));
        assert_eq!(tb.tab_text(0), Some("Tab 1"));
    }

    #[test]
    fn tabbar_multiple_tabs() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("First".to_string());
        tb.add_tab("Second".to_string());
        tb.add_tab("Third".to_string());
        assert_eq!(tb.count(), 3);
        assert_eq!(tb.tab_text(0), Some("First"));
        assert_eq!(tb.tab_text(1), Some("Second"));
        assert_eq!(tb.tab_text(2), Some("Third"));
    }

    #[test]
    fn tabbar_insert_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("C".to_string());
        tb.insert_tab(1, "B".to_string());
        assert_eq!(tb.count(), 3);
        assert_eq!(tb.tab_text(1), Some("B"));
    }

    #[test]
    fn tabbar_remove_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("B".to_string());
        tb.add_tab("C".to_string());
        tb.remove_tab(1);
        assert_eq!(tb.count(), 2);
        assert_eq!(tb.tab_text(1), Some("C"));
    }

    #[test]
    fn tabbar_clear() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("B".to_string());
        tb.clear();
        assert!(tb.is_empty());
        assert_eq!(tb.current_index(), None);
    }

    #[test]
    fn tabbar_set_current_index() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("B".to_string());
        tb.set_current_index(1);
        assert_eq!(tb.current_index(), Some(1));
        tb.set_current_index(0);
        assert_eq!(tb.current_index(), Some(0));
    }

    #[test]
    fn tabbar_set_current_index_requests_redraw() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("B".to_string());

        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        tb.base.redraw_requested.connect({
            let flag = std::sync::Arc::clone(&fired);
            move || flag.store(true, std::sync::atomic::Ordering::SeqCst)
        });

        tb.set_current_index(1);

        assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn tabbar_set_tab_text() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("Old".to_string());
        tb.set_tab_text(0, "New".to_string());
        assert_eq!(tb.tab_text(0), Some("New"));
    }

    #[test]
    fn tabbar_tab_enabled() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("Tab".to_string());
        assert_eq!(tb.tab_enabled(0), Some(true));
        tb.set_tab_enabled(0, false);
        assert_eq!(tb.tab_enabled(0), Some(false));
    }

    #[test]
    fn tabbar_closable_movable() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert!(!tb.closable());
        tb.set_closable(true);
        assert!(tb.closable());
        assert!(!tb.movable());
        tb.set_movable(true);
        assert!(tb.movable());
    }

    #[test]
    fn tabbar_tab_position_shape() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert_eq!(tb.tab_position(), TabPosition::North);
        tb.set_tab_position(TabPosition::South);
        assert_eq!(tb.tab_position(), TabPosition::South);
        assert_eq!(tb.tab_shape(), TabShape::Rounded);
        tb.set_tab_shape(TabShape::Rectangular);
        assert_eq!(tb.tab_shape(), TabShape::Rectangular);
    }

    #[test]
    fn tabbar_min_max_width() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.set_tab_min_width(50);
        assert_eq!(tb.tab_min_width(), 50);
        tb.set_tab_max_width(100);
        assert_eq!(tb.tab_max_width(), 100);
    }

    #[test]
    fn tabbar_signal_accessors() {
        let tb = TabBar::new(Rect::new(0, 0, 400, 24));
        let _ = &tb.current_changed;
        let _ = &tb.tab_close_requested;
        let _ = &tb.tab_moved;
    }

    #[test]
    fn tabbar_geometry_delegation() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.set_geometry(Rect::new(10, 10, 500, 30));
        assert_eq!(tb.geometry(), Rect::new(10, 10, 500, 30));
    }

    #[test]
    fn tabbar_visibility() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert!(tb.is_visible());
        tb.hide();
        assert!(!tb.is_visible());
        tb.show();
        assert!(tb.is_visible());
    }

    #[test]
    fn tabbar_id_kind() {
        let a = TabBar::new(Rect::new(0, 0, 400, 24));
        let b = TabBar::new(Rect::new(0, 0, 400, 24));
        assert_ne!(a.id(), b.id());
        assert_eq!(a.kind(), WidgetKind::TabBar);
    }

    #[test]
    fn tabbar_draw_produces_svg() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("One".to_string());
        tb.add_tab("Two".to_string());
        tb.set_current_index(0);
        let svg = crate::widget::svg::render_to_svg(&mut tb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.len() > 100);
    }

    /// Regression: `set_tab_min_width` used to leave `tab_max_width` behind, so a minimum above
    /// the default maximum produced an inverted pair and `compute_tab_width`'s `clamp(min, max)`
    /// **panicked**. Both setters must keep `min <= max`.
    #[test]
    fn tabbar_min_width_above_max_does_not_invert_pair() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert!(tb.tab_min_width() <= tb.tab_max_width());

        tb.set_tab_min_width(5000);
        assert_eq!(tb.tab_min_width(), 5000);
        assert!(
            tb.tab_min_width() <= tb.tab_max_width(),
            "min {} must not exceed max {}",
            tb.tab_min_width(),
            tb.tab_max_width()
        );
        assert_eq!(tb.tab_max_width(), 5000, "maximum must rise to meet the new minimum");

        // The clamp itself must not panic, and neither must a full render.
        tb.add_tab("A very long tab title that would exceed the default maximum".to_string());
        assert!(tb.tab_rect(0).is_some());
        let svg = crate::widget::svg::render_to_svg(&mut tb);
        assert!(svg.starts_with("<svg"));
    }

    /// The mirror case: an inverted *maximum* must be pulled back up to the minimum, not left to
    /// poison the clamp.
    #[test]
    fn tabbar_max_width_below_min_does_not_invert_pair() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.set_tab_min_width(120);
        tb.set_tab_max_width(10);
        assert!(tb.tab_min_width() <= tb.tab_max_width());
        assert_eq!(tb.tab_max_width(), 120);
        tb.add_tab("X".to_string());
        assert!(tb.tab_rect(0).is_some());
    }

    #[test]
    fn tabbar_mouse_click_selects_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("First".to_string());
        tb.add_tab("Second".to_string());
        // Click on the first tab area
        tb.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1 });
        assert_eq!(tb.current_index(), Some(0));
        // Hover sets hovered_index
        tb.handle_event(&Event::MouseMove { pos: Point::new(5, 5) });
    }
}
