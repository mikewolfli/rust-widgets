use std::fs;

use crate::gpu::GpuType;

use super::*;

#[test]
fn test_menu_config_default() {
    let config = MenuConfig::new();

    assert!(config.animation_speed() > 0.0);
    assert!(config.max_visible_items() >= 5);
    assert!(!config.has_user_overrides());
}

#[test]
fn test_user_overrides() {
    let mut config = MenuConfig::new();

    config.set_animations_enabled(false);
    config.set_transparency_enabled(true);
    config.set_animation_speed(1.5);

    assert!(config.has_user_overrides());
    assert!(!config.animations_enabled());
    assert!(config.transparency_enabled());
    assert_eq!(config.animation_speed(), 1.5);

    assert_eq!(config.user_overrides().animations, Some(false));
    assert_eq!(config.user_overrides().transparency, Some(true));
}

#[test]
fn test_reset_to_defaults() {
    let mut config = MenuConfig::new();

    let original_animations = config.animations_enabled();

    config.set_animations_enabled(!original_animations);
    assert!(config.has_user_overrides());

    config.reset_to_defaults();

    assert!(!config.has_user_overrides());
    assert_eq!(config.animations_enabled(), original_animations);
}

#[test]
fn test_animation_speed_clamping() {
    let mut config = MenuConfig::new();

    config.set_animation_speed(0.05);
    assert_eq!(config.animation_speed(), 0.1);

    config.set_animation_speed(5.0);
    assert_eq!(config.animation_speed(), 3.0);

    config.set_animation_speed(1.5);
    assert_eq!(config.animation_speed(), 1.5);
}

#[test]
fn test_max_visible_items_minimum() {
    let mut config = MenuConfig::new();

    config.set_max_visible_items(2);
    assert_eq!(config.max_visible_items(), 5);

    config.set_max_visible_items(30);
    assert_eq!(config.max_visible_items(), 30);
}

#[test]
fn test_settings_summary() {
    let config = MenuConfig::new();
    let summary = config.settings_summary();

    assert!(summary.contains("Menu Settings:"));
    assert!(summary.contains("Animations:"));
    assert!(summary.contains("Transparency:"));
    assert!(summary.contains("Performance Level:"));
}

#[test]
fn test_config_manager() {
    let mut manager = MenuConfigManager::new();

    assert!(manager.auto_adjust());

    manager.set_auto_adjust(false);
    assert!(!manager.auto_adjust());

    let _ = manager.config().animations_enabled();
}

#[test]
fn test_config_persistence_roundtrip() {
    use std::env;

    let temp_dir = env::temp_dir().join("rust-widgets-test");
    let persistence = ConfigPersistence::with_dir(temp_dir.clone());

    let _ = persistence.clear();

    let mut config = MenuConfig::new();
    config.set_animations_enabled(false);
    config.set_transparency_enabled(true);
    config.set_animation_speed(1.5);

    persistence.save(&config).unwrap();
    assert!(persistence.exists());

    let overrides = persistence.load().unwrap();
    assert_eq!(overrides.animations, Some(false));
    assert_eq!(overrides.transparency, Some(true));
    assert_eq!(overrides.animation_speed, Some(1.5));

    let _ = persistence.clear();
    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_config_dialog() {
    let config = MenuConfig::new();
    let mut dialog = MenuConfigDialog::new(config);

    let initial_animations = dialog.config().animations_enabled();
    dialog.toggle_animations();
    assert_eq!(dialog.config().animations_enabled(), !initial_animations);

    let initial_speed = dialog.config().animation_speed();
    dialog.increase_animation_speed();
    assert!(dialog.config().animation_speed() > initial_speed);

    dialog.decrease_animation_speed();
    assert!(dialog.config().animation_speed() <= initial_speed + 0.1);

    let initial_max = dialog.config().max_visible_items();
    dialog.increase_max_items();
    assert_eq!(dialog.config().max_visible_items(), initial_max + 5);

    dialog.decrease_max_items();
    assert_eq!(dialog.config().max_visible_items(), initial_max);

    dialog.reset_to_defaults();
    assert!(!dialog.has_overrides());

    let summary = dialog.settings_summary();
    assert!(summary.contains("Menu Settings:"));
    assert!(summary.contains("Performance Level:"));

    let gpu_desc = dialog.gpu_description();
    assert!(!gpu_desc.is_empty());
}

#[test]
fn test_performance_level_enum() {
    assert_ne!(PerformanceLevel::Low, PerformanceLevel::Medium);
    assert_ne!(PerformanceLevel::Medium, PerformanceLevel::High);

    assert!(matches!(
        PerformanceLevel::default(),
        PerformanceLevel::Medium
    ));
}

#[test]
fn test_hardware_capabilities() {
    let caps = HardwareCapabilities {
        gpu_type: GpuType::Discrete,
        gpu_memory_mb: 4096,
        gpu_performance_score: 80,
        system_ram_mb: 16384,
        cpu_performance_score: 70,
        on_battery: false,
        performance_level: PerformanceLevel::High,
    };

    assert!(matches!(caps.gpu_type, GpuType::Discrete));
    assert_eq!(caps.gpu_memory_mb, 4096);
    assert!(!caps.on_battery);
    assert!(matches!(caps.performance_level, PerformanceLevel::High));
}
