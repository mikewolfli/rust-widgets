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
    #[cfg(feature = "gpu-wgpu")]
    pub fn detect_primary() -> Option<Self> {
        // Walk the same degradation ladder the renderer uses, so detection and
        // rendering never disagree about which adapter is in play. Without this, a
        // host that only offers GL would report "no GPU" here while the renderer
        // happily ran on OpenGL ES.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: crate::gpu::backend_ladder::instance_backends(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });
        let adapter =
            match pollster::block_on(crate::gpu::backend_ladder::select_adapter_with_gl_fallback(
                &instance,
                wgpu::PowerPreference::HighPerformance,
                None,
            )) {
                Some((adapter, _tier)) => adapter,
                None => return None,
            };
        let info = adapter.get_info();
        Some(GpuType::from(GpuDeviceType::from(info.device_type)))
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
        self.priority().cmp(&other.priority())
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
    /// Gated on `gpu`: the tier vocabulary lives in the `backend_ladder` module,
    /// which is itself only compiled when `wgpu` is available.
    #[cfg(feature = "gpu")]
    pub fn backend_tier(&self) -> crate::gpu::backend_ladder::GpuBackendTier {
        use crate::gpu::backend_ladder::GpuBackendTier;
        match self.backend.as_str() {
            // Primary tier: the APIs `wgpu` supports first-class.
            "Vulkan" | "Metal" | "Dx12" | "BrowserWebGpu" => GpuBackendTier::Primary,
            // GL covers OpenGL ES (Linux/Android), WebGL (web) and desktop
            // OpenGL through ANGLE (Windows/macOS).
            "Gl" => GpuBackendTier::OpenGlEs,
            // Everything else — `Noop`, or the synthetic "CPU" tag set by
            // `cpu_fallback` — is the terminal software rung.
            _ => GpuBackendTier::Software,
        }
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
    /// Sets whether to allow fallback to lower priority adapters
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
    /// Selects the best adapter based on strategy with fallback chain
    #[cfg(feature = "gpu-wgpu")]
    pub async fn select_adapter_with_fallback(
        &self,
        compatible_surface: Option<&wgpu::Surface<'static>>,
    ) -> Result<AdapterInfo, AdapterSelectionError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: crate::gpu::backend_ladder::instance_backends(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });
        // Try to get adapter based on strategy
        let adapter = match self.strategy {
            AdapterSelectionStrategy::PreferPerformance | AdapterSelectionStrategy::Auto => {
                // Try discrete GPU first
                if let Ok(adapter) = instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        compatible_surface,
                        force_fallback_adapter: false,
                        apply_limit_buckets: false,
                    })
                    .await
                {
                    let info = adapter.get_info();
                    if info.device_type == wgpu::DeviceType::DiscreteGpu {
                        return Ok(AdapterInfo::from_wgpu(&info));
                    }
                    // If we got integrated but wanted discrete, continue to fallback
                    if self.allow_fallback {
                        return Ok(AdapterInfo::from_wgpu(&info));
                    }
                }
                // Fallback to any available adapter
                if self.allow_fallback {
                    Some(
                        match instance
                            .request_adapter(&wgpu::RequestAdapterOptions {
                                power_preference: wgpu::PowerPreference::LowPower,
                                compatible_surface,
                                force_fallback_adapter: false,
                                apply_limit_buckets: false,
                            })
                            .await
                        {
                            Ok(adapter) => adapter,
                            Err(e) => {
                                return Err(AdapterSelectionError::RequestFailed(e.to_string()))
                            }
                        },
                    )
                } else {
                    None
                }
            }
            AdapterSelectionStrategy::PreferPowerEfficiency => Some(
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface,
                        force_fallback_adapter: false,
                        apply_limit_buckets: false,
                    })
                    .await
                    .map_err(|e| AdapterSelectionError::RequestFailed(e.to_string()))?,
            ),
            AdapterSelectionStrategy::ForceDiscrete => {
                let a = instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        compatible_surface,
                        force_fallback_adapter: false,
                        apply_limit_buckets: false,
                    })
                    .await
                    .map_err(|e| AdapterSelectionError::RequestFailed(e.to_string()))?;
                if a.get_info().device_type != wgpu::DeviceType::DiscreteGpu {
                    return Err(AdapterSelectionError::DiscreteGpuNotFound);
                }
                Some(a)
            }
            AdapterSelectionStrategy::ForceIntegrated => {
                let a = instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface,
                        force_fallback_adapter: false,
                        apply_limit_buckets: false,
                    })
                    .await
                    .map_err(|e| AdapterSelectionError::RequestFailed(e.to_string()))?;
                if a.get_info().device_type != wgpu::DeviceType::IntegratedGpu {
                    return Err(AdapterSelectionError::IntegratedGpuNotFound);
                }
                Some(a)
            }
            AdapterSelectionStrategy::ForceCpu => Some(
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface,
                        force_fallback_adapter: true,
                        apply_limit_buckets: false,
                    })
                    .await
                    .map_err(|e| AdapterSelectionError::RequestFailed(e.to_string()))?,
            ),
        };
        match adapter {
            Some(adapter) => {
                let info = adapter.get_info();
                Ok(AdapterInfo::from_wgpu(&info))
            }
            None => {
                if self.allow_fallback {
                    // Return CPU fallback
                    Ok(AdapterInfo::cpu_fallback())
                } else {
                    Err(AdapterSelectionError::NoAdapterFound)
                }
            }
        }
    }
    /// Returns the current selection strategy
    pub fn strategy(&self) -> AdapterSelectionStrategy {
        self.strategy
    }
    /// Sets the selection strategy
    pub fn set_strategy(&mut self, strategy: AdapterSelectionStrategy) {
        self.strategy = strategy;
    }
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
