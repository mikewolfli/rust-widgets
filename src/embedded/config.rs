// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Device description and resource accounting for embedded targets.
//!
//! [`EmbeddedConfig`] is the static description of the device a build is
//! targeting — panel size, DPI, and which expensive visual effects are allowed.
//! [`ResourceManager`] is the dynamic ledger a caller uses to keep track of how
//! much of a device-class budget it has spent. The two do not talk to each
//! other: a config is advisory input, a ledger is a counter the caller maintains.
//!
//! The *effective* budgets for the running build do not live here. They come from
//! [`crate::platform::profile::surface_policy`], read back through
//! [`crate::embedded::flags`], which also applies any caller-requested reduction
//! from [`EmbeddedConfig::low_memory_mode`].

use crate::core::Size;
/// Tunable knobs describing the device an embedded build is drawing for.
///
/// This is the *input* to an embedded setup, not a live resource ledger: it is
/// read by [`crate::embedded::flags::init_embedded`] to set the run-time mode
/// flags, and the *effective* budgets are then read back from
/// [`crate::embedded::flags`], which consults
/// [`crate::platform::profile::surface_policy`] as well. Nothing on this struct
/// enforces anything on its own — the counts and limits here are advisory, and
/// the renderer does not check them.
///
/// The builder setters all take `self` by value and return `Self`, so a config
/// is assembled as one expression; see [`EmbeddedConfig::new`] for the defaults.
#[derive(Debug, Clone)]
pub struct EmbeddedConfig {
    /// The panel the UI is laid out for, in logical pixels.
    ///
    /// Advisory: it seeds the initial surface size, it does not clamp resizes.
    pub screen_size: Size,
    /// DPI to report instead of querying the host, or `None` to leave DPI
    /// detection alone.
    ///
    /// When set, [`crate::embedded::flags::init_embedded`] forwards it to
    /// [`crate::embedded::dpi::set_fixed_dpi`], which pins the process-wide
    /// scale factor. Dots per inch, not a scale factor: `96` means 1:1.
    pub fixed_dpi: Option<u32>,
    /// Whether the caller asked for the reduced budget.
    ///
    /// Derived from `fixed_dpi` in [`EmbeddedConfig::new`] and set by
    /// [`EmbeddedConfig::low_memory`]; it is what
    /// [`crate::embedded::flags::init_embedded`] passes to
    /// `set_low_memory_mode`, which narrows the profile's own budget but can
    /// never widen it.
    pub low_memory_mode: bool,
    /// Advisory cap on simultaneously mounted widgets. Default `100`;
    /// [`EmbeddedConfig::low_memory`] lowers it to `50`.
    ///
    /// Distinct from [`crate::embedded::flags::max_widgets`], which reports the
    /// budget the running profile actually has.
    pub max_widgets: usize,
    /// Advisory cap on one texture's edge length, in pixels. Default `1024`;
    /// [`EmbeddedConfig::low_memory`] lowers it to `512`.
    ///
    /// A cap on the edge, not on area or bytes: such a texture needs
    /// `max_texture_size²` pixels before any format/alignment multiplier.
    pub max_texture_size: u32,
    /// Whether animation is allowed at all. Default `true`.
    ///
    /// Intended as a yes/no feature gate rather than a duration or frame-rate
    /// limit — it carries no frame budget. [`EmbeddedConfig::low_memory`]
    /// clears it, because a changing frame must be rebuilt and re-uploaded.
    pub enable_animations: bool,
    /// Whether widgets may draw drop shadows. Default `false`.
    ///
    /// Shadows need a larger blurred pass than the widget's own bounds, which is
    /// why the embedded default is off rather than on.
    pub enable_shadows: bool,
    /// Whether widgets may paint gradients. Default `true`.
    ///
    /// A gradient costs per-pixel interpolation but no extra allocation;
    /// [`EmbeddedConfig::low_memory`] clears it.
    pub enable_gradients: bool,
    /// Multiplier applied to font sizes when text is laid out. Default `1.0`
    /// (unscaled).
    ///
    /// Kept without units: it is a ratio, not a point size, so `1.5` means
    /// "half again as large". [`EmbeddedConfig::with_font_scale`] clamps it to
    /// `0.5..=3.0` rather than rejecting out-of-range input.
    pub font_scale: f32,
    /// Whether touch input is expected from the device. Default `true`.
    ///
    /// Gates touch affordances such as larger hit targets; it does not enable
    /// or disable the touch event path itself, which is a build-time feature.
    pub touch_enabled: bool,
    /// Whether the renderer may use GPU acceleration. Default `false`.
    ///
    /// Off by default because the embedded profiles rasterise in software and
    /// have no GPU backend linked in; setting it on such a build does not add
    /// one.
    pub hardware_acceleration: bool,
}
impl EmbeddedConfig {
    /// Builds a config for `screen_size`, with embedded-leaning defaults.
    ///
    /// Defaults: no fixed DPI, `low_memory_mode` off, `max_widgets` `100`,
    /// `max_texture_size` `1024`, animations and gradients on, shadows and
    /// hardware acceleration off, `font_scale` `1.0`, touch on.
    pub fn new(screen_size: Size) -> Self {
        Self {
            screen_size,
            fixed_dpi: None,
            low_memory_mode: false,
            max_widgets: 100,
            max_texture_size: 1024,
            enable_animations: true,
            enable_shadows: false,
            enable_gradients: true,
            font_scale: 1.0,
            touch_enabled: true,
            hardware_acceleration: false,
        }
    }
    /// Sets [`EmbeddedConfig::fixed_dpi`] to `dpi` and returns the config.
    ///
    /// `dpi` is dots per inch, where `96` is the unscaled baseline. `0` is not
    /// special-cased here, but it is treated as "unset" downstream:
    /// [`crate::embedded::dpi::get_fixed_dpi`] reports `None` for a stored `0`,
    /// so the scale factor stays `1.0`.
    pub fn with_fixed_dpi(mut self, dpi: u32) -> Self {
        self.fixed_dpi = Some(dpi);
        self
    }
    /// Turns on low-memory mode and narrows the advisory caps to match.
    ///
    /// Sets [`EmbeddedConfig::low_memory_mode`], drops `max_widgets` to `50`,
    /// drops `max_texture_size` to `512`, and switches off animations, shadows
    /// and gradients. It does not touch `screen_size`, `font_scale` or the DPI
    /// settings. Because it also clears `enable_animations`, calling it after a
    /// caller opted into animations silently undoes that choice — the order of
    /// builder calls matters, and this one wins.
    pub fn low_memory(mut self) -> Self {
        self.low_memory_mode = true;
        self.max_widgets = 50;
        self.max_texture_size = 512;
        self.enable_animations = false;
        self.enable_shadows = false;
        self.enable_gradients = false;
        self
    }
    /// Overrides the advisory widget cap [`EmbeddedConfig::max_widgets`].
    ///
    /// The value is stored as given, with no sanity check, so it can re-widen a
    /// cap that [`EmbeddedConfig::low_memory`] had narrowed.
    pub fn with_max_widgets(mut self, count: usize) -> Self {
        self.max_widgets = count;
        self
    }
    /// Sets [`EmbeddedConfig::touch_enabled`].
    pub fn with_touch(mut self, enabled: bool) -> Self {
        self.touch_enabled = enabled;
        self
    }
    /// Sets [`EmbeddedConfig::hardware_acceleration`].
    ///
    /// This records a preference only; on a profile whose backend has no GPU
    /// path the flag has no effect on rendering.
    pub fn with_hardware_acceleration(mut self, enabled: bool) -> Self {
        self.hardware_acceleration = enabled;
        self
    }
    /// Sets [`EmbeddedConfig::font_scale`], clamped into `0.5..=3.0`.
    ///
    /// Clamping is silent — a `NaN` or out-of-range `scale` is replaced rather
    /// than reported — so a caller that needs to know its request was refused
    /// must check the stored field afterwards.
    pub fn with_font_scale(mut self, scale: f32) -> Self {
        self.font_scale = scale.clamp(0.5, 3.0);
        self
    }
}
impl Default for EmbeddedConfig {
    /// A config for an 800x600 logical-pixel surface; see [`EmbeddedConfig::new`]
    /// for the remaining field defaults.
    fn default() -> Self {
        Self::new(Size::new(800, 600))
    }
}
/// How tight the memory budget for this device is, on a four-step ladder.
///
/// Used by [`ResourceManager::new`] to pick a memory ceiling and a widget
/// ceiling. The ordering is deliberate — `None` is the loosest and `High` is the
/// tightest *constraint*, not the largest budget — so the variants read as "how
/// constrained", which is the opposite of what the names suggest at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourceConstraint {
    /// No constraint: both ceilings are `usize::MAX`, so nothing is ever refused.
    #[default]
    None,
    /// 16 MiB and 50 widgets. Suited to a microcontroller-class target.
    Low,
    /// 64 MiB and 200 widgets.
    Medium,
    /// 256 MiB and 1000 widgets — the loosest constrained tier, for a set-top-box
    /// class device that still runs the embedded profile.
    High,
}
/// A ledger of the memory and widget budget a caller has already spent.
///
/// Both budgets are tracked as counters, and neither is connected to the real
/// allocator: [`ResourceManager::allocate`] records that a caller *claims* to
/// have taken `size` bytes, and releasing a widget or memory is likewise the
/// caller's word. Nothing is measured, so the numbers are only as honest as the
/// code that calls this. It is deliberately dependency-free — no allocation, no
/// clock, no atomics — so it can run before the runtime is up.
///
/// Memory and widgets are separate ceilings: being at the widget cap does not
/// stop a memory allocation, and vice versa.
pub struct ResourceManager {
    // constraint: ResourceConstraint,
    allocated_memory: usize,
    max_memory: usize,
    widget_count: usize,
    max_widgets: usize,
}
impl ResourceManager {
    /// Creates a ledger whose ceilings come from `constraint`.
    ///
    /// The ceilings in bytes (and widget counts) are: [`ResourceConstraint::None`]
    /// `usize::MAX`/`usize::MAX`, [`ResourceConstraint::Low`] `16 MiB`/`50`,
    /// [`ResourceConstraint::Medium`] `64 MiB`/`200`, [`ResourceConstraint::High`]
    /// `256 MiB`/`1000`. The used counters all start at zero.
    ///
    /// The chosen `constraint` is not stored, so it cannot be read back or
    /// changed later.
    pub fn new(constraint: ResourceConstraint) -> Self {
        let (max_memory, max_widgets) = match constraint {
            ResourceConstraint::None => (usize::MAX, usize::MAX),
            ResourceConstraint::Low => (16 * 1024 * 1024, 50),
            ResourceConstraint::Medium => (64 * 1024 * 1024, 200),
            ResourceConstraint::High => (256 * 1024 * 1024, 1000),
        };
        Self {
            // constraint,
            allocated_memory: 0,
            max_memory,
            widget_count: 0,
            max_widgets,
        }
    }
    /// Returns whether `size` more bytes would still fit under the memory ceiling.
    ///
    /// `size` is in bytes. A query, not a reservation: two calls that both return
    /// `true` may leave too little room for both, so it is only meaningful when
    /// the caller immediately follows it with [`ResourceManager::allocate`].
    pub fn can_allocate(&self, size: usize) -> bool {
        self.allocated_memory + size <= self.max_memory
    }
    /// Charges `size` bytes against the memory budget.
    ///
    /// `size` is in bytes. Returns `true` and adds to the total, or returns
    /// `false` and changes nothing when the result would exceed the ceiling — a
    /// refused allocation is not truncated or partially charged.
    pub fn allocate(&mut self, size: usize) -> bool {
        if self.can_allocate(size) {
            self.allocated_memory += size;
            true
        } else {
            false
        }
    }
    /// Credits `size` bytes back to the memory budget.
    ///
    /// `size` is in bytes. The total saturates at zero, so crediting more than
    /// was ever charged cannot drive the usage negative or free budget for a
    /// later allocation.
    pub fn deallocate(&mut self, size: usize) {
        self.allocated_memory = self.allocated_memory.saturating_sub(size);
    }
    /// Returns whether another widget would still fit under the widget ceiling.
    pub fn can_create_widget(&self) -> bool {
        self.widget_count < self.max_widgets
    }
    /// Charges one widget against the widget budget.
    ///
    /// Returns `true` and increments the count, or returns `false` and leaves the
    /// count alone when the ceiling has been reached.
    pub fn register_widget(&mut self) -> bool {
        if self.can_create_widget() {
            self.widget_count += 1;
            true
        } else {
            false
        }
    }
    /// Credits one widget back to the widget budget.
    ///
    /// The count saturates at zero, so an unmatched call cannot push it negative
    /// (which would silently buy extra headroom for later registrations).
    pub fn unregister_widget(&mut self) {
        self.widget_count = self.widget_count.saturating_sub(1);
    }
    /// Returns the memory charged so far, in bytes.
    ///
    /// This is the ledger's own total of [`ResourceManager::allocate`] calls, not
    /// a measurement of the process heap.
    pub fn memory_usage(&self) -> usize {
        self.allocated_memory
    }
    /// Returns memory usage as a percentage of the ceiling, in `0.0..=100.0`.
    ///
    /// Returns exactly `0.0` for [`ResourceConstraint::None`], whose ceiling is
    /// `usize::MAX`: the division would be a rounding-to-zero artefact, so it is
    /// special-cased rather than reported as a meaningless fraction. A real
    /// ceiling is divided as `f32`, which loses precision above 2^24 bytes — fine
    /// for a pressure indicator, not for accounting.
    pub fn memory_percentage(&self) -> f32 {
        if self.max_memory == usize::MAX {
            0.0
        } else {
            (self.allocated_memory as f32 / self.max_memory as f32) * 100.0
        }
    }
    /// Returns how many widgets are currently registered.
    pub fn widget_count(&self) -> usize {
        self.widget_count
    }
    /// Returns whether the ledger has crossed either pressure line.
    ///
    /// Pressure means **more than** 80% of the memory ceiling, or **more than**
    /// 90% of the widget ceiling — the comparisons are strict, so exactly 80% is
    /// not pressure. Advisory only: nothing in this module acts on the answer.
    ///
    /// Note that a [`ResourceConstraint::None`] ledger reports `0.0%` memory
    /// (see [`ResourceManager::memory_percentage`]) and a ceiling of
    /// `usize::MAX`, so it can still report pressure from the *widget* ratio,
    /// whose denominator is also `usize::MAX` and therefore never reaches 0.9.
    pub fn is_under_pressure(&self) -> bool {
        self.memory_percentage() > 80.0
            || (self.widget_count as f32 / self.max_widgets as f32) > 0.9
    }
}
impl Default for ResourceManager {
    /// An unconstrained ledger; identical to
    /// `ResourceManager::new(ResourceConstraint::None)`.
    fn default() -> Self {
        Self::new(ResourceConstraint::None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_embedded_config() {
        let config = EmbeddedConfig::new(Size::new(1024, 768))
            .with_fixed_dpi(96)
            .low_memory()
            .with_touch(true);
        assert_eq!(config.screen_size.width, 1024);
        assert_eq!(config.fixed_dpi, Some(96));
        assert!(config.low_memory_mode);
        assert!(config.touch_enabled);
    }
    #[test]
    fn test_resource_manager() {
        let mut manager = ResourceManager::new(ResourceConstraint::Low);
        assert!(manager.can_allocate(1024));
        assert!(manager.allocate(1024));
        assert_eq!(manager.memory_usage(), 1024);
        manager.deallocate(512);
        assert_eq!(manager.memory_usage(), 512);
    }
    #[test]
    fn test_widget_limit() {
        let mut manager = ResourceManager::new(ResourceConstraint::Low);
        for _ in 0..50 {
            assert!(manager.register_widget());
        }
        assert!(!manager.register_widget());
        assert_eq!(manager.widget_count(), 50);
        manager.unregister_widget();
        assert_eq!(manager.widget_count(), 49);
    }
}
