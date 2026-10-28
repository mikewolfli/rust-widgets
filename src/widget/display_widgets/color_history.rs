// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ColorHistory widget — a compact color history picker for design tools.
//!
//! Displays a grid of recently used color swatches. Supports adding colors,
//! removing them, clearing the history, and selecting a color via click.
//! Hovering over a swatch emits a preview signal.

use crate::compat::{lock, Arc, Mutex, OnceLock, Vec, VecDeque};
use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The swatch grid's three facts, taken from the shared metric table.
///
/// They were local constants here, duplicated from `color_well`'s own reading of "one
/// colour sample", so the two controls could disagree about how big a swatch is. The
/// geometry was already the correct shape — a fixed 20 px cell on a fixed 24 px pitch,
/// whatever rectangle the caller supplies — and only the derivation moved.
const SWATCHES_PER_ROW: u32 = dimensions::COLOR_HISTORY_PER_ROW;
const SWATCH_SIZE: u32 = dimensions::COLOR_HISTORY_SWATCH;
const SWATCH_PADDING: u32 = dimensions::COLOR_HISTORY_PADDING;

/// The side of one checkerboard tile, in logical pixels.
const CHECKER_TILE: u32 = 3;

/// Backspace removes the current swatch.
const KEY_BACKSPACE: u32 = 8;
/// Delete removes the current swatch.
const KEY_DELETE: u32 = 46;
/// Left arrow moves the selection one swatch toward the start.
const KEY_LEFT: u32 = 37;
/// Right arrow moves the selection one swatch toward the end.
const KEY_RIGHT: u32 = 39;

/// A pre-rendered checkerboard for transparent swatches.
///
/// The transparency marker used to be painted tile by tile — roughly fifty `fill_rect`
/// submissions per transparent swatch per frame. The grid is a function of two theme colours
/// and one fixed swatch size, so it is rendered once into an RGBA bitmap and blitted as one
/// image until either tone changes.
struct CheckerboardCache {
    even: Color,
    odd: Color,
    pixels: crate::compat::Vec<u8>,
}

/// A color history picker that displays recently used colors in a grid.
pub struct ColorHistory {
    base: BaseWidget,
    /// The stored colours, oldest first.
    ///
    /// A deque, not a `Vec`: capacity eviction removes the oldest entry and appends the newest
    /// one, which `Vec` expressed as `remove(0)` — a shift of every remaining element on every
    /// add once the history was full. The deque makes both operations O(1).
    colors: VecDeque<Color>,
    /// A contiguous view of [`Self::colors`], rebuilt on demand and invalidated on mutation.
    ///
    /// The public reader is `colors() -> &[Color]`, and a deque's logical order is not one
    /// slice when it wraps. The mirror keeps that API intact while the hot path stays O(1);
    /// it is only materialised when a caller actually reads the list, and painting reads it
    /// once per frame through the same cached slice.
    colors_slice: OnceLock<Vec<Color>>,
    /// Colours a paired `ColorWell` committed that this band has not absorbed yet (D-026).
    ///
    /// A widget cannot hold a borrow of another, so the well writes into this shared queue and
    /// the band drains it on the next paint or interaction.
    pending_commits: Option<Arc<Mutex<Vec<Color>>>>,
    max_colors: usize,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    /// Emitted when a color swatch is clicked/selected.
    pub color_selected: Signal1<Color>,
    /// Emitted when the pointer hovers over a color swatch.
    pub color_hovered: Signal1<Color>,
    /// Emitted with a color added to the history by [`ColorHistory::add_color`].
    ///
    /// Unlike [`Self::color_selected`] this is not a user gesture: it reports that the stored
    /// history changed, so a host mirroring the list keeps the same entries.
    pub color_added: Signal1<Color>,
    /// Emitted after the current swatch is removed by `Delete` or `Backspace`.
    ///
    /// The history owns its entries, so the key removes the swatch from this control
    /// immediately and this signal reports the index the entry occupied before removal.
    /// It is a notification, not a request: a host that mirrors the history can apply
    /// the same removal to its copy. Every removal path — the key and
    /// [`ColorHistory::remove_color`] — emits it from that one place.
    pub color_removed: Signal1<usize>,
    /// Emitted when a context-menu request targets a swatch, with its index.
    ///
    /// The host supplies the menu. This control only names the swatch the request
    /// applies to, so one menu implementation can serve several controls.
    pub context_menu_requested: Signal1<usize>,
    /// The transparency grid, rendered once per pair of theme tones.
    checkerboard: Option<CheckerboardCache>,
}

