use crate::core::Size;
#[derive(Debug, Clone)]
pub struct EmbeddedConfig {
    pub screen_size: Size,
    pub fixed_dpi: Option<u32>,
    pub low_memory_mode: bool,
    pub max_widgets: usize,
    pub max_texture_size: u32,
    pub enable_animations: bool,
    pub enable_shadows: bool,
    pub enable_gradients: bool,
    pub font_scale: f32,
    pub touch_enabled: bool,
    pub hardware_acceleration: bool,
}
impl EmbeddedConfig {
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
    pub fn with_fixed_dpi(mut self, dpi: u32) -> Self {
        self.fixed_dpi = Some(dpi);
        self
    }
    pub fn low_memory(mut self) -> Self {
        self.low_memory_mode = true;
        self.max_widgets = 50;
        self.max_texture_size = 512;
        self.enable_animations = false;
        self.enable_shadows = false;
        self.enable_gradients = false;
        self
    }
    pub fn with_max_widgets(mut self, count: usize) -> Self {
        self.max_widgets = count;
        self
    }
    pub fn with_touch(mut self, enabled: bool) -> Self {
        self.touch_enabled = enabled;
        self
    }
    pub fn with_hardware_acceleration(mut self, enabled: bool) -> Self {
        self.hardware_acceleration = enabled;
        self
    }
    pub fn with_font_scale(mut self, scale: f32) -> Self {
        self.font_scale = scale.clamp(0.5, 3.0);
        self
    }
}
impl Default for EmbeddedConfig {
    fn default() -> Self {
        Self::new(Size::new(800, 600))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceConstraint {
    None,
    Low,
    Medium,
    High,
}
impl Default for ResourceConstraint {
    fn default() -> Self {
        Self::None
    }
}
pub struct ResourceManager {
    // constraint: ResourceConstraint,
    allocated_memory: usize,
    max_memory: usize,
    widget_count: usize,
    max_widgets: usize,
}
impl ResourceManager {
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
    pub fn can_allocate(&self, size: usize) -> bool {
        self.allocated_memory + size <= self.max_memory
    }
    pub fn allocate(&mut self, size: usize) -> bool {
        if self.can_allocate(size) {
            self.allocated_memory += size;
            true
        } else {
            false
        }
    }
    pub fn deallocate(&mut self, size: usize) {
        self.allocated_memory = self.allocated_memory.saturating_sub(size);
    }
    pub fn can_create_widget(&self) -> bool {
        self.widget_count < self.max_widgets
    }
    pub fn register_widget(&mut self) -> bool {
        if self.can_create_widget() {
            self.widget_count += 1;
            true
        } else {
            false
        }
    }
    pub fn unregister_widget(&mut self) {
        self.widget_count = self.widget_count.saturating_sub(1);
    }
    pub fn memory_usage(&self) -> usize {
        self.allocated_memory
    }
    pub fn memory_percentage(&self) -> f32 {
        if self.max_memory == usize::MAX {
            0.0
        } else {
            (self.allocated_memory as f32 / self.max_memory as f32) * 100.0
        }
    }
    pub fn widget_count(&self) -> usize {
        self.widget_count
    }
    pub fn is_under_pressure(&self) -> bool {
        self.memory_percentage() > 80.0
            || (self.widget_count as f32 / self.max_widgets as f32) > 0.9
    }
}
impl Default for ResourceManager {
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
