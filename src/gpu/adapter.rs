// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! GPU adapter detection and selection with hardware auto-detection.
//!
//! This module provides intelligent GPU adapter selection with the following priority:
//! 1. Discrete GPU - Highest performance
//! 2. Integrated GPU - Balanced performance
//! 3. CPU Software Rendering (CPU) - Fallback mode
//!
//! # Example (requires `gpu-wgpu` feature + async runtime)
//! ```text
//! use rust_widgets::gpu::adapter::{AdapterSelector, AdapterSelectionStrategy};
//!
//! let selector = AdapterSelector::new();
//! // select_adapter_with_fallback is async and requires pollster or tokio runtime
//! let adapter_info = pollster::block_on(selector.select_adapter_with_fallback(None)).unwrap();
//! println!("Selected: {:?}", adapter_info.device_type);
//! ```
#[cfg(feature = "gpu-wgpu")]
use crate::compat::format;
use crate::compat::MiniToString;
use crate::compat::String;
#[cfg(feature = "gpu-wgpu")]
use crate::compat::{lock, Mutex, OnceLock};
use core::fmt;

/// GPU type for simplified hardware detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuType {
    /// Discrete GPU
    Discrete,
    /// Integrated GPU
    Integrated,
    /// CPU software rendering
    Cpu,
}
impl GpuType {
    /// Detects the primary GPU type from system
    ///
    /// # One selection path (G-230)
    ///
    /// Detection used to construct a fresh `wgpu::Instance` on every call and
    /// walk the ladder privately, repeating the backend initialisation the GPU
    /// manager had already paid for. It now routes through the same selection
    /// path the manager uses
    /// ([`AdapterSelector::select_adapter_with_fallback`]): the surface-
    /// independent result is memoised in the process-wide selection cache
    /// (G-62), so a repeat call answers from the cache without creating an
    /// `Instance`, and a cache miss walks the shared degradation ladder once and
    /// publishes the answer as the shared selection (G-211) that other GPU paths
    /// read.
    ///
    /// The strategy mirrors the power preference this function always probed
    /// with (`HighPerformance`), so the answer is unchanged. The only behavioural
    /// difference is on a host where *no* rung of the ladder can produce an
    /// adapter at all: the selection path reports its documented CPU-fallback
    /// answer (`GpuType::Cpu`) where this function used to report `None` — the
    /// same answer [`GpuManager`](super::manager::GpuManager) gives for the same
    /// condition, and more honest than the caller-side `unwrap_or(GpuType::Integrated)`
    /// that the `None` used to trigger.
    #[cfg(feature = "gpu-wgpu")]
    pub fn detect_primary() -> Option<Self> {
        let selector = AdapterSelector::with_strategy(AdapterSelectionStrategy::PreferPerformance);
        pollster::block_on(selector.select_adapter_with_fallback(None))
            .ok()
            .map(|info| GpuType::from(info.device_type))
    }

