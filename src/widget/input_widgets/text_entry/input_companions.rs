// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Companion controls shared by the input-widget family (COMP-C01..COMP-C05).
//!
//! Each control here is the *partner* of a primary input control rather than a standalone
//! feature: a chip for `tag_input`, stepper buttons for `spin_box`/`number_picker`, a scrollbar
//! indicator for the editors, and a format toolbar for `rich_edit`.
//!
//! # SuggestionPopup lives in `suggestion_popup`
//!
//! COMP-C01's candidate-list host and COMP-B01's candidate-list widget were two implementations of
//! one concept, so they were merged into `super::suggestion_popup` (principle #101). This module
//! re-exports the merged [`SuggestionPopup`], [`PopupItem`] and [`MatchMode`] so COMP-C01's
//! documented path still resolves, while filtering, flipping, the scroll window and outside
//! dismissal are implemented exactly once.
//!
//! # Registration
//!
//! This module is written to be re-exported from `src/widget/companions.rs`. Until that line is
//! added (it is owned by the companion surface, not by the input controls), the types are only
//! reachable through `crate::widget::input_widgets::input_companions`.
//!
//! # Safety
//!
//! These controls contain no `unsafe` code and own all of their state. Geometry and numeric
//! inputs are validated and clamped in safe Rust.

use crate::compat::{MiniToString, String};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::metrics::{estimate_text_width, role_font};
use crate::widget::Draw;

/// The shared swipe-gesture kernel used by `SwipeActions` and `SwipeToDismiss` (COMP-A03).
///
/// # Why it lives here
///
/// A swiped row is one gesture with two presentations: a background that reveals actions
/// (`SwipeActions`) and a wrapper that dismisses its child (`SwipeToDismiss`). Each used to
/// spell the threshold, the rubber-band damping, the flick rule and the settle out of its own
/// copy, which is the shape where one copy drifts and the two controls stop agreeing about
/// what "the same drag" means. The model is written once here and both controls consume it.
///
/// Gated on `full_widgets` because that is the only profile either consumer is compiled in;
/// `mini`/`embedded` never build this code and must not carry it.
#[cfg(full_widgets)]
pub(crate) mod swipe_kernel {
    use crate::style::{MotionSlot, PropertyDriver};

    /// The distance a kernel threshold is clamped to, in logical pixels.
    ///
    /// A zero threshold would make every tap a decision and a negative one would make the
    /// comparison meaningless, so both resolve to the nearest legal value.
    const MIN_THRESHOLD: f32 = 1.0;

    fn legal_threshold(threshold: f32) -> f32 {
        if threshold.is_finite() {
            threshold.max(MIN_THRESHOLD)
        } else {
            MIN_THRESHOLD
        }
    }

