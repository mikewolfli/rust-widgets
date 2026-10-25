// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AudioVisualizer widget — real-time audio waveform/spectrum visualization.
//!
//! The widget draws vertical bars in one of two modes, selected with
//! [`AudioVisualizer::set_mode`]:
//!
//! * [`VisualizerMode::Waveform`] (the default) splits the retained sample history into one chunk
//!   per bar and draws each bar at that chunk's **mean absolute sample value**. That is a
//!   time-domain loudness, not a frequency analysis — the bars are not frequency bands.
//! * [`VisualizerMode::Spectrum`] draws one bar per **logarithmic frequency band**. Levels come
//!   from host-supplied bins when the host runs its own transform
//!   ([`AudioVisualizer::set_spectrum`]), and otherwise from a self-contained 256-point radix-2
//!   FFT with a Hann window over the newest samples. The FFT path lays its bands out against the
//!   Nyquist frequency, so the bars are the same at any sample rate; at 48 kHz one of its 128
//!   bins spans 187.5 Hz, which is the figure to use when a bar has to be named in Hz.
//!
//! The retained history is bounded to 512 samples (two analysis windows): `set_samples` keeps the
//! newest window pair and `add_sample` drops the oldest sample once the buffer is full, so a
//! streaming host cannot grow the widget without bound.
//!
//! Both modes support mirror mode (bottom half mirrors top), peak hold indicators, and
//! configurable bar count and spacing.

use alloc::collections::VecDeque;
use core::cell::Cell;

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_f32, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// How [`AudioVisualizer`] derives each bar's level.
///
/// The default is [`VisualizerMode::Waveform`], the time-domain mean magnitude the control has
/// always drawn; [`VisualizerMode::Spectrum`] opts into frequency-band bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualizerMode {
    /// Time-domain loudness: each bar is the mean absolute sample of its chunk of the history.
    ///
    /// This is **not** a frequency analysis. A host that wants frequency bands must select
    /// [`VisualizerMode::Spectrum`] or feed its own spectrum with
    /// [`AudioVisualizer::set_spectrum`].
    #[default]
    Waveform,
    /// Frequency bands on a logarithmic axis: each bar is the peak magnitude of its band.
    ///
    /// Levels come from [`AudioVisualizer::set_spectrum`] when the host supplied bins, and
    /// otherwise from the widget's own FFT over the newest samples.
    Spectrum,
}

/// Audio waveform/spectrum visualization widget.
///
/// Renders vertical bars derived from audio samples (normalized -1.0 to 1.0). The default
/// [`VisualizerMode::Waveform`] draws time-domain loudness per chunk;
/// [`VisualizerMode::Spectrum`] draws logarithmic frequency bands. Supports mirroring, peak hold,
/// and configurable appearance.
pub struct AudioVisualizer {
    base: BaseWidget,
    /// Audio sample data normalized to -1.0 to 1.0, at most `SAMPLE_HISTORY_LIMIT` of them.
    ///
    /// A deque rather than a `Vec`: [`Self::add_sample`] is fed per sample by a streaming host, so
    /// dropping the oldest entry must be O(1) instead of shifting the whole window (`remove(0)`
    /// moved 512 elements tens of thousands of times a second at 44.1 kHz).
    samples: VecDeque<f32>,
    /// Which derivation [`AudioVisualizer::bar_value`] uses for each bar.
    mode: VisualizerMode,
    /// Host-computed spectrum bins, when the host already runs its own transform.
    ///
    /// `None` means "no host spectrum", and the spectrum mode then derives magnitudes itself. An
    /// empty vector is treated the same way: there is nothing to draw, and inventing zeros would
    /// claim a silence the host never reported.
    spectrum: Option<Vec<f32>>,
    /// Magnitudes of the newest analysis window, cached until the sample history changes.
    ///
    /// Without it a draw (plus the peak-hold clock's sweep) would run the FFT once **per bar**,
    /// although the spectrum is identical for every bar — only the band a bar reads differs.
    /// `Cell` rather than `&mut self` because `bar_value` is a `&self` query used by both the paint
    /// path and [`Widget::is_animating`].
    spectrum_cache: Cell<Option<[f32; SPECTRUM_BINS]>>,
    /// Number of vertical bars to display.
    bar_count: usize,
    /// Spacing between bars in pixels.
    bar_spacing: f32,
    /// Color of the bars, when the caller has chosen one.
    ///
    /// `None` means "derive the per-bar gradient from the level", which is what the control
    /// paints by default. An explicit colour paints every bar in that colour.
    bar_color: Option<Color>,
    /// Reused per-bar level buffer, so a draw does not allocate a vector every frame.
    bar_buffer: Vec<f32>,
    /// Whether the peak markers still owe frames (set by the tick that advanced them).
    peak_moving: bool,
    /// Whether the smoother still owes frames.
    smoothing_moving: bool,
    /// Background color of the visualization area, when the caller has chosen one.
    ///
    /// `None` means "use the active theme's surface", which is what a visualizer drawn on a themed
    /// page wants. It used to be a **fixed** `rgb(20, 20, 30)` initialised in `new()`, so the
    /// control painted a near-black rectangle on the light appearance as well and the
    /// `style.background_color` that `apply_active_theme` wrote was read by nobody — a declared
    /// value with no consumer. Keeping it `Option` is what lets "the caller set one" and "the
    /// theme supplies one" be told apart, the same distinction `WidgetStyle::theme_derived`
    /// records for the base style.
    background_color: Option<Color>,
    /// Whether to mirror the visualization (bottom half mirrors top).
    mirror: bool,
    /// Whether to show peak hold markers.
    peak_hold: bool,
    /// Duration in ms to hold peak values.
    peak_hold_duration: u64,
    /// Current peak hold values for each bar.
    peak_values: Vec<f32>,
    /// Smoothed bar levels, parallel to `peak_values`.
    ///
    /// Only used when smoothing is enabled; an empty vector (the default) means "draw the raw
    /// level", which keeps every existing picture byte-identical.
    smoothed_levels: Vec<f32>,
    /// Attack time constant in milliseconds (how fast a rising bar follows the signal).
    attack_ms: f32,
    /// Release time constant in milliseconds (how fast a falling bar follows the signal).
    release_ms: f32,
    /// Milliseconds each bar's peak has been held, indexed like `peak_values`.
    ///
    /// Per bar rather than one clock for the whole control: a peak is raised by *its own* bar, so
    /// one bar falling silent must not release the peak the bar next to it has just set. A single
    /// shared timer would do exactly that, and the symptom is every marker sliding down together as
    /// soon as the loudest bar stopped.
    peak_ages: Vec<u64>,
    /// Emitted with the overall level, throttled to `level_report_interval_ms`.
    pub level_changed: Signal1<f32>,
    /// Emitted when the window peak exceeds [`Self::peak_threshold`], with that peak.
    pub peak_exceeded: Signal1<f32>,
    /// Emitted with the new mode name when [`Self::set_mode`] changes it.
    pub mode_changed: Signal1<String>,
    /// Threshold above which `peak_exceeded` fires, `0.0..=1.0`.
    peak_threshold: f32,
    /// Minimum milliseconds between two `level_changed` / `peak_exceeded` reports.
    level_report_interval_ms: u32,
    /// Milliseconds accumulated towards the next throttled report.
    report_clock_ms: u32,
    /// The level last announced, so an unchanged meter stays quiet.
    last_reported_level: f32,
    /// The peak last announced.
    last_reported_peak: f32,
}

/// The fall rate of a released peak, in units of full height per second.
///
/// A held peak does not *vanish* when its duration expires -- that reads as a flicker rather than
/// as a marker falling back. It descends at a constant speed, one full bar height per second, which
/// is slow enough to follow at 60 fps and fast enough that a stale marker is gone by the time the
/// next beat arrives.
const PEAK_FALL_PER_SECOND: f32 = 1.0;

/// Samples per analysis window used by the spectrum path.
///
/// 256 is the power of two that keeps the per-frame cost negligible while still resolving the
/// low end of the log axis: at a 48 kHz rate one bin spans 187.5 Hz.
const SPECTRUM_WINDOW: usize = 256;