    /// Detects the primary GPU type from system (no-wgpu fallback)
    #[cfg(not(feature = "gpu-wgpu"))]
    pub fn detect_primary() -> Option<Self> {
        None
    }
    /// Returns a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            GpuType::Discrete => "Discrete GPU",
            GpuType::Integrated => "Integrated GPU",
            GpuType::Cpu => "CPU Software Rendering",
        }
    }
}
impl From<GpuDeviceType> for GpuType {
    fn from(device_type: GpuDeviceType) -> Self {
        match device_type {
            GpuDeviceType::DiscreteGpu => GpuType::Discrete,
            GpuDeviceType::IntegratedGpu => GpuType::Integrated,
            GpuDeviceType::VirtualGpu => GpuType::Integrated,
            GpuDeviceType::Other => GpuType::Integrated,
            GpuDeviceType::Cpu => GpuType::Cpu,
        }
    }
}
/// GPU device type with priority ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuDeviceType {
    /// Discrete GPU — highest priority
    DiscreteGpu,
    /// Integrated GPU — medium priority
    IntegratedGpu,
    /// Virtual GPU — low priority
    VirtualGpu,
    /// Other GPU types
    Other,
    /// CPU software rendering — lowest priority (fallback)
    Cpu,
}
impl GpuDeviceType {
    /// Returns the priority value (higher = better)
    pub fn priority(&self) -> u8 {
        match self {
            Self::DiscreteGpu => 5,
            Self::IntegratedGpu => 3,
            Self::VirtualGpu => 2,
            Self::Other => 2,
            Self::Cpu => 1,
        }
    }
}
impl PartialOrd for GpuDeviceType {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for GpuDeviceType {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        // A total order consistent with `Eq`: distinct variants must never
        // compare `Equal`. `priority()` alone gives `VirtualGpu` and `Other` the
        // same value (2), so it is used as the primary key and the variant's
        // discriminant as the tie-break. Without the tie-break, `cmp` returned
        // `Equal` for two variants that `==` reports as different, violating the
        // `Ord`/`Eq` consistency contract (N-S-58).
        self.priority().cmp(&other.priority()).then_with(|| self.rank().cmp(&other.rank()))
    }
}
impl GpuDeviceType {
    /// Stable intra-priority tie-break rank.
    ///
    /// Two variants that share a [`Self::priority`] still need a strict order, so
    /// each variant maps to a distinct rank. The ordering follows the priority
    /// narrative (discrete best, CPU worst) and is only ever consulted between
    /// equal priorities, so it never changes the coarse ordering.
    fn rank(&self) -> u8 {
        match self {
            Self::DiscreteGpu => 4,
            Self::IntegratedGpu => 3,
            Self::VirtualGpu => 2,
            Self::Other => 1,
            Self::Cpu => 0,
        }
    }
}
impl GpuDeviceType {
    /// Returns true if this is a hardware GPU (not CPU)
    pub fn is_hardware_gpu(&self) -> bool {
        !matches!(self, GpuDeviceType::Cpu)
    }
    /// Returns true if this is a discrete GPU
    pub fn is_discrete(&self) -> bool {
        matches!(self, GpuDeviceType::DiscreteGpu)
    }
    /// Returns true if this is an integrated GPU
    pub fn is_integrated(&self) -> bool {
        matches!(self, GpuDeviceType::IntegratedGpu)
    }
    /// Returns true if this is CPU software rendering
    pub fn is_cpu(&self) -> bool {
        matches!(self, GpuDeviceType::Cpu)
    }
    /// Returns a human-readable description (English).
    /// i18n: localize via display name lookup table if needed.
    pub fn description(&self) -> &'static str {
        match self {
            GpuDeviceType::DiscreteGpu => "Discrete GPU",
            GpuDeviceType::IntegratedGpu => "Integrated GPU",
            GpuDeviceType::VirtualGpu => "Virtual GPU",
            GpuDeviceType::Other => "Other GPU",
            GpuDeviceType::Cpu => "CPU Software Rendering",
        }
    }
    /// Returns the performance tier (0-5, higher is better)
    pub fn performance_tier(&self) -> u8 {
        match self {
            GpuDeviceType::DiscreteGpu => 5,
            GpuDeviceType::IntegratedGpu => 3,
            GpuDeviceType::VirtualGpu => 2,
            GpuDeviceType::Other => 2,
            GpuDeviceType::Cpu => 1,
        }
    }
}
impl fmt::Display for GpuDeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}
#[cfg(feature = "gpu-wgpu")]
impl From<wgpu::DeviceType> for GpuDeviceType {
    fn from(device_type: wgpu::DeviceType) -> Self {
        match device_type {
            wgpu::DeviceType::DiscreteGpu => GpuDeviceType::DiscreteGpu,
            wgpu::DeviceType::IntegratedGpu => GpuDeviceType::IntegratedGpu,
            wgpu::DeviceType::VirtualGpu => GpuDeviceType::VirtualGpu,
            wgpu::DeviceType::Other => GpuDeviceType::Other,
            wgpu::DeviceType::Cpu => GpuDeviceType::Cpu,
        }
    }
}
/// Information about a GPU adapter.
///
/// # Examples
///
/// ```rust
/// use rust_widgets::gpu::{AdapterInfo, GpuDeviceType};
///
/// let info = AdapterInfo {
///     device_type: GpuDeviceType::DiscreteGpu,
///     vendor: String::from("Mock"),
///     name: String::from("Mock GPU"),
///     backend: String::from("Vulkan"),
///     driver: String::from("Mock driver"),
///     driver_version: 1,
///     is_selected: false,
/// };
/// assert!(!info.is_selected);
/// ```
#[derive(Debug, Clone)]
pub struct AdapterInfo {
    /// Device type
    pub device_type: GpuDeviceType,
    /// Vendor name
    pub vendor: String,
    /// Device name
    pub name: String,
    /// Backend (Vulkan, Metal, DX12, etc.)
    pub backend: String,
    /// Driver info
    pub driver: String,
    /// Driver version
    pub driver_version: u64,
    /// Whether this is the selected adapter
    pub is_selected: bool,
}
impl AdapterInfo {
    /// Creates adapter info from wgpu adapter info
    #[cfg(feature = "gpu-wgpu")]
    pub fn from_wgpu(info: &wgpu::AdapterInfo) -> Self {
        Self {
            device_type: info.device_type.into(),
            vendor: format!("{:04x}", info.vendor),
            name: info.name.clone(),
            backend: format!("{:?}", info.backend),
            driver: info.driver.clone(),
            driver_version: 0, // Not available in this version of wgpu
            is_selected: false,
        }
    }
    /// Creates a CPU fallback adapter info
    pub fn cpu_fallback() -> Self {
        Self {
            device_type: GpuDeviceType::Cpu,
            vendor: "CPU".to_string(),
            name: "Software Renderer".to_string(),
            backend: "CPU".to_string(),
            driver: "Software".to_string(),
            driver_version: 0,
            is_selected: true,
        }
    }
    /// Returns true if this adapter supports high quality rendering
    pub fn supports_high_quality(&self) -> bool {
        self.device_type.performance_tier() >= 3
    }