    /// What a released swipe decided.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum SwipeCommit {
        /// The travel, or its velocity, passed the threshold; the gesture commits.
        Commit,
        /// The gesture stayed below both the distance and the flick rule; it rebounds.
        Rebound,
    }

    /// The distance/velocity model shared by both swipe controls.
    ///
    /// `threshold` is the travel at which the gesture commits. `SwipeToDismiss` passes its
    /// dismiss distance; `SwipeActions` passes the span its reveal band covers, so "the whole
    /// action set is exposed" and "the row would dismiss" are the same measurement.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(crate) struct SwipeKernel {
        threshold: f32,
        damping: f32,
    }

    impl SwipeKernel {
        /// How much of the travel past the threshold a drag keeps.
        pub(crate) const DEFAULT_DAMPING: f32 = 0.35;
        /// The pointer speed, in pixels per millisecond, that counts as a flick.
        pub(crate) const FLICK_VELOCITY: f32 = 0.5;
        /// The shortest travel a flick may commit from, as a fraction of the threshold.
        pub(crate) const FLICK_FLOOR_RATIO: f32 = 0.25;
        /// The largest distance, in pixels, that the flick floor may reach.
        pub(crate) const FLICK_FLOOR_MAX: f32 = 40.0;
        /// The frame length a pointer sample is priced against, in milliseconds.
        ///
        /// The platform event carries no timestamp, and the exact wall-clock rate does not
        /// matter for the "was this a flick" decision, so both controls price a sample the
        /// same way rather than each inventing a different rate.
        pub(crate) const NOMINAL_FRAME_MS: f32 = 16.0;

        /// Builds a kernel whose `threshold` is the distance that commits a gesture.
        pub(crate) fn new(threshold: f32) -> Self {
            Self { threshold: legal_threshold(threshold), damping: Self::DEFAULT_DAMPING }
        }

        /// The distance at which a gesture commits.
        pub(crate) fn threshold(&self) -> f32 {
            self.threshold
        }

        /// Sets the commit distance; a non-finite write is ignored.
        ///
        /// Returns whether the value changed, so a caller can decide whether to repaint.
        pub(crate) fn set_threshold(&mut self, threshold: f32) -> bool {
            if !threshold.is_finite() {
                return false;
            }
            let threshold = legal_threshold(threshold);
            if (self.threshold - threshold).abs() < f32::EPSILON {
                return false;
            }
            self.threshold = threshold;
            true
        }

        /// The signed travel a drag of `raw` pixels is drawn at.
        ///
        /// Past the threshold the row keeps only [`Self::DEFAULT_DAMPING`] of each further
        /// pixel: an over-drag gets resistance instead of dragging the row off its own row.
        pub(crate) fn damp(&self, raw: f32) -> f32 {
            if !raw.is_finite() {
                return 0.0;
            }
            let magnitude = raw.abs();
            if magnitude <= self.threshold {
                return raw;
            }
            raw.signum() * (self.threshold + (magnitude - self.threshold) * self.damping)
        }

        /// How far a gesture has travelled toward its threshold, in `0.0..=1.0`.
        pub(crate) fn progress(&self, offset: f32) -> f32 {
            if !offset.is_finite() {
                return 0.0;
            }
            (offset.abs() / self.threshold).clamp(0.0, 1.0)
        }

        /// The distance an `offset` reveals toward one side, in pixels.
        ///
        /// `toward_positive` selects the side: a positive offset reveals the leading band, a
        /// negative one the trailing band. The result is the clamp of the travel to the
        /// threshold, which is the same span the reveal geometry is laid out in.
        pub(crate) fn reveal(&self, offset: f32, toward_positive: bool) -> f32 {
            if !offset.is_finite() {
                return 0.0;
            }
            let travels_that_way = if toward_positive { offset > 0.0 } else { offset < 0.0 };
            if !travels_that_way {
                return 0.0;
            }
            self.progress(offset) * self.threshold
        }

        /// The pointer velocity between two samples, in pixels per millisecond.
        pub(crate) fn velocity(previous_x: f32, x: f32) -> f32 {
            if !previous_x.is_finite() || !x.is_finite() {
                return 0.0;
            }
            (x - previous_x) / Self::NOMINAL_FRAME_MS
        }

        /// Whether `offset` alone has reached the threshold.
        pub(crate) fn crosses(&self, offset: f32) -> bool {
            offset.is_finite() && offset.abs() >= self.threshold
        }

        /// The decision a release at `offset` moving at `velocity` makes.
        ///
        /// Distance **or** a flick commits. The flick floor stops a fast tap from dismissing a
        /// row it never really dragged, which is the rule both controls now share.
        pub(crate) fn decision(&self, offset: f32, velocity: f32) -> SwipeCommit {
            if !offset.is_finite() {
                return SwipeCommit::Rebound;
            }
            let distance = offset.abs();
            let floor =
                (self.threshold * Self::FLICK_FLOOR_RATIO).clamp(1.0, Self::FLICK_FLOOR_MAX);
            let flicked =
                velocity.is_finite() && velocity.abs() > Self::FLICK_VELOCITY && distance >= floor;
            if distance >= self.threshold || flicked {
                SwipeCommit::Commit
            } else {
                SwipeCommit::Rebound
            }
        }
    }

    /// The rebound half of the kernel: the residual a released gesture travels back through.
    ///
    /// The driver is always priced by the theme's `normal` motion, so a themed tempo reaches
    /// the spring-back; at rest its value is exactly zero, which is what keeps a settled
    /// picture identical to an untouched one.
    pub(crate) struct SwipeSettle {
        driver: PropertyDriver,
    }

    impl SwipeSettle {
        /// A settle driver resting at zero.
        pub(crate) fn new() -> Self {
            Self { driver: PropertyDriver::at(0.0, MotionSlot::Normal) }
        }

        /// Places the residual at `value` without animating, for the release frame.
        pub(crate) fn jump_to(&mut self, value: f32) {
            self.driver.jump_to(value);
        }

        /// Aims the residual at `target`.
        pub(crate) fn aim(&mut self, target: f32) {
            self.driver.set_target(target);
        }

        /// Places the residual at `offset` and aims it according to `decision`.
        ///
        /// A rebound travels back to zero; a commit leaves the residual where the gesture
        /// released it, so whatever carries the departure continues from that point.
        pub(crate) fn release(&mut self, offset: f32, decision: SwipeCommit) {
            self.jump_to(offset);
            if decision == SwipeCommit::Rebound {
                self.aim(0.0);
            }
        }

        /// The current residual, in pixels.
        pub(crate) fn value(&self) -> f32 {
            self.driver.value()
        }

        /// Whether the residual is still travelling.
        pub(crate) fn is_moving(&self) -> bool {
            self.driver.is_moving()
        }

        /// Advances the residual by `delta_ms`, reporting whether it is still travelling.
        pub(crate) fn tick(&mut self, delta_ms: u32) -> bool {
            self.driver.tick(delta_ms)
        }
    }

    impl Default for SwipeSettle {
        fn default() -> Self {
            Self::new()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_kernel_clamps_a_threshold_to_at_least_one_pixel() {
            let mut kernel = SwipeKernel::new(0.0);
            assert_eq!(kernel.threshold(), 1.0);
            assert!(!kernel.set_threshold(f32::NAN), "a non-finite write is ignored");
            assert_eq!(kernel.threshold(), 1.0);
            assert!(kernel.set_threshold(64.0));
            assert_eq!(kernel.threshold(), 64.0);
            assert!(!kernel.set_threshold(64.0), "an unchanged write reports no change");
        }

        #[test]
        fn travel_past_the_threshold_is_damped_and_below_it_is_not() {
            let kernel = SwipeKernel::new(100.0);
            assert_eq!(kernel.damp(80.0), 80.0);
            assert_eq!(kernel.damp(-80.0), -80.0);
            assert_eq!(kernel.damp(140.0), 100.0 + 40.0 * SwipeKernel::DEFAULT_DAMPING);
            assert_eq!(kernel.damp(-140.0), -(100.0 + 40.0 * SwipeKernel::DEFAULT_DAMPING));
        }

        #[test]
        fn progress_and_reveal_clamp_to_the_threshold_span() {
            let kernel = SwipeKernel::new(100.0);
            assert_eq!(kernel.progress(50.0), 0.5);
            assert_eq!(kernel.progress(-400.0), 1.0);
            assert!(
                (kernel.reveal(40.0, true) - 40.0).abs() < 0.001,
                "a travel inside the span reveals exactly itself"
            );
            assert_eq!(
                kernel.reveal(40.0, false),
                0.0,
                "a positive offset never reveals the other side"
            );
            assert_eq!(kernel.reveal(-400.0, false), 100.0);
        }

        #[test]
        fn a_release_commits_on_distance_or_on_a_flick_above_the_floor() {
            let kernel = SwipeKernel::new(100.0);
            assert_eq!(kernel.decision(99.0, 0.0), SwipeCommit::Rebound);
            assert_eq!(kernel.decision(100.0, 0.0), SwipeCommit::Commit);
            // A flick needs both speed and the flick floor (25 px here).
            assert_eq!(kernel.decision(24.0, 10.0), SwipeCommit::Rebound);
            assert_eq!(kernel.decision(25.0, 10.0), SwipeCommit::Commit);
            assert_eq!(kernel.decision(60.0, 0.1), SwipeCommit::Rebound);
        }

        #[test]
        fn the_settle_rebinds_a_rebound_and_holds_a_commit() {
            let mut settle = SwipeSettle::new();
            settle.release(-60.0, SwipeCommit::Rebound);
            assert_eq!(settle.value(), -60.0);
            assert!(settle.is_moving());
            let mut guard = 64;
            while guard > 0 && settle.tick(16) {
                guard -= 1;
            }
            assert!(guard > 0, "the spring-back must terminate");
            assert!(settle.value().abs() < 0.01, "a settled rebound rests at zero");
            assert!(!settle.is_moving());

            settle.release(-60.0, SwipeCommit::Commit);
            assert_eq!(settle.value(), -60.0);
            assert!(!settle.is_moving(), "a commit does not travel back");
        }
    }
}

