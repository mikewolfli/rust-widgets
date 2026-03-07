//! Embedded scenario tests

use rust_widgets::embedded::{
    EmbeddedConfig, LightweightConfig, ResourceConstraint,
    is_embedded_mode, is_low_memory_mode, set_embedded_mode, set_low_memory_mode,
    recommended_buffer_size, max_texture_size, font_cache_size, event_queue_size,
    init_embedded, init_desktop,
};
use rust_widgets::core::Size;

#[test]
fn test_embedded_config_default() {
    let config = EmbeddedConfig::default();
    
    assert_eq!(config.screen_size, Size::new(800, 600));
    assert_eq!(config.fixed_dpi, None);
    assert!(!config.low_memory_mode);
    assert_eq!(config.max_widgets, 100);
}

#[test]
fn test_embedded_config_new() {
    let config = EmbeddedConfig::new(Size::new(1024, 768));
    
    assert_eq!(config.screen_size, Size::new(1024, 768));
    assert_eq!(config.max_widgets, 100);
    assert!(config.enable_animations);
    assert!(!config.enable_shadows);
    assert!(config.enable_gradients);
}

#[test]
fn test_embedded_config_builder() {
    let config = EmbeddedConfig::new(Size::new(800, 600))
        .with_fixed_dpi(144)
        .low_memory()
        .with_max_widgets(50)
        .with_touch(true)
        .with_hardware_acceleration(true)
        .with_font_scale(1.5);
    
    assert_eq!(config.fixed_dpi, Some(144));
    assert!(config.low_memory_mode);
    assert_eq!(config.max_widgets, 50);
    assert!(config.touch_enabled);
    assert!(config.hardware_acceleration);
    assert!((config.font_scale - 1.5).abs() < 0.01);
}

#[test]
fn test_embedded_config_low_memory() {
    let config = EmbeddedConfig::new(Size::new(800, 600)).low_memory();
    
    assert!(config.low_memory_mode);
    assert_eq!(config.max_widgets, 50);
    assert_eq!(config.max_texture_size, 512);
    assert!(!config.enable_animations);
    assert!(!config.enable_shadows);
    assert!(!config.enable_gradients);
}

#[test]
fn test_lightweight_config_default() {
    let config = LightweightConfig::default();
    
    assert!(!config.disable_animations);
    assert!(!config.disable_shadows);
    assert!(!config.disable_gradients);
    assert!(!config.simple_borders);
    assert!(!config.reduced_padding);
    assert!(!config.minimal_signals);
}

#[test]
fn test_lightweight_config_minimal() {
    let config = LightweightConfig::minimal();
    
    assert!(config.disable_animations);
    assert!(config.disable_shadows);
    assert!(config.disable_gradients);
    assert!(config.simple_borders);
    assert!(config.reduced_padding);
    assert!(config.minimal_signals);
}

#[test]
fn test_lightweight_config_builder() {
    let config = LightweightConfig::new()
        .with_shadows_disabled()
        .with_animations_disabled()
        .with_gradients_disabled();
    
    assert!(config.disable_animations);
    assert!(config.disable_shadows);
    assert!(config.disable_gradients);
}

#[test]
fn test_embedded_mode_global() {
    set_embedded_mode(true);
    assert!(is_embedded_mode());
    
    set_embedded_mode(false);
    assert!(!is_embedded_mode());
}

#[test]
fn test_low_memory_mode_global() {
    set_low_memory_mode(true);
    assert!(is_low_memory_mode());
    
    set_low_memory_mode(false);
    assert!(!is_low_memory_mode());
}

#[test]
fn test_recommended_buffer_size() {
    set_low_memory_mode(true);
    let low_mem_size = recommended_buffer_size();
    assert_eq!(low_mem_size, Size::new(800, 600));
    
    set_low_memory_mode(false);
    let normal_size = recommended_buffer_size();
    assert_eq!(normal_size, Size::new(1920, 1080));
}

#[test]
fn test_max_texture_size() {
    set_embedded_mode(true);
    let embedded_max = max_texture_size();
    assert_eq!(embedded_max, 1024);
    
    set_embedded_mode(false);
    let desktop_max = max_texture_size();
    assert_eq!(desktop_max, 4096);
}

#[test]
fn test_font_cache_size() {
    set_low_memory_mode(true);
    let low_mem_cache = font_cache_size();
    assert_eq!(low_mem_cache, 256 * 1024);
    
    set_low_memory_mode(false);
    let normal_cache = font_cache_size();
    assert_eq!(normal_cache, 2 * 1024 * 1024);
}