/// Number of magnitude bins produced by [`fft_magnitudes`]: the half spectrum, DC included and the
/// Nyquist bin excluded.
const SPECTRUM_BINS: usize = SPECTRUM_WINDOW / 2;

/// Sample history the widget retains, in samples: two analysis windows.
///
/// The newest window feeds the FFT and the one before it keeps the history meaningful for the
/// waveform mode. The bound is what stops [`AudioVisualizer::add_sample`] from growing the buffer
/// for the lifetime of the widget.
const SAMPLE_HISTORY_LIMIT: usize = SPECTRUM_WINDOW * 2;

impl AudioVisualizer {
    /// Creates a new AudioVisualizer widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        let bar_count = 64;
        Self {
            base: BaseWidget::new(WidgetKind::AudioVisualizer, geometry, "AudioVisualizer"),
            samples: VecDeque::new(),
            mode: VisualizerMode::Waveform,
            spectrum: None,
            spectrum_cache: Cell::new(None),
            bar_count,
            bar_spacing: 2.0,
            bar_color: None,
            bar_buffer: Vec::new(),
            peak_moving: false,
            smoothing_moving: false,
            background_color: None,
            mirror: false,
            peak_hold: false,
            peak_hold_duration: 500,
            peak_values: vec![0.0; bar_count],
            peak_ages: vec![0; bar_count],
            smoothed_levels: Vec::new(),
            attack_ms: 0.0,
            release_ms: 0.0,
            level_changed: Signal1::new(),
            peak_exceeded: Signal1::new(),
            mode_changed: Signal1::new(),
            peak_threshold: 0.9,
            level_report_interval_ms: 50,
            report_clock_ms: 0,
            last_reported_level: 0.0,
            last_reported_peak: 0.0,
        }
    }

    /// Sets the audio sample data. Values should be normalized to -1.0 to 1.0.
    ///
    /// Only the newest 512 samples are retained. The bound is deliberate: the spectrum path
    /// analyses a 256-sample window, and two windows are enough history for both modes, whereas
    /// keeping whatever length the caller passed made a long stream grow the widget without limit.
    pub fn set_samples(&mut self, data: Vec<f32>) {
        let skip = data.len().saturating_sub(SAMPLE_HISTORY_LIMIT);
        self.samples = data.into_iter().skip(skip).map(|v| v.clamp(-1.0, 1.0)).collect();
        self.spectrum_cache.set(None);
        if !self.smoothed_levels.is_empty() {
            self.smoothing_moving = true;
        }
        self.base.request_redraw();
    }

    /// Appends a block of samples, keeping the bounded history.
    ///
    /// This is the bulk entry point: a host that decodes a frame block of audio should call it
    /// once per block rather than once per sample. It performs one `pop_front` per discarded
    /// sample and one allocation-free `push_back` per new sample.
    ///
    /// # FFT cache contract
    ///
    /// The spectrum magnitudes are cached until the sample history changes. A host that feeds a
    /// block must therefore feed the *whole* block before drawing: the first draw after the call
    /// runs the transform once, and every later draw of the same history reuses it.
    pub fn set_samples_bulk(&mut self, data: &[f32]) {
        for &sample in data {
            self.push_sample(sample);
        }
        self.spectrum_cache.set(None);
        if !self.smoothed_levels.is_empty() {
            self.smoothing_moving = true;
        }
        self.base.request_redraw();
    }

    /// Appends one sample, evicting the oldest in O(1) once the history is full.
    fn push_sample(&mut self, value: f32) {
        if self.samples.len() >= SAMPLE_HISTORY_LIMIT {
            self.samples.pop_front();
        }
        self.samples.push_back(value.clamp(-1.0, 1.0));
    }

    /// Adds a single audio sample. Useful for streaming data.
    ///
    /// Once the history is full the oldest sample is dropped, so the buffer stays bounded at 512
    /// samples instead of growing for the lifetime of the widget.
    pub fn add_sample(&mut self, value: f32) {
        self.push_sample(value);
        self.spectrum_cache.set(None);
        if !self.smoothed_levels.is_empty() {
            self.smoothing_moving = true;
        }
        self.base.request_redraw();
    }

    /// Sets a one-pole attack/release smoother for the bar bodies.
    ///
    /// `attack_ms` is the time constant for a rising bar and `release_ms` for a falling one, in
    /// milliseconds. Zero for both (the default) disables smoothing entirely, so the bars are
    /// the raw level. Enabling it is a visual choice: the peaks still use the raw value, so a
    /// marker is not slowed down by the body's filter.
    pub fn set_smoothing(&mut self, attack_ms: f32, release_ms: f32) {
        let attack = if attack_ms.is_finite() { attack_ms.max(0.0) } else { 0.0 };
        let release = if release_ms.is_finite() { release_ms.max(0.0) } else { 0.0 };
        if self.attack_ms == attack && self.release_ms == release {
            return;
        }
        self.attack_ms = attack;
        self.release_ms = release;
        if attack == 0.0 && release == 0.0 {
            self.smoothed_levels.clear();
            self.smoothing_moving = false;
        } else {
            self.smoothed_levels.resize(self.bar_count, 0.0);
            self.smoothing_moving = true;
        }
        self.base.request_redraw();
    }

    /// Whether the one-pole smoother is active.
    pub fn is_smoothing_enabled(&self) -> bool {
        !self.smoothed_levels.is_empty()
    }

    /// The level the paint path should use for bar `index`: the smoothed value when the
    /// smoother is active, otherwise the raw derivation.
    fn display_level(&self, index: usize) -> f32 {
        self.smoothed_levels.get(index).copied().unwrap_or_else(|| self.bar_value(index))
    }

    /// Advances the one-pole smoother by `delta_ms`; `true` if any level moved.
    fn advance_smoothing(&mut self, delta_ms: u32) -> bool {
        if self.smoothed_levels.is_empty() || delta_ms == 0 {
            return false;
        }
        let mut moved = false;
        for index in 0..self.smoothed_levels.len() {
            let target = self.bar_value(index);
            let current = self.smoothed_levels[index];
            let tau = if target > current { self.attack_ms } else { self.release_ms };
            let next = if tau <= 0.0 {
                target
            } else {
                let coefficient = 1.0 - (-(delta_ms as f32) / tau).exp();
                current + (target - current) * coefficient
            };
            if (next - current).abs() > f32::EPSILON {
                self.smoothed_levels[index] = next;
                moved = true;
            }
        }
        moved
    }

    /// Clears all audio samples.
    pub fn clear_samples(&mut self) {
        self.samples.clear();
        self.spectrum_cache.set(None);
        self.base.request_redraw();
    }

    /// Returns the retained samples, at most 512 of the newest ones.
    ///
    /// The storage is a ring, so this materialises an ordered copy for inspection rather than
    /// exposing the internal buffer. The streaming paths (`add_sample`, `set_samples_bulk`) do not
    /// call it and never compact.
    pub fn samples(&self) -> Vec<f32> {
        self.samples.iter().copied().collect()
    }

    /// Selects how each bar's level is derived.
    ///
    /// Switching modes clears neither the sample history nor a host-supplied spectrum, so a host
    /// can flip between the two views of the same stream without losing data.
    pub fn set_mode(&mut self, mode: VisualizerMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        let name = match mode {
            VisualizerMode::Waveform => "waveform",
            VisualizerMode::Spectrum => "spectrum",
        };
        self.mode_changed.emit(name.to_string());
        self.base.request_redraw();
    }

    /// The threshold above which `peak_exceeded` fires.
    pub fn peak_threshold(&self) -> f32 {
        self.peak_threshold
    }

    /// Sets the peak threshold, clamped into `0.0..=1.0`.
    pub fn set_peak_threshold(&mut self, threshold: f32) {
        if !threshold.is_finite() {
            return;
        }
        let threshold = threshold.clamp(0.0, 1.0);
        if self.peak_threshold == threshold {
            return;
        }
        self.peak_threshold = threshold;
    }

    /// The minimum milliseconds between two throttled level reports; `0` reports on every tick.
    pub fn level_report_interval(&self) -> u32 {
        self.level_report_interval_ms
    }

    /// Sets the level report throttle interval.
    pub fn set_level_report_interval(&mut self, ms: u32) {
        if self.level_report_interval_ms == ms {
            return;
        }
        self.level_report_interval_ms = ms;
    }

    /// Returns the active bar-derivation mode.
    pub fn mode(&self) -> VisualizerMode {
        self.mode
    }

    /// Supplies a spectrum the host computed itself, taking precedence over the widget's FFT.
    ///
    /// Bins are interpreted as a linear spectrum over the host's own frequency axis, lowest bin
    /// first, with bin 0 as DC. Values are clamped into the drawn 0.0..=1.0 range and non-finite
    /// values become 0.0, so one bad bin cannot poison a bar. An empty vector clears the host
    /// spectrum and returns the widget to its own FFT.
    pub fn set_spectrum(&mut self, bins: Vec<f32>) {
        let sanitized: Vec<f32> = bins
            .into_iter()
            .map(|bin| if bin.is_finite() { bin.clamp(0.0, 1.0) } else { 0.0 })
            .collect();
        self.spectrum = if sanitized.is_empty() { None } else { Some(sanitized) };
        self.base.request_redraw();
    }

    /// Sets the number of vertical bars to display.
    pub fn set_bar_count(&mut self, n: usize) {
        let bar_count = n.max(1);
        if self.bar_count == bar_count {
            return;
        }
        self.bar_count = bar_count;
        // Both per-bar vectors resize together. They are indexed alike, so resizing one alone left
        // the other short -- and the marker walk, which indexes both, would have panicked on the
        // next tick after any widening.
        self.peak_values.resize(self.bar_count, 0.0);
        self.peak_ages.resize(self.bar_count, 0);
        self.bar_buffer.resize(self.bar_count, 0.0);
        if !self.smoothed_levels.is_empty() {
            self.smoothed_levels.resize(self.bar_count, 0.0);
        }
        self.base.request_redraw();
    }

    /// Returns the current bar count.
    pub fn bar_count(&self) -> usize {
        self.bar_count
    }

    /// Sets the spacing between bars in pixels.
    pub fn set_bar_spacing(&mut self, spacing: f32) {
        let spacing = spacing.max(0.0);
        if self.bar_spacing == spacing {
            return;
        }
        self.bar_spacing = spacing;
        self.base.request_redraw();
    }

    /// Returns the current bar spacing.
    pub fn bar_spacing(&self) -> f32 {
        self.bar_spacing
    }

    /// Sets the color of the bars.
    ///
    /// The paint path reads the stored colour as the bar fill; without that read the setter was
    /// a dead interface that requested a repaint and changed nothing (MW-02).
    pub fn set_bar_color(&mut self, color: Color) {
        if self.bar_color == Some(color) {
            return;
        }
        self.bar_color = Some(color);
        self.base.request_redraw();
    }

    /// Returns the caller's bar colour, or `None` when the level gradient is painted.
    pub fn bar_color(&self) -> Option<Color> {
        self.bar_color
    }

    /// Sets the background color of the visualization area.
    pub fn set_background_color(&mut self, color: Color) {
        if self.background_color == Some(color) {
            return;
        }
        self.background_color = Some(color);
        self.base.request_redraw();
    }

    /// Returns the background color the visualizer will paint.
    ///
    /// An explicit [`Self::set_background_color`] wins; otherwise this is the active theme's
    /// surface, so the control sits on the page rather than on a fixed near-black. The final
    /// fallback covers a build with no theme at all.
    pub fn background_color(&self) -> Color {
        self.resolved_background()
    }

    /// The fill the draw path should use: the caller's colour, else the theme's surface.
    fn resolved_background(&self) -> Color {
        if let Some(chosen) = self.background_color {
            return chosen;
        }
        // A visualizer's panel is a **surface one step above the page**, the same role
        // `font_preview` and the popup panels use. The theme guard is released before returning.
        let themed = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.surface_container);
        themed.unwrap_or(Color::rgba(20, 20, 30, 255))
    }

    /// Enables or disables mirror mode. When enabled, the bottom half mirrors the top.
    pub fn set_mirror(&mut self, mirror: bool) {
        if self.mirror == mirror {
            return;
        }
        self.mirror = mirror;
        self.base.request_redraw();
    }

    /// Returns whether mirror mode is enabled.
    pub fn is_mirror_enabled(&self) -> bool {
        self.mirror
    }

    /// Enables or disables peak hold indicators.
    pub fn set_peak_hold(&mut self, enabled: bool) {
        if self.peak_hold == enabled {
            return;
        }
        self.peak_hold = enabled;
        self.peak_moving = enabled;
        self.base.request_redraw();
    }

    /// Returns whether peak hold is enabled.
    pub fn is_peak_hold_enabled(&self) -> bool {
        self.peak_hold
    }

    /// Sets the peak hold duration in milliseconds.
    ///
    /// The field is read by [`AudioVisualizer::advance_peak_hold`], which the animation bus drives:
    /// a peak is held for this long after the bar leaves it, then falls. Before that method existed
    /// the field was stored, published and read by nothing, and a marker sat at its highest ever
    /// value for the lifetime of the widget -- which is not "hold for 500 ms", it is "hold forever".
    pub fn set_peak_hold_duration(&mut self, ms: u64) {
        if self.peak_hold_duration == ms {
            return;
        }
        self.peak_hold_duration = ms;
        self.base.request_redraw();
    }

    /// Returns the peak hold duration in milliseconds.
    pub fn peak_hold_duration(&self) -> u64 {
        self.peak_hold_duration
    }

    /// Advances the peak-hold markers by `delta_ms`, and reports whether anything moved.
    ///
    /// # The two jobs, and why they are one method
    ///
    /// A marker is raised to its bar's current level and then held for [`Self::peak_hold_duration`]
    /// before falling. Raising is a *state* and falling is a *clock*, and they share the per-bar
    /// vectors: a caller that ran one without the other would get a marker that rises and never falls
    /// (the defect this replaces) or one whose age is reset by the bar it belongs to. Both are here
    /// so neither can be called alone.
    ///
    /// # Why the animation bus rather than `draw`
    ///
    /// A duration is only observable if time passes for the widget. `draw` runs when something asks
    /// for a repaint, which for a paused stream is never, so a marker's age must not be computed
    /// there. [`Widget::tick`] is the bus's clock, so the duration reaches it here and the control
    /// asks for the repaint its falling marker needs.
    ///
    /// Returns `true` when a marker moved, which is what tells the bus to keep ticking.
    fn advance_peak_hold(&mut self, delta_ms: u64) -> bool {
        if !self.peak_hold || delta_ms == 0 {
            return false;
        }
        let fall = PEAK_FALL_PER_SECOND * (delta_ms as f32 / 1000.0);
        let mut moved = false;
        for index in 0..self.peak_values.len() {
            let value = self.bar_value(index);
            if value >= self.peak_values[index] {
                // The bar has reached or passed its marker, so the marker is re-set and its clock
                // restarts. `>=` rather than `>` so a steady tone keeps its marker pinned instead of
                // letting it drift down a step at a time.
                moved |= self.peak_values[index] != value;
                self.peak_values[index] = value;
                self.peak_ages[index] = 0;
                continue;
            }
            self.peak_ages[index] = self.peak_ages[index].saturating_add(delta_ms);
            if self.peak_ages[index] <= self.peak_hold_duration {
                continue;
            }
            let next = (self.peak_values[index] - fall).max(self.peak_values[index].min(value));
            // A marker that has fallen to (or below) its bar has done its job and is released here
            // rather than being left a hair above it, which would paint a fraction of a pixel of
            // highlight on every quiet bar for the widget's whole life.
            if next <= 0.0 || next <= value {
                self.peak_values[index] = value;
                self.peak_ages[index] = 0;
            } else {
                self.peak_values[index] = next;
            }
            moved = true;
        }
        moved
    }

    /// The normalized level of bar `index` in the active mode.
    ///
    /// # Why the paint and the marker share this
    ///
    /// The paint used to derive its own `bars` vector inline, and a peak-hold marker that measured
    /// the same bar *differently* would sit above or below the bar it belongs to. So the derivation
    /// is one function: index in, level out, and the mode is applied here rather than in a caller.
    fn bar_value(&self, index: usize) -> f32 {
        if self.bar_count == 0 || index >= self.bar_count {
            return 0.0;
        }
        match self.mode {
            VisualizerMode::Waveform => self.waveform_bar_value(index),
            VisualizerMode::Spectrum => self.spectrum_bar_value(index),
        }
    }

    /// The waveform-mode level of bar `index`, or a synthetic level when no samples were supplied.
    ///
    /// # Mean magnitude, not peak
    ///
    /// The level is the mean of the chunk's absolute samples, which is what this control's bars
    /// have always meant -- a *loudness* per chunk rather than a sample peak. A marker derived from
    /// a maximum would ride above the bar it marks on any non-constant window, which is the one
    /// thing a peak-hold marker must not do.
    fn waveform_bar_value(&self, index: usize) -> f32 {
        if self.samples.is_empty() {
            // The paint's synthetic sweep, so a control with no data still has bars for its markers
            // to belong to rather than a row of markers over an empty panel.
            let t = index as f32 / self.bar_count as f32;
            return ((t * std::f32::consts::PI * 4.0).sin().abs() * 0.6 + 0.1).min(1.0);
        }
        let step = (self.samples.len() as f32 / self.bar_count as f32).max(1.0);
        let start = (index as f32 * step) as usize;
        let end = (((index as f32 + 1.0) * step) as usize).min(self.samples.len());
        if start >= end {
            return 0.0;
        }
        let sum: f32 =
            self.samples.iter().skip(start).take(end - start).map(|sample| sample.abs()).sum();
        (sum / (end - start) as f32).min(1.0)
    }

    /// The spectrum-mode level of bar `index`: the peak magnitude of its logarithmic band.
    ///
    /// Host-supplied bins win over the widget's own transform, which is the precedence a host that
    /// already runs a better FFT expects. With neither a spectrum nor a sample history there is
    /// nothing to analyse, so the bar is 0.0 rather than the waveform mode's synthetic sweep: a
    /// silent spectrum is flat, and painting a decorative sweep would claim data that does not
    /// exist.
    ///
    /// # Why low bands may repeat a bin
    ///
    /// The bar axis is logarithmic and the FFT's bins are not, so near the bottom several bands
    /// are narrower than one bin and read the same value. That is a property of the 256-point
    /// resolution (187.5 Hz per bin at 48 kHz), not of the mapping.
    fn spectrum_bar_value(&self, index: usize) -> f32 {
        if let Some(bins) = self.spectrum.as_deref() {
            let (start, end) = self.bar_bin_range(index, bins.len());
            return peak_in_band(bins, start, end);
        }
        if self.samples.is_empty() {
            return 0.0;
        }
        let magnitudes = self.derived_spectrum();
        let (start, end) = self.bar_bin_range(index, SPECTRUM_BINS);
        peak_in_band(&magnitudes, start, end)
    }

    /// The half-open bin range `[start, end)` owned by bar `index` on a log-frequency axis.
    ///
    /// The axis runs from the first non-DC bin to the top of the spectrum (the Nyquist frequency
    /// for the widget's own FFT), and each bar covers an equal *ratio* rather than an equal number
    /// of bins. Bin 0 is DC and is skipped whenever the spectrum has more than one bin, because a
    /// DC offset is not an audible band.
    fn bar_bin_range(&self, index: usize, bin_count: usize) -> (usize, usize) {
        if bin_count == 0 {
            return (0, 0);
        }
        if bin_count == 1 {
            return (0, 1);
        }
        let bars = self.bar_count.max(1);
        let first = 1.0f32;
        let last = bin_count as f32;
        let ratio = last / first;
        let position = |fraction: f32| first * ratio.powf(fraction.clamp(0.0, 1.0));
        let start_position = position(index as f32 / bars as f32);
        let end_position = position((index + 1) as f32 / bars as f32);
        let start = (start_position.floor() as usize).clamp(1, bin_count - 1);
        let end = (end_position.round() as usize).clamp(start + 1, bin_count);
        (start, end)
    }

    /// The newest analysis window, zero-padded at the end when the history is shorter.
    ///
    /// Padding at the end keeps the samples at their original positions. The magnitudes would be
    /// the same with the silence first — a circular shift only rotates the phase — so this is the
    /// conventional placement rather than a claim of extra information.
    fn analysis_window(&self) -> [f32; SPECTRUM_WINDOW] {
        let mut window = [0.0f32; SPECTRUM_WINDOW];
        let available = self.samples.len().min(SPECTRUM_WINDOW);
        let start = self.samples.len() - available;
        for (slot, sample) in self.samples.iter().skip(start).enumerate() {
            window[slot] = *sample;
        }
        window
    }

    /// The magnitudes of the newest analysis window, computed at most once per sample change.
    fn derived_spectrum(&self) -> [f32; SPECTRUM_BINS] {
        if let Some(cached) = self.spectrum_cache.get() {
            return cached;
        }
        let magnitudes = fft_magnitudes(&self.analysis_window());
        self.spectrum_cache.set(Some(magnitudes));
        magnitudes
    }

    /// Emits the throttled level and peak reports; `true` when either fired.
    fn advance_level_report(&mut self, delta_ms: u32) -> bool {
        self.report_clock_ms = self.report_clock_ms.saturating_add(delta_ms);
        if self.level_report_interval_ms != 0
            && self.report_clock_ms < self.level_report_interval_ms
        {
            return false;
        }
        self.report_clock_ms = 0;

        let level = if self.bar_count == 0 {
            0.0
        } else {
            (0..self.bar_count).map(|index| self.display_level(index)).sum::<f32>()
                / self.bar_count as f32
        };
        let mut reported = false;
        if (level - self.last_reported_level).abs() > 0.005 {
            self.last_reported_level = level;
            self.level_changed.emit(level);
            reported = true;
        }

        let peak = self.samples.iter().fold(0.0f32, |current, sample| current.max(sample.abs()));
        if peak > self.peak_threshold && (peak - self.last_reported_peak).abs() > 0.005 {
            self.last_reported_peak = peak;
            self.peak_exceeded.emit(peak);
            reported = true;
        }
        reported
    }
}

