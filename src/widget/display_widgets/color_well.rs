// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ColorWell widget — a compact color swatch that displays the current color
//! and emits a click signal when pressed.
//!
//! The ColorWell shows a filled rectangle with the selected color. When the
//! color has alpha transparency, a checkerboard pattern is rendered behind it
//! to indicate the transparent regions. An optional border frames the swatch.

use crate::compat::{lock, Arc, Duration, Mutex, Vec};
use crate::core::{Color, Point, Rect};
use crate::event::dnd::{DragPayload, DragSource, DropEffect, DropTarget};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::display_widgets::color_history::ColorHistory;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The payload type a colour well carries and accepts.
const COLOR_PAYLOAD_TYPE: &str = "color";

/// The side of one checkerboard tile, in logical pixels.
const CHECKER_TILE: u32 = 4;

/// How long two presses may be apart and still count as one double click.
const DOUBLE_CLICK_INTERVAL_MS: u64 = 400;

/// How far apart two presses may be and still count as one double click.
const DOUBLE_CLICK_SLOP: i32 = 4;

/// A pre-rendered transparency checkerboard for the well.
///
/// The grid is a function of two theme colours, a fixed tile size and the well's size, so the tile
/// plan is computed once and replayed until either tone or the size changes. It is replayed as
/// rectangles rather than as an RGBA blit because the marker exists to be visible in *every*
/// backend: the vector exporter draws rectangles, so a bitmap-only grid silently disappeared there
/// (and left the well looking empty).
struct CheckerboardCache {
    even: Color,
    odd: Color,
    width: u32,
    height: u32,
    /// `(x, y, width, height, colour)` in the well's local coordinates.
    tiles: crate::compat::Vec<(i32, i32, u32, u32, Color)>,
}

/// The write end of a paired [`ColorHistory`]'s commit stream (D-026).
///
/// The queue carries the colours; the redraw signal is the history's own, so a commit repaints
/// the band even when it did not originate from a pointer gesture.
struct ColorCommitSink {
    colors: Arc<Mutex<Vec<Color>>>,
    redraw: GenericSignal,
}

/// A compact color swatch widget that displays a color and emits a signal
/// when clicked.
pub struct ColorWell {
    base: BaseWidget,
    color: Color,
    show_border: bool,
    /// The previous left press and its reading, for double-click detection.
    ///
    /// The interval and distance thresholds are the platform conventions:
    /// two presses within 400 ms and 4 px are one gesture. Keeping the reading
    /// in the control means a host need not pre-classify the pointer stream.
    last_press: Option<(crate::compat::Instant, Point)>,
    /// Emitted when the color well is clicked.
    pub clicked: GenericSignal,
    /// Emitted when a second press lands within 400 ms and 4 px of the first.
    ///
    /// This is the gesture that opens a color chooser. The control does not open
    /// one itself: the host owns the picker, so the signal is the request.
    pub picker_requested: GenericSignal,
    /// Emitted with the new colour whenever [`ColorWell::set_color`] changes it.
    ///
    /// Every colour-changing path — the property route, a drop, and the host's own
    /// calls — ends in `set_color`, so a mirrored copy of the well sees them all.
    /// This is an assignment signal, not a commit signal: a host previewing a colour
    /// streams values through it without filling the recent-colour band.
    pub color_changed: Signal1<Color>,
    /// The commit stream of a paired [`ColorHistory`], when one is attached (D-026).
    ///
    /// Only [`Self::commit_color`], [`Self::commit`] and a drop write here. A plain
    /// [`Self::set_color`] is a preview and deliberately leaves the history alone.
    history_commits: Option<ColorCommitSink>,
    /// The transparency grid, rendered once per pair of tones and well size.
    checkerboard: Option<CheckerboardCache>,
}