#[test]
fn test_event_queue_size() {
    set_embedded_mode(true);
    let embedded_queue = event_queue_size();
    assert_eq!(embedded_queue, 64);
    
    set_embedded_mode(false);
    let desktop_queue = event_queue_size();
    assert_eq!(desktop_queue, 256);
}

#[test]
fn test_init_embedded() {
    let config = EmbeddedConfig::new(Size::new(800, 480))
        .with_fixed_dpi(120)
        .low_memory();
    
    init_embedded(config);
    
    assert!(is_embedded_mode());
    assert!(is_low_memory_mode());
    
    init_desktop();
    
    assert!(!is_embedded_mode());
    assert!(!is_low_memory_mode());
}

#[test]
fn test_init_desktop() {
    init_desktop();
    
    assert!(!is_embedded_mode());
    assert!(!is_low_memory_mode());
}

#[test]
fn test_resource_constraint_variants() {
    let none = ResourceConstraint::None;
    let low = ResourceConstraint::Low;
    let medium = ResourceConstraint::Medium;
    let high = ResourceConstraint::High;
    
    assert!(none != low);
    assert!(low != medium);
    assert!(medium != high);
}

#[test]
fn test_resource_constraint_default() {
    let constraint = ResourceConstraint::default();
    assert_eq!(constraint, ResourceConstraint::None);
}

#[test]
fn test_embedded_scenario_raspberry_pi() {
    let config = EmbeddedConfig::new(Size::new(800, 480))
        .with_fixed_dpi(96)
        .low_memory()
        .with_touch(true);
    
    assert!(config.low_memory_mode);
    assert!(config.touch_enabled);
    assert_eq!(config.max_widgets, 50);
    assert_eq!(config.max_texture_size, 512);
    assert!(!config.enable_animations);
    assert!(!config.enable_shadows);
    assert!(!config.enable_gradients);
}

#[test]
fn test_embedded_scenario_industrial_display() {
    let config = EmbeddedConfig::new(Size::new(1024, 768))
        .with_fixed_dpi(120)
        .with_touch(true)
        .with_hardware_acceleration(true);
    
    assert!(!config.low_memory_mode);
    assert_eq!(config.fixed_dpi, Some(120));
    assert!(config.touch_enabled);
    assert!(config.hardware_acceleration);
    assert!(config.enable_animations);
    assert!(config.enable_gradients);
}

#[test]
fn test_embedded_scenario_headless_server() {
    let config = EmbeddedConfig::new(Size::new(1, 1))
        .low_memory();
    
    assert!(config.low_memory_mode);
    assert!(!config.enable_animations);
    assert!(!config.enable_shadows);
    assert!(!config.enable_gradients);
}

#[test]
fn test_lightweight_scenario_minimal_ui() {
    let config = LightweightConfig::minimal();
    
    assert!(config.disable_animations);
    assert!(config.disable_shadows);
    assert!(config.disable_gradients);
    assert!(config.simple_borders);
    assert!(config.reduced_padding);
    assert!(config.minimal_signals);
}

#[test]
fn test_lightweight_scenario_partial() {
    let config = LightweightConfig::new()
        .with_shadows_disabled()
        .with_animations_disabled();
    
    assert!(config.disable_animations);
    assert!(config.disable_shadows);
    assert!(!config.disable_gradients);
    assert!(!config.simple_borders);
}

#[test]
fn test_embedded_config_font_scale_clamping() {
    let config_too_small = EmbeddedConfig::new(Size::new(800, 600))
        .with_font_scale(0.1);
    assert!((config_too_small.font_scale - 0.5).abs() < 0.01);
    
    let config_too_large = EmbeddedConfig::new(Size::new(800, 600))
        .with_font_scale(5.0);
    assert!((config_too_large.font_scale - 3.0).abs() < 0.01);
    
    let config_valid = EmbeddedConfig::new(Size::new(800, 600))
        .with_font_scale(1.5);
    assert!((config_valid.font_scale - 1.5).abs() < 0.01);
}

#[test]
fn test_embedded_config_effect_settings() {
    let full_effects = EmbeddedConfig::new(Size::new(800, 600));
    assert!(full_effects.enable_animations);
    assert!(!full_effects.enable_shadows);
    assert!(full_effects.enable_gradients);
    
    let minimal_effects = EmbeddedConfig::new(Size::new(800, 600)).low_memory();
    assert!(!minimal_effects.enable_animations);
    assert!(!minimal_effects.enable_shadows);
    assert!(!minimal_effects.enable_gradients);
}