/// The theme style a model-only companion paints with.
///
/// These controls own no `BaseWidget`, so their colours come from the active theme's
/// resolution for the control they partner with. A profile without a theme resolves to an
/// empty style, and each painter then falls back to its documented neutral colours.
fn companion_style(control: &str) -> WidgetStyle {
    crate::style::resolved_theme_style_shared(control)
        .map(|style| (*style).clone())
        .unwrap_or_default()
}

/// One palette read shared by the painters, with neutral fallbacks.
fn companion_colors() -> (Color, Color, Color) {
    let palette = crate::style::palette_shared();
    let surface = palette
        .as_ref()
        .map(|colors| colors.surface_container)
        .unwrap_or(Color::rgb(245, 246, 248));
    let ink = palette.as_ref().map(|colors| colors.foreground).unwrap_or(Color::BLACK);
    let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
    (surface, ink, accent)
}

// COMP-C01's single home. Re-exporting rather than re-defining is what keeps one implementation
// path for the concept (principle #101): the merge is announced here and in `suggestion_popup`.
pub use super::suggestion_popup::{MatchMode, PopupItem, SuggestionPopup};

// ───────────────────────────── COMP-C02 Chip ─────────────────────────────

/// Which side of a chip a drop indicator marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropSide {
    /// Before the chip.
    Before,
    /// After the chip.
    After,
}

/// A tag/token chip with an optional remove affordance and drag state (COMP-C02).
///
/// The chip owns one measured ruler ([`Self::intrinsic_width`]) so a host lays a row out, paints
/// it and hit-tests it with the same number.
pub struct Chip {
    label: String,
    removable: bool,
    chip_radius: u32,
    close_radius: u32,
    selected: bool,
    accent: Option<Color>,
    draggable: bool,
    dragging: bool,
    drop_indicator: Option<DropSide>,
    drag_from: usize,
    /// The box this chip paints into when a host draws it directly through [`Draw`].
    ///
    /// A host that owns the layout still passes its own rectangle to
    /// [`Self::press_at`] and [`Self::close_center`]; this field lets the same chip also be
    /// a paint target for the SVG exporter and for a caller that holds it as a widget-like
    /// object.
    bounds: Rect,
    /// Emitted on a press of the chip body.
    pub chip_clicked: GenericSignal,
    /// Emitted when the close affordance is pressed.
    pub remove_requested: GenericSignal,
    /// Emitted with `(from, to)` when a drag finishes.
    pub drag_finished: Signal1<(usize, usize)>,
}

impl Chip {
    /// The corner radius a host should paint this chip with.
    ///
    /// The companion owns no paint surface of its own; it states the radius so every host that
    /// draws a chip (tag input, filter row, token field) uses one value instead of a local literal.
    pub fn chip_radius(&self) -> u32 {
        self.chip_radius
    }

