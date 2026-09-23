// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Standalone TabBar widget — decoupled from TabWidget, draws a row/column of tabs.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::{
    expect_bool, expect_text_direction, expect_usize, text_direction_to_str,
};
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
    /// The writing direction the horizontal strip runs in.
    ///
    /// # Why a tab strip needs this
    ///
    /// A tab strip is a *sequence*: "the first tab is where the strip begins" is a statement about
    /// the line's reading order, not about its geometry. In an Arabic or Hebrew interface the strip
    /// begins at the right, so tab 0 belongs on the right and the overflow grows leftward — otherwise
    /// the visual order of the tabs contradicts the order of the pages they select, and the keyboard
    /// arrows move along the strip the opposite way from the one the reader's eye follows.
    ///
    /// Only the horizontal positions are affected: `West`/`East` strips are *not* mirrored onto each
    /// other, because the side a strip is attached to is a layout decision rather than a reading one.
    /// Defaults to left-to-right, so a strip that never asks behaves exactly as it did.
    direction: crate::core::TextDirection,
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
            direction: crate::core::TextDirection::default(),
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

    /// Returns the writing direction the horizontal strip runs in.
    pub fn direction(&self) -> crate::core::TextDirection {
        self.direction
    }

    /// Sets the writing direction the horizontal strip runs in, and repaints.
    ///
    /// A right-to-left strip starts at its **right** edge, so tab 0 and the overflow both grow
    /// leftward — the visual order then matches the order of the pages the tabs select. The vertical
    /// positions are unaffected; see the field for why the side a strip is attached to is not a
    /// reading fact.
    pub fn set_direction(&mut self, direction: crate::core::TextDirection) {
        if self.direction != direction {
            self.direction = direction;
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
    ///
    /// # Why the width is scaled before the position is derived
    ///
    /// The position used to be `(tab_width + spacing) * index`, which never mentioned
    /// `rect.width`. With six tabs at 200 px on a 240 px strip the fifth began at x = 808 and the
    /// sixth at x = 1010 — both drawn entirely outside the control. The clamp below is the rule
    /// [`crate::widget::nav_widgets::tab_view`] already applies: when the tabs do not fit, they
    /// shrink to `available / count` and every tab is on screen. The *spacing* has to be inside
    /// the fit calculation for the same reason: `count` tabs need `count` widths plus
    /// `count - 1` gaps, and leaving the gaps out is how "they fit now" becomes false again one
    /// tab later.
    fn tab_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() {
            return None;
        }
        let rect = self.base.geometry();
        let spacing = TAB_SPACING;
        // The strip's *reading* order decides which end tab 0 is at, and the direction is applied to
        // the finished offset rather than to the index: `offset` is "how far into the strip this tab
        // begins in reading order", and the conversion to a left-edge distance is
        // `TextDirection`'s single job. Doing it the other way — reversing the index — would also
        // reverse the overflow, so the tabs would grow off the *wrong* end once they stopped fitting.
        let rtl = self.direction.is_right_to_left();
        match self.tab_position {
            TabPosition::North | TabPosition::South => {
                let tab_width = self.fitted_tab_width();
                let offset = (tab_width as i32 + spacing) * index as i32;
                let x =
                    if rtl { rect.right() - offset - tab_width as i32 } else { rect.x + offset };
                // The last tab must end *inside* the strip. `fitted_tab_width` divides by the count
                // and subtracts the whole gap budget, so a tab entirely on screen needs the whole
                // run to fit; the clamp still holds for a tab whose own measured width is larger
                // than its slot, and it is what keeps the assertion of that visible rather than
                // implicit.
                //
                // In RTL the tab's **leading** edge is on its right, so the room it may take is
                // measured from the strip's left edge — the mirror of the LTR clamp, not a second
                // rule.
                let width = if rtl {
                    tab_width.min((x + tab_width as i32 - rect.x).max(0) as u32)
                } else {
                    tab_width.min((rect.right() - x).max(0) as u32)
                };
                let y = if self.tab_position == TabPosition::North {
                    rect.y
                } else {
                    rect.y + rect.height as i32 - TAB_HEIGHT
                };
                Some(Rect::new(x, y, width, TAB_HEIGHT as u32))
            }
            TabPosition::West | TabPosition::East => {
                let tab_height = TAB_HEIGHT as u32;
                let y = rect.y + (TAB_HEIGHT + spacing) * index as i32;
                // The side a vertical strip is attached to is a layout choice, not a reading one, so
                // the direction does not move it: a `West` strip stays on the left in every locale,
                // and its tabs still run top-to-bottom.
                let x = if self.tab_position == TabPosition::West {
                    rect.x
                } else {
                    rect.x + rect.width as i32 - TAB_HEIGHT
                };
                Some(Rect::new(x, y, TAB_HEIGHT as u32, tab_height))
            }
        }
    }

    /// The width every tab is drawn at, after shrinking to fit the strip when it has to.
    ///
    /// # Why the fit is applied here rather than per tab
    ///
    /// [`Self::compute_tab_width`] answers "how wide does this tab's own label want to be",
    /// which is a per-tab question; whether the whole *strip* fits is not. Deciding it in one
    /// place means every tab is scaled by the same rule, so a strip never mixes fitted and
    /// unfitted tabs — which would show as a tab whose label is elided next to one that is not.
    fn fitted_tab_width(&self) -> u32 {
        let rect = self.base.geometry();
        let count = self.tabs.len();
        if count == 0 {
            return 0;
        }
        let count = count as u32;
        let gaps = (TAB_SPACING as u32).saturating_mul(count.saturating_sub(1));
        let available = rect.width.saturating_sub(gaps).max(1);
        let natural = self.compute_tab_width(0).max(self.tab_min_width).max(1);
        natural.min(available / count).max(1)
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
        //
        // The three `TabShape` values were drawn *identically* — all three arms were
        // `fill_rect` + `draw_rect` — so the property moved a value and nothing on screen, and
        // the `Triangular` arm's comment claimed top corners were clipped by work that was never
        // done. The shapes here are the ones [`crate::widget::container_widgets::tabwidget`]
        // draws for the same enum: the two tab controls agree about what `TabShape` means, which
        // is what lets a theme or a caller move a strip from one to the other unchanged.
        //
        // `Rectangular` is the bottom arm because it is the plain case, so a shape added later
        // cannot silently take it over the way the wildcard arm took over `Rectangular` before.
        match self.tab_shape {
            TabShape::Rounded => {
                let radius = 4;
                context.fill_rounded_rect(tab_rect, radius, bg);
                context.draw_rounded_rect_stroke(tab_rect, radius, border, 1);
                // The current tab shares its edge with what it labels, so its bottom edge is
                // overdrawn with the tab's own fill to hide the outline there — the same
                // "blends with the content area" idea `tabwidget` gets for free because its
                // current tab *is* painted in the content colour.
                if is_current {
                    let bottom_rect = Rect::new(
                        tab_rect.x + 1,
                        tab_rect.y + tab_rect.height as i32 - 1,
                        tab_rect.width.saturating_sub(2),
                        2,
                    );
                    context.fill_rect(bottom_rect, bg);
                }
            }
            TabShape::Triangular => {
                // A real triangle: apex on the top edge at the tab's centre, base along the
                // bottom edge of the strip. The comment used to say this and the code did not do
                // it — it filled the whole rectangle and claimed the corners were "clipped".
                let apex_x = tab_rect.x + tab_rect.width as i32 / 2;
                let base_y = tab_rect.y + tab_rect.height as i32;
                let points = [
                    Point::new(apex_x, tab_rect.y),
                    Point::new(tab_rect.x, base_y),
                    Point::new(tab_rect.x + tab_rect.width as i32, base_y),
                ];
                context.draw_path(&points, true, bg, true, 0);
                context.draw_path(&points, true, border, false, 1);
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
            "direction" => {
                Ok(CapabilityValue::String(text_direction_to_str(self.direction()).to_string()))
            }
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
            "direction" => {
                self.set_direction(expect_text_direction(value)?);
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
            "direction",
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

    /// The defect: the tab position was `(tab_width + spacing) * index` with no reference to
    /// `rect.width`, so tabs past the strip's end were drawn entirely outside the control.
    ///
    /// The expectation is read off the control's own API — every tab's rect must be inside the
    /// control's own rectangle — rather than restating any particular fitted width.
    #[test]
    fn tabbar_every_tab_rect_stays_inside_the_control() {
        let geometry = Rect::new(0, 0, 240, 120);
        for count in [1usize, 2, 3, 6, 12] {
            let mut tb = TabBar::new(geometry);
            for i in 0..count {
                tb.add_tab(format!("Tab {i}"));
            }
            let mut right_edge = 0i32;
            for i in 0..count {
                let r = tb.tab_rect(i).expect("every added tab has a rect");
                assert!(
                    r.x >= geometry.x && r.right() <= geometry.right(),
                    "tab {i} of {count} at {r:?} escapes {geometry:?}"
                );
                assert!(r.width > 0, "tab {i} of {count} was shrunk to nothing");
                right_edge = right_edge.max(r.right());
            }
            assert!(
                right_edge <= geometry.right(),
                "{count} tabs reach x = {right_edge} in a {}px strip",
                geometry.width
            );
        }
    }

    /// Shrinking is the strip's overflow rule, and a strip that fits must **not** be shrunk: a
    /// fit rule applied unconditionally would make two tabs on a wide bar tiny.
    #[test]
    fn tabbar_tabs_are_not_shrunk_while_they_fit() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("One".to_string());
        tb.add_tab("Two".to_string());
        let fitted = tb.tab_rect(0).unwrap().width;
        assert_eq!(tb.tab_rect(1).unwrap().width, fitted, "fitted tabs share one width");
        assert!(fitted >= tb.tab_min_width());
        assert!(fitted * 2 < tb.geometry().width, "two tabs must not fill a 400px strip");
    }

    /// In a right-to-left strip the first tab belongs at the strip's **right** edge, and the strip
    /// fills leftward.
    ///
    /// # The defect this pins
    ///
    /// A tab strip is a sequence, so "where does tab 0 sit" is a statement about reading order. The
    /// control placed tab 0 at `rect.x` unconditionally, which in an Arabic or Hebrew interface put
    /// the first tab where the reader looks last and made the visual order contradict the order of
    /// the pages the tabs select.
    #[test]
    fn a_right_to_left_strip_starts_at_its_right_edge() {
        let geometry = Rect::new(0, 0, 240, 120);
        for count in [1usize, 2, 3, 6] {
            let mut tb = TabBar::new(geometry);
            for i in 0..count {
                tb.add_tab(format!("Tab {i}"));
            }
            tb.set_direction(crate::core::TextDirection::RightToLeft);

            let first = tb.tab_rect(0).expect("every added tab has a rect");
            assert_eq!(
                first.right(),
                geometry.right(),
                "with {count} tabs, tab 0 must begin at the strip's beginning (its right edge)"
            );

            // The order is reversed on screen: every later tab is strictly further left.
            let mut previous_x = first.x;
            for i in 1..count {
                let r = tb.tab_rect(i).expect("every added tab has a rect");
                assert!(
                    r.right() <= previous_x,
                    "tab {i} of {count} must sit left of tab {} ({} > {})",
                    i - 1,
                    r.right(),
                    previous_x
                );
                previous_x = r.x;
            }
            // And the run as a whole stays inside the control on the left as well.
            let last = tb.tab_rect(count - 1).unwrap();
            assert!(
                last.x >= geometry.x,
                "the last tab of {count} starts at {} and escapes the strip",
                last.x
            );
        }
    }

    /// The mirror of the overflow rule: an RTL strip whose tabs no longer fit must shrink them
    /// rather than let the run grow off the left end.
    ///
    /// This is the case where reversing the *index* instead of the *offset* would have looked
    /// correct while the tabs still fit and broken as soon as they did not, so the test drives the
    /// shrinking path deliberately.
    #[test]
    fn a_right_to_left_strip_keeps_every_tab_inside_when_they_overflow() {
        let geometry = Rect::new(0, 0, 120, 120);
        let mut tb = TabBar::new(geometry);
        for i in 0..8 {
            tb.add_tab(format!("Tab {i}"));
        }
        tb.set_direction(crate::core::TextDirection::RightToLeft);
        for i in 0..8 {
            let r = tb.tab_rect(i).expect("every added tab has a rect");
            assert!(
                r.x >= geometry.x && r.right() <= geometry.right(),
                "tab {i} at {r:?} escapes the {geometry:?} strip"
            );
            assert!(r.width > 0, "tab {i} was shrunk to nothing");
        }
    }

    /// A left-to-right strip must be byte-for-byte what it was before the field existed, and the
    /// two directions must genuinely differ — otherwise "defaults to LTR" could be satisfied by
    /// ignoring the setting entirely.
    #[test]
    fn setting_the_strip_direction_moves_the_tabs_and_defaults_unchanged() {
        fn render(direction: Option<crate::core::TextDirection>) -> String {
            let mut tb = TabBar::new(Rect::new(0, 0, 300, 120));
            tb.add_tab("One".to_string());
            tb.add_tab("Two".to_string());
            tb.add_tab("Three".to_string());
            if let Some(d) = direction {
                tb.set_direction(d);
            }
            crate::widget::svg::render_to_svg(&mut tb)
        }

        let untouched = render(None);
        let ltr = render(Some(crate::core::TextDirection::LeftToRight));
        let rtl = render(Some(crate::core::TextDirection::RightToLeft));
        assert_eq!(untouched, ltr, "the default must be left-to-right, so the output is identical");
        assert_ne!(ltr, rtl, "a right-to-left strip must draw its tabs somewhere else");
    }

    /// The vertical strips are attached to a side, not to a line, so the direction must not mirror
    /// them onto each other.
    ///
    /// # Why this is an assertion and not a comment
    ///
    /// `West` and `East` are two of the four `TabPosition` values the same field could plausibly be
    /// applied to. "Which side is the strip on" is a layout choice a caller made; applying the
    /// writing direction to it would silently move a strip the caller had explicitly placed.
    #[test]
    fn a_vertical_strip_is_not_mirrored_by_the_direction() {
        for position in [TabPosition::West, TabPosition::East] {
            let mut ltr = TabBar::new(Rect::new(0, 0, 240, 120));
            let mut rtl = TabBar::new(Rect::new(0, 0, 240, 120));
            for tb in [&mut ltr, &mut rtl] {
                tb.add_tab("One".to_string());
                tb.add_tab("Two".to_string());
                tb.set_tab_position(position);
            }
            rtl.set_direction(crate::core::TextDirection::RightToLeft);
            for i in 0..2 {
                assert_eq!(
                    ltr.tab_rect(i),
                    rtl.tab_rect(i),
                    "{position:?} tab {i} moved when only the direction changed"
                );
            }
        }
    }

    /// The direction is reachable from the property API, and the token written is the token read
    /// back.
    ///
    /// A settable-only field, or one whose `get` and `set` spellings disagree, would leave a caller
    /// driving the control from a document unable to restore what they just set.
    #[test]
    fn the_direction_round_trips_through_the_property_api() {
        let mut tb = TabBar::new(Rect::new(0, 0, 240, 24));
        tb.add_tab("One".to_string());

        assert_eq!(tb.get("direction").unwrap().as_str(), Some("ltr"), "default is ltr");
        tb.set("direction", CapabilityValue::String(String::from("rtl"))).unwrap();
        assert_eq!(tb.direction(), crate::core::TextDirection::RightToLeft);
        assert_eq!(tb.get("direction").unwrap().as_str(), Some("rtl"));
        // The long spelling is accepted too, so a caller need not know which one this crate picked.
        tb.set("direction", CapabilityValue::String(String::from("left_to_right"))).unwrap();
        assert_eq!(tb.direction(), crate::core::TextDirection::LeftToRight);
    }

    /// The three `TabShape` values were drawn identically, so the property changed nothing on
    /// screen. Each shape must now produce a different drawing.
    #[test]
    fn tabbar_tab_shapes_are_distinct() {
        fn render(shape: TabShape) -> String {
            let mut tb = TabBar::new(Rect::new(0, 0, 240, 120));
            tb.add_tab("One".to_string());
            tb.add_tab("Two".to_string());
            tb.set_current_index(0);
            tb.set_tab_shape(shape);
            crate::widget::svg::render_to_svg(&mut tb)
        }

        let rounded = render(TabShape::Rounded);
        let triangular = render(TabShape::Triangular);
        let rectangular = render(TabShape::Rectangular);
        assert_ne!(rounded, triangular, "rounded and triangular must not draw the same");
        assert_ne!(rounded, rectangular, "rounded and rectangular must not draw the same");
        assert_ne!(triangular, rectangular, "triangular and rectangular must not draw the same");
        // Each shape leaves its own mark: rounded corners, a closed triangle path, plain rects.
        assert!(rounded.contains("rx="), "a rounded tab must have rounded corners");
        assert!(
            triangular.contains("<polygon") || triangular.contains("<path"),
            "a triangular tab must be drawn as a triangle"
        );
        assert!(
            !rectangular.contains("rx=") && !rectangular.contains("<polygon"),
            "a rectangular tab must be a plain rectangle"
        );
    }
}
