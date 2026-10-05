// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Core `App` type — lifecycle wrapper with configuration and callbacks.

use crate::core::ObjectId;
use crate::WidgetTriggerEvent;

use super::handle::{dispatch_trigger, WindowHandle};
use super::lifecycle::AppLifecycle;

// ═══════════════════════════════════════════════════════════════
// AppConfig
// ═══════════════════════════════════════════════════════════════

/// Configuration options for an [`App`] instance.
///
/// Use [`AppConfig::default`] or the builder-style setters:
///
/// ```
/// use rust_widgets::app::AppConfig;
///
/// let config = AppConfig::default()
///     .with_app_name("MyApp")
///     .with_organization("Acme Corp")
///     .with_version("1.0.0");
/// ```
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Human-readable application name (used for window titles, etc.).
    pub app_name: String,
    /// Organization or vendor name.
    pub organization: String,
    /// Application version string (e.g. "1.0.0").
    pub version: String,
    /// Whether to initialise the i18n subsystem (default: `true`).
    pub enable_i18n: bool,
    /// Whether to initialise the accessibility subsystem (default: `true`).
    pub enable_accessibility: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app_name: String::new(),
            organization: String::new(),
            version: String::new(),
            enable_i18n: true,
            enable_accessibility: true,
        }
    }
}

impl AppConfig {
    /// Set the application name.
    pub fn with_app_name(mut self, name: &str) -> Self {
        self.app_name = name.to_owned();
        self
    }

    /// Set the organization name.
    pub fn with_organization(mut self, org: &str) -> Self {
        self.organization = org.to_owned();
        self
    }

    /// Set the application version string.
    pub fn with_version(mut self, version: &str) -> Self {
        self.version = version.to_owned();
        self
    }

    /// Enable or disable i18n initialisation.
    pub fn with_i18n(mut self, enable: bool) -> Self {
        self.enable_i18n = enable;
        self
    }

    /// Enable or disable accessibility bridge initialisation.
    pub fn with_accessibility(mut self, enable: bool) -> Self {
        self.enable_accessibility = enable;
        self
    }
}

// ═══════════════════════════════════════════════════════════════
// App
// ═══════════════════════════════════════════════════════════════

/// High-level application wrapper that manages the event loop lifecycle.
///
/// # Examples
///
/// ## Minimal (no configuration)
///
/// ```rust,no_run
/// use rust_widgets::app::{App, WidgetHandle};
///
/// let mut app = App::new();
/// app.init();
/// let win = app.new_window("Hello", 100, 100, 640, 480);
/// let btn = win.new_button("Click me", 10, 10, 120, 32);
///
/// btn.on_click(|| {
///     println!("Button clicked!");
/// });
///
/// app.run();
/// ```
///
/// ## With configuration and callbacks
///
/// ```rust,no_run
/// use rust_widgets::app::{App, AppConfig};
///
/// let mut app = App::with_config(
///     AppConfig::default()
///         .with_app_name("MyApp")
///         .with_organization("Acme Inc"),
/// )
/// .on_startup(|| {
///     // Called once after init() completes.
/// })
/// .on_shutdown(|| {
///     // Called once before the event loop exits.
/// });
///
/// app.init();
/// // ... create widgets ...
/// app.run();
/// ```
pub struct App {
    config: AppConfig,
    lifecycle: AppLifecycle,
}

impl App {
    /// Create a new application handle with default configuration.
    pub fn new() -> Self {
        Self { config: AppConfig::default(), lifecycle: AppLifecycle::new() }
    }

    /// Create a new application handle with a custom configuration.
    pub fn with_config(config: AppConfig) -> Self {
        Self { config, lifecycle: AppLifecycle::new() }
    }