    /// Overrides the painted corner radius.
    pub fn set_chip_radius(&mut self, radius: u32) {
        self.chip_radius = radius;
    }
    /// Creates a removable chip for `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            removable: true,
            chip_radius: 12,
            close_radius: 7,
            selected: false,
            accent: None,
            draggable: false,
            dragging: false,
            drop_indicator: None,
            drag_from: 0,
            bounds: Rect::new(0, 0, 0, 0),
            chip_clicked: GenericSignal::new(),
            remove_requested: GenericSignal::new(),
            drag_finished: Signal1::new(),
        }
    }

    /// Sets the label.
    pub fn set_label(&mut self, label: &str) {
        self.label = label.to_string();
    }

    /// The label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Sets the box this chip paints into when drawn through [`Draw`].
    ///
    /// The box is the caller's, not the chip's: a chip is laid out by the row that owns it
    /// (a `tag_input`, a filter row), so the paint entry accepts the rectangle it was given
    /// rather than inventing one. This stores that rectangle for the `Draw` path.
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    /// The box this chip paints into through [`Draw`].
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Paints the chip into `rect`.
    ///
    /// The close affordance uses [`Self::close_center`], the same geometry
    /// [`Self::press_at`] hit-tests, so what the user sees and what the control accepts
    /// cannot disagree.
    pub fn paint(&self, context: &mut RenderContext, rect: Rect) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = companion_style("tag_input");
        let font = role_font(&style, 12.0);
        let (surface, ink, theme_accent) = companion_colors();
        let accent = self.accent.unwrap_or(theme_accent);
        let fill = if self.selected {
            accent
        } else if self.dragging {
            accent.with_alpha(40)
        } else {
            surface
        };
        let border =
            if self.selected || self.dragging { accent } else { surface.blend(&ink, 0.35) };
        let radius = self.chip_radius.min(rect.height / 2).max(1);
        context.fill_rounded_rect(rect, radius, fill);
        context.draw_rounded_rect_stroke(rect, radius, border, 1);

        let text_ink = if self.selected { accent.contrast_color() } else { ink };
        let close_band = if self.removable { self.close_radius * 2 + 12 } else { 0 };
        let text_width = rect.width.saturating_sub(12 + close_band);
        if text_width > 0 {
            context.draw_text_line_cow(
                Rect::new(rect.x + 6, rect.y, text_width, rect.height),
                &self.label,
                &font,
                text_ink,
                HorizontalAlignment::Left,
            );
        }
        if let Some(center) = self.close_center(rect) {
            let reach = self.close_radius.min(rect.height / 2) as i32;
            let mark = if self.selected { accent.contrast_color() } else { ink.with_alpha(160) };
            context.draw_line(
                Point::new(center.x - reach, center.y - reach),
                Point::new(center.x + reach, center.y + reach),
                mark,
            );
            context.draw_line(
                Point::new(center.x + reach, center.y - reach),
                Point::new(center.x - reach, center.y + reach),
                mark,
            );
        }
        if let Some(side) = self.drop_indicator {
            let x = match side {
                DropSide::Before => rect.x,
                DropSide::After => rect.right(),
            };
            context.fill_rect(Rect::new(x.saturating_sub(1), rect.y, 2, rect.height), accent);
        }
    }

    /// Sets whether the close affordance is drawn.
    pub fn set_removable(&mut self, removable: bool) {
        self.removable = removable;
    }

    /// Whether the close affordance is drawn.
    pub fn removable(&self) -> bool {
        self.removable
    }

    /// Sets the selected state.
    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    /// Whether the chip is selected.
    pub fn selected(&self) -> bool {
        self.selected
    }

    /// Sets the accent; `None` derives it from the theme at paint time.
    pub fn set_accent(&mut self, accent: Option<Color>) {
        self.accent = accent;
    }

    /// The explicit accent, if any.
    pub fn accent(&self) -> Option<Color> {
        self.accent
    }

    /// Enables drag-and-drop for this chip.
    pub fn set_draggable(&mut self, draggable: bool) {
        self.draggable = draggable;
    }

    /// Marks the chip as being dragged.
    pub fn set_dragging(&mut self, dragging: bool) {
        self.dragging = dragging;
    }

    /// Whether the chip is being dragged.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Sets the drop indicator side.
    pub fn set_drop_indicator(&mut self, side: Option<DropSide>) {
        self.drop_indicator = side;
    }

    /// The drop indicator side.
    pub fn drop_indicator(&self) -> Option<DropSide> {
        self.drop_indicator
    }

    /// Begins a drag from the row index `from`.
    pub fn begin_drag(&mut self, from: usize) {
        if self.draggable {
            self.dragging = true;
            self.drag_from = from;
        }
    }

    /// Finishes a drag at row index `to`, emitting [`Self::drag_finished`].
    pub fn finish_drag(&mut self, to: usize) {
        if self.dragging {
            self.dragging = false;
            self.drag_finished.emit((self.drag_from, to));
        }
    }

    /// The chip's own width under `font`, including label, padding and close affordance.
    ///
    /// This is the single ruler the item asks for: layout, paint and hit testing all call it.
    pub fn intrinsic_width(&self, font: &Font) -> u32 {
        let text = estimate_text_width(&self.label, font, 1.0);
        let padding = 12u32;
        let close = if self.removable { self.close_radius * 2 + 4 } else { 0 };
        text + padding + close
    }

    /// The close affordance's centre inside `chip_rect`, if it is drawn.
    pub fn close_center(&self, chip_rect: Rect) -> Option<Point> {
        self.removable.then(|| {
            Point::new(
                chip_rect.x + chip_rect.width as i32 - 6 - self.close_radius as i32,
                chip_rect.y + chip_rect.height as i32 / 2,
            )
        })
    }

    /// Registers a press at `pos` inside `chip_rect`, emitting the matching signal.
    pub fn press_at(&mut self, pos: Point, chip_rect: Rect) {
        if let Some(center) = self.close_center(chip_rect) {
            let dx = (pos.x - center.x) as i64;
            let dy = (pos.y - center.y) as i64;
            let radius = self.close_radius as i64;
            if dx * dx + dy * dy <= radius * radius {
                self.remove_requested.emit();
                return;
            }
        }
        if chip_rect.contains_point(pos) {
            self.chip_clicked.emit();
        }
    }
}

impl Draw for Chip {
    fn draw(&mut self, context: &mut RenderContext) {
        // An unset box paints at the chip's own intrinsic size, so the control is a real paint
        // target even before a host lays it out. A host that sets `bounds` positions it exactly.
        let rect = if self.bounds.width == 0 || self.bounds.height == 0 {
            let style = companion_style("tag_input");
            let font = role_font(&style, 12.0);
            Rect::new(self.bounds.x, self.bounds.y, self.intrinsic_width(&font), 24)
        } else {
            self.bounds
        };
        self.paint(context, rect);
    }
}

// ───────────────────────────── COMP-C03 StepButtons ─────────────────────────────

/// How a [`StepButtons`] pair is arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepOrientation {
    /// Up above down.
    #[default]
    Vertical,
    /// Down beside up.
    Horizontal,
}

/// One direction of a stepper pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepDirection {
    /// The increment button.
    Up,
    /// The decrement button.
    Down,
}

/// A pair of stepper buttons with held-button repeat (COMP-C03).
pub struct StepButtons {
    bounds: Rect,
    orientation: StepOrientation,
    repeat_delay_ms: u32,
    repeat_interval_ms: u32,
    up_enabled: bool,
    down_enabled: bool,
    repeat_active: Option<StepDirection>,
    elapsed: u32,
    /// Emitted for each step, including each auto-repeat step.
    pub step_requested: Signal1<StepDirection>,
}