/// The largest value in `bins[start..end]`, or 0.0 for an empty range.
fn peak_in_band(bins: &[f32], start: usize, end: usize) -> f32 {
    let end = end.min(bins.len());
    if start >= end {
        return 0.0;
    }
    bins[start..end].iter().copied().fold(0.0, f32::max)
}

/// Reverses the low `bits` bits of `value`, the permutation a radix-2 FFT applies first.
fn reverse_bits(mut value: u32, bits: u32) -> u32 {
    let mut reversed = 0u32;
    for _ in 0..bits {
        reversed = (reversed << 1) | (value & 1);
        value >>= 1;
    }
    reversed
}

/// Magnitudes of the half spectrum of a 256-sample window, normalized to 0.0..=1.0.
///
/// A self-contained iterative radix-2 FFT (bit-reversal permutation followed by in-place
/// butterflies) with a Hann window applied before the transform, so the crate needs no FFT
/// dependency. The Hann window's coherent gain is `N / 2`, so a full-scale sine on a bin leaves a
/// peak magnitude of `N / 4`; dividing by that (`4 / N`) makes a full-scale tone read 1.0. Bin 0 is
/// DC and is present in the result but skipped by the bar axis; the Nyquist bin is not returned.
fn fft_magnitudes(window: &[f32; SPECTRUM_WINDOW]) -> [f32; SPECTRUM_BINS] {
    let mut re = [0.0f32; SPECTRUM_WINDOW];
    let mut im = [0.0f32; SPECTRUM_WINDOW];

    for (index, sample) in window.iter().enumerate() {
        let phase = 2.0 * std::f32::consts::PI * index as f32 / SPECTRUM_WINDOW as f32;
        let hann = 0.5 - 0.5 * phase.cos();
        re[index] = sample * hann;
    }

    let bits = SPECTRUM_WINDOW.trailing_zeros();
    for index in 0..SPECTRUM_WINDOW {
        let mirrored = reverse_bits(index as u32, bits) as usize;
        if mirrored > index {
            re.swap(index, mirrored);
            im.swap(index, mirrored);
        }
    }

    let mut length = 2;
    while length <= SPECTRUM_WINDOW {
        let angle = -2.0 * std::f32::consts::PI / length as f32;
        let (twiddle_re, twiddle_im) = (angle.cos(), angle.sin());
        let mut block = 0;
        while block < SPECTRUM_WINDOW {
            let mut rotation_re = 1.0f32;
            let mut rotation_im = 0.0f32;
            for offset in 0..length / 2 {
                let even = block + offset;
                let odd = even + length / 2;
                let odd_re = re[odd] * rotation_re - im[odd] * rotation_im;
                let odd_im = re[odd] * rotation_im + im[odd] * rotation_re;
                re[odd] = re[even] - odd_re;
                im[odd] = im[even] - odd_im;
                re[even] += odd_re;
                im[even] += odd_im;
                let next_re = rotation_re * twiddle_re - rotation_im * twiddle_im;
                rotation_im = rotation_re * twiddle_im + rotation_im * twiddle_re;
                rotation_re = next_re;
            }
            block += length;
        }
        length <<= 1;
    }

    let scale = 4.0 / SPECTRUM_WINDOW as f32;
    let mut magnitudes = [0.0f32; SPECTRUM_BINS];
    for (bin, magnitude) in magnitudes.iter_mut().enumerate() {
        *magnitude = (re[bin].hypot(im[bin]) * scale).min(1.0);
    }
    magnitudes
}