    /// Return a reference to the current configuration.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Return a mutable reference to the current configuration.
    pub fn config_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }

    /// Return a reference to the application lifecycle manager.
    pub fn lifecycle(&self) -> &AppLifecycle {
        &self.lifecycle
    }

    /// Return a mutable reference to the application lifecycle manager.
    pub fn lifecycle_mut(&mut self) -> &mut AppLifecycle {
        &mut self.lifecycle
    }

    /// Register a callback invoked once after [`init`](App::init) completes.
    ///
    /// Returns `self` so calls can be chained.
    pub fn on_startup<F: FnOnce() + Send + 'static>(self, f: F) -> Self {
        with_startup(|slot| {
            *slot = Some(Box::new(f));
        });
        self
    }

    /// Register a callback invoked once before the event loop exits.
    ///
    /// Returns `self` so calls can be chained.
    pub fn on_shutdown<F: FnOnce() + Send + 'static>(self, f: F) -> Self {
        with_shutdown(|slot| {
            *slot = Some(Box::new(f));
        });
        self
    }

    /// Initialize global platform and (optionally) i18n subsystems.
    ///
    /// Call this once before creating windows or running the loop.
    ///
    /// # The `AppConfig` switches are honoured here
    ///
    /// `AppConfig::enable_i18n` / `enable_accessibility` are declared as init switches,
    /// so this method must be the place they take effect. It used to call the
    /// unparameterised `crate::init()`, which unconditionally brought up every optional
    /// subsystem — so `with_i18n(false)` and `with_accessibility(false)` changed nothing.
    ///
    /// The two halves of `crate::init()` are now driven separately with the configured
    /// flags: the platform runtime is always brought up (a control cannot exist without
    /// it), and [`platform::profile::init_optional_subsystems`] — the i18n catalogue and
    /// its watcher — runs only when `enable_i18n` is set. The accessibility switch is
    /// recorded process-wide so capability wiring that asks can honour it; see
    /// [`App::accessibility_enabled`]. The no-config crate entry [`crate::init`] keeps the
    /// previous all-on behaviour, because its callers have no `AppConfig` to consult.
    ///
    /// [`platform::profile::init_optional_subsystems`]: crate::platform::profile::init_optional_subsystems
    pub fn init(&mut self) {
        trace_runtime_route("app::init");

        // The platform runtime is unconditional: it owns the window and the event loop,
        // which every profile that can create a control needs.
        crate::platform::profile::runtime_init();

        if self.config.enable_i18n {
            crate::platform::profile::init_optional_subsystems();
        } else {
            log::debug!("[app] i18n initialisation skipped: AppConfig::with_i18n(false) was set");
        }

        // Record the accessibility switch so capability wiring can honour it. Set before
        // the lifecycle transition and the startup callback, so a startup callback that
        // queries accessibility observes the configured value.
        set_accessibility_enabled(self.config.enable_accessibility);

        // Transition lifecycle to Foreground after init completes.
        self.lifecycle.transition(crate::app::lifecycle::AppLifecycleState::Foreground);

        // Fire the startup callback after everything is initialised.
        fire_startup();
    }

    /// Whether this application's accessibility subsystem was enabled at [`init`](App::init).
    ///
    /// Mirrors [`AppConfig::enable_accessibility`](AppConfig::enable_accessibility): the
    /// config switch is a declaration, and this reports the value the running application
    /// actually recorded when it initialised. Before `init` it reports the process default
    /// (`true`).
    pub fn accessibility_enabled(&self) -> bool {
        accessibility_enabled()
    }

    /// Run the platform main event loop (blocks).
    ///
    /// While the loop runs, every polled [`WidgetTriggerEvent`] is dispatched
    /// to the callbacks registered via `WidgetHandle::on_click` /
    /// `WidgetHandle::on_value_changed`.
    pub fn run(&self) {
        trace_runtime_route("app::run");

        // Enter the platform event loop.  On each tick the platform will
        // invoke dispatch_trigger for us (or we poll manually below).
        // We also drain any remaining events after the loop ends.
        crate::run();

        fire_shutdown();
    }

    /// Request the event loop to shut down.
    pub fn quit(&mut self) {
        crate::quit();
        self.lifecycle.transition(crate::app::lifecycle::AppLifecycleState::Terminating);
    }

    /// Run the platform event loop on a background thread (non-blocking).
    ///
    /// Returns a `JoinHandle` that completes when the event loop exits.
    /// This allows the calling thread to continue working while the
    /// event loop runs in parallel.
    ///
    /// Only available on desktop targets where threading is supported.
    /// On other targets, falls back to blocking `run()` via `run_blocking`.
    pub fn run_async(&self) -> std::thread::JoinHandle<()> {
        trace_runtime_route("app::run_async");
        std::thread::spawn(|| {
            crate::run();
            fire_shutdown();
        })
    }

    /// Create a top-level window and return a type-safe handle.
    pub fn new_window(&self, title: &str, x: i32, y: i32, w: u32, h: u32) -> WindowHandle {
        let id = crate::create_window(title, x, y, w, h);
        // Record the size here so a layout applied immediately afterwards has a rect to
        // work with. See `WindowHandle::record_created_geometry`.
        WindowHandle::record_created_geometry(id, x, y, w, h);
        WindowHandle::from_raw(id)
    }

    /// Poll the next triggered event from the platform layer.
    ///
    /// When a matching callback is registered the event is dispatched
    /// automatically; you only need this method for manual event loops.
    pub fn poll_event(&self) -> Option<WidgetTriggerEvent> {
        let ev = crate::poll_widget_trigger_event();
        if let Some(ref ev) = ev {
            dispatch_trigger(ev.widget_id, ev.kind);
        }
        ev
    }

    /// Poll the raw `ObjectId` of the most recently triggered widget.
    pub fn poll_triggered(&self) -> Option<ObjectId> {
        crate::poll_widget_triggered()
    }
}