    /// The degradation tier this adapter belongs to.
    ///
    /// Lets a host ask whether it is running on the preferred primary backend or
    /// on the OpenGL ES / software fallback, and warn accordingly. The mapping is
    /// derived from the `backend` string because `AdapterInfo` is a plain data
    /// type that also exists without the `gpu-wgpu` feature; the string values
    /// come from `format!("{:?}", wgpu::Backend)` in `Self::from_wgpu`.
    ///
    /// A CPU adapter is forced to `GpuBackendTier::Software` regardless of the
    /// backend name: lavapipe reports Vulkan and llvmpipe reports GL, so mapping
    /// on the backend string alone would report a software rasterizer as a
    /// primary or GL tier and hide the degradation (G-211).
    ///
    /// Gated on `gpu`: the tier vocabulary lives in the `backend_ladder` module,
    /// which is itself only compiled when `wgpu` is available.
    #[cfg(feature = "gpu")]
    pub fn backend_tier(&self) -> crate::gpu::backend_ladder::GpuBackendTier {
        use crate::gpu::backend_ladder::GpuBackendTier;
        if self.device_type.is_cpu() {
            return GpuBackendTier::Software;
        }
        // One decision table for the whole crate: the same function the ladder uses for
        // `wgpu::Backend` (G-229).
        GpuBackendTier::from_backend_name(self.backend.as_str())
    }

    /// Whether this adapter is running below the primary backend tier.
    #[cfg(feature = "gpu")]
    pub fn is_degraded_backend(&self) -> bool {
        self.backend_tier().is_degraded()
    }
    /// Returns true if this adapter is suitable for the target quality
    pub fn is_suitable_for_quality(&self, quality: crate::quality::QualityLevel) -> bool {
        match quality {
            crate::quality::QualityLevel::High => self.device_type.performance_tier() >= 4,
            crate::quality::QualityLevel::Medium => self.device_type.performance_tier() >= 2,
            crate::quality::QualityLevel::Low => true,
        }
    }
}
/// Strategy for adapter selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdapterSelectionStrategy {
    /// Prefer discrete GPU, fallback to integrated, then CPU
    PreferPerformance,
    /// Prefer integrated GPU for power efficiency
    PreferPowerEfficiency,
    /// Force discrete GPU only (fail if not available)
    ForceDiscrete,
    /// Force integrated GPU only (fail if not available)
    ForceIntegrated,
    /// Force CPU software rendering
    ForceCpu,
    /// Auto-detect with fallback chain
    #[default]
    Auto,
}
/// Inputs that decide a surface-independent selection.
///
/// Part of the memoisation key (G-62): if the strategy, the fallback policy or
/// the `WGPU_BACKEND` pin changes, the cached answer no longer applies.
#[cfg(feature = "gpu-wgpu")]
#[derive(Debug, Clone, Copy, PartialEq)]
struct SelectionCacheKey {
    strategy: AdapterSelectionStrategy,
    allow_fallback: bool,
    /// `WGPU_BACKEND` value observed when the entry was produced.
    pinned: Option<wgpu::Backends>,
}

/// A memoised selection result.
///
/// `AdapterInfo` is a plain value, so the result can be cloned out to every
/// caller instead of probing the adapter list again.
#[cfg(feature = "gpu-wgpu")]
#[derive(Debug, Clone)]
struct CachedSelection {
    key: SelectionCacheKey,
    result: Result<AdapterInfo, AdapterSelectionError>,
}

/// The process-wide selection cache (G-62).
///
/// Process-wide rather than per-selector because `GpuManager` builds a fresh
/// `AdapterSelector` each time; the expensive part (creating an `Instance` and
/// walking the ladder) does not depend on the selector object, only on the key.
#[cfg(feature = "gpu-wgpu")]
fn selection_cache() -> &'static Mutex<Option<CachedSelection>> {
    static CACHE: OnceLock<Mutex<Option<CachedSelection>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Adapter selector with hardware auto-detection
