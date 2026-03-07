//! GPU adapter detection and selection with hardware auto-detection.
//!
//! This module provides intelligent GPU adapter selection with the following priority:
//! 1. Discrete GPU (独显) - Highest performance
//! 2. Integrated GPU (集显) - Balanced performance
//! 3. CPU Software Rendering (CPU) - Fallback mode
//!
//! # Example
//! ```
//! use rust_widgets::gpu::adapter::{AdapterSelector, AdapterSelectionStrategy};
//!
//! let selector = AdapterSelector::new();
//! let adapter_info = selector.select_adapter_with_fallback(None).unwrap();
//! println!("Selected: {:?}", adapter_info.device_type);
//! ```

use std::fmt;

/// GPU type for simplified hardware detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuType {
    /// Discrete GPU (独立显卡)
    Discrete,
    /// Integrated GPU (集成显卡)
    Integrated,
    /// CPU software rendering
    Cpu,
}

impl GpuType {
    /// Detects the primary GPU type from system
    pub fn detect_primary() -> Option<Self> {
        // In a real implementation, this would query system GPU info
        // For now, return None to trigger auto-detection
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
    /// Discrete GPU (独立显卡) - Highest priority
    DiscreteGpu,
    /// Integrated GPU (集成显卡) - Medium priority
    IntegratedGpu,
    /// Virtual GPU (虚拟显卡) - Low priority
    VirtualGpu,
    /// Other GPU types
    Other,
    /// CPU Software Rendering (CPU软件渲染) - Lowest priority, fallback
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
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.priority().partial_cmp(&other.priority())
    }
}

impl Ord for GpuDeviceType {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
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

    /// Returns a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            GpuDeviceType::DiscreteGpu => "Discrete GPU (独立显卡)",
            GpuDeviceType::IntegratedGpu => "Integrated GPU (集成显卡)",
            GpuDeviceType::VirtualGpu => "Virtual GPU (虚拟显卡)",
            GpuDeviceType::Other => "Other GPU",
            GpuDeviceType::Cpu => "CPU Software Rendering (CPU软件渲染)",
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

/// Information about a GPU adapter
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Auto,
}

impl Default for AdapterSelectionStrategy {
    fn default() -> Self {
        Self::Auto
    }
}

/// Adapter selector with hardware auto-detection
pub struct AdapterSelector {
    strategy: AdapterSelectionStrategy,
    allow_fallback: bool,
}

impl AdapterSelector {
    /// Creates a new adapter selector with default strategy
    pub fn new() -> Self {
        Self {
            strategy: AdapterSelectionStrategy::Auto,
            allow_fallback: true,
        }
    }

    /// Creates a new adapter selector with specific strategy
    pub fn with_strategy(strategy: AdapterSelectionStrategy) -> Self {
        Self {
            strategy,
            allow_fallback: true,
        }
    }

    /// Sets whether to allow fallback to lower priority adapters
    pub fn allow_fallback(mut self, allow: bool) -> Self {
        self.allow_fallback = allow;
        self
    }

