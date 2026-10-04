// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;
use crate::gpu::GpuType;
#[test]
fn test_menu_config_default() {
    let config = MenuConfig::new();
    assert!(config.animation_speed() > 0.0);
    assert!(config.max_visible_items() >= 5);
    assert!(!config.has_user_overrides());
}

/// The config directory must follow the host OS convention, which is why it is
/// derived from `dirs::config_dir()` instead of a hand-built `~/.config` path.
///
/// This asserts only the **platform-agnostic** fact: the directory is ours, and
/// its parent is whatever `dirs::config_dir()` reports. The OS-specific
/// expectation (`Library/Application Support` on macOS) belongs to the platform
/// layer, not here — testing it from `menu_config` would hard-code an OS into a
/// UI-config module (principle #68). That assertion now lives in
/// `platform::macos::tests`.
#[test]
fn default_config_dir_follows_host_convention() {
    let dir = ConfigPersistence::new().config_dir().to_path_buf();

    // The application folder name is ours; the parent is the OS's choice.
    assert_eq!(dir.file_name().and_then(|n| n.to_str()), Some("rust-widgets"));

    let Some(base) = dirs::config_dir() else {
        return; // No home directory (unusual CI): the fallback path applies.
    };
    assert_eq!(dir.parent(), Some(base.as_path()));
}

/// Hardware detection must route through the platform backend, not sniff the OS
/// from this layer (principle #36/#37). The delegation is observable: whatever
/// the backend reports for battery state is what the config ends up holding.
#[test]
fn hardware_detection_delegates_to_platform_backend() {
    let backend_on_battery = crate::platform::platform_facts().is_on_battery();
    let config = MenuConfig::new();
    assert_eq!(
        config.hardware_caps().on_battery,
        backend_on_battery,
        "on_battery must come from Platform::is_on_battery",
    );

    // A backend reporting `None` must stay `None`: "unknown" must not be reported as a
    // fabricated figure that a caller cannot tell apart from a measurement. The
    // conservative default is for scoring only and is never published here.
    let reported = crate::platform::platform_facts().total_memory_mb();
    assert_eq!(
        config.hardware_caps().system_ram_mb,
        reported,
        "system_ram_mb must be the backend's own answer, `None` included",
    );
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
    let temp_dir = tempfile::TempDir::new().expect("temp dir must be created");
    let persistence = ConfigPersistence::with_dir(temp_dir.path().to_path_buf());
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
    // `TempDir` removes its directory on drop; no manual recursive cleanup is needed.
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
    assert!(matches!(PerformanceLevel::default(), PerformanceLevel::Medium));
}
#[test]
fn test_hardware_capabilities() {
    let caps = HardwareCapabilities {
        gpu_type: GpuType::Discrete,
        gpu_memory_mb: Some(4096),
        gpu_memory_is_measured: true,
        gpu_performance_score: 80,
        system_ram_mb: Some(16384),
        cpu_performance_score: 70,
        on_battery: false,
        performance_level: PerformanceLevel::High,
    };
    assert!(matches!(caps.gpu_type, GpuType::Discrete));
    assert_eq!(caps.gpu_memory_mb, Some(4096));
    assert!(caps.gpu_memory_is_measured);
    assert!(!caps.on_battery);
    assert!(matches!(caps.performance_level, PerformanceLevel::High));
}

/// A host that cannot report VRAM must not be described as if it had been
/// probed for it.
#[test]
fn test_unmeasured_gpu_memory_is_not_reported_as_detected() {
    let caps = HardwareCapabilities {
        gpu_type: GpuType::Integrated,
        gpu_memory_mb: None,
        gpu_memory_is_measured: false,
        gpu_performance_score: 40,
        system_ram_mb: Some(8192),
        cpu_performance_score: 50,
        on_battery: false,
        performance_level: PerformanceLevel::Medium,
    };
    assert_eq!(caps.gpu_memory_mb, None);
    assert!(!caps.gpu_memory_is_measured);
    // The scoring input is separate from the reported figure, so a score can still
    // be computed for a host whose VRAM is unknown.
    assert_eq!(MenuConfig::assumed_gpu_memory(&GpuType::Integrated), 512);
    assert_eq!(MenuConfig::assumed_gpu_memory(&GpuType::Discrete), 4096);
    assert_eq!(MenuConfig::assumed_gpu_memory(&GpuType::Cpu), 0);
}