impl ColorWell {
    /// Creates a new ColorWell widget with the given color and geometry.
    ///
    /// The border is enabled by default.
    pub fn new(color: Color, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ColorWell, geometry, "ColorWell"),
            color,
            show_border: true,
            last_press: None,
            clicked: GenericSignal::new(),
            picker_requested: GenericSignal::new(),
            color_changed: Signal1::new(),
            history_commits: None,
            checkerboard: None,
        }
    }

    /// Returns the current color.
    pub fn color(&self) -> Color {
        self.color
    }

    /// Sets the current color and requests a redraw.
    ///
    /// This is the *preview* route: it changes what the well paints and reports the value through
    /// [`Self::color_changed`], but it does not record the colour in a paired history. Use
    /// [`Self::commit_color`] when the value is final (D-026).
    pub fn set_color(&mut self, color: Color) {
        if self.color == color {
            return;
        }
        self.color = color;
        self.base.request_redraw();
        self.color_changed.emit(color);
    }

    /// Pairs this well with `history`, so a committed colour enters the recent-colour band.
    ///
    /// Attaching replaces any previous pairing. The history drains the commit stream on its next
    /// paint or interaction, so the host does not have to poll (D-026).
    pub fn attach_history(&mut self, history: &mut ColorHistory) {
        let commits = Arc::new(Mutex::new(Vec::new()));
        let redraw = history.accept_color_commits(Arc::clone(&commits));
        self.history_commits = Some(ColorCommitSink { colors: commits, redraw });
    }

    /// Breaks the pairing, so later commits no longer reach the history.
    pub fn detach_history(&mut self) {
        self.history_commits = None;
    }

    /// Whether this well is paired with a history.
    pub fn is_history_attached(&self) -> bool {
        self.history_commits.is_some()
    }

    /// Commits `color`: assigns it and records it with the paired history, when one is attached.
    ///
    /// A commit is a deliberate choice (a picker result, a drop, a press on the well), which is
    /// exactly the moment the recent-colour band should grow. Re-committing the current colour is
    /// allowed; the history itself skips a repeated newest entry.
    pub fn commit_color(&mut self, color: Color) {
        self.set_color(color);
        self.forward_commit(color);
    }

    /// Commits the colour the well currently shows.
    pub fn commit(&mut self) {
        self.forward_commit(self.color);
    }

    /// Writes `color` into the paired history's commit stream.
    fn forward_commit(&self, color: Color) {
        let Some(sink) = self.history_commits.as_ref() else {
            return;
        };
        lock(&sink.colors).push(color);
        // Wake the history, so its drain-on-paint sees the commit in the very next frame.
        sink.redraw.emit();
    }

    /// Returns whether the border is visible.
    pub fn show_border(&self) -> bool {
        self.show_border
    }

    /// Sets whether the border is visible and requests a redraw.
    pub fn set_show_border(&mut self, show_border: bool) {
        if self.show_border != show_border {
            self.show_border = show_border;
            self.base.request_redraw();
        }
    }

    /// Rebuilds the transparency grid when either tone or the well's size changed.
    ///
    /// The plan carries the same rectangles the per-tile fills used, so the picture is identical
    /// to the old loop while the per-frame work is one pass over the cached tiles.
    fn ensure_checkerboard(&mut self, even: Color, odd: Color, width: u32, height: u32) {
        let fresh = self.checkerboard.as_ref().is_some_and(|cache| {
            cache.even == even && cache.odd == odd && cache.width == width && cache.height == height
        });
        if fresh {
            return;
        }
        let mut tiles = crate::compat::Vec::new();
        let mut y = 0u32;
        while y < height {
            let tile_h = CHECKER_TILE.min(height - y);
            let mut x = 0u32;
            while x < width {
                let tile_w = CHECKER_TILE.min(width - x);
                let light = ((x / CHECKER_TILE) + (y / CHECKER_TILE)).is_multiple_of(2);
                let color = if light { even } else { odd };
                tiles.push((x as i32, y as i32, tile_w, tile_h, color));
                x += tile_w;
            }
            y += tile_h;
        }
        self.checkerboard = Some(CheckerboardCache { even, odd, width, height, tiles });
    }

    /// Records a left press and returns whether it completes a double click.
    ///
    /// A completed pair clears the stored press, so three quick presses are one
    /// double click followed by a new first press rather than two overlapping pairs.
    fn is_double_press(&mut self, pos: Point) -> bool {
        let now = crate::compat::Instant::now();
        let double = self.last_press.is_some_and(|(when, previous)| {
            now.checked_duration_since(when)
                .is_some_and(|elapsed| elapsed <= Duration::from_millis(DOUBLE_CLICK_INTERVAL_MS))
                && (pos.x - previous.x).abs() <= DOUBLE_CLICK_SLOP
                && (pos.y - previous.y).abs() <= DOUBLE_CLICK_SLOP
        });
        self.last_press = if double { None } else { Some((now, pos)) };
        double
    }
}