pub struct AdapterSelector {
    strategy: AdapterSelectionStrategy,
    allow_fallback: bool,
}
impl AdapterSelector {
    /// Creates a new adapter selector with default strategy
    pub fn new() -> Self {
        Self { strategy: AdapterSelectionStrategy::Auto, allow_fallback: true }
    }
    /// Creates a new adapter selector with specific strategy
    pub fn with_strategy(strategy: AdapterSelectionStrategy) -> Self {
        Self { strategy, allow_fallback: true }
    }
    /// Sets whether the CPU software adapter may be used when no hardware
    /// adapter exists.
    ///
    /// This gates the *terminal* rung only. A degraded hardware tier (GL instead
    /// of Vulkan/Metal/DX12) is still a real GPU and is accepted either way; it is
    /// reported through [`AdapterInfo::is_degraded_backend`] and this module's
    /// logs so the caller can warn rather than have selection silently fail
    /// (principle #45). A `WGPU_BACKEND` pin is never overridden by this flag — an
    /// unavailable pin is an error (principle #46).
    pub fn allow_fallback(mut self, allow: bool) -> Self {
        self.allow_fallback = allow;
        self
    }
    /// Enumerates all available adapters with wgpu
    ///
    /// Uses [`instance_backends`](crate::gpu::backend_ladder::instance_backends)
    /// rather than `Backends::all()` so enumeration covers exactly the backend
    /// tiers the library knows how to use — every rung of the degradation ladder,
    /// plus any explicit `WGPU_BACKEND` pin. This keeps discovery and selection
    /// consistent: an adapter that enumeration reports can always be selected.
    #[cfg(feature = "gpu-wgpu")]
    pub async fn enumerate_adapters(&self) -> Vec<AdapterInfo> {
        let backends = crate::gpu::backend_ladder::instance_backends();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });
        let adapters = instance.enumerate_adapters(backends).await;
        adapters
            .into_iter()
            .map(|adapter| {
                let info = adapter.get_info();
                AdapterInfo::from_wgpu(&info)
            })
            .collect()
    }
    /// Selects the best adapter based on strategy with fallback chain.
    ///
    /// # One selection path (G-211)
    ///
    /// Selection delegates to the degradation ladder in
    /// [`crate::gpu::backend_ladder`] instead of re-implementing it with
    /// `request_adapter` calls. Both the GPU manager (this function) and the wgpu
    /// renderer
    /// ([`select_adapter_with_gl_fallback`](crate::gpu::backend_ladder::select_adapter_with_gl_fallback))
    /// therefore apply the same tier order, the same `WGPU_BACKEND` pin handling
    /// and the same adapter ranking, so the two cannot reach different answers.
    ///
    /// The selected backend tier is published through
    /// [`publish_shared_selection`](crate::gpu::backend_ladder::publish_shared_selection)
    /// and is also derivable from the returned [`AdapterInfo`] via
    /// [`AdapterInfo::backend_tier`], so a degraded start is observable rather
    /// than silent.
    ///
    /// # Caching (G-62)
    ///
    /// A surface-independent result is memoised and returned without creating a
    /// new `Instance`; the cache key covers the strategy, the fallback policy and
    /// the `WGPU_BACKEND` pin. A surface-bound call is never cached, because
    /// adapter suitability depends on the particular surface.
    #[cfg(feature = "gpu-wgpu")]
    pub async fn select_adapter_with_fallback(
        &self,
        compatible_surface: Option<&wgpu::Surface<'static>>,
    ) -> Result<AdapterInfo, AdapterSelectionError> {
        let key = SelectionCacheKey {
            strategy: self.strategy,
            allow_fallback: self.allow_fallback,
            pinned: crate::gpu::backend_ladder::backends_from_env(),
        };

        if compatible_surface.is_none() {
            let cached = lock(selection_cache()).clone();
            if let Some(cached) = cached {
                if cached.key == key {
                    return cached.result;
                }
            }
        }

        let result = self.select_adapter_probing(key.pinned, compatible_surface).await;

        if compatible_surface.is_none() {
            *lock(selection_cache()) = Some(CachedSelection { key, result: result.clone() });
        }
        result
    }

    /// Performs the actual probe without consulting or updating the cache.
    #[cfg(feature = "gpu-wgpu")]
    async fn select_adapter_probing(
        &self,
        pinned: Option<wgpu::Backends>,
        compatible_surface: Option<&wgpu::Surface<'static>>,
    ) -> Result<AdapterInfo, AdapterSelectionError> {
        use crate::gpu::backend_ladder::{instance_backends, select_adapter_with_gl_fallback};

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: instance_backends(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });

        match self.strategy {
            AdapterSelectionStrategy::Auto | AdapterSelectionStrategy::PreferPerformance => {
                match select_adapter_with_gl_fallback(
                    &instance,
                    wgpu::PowerPreference::HighPerformance,
                    compatible_surface,
                )
                .await
                {
                    Some((adapter, tier)) => self.accept_ladder_result(adapter, tier),
                    None => self.no_adapter(pinned),
                }
            }
            AdapterSelectionStrategy::PreferPowerEfficiency => {
                match select_adapter_with_gl_fallback(
                    &instance,
                    wgpu::PowerPreference::LowPower,
                    compatible_surface,
                )
                .await
                {
                    Some((adapter, tier)) => self.accept_ladder_result(adapter, tier),
                    None => self.no_adapter(pinned),
                }
            }
            AdapterSelectionStrategy::ForceDiscrete => {
                match self
                    .find_device_type(
                        &instance,
                        wgpu::DeviceType::DiscreteGpu,
                        pinned,
                        compatible_surface,
                    )
                    .await
                {
                    Some(adapter) => Ok(publish_adapter_with_tier(&adapter)),
                    None => Err(match pinned {
                        Some(pinned) => AdapterSelectionError::RequestFailed(format!(
                            "WGPU_BACKEND pinned {pinned:?} but no discrete GPU exposes that backend"
                        )),
                        None => AdapterSelectionError::DiscreteGpuNotFound,
                    }),
                }
            }
            AdapterSelectionStrategy::ForceIntegrated => {
                match self
                    .find_device_type(
                        &instance,
                        wgpu::DeviceType::IntegratedGpu,
                        pinned,
                        compatible_surface,
                    )
                    .await
                {
                    Some(adapter) => Ok(publish_adapter_with_tier(&adapter)),
                    None => Err(match pinned {
                        Some(pinned) => AdapterSelectionError::RequestFailed(format!(
                            "WGPU_BACKEND pinned {pinned:?} but no integrated GPU exposes that backend"
                        )),
                        None => AdapterSelectionError::IntegratedGpuNotFound,
                    }),
                }
            }
            AdapterSelectionStrategy::ForceCpu => {
                let adapter = instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface,
                        force_fallback_adapter: true,
                        apply_limit_buckets: false,
                    })
                    .await
                    .map_err(|e| AdapterSelectionError::RequestFailed(e.to_string()))?;
                // Forced software is the terminal rung regardless of the API that
                // exposes the CPU adapter (lavapipe reports Vulkan).
                Ok(publish_adapter(&adapter, crate::gpu::backend_ladder::GpuBackendTier::Software))
            }
        }
    }

    /// Accepts a ladder result unless it is the software rung and the caller
    /// forbade falling back to it.
    ///
    /// The ladder always attempts the CPU rung (principle #45), so this is where
    /// `allow_fallback(false)` refuses its result: a refused software adapter is
    /// turned into the same "no adapter" answer as a missing one, and is not
    /// published as the shared selection.
    #[cfg(feature = "gpu-wgpu")]
    fn accept_ladder_result(
        &self,
        adapter: wgpu::Adapter,
        tier: crate::gpu::backend_ladder::GpuBackendTier,
    ) -> Result<AdapterInfo, AdapterSelectionError> {
        if !self.allow_fallback && tier == crate::gpu::backend_ladder::GpuBackendTier::Software {
            log::warn!(
                "[gpu] only the CPU software adapter is available, but fallback is disabled"
            );
            return Err(AdapterSelectionError::NoAdapterFound);
        }
        Ok(publish_adapter(&adapter, tier))
    }

    /// Finds the first adapter of `wanted` type that can present to the surface.
    ///
    /// Honours the `WGPU_BACKEND` pin by enumerating only the pinned backends,
    /// falling back to the whole ladder when nothing is pinned (principle #46).
    #[cfg(feature = "gpu-wgpu")]
    async fn find_device_type(
        &self,
        instance: &wgpu::Instance,
        wanted: wgpu::DeviceType,
        pinned: Option<wgpu::Backends>,
        compatible_surface: Option<&wgpu::Surface<'static>>,
    ) -> Option<wgpu::Adapter> {
        let backends = pinned.unwrap_or_else(crate::gpu::backend_ladder::instance_backends);
        instance.enumerate_adapters(backends).await.into_iter().find(|adapter| {
            adapter.get_info().device_type == wanted
                && compatible_surface.is_none_or(|surface| adapter.is_surface_supported(surface))
        })
    }

    /// Builds the "no adapter" answer for a strategy that walks the ladder.
    ///
    /// A `WGPU_BACKEND` pin is never silently replaced: if the pinned backend is
    /// unavailable the caller gets an error naming the pin (principle #46).
    /// Without a pin, `allow_fallback` decides between the honest CPU software
    /// adapter and `NoAdapterFound`.
    #[cfg(feature = "gpu-wgpu")]
    fn no_adapter(
        &self,
        pinned: Option<wgpu::Backends>,
    ) -> Result<AdapterInfo, AdapterSelectionError> {
        if let Some(pinned) = pinned {
            log::error!(
                "[gpu] WGPU_BACKEND pinned {pinned:?} but this host provides no adapter for it"
            );
            return Err(AdapterSelectionError::RequestFailed(format!(
                "WGPU_BACKEND pinned {pinned:?} but this host provides no adapter for that backend"
            )));
        }
        if self.allow_fallback {
            log::warn!("[gpu] no adapter on any backend tier; using the CPU software fallback");
            crate::gpu::backend_ladder::publish_shared_selection(
                crate::gpu::backend_ladder::GpuBackendSelection {
                    tier: crate::gpu::backend_ladder::GpuBackendTier::Software,
                    backend: wgpu::Backend::Noop,
                },
            );
            Ok(AdapterInfo::cpu_fallback())
        } else {
            Err(AdapterSelectionError::NoAdapterFound)
        }
    }
    /// Returns the current selection strategy
    pub fn strategy(&self) -> AdapterSelectionStrategy {
        self.strategy
    }
    /// Sets the selection strategy
    ///
    /// The selection cache is keyed by strategy (G-62), so a change makes the
    /// next call probe again; there is no stale entry to clear.
    pub fn set_strategy(&mut self, strategy: AdapterSelectionStrategy) {
        self.strategy = strategy;
    }
}