impl StepButtons {
    /// Creates an enabled vertical pair occupying `bounds`.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            orientation: StepOrientation::Vertical,
            repeat_delay_ms: 400,
            repeat_interval_ms: 120,
            up_enabled: true,
            down_enabled: true,
            repeat_active: None,
            elapsed: 0,
            step_requested: Signal1::new(),
        }
    }

    /// Moves the pair's bounds.
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    /// The pair's bounds.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Sets the arrangement.
    pub fn set_orientation(&mut self, orientation: StepOrientation) {
        self.orientation = orientation;
    }

    /// The arrangement.
    pub fn orientation(&self) -> StepOrientation {
        self.orientation
    }

    /// Sets the repeat timings.
    pub fn set_repeat(&mut self, delay_ms: u32, interval_ms: u32) {
        self.repeat_delay_ms = delay_ms;
        self.repeat_interval_ms = interval_ms.max(1);
    }

    /// Enables or disables each button; a disabled button refuses to step.
    pub fn set_enabled_pair(&mut self, up: bool, down: bool) {
        self.up_enabled = up;
        self.down_enabled = down;
        if (self.repeat_active == Some(StepDirection::Up) && !up)
            || (self.repeat_active == Some(StepDirection::Down) && !down)
        {
            self.repeat_active = None;
        }
    }

    /// Whether each button is enabled.
    pub fn enabled_pair(&self) -> (bool, bool) {
        (self.up_enabled, self.down_enabled)
    }

    /// The up button's rectangle.
    pub fn up_rect(&self) -> Rect {
        match self.orientation {
            StepOrientation::Vertical => {
                Rect::new(self.bounds.x, self.bounds.y, self.bounds.width, self.bounds.height / 2)
            }
            StepOrientation::Horizontal => {
                Rect::new(self.bounds.x, self.bounds.y, self.bounds.width / 2, self.bounds.height)
            }
        }
    }

    /// The down button's rectangle.
    pub fn down_rect(&self) -> Rect {
        match self.orientation {
            StepOrientation::Vertical => Rect::new(
                self.bounds.x,
                self.bounds.y + (self.bounds.height / 2) as i32,
                self.bounds.width,
                self.bounds.height - self.bounds.height / 2,
            ),
            StepOrientation::Horizontal => Rect::new(
                self.bounds.x + (self.bounds.width / 2) as i32,
                self.bounds.y,
                self.bounds.width - self.bounds.width / 2,
                self.bounds.height,
            ),
        }
    }

    /// Whether `pos` is on the up button.
    pub fn contains_up(&self, pos: Point) -> bool {
        self.up_rect().contains_point(pos)
    }

    /// Whether `pos` is on the down button.
    pub fn contains_down(&self, pos: Point) -> bool {
        self.down_rect().contains_point(pos)
    }

    /// The direction `pos` names.
    pub fn direction_at(&self, pos: Point) -> Option<StepDirection> {
        if self.contains_up(pos) {
            Some(StepDirection::Up)
        } else if self.contains_down(pos) {
            Some(StepDirection::Down)
        } else {
            None
        }
    }

    /// Begins a held-button repeat, emitting the first step immediately.
    pub fn begin_repeat(&mut self, direction: StepDirection) {
        let enabled = match direction {
            StepDirection::Up => self.up_enabled,
            StepDirection::Down => self.down_enabled,
        };
        if !enabled {
            return;
        }
        self.repeat_active = Some(direction);
        self.elapsed = 0;
        self.step_requested.emit(direction);
    }

    /// Stops any held-button repeat.
    pub fn end_repeat(&mut self) {
        self.repeat_active = None;
        self.elapsed = 0;
    }

    /// Whether a held-button repeat is running.
    pub fn repeat_active(&self) -> Option<StepDirection> {
        self.repeat_active
    }

    /// Advances a held-button repeat, returning whether another frame is owed.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let Some(direction) = self.repeat_active else {
            return false;
        };
        self.elapsed = self.elapsed.saturating_add(delta_ms);
        if self.elapsed < self.repeat_delay_ms {
            return true;
        }
        let mut remaining = self.elapsed - self.repeat_delay_ms;
        while remaining >= self.repeat_interval_ms {
            remaining -= self.repeat_interval_ms;
            self.step_requested.emit(direction);
        }
        self.elapsed = self.repeat_delay_ms + remaining;
        true
    }
}

impl Draw for StepButtons {
    fn draw(&mut self, context: &mut RenderContext) {
        let bounds = self.bounds;
        if bounds.width == 0 || bounds.height == 0 {
            return;
        }
        let (surface, ink, _accent) = companion_colors();
        let border = surface.blend(&ink, 0.35);
        let radius = 4.min(bounds.height / 2).max(1);
        context.fill_rounded_rect(bounds, radius, surface);
        context.draw_rounded_rect_stroke(bounds, radius, border, 1);

        let up = self.up_rect();
        let down = self.down_rect();
        // The divider is the boundary the two hit rectangles share, so the drawn split and
        // `direction_at` cannot point at different places.
        match self.orientation {
            StepOrientation::Vertical => {
                let y = up.y + up.height as i32;
                context.fill_rect(
                    Rect::new(bounds.x + 2, y, bounds.width.saturating_sub(4), 1),
                    border,
                );
            }
            StepOrientation::Horizontal => {
                let x = up.x + up.width as i32;
                context.fill_rect(
                    Rect::new(x, bounds.y + 2, 1, bounds.height.saturating_sub(4)),
                    border,
                );
            }
        }

        for (rect, enabled, up_direction) in
            [(up, self.up_enabled, true), (down, self.down_enabled, false)]
        {
            if rect.width == 0 || rect.height == 0 {
                continue;
            }
            let color = if enabled { ink } else { ink.with_alpha(80) };
            let cx = rect.x + rect.width as i32 / 2;
            let cy = rect.y + rect.height as i32 / 2;
            let reach = (rect.width.min(rect.height) as i32 / 4).max(2);
            let (tip_y, base_y) = if up_direction {
                (cy - reach, cy + reach / 2)
            } else {
                (cy + reach, cy - reach / 2)
            };
            context.draw_line(Point::new(cx - reach, base_y), Point::new(cx, tip_y), color);
            context.draw_line(Point::new(cx, tip_y), Point::new(cx + reach, base_y), color);
        }
    }
}