impl Widget for AudioVisualizer {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::AUDIO_VISUALIZER_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::AUDIO_VISUALIZER_DEFAULT_HEIGHT,
        )
    }

    /// Drives the peak-hold markers' clock.
    ///
    /// The trait's `tick` is the animation bus's entry point, which is the only place a *duration*
    /// can be observed: `draw` runs when something asks for a repaint, and a paused stream asks for
    /// none. Returning `advance_peak_hold`'s answer keeps the bus ticking while markers fall and lets
    /// it stop once they have settled, rather than asking a still visualizer to run at frame rate
    /// forever.
    fn tick(&mut self, delta_ms: u32) -> bool {
        let peaked = self.advance_peak_hold(delta_ms as u64);
        let smoothed = self.advance_smoothing(delta_ms);
        let reported = self.advance_level_report(delta_ms);
        // The "am I animating?" answer is read from these flags, not recomputed from the bar
        // values: a query must not run the spectrum derivation (L-P05).
        self.peak_moving = peaked;
        self.smoothing_moving = smoothed;
        if peaked || smoothed {
            self.base.request_redraw();
        }
        peaked || smoothed || reported
    }

    /// Whether the peak-hold markers are still falling, i.e. whether a frame is owed.
    ///
    /// # Why this is not the trait's default
    ///
    /// `Widget::is_animating` answers `false` unless a control overrides it, and a host uses the
    /// answer to decide whether a frame is needed **before** paying for a sweep. A visualizer whose
    /// markers were still falling answered `false`, so such a host would stop scheduling frames
    /// mid-decline and the markers would freeze part-way — the exact "held forever" shape the
    /// peak-hold clock was written to end, reintroduced one layer up. The condition mirrors
    /// [`Self::advance_peak_hold`]: a marker is in flight while peak-hold is on and it sits above the
    /// bar it tracks.
    fn is_animating(&self) -> bool {
        self.peak_moving || self.smoothing_moving
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `AudioVisualizer` implements `Draw`, so `Some(self)` is total and cannot be wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();

    /// Resolves a published event name to the control's own signal.
    ///
    /// Without these arms the three names were accepted by `connect_event` and wired to nothing:
    /// the meter emitted into its own fields while the hub held slots nobody called.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "level_changed" => {
                Some(EventSignalRef::mapped("level_changed", &self.level_changed, |v| {
                    CapabilityValue::Float(*v as f64)
                }))
            }
            "peak_exceeded" => {
                Some(EventSignalRef::mapped("peak_exceeded", &self.peak_exceeded, |v| {
                    CapabilityValue::Float(*v as f64)
                }))
            }
            "mode_changed" => {
                Some(EventSignalRef::mapped("mode_changed", &self.mode_changed, |v| {
                    CapabilityValue::String((*v).clone())
                }))
            }
            _ => None,
        }
    }
}