/// Converts a probed adapter into [`AdapterInfo`] and publishes the selection.
#[cfg(feature = "gpu-wgpu")]
fn publish_adapter(
    adapter: &wgpu::Adapter,
    tier: crate::gpu::backend_ladder::GpuBackendTier,
) -> AdapterInfo {
    let info = adapter.get_info();
    let adapter_info = AdapterInfo::from_wgpu(&info);
    crate::gpu::backend_ladder::publish_shared_selection(
        crate::gpu::backend_ladder::GpuBackendSelection { tier, backend: info.backend },
    );
    adapter_info
}

/// Converts a probed adapter into [`AdapterInfo`], deriving and publishing its tier.
#[cfg(feature = "gpu-wgpu")]
fn publish_adapter_with_tier(adapter: &wgpu::Adapter) -> AdapterInfo {
    let info = adapter.get_info();
    let adapter_info = AdapterInfo::from_wgpu(&info);
    crate::gpu::backend_ladder::publish_shared_selection(
        crate::gpu::backend_ladder::GpuBackendSelection {
            tier: adapter_info.backend_tier(),
            backend: info.backend,
        },
    );
    adapter_info
}
crate::impl_default_via_new!(AdapterSelector);
/// Errors that can occur during adapter selection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterSelectionError {
    /// No GPU adapter found
    NoAdapterFound,
    /// Discrete GPU not found but was required
    DiscreteGpuNotFound,
    /// Integrated GPU not found but was required
    IntegratedGpuNotFound,
    /// Adapter request failed
    RequestFailed(String),
}
impl fmt::Display for AdapterSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapterFound => write!(f, "No GPU adapter found"),
            Self::DiscreteGpuNotFound => write!(f, "Discrete GPU not found"),
            Self::IntegratedGpuNotFound => write!(f, "Integrated GPU not found"),
            Self::RequestFailed(msg) => write!(f, "Adapter request failed: {msg}"),
        }
    }
}
#[cfg(not(alloc_frugal))]
impl core::error::Error for AdapterSelectionError {}
/// Detects if running in a browser environment with forced integrated GPU.
///
/// This keys off the compilation *target architecture*, not the operating system:
/// wasm32 code always runs inside a browser, where discrete adapters are not
/// reachable. That is a property of the architecture, so principle #36 (which
/// bans OS branching outside `src/platform/`) does not apply.
#[cfg(target_arch = "wasm32")]
pub fn detect_browser_forced_integrated_gpu() -> bool {
    // In WASM/browser, we often can't access discrete GPU due to browser restrictions
    // This is a heuristic detection
    true // Assume forced integrated in browser
}
/// Always returns `false` on non-wasm32 targets: no browser is involved, so
/// there is nothing forcing an integrated adapter.
///
/// See the `wasm32` definition above for why the check keys off the target
/// architecture rather than the operating system.
#[cfg(not(target_arch = "wasm32"))]
pub fn detect_browser_forced_integrated_gpu() -> bool {
    false // Not in browser
}
#[cfg(test)]
// A later impl block follows the tests (layout helpers kept next to their users); the
// lint is suppressed here for that file-shape reason, as in demo/control/src/app.rs.
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    #[test]
    fn test_gpu_device_type_priority() {
        assert!(GpuDeviceType::DiscreteGpu > GpuDeviceType::IntegratedGpu);
        assert!(GpuDeviceType::IntegratedGpu > GpuDeviceType::Cpu);
        assert!(GpuDeviceType::DiscreteGpu > GpuDeviceType::Cpu);
    }
    #[test]
    fn test_gpu_device_type_checks() {
        assert!(GpuDeviceType::DiscreteGpu.is_discrete());
        assert!(GpuDeviceType::IntegratedGpu.is_integrated());
        assert!(GpuDeviceType::Cpu.is_cpu());
        assert!(!GpuDeviceType::Cpu.is_hardware_gpu());
        assert!(GpuDeviceType::DiscreteGpu.is_hardware_gpu());
    }
    #[test]
    fn test_performance_tier() {
        assert_eq!(GpuDeviceType::DiscreteGpu.performance_tier(), 5);
        assert_eq!(GpuDeviceType::IntegratedGpu.performance_tier(), 3);
        assert_eq!(GpuDeviceType::Cpu.performance_tier(), 1);
    }
    #[test]
    fn test_adapter_info_cpu_fallback() {
        let info = AdapterInfo::cpu_fallback();
        assert!(info.device_type.is_cpu());
        assert!(info.is_selected);
        assert!(!info.supports_high_quality());
    }
    #[test]
    fn test_adapter_selection_error_display() {
        let err = AdapterSelectionError::NoAdapterFound;
        assert_eq!(err.to_string(), "No GPU adapter found");
    }
    #[test]
    fn test_gpu_type_from_device_type() {
        assert!(matches!(GpuType::from(GpuDeviceType::DiscreteGpu), GpuType::Discrete));
        assert!(matches!(GpuType::from(GpuDeviceType::IntegratedGpu), GpuType::Integrated));
        assert!(matches!(GpuType::from(GpuDeviceType::Cpu), GpuType::Cpu));
    }
    #[test]
    fn test_gpu_type_description() {
        assert_eq!(GpuType::Discrete.description(), "Discrete GPU");
        assert_eq!(GpuType::Integrated.description(), "Integrated GPU");
        assert_eq!(GpuType::Cpu.description(), "CPU Software Rendering");
    }

    /// G-230: detection must route through the shared selection path, not
    /// build a private `wgpu::Instance` per call. Routing through
    /// `select_adapter_with_fallback` is observable in the process-wide
    /// selection cache (G-62): after one surface-independent detection the
    /// memoised entry exists and carries detection's own key, and a second call
    /// answers identically from it.
    #[cfg(feature = "gpu-wgpu")]
    #[test]
    fn detect_primary_routes_through_the_selection_cache() {
        let first = GpuType::detect_primary();
        let cached = lock(selection_cache()).clone();
        let cached = cached.expect("surface-independent detection must populate the cache");
        assert_eq!(
            cached.key.strategy,
            AdapterSelectionStrategy::PreferPerformance,
            "detection probes with the HighPerformance preference it always used"
        );
        assert!(cached.key.allow_fallback, "detection keeps the CPU fallback answer");
        assert_eq!(
            first,
            cached.result.ok().map(|info| GpuType::from(info.device_type)),
            "the answer must agree with the memoised probe"
        );
        // The second call reads the memoised result, so it must agree without
        // constructing another `Instance`.
        assert_eq!(GpuType::detect_primary(), first, "repeat detection is stable");
    }

    /// N-S-58: `Ord` must be a total order consistent with `Eq` — `cmp` returns
    /// `Equal` for two variants *iff* they are the same variant. The old impl
    /// compared only `priority()`, so `VirtualGpu` and `Other` (both priority 2)
    /// compared `Equal` despite being distinct.
    #[test]
    fn gpu_device_type_ord_is_consistent_with_eq() {
        use core::cmp::Ordering;

        const ALL: [GpuDeviceType; 5] = [
            GpuDeviceType::DiscreteGpu,
            GpuDeviceType::IntegratedGpu,
            GpuDeviceType::VirtualGpu,
            GpuDeviceType::Other,
            GpuDeviceType::Cpu,
        ];

        for &a in &ALL {
            for &b in &ALL {
                let eq = a == b;
                let cmp_equal = a.cmp(&b) == Ordering::Equal;
                assert_eq!(
                    cmp_equal, eq,
                    "cmp({a:?}, {b:?}) == Equal is {cmp_equal} but == is {eq}"
                );
                // Antisymmetry: cmp(a,b) == cmp(b,a).reverse().
                assert_eq!(
                    a.cmp(&b),
                    b.cmp(&a).reverse(),
                    "cmp must be antisymmetric ({a:?},{b:?})"
                );
            }
        }

        // The coarse priority ordering is preserved for the distinct-priority
        // pairs that matter in selection.
        assert!(GpuDeviceType::DiscreteGpu > GpuDeviceType::IntegratedGpu);
        assert!(GpuDeviceType::IntegratedGpu > GpuDeviceType::VirtualGpu);
        assert!(GpuDeviceType::VirtualGpu > GpuDeviceType::Cpu);
    }

    /// `backend_tier` must classify every backend string `from_wgpu` can produce,
    /// including the synthetic `"CPU"` tag, so a caller never gets a stale answer
    /// after a fallback.
    #[cfg(feature = "gpu")]
    #[test]
    fn adapter_info_backend_tier_covers_every_backend_label() {
        use crate::gpu::backend_ladder::GpuBackendTier;

        let with_backend = |backend: &str| AdapterInfo {
            device_type: GpuDeviceType::IntegratedGpu,
            vendor: "0000".to_string(),
            name: "test".to_string(),
            backend: backend.to_string(),
            driver: String::new(),
            driver_version: 0,
            is_selected: true,
        };

        // The labels `format!("{:?}", wgpu::Backend)` emits for each tier.
        for primary in ["Vulkan", "Metal", "Dx12", "BrowserWebGpu"] {
            assert_eq!(
                with_backend(primary).backend_tier(),
                GpuBackendTier::Primary,
                "{primary} must be the primary tier"
            );
            assert!(!with_backend(primary).is_degraded_backend());
        }

        assert_eq!(with_backend("Gl").backend_tier(), GpuBackendTier::OpenGlEs);
        assert!(with_backend("Gl").is_degraded_backend());

        // The CPU fallback sets `backend = "CPU"`, and `Noop` is the wgpu label
        // for a no-op backend — both sit on the terminal software rung.
        assert_eq!(AdapterInfo::cpu_fallback().backend_tier(), GpuBackendTier::Software);
        assert!(AdapterInfo::cpu_fallback().is_degraded_backend());
        assert_eq!(with_backend("Noop").backend_tier(), GpuBackendTier::Software);

        // A CPU adapter must report the software rung whichever backend name it
        // carries: lavapipe exposes the software rasterizer through Vulkan, and
        // llvmpipe through GL. Mapping on the backend string alone would call
        // those "Primary"/"OpenGlEs" and hide the degradation (G-211).
        for backend in ["Vulkan", "Metal", "Dx12", "BrowserWebGpu", "Gl", "Noop", "CPU"] {
            let info = AdapterInfo { device_type: GpuDeviceType::Cpu, ..with_backend(backend) };
            assert_eq!(
                info.backend_tier(),
                GpuBackendTier::Software,
                "a CPU adapter on {backend} is still the software rung"
            );
            assert!(info.is_degraded_backend());
        }
    }
}
/// GPU adapter with detection capabilities
pub struct GpuAdapter;
impl GpuAdapter {
    /// Detects the primary GPU type
    pub fn detect_primary_gpu_type() -> Option<GpuType> {
        GpuType::detect_primary()
    }
    /// Reports whether this build can measure the GPU's memory at all.
    ///
    /// There is deliberately no `detect_gpu_memory_mb()` beside this returning a
    /// number: this crate has no portable way to query VRAM. wgpu exposes
    /// `AdapterInfo` but no memory size (its `device` limits describe per-allocation
    /// caps, not total VRAM), so any figure produced here would be invented.
    ///
    /// The previous pair of methods did exactly that — `detect_gpu_memory_mb() -> 512`
    /// and `detect_battery_status() -> false`, each with a comment saying production
    /// would query the real source — and neither had a single caller. A fabricated
    /// measurement is worse than a missing one (principle #12/#37), so the two were
    /// deleted rather than left as documentation of work not done.
    ///
    /// Callers that need a *real* battery fact use
    /// [`crate::platform::Platform::is_on_battery`] via
    /// [`crate::platform::platform_facts`], which every backend implements from its
    /// own OS API.
    pub fn can_detect_gpu_memory() -> bool {
        // `true` only where the platform reports a real total, so an adaptive
        // caller can decide between measuring and assuming instead of guessing.
        crate::platform::platform_facts().total_memory_mb().is_some()
    }
}