impl Widget for ColorWell {
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
        crate::core::Size::new(40, 24)
    }

    /// Opts the well into the runtime's drag coordinator as a source.
    fn drag_source_mut(&mut self) -> Option<&mut dyn DragSource> {
        Some(self)
    }

    /// Opts the well into the runtime's drag coordinator as a drop target.
    fn drop_target_mut(&mut self) -> Option<&mut dyn DropTarget> {
        Some(self)
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
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "clicked" => Some(EventSignalRef::unit("clicked", &self.clicked)),
            "picker_requested" => {
                Some(EventSignalRef::unit("picker_requested", &self.picker_requested))
            }
            "color_changed" => Some(EventSignalRef::mapped(
                "color_changed",
                &self.color_changed,
                |color: &Color| CapabilityValue::String(color.to_hex_rgba()),
            )),
            _ => None,
        }
    }
}

/// `ColorWell`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The colour travels as the
/// `#RRGGBBAA` hex string [`Color::to_hex_rgba`] produces, and a write accepts
/// anything [`Color::parse_hex`] understands.
impl WidgetProperties for ColorWell {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The property's declared kind is `Color`, so it is read back as one. It
            // previously travelled as a `String`, which meant a caller had to know the
            // spelling and could not tell the value's type from the schema.
            "color" => Ok(CapabilityValue::Color(self.color())),
            "show_border" => Ok(CapabilityValue::Bool(self.show_border())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "color" => {
                // Accept only the declared kind. A string is still accepted because the
                // C ABI and JSON surfaces carry colours as CSS text, and both parse it
                // into `Color` before it reaches here; a bare string arriving means a
                // caller reached past those layers, and refusing it would break code
                // that has not migrated yet.
                let color = match value {
                    CapabilityValue::Color(color) => color,
                    CapabilityValue::String(raw) => {
                        let Some(color) = Color::parse_hex(&raw) else {
                            return Err(CapabilityAccessError::TypeMismatch);
                        };
                        color
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_color(color);
                Ok(())
            }
            "show_border" => {
                self.set_show_border(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["color", "show_border", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `color_well` publishes.
    ///
    /// `set_color` is the only published name and it assigns the swatch's colour, so
    /// it needs an argument a command carries none of. It is refused as
    /// [`CapabilityAccessError::OutOfRange`] — the name is right, the value belongs on
    /// the property route (`set("color", ..)`) — rather than reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_color" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for ColorWell {
    fn draw(&mut self, context: &mut RenderContext) {
        // ── The well is a fixed-size sample, not a panel ──
        //
        // A colour well is one swatch of chrome, `COLOR_WELL_SIZE` square: the same
        // object in a form row and in a full-screen cell. Every measurement below used
        // to be derived from the rectangle the caller handed over, so the 240x120 census
        // cell drew a **240x120 checkerboard** with a 176x56 swatch flooding it — a
        // 1808-rectangle image for a colour control — and the same well was a different
        // size in every layout. `ControlMetrics::centered_square` is the shared
        // derivation: one square, centred, clamped to the control rather than painted
        // outside it.
        let rect = self.geometry();
        let well = ControlMetrics::centered_square(rect, dimensions::COLOR_WELL_SIZE);
        // A control with no room for even a pixel has nothing to show: emitting a
        // zero-extent swatch would be an invisible element rather than a small one.
        if well.width == 0 || well.height == 0 {
            return;
        }

        // The well's chrome — the checkerboard that marks transparency and the
        // framing border — resolves explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously both were hardcoded, so
        // light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("color_well");
        // The swatch itself (`self.color`) is the datum the control exists to show,
        // so it is painted verbatim; only the surface behind a translucent swatch
        // and the border around it follow the appearance.
        //
        // `color_well` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — the window's own
        // colour. The well's frame is therefore a step toward the foreground, or it
        // would be indistinguishable from the window behind it.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| resolved.blend(&Color::BLACK, 0.35));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = resolved.blend(&text_color, 0.08);

        // Draw the checkerboard for transparency indication over the **well's own box**:
        // two steps of the resolved surface, so a translucent swatch composites over a
        // pattern that follows the appearance rather than a fixed grey-and-white pair.
        let even = background;
        let odd = background.blend(&border, 0.25);
        self.ensure_checkerboard(even, odd, well.width, well.height);
        if let Some(cache) = self.checkerboard.as_ref() {
            for &(x, y, w, h, color) in &cache.tiles {
                context.fill_rect(Rect::new(well.x + x, well.y + y, w, h), color);
            }
        }

        // The swatch is inset inside the well rather than flooding it. The swatch is
        // the datum the control exists to show, so it is painted verbatim — but a
        // control whose *entire* area is caller data has no chrome left to follow an
        // appearance switch, and the well reads as a bare colour rather than as a
        // colour *control*. The housing around the swatch is what makes the well a
        // control: it is the resolved surface, so it follows the appearance, and it
        // is large enough to be the colour the eye reads first.
        //
        // Both the inset and the housing are measured from the **well**, not from the
        // control: an inset taken from the control's shortest side was 40 px in the
        // census cell, which is why the swatch below it was a 176x56 slab rather than a
        // square sample.
        let inset =
            (well.width.min(well.height) / 3).clamp(3, 32).min(well.width.min(well.height) / 2);
        let swatch = Rect::new(
            well.x + inset as i32,
            well.y + inset as i32,
            well.width.saturating_sub(inset * 2),
            well.height.saturating_sub(inset * 2),
        );
        // A translucent swatch composites over the checkerboard; an opaque one
        // covers it. Either way the surrounding housing stays the resolved surface.
        context.fill_rect(swatch, self.color);

        // The housing between the well's edge and the swatch is the resolved surface,
        // drawn last so it reads as the control's chrome rather than as a hole. Each strip
        // is derived from the well's own box, so the four of them tile it exactly.
        context.fill_rect(Rect::new(well.x, well.y, well.width, inset), background);
        context.fill_rect(
            Rect::new(well.x, swatch.y + swatch.height as i32, well.width, inset),
            background,
        );
        context.fill_rect(Rect::new(well.x, swatch.y, inset, swatch.height), background);
        context.fill_rect(
            Rect::new(well.x + well.width as i32 - inset as i32, swatch.y, inset, swatch.height),
            background,
        );

        // Draw border if enabled
        if self.show_border {
            context.draw_rect_stroke(well, border, 1);
        }

        // Hover and press are painted as a ring and a faint wash, not a different
        // swatch: the colour is caller data and must stay readable. Both states are
        // opt-in by interaction, so an untouched well paints exactly the same bytes
        // as before.
        let hovered = self.base.is_hovered();
        let pressed = self.base.is_pressed();
        if hovered || pressed {
            let accent = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.primary)
                .unwrap_or_else(|| border.blend(&text_color, 0.5));
            if pressed {
                context.fill_rounded_rect(well, 2, accent.with_alpha(40));
                crate::widget::draw::stroke_surface_border(context, well, [2, 2, 2, 2], accent, 2);
            } else {
                crate::widget::draw::stroke_surface_border(
                    context,
                    well,
                    [2, 2, 2, 2],
                    accent.with_alpha(160),
                    2,
                );
            }
        }
    }
}

impl EventHandler for ColorWell {
    /// Emits `clicked` only for a press that lands on the well, and tracks the
    /// hover/press state the paint path reads.
    ///
    /// The position used to be discarded (`pos: _`), so a left press **anywhere**
    /// emitted this control's `clicked` — a press on a neighbouring control fired the
    /// well's colour-changed path. 43 other widgets in this crate guard the press with
    /// [`Rect::contains_point`](crate::core::Rect::contains_point); this one now does too.
    ///
    /// A second press within the double-click window additionally emits
    /// [`Self::picker_requested`]; the ordinary click is still emitted for that press,
    /// because the two are separate questions ("was I pressed" and "should a chooser
    /// open").
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        let was_hovered = self.base.is_hovered();
        let was_pressed = self.base.is_pressed();
        self.base.handle_event(event);
        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == 1 && self.geometry().contains_point(*pos) {
                    if self.is_double_press(*pos) {
                        self.picker_requested.emit();
                    }
                    self.clicked.emit();
                    // A press on the well selects the colour it shows, which is a commit
                    // gesture rather than a preview (D-026).
                    self.commit();
                } else if *button == 1 {
                    // A press elsewhere ends the gesture, so the next press here
                    // is the first of a new pair rather than the second of a stale
                    // one.
                    self.last_press = None;
                }
            }
            // A pointer that leaves the well abandons the pressed look even though
            // the release will still be routed here.
            Event::MouseLeave { .. } => {
                self.base.set_pressed(false);
            }
            _ => {}
        }
        if was_hovered != self.base.is_hovered() || was_pressed != self.base.is_pressed() {
            self.base.request_redraw();
        }
    }
}

/// A press on the well carries its colour as a `"color"` payload.
///
/// The item id is the `#RRGGBBAA` spelling [`Color::to_hex_rgba`] produces, so a
/// target reads the value back with [`Color::parse_hex`] and no shared memory is
/// needed between the two ends of the drag. The drag is a copy: the well keeps its
/// colour whatever the target reports.
impl DragSource for ColorWell {
    fn drag_payload(&self, pos: Point) -> Option<DragPayload> {
        if !self.base.is_enabled() || !self.geometry().contains_point(pos) {
            return None;
        }
        let hex = self.color.to_hex_rgba();
        Some(DragPayload::new(COLOR_PAYLOAD_TYPE, hex.clone()).with_label(hex).with_origin(pos))
    }
}

/// A `"color"` payload dropped on the well becomes its colour.
///
/// The effect is [`DropEffect::Copy`]: the source keeps its own colour, because a
/// well is a value display rather than a container of the dragged item.
impl DropTarget for ColorWell {
    fn can_accept(&self, payload: &DragPayload) -> bool {
        self.base.is_enabled()
            && payload.is_type(COLOR_PAYLOAD_TYPE)
            && Color::parse_hex(&payload.item_id).is_some()
    }

    fn on_drop(&mut self, payload: &DragPayload, _pos: Point) -> DropEffect {
        if !self.can_accept(payload) {
            return DropEffect::None;
        }
        let Some(color) = Color::parse_hex(&payload.item_id) else {
            return DropEffect::None;
        };
        // A drop is a final choice, so it commits rather than merely previews.
        self.commit_color(color);
        DropEffect::Copy
    }

    /// Highlights the well's own square, which is where the colour is shown.
    fn preview_rect(&self, payload: &DragPayload, _pos: Point) -> Option<Rect> {
        if !self.can_accept(payload) {
            return None;
        }
        Some(ControlMetrics::centered_square(self.geometry(), dimensions::COLOR_WELL_SIZE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Duration;
    use crate::core::Point;
    use crate::event::dnd::{DragPayload, DragSource, DropEffect, DropTarget};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn color_well_default_color() {
        let cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        assert_eq!(cw.color(), Color::RED);
        assert!(cw.show_border());
        assert_eq!(cw.kind(), WidgetKind::ColorWell);
    }

    #[test]
    fn color_well_set_color() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        cw.set_color(Color::BLUE);
        assert_eq!(cw.color(), Color::BLUE);
    }

    /// D-026: only a committed colour enters the paired recent-colour band.
    #[test]
    fn a_committed_colour_reaches_the_paired_history() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let mut history = ColorHistory::new(Rect::new(0, 0, 200, 80));
        cw.attach_history(&mut history);
        assert!(cw.is_history_attached());

        // A preview assignment changes the well but leaves the history untouched.
        cw.set_color(Color::BLUE);
        assert_eq!(history.sync_committed_colors(), 0);
        assert!(history.colors().is_empty(), "a preview is not a commit");

        // A commit enters the band.
        cw.commit_color(Color::GREEN);
        assert_eq!(history.sync_committed_colors(), 1);
        assert_eq!(history.colors(), &[Color::GREEN]);

        // A press on the well commits the colour it currently shows.
        cw.set_color(Color::BLUE);
        cw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert_eq!(history.sync_committed_colors(), 1);
        assert_eq!(history.colors(), &[Color::GREEN, Color::BLUE]);

        // A detached well queues nothing.
        cw.detach_history();
        assert!(!cw.is_history_attached());
        cw.commit_color(Color::RED);
        assert_eq!(history.sync_committed_colors(), 0);
    }

    #[test]
    fn color_well_signal_emitted_on_click() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let clicked = Arc::new(AtomicBool::new(false));
        cw.clicked.connect({
            let clicked = Arc::clone(&clicked);
            move || {
                clicked.store(true, Ordering::SeqCst);
            }
        });

        cw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn color_well_border_toggle() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        assert!(cw.show_border());

        cw.set_show_border(false);
        assert!(!cw.show_border());

        cw.set_show_border(true);
        assert!(cw.show_border());
    }

    #[test]
    fn color_well_disabled_blocks_events() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let clicked = Arc::new(AtomicBool::new(false));
        cw.clicked.connect({
            let clicked = Arc::clone(&clicked);
            move || {
                clicked.store(true, Ordering::SeqCst);
            }
        });

        cw.set_enabled(false);
        cw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn color_well_svg_output() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let svg = crate::widget::svg::render_to_svg(&mut cw);
        assert!(svg.starts_with("<svg"));
    }

    /// A colour well is a fixed-size sample, not a panel.
    ///
    /// This pins the defect the fix removes: the checkerboard and the swatch were both
    /// measured from the control's rectangle, so the 240x120 census cell drew a
    /// **240x120 chequerboard** (1808 emitted rectangles) with a 176x56 swatch flooding
    /// it. The well is now `COLOR_WELL_SIZE` square and centred, so the same control is
    /// the same object in a form row and in a full-screen cell.
    #[test]
    fn the_well_is_a_fixed_square_in_any_rectangle() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        for rect in [Rect::new(0, 0, 240, 120), Rect::new(0, 0, 300, 300)] {
            let mut cw = ColorWell::new(Color::RED, rect);
            let svg = crate::widget::svg::render_to_svg(&mut cw);
            let expected = ControlMetrics::centered_square(rect, dimensions::COLOR_WELL_SIZE);
            assert_eq!(expected.width, dimensions::COLOR_WELL_SIZE, "at {rect:?}");
            assert_eq!(expected.height, dimensions::COLOR_WELL_SIZE, "at {rect:?}");
            // The border is the well's own outline, and every emitted tile is inside it. An
            // outline is inset by half its stroke so the whole rule lies inside the box (SVG
            // centres a stroke on its path), so the expectation is built by the same helper the
            // backend uses rather than from the raw box.
            let outline = crate::render::svg::convert::outline_rect_attrs(&expected, 1);
            assert!(
                svg.contains(&format!("<rect {outline} fill=\"none\"")),
                "the well is outlined at its own box: {svg}"
            );
            // The defect's signature: a canvas-sized chequerboard. The control's own
            // tiles are 4 px; a tile at the control's top-left corner would mean the
            // chequerboard was measured from `rect` rather than from the well.
            if rect.height > dimensions::COLOR_WELL_SIZE {
                assert!(
                    !svg.contains(&format!("<rect x=\"{}\" y=\"0\" width=\"4\"", rect.x)),
                    "the chequerboard must start at the well, not the control's top edge: {svg}"
                );
            }
        }
        // A control smaller than the well clamps it rather than painting outside.
        let mut small = ColorWell::new(Color::RED, Rect::new(0, 0, 20, 20));
        let svg = crate::widget::svg::render_to_svg(&mut small);
        assert!(svg.contains("width=\"20\" height=\"20\""), "the well clamps to 20x20: {svg}");
    }

