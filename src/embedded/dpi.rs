use std::sync::atomic::{AtomicU32, Ordering};
static FIXED_DPI: AtomicU32 = AtomicU32::new(0);
static BASE_DPI: u32 = 96;
/// Set fixed DPI for embedded systems
pub fn set_fixed_dpi(dpi: u32) {
    FIXED_DPI.store(dpi, Ordering::Relaxed);
}
/// Get fixed DPI if set
pub fn get_fixed_dpi() -> Option<u32> {
    let dpi = FIXED_DPI.load(Ordering::Relaxed);
    if dpi > 0 {
        Some(dpi)
    } else {
        None
    }
}
/// Clear fixed DPI setting
pub fn clear_fixed_dpi() {
    FIXED_DPI.store(0, Ordering::Relaxed);
}
/// Check if fixed DPI mode is enabled
pub fn is_fixed_dpi() -> bool {
    FIXED_DPI.load(Ordering::Relaxed) > 0
}
/// Calculate scale factor based on DPI
pub fn scale_factor() -> f32 {
    get_fixed_dpi()
        .map(|dpi| dpi as f32 / BASE_DPI as f32)
        .unwrap_or(1.0)
}
/// Scale a value by the current DPI factor
pub fn scale(value: i32) -> i32 {
    (value as f32 * scale_factor()) as i32
}
/// Scale a value by the current DPI factor (unsigned)
pub fn scale_u32(value: u32) -> u32 {
    (value as f32 * scale_factor()) as u32
}
/// Scale a value by the current DPI factor (float)
pub fn scale_f32(value: f32) -> f32 {
    value * scale_factor()
}
/// Convert pixels to points (1/72 inch)
pub fn pixels_to_points(pixels: f32, dpi: u32) -> f32 {
    pixels * 72.0 / dpi as f32
}
/// Convert points to pixels
pub fn points_to_pixels(points: f32, dpi: u32) -> f32 {
    points * dpi as f32 / 72.0
}
/// DPI-aware size calculator
#[derive(Debug, Clone, Copy)]
pub struct DpiScaler {
    dpi: u32,
    base_dpi: u32,
}
impl DpiScaler {
    pub fn new(dpi: u32) -> Self {
        Self {
            dpi,
            base_dpi: BASE_DPI,
        }
    }
    pub fn with_base_dpi(mut self, base: u32) -> Self {
        self.base_dpi = base;
        self
    }
    pub fn scale_factor(&self) -> f32 {
        self.dpi as f32 / self.base_dpi as f32
    }
    pub fn scale(&self, value: i32) -> i32 {
        (value as f32 * self.scale_factor()) as i32
    }
    pub fn scale_u32(&self, value: u32) -> u32 {
        (value as f32 * self.scale_factor()) as u32
    }
    pub fn scale_f32(&self, value: f32) -> f32 {
        value * self.scale_factor()
    }
    pub fn unscale(&self, value: i32) -> i32 {
        (value as f32 / self.scale_factor()) as i32
    }
    pub fn unscale_u32(&self, value: u32) -> u32 {
        (value as f32 / self.scale_factor()) as u32
    }
    pub fn unscale_f32(&self, value: f32) -> f32 {
        value / self.scale_factor()
    }
}
impl Default for DpiScaler {
    fn default() -> Self {
        Self::new(BASE_DPI)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_fixed_dpi() {
        set_fixed_dpi(144);
        assert!(is_fixed_dpi());
        assert_eq!(get_fixed_dpi(), Some(144));
        assert_eq!(scale_factor(), 1.5);
        clear_fixed_dpi();
        assert!(!is_fixed_dpi());
        assert_eq!(get_fixed_dpi(), None);
    }
    #[test]
    fn test_scale_functions() {
        set_fixed_dpi(192);
        assert_eq!(scale(100), 200);
        assert_eq!(scale_u32(100), 200);
        assert!((scale_f32(100.0) - 200.0).abs() < 0.01);
        clear_fixed_dpi();
    }
    #[test]
    fn test_dpi_scaler() {
        let scaler = DpiScaler::new(144);
        assert!((scaler.scale_factor() - 1.5).abs() < 0.01);
        assert_eq!(scaler.scale(100), 150);
        assert_eq!(scaler.unscale(150), 100);
    }
    #[test]
    fn test_points_conversion() {
        let pixels = points_to_pixels(12.0, 96);
        assert!((pixels - 16.0).abs() < 0.01);
        let points = pixels_to_points(16.0, 96);
        assert!((points - 12.0).abs() < 0.01);
    }
}