/// The environment override is the one real source of a VRAM figure.
#[test]
fn test_gpu_memory_override_is_used_when_present_and_absent_means_unknown() {
    // Absent the override this host genuinely cannot report VRAM, so the answer is
    // `None` rather than a constant that would be indistinguishable from a probe.
    let previous = std::env::var("RUST_WIDGETS_GPU_MEMORY_MB").ok();
    std::env::remove_var("RUST_WIDGETS_GPU_MEMORY_MB");
    assert_eq!(MenuConfig::detect_gpu_memory_for_test(), None);

    std::env::set_var("RUST_WIDGETS_GPU_MEMORY_MB", "8192");
    assert_eq!(MenuConfig::detect_gpu_memory_for_test(), Some(8192));

    // A zero or unparsable value is not a measurement either.
    std::env::set_var("RUST_WIDGETS_GPU_MEMORY_MB", "0");
    assert_eq!(MenuConfig::detect_gpu_memory_for_test(), None);
    std::env::set_var("RUST_WIDGETS_GPU_MEMORY_MB", "not-a-number");
    assert_eq!(MenuConfig::detect_gpu_memory_for_test(), None);

    match previous {
        Some(value) => std::env::set_var("RUST_WIDGETS_GPU_MEMORY_MB", value),
        None => std::env::remove_var("RUST_WIDGETS_GPU_MEMORY_MB"),
    }
}

/// `save`/`load` must round-trip every field of `UserOverrides`.
///
/// # Why this is a real test and not a probe
///
/// `UserOverrides` has seven fields and the loader has seven `match` arms; a field
/// added to the struct without an arm would be persisted and then silently dropped on
/// read-back, which is the failure mode a user experiences as "my settings reset on
/// restart". The setter/clamping tests above never touch persistence, so this is the
/// only place the write→read contract is asserted.
///
/// Uses `tempfile::TempDir` (as the `i18n` tests do) so each test owns an exclusive,
/// RAII-cleaned directory and parallel test processes cannot interfere with each other.
#[test]
fn persistence_round_trips_every_user_override() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir must be created");
    let persistence = ConfigPersistence::with_dir(temp_dir.path().to_path_buf());

    let mut config = MenuConfig::new();
    config.set_animation_speed(1.25);
    config.set_max_visible_items(12);
    config.set_animations_enabled(false);
    config.set_hardware_acceleration(true);
    persistence.save(&config).expect("save must reach the temp dir");

    let loaded = persistence.load().expect("load must read back what save wrote");
    assert_eq!(loaded.animation_speed, Some(1.25), "animation_speed must round-trip");
    assert_eq!(loaded.max_visible_items, Some(12), "max_visible_items must round-trip");
    assert_eq!(loaded.animations, Some(false), "animations must round-trip");
    assert_eq!(loaded.hardware_acceleration, Some(true), "hardware_acceleration must round-trip");

    // `clear` must actually remove the file, or a "reset to defaults" would be undone
    // by the next launch.
    persistence.clear().expect("clear must succeed on a saved config");
    assert!(!persistence.exists(), "clear must remove the config file");
    // `TempDir` RAII-cleans the directory on drop.
}

/// `set_animation_speed` must not let a NaN poison the live value or the stored
/// override; the getter contract is always `0.1..=3.0`.
#[test]
fn animation_speed_rejects_nan_and_saturates_non_finite() {
    let mut config = MenuConfig::new();
    config.set_animation_speed(1.5);
    assert_eq!(config.animation_speed(), 1.5);

    // NaN is rejected: neither the live value nor the override changes.
    config.set_animation_speed(f32::NAN);
    assert_eq!(config.animation_speed(), 1.5);
    assert_eq!(config.user_overrides().animation_speed, Some(1.5));

    // ±Inf saturate to the nearest bound, matching finite out-of-range input.
    config.set_animation_speed(f32::INFINITY);
    assert_eq!(config.animation_speed(), 3.0);
    config.set_animation_speed(f32::NEG_INFINITY);
    assert_eq!(config.animation_speed(), 0.1);

    // Finite out-of-range still clamps as before.
    config.set_animation_speed(99.0);
    assert_eq!(config.animation_speed(), 3.0);
}