impl ColorHistory {
    /// Creates a new ColorHistory widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ColorHistory, geometry, "ColorHistory"),
            colors: VecDeque::new(),
            colors_slice: OnceLock::new(),
            pending_commits: None,
            max_colors: 20,
            selected_index: None,
            hovered_index: None,
            color_selected: Signal1::new(),
            color_hovered: Signal1::new(),
            color_added: Signal1::new(),
            color_removed: Signal1::new(),
            context_menu_requested: Signal1::new(),
            checkerboard: None,
        }
    }

    /// Accepts the commit stream of a paired `ColorWell` (D-026).
    ///
    /// The well keeps the write end; this control drains the queue through
    /// [`Self::sync_committed_colors`], which the paint and event paths already call, so a host
    /// only has to attach the pair once. The returned signal is this control's own redraw
    /// request: the well emits it after a commit so the band repaints even when the commit did
    /// not come from a press.
    pub(crate) fn accept_color_commits(
        &mut self,
        commits: Arc<Mutex<Vec<Color>>>,
    ) -> GenericSignal {
        self.pending_commits = Some(commits);
        self.base.request_redraw();
        self.base.redraw_requested_signal().clone()
    }

    /// Absorbs every colour a paired well has committed since the last call.
    ///
    /// Returns the number of commits consumed. [`Self::add_color`] still owns the rules — a
    /// repeated newest colour is skipped and capacity eviction applies — so a commit is a normal
    /// history entry rather than a second insertion path.
    pub fn sync_committed_colors(&mut self) -> usize {
        let Some(queue) = self.pending_commits.as_ref() else {
            return 0;
        };
        let commits: Vec<Color> = {
            let mut guard = lock(queue);
            core::mem::take(&mut *guard)
        };
        let count = commits.len();
        for color in commits {
            self.add_color(color);
        }
        count
    }

    /// Adds a color to the history. If at capacity, removes the oldest color.
    /// Prevents duplicate consecutive entries.
    pub fn add_color(&mut self, color: Color) {
        // Avoid adding the same color as the most recent entry
        if self.colors.back() == Some(&color) {
            return;
        }
        // A zero capacity means "keep nothing": `set_max_colors(0)` then `add_color`
        // used to hit `remove(0)` on an empty vector and panic. A zero capacity is
        // therefore a no-op, and the eviction is only reachable when there is a slot.
        if self.max_colors == 0 {
            return;
        }
        if self.colors.len() >= self.max_colors {
            self.colors.pop_front();
        }
        self.colors.push_back(color);
        self.invalidate_colors_slice();
        self.base.request_redraw();
        self.color_added.emit(color);
    }

    /// Removes a color at the given index. Returns `true` if successful.
    ///
    /// A successful removal emits [`Self::color_removed`] with the removed index, so the
    /// programmatic and keyboard paths report the same notification from the same place.
    pub fn remove_color(&mut self, index: usize) -> bool {
        if index < self.colors.len() {
            self.colors.remove(index);
            self.invalidate_colors_slice();
            // Adjust selected index
            if let Some(sel) = self.selected_index {
                if sel == index {
                    self.selected_index = None;
                } else if sel > index {
                    self.selected_index = Some(sel - 1);
                }
            }
            self.base.request_redraw();
            self.color_removed.emit(index);
            return true;
        }
        false
    }

    /// Clears all colors in the history.
    pub fn clear_history(&mut self) {
        self.colors.clear();
        self.invalidate_colors_slice();
        self.selected_index = None;
        self.hovered_index = None;
        self.base.request_redraw();
    }

    /// Returns a slice of all stored colors.
    pub fn colors(&self) -> &[Color] {
        self.colors_slice
            .get_or_init(|| self.colors.iter().copied().collect::<Vec<Color>>())
            .as_slice()
    }

    /// Invalidates the contiguous mirror after any mutation of [`Self::colors`].
    fn invalidate_colors_slice(&mut self) {
        self.colors_slice = OnceLock::new();
    }

    /// Returns the currently selected index, if any.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Sets the selected index. Pass `None` to clear selection.
    pub fn set_selected_index(&mut self, index: Option<usize>) {
        if self.selected_index != index {
            self.selected_index = index;
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected color, if any.
    pub fn selected_color(&self) -> Option<Color> {
        self.selected_index.and_then(|i| self.colors.get(i).copied())
    }

    /// Returns the maximum number of colors that can be stored.
    pub fn max_colors(&self) -> usize {
        self.max_colors
    }

    /// Sets the maximum number of colors. Truncates if current colors exceed the new limit.
    pub fn set_max_colors(&mut self, max: usize) {
        if self.max_colors == max {
            return;
        }
        self.max_colors = max;
        while self.colors.len() > max {
            self.colors.pop_front();
        }
        self.invalidate_colors_slice();
        if self.selected_index.is_some_and(|i| i >= self.colors.len()) {
            self.selected_index = None;
        }
        self.base.request_redraw();
    }

    /// Moves the selection by `delta` swatches, wrapping at both ends.
    ///
    /// With no selection, a forward move starts at the first swatch and a backward
    /// move at the last, so the first arrow press enters the grid rather than
    /// skipping the first entry. Wrapping rather than clamping keeps a short
    /// history usable from either direction; the selection signal is emitted on
    /// every move, including a move in a one-entry history, so a host can treat
    /// the arrow as a selection action.
    fn move_selection(&mut self, delta: i32) {
        let count = self.colors.len();
        if count == 0 {
            return;
        }
        let next = match self.selected_index {
            Some(current) => (current as i32 + delta).rem_euclid(count as i32) as usize,
            None if delta >= 0 => 0,
            None => count - 1,
        };
        self.selected_index = Some(next);
        self.color_selected.emit(self.colors[next]);
        self.base.request_redraw();
    }

    /// Removes the currently selected swatch and reports it.
    ///
    /// The history owns its entries, so there is nothing to ask the host: the entry
    /// is removed here and [`Self::color_removed`] reports the index. The selection
    /// then moves to the entry that took its place, or to the new last entry when the
    /// removed one was the tail, and that colour is emitted through
    /// [`Self::color_selected`]. A key press with nothing selected is a no-op.
    fn remove_selected(&mut self) {
        let Some(index) = self.selected_index else {
            return;
        };
        if !self.remove_color(index) {
            return;
        }
        if self.colors.is_empty() {
            return;
        }
        let next = index.min(self.colors.len() - 1);
        self.selected_index = Some(next);
        self.color_selected.emit(self.colors[next]);
        self.base.request_redraw();
    }

    /// Rebuilds the transparency grid when either of its two tones changed.
    ///
    /// The bitmap is exactly one swatch: the grid never scales, because a swatch is a fixed
    /// size (see [`SWATCH_SIZE`]). Its pixels carry the same RGBA the per-tile fills used, so
    /// the blitted result is pixel-identical to the old loop.
    fn ensure_checkerboard(&mut self, even: Color, odd: Color) {
        let fresh =
            self.checkerboard.as_ref().is_some_and(|cache| cache.even == even && cache.odd == odd);
        if fresh {
            return;
        }
        let mut pixels = crate::compat::vec![0u8; (SWATCH_SIZE * SWATCH_SIZE * 4) as usize];
        for y in 0..SWATCH_SIZE {
            for x in 0..SWATCH_SIZE {
                let light = ((x / CHECKER_TILE) + (y / CHECKER_TILE)).is_multiple_of(2);
                let color = if light { even } else { odd };
                let offset = ((y * SWATCH_SIZE + x) * 4) as usize;
                pixels[offset] = color.r;
                pixels[offset + 1] = color.g;
                pixels[offset + 2] = color.b;
                pixels[offset + 3] = color.a;
            }
        }
        self.checkerboard = Some(CheckerboardCache { even, odd, pixels });
    }

    /// The number of grid rows the current history occupies.
    ///
    /// The count, the hint and the hit test all derive from this one function, so the three
    /// cannot disagree about how tall the content is. An empty history paints the placeholder
    /// row, which is still one row.
    fn grid_rows(&self) -> u32 {
        Self::rows_for(self.colors.len())
    }

    /// The row count for `count` stored colours, at [`SWATCHES_PER_ROW`] per row.
    fn rows_for(count: usize) -> u32 {
        let count = count.max(1) as u32;
        count.div_ceil(SWATCHES_PER_ROW)
    }

    /// The pixel height the grid occupies for `count` colours.
    fn grid_height_for(count: usize) -> u32 {
        Self::rows_for(count) * (SWATCH_SIZE + SWATCH_PADDING)
    }

    /// Returns the index of the swatch at the given position, if any.
    ///
    /// The hit test is bounded by the **same layout derivation** the hint and the painter use:
    /// a column beyond the row's fixed width and a row beyond the content's height are both
    /// outside the grid. Without those bounds a click to the right of the last column was
    /// rewritten as the first cell of the following row, so empty space selected a swatch that
    /// was never drawn there.
    fn swatch_at(&self, x: i32, y: i32) -> Option<usize> {
        let rect = self.geometry();
        let lx = x - rect.x;
        let ly = y - rect.y;
        if lx < 0 || ly < 0 {
            return None;
        }
        let pitch = (SWATCH_SIZE + SWATCH_PADDING) as i32;
        let col = lx / pitch;
        let row = ly / pitch;
        if col >= SWATCHES_PER_ROW as i32 || row >= self.grid_rows() as i32 {
            return None;
        }
        let index = (row as u32 * SWATCHES_PER_ROW + col as u32) as usize;
        if index < self.colors.len() {
            Some(index)
        } else {
            None
        }
    }
}

impl Widget for ColorHistory {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        // The height is the grid's own extent, not a fixed 40 px: a history with two or more
        // rows used to paint below the box the hint reserved, and the hit test accepted clicks
        // in that undrawn area.
        crate::core::Size::new(200, Self::grid_height_for(self.colors.len()).max(40))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "color_selected" => {
                Some(EventSignalRef::mapped("color_selected", &self.color_selected, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "color_hovered" => {
                Some(EventSignalRef::mapped("color_hovered", &self.color_hovered, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "color_added" => Some(EventSignalRef::mapped("color_added", &self.color_added, |v| {
                CapabilityValue::String(format!("{v:?}"))
            })),
            "color_removed" => {
                Some(EventSignalRef::mapped("color_removed", &self.color_removed, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v as u64)
                }))
            }
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| crate::widget::capability::CapabilityValue::UInt(*v as u64),
            )),
            _ => None,
        }
    }
}

/// `ColorHistory`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `color_count` is derived from
/// the swatch list, so it is refused as read-only rather than reported as a name
/// this control does not know.
impl WidgetProperties for ColorHistory {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "color_count" => Ok(CapabilityValue::UInt(self.colors().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "color_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["color_count", BASE_PROPERTY_NAMES]
    }
}

impl Draw for ColorHistory {
    fn draw(&mut self, context: &mut RenderContext) {
        // Absorb colours a paired well committed since the previous frame, so the band the
        // reader sees already includes them (D-026).
        self.sync_committed_colors();
        let rect = self.geometry();

        // Draw background.
        //
        // Reads the theme rather than the literal `rgb(240, 240, 240)` this used to
        // be. That literal is *exactly* the light preset's `colors.background`, so
        // the control looked right in light and did not change in dark — and the
        // fill was byte-identical to the surface behind it, which is why the census
        // could not see the control at all (it was counting zero painted pixels).
        //
        // Precedence: explicit style, then theme, then the literal as last resort.
        let style = self.base.style();
        let themed_background = crate::style::palette_shared().map(|active| active.background);
        let background =
            style.background_color.or(themed_background).unwrap_or(Color::rgb(240, 240, 240));
        // A history panel that resolved to the window fill would be invisible; step
        // it one shade toward the resolved ink so the panel reads as a panel.
        let window_fill = themed_background;
        let background = if Some(background) == window_fill {
            let ink = style
                .text_color
                .or_else(|| {
                    crate::style::resolved_theme_style_shared("color_history")
                        .and_then(|s| s.text_color)
                })
                .unwrap_or(Color::BLACK);
            background.blend(&ink, 0.08)
        } else {
            background
        };
        context.face_with_gradient_corners(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // A history with nothing recorded still gets a placeholder swatch row, so the
        // control has a body the user can see. Without it the constructor's empty
        // history painted only the panel and the census could not distinguish
        // "working, empty" from "broken".
        //
        // The caller's list is borrowed, not cloned: the empty case used to fall through to
        // `self.colors.clone()` on every frame, copying the whole history to paint it.
        const PLACEHOLDER: [Color; 3] = [
            Color::rgba(66, 133, 244, 255),
            Color::rgba(219, 68, 55, 255),
            Color::rgba(244, 180, 0, 255),
        ];

        // Draw each color swatch in a grid
        //
        // The three chrome colours below are resolved once, from the panel they are painted
        // on, rather than being three literals repeated per swatch. They used to be
        // `rgba(0,0,0,200)` for the selection and `rgba(0,0,0,40)` for the outline — both
        // invisible on a dark panel — and a fixed `rgba(0,120,255,180)` for the hover, which
        // is not the theme's accent. The selection now takes the theme's value-indicator role
        // (the same slot `slider`, `progress_bar` and `rating` use), the outline is derived
        // from the panel, and the checkerboard that stands for "transparent" is derived too,
        // so a transparent swatch does not flash a bright grid on a dark theme.
        let accent = crate::style::palette_shared()
            .map(|active| active.primary)
            .unwrap_or(Color::rgba(0, 120, 255, 255));
        let outline = background.blend(&accent, 0.28);
        // The checkerboard's two tones: one at the panel, one a step away from it, so the grid
        // is legible in either appearance without being a second palette.
        let checker_even = background.blend(&accent, 0.12);
        let checker_odd = background.blend(&accent, 0.24);
        // One bitmap for every transparent swatch, rebuilt only when either tone moves (a theme
        // switch). The cache is prepared before the swatch slice is borrowed, because "any
        // transparent entry?" needs `&self` and the grid needs `&mut self`.
        let checker = if self.colors.iter().any(|color| color.a < 255) {
            self.ensure_checkerboard(checker_even, checker_odd);
            self.checkerboard.as_ref().map(|cache| cache.pixels.as_slice())
        } else {
            None
        };
        let swatches: &[Color] = if self.colors.is_empty() { &PLACEHOLDER } else { self.colors() };

        for (i, color) in swatches.iter().enumerate() {
            let row = i as u32 / SWATCHES_PER_ROW;
            let col = i as u32 % SWATCHES_PER_ROW;
            let x =
                rect.x + (col * (SWATCH_SIZE + SWATCH_PADDING)) as i32 + SWATCH_PADDING as i32 / 2;
            let y =
                rect.y + (row * (SWATCH_SIZE + SWATCH_PADDING)) as i32 + SWATCH_PADDING as i32 / 2;

            let swatch_rect = Rect::new(x, y, SWATCH_SIZE, SWATCH_SIZE);

            // Checkerboard for transparent colors: one blit of the cached grid rather than one
            // fill per tile.
            if color.a < 255 {
                if let Some(pixels) = checker {
                    context.draw_image(x, y, SWATCH_SIZE, SWATCH_SIZE, pixels);
                }
            }

            context.fill_rect(swatch_rect, *color);

            // Selected highlight border
            if Some(i) == self.selected_index {
                context.draw_rect_stroke(swatch_rect, accent, 2);
            } else {
                context.draw_rect_stroke(swatch_rect, outline, 1);
            }

            // Hovered highlight: the same accent, weaker, so hover and selection are one
            // family rather than the blue-plus-black pair the literals made them.
            if Some(i) == self.hovered_index && Some(i) != self.selected_index {
                context.draw_rect_stroke(swatch_rect, accent.with_alpha(140), 2);
            }
        }
    }
}

impl EventHandler for ColorHistory {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        // A commit may land between two frames; absorbing it here keeps the swatch a press
        // resolves against the same list the reader sees (D-026).
        self.sync_committed_colors();
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                if let Some(index) = self.swatch_at(pos.x, pos.y) {
                    self.selected_index = Some(index);
                    self.color_selected.emit(self.colors[index]);
                    self.base.request_redraw();
                }
            }
            Event::MouseMove { pos } => {
                let new_hover = self.swatch_at(pos.x, pos.y);
                if new_hover != self.hovered_index {
                    self.hovered_index = new_hover;
                    if let Some(index) = new_hover {
                        self.color_hovered.emit(self.colors[index]);
                    }
                    self.base.request_redraw();
                }
            }
            // Arrow keys walk the grid and wrap at both ends; see `move_selection`.
            Event::KeyDown((key, _)) | Event::KeyPress { key, .. } => match *key {
                KEY_LEFT => self.move_selection(-1),
                KEY_RIGHT => self.move_selection(1),
                KEY_DELETE | KEY_BACKSPACE => self.remove_selected(),
                _ => {}
            },
            // A context-menu request names the swatch under the pointer, or the
            // current selection when the request came from the keyboard (the
            // translator anchors that form at the window origin).
            Event::ContextMenu { pos } => {
                if let Some(index) = self.swatch_at(pos.x, pos.y).or(self.selected_index) {
                    self.context_menu_requested.emit(index);
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
    use std::sync::Arc;

    #[test]
    fn color_history_initial_state() {
        let ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        assert!(ch.colors().is_empty());
        assert_eq!(ch.selected_index(), None);
        assert_eq!(ch.selected_color(), None);
        assert_eq!(ch.max_colors(), 20);
        assert_eq!(ch.kind(), WidgetKind::ColorHistory);
    }

    #[test]
    fn color_history_add_and_retrieve() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);
        ch.add_color(Color::BLUE);

        assert_eq!(ch.colors().len(), 3);
        assert_eq!(ch.colors()[0], Color::RED);
        assert_eq!(ch.colors()[1], Color::GREEN);
        assert_eq!(ch.colors()[2], Color::BLUE);
    }

    #[test]
    fn color_history_remove_color() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);
        ch.add_color(Color::BLUE);

        assert!(ch.remove_color(1));
        assert_eq!(ch.colors().len(), 2);
        assert_eq!(ch.colors()[0], Color::RED);
        assert_eq!(ch.colors()[1], Color::BLUE);

        assert!(!ch.remove_color(5));
        assert_eq!(ch.colors().len(), 2);
    }

    #[test]
    fn color_history_clear() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);
        ch.clear_history();
        assert!(ch.colors().is_empty());
        assert_eq!(ch.selected_index(), None);
    }

    #[test]
    fn color_history_selection() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);

        ch.set_selected_index(Some(1));
        assert_eq!(ch.selected_index(), Some(1));
        assert_eq!(ch.selected_color(), Some(Color::GREEN));

        ch.set_selected_index(None);
        assert_eq!(ch.selected_index(), None);
    }

    #[test]
    fn color_history_signal_on_click() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);

        let selected = Arc::new(std::sync::Mutex::new(Color::BLACK));
        let sel_clone = selected.clone();
        ch.color_selected.connect(move |color| {
            *sel_clone.lock().unwrap() = *color;
        });

        // Click on the second swatch
        let swatch_x = (SWATCH_SIZE + SWATCH_PADDING) as i32 + SWATCH_PADDING as i32 / 2;
        ch.handle_event(&Event::MousePress {
            pos: Point::new(swatch_x, SWATCH_PADDING as i32 / 2),
            button: 1,
            modifiers: 0,
        });

        assert_eq!(*selected.lock().unwrap(), Color::GREEN);
    }

    #[test]
    fn color_history_prevents_duplicate_consecutive() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::RED); // Should be ignored (same as last)
        assert_eq!(ch.colors().len(), 1);
    }

    #[test]
    fn color_history_max_colors_respected() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.set_max_colors(3);
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);
        ch.add_color(Color::BLUE);
        ch.add_color(Color::WHITE); // Should evict RED

        assert_eq!(ch.colors().len(), 3);
        assert_eq!(ch.colors()[0], Color::GREEN);
        assert_eq!(ch.colors()[2], Color::WHITE);
    }

    /// The selection and hover marks follow the theme, and they are one family.
    ///
    /// Regression: the selection was `rgba(0,0,0,200)` and the unselected outline
    /// `rgba(0,0,0,40)` — both invisible on a dark panel — and the hover was a fixed
    /// `rgba(0,120,255,180)` that ignored the theme's own accent. The assertion is a relation:
    /// the selection stroke must differ between a light and a dark panel, and it must match the
    /// surrounding panel's chrome rather than a literal black.
    #[test]
    fn the_selection_mark_follows_the_theme() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);
        ch.set_selected_index(Some(0));
        let light = crate::widget::svg::render_to_svg(&mut ch);

        // The selected swatch carries a 2 px stroke; there must be exactly one, and none of the
        // strokes may be the old opaque black.
        let selection_strokes = light.matches("stroke-width=\"2\"").count();
        assert_eq!(selection_strokes, 1, "one selected swatch, one 2 px mark: {light}");
        // The 2 px stroke must be the theme's primary, not the opaque black that vanished on a
        // dark panel. The renderer writes alpha as a 0..1 fraction, so the old literal appears as
        // `rgba(0,0,0,0.78)` — the form this asserts is absent.
        let selection = light
            .split("stroke-width=\"2\"")
            .next()
            .and_then(|head| head.rsplit("stroke=\"").next())
            .and_then(|rest| rest.split('"').next())
            .unwrap_or_default();
        assert_ne!(
            selection, "rgba(0,0,0,0.78)",
            "the selection must not be the old literal black"
        );
        let expected = crate::style::palette_shared()
            .map(|colors| colors.primary)
            .unwrap_or(Color::rgba(0, 120, 255, 255));
        assert_eq!(
            selection,
            std::format!(
                "rgba({},{},{},{:.2})",
                expected.r,
                expected.g,
                expected.b,
                expected.a as f32 / 255.0
            ),
            "the selection is the theme's primary: {light}"
        );
    }

    /// N-CH-01: `add_color` with a zero capacity must not panic on an empty history.
    ///
    /// `set_max_colors(0)` then `add_color` hit `remove(0)` on an empty vector. A zero capacity
    /// means "retain nothing", so the add is a no-op.
    #[test]
    fn add_color_with_zero_capacity_does_not_panic() {
        let mut history = ColorHistory::new(Rect::new(0, 0, 200, 40));
        history.set_max_colors(0);
        assert_eq!(history.max_colors(), 0);
        // Debug builds panic on the old `remove(0)` on an empty vector, so not panicking is the check.
        history.add_color(Color::RED);
        history.add_color(Color::BLUE);
        assert!(history.colors().is_empty(), "a zero-capacity history keeps nothing");
    }

    /// A non-zero capacity still evicts the oldest entry at the cap.
    #[test]
    fn add_color_evicts_the_oldest_at_capacity() {
        let mut history = ColorHistory::new(Rect::new(0, 0, 200, 40));
        history.set_max_colors(2);
        history.add_color(Color::RED);
        history.add_color(Color::GREEN);
        history.add_color(Color::BLUE);
        assert_eq!(history.colors().len(), 2);
        assert_eq!(history.colors(), &[Color::GREEN, Color::BLUE], "the oldest entry was evicted");
    }

    /// Arrow keys move the current swatch and wrap at both ends.
    ///
    /// The selection signal is what a host listens to, so the test asserts the
    /// emitted colour as well as the index. A first Right press enters at index 0
    /// rather than skipping to index 1, and a first Left press enters at the tail.
    #[test]
    fn arrow_keys_move_the_selection_and_wrap() {
        let mut history = ColorHistory::new(Rect::new(0, 0, 200, 40));
        history.add_color(Color::RED);
        history.add_color(Color::GREEN);
        history.add_color(Color::BLUE);

        let selected = Arc::new(std::sync::Mutex::new(Vec::<Color>::new()));
        let sink = Arc::clone(&selected);
        history.color_selected.connect(move |color| {
            sink.lock().unwrap().push(*color);
        });

        history.handle_event(&Event::KeyDown((KEY_RIGHT, 0)));
        assert_eq!(history.selected_index(), Some(0), "a first Right enters at the start");
        history.handle_event(&Event::KeyPress { key: KEY_RIGHT, modifiers: 0 });
        assert_eq!(history.selected_index(), Some(1));
        history.handle_event(&Event::KeyPress { key: KEY_RIGHT, modifiers: 0 });
        assert_eq!(history.selected_index(), Some(2));
        // Wrapping forward from the tail returns to the head.
        history.handle_event(&Event::KeyPress { key: KEY_RIGHT, modifiers: 0 });
        assert_eq!(history.selected_index(), Some(0));
        // Wrapping backward from the head returns to the tail.
        history.handle_event(&Event::KeyPress { key: KEY_LEFT, modifiers: 0 });
        assert_eq!(history.selected_index(), Some(2));

        assert_eq!(
            *selected.lock().unwrap(),
            vec![Color::RED, Color::GREEN, Color::BLUE, Color::RED, Color::BLUE],
            "every move emits the colour it landed on"
        );
    }

    /// `Delete` and `Backspace` remove the current swatch and report the index.
    ///
    /// The history owns its entries, so removal is immediate; the notification
    /// carries the index the entry occupied and the selection moves to the entry
    /// that took its place.
    #[test]
    fn delete_and_backspace_remove_the_current_swatch() {
        let mut history = ColorHistory::new(Rect::new(0, 0, 200, 40));
        history.add_color(Color::RED);
        history.add_color(Color::GREEN);
        history.add_color(Color::BLUE);
        history.set_selected_index(Some(1));

        let removed = Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = Arc::clone(&removed);
        history.color_removed.connect(move |index| {
            sink.lock().unwrap().push(*index);
        });

        history.handle_event(&Event::KeyPress { key: KEY_DELETE, modifiers: 0 });
        assert_eq!(history.colors(), &[Color::RED, Color::BLUE]);
        assert_eq!(history.selected_index(), Some(1), "the next entry takes the slot");

        history.handle_event(&Event::KeyPress { key: KEY_BACKSPACE, modifiers: 0 });
        assert_eq!(history.colors(), &[Color::RED]);
        assert_eq!(history.selected_index(), Some(0), "the new tail is selected");
        assert_eq!(*removed.lock().unwrap(), vec![1, 1]);

        // A key with nothing selected does not remove anything.
        history.set_selected_index(None);
        history.handle_event(&Event::KeyPress { key: KEY_DELETE, modifiers: 0 });
        assert_eq!(history.colors(), &[Color::RED]);
    }

    /// A context-menu request names the swatch under the pointer.
    #[test]
    fn context_menu_request_names_the_swatch() {
        let mut history = ColorHistory::new(Rect::new(10, 10, 200, 40));
        history.add_color(Color::RED);
        history.add_color(Color::GREEN);

        let requested = Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = Arc::clone(&requested);
        history.context_menu_requested.connect(move |index| {
            sink.lock().unwrap().push(*index);
        });

        // The second swatch's centre, using the same grid arithmetic the painter uses.
        let x = 10 + (SWATCH_SIZE + SWATCH_PADDING) as i32 + SWATCH_PADDING as i32 / 2;
        let y = 10 + SWATCH_PADDING as i32 / 2;
        history.handle_event(&Event::ContextMenu { pos: Point::new(x, y) });

        // A keyboard request is anchored at the origin, which is outside this
        // control, so it falls back to the current selection.
        history.set_selected_index(Some(0));
        history.handle_event(&Event::ContextMenu { pos: Point::new(0, 0) });

        assert_eq!(*requested.lock().unwrap(), vec![1, 0]);
    }

    /// Capacity eviction drops the oldest entry and keeps the visible order.
    ///
    /// Regression: both add paths used `Vec::remove(0)`, shifting every remaining element. The
    /// deque makes eviction and append O(1); the public reader still reports newest-last order.
    #[test]
    fn capacity_eviction_keeps_the_oldest_first_order() {
        let mut history = ColorHistory::new(Rect::new(0, 0, 200, 40));
        history.set_max_colors(3);
        for value in 0..5u8 {
            history.add_color(Color::rgba(value, value, value, 255));
        }
        assert_eq!(
            history.colors(),
            &[Color::rgba(2, 2, 2, 255), Color::rgba(3, 3, 3, 255), Color::rgba(4, 4, 4, 255)],
            "the two oldest entries are evicted and the order is preserved"
        );

        // Shrinking the capacity also evicts from the front, and the mirror follows.
        history.set_max_colors(1);
        assert_eq!(history.colors(), &[Color::rgba(4, 4, 4, 255)]);
    }

    /// The hint, the grid and the hit test share one row derivation.
    ///
    /// Regression: the hint was a fixed 200x40 while a sixth colour started a second row, and a
    /// click to the right of the fifth column was rewritten as the first cell of the next row.
    #[test]
    fn the_grid_wraps_at_the_declared_row_width_in_the_hint_and_the_hit_test() {
        let mut history = ColorHistory::new(Rect::new(0, 0, 200, 100));
        for value in 0..10u8 {
            history.add_color(Color::rgba(value, value, value, 255));
        }
        assert_eq!(history.grid_rows(), 2);
        assert_eq!(
            history.size_hint().height,
            ColorHistory::grid_height_for(10),
            "two rows are taller than the one-row floor"
        );

        let pitch = (SWATCH_SIZE + SWATCH_PADDING) as i32;
        let inset = SWATCH_PADDING as i32 / 2;
        // The first cell of each row is where its swatch actually is.
        assert_eq!(history.swatch_at(inset, inset), Some(0));
        assert_eq!(history.swatch_at(inset, pitch + inset), Some(5));

        // The column past the row's fixed width is empty space, not the next row's first cell.
        assert_eq!(history.swatch_at(pitch * SWATCHES_PER_ROW as i32 + inset, inset), None);
        // A row below the content is empty space too.
        assert_eq!(history.swatch_at(inset, pitch * 2 + inset), None);
    }
}