// ───────────────────────────── COMP-C04 Scrollbar ─────────────────────────────

/// How a [`Scrollbar`] runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollOrientation {
    /// A right-edge vertical bar.
    #[default]
    Vertical,
    /// A bottom-edge horizontal bar.
    Horizontal,
}

/// A proportional scroll indicator (COMP-C04).
pub struct Scrollbar {
    orientation: ScrollOrientation,
    content_size: u32,
    viewport_size: u32,
    scroll_offset: u32,
    thickness: u32,
    auto_hide: bool,
    thumb_min_length: u32,
    track: Rect,
    /// Emitted when a drag produces a new offset.
    pub scroll_offset_changed: Signal1<u32>,
}

impl Scrollbar {
    /// Creates a vertical bar that hides itself when the content fits.
    pub fn new() -> Self {
        Self {
            orientation: ScrollOrientation::Vertical,
            content_size: 0,
            viewport_size: 0,
            scroll_offset: 0,
            thickness: 4,
            auto_hide: true,
            thumb_min_length: 8,
            track: Rect::new(0, 0, 0, 0),
            scroll_offset_changed: Signal1::new(),
        }
    }

    /// Sets the bar's arrangement and thickness.
    pub fn set_orientation(&mut self, orientation: ScrollOrientation, thickness: u32) {
        self.orientation = orientation;
        self.thickness = thickness.max(1);
    }

    /// The arrangement.
    pub fn orientation(&self) -> ScrollOrientation {
        self.orientation
    }

    /// Sets the track rectangle the host laid out.
    pub fn set_track(&mut self, track: Rect) {
        self.track = track;
    }

    /// The track rectangle.
    pub fn track(&self) -> Rect {
        self.track
    }

    /// Sets the content/viewport sizes and the current offset.
    pub fn set_metrics(&mut self, content: u32, viewport: u32, offset: u32) {
        self.content_size = content;
        self.viewport_size = viewport;
        self.scroll_offset = offset;
    }

    /// The content size.
    pub fn content_size(&self) -> u32 {
        self.content_size
    }

    /// The viewport size.
    pub fn viewport_size(&self) -> u32 {
        self.viewport_size
    }

    /// The current offset.
    pub fn scroll_offset(&self) -> u32 {
        self.scroll_offset
    }

    /// Enables or disables hiding when the content fits.
    pub fn set_auto_hide(&mut self, auto_hide: bool) {
        self.auto_hide = auto_hide;
    }

    /// Sets the minimum thumb length.
    pub fn set_thumb_min_length(&mut self, length: u32) {
        self.thumb_min_length = length.max(1);
    }

    /// Whether the bar should be painted at all.
    pub fn is_visible(&self) -> bool {
        !self.auto_hide || self.content_size > self.viewport_size
    }

    /// The thumb rectangle within `track`.
    pub fn thumb_rect(&self, track: Rect) -> Rect {
        let (track_len, content, viewport) = self.axis(track);
        if content == 0 || viewport == 0 {
            return Rect::new(track.x, track.y, track.width, track.height);
        }
        let max_offset = content.saturating_sub(viewport);
        let thumb_len = ((viewport as f32 / content as f32) * track_len as f32)
            .max(self.thumb_min_length as f32)
            .min(track_len as f32) as u32;
        let travel = track_len.saturating_sub(thumb_len);
        let fraction = if max_offset == 0 {
            0.0
        } else {
            self.scroll_offset.min(max_offset) as f32 / max_offset as f32
        };
        match self.orientation {
            ScrollOrientation::Vertical => Rect::new(
                track.x,
                track.y + (travel as f32 * fraction) as i32,
                track.width,
                thumb_len,
            ),
            ScrollOrientation::Horizontal => Rect::new(
                track.x + (travel as f32 * fraction) as i32,
                track.y,
                thumb_len,
                track.height,
            ),
        }
    }

    /// Whether `pos` is on the thumb of `track`.
    pub fn hit_thumb(&self, pos: Point) -> bool {
        self.thumb_rect(self.track).contains_point(pos)
    }

    /// Maps a drag position to an offset, updates the bar and emits the change.
    pub fn drag_thumb(&mut self, pos: Point) -> u32 {
        let track = self.track;
        let thumb = self.thumb_rect(track);
        let (track_len, content, viewport) = self.axis(track);
        let max_offset = content.saturating_sub(viewport);
        if max_offset == 0 {
            return self.scroll_offset;
        }
        let thumb_len = match self.orientation {
            ScrollOrientation::Vertical => thumb.height,
            ScrollOrientation::Horizontal => thumb.width,
        };
        let travel = track_len.saturating_sub(thumb_len).max(1) as f32;
        let position = match self.orientation {
            ScrollOrientation::Vertical => (pos.y - track.y - thumb_len as i32 / 2) as f32,
            ScrollOrientation::Horizontal => (pos.x - track.x - thumb_len as i32 / 2) as f32,
        };
        let fraction = (position / travel).clamp(0.0, 1.0);
        let next = (fraction * max_offset as f32).round() as u32;
        if next != self.scroll_offset {
            self.scroll_offset = next;
            self.scroll_offset_changed.emit(next);
        }
        next
    }

    fn axis(&self, track: Rect) -> (u32, u32, u32) {
        match self.orientation {
            ScrollOrientation::Vertical => (track.height, self.content_size, self.viewport_size),
            ScrollOrientation::Horizontal => (track.width, self.content_size, self.viewport_size),
        }
    }
}

impl Default for Scrollbar {
    fn default() -> Self {
        Self::new()
    }
}