crate::impl_default_via_new!(App);

// ── Startup / shutdown one-shot callbacks ─────────────────────
// Using OnceLock + Mutex instead of thread_local! so callbacks
// are available across threads (e.g. in run_async).

use crate::compat::Mutex;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::OnceLock;

static STARTUP: OnceLock<Mutex<Option<Box<dyn FnOnce() + Send>>>> = OnceLock::new();
static SHUTDOWN: OnceLock<Mutex<Option<Box<dyn FnOnce() + Send>>>> = OnceLock::new();

/// Whether the accessibility subsystem is enabled for the running application.
///
/// Defaults to `true`, matching [`AppConfig::default`]. [`App::init`] writes
/// [`AppConfig::enable_accessibility`] here so the switch is a real, queryable fact rather
/// than a field nothing reads. Process-wide because accessibility is a property of the
/// running application, not of one window.
static A11Y_ENABLED: AtomicBool = AtomicBool::new(true);

/// Record whether accessibility is enabled for the running application.
fn set_accessibility_enabled(enabled: bool) {
    A11Y_ENABLED.store(enabled, AtomicOrdering::SeqCst);
}

/// Whether accessibility is enabled for the running application.
///
/// `pub(crate)` so capability wiring elsewhere can gate on it without importing `App`.
/// Returns the process default (`true`) until [`App::init`] records a configuration.
pub(crate) fn accessibility_enabled() -> bool {
    A11Y_ENABLED.load(AtomicOrdering::SeqCst)
}