/// `AudioVisualizer`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_media.in.rs` / `access_write_media.in.rs` dispatch: `bar_count`
/// is published as an unsigned integer and clamped to at least one bar on write.
impl WidgetProperties for AudioVisualizer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "bar_count" => Ok(CapabilityValue::UInt(self.bar_count() as u64)),
            "peak_threshold" => Ok(CapabilityValue::Float(self.peak_threshold() as f64)),
            "mode" => Ok(CapabilityValue::String(
                match self.mode() {
                    VisualizerMode::Waveform => "waveform",
                    VisualizerMode::Spectrum => "spectrum",
                }
                .to_string(),
            )),
            "level_report_interval" => {
                Ok(CapabilityValue::UInt(self.level_report_interval() as u64))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "bar_count" => {
                self.set_bar_count(expect_usize(value)?);
                Ok(())
            }
            "peak_threshold" => {
                self.set_peak_threshold(expect_f32(value)?);
                Ok(())
            }
            "mode" => {
                let mode = match expect_string(value)?.to_ascii_lowercase().as_str() {
                    "waveform" => VisualizerMode::Waveform,
                    "spectrum" => VisualizerMode::Spectrum,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_mode(mode);
                Ok(())
            }
            "level_report_interval" => {
                let ms = expect_usize(value)?.min(u32::MAX as usize) as u32;
                self.set_level_report_interval(ms);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "bar_count",
            "peak_threshold",
            "mode",
            "level_report_interval",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for AudioVisualizer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let w = rect.width as f32;
        let h = rect.height as f32;

        if w <= 0.0 || h <= 0.0 {
            return;
        }

        // Draw background
        context.face_with_gradient(
            rect,
            self.resolved_background(),
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // Calculate bar layout
        let total_spacing = self.bar_spacing * (self.bar_count as f32 + 1.0);
        let bar_width = ((w - total_spacing) / self.bar_count as f32).max(1.0);
        let center_y = rect.y as f32 + h / 2.0;
        let half_height = h / 2.0 - 2.0;

        // Prepare sample data: one shared derivation, so the marker and its bar cannot measure
        // different samples. Empty samples yield the synthetic sweep, which is what makes a
        // data-less control still a usable preview. The buffer is the control's own and is
        // reused, so a playing visualizer allocates nothing per frame (L-P05).
        self.bar_buffer.clear();
        self.bar_buffer.reserve(self.bar_count);
        for index in 0..self.bar_count {
            let level = self.display_level(index);
            self.bar_buffer.push(level);
        }
        let explicit_bar_color = self.bar_color;

        for (i, &value) in self.bar_buffer.iter().enumerate() {
            let value = value.min(1.0);
            let bar_height = (value * half_height).max(1.0);
            let x = rect.x as f32 + self.bar_spacing + i as f32 * (bar_width + self.bar_spacing);
            let bar_rect = Rect::new(
                x as i32,
                (center_y - bar_height) as i32,
                bar_width as u32,
                (bar_height * 2.0) as u32,
            );

            // Color gradient based on amplitude. A caller's colour replaces the gradient for
            // every bar, which is what makes `set_bar_color` visible (MW-02).
            let intensity = (value * 255.0) as u8;
            let bar_color = explicit_bar_color.unwrap_or_else(|| {
                if value > 0.7 {
                    Color::rgba(255, intensity, intensity, 255)
                } else if value > 0.4 {
                    Color::rgba(intensity, 200, 255, 255)
                } else {
                    Color::rgba(intensity / 2, intensity / 2, 200, 255)
                }
            });

            if self.mirror {
                // Draw only top half-bar and mirror it
                let top_bar = Rect::new(
                    x as i32,
                    (center_y - bar_height) as i32,
                    bar_width as u32,
                    bar_height as u32,
                );
                let bottom_bar =
                    Rect::new(x as i32, center_y as i32, bar_width as u32, bar_height as u32);
                context.fill_rect(top_bar, bar_color);
                context.fill_rect(bottom_bar, bar_color);
            } else {
                context.fill_rect(bar_rect, bar_color);
            }

            // Peak hold indicator. The marker's own value is raised by `advance_peak_hold` on the
            // animation bus, not here: a duration is only observable if time passes for the widget,
            // and `draw` runs only when something asks for a repaint. This arm reads the value and
            // paints it, so the two callers cannot disagree about what the marker is.
            if self.peak_hold {
                let peak_value = self.peak_values[i];
                if peak_value > 0.0 {
                    let peak_y = center_y - peak_value * half_height;
                    let peak_rect = Rect::new(
                        x as i32,
                        peak_y as i32,
                        bar_width as u32,
                        std::cmp::max(1, (bar_width * 0.5) as u32),
                    );
                    context.fill_rect(peak_rect, Color::rgba(255, 255, 100, 255));
                }
            }
        }
    }
}

impl EventHandler for AudioVisualizer {
    /// Toggles peak-hold on a press that lands on the visualizer.
    ///
    /// The position used to be discarded, so a press on any other control in the same
    /// window flipped this one's peak-hold. The duplicate `MousePress` arm below it —
    /// an empty body for any non-left button — was dead weight and is gone with it.
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. }
                if *button == 1 && self.geometry().contains_point(*pos) =>
            {
                self.peak_hold = !self.peak_hold;
                self.base.request_redraw();
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

    #[test]
    fn audio_visualizer_default_state() {
        let av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        assert_eq!(av.bar_count(), 64);
        assert!(av.samples().is_empty());
        assert!(!av.is_mirror_enabled());
        assert!(!av.is_peak_hold_enabled());
        assert_eq!(av.kind(), WidgetKind::AudioVisualizer);
    }

    #[test]
    fn audio_visualizer_set_samples() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        let data = vec![0.0, 0.5, 1.0, -0.5, 0.0];
        av.set_samples(data.clone());
        assert_eq!(av.samples(), data);
    }

    #[test]
    fn audio_visualizer_add_and_clear_samples() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.add_sample(0.5);
        av.add_sample(-0.3);
        av.add_sample(0.8);
        assert_eq!(av.samples().len(), 3);
        av.clear_samples();
        assert!(av.samples().is_empty());
    }

    #[test]
    fn audio_visualizer_set_bar_count() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_bar_count(32);
        assert_eq!(av.bar_count(), 32);
        av.set_bar_count(0); // Should clamp to 1
        assert_eq!(av.bar_count(), 1);
    }

    #[test]
    fn audio_visualizer_toggle_mirror_and_peak_hold() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        assert!(!av.is_mirror_enabled());
        av.set_mirror(true);
        assert!(av.is_mirror_enabled());
        assert!(!av.is_peak_hold_enabled());
        av.set_peak_hold(true);
        assert!(av.is_peak_hold_enabled());
    }

    /// `is_animating` answers for the peak-hold clock, and stops once the markers settle.
    ///
    /// # The defect this pins
    ///
    /// `Widget::is_animating` answers `false` unless a control overrides it, and a host consults the
    /// answer to decide whether a frame is needed **before** paying for a sweep. A visualizer whose
    /// markers were still falling answered `false`, so such a host would stop scheduling frames
    /// mid-decline and the markers would freeze part-way — the "held forever" shape the peak-hold
    /// clock was written to end, reintroduced one layer up.
    ///
    /// # Why the assertion is a pair, not a single check
    ///
    /// "It reports `true` while markers fall" alone would pass for a control that answered `true`
    /// forever, which would pin a host at frame rate for the life of the widget. The second half — it
    /// goes back to `false` once the markers have settled — is what makes the answer an animation
    /// rather than a traffic loop.
    #[test]
    fn is_animating_follows_the_peak_hold_clock() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        assert!(!av.is_animating(), "a rest visualizer owes no frames");

        av.set_peak_hold(true);
        av.set_bar_count(4);
        av.set_peak_hold_duration(0);
        // A loud window raises the markers to its own level.
        av.set_samples(vec![1.0, 1.0, 1.0, 1.0]);
        assert!(av.tick(16), "raising the markers is a frame-worthy change");

        // A quiet window then leaves the markers *above* their bars, which is the state the clock
        // exists to animate and therefore the state in which a frame is owed.
        av.set_samples(vec![0.0, 0.0, 0.0, 0.0]);
        assert!(av.is_animating(), "markers above their bars are in flight");

        // Let the clock run: the markers fall to their bars and then stop.
        for _ in 0..400 {
            if !av.tick(16) {
                break;
            }
        }
        assert!(!av.is_animating(), "a settled visualizer must stop asking for frames");
    }

    #[test]
    fn audio_visualizer_bar_spacing() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        assert!((av.bar_spacing() - 2.0).abs() < f32::EPSILON);
        av.set_bar_spacing(5.0);
        assert!((av.bar_spacing() - 5.0).abs() < f32::EPSILON);
    }

    /// The panel colour is the theme's surface unless the caller chose one.
    ///
    /// # The defect this pins
    ///
    /// `new()` initialised `background_color` to a fixed `rgb(20, 20, 30)` and `draw` read that
    /// field directly, so the visualizer painted a near-black rectangle on the light appearance too,
    /// and the `style.background_color` that `apply_active_theme` wrote was read by **nobody** — a
    /// declared value with no consumer. It now resolves to `surface_container` unless
    /// `set_background_color` was called, which is what the `Option` field distinguishes.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_background_defaults_to_the_theme_and_still_honours_an_override() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        let themed = |appearance| -> Color {
            crate::theme::global_theme_manager().set_appearance(appearance);
            AudioVisualizer::new(Rect::new(0, 0, 300, 150)).background_color()
        };

        let dark = themed(crate::theme::AppearanceMode::Dark);
        let light = themed(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark, light,
            "an un-configured visualizer must take the theme's surface, not a fixed near-black; \
             both were {dark:?}"
        );

        // An explicit choice still wins over the theme, which is what keeps the setter meaningful.
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        let chosen = Color::rgb(1, 2, 3);
        av.set_background_color(chosen);
        assert_eq!(av.background_color(), chosen, "a caller's colour must survive the theme");
    }

    // ── `peak_hold_duration` drives the markers' clock ──

    /// A marker is held for `peak_hold_duration` and then falls; the duration is what decides when.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `peak_hold_duration` was stored, published (getter, setter, schema row, round-trip test) and
    /// read by nothing. Worse, the peak values themselves were raised in `draw` and **never fell**,
    /// so a marker sat at its highest ever value for the widget's whole life. "Hold for 500 ms" was
    /// implemented as "hold forever", and no duration could change that because no clock existed.
    #[test]
    fn peak_hold_holds_for_its_duration_then_falls() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_bar_count(1);
        av.set_peak_hold(true);
        av.set_peak_hold_duration(500);
        // A loud moment, then silence: the marker is raised by the loud one and has to fall on its
        // own once the bar left it.
        av.set_samples(vec![1.0]);
        assert!(av.advance_peak_hold(16), "the marker rises to meet the bar");
        assert!((av.peak_values[0] - 1.0).abs() < 0.001, "it rose to the bar's level");

        av.set_samples(vec![0.0]);
        // Well inside the hold window: the marker does not move at all, which is the "hold" half.
        assert!(!av.advance_peak_hold(400), "a held marker owes no frame");
        assert!((av.peak_values[0] - 1.0).abs() < 0.001, "still at its peak, not falling yet");

        // Past the window: now it descends rather than vanishing.
        assert!(av.advance_peak_hold(200), "past the hold, the marker moves");
        let falling = av.peak_values[0];
        assert!(
            falling < 1.0 && falling > 0.0,
            "a released marker descends instead of jumping to zero: {falling}"
        );

        // The duration is the knob: a longer hold keeps it up for longer from the same moment.
        let mut patient = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        patient.set_bar_count(1);
        patient.set_peak_hold(true);
        patient.set_peak_hold_duration(5_000);
        patient.set_samples(vec![1.0]);
        patient.advance_peak_hold(16);
        patient.set_samples(vec![0.0]);
        assert!(!patient.advance_peak_hold(200), "a 5 s hold is still holding at 200 ms");
        assert!((patient.peak_values[0] - 1.0).abs() < 0.001);

        // Once it has fallen back to the bar it marks, it is released and stops owing frames.
        for _ in 0..40 {
            av.advance_peak_hold(100);
        }
        assert!(
            (av.peak_values[0] - 0.0).abs() < 0.001,
            "a fully released marker sits on its bar: {}",
            av.peak_values[0]
        );
        assert!(!av.advance_peak_hold(100), "a settled marker owes no frame");
    }

    /// The duration is genuinely *observable*: two controls differing only in it render differently
    /// at the same instant.
    ///
    /// The state assertions above say when a marker moves; this says the marker is painted where the
    /// state says it is -- a `peak_values` nothing drew would satisfy every number above.
    #[test]
    fn the_hold_duration_reaches_the_pixels() {
        let build = |duration| -> String {
            let mut av = AudioVisualizer::new(Rect::new(0, 0, 200, 60));
            av.set_bar_count(1);
            av.set_peak_hold(true);
            av.set_peak_hold_duration(duration);
            av.set_samples(vec![1.0]);
            av.advance_peak_hold(16);
            av.set_samples(vec![0.0]);
            // One step past the shorter window and inside the longer one.
            av.advance_peak_hold(600);
            render_to_svg(&mut av)
        };
        assert_ne!(
            build(500),
            build(5_000),
            "the same instant must look different under two hold durations"
        );
    }

    /// Every bar carries its own clock, so one bar going quiet does not release its neighbour's peak.
    #[test]
    fn each_bar_holds_its_own_peak() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_bar_count(2);
        av.set_peak_hold(true);
        av.set_peak_hold_duration(0);
        // The first bar is loud, the second silent: both markers are raised where they stand.
        av.set_samples(vec![1.0, 0.0]);
        av.advance_peak_hold(16);
        assert!((av.peak_values[0] - 1.0).abs() < 0.001);
        assert!(av.peak_values[1].abs() < 0.001);

        // Bar 0 goes quiet while bar 1 becomes loud. Bar 1's marker must rise, and bar 0's must fall
        // without taking bar 1's with it.
        av.set_samples(vec![0.0, 1.0]);
        av.advance_peak_hold(1_000);
        assert!((av.peak_values[1] - 1.0).abs() < 0.001, "bar 1's marker rose");
        assert!(
            av.peak_values[0] < 1.0,
            "bar 0's marker fell on its own clock: {}",
            av.peak_values[0]
        );
    }

    // ── The two bar-derivation modes ──

    /// The default mode is the historical time-domain one, and the accessor round-trips.
    #[test]
    fn mode_defaults_to_waveform_and_round_trips() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        assert_eq!(av.mode(), VisualizerMode::Waveform);
        assert_eq!(VisualizerMode::default(), VisualizerMode::Waveform);

        av.set_mode(VisualizerMode::Spectrum);
        assert_eq!(av.mode(), VisualizerMode::Spectrum);
        av.set_mode(VisualizerMode::Waveform);
        assert_eq!(av.mode(), VisualizerMode::Waveform);
    }

    /// Waveform mode still means "mean absolute sample per chunk", i.e. the old formula.
    ///
    /// The mode was added so the module's frequency-band claim could be made true, not to change
    /// what the default draws. Both the chunked formula and the concrete numbers are asserted: the
    /// first catches a different chunking, the second catches a different statistic (peak instead
    /// of mean, for instance) that a self-consistent formula check would miss.
    #[test]
    fn waveform_mode_keeps_the_mean_magnitude_formula() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_bar_count(4);
        let samples = vec![0.5, -0.25, 0.75, -1.0, 0.0, 0.25, -0.5, 1.0];
        av.set_samples(samples.clone());
        assert_eq!(av.mode(), VisualizerMode::Waveform);

        let step = samples.len() as f32 / 4.0;
        for index in 0..4 {
            let start = (index as f32 * step) as usize;
            let end = (((index as f32 + 1.0) * step) as usize).min(samples.len());
            let expected: f32 = samples[start..end].iter().map(|sample| sample.abs()).sum::<f32>()
                / (end - start) as f32;
            assert!(
                (av.bar_value(index) - expected).abs() < 1e-6,
                "bar {index}: {} vs {expected}",
                av.bar_value(index)
            );
        }

        for (index, expected) in [0.375f32, 0.875, 0.125, 0.75].into_iter().enumerate() {
            assert!(
                (av.bar_value(index) - expected).abs() < 1e-6,
                "bar {index} must be the mean magnitude {expected}, got {}",
                av.bar_value(index)
            );
        }
    }

    /// A 1 kHz sine fed at 48 kHz peaks in the bar that covers 1 kHz on the log axis.
    ///
    /// # Why the assertion is a region, not a bar
    ///
    /// A 256-sample window at 48 kHz has 187.5 Hz bins, so 1 kHz falls between bins 5 and 6 and a
    /// Hann main lobe spills into its neighbours. Claiming one exact bar would pin float leakage;
    /// what must hold is that the energy lands on the log-frequency region around 1 kHz and nowhere
    /// near the ends of the axis. The expected region is computed here from the documented axis
    /// (logarithmic from bin 1 to Nyquist) rather than read out of the implementation.
    #[test]
    fn spectrum_mode_places_a_one_kilohertz_sine_in_the_expected_band() {
        const BARS: usize = 64;
        let sample_rate = 48_000.0f32;
        let tone = 1_000.0f32;

        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_mode(VisualizerMode::Spectrum);
        av.set_bar_count(BARS);
        assert_eq!(av.mode(), VisualizerMode::Spectrum);

        let samples: Vec<f32> = (0..SPECTRUM_WINDOW)
            .map(|n| (2.0 * std::f32::consts::PI * tone * n as f32 / sample_rate).sin())
            .collect();
        av.set_samples(samples.clone());

        // The transform itself: 1000 / (48000 / 256) = 5.33, so bins 5 and 6 hold the tone.
        let mut window = [0.0f32; SPECTRUM_WINDOW];
        window.copy_from_slice(&samples);
        let magnitudes = fft_magnitudes(&window);
        let bin_width = sample_rate / SPECTRUM_WINDOW as f32;
        let expected_bin = tone / bin_width;
        assert!(
            (expected_bin - 5.33).abs() < 0.01,
            "the test's own arithmetic drifted: {expected_bin}"
        );
        let peak_bin = (0..SPECTRUM_BINS).fold(0usize, |best, bin| {
            if magnitudes[bin] > magnitudes[best] {
                bin
            } else {
                best
            }
        });
        assert!(
            peak_bin == 5 || peak_bin == 6,
            "a 1 kHz tone must peak on bins 5/6, got {peak_bin}"
        );
        assert!(
            magnitudes[peak_bin] > 0.5,
            "the tone must carry real energy, got {}",
            magnitudes[peak_bin]
        );

        // Expected bar region: the log bands overlapping one bin width around the tone.
        let lowest = bin_width; // bin 1, where the axis starts
        let highest = sample_rate / 2.0; // Nyquist, the top of the half spectrum
        let ratio = highest / lowest;
        let band = |index: usize| {
            (
                lowest * ratio.powf(index as f32 / BARS as f32),
                lowest * ratio.powf((index + 1) as f32 / BARS as f32),
            )
        };
        let expected_region: Vec<usize> = (0..BARS)
            .filter(|&index| {
                let (low, high) = band(index);
                high >= tone - bin_width && low <= tone + bin_width
            })
            .collect();
        assert!(
            !expected_region.is_empty(),
            "the expected region must not be empty, or the assertion below is vacuous"
        );

        let peak_bar =
            (0..BARS).fold(
                0usize,
                |best, bar| {
                    if av.bar_value(bar) > av.bar_value(best) {
                        bar
                    } else {
                        best
                    }
                },
            );
        assert!(
            expected_region.contains(&peak_bar),
            "peak bar {peak_bar} (band {:?}) must sit in the expected region {expected_region:?}",
            band(peak_bar)
        );

        // The ends of the axis must stay quiet; otherwise "the peak" is meaningless.
        assert!(
            av.bar_value(0) < 0.1,
            "the low end must not pick up a 1 kHz tone: {}",
            av.bar_value(0)
        );
        assert!(
            av.bar_value(BARS - 1) < 0.1,
            "the high end must not pick up a 1 kHz tone: {}",
            av.bar_value(BARS - 1)
        );
    }

    /// A short history is zero-padded, not treated as a slower signal.
    ///
    /// If the widget ran the transform over whatever length it had and *interpreted* that as a
    /// 256-point window, a 64-sample history would put the tone four times lower on the axis and
    /// the bars would lie about its frequency. Zero padding keeps the observation time — and
    /// therefore the bin spacing — fixed. The second half of the test also pins cache invalidation:
    /// a stale cached spectrum would leave the short history reading the full window's level.
    #[test]
    fn a_short_history_is_zero_padded_instead_of_stretched() {
        const BARS: usize = 64;
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_bar_count(BARS);
        av.set_mode(VisualizerMode::Spectrum);

        let tone = |count: usize| -> Vec<f32> {
            (0..count)
                .map(|n| (2.0 * std::f32::consts::PI * 1_000.0 * n as f32 / 48_000.0).sin())
                .collect()
        };
        let peak_bar =
            |av: &AudioVisualizer| {
                (0..BARS).fold(0usize, |best, bar| {
                    if av.bar_value(bar) > av.bar_value(best) {
                        bar
                    } else {
                        best
                    }
                })
            };

        av.set_samples(tone(SPECTRUM_WINDOW));
        let full_peak = peak_bar(&av);
        let full_level = av.bar_value(full_peak);

        av.set_samples(tone(SPECTRUM_WINDOW / 4));
        let short_peak = peak_bar(&av);
        assert!(
            short_peak.abs_diff(full_peak) <= 2,
            "zero padding must keep the tone's band, full={full_peak} short={short_peak}"
        );
        assert!(
            av.bar_value(short_peak) > 0.05,
            "the short window still carries the tone: {}",
            av.bar_value(short_peak)
        );
        assert!(
            av.bar_value(short_peak) < full_level,
            "a changed history must invalidate the cached spectrum"
        );
    }

    /// A streaming host cannot grow the sample history without bound.
    ///
    /// `add_sample` used to push for the lifetime of the widget, and `set_samples` stored whatever
    /// length it was handed. Both are now capped at two analysis windows, keeping the *newest*
    /// samples so the FFT sees the recent signal rather than the first one ever received.
    #[test]
    fn sample_history_is_bounded() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        for step in 0..5_000 {
            av.add_sample((step as f32 / 5_000.0) - 0.5);
        }
        assert_eq!(
            av.samples().len(),
            SAMPLE_HISTORY_LIMIT,
            "a stream must not grow the history without bound"
        );
        let newest = (4_999.0 / 5_000.0) - 0.5;
        assert!(
            (av.samples()[SAMPLE_HISTORY_LIMIT - 1] - newest).abs() < 1e-6,
            "the newest sample survives the cap"
        );
        let oldest = ((5_000 - SAMPLE_HISTORY_LIMIT) as f32 / 5_000.0) - 0.5;
        assert!((av.samples()[0] - oldest).abs() < 1e-6, "the oldest samples are the ones dropped");

        // `set_samples` enforces the same bound and keeps the tail as well.
        let bulk: Vec<f32> = (0..4_096).map(|index| index as f32 / 4_096.0).collect();
        av.set_samples(bulk);
        assert_eq!(av.samples().len(), SAMPLE_HISTORY_LIMIT);
        assert!((av.samples()[SAMPLE_HISTORY_LIMIT - 1] - (4_095.0 / 4_096.0)).abs() < 1e-6);
        assert!((av.samples()[0] - (3_584.0 / 4_096.0)).abs() < 1e-6);
    }

    /// Host-supplied bins win over the widget's own FFT, and the host path is deterministic.
    ///
    /// Precedence is proved by *moving* the peak: a 1 kHz tone makes the derived spectrum peak in
    /// the low half of the axis, while a host spectrum with its energy in a high bin must move the
    /// peak to the high half. Clearing the host spectrum returns the derived peak, which is the
    /// other half of the same contract.
    #[test]
    fn a_host_supplied_spectrum_takes_precedence() {
        const BARS: usize = 64;
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_bar_count(BARS);
        av.set_mode(VisualizerMode::Spectrum);
        let samples: Vec<f32> = (0..SPECTRUM_WINDOW)
            .map(|n| (2.0 * std::f32::consts::PI * 1_000.0 * n as f32 / 48_000.0).sin())
            .collect();
        av.set_samples(samples);

        let peak =
            |av: &AudioVisualizer| {
                (0..BARS).fold(0usize, |best, bar| {
                    if av.bar_value(bar) > av.bar_value(best) {
                        bar
                    } else {
                        best
                    }
                })
            };
        let derived_peak = peak(&av);
        assert!(
            derived_peak < BARS / 2,
            "the widget's own FFT peaks low for a 1 kHz tone: {derived_peak}"
        );

        let mut bins = vec![0.0f32; SPECTRUM_BINS];
        bins[64] = 1.0;
        av.set_spectrum(bins);
        let host_peak = peak(&av);
        assert!(
            host_peak > BARS / 2,
            "the host's high bin must move the peak up, got bar {host_peak}"
        );
        assert_eq!(av.bar_value(host_peak), 1.0, "a full-scale host bin is drawn full scale");

        // The supplied bins are the whole story: only the bands containing bin 64 carry energy.
        let sounding = (0..BARS).filter(|&bar| av.bar_value(bar) > 0.0).count();
        assert!(
            (1..=2).contains(&sounding),
            "only the band(s) around the supplied bin may sound, got {sounding} bars"
        );

        // Same input, same output: the host path has no hidden state or clock in it.
        let first: Vec<f32> = (0..BARS).map(|bar| av.bar_value(bar)).collect();
        let second: Vec<f32> = (0..BARS).map(|bar| av.bar_value(bar)).collect();
        assert_eq!(first, second, "the host spectrum path must be deterministic");

        // An empty spectrum is "no host data", not "all zeros": the FFT answers again.
        av.set_spectrum(Vec::new());
        assert_eq!(
            peak(&av),
            derived_peak,
            "an empty spectrum must return the widget to its own FFT"
        );
    }

    #[test]
    fn smoothing_approaches_the_signal_instead_of_jumping() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_bar_count(4);
        av.set_samples(vec![1.0, 1.0, 1.0, 1.0]);
        let raw = av.bar_value(0);
        av.set_smoothing(100.0, 100.0);
        assert!(av.is_smoothing_enabled());
        assert_eq!(av.display_level(0), 0.0, "a fresh smoother starts at rest");
        av.tick(16);
        let first = av.display_level(0);
        assert!(first > 0.0 && first < raw, "one tick is a step toward the signal: {first}");
        av.tick(1000);
        assert!((av.display_level(0) - raw).abs() < 1e-3);
    }

    #[test]
    fn the_meter_publishes_throttled_level_and_peak_reports() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        let levels = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let peaks = std::sync::Arc::new(std::sync::Mutex::new(0.0f32));
        let modes = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let l = levels.clone();
        av.level_changed.connect(move |_value: std::sync::Arc<f32>| {
            l.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        let p = peaks.clone();
        av.peak_exceeded.connect(move |value: std::sync::Arc<f32>| {
            *p.lock().unwrap() = *value;
        });
        let m = modes.clone();
        av.mode_changed.connect(move |value: std::sync::Arc<String>| {
            *m.lock().unwrap() = (*value).clone();
        });

        av.set_peak_threshold(0.5);
        av.set_level_report_interval(50);
        av.set_samples(vec![1.0, -1.0, 0.5, 0.5]);
        assert!(!av.tick(16), "a report is not due yet");
        assert_eq!(levels.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(av.tick(50));
        assert_eq!(levels.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(*peaks.lock().unwrap() > 0.99);

        av.set_mode(VisualizerMode::Spectrum);
        assert_eq!(*modes.lock().unwrap(), "spectrum");
        for name in ["level_changed", "peak_exceeded", "mode_changed"] {
            assert!(av.event_signal_dyn(name).is_some(), "{name} must resolve");
        }
    }

    #[test]
    fn bulk_samples_keep_the_ring_ordered_and_bounded() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_samples_bulk(&[0.1, 0.2, 0.3]);
        assert_eq!(av.samples(), vec![0.1, 0.2, 0.3]);
        let block: Vec<f32> =
            (0..SAMPLE_HISTORY_LIMIT + 100).map(|i| (i % 7) as f32 / 7.0).collect();
        av.set_samples_bulk(&block);
        assert_eq!(av.samples().len(), SAMPLE_HISTORY_LIMIT);
        // The newest block tail survives, in order.
        assert_eq!(av.samples()[SAMPLE_HISTORY_LIMIT - 1], block[block.len() - 1]);
    }
}