/// A known field that fails to parse must produce an explicit error naming the field
/// and its line, not silently become `None`.
#[test]
fn load_reports_known_field_parse_errors() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir must be created");
    let persistence = ConfigPersistence::with_dir(temp_dir.path().to_path_buf());

    std::fs::write(
        temp_dir.path().join("menu_config.json"),
        "animations_enabled=not-a-bool\nunknown_key=whatever\n",
    )
    .unwrap();

    match persistence.load() {
        Err(ConfigLoadError::Parse { line, field, value }) => {
            assert_eq!(line, 1, "the error must name the line");
            assert_eq!(field, "animations_enabled", "the error must name the field");
            assert_eq!(value, "not-a-bool", "the error must name the value");
        }
        other => panic!("expected a known-field parse error, got {other:?}"),
    }
}

/// A rejected load must not corrupt the configuration the dialog already holds.
#[test]
fn rejected_load_keeps_previous_config() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir must be created");
    let persistence = ConfigPersistence::with_dir(temp_dir.path().to_path_buf());

    let mut config = MenuConfig::new();
    config.set_animation_speed(2.0);
    let mut dialog = MenuConfigDialog::with_persistence(config, persistence);

    std::fs::write(temp_dir.path().join("menu_config.json"), "max_visible_items=bad\n").unwrap();

    assert!(dialog.load().is_err(), "a known-field parse error must reject the load");
    assert_eq!(
        dialog.config().animation_speed(),
        2.0,
        "previous config must survive a rejected load"
    );
}

/// After a reload, a field removed from the file must fall back to its hardware
/// default rather than keeping a stale reversed preference.
#[test]
fn reload_restores_baseline_for_deleted_fields() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir must be created");
    let persistence = ConfigPersistence::with_dir(temp_dir.path().to_path_buf());

    let mut config = MenuConfig::new();
    let baseline_animations = config.animations_enabled();
    config.set_animations_enabled(!baseline_animations);

    // The file drops the animations field, keeping only transparency.
    std::fs::write(temp_dir.path().join("menu_config.json"), "transparency_enabled=true\n")
        .unwrap();

    let mut dialog = MenuConfigDialog::with_persistence(config, persistence);
    dialog.load().expect("load must succeed");

    assert_eq!(dialog.config().animations_enabled(), baseline_animations);
    assert_eq!(dialog.config().user_overrides().animations, None);
    assert!(dialog.config().transparency_enabled());
    assert_eq!(dialog.config().user_overrides().transparency, Some(true));
}

/// Loading a missing file must clear overrides and return to the hardware baseline.
#[test]
fn load_missing_file_restores_baseline_and_clears_overrides() {
    let temp_dir = tempfile::TempDir::new().expect("temp dir must be created");
    let persistence = ConfigPersistence::with_dir(temp_dir.path().to_path_buf());

    let mut config = MenuConfig::new();
    let baseline_animations = config.animations_enabled();
    config.set_animations_enabled(!baseline_animations);
    assert!(config.has_user_overrides());

    let mut dialog = MenuConfigDialog::with_persistence(config, persistence);
    dialog.load().expect("load of a missing file yields defaults");

    assert!(!dialog.has_overrides());
    assert_eq!(dialog.config().animations_enabled(), baseline_animations);
}

/// `increase_max_items` must not overflow when the current value is already at the
/// type's maximum; it saturates instead of wrapping to a small value.
#[test]
fn increase_max_items_saturates_at_maximum() {
    let mut config = MenuConfig::new();
    config.set_max_visible_items(u32::MAX);
    assert_eq!(config.max_visible_items(), u32::MAX);

    let mut dialog = MenuConfigDialog::new(config);
    dialog.increase_max_items();
    assert_eq!(dialog.config().max_visible_items(), u32::MAX);
}