    /// Enumerates all available adapters with wgpu
    #[cfg(feature = "gpu-wgpu")]
    pub async fn enumerate_adapters(&self) -> Vec<AdapterInfo> {
        let instance = wgpu::Instance::default();
        let adapters = instance.enumerate_adapters(wgpu::Backends::all());

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
        compatible_surface: Option<&wgpu::Surface>,
    ) -> Result<AdapterInfo, AdapterSelectionError> {
        let instance = wgpu::Instance::default();

        // Try to get adapter based on strategy
        let adapter = match self.strategy {
            AdapterSelectionStrategy::PreferPerformance | AdapterSelectionStrategy::Auto => {
                // Try discrete GPU first
                if let Some(adapter) = instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        compatible_surface,
                        force_fallback_adapter: false,
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
                    instance
                        .request_adapter(&wgpu::RequestAdapterOptions {
                            power_preference: wgpu::PowerPreference::LowPower,
                            compatible_surface,
                            force_fallback_adapter: false,
                        })
                        .await
                } else {
                    None
                }
            }
            AdapterSelectionStrategy::PreferPowerEfficiency => {
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface,
                        force_fallback_adapter: false,
                    })
                    .await
            }
            AdapterSelectionStrategy::ForceDiscrete => {
                let adapter = instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        compatible_surface,
                        force_fallback_adapter: false,
                    })
                    .await;

                if let Some(ref a) = adapter {
                    let info = a.get_info();
                    if info.device_type != wgpu::DeviceType::DiscreteGpu {
                        return Err(AdapterSelectionError::DiscreteGpuNotFound);
                    }
                }
                adapter
            }
            AdapterSelectionStrategy::ForceIntegrated => {
                let adapter = instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface,
                        force_fallback_adapter: false,
                    })
                    .await;

                if let Some(ref a) = adapter {
                    let info = a.get_info();
                    if info.device_type != wgpu::DeviceType::IntegratedGpu {
                        return Err(AdapterSelectionError::IntegratedGpuNotFound);
                    }
                }
                adapter
            }
            AdapterSelectionStrategy::ForceCpu => {
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface,
                        force_fallback_adapter: true,
                    })
                    .await
            }
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

impl Default for AdapterSelector {
    fn default() -> Self {
        Self::new()
    }
}

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
            Self::RequestFailed(msg) => write!(f, "Adapter request failed: {}", msg),
        }
    }
}

impl std::error::Error for AdapterSelectionError {}

/// Detects if running in a browser environment with forced integrated GPU
#[cfg(target_arch = "wasm32")]
pub fn detect_browser_forced_integrated_gpu() -> bool {
    // In WASM/browser, we often can't access discrete GPU due to browser restrictions
    // This is a heuristic detection
    true // Assume forced integrated in browser
}

#[cfg(not(target_arch = "wasm32"))]
pub fn detect_browser_forced_integrated_gpu() -> bool {
    false // Not in browser
}

/// Detects Windows browser environment that forces integrated GPU
#[cfg(target_os = "windows")]
pub fn detect_windows_browser_forced_igpu() -> Option<String> {
    use std::env;
    
    // Check if we're in a browser environment on Windows
    // Common browser executables that force iGPU
    let browser_processes = ["chrome.exe", "firefox.exe", "msedge.exe", "opera.exe"];
    
    if let Ok(parent) = env::var("RW_PARENT_PROCESS") {
        for browser in &browser_processes {
            if parent.to_lowercase().contains(browser) {
                return Some(format!("Detected browser: {}", browser));
            }
        }
    }
    
    // Check for Electron apps
    if let Ok(electron) = env::var("RW_ELECTRON_APP") {
        if !electron.is_empty() {
            return Some(format!("Detected Electron app: {}", electron));
        }
    }
    
    None
}

#[cfg(not(target_os = "windows"))]
pub fn detect_windows_browser_forced_igpu() -> Option<String> {
    None
}

#[cfg(test)]
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
        assert!(matches!(
            GpuType::from(GpuDeviceType::DiscreteGpu),
            GpuType::Discrete
        ));
        assert!(matches!(
            GpuType::from(GpuDeviceType::IntegratedGpu),
            GpuType::Integrated
        ));
        assert!(matches!(GpuType::from(GpuDeviceType::Cpu), GpuType::Cpu));
    }

    #[test]
    fn test_gpu_type_description() {
        assert_eq!(GpuType::Discrete.description(), "Discrete GPU");
        assert_eq!(GpuType::Integrated.description(), "Integrated GPU");
        assert_eq!(GpuType::Cpu.description(), "CPU Software Rendering");
    }
}

/// GPU adapter with detection capabilities
pub struct GpuAdapter;

impl GpuAdapter {
    /// Detects the primary GPU type
    pub fn detect_primary_gpu_type() -> Option<GpuType> {
        GpuType::detect_primary()
    }

    /// Detects GPU memory in MB (estimated)
    pub fn detect_gpu_memory_mb() -> u32 {
        // Simplified detection - in production would query GPU driver
        512
    }

    /// Checks if running on battery power
    pub fn detect_battery_status() -> bool {
        // Simplified - in production would query power management
        false
    }
}