    /// Two presses inside the window and the slop are one double click.
    #[test]
    fn color_well_double_click_requests_the_picker() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let fired = Arc::new(AtomicBool::new(false));
        cw.picker_requested.connect({
            let fired = Arc::clone(&fired);
            move || fired.store(true, Ordering::SeqCst)
        });

        // The first press of the pair, as the clock would have recorded it.
        cw.last_press =
            Some((crate::compat::Instant::now() - Duration::from_millis(100), Point::new(10, 10)));
        cw.handle_event(&Event::mouse_press(12, 12, 1));

        assert!(fired.load(Ordering::SeqCst), "the second press opens the picker");
        assert!(cw.last_press.is_none(), "a completed pair does not arm a third press");
    }

    /// A slow or distant press is the start of a new gesture, not a double click.
    #[test]
    fn color_well_slow_or_far_presses_are_not_a_double_click() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let fired = Arc::new(AtomicBool::new(false));
        cw.picker_requested.connect({
            let fired = Arc::clone(&fired);
            move || fired.store(true, Ordering::SeqCst)
        });

        // Outside the 400 ms window.
        cw.last_press =
            Some((crate::compat::Instant::now() - Duration::from_millis(500), Point::new(10, 10)));
        cw.handle_event(&Event::mouse_press(10, 10, 1));
        assert!(!fired.load(Ordering::SeqCst), "a slow press is not a double click");

        // Outside the 4 px slop, but promptly.
        cw.last_press =
            Some((crate::compat::Instant::now() - Duration::from_millis(100), Point::new(10, 10)));
        cw.handle_event(&Event::mouse_press(20, 20, 1));
        assert!(!fired.load(Ordering::SeqCst), "a distant press is not a double click");
    }

    /// A drag from the well carries the colour as `#RRGGBBAA` under `"color"`.
    #[test]
    fn color_well_drag_source_carries_the_hex_colour() {
        let cw = ColorWell::new(Color::rgba(0x11, 0x22, 0x33, 0x44), Rect::new(0, 0, 40, 40));
        let payload =
            cw.drag_payload(Point::new(10, 10)).expect("a press on the well starts a drag");
        assert_eq!(payload.type_id, COLOR_PAYLOAD_TYPE);
        assert_eq!(payload.item_id, "#11223344");
        assert_eq!(payload.label, "#11223344");
        // A press outside the well starts nothing.
        assert!(cw.drag_payload(Point::new(100, 100)).is_none());
    }

    /// A `"color"` payload dropped on the well is accepted and becomes its colour.
    #[test]
    fn color_well_drop_target_accepts_a_colour() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let payload = DragPayload::new(COLOR_PAYLOAD_TYPE, "#00FF0080");
        assert!(cw.can_accept(&payload));
        assert_eq!(cw.on_drop(&payload, Point::new(10, 10)), DropEffect::Copy);
        assert_eq!(cw.color(), Color::rgba(0, 255, 0, 128));
        assert!(cw.preview_rect(&payload, Point::new(10, 10)).is_some());
    }

    /// Another payload type, or an unparseable colour, is refused.
    #[test]
    fn color_well_drop_target_refuses_other_payloads() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let text = DragPayload::new("text/plain", "#00FF00FF");
        assert!(!cw.can_accept(&text));
        assert_eq!(cw.on_drop(&text, Point::new(10, 10)), DropEffect::None);

        let broken = DragPayload::new(COLOR_PAYLOAD_TYPE, "not-a-colour");
        assert!(!cw.can_accept(&broken));
        assert_eq!(cw.color(), Color::RED, "a refused drop must not change the colour");
    }

    /// Hover and press are opt-in painting states: an idle well is unchanged.
    #[test]
    fn color_well_hover_and_press_paint_feedback() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let idle = crate::widget::svg::render_to_svg(&mut cw);

        cw.handle_event(&Event::mouse_enter(10, 10));
        let hovered = crate::widget::svg::render_to_svg(&mut cw);
        assert_ne!(idle, hovered, "hover paints a ring");

        cw.handle_event(&Event::mouse_press(10, 10, 1));
        let pressed = crate::widget::svg::render_to_svg(&mut cw);
        assert_ne!(hovered, pressed, "a press paints its own state");

        cw.handle_event(&Event::mouse_leave(100, 100));
        cw.handle_event(&Event::mouse_release(100, 100, 1));
        let released = crate::widget::svg::render_to_svg(&mut cw);
        assert_eq!(released, idle, "leaving and releasing restores the idle drawing");
    }

    /// The transparency grid plan is rebuilt only when a tone or the size changes.
    #[test]
    fn the_checkerboard_is_a_cached_plan_not_a_per_tile_fill() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut cw = ColorWell::new(Color::rgba(255, 0, 0, 128), Rect::new(0, 0, 40, 40));
        cw.ensure_checkerboard(Color::WHITE, Color::rgb(220, 220, 220), 40, 40);
        let first = cw.checkerboard.as_ref().expect("the grid is built");
        // 40x40 at a 4 px tile is 10x10 tiles, and the plan covers the whole box.
        assert_eq!(first.tiles.len(), 100);
        assert_eq!(first.tiles[0].0, 0);
        assert_eq!(first.tiles[0].1, 0);
        assert_eq!(first.tiles[0].2, CHECKER_TILE);
        assert_eq!(first.tiles[0].3, CHECKER_TILE);
        let first_ptr = first.tiles.as_ptr();
        // The same tones and size reuse the same allocation.
        cw.ensure_checkerboard(Color::WHITE, Color::rgb(220, 220, 220), 40, 40);
        assert_eq!(
            cw.checkerboard.as_ref().unwrap().tiles.as_ptr(),
            first_ptr,
            "an unchanged grid must not be rebuilt"
        );
        // A changed tone rebuilds it.
        cw.ensure_checkerboard(Color::rgb(250, 250, 250), Color::rgb(220, 220, 220), 40, 40);
        assert_ne!(cw.checkerboard.as_ref().unwrap().tiles.as_ptr(), first_ptr);
    }
}