impl Draw for Scrollbar {
    fn draw(&mut self, context: &mut RenderContext) {
        let track = self.track;
        if !self.is_visible() || track.width == 0 || track.height == 0 {
            return;
        }
        let (surface, ink, accent) = companion_colors();
        let radius = (self.thickness / 2).max(1);
        // The lane is a quiet step off the surface; the thumb is the accent. Both come from
        // `thumb_rect`, the same rectangle `hit_thumb` and `drag_thumb` use.
        context.fill_rounded_rect(track, radius, surface.blend(&ink, 0.12));
        let thumb = self.thumb_rect(track);
        if thumb.width > 0 && thumb.height > 0 {
            context.fill_rounded_rect(thumb, radius, accent.with_alpha(210));
        }
    }
}

// ───────────────────────────── COMP-C05 FormatToolbar ─────────────────────────────

#[cfg(widgets_unstripped)]
use crate::widget::input_widgets::rich_edit::CharFormat;

/// Which format buttons a [`FormatToolbar`] publishes.
#[cfg(widgets_unstripped)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatActionSet {
    /// The bold button.
    pub bold: bool,
    /// The italic button.
    pub italic: bool,
    /// The underline button.
    pub underline: bool,
    /// The colour button.
    pub color: bool,
    /// The clear-format button.
    pub clear_format: bool,
}

#[cfg(widgets_unstripped)]
impl Default for FormatActionSet {
    fn default() -> Self {
        Self { bold: true, italic: true, underline: true, color: true, clear_format: true }
    }
}

/// One action a [`FormatToolbar`] can request.
#[cfg(widgets_unstripped)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatAction {
    /// Toggle bold.
    Bold,
    /// Toggle italic.
    Italic,
    /// Toggle underline.
    Underline,
    /// Apply a colour (carried beside the action).
    Color,
    /// Clear every format.
    ClearFormat,
}

/// The actions a toolbar paints, in the order their cells are laid out.
#[cfg(widgets_unstripped)]
const FORMAT_ACTIONS: [FormatAction; 5] = [
    FormatAction::Bold,
    FormatAction::Italic,
    FormatAction::Underline,
    FormatAction::Color,
    FormatAction::ClearFormat,
];
/// Width of one format cell, in logical pixels.
#[cfg(widgets_unstripped)]
const FORMAT_CELL_WIDTH: u32 = 28;
/// Gap between two format cells, in logical pixels.
#[cfg(widgets_unstripped)]
const FORMAT_CELL_GAP: u32 = 2;
/// Height of a cell when the toolbar has not been given a box yet.
#[cfg(widgets_unstripped)]
const FORMAT_DEFAULT_HEIGHT: u32 = 28;
/// The height a [`FormatToolbar`] occupies when it paints itself through [`Draw`].
///
/// Exposed so a control that hosts the toolbar as a strip (rather than painting it on its
/// own) can reserve exactly the box the toolbar would have painted into.
#[cfg(widgets_unstripped)]
pub const FORMAT_TOOLBAR_HEIGHT: u32 = FORMAT_DEFAULT_HEIGHT;

/// A format toolbar that reports the user's intent without touching an editor (COMP-C05).
#[cfg(widgets_unstripped)]
///
/// The host connects [`Self::format_requested`] to `rich_edit::apply_char_format` (or the selected
/// span set), which keeps the editor from depending on the toolbar.
pub struct FormatToolbar {
    actions: FormatActionSet,
    active_format: CharFormat,
    accent: Option<Color>,
    ink: Option<Color>,
    compact: bool,
    /// The box this toolbar paints into when a host draws it directly through [`Draw`].
    bounds: Rect,
    /// Emitted with the requested action and an optional colour value.
    pub format_requested: Signal1<(FormatAction, Option<Color>)>,
}

#[cfg(widgets_unstripped)]
impl FormatToolbar {
    /// Creates a toolbar publishing every action.
    pub fn new() -> Self {
        Self {
            actions: FormatActionSet::default(),
            active_format: CharFormat::default(),
            accent: None,
            ink: None,
            compact: false,
            bounds: Rect::new(0, 0, 0, 0),
            format_requested: Signal1::new(),
        }
    }

    /// The cursor's current format, which drives the buttons' highlight state.
    pub fn active_format(&self) -> CharFormat {
        self.active_format.clone()
    }

    /// Sets the cursor's current format.
    pub fn set_active_format(&mut self, format: CharFormat) {
        self.active_format = format;
    }

    /// The published action set.
    pub fn actions(&self) -> FormatActionSet {
        self.actions
    }

    /// Restricts the toolbar to a subset of the actions.
    pub fn set_enabled(&mut self, actions: FormatActionSet) {
        self.actions = actions;
    }

    /// Sets the accent and ink colours; `None` derives them from the theme.
    pub fn set_colors(&mut self, accent: Option<Color>, ink: Option<Color>) {
        self.accent = accent;
        self.ink = ink;
    }

    /// The explicit accent.
    pub fn accent(&self) -> Option<Color> {
        self.accent
    }

    /// The explicit ink.
    pub fn ink(&self) -> Option<Color> {
        self.ink
    }

    /// Whether the compact (icon-only) layout is requested.
    pub fn compact(&self) -> bool {
        self.compact
    }

    /// Requests the compact layout.
    pub fn set_compact(&mut self, compact: bool) {
        self.compact = compact;
    }

    /// Whether `action` is published by the current set.
    pub fn is_enabled(&self, action: FormatAction) -> bool {
        match action {
            FormatAction::Bold => self.actions.bold,
            FormatAction::Italic => self.actions.italic,
            FormatAction::Underline => self.actions.underline,
            FormatAction::Color => self.actions.color,
            FormatAction::ClearFormat => self.actions.clear_format,
        }
    }

    /// Reports a press on `action`; disabled actions are ignored.
    ///
    /// A colour action also carries the current accent, which is what the host applies.
    pub fn press(&mut self, action: FormatAction) {
        if !self.is_enabled(action) {
            return;
        }
        let value = matches!(action, FormatAction::Color).then_some(self.accent).flatten();
        self.format_requested.emit((action, value));
    }

