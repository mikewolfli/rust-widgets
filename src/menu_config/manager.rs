// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::MenuConfig;
use crate::compat::{lock, Mutex, MutexGuard};
use crate::style::MenuEffects;

/// The process-wide menu configuration.
///
/// # Why `Option` rather than an initialized `OnceLock`
///
/// Building a [`MenuConfig`] runs hardware detection (GPU adapter enumeration, a
/// battery probe). Doing that from the first `draw` would pay it on the UI thread at
/// the worst possible moment. The slot therefore starts empty and a draw pass reads
/// [`MenuEffects::legacy`] until the host calls [`init`] (which `App::init` does at
/// startup) or installs a policy through [`install_policy`].
static GLOBAL_MENU_CONFIG: Mutex<Option<MenuConfigManager>> = Mutex::new(None);

/// Initializes the process-wide manager, running hardware detection once.
///
/// Idempotent: the first call (normally from `App::init`) does the work, later calls
/// only observe the result. A policy installed by [`install_policy`] beforehand is left
/// untouched, so a pinned verification policy survives `App::init`.
pub fn init() {
    let mut slot = lock(&GLOBAL_MENU_CONFIG);
    if slot.is_none() {
        *slot = Some(MenuConfigManager::new());
    }
}

/// The process-wide manager, or `None` when [`init`] has not run.
pub fn manager() -> MutexGuard<'static, Option<MenuConfigManager>> {
    lock(&GLOBAL_MENU_CONFIG)
}

/// Installs a caller-supplied policy as the process-wide configuration.
///
/// Unlike [`init`], this never probes the host: the caller decides the policy. That is
/// what makes the snapshot export and the rendering census reproducible — their
/// committed output must not change with the machine that regenerated it.
pub fn install_policy(config: MenuConfig) {
    *lock(&GLOBAL_MENU_CONFIG) = Some(MenuConfigManager::from_config(config));
}

/// Installs [`MenuConfig::verification`], the deterministic verification policy.
pub fn use_verification_policy() {
    install_policy(MenuConfig::verification());
}

/// The effect decisions menu controls should use right now.
///
/// Never initializes: an uninitialized slot answers [`MenuEffects::legacy`], which is
/// the pre-wiring rendering. This is what keeps the draw path free of hardware probes.
pub fn current_effects() -> MenuEffects {
    match *lock(&GLOBAL_MENU_CONFIG) {
        Some(ref manager) => manager.config().effects(),
        None => MenuEffects::legacy(),
    }
}

/// Turns the adaptive configuration on or off, initializing the manager if needed.
pub fn set_enabled(enabled: bool) {
    init();
    let mut slot = lock(&GLOBAL_MENU_CONFIG);
    if let Some(manager) = slot.as_mut() {
        manager.config_mut().set_enabled(enabled);
    }
}

/// Replaces the persisted overrides and re-applies them over a fresh hardware
/// baseline.
pub fn apply_overrides(overrides: super::UserOverrides) {
    init();
    let mut slot = lock(&GLOBAL_MENU_CONFIG);
    if let Some(manager) = slot.as_mut() {
        manager.config_mut().reload_overrides(overrides);
    }
}

/// Global menu configuration manager.
pub struct MenuConfigManager {
    config: MenuConfig,
    auto_adjust: bool,
}
impl MenuConfigManager {
    /// Creates a new configuration manager.
    pub fn new() -> Self {
        Self { config: MenuConfig::new(), auto_adjust: true }
    }

    /// Wraps an already-decided configuration.
    ///
    /// Hardware re-detection is off: a pinned policy must survive
    /// [`Self::refresh_hardware_detection`] unchanged, otherwise a later refresh would
    /// silently re-probe the host and reintroduce machine-dependent output.
    fn from_config(config: MenuConfig) -> Self {
        Self { config, auto_adjust: false }
    }
    /// Gets the current configuration.
    pub fn config(&self) -> &MenuConfig {
        &self.config
    }
    /// Gets mutable access to configuration.
    pub fn config_mut(&mut self) -> &mut MenuConfig {
        &mut self.config
    }
    /// Enables or disables automatic adjustment based on hardware changes.
    pub fn set_auto_adjust(&mut self, enabled: bool) {
        self.auto_adjust = enabled;
    }
    /// Returns true if auto-adjustment is enabled.
    pub fn auto_adjust(&self) -> bool {
        self.auto_adjust
    }
    /// Re-detects hardware and updates configuration if auto-adjust is enabled.
    pub fn refresh_hardware_detection(&mut self) {
        if self.auto_adjust && !self.config.has_user_overrides() {
            // Re-detection rebuilds from the product default, which keeps the master
            // switch on. Recording that as an override would be a lie — the guard above
            // already proved there is none — and the false entry would then make
            // `has_user_overrides()` true and silently block every later refresh.
            self.config = MenuConfig::new();
            // A fresh detection has just read the current power source, so the battery
            // reduction is applied in the same pass rather than waiting for a caller that
            // has no reason to know a refresh happened.
            self.adapt_for_battery_power();
        }
    }

    /// Applies the battery reduction when the power source changed.
    ///
    /// This is the automatic trigger point: a host that receives a platform power-source
    /// notification calls this once, and the policy adapts. It is idempotent — the
    /// reduction functions write the same values — so a duplicate notification is
    /// harmless.
    pub fn on_power_source_changed(&mut self, on_battery: bool) {
        self.config.set_on_battery(on_battery);
        self.adapt_for_battery_power();
    }

    /// Turns the adaptive configuration on or off.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.set_enabled(enabled);
    }

    /// The effect snapshot this manager's policy implies.
    pub fn effects(&self) -> MenuEffects {
        self.config.effects()
    }
    /// Adapts settings for battery power (reduces effects).
    pub fn adapt_for_battery_power(&mut self) {
        if self.config.hardware_caps().on_battery && self.auto_adjust {
            self.config.apply_battery_adaptive_reduction();
        }
    }
}
crate::impl_default_via_new!(MenuConfigManager);