fn with_startup<F, R>(f: F) -> R
where
    F: FnOnce(&mut Option<Box<dyn FnOnce() + Send>>) -> R,
{
    let lock = STARTUP.get_or_init(|| Mutex::new(None));
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

fn with_shutdown<F, R>(f: F) -> R
where
    F: FnOnce(&mut Option<Box<dyn FnOnce() + Send>>) -> R,
{
    let lock = SHUTDOWN.get_or_init(|| Mutex::new(None));
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

/// Run a registered startup callback, releasing the lock before invoking it.
///
/// The callback is taken out of the slot and invoked **after** the guard is dropped. A startup
/// callback that re-registers a startup callback must not re-lock the same non-reentrant `Mutex`
/// while it is already held on this thread — that was a same-thread deadlock. A callback
/// re-registered here takes effect on the next [`fire_startup`] (i.e. the next `init`).
fn fire_startup() {
    let cb = with_startup(|slot| slot.take());
    if let Some(cb) = cb {
        cb();
    }
}

/// Run a registered shutdown callback, releasing the lock before invoking it.
///
/// Mirrors [`fire_startup`]: a shutdown callback that re-registers a shutdown callback takes
/// effect on the next [`fire_shutdown`] (i.e. the next `run`/`run_async`), and never deadlocks.
fn fire_shutdown() {
    let cb = with_shutdown(|slot| slot.take());
    if let Some(cb) = cb {
        cb();
    }
}

/// Records an `App` lifecycle stage in the runtime trace.
///
/// Forwards to the crate-root trace so both report the same fields: the local copy
/// that used to live here logged only `stage=`, which meant the app stages and the
/// `lib.rs` stages produced two different record shapes for one audit line
/// (principle #54 — one definition per meaning).
fn trace_runtime_route(stage: &str) {
    crate::trace_runtime_route(stage);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    /// A startup callback that re-registers a startup callback must not deadlock.
    ///
    /// The old `init` invoked the callback while holding the global `STARTUP` mutex, so the
    /// re-registering `on_startup` re-locked the same non-reentrant mutex on the same thread and
    /// hung. `fire_startup` takes the slot and releases the lock before invoking, so the
    /// re-registered callback simply takes effect on the next startup.
    #[test]
    fn a_startup_callback_can_reenter_registration() {
        let calls = Arc::new(AtomicU32::new(0));

        let c1 = Arc::clone(&calls);
        let c2 = Arc::clone(&calls);
        let _app = App::new().on_startup(move || {
            c1.fetch_add(1, Ordering::SeqCst);
            // Re-register from within the callback; this used to deadlock.
            let c3 = Arc::clone(&c2);
            let _ = App::new().on_startup(move || {
                c3.fetch_add(10, Ordering::SeqCst);
            });
        });

        fire_startup();
        assert_eq!(calls.load(Ordering::SeqCst), 1, "the outer startup callback runs");

        // The re-registered callback takes effect on the *next* startup.
        fire_startup();
        assert_eq!(calls.load(Ordering::SeqCst), 11, "the re-registered callback runs later");
    }

    /// A shutdown callback that re-registers a shutdown callback must not deadlock.
    #[test]
    fn a_shutdown_callback_can_reenter_registration() {
        let calls = Arc::new(AtomicU32::new(0));

        let c1 = Arc::clone(&calls);
        let c2 = Arc::clone(&calls);
        let _app = App::new().on_shutdown(move || {
            c1.fetch_add(1, Ordering::SeqCst);
            // Re-register from within the callback; this used to deadlock.
            let c3 = Arc::clone(&c2);
            let _ = App::new().on_shutdown(move || {
                c3.fetch_add(10, Ordering::SeqCst);
            });
        });

        fire_shutdown();
        assert_eq!(calls.load(Ordering::SeqCst), 1, "the outer shutdown callback runs");

        fire_shutdown();
        assert_eq!(calls.load(Ordering::SeqCst), 11, "the re-registered callback runs later");
    }

    /// Serialises the tests that drive `App::init`, because `init` mutates two process-wide
    /// globals (the i18n manager and the accessibility flag).
    fn app_init_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// N-S-70: `with_i18n(false)` must actually skip i18n initialisation.
    ///
    /// `AppConfig::enable_i18n` was declared but `App::init` called the unparameterised
    /// `crate::init()`, so the global i18n manager was brought up regardless. After the
    /// fix the optional-subsystem step is skipped and the manager stays absent.
    #[test]
    fn with_i18n_false_skips_i18n_initialisation() {
        let _lock = app_init_test_lock();
        let _i18n = crate::i18n::global_i18n_test_lock();
        // Start from a known-uninitialised state.
        *crate::compat::lock(&crate::i18n::GLOBAL_I18N) = None;

        let mut app = App::with_config(AppConfig::default().with_i18n(false));
        app.init();

        assert!(
            crate::i18n::GLOBAL_I18N.lock().unwrap_or_else(|e| e.into_inner()).is_none(),
            "with_i18n(false) must leave the i18n manager uninitialised"
        );
    }

    /// N-S-70: the default (and `with_i18n(true)`) still brings i18n up, so the switch
    /// changed behaviour rather than disabling it outright.
    #[test]
    fn with_i18n_true_initialises_i18n() {
        let _lock = app_init_test_lock();
        let _i18n = crate::i18n::global_i18n_test_lock();
        *crate::compat::lock(&crate::i18n::GLOBAL_I18N) = None;

        let mut app = App::new();
        app.init();

        let count = crate::i18n::GLOBAL_I18N
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|manager| manager.translation_count())
            .unwrap_or(0);
        assert!(count > 0, "the default configuration must initialise the embedded catalogue");

        *crate::compat::lock(&crate::i18n::GLOBAL_I18N) = None;
    }

    /// N-S-70: `with_accessibility(false)` must be observable through the running app.
    #[test]
    fn accessibility_switch_is_recorded_by_init() {
        let _lock = app_init_test_lock();
        let _i18n = crate::i18n::global_i18n_test_lock();

        let mut off =
            App::with_config(AppConfig::default().with_accessibility(false).with_i18n(false));
        off.init();
        assert!(!off.accessibility_enabled(), "with_accessibility(false) must be recorded off");
        assert!(!accessibility_enabled(), "the process-wide flag must reflect the config");

        let mut on =
            App::with_config(AppConfig::default().with_accessibility(true).with_i18n(false));
        on.init();
        assert!(on.accessibility_enabled(), "the default/true switch must be recorded on");
        assert!(accessibility_enabled());
    }
}