    /// The format an action would produce, for the host's preview/highlight.
    pub fn preview(&self, action: FormatAction) -> CharFormat {
        self.preview_from(&self.active_format, action)
    }

    /// The format `action` would produce from `base`.
    ///
    /// [`Self::preview`] applies the toolbar's cached [`Self::active_format`]; a host that
    /// hosts the toolbar inside an editor has the cursor's real format instead, which is what
    /// this entry point takes. Both readers run the same rule, so an embedded toolbar and a
    /// standalone one cannot disagree about what a button does.
    pub fn preview_from(&self, base: &CharFormat, action: FormatAction) -> CharFormat {
        let mut format = base.clone();
        match action {
            FormatAction::Bold => format.bold = !format.bold,
            FormatAction::Italic => format.italic = !format.italic,
            FormatAction::Underline => format.underline = !format.underline,
            FormatAction::Color => format.color = self.accent,
            FormatAction::ClearFormat => format = CharFormat::default(),
        }
        format
    }

    /// Sets the box this toolbar paints into through [`Draw`].
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    /// The box this toolbar paints into through [`Draw`].
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// The cell one published action occupies inside `bounds`, in layout order.
    ///
    /// The index counts only published actions, so disabling an action closes the gap rather
    /// than leaving an empty cell the hit test would still accept.
    fn cell_rect(&self, action: FormatAction, bounds: Rect) -> Option<Rect> {
        if !self.is_enabled(action) {
            return None;
        }
        let column = FORMAT_ACTIONS
            .iter()
            .take_while(|candidate| **candidate != action)
            .filter(|candidate| self.is_enabled(**candidate))
            .count();
        let width =
            if self.compact { FORMAT_CELL_WIDTH.saturating_sub(6) } else { FORMAT_CELL_WIDTH };
        let x_offset = column as u32 * (width + FORMAT_CELL_GAP);
        if x_offset >= bounds.width {
            return None;
        }
        let available = bounds.width - x_offset;
        Some(Rect::new(bounds.x + x_offset as i32, bounds.y, width.min(available), bounds.height))
    }

    /// The published action under `pos` when the toolbar has been given a box.
    ///
    /// This is the geometric partner of [`Self::press`]: a host that routes pointer input can
    /// find the action the same way the painter lays its cells out.
    pub fn action_at(&self, pos: Point) -> Option<FormatAction> {
        FORMAT_ACTIONS.iter().copied().find(|action| {
            self.cell_rect(*action, self.bounds).is_some_and(|rect| rect.contains_point(pos))
        })
    }

    /// Paints the toolbar into `rect`.
    ///
    /// Each enabled action gets one cell; an active format is tinted with the accent so the
    /// toolbar reports the cursor's format in the same colours the editor uses.
    pub fn paint(&self, context: &mut RenderContext, rect: Rect) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = companion_style("rich_edit");
        let font = role_font(&style, 12.0);
        let (surface, ink, theme_accent) = companion_colors();
        let accent = self.accent.unwrap_or(theme_accent);
        let ink = self.ink.unwrap_or(ink);
        let border = surface.blend(&ink, 0.3);
        context.fill_rounded_rect(rect, 4, surface);
        context.draw_rounded_rect_stroke(rect, 4, border, 1);

        for action in FORMAT_ACTIONS {
            let Some(cell) = self.cell_rect(action, rect) else {
                continue;
            };
            if cell.width == 0 || cell.height == 0 {
                continue;
            }
            let active = match action {
                FormatAction::Bold => self.active_format.bold,
                FormatAction::Italic => self.active_format.italic,
                FormatAction::Underline => self.active_format.underline,
                FormatAction::Color => self.active_format.color.is_some(),
                FormatAction::ClearFormat => false,
            };
            let cell_fill = if active { accent.with_alpha(40) } else { surface };
            context.fill_rounded_rect(cell, 3, cell_fill);
            let color = if active { accent } else { ink };
            match action {
                FormatAction::Bold => {
                    context.draw_text_line_cow(
                        cell,
                        "B",
                        &font.with_bold(true),
                        color,
                        HorizontalAlignment::Center,
                    );
                }
                FormatAction::Italic => {
                    context.draw_text_line_cow(
                        cell,
                        "I",
                        &font.with_italic(true),
                        color,
                        HorizontalAlignment::Center,
                    );
                }
                FormatAction::Underline => {
                    context.draw_text_line_cow(
                        cell,
                        "U",
                        &font,
                        color,
                        HorizontalAlignment::Center,
                    );
                    let y = cell.y + cell.height as i32 - 5;
                    context.draw_line(
                        Point::new(cell.x + 6, y),
                        Point::new(cell.x + cell.width as i32 - 6, y),
                        color,
                    );
                }
                FormatAction::Color => {
                    let center =
                        Point::new(cell.x + cell.width as i32 / 2, cell.y + cell.height as i32 / 2);
                    context.fill_circle(center, (cell.height / 5).max(3), accent);
                }
                FormatAction::ClearFormat => {
                    context.draw_text_line_cow(
                        cell,
                        "Tx",
                        &font,
                        color,
                        HorizontalAlignment::Center,
                    );
                }
            }
        }
    }
}

#[cfg(widgets_unstripped)]
impl Default for FormatToolbar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(widgets_unstripped)]
impl Draw for FormatToolbar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = if self.bounds.width == 0 || self.bounds.height == 0 {
            let columns =
                FORMAT_ACTIONS.iter().filter(|action| self.is_enabled(**action)).count() as u32;
            let width = columns.saturating_mul(FORMAT_CELL_WIDTH + FORMAT_CELL_GAP);
            Rect::new(self.bounds.x, self.bounds.y, width, FORMAT_DEFAULT_HEIGHT)
        } else {
            self.bounds
        };
        self.paint(context, rect);
    }
}
