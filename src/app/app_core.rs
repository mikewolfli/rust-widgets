// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Core `App` type — lifecycle wrapper with configuration and callbacks.

use crate::core::ObjectId;
use crate::WidgetTriggerEvent;

use super::handle::{dispatch_trigger, WindowHandle};
use super::lifecycle::{AppLifecycle, AppLifecycleState};

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
    /// Returns `self` so calls can be chained. Adding a second callback keeps the first: the
    /// callbacks form a subscription set, so the later registration no longer overwrites the earlier
    /// one (APP-011).
    pub fn on_startup<F: FnOnce() + Send + 'static>(self, f: F) -> Self {
        with_startup(|callbacks| callbacks.push(Box::new(f)));
        self
    }

    /// Register a callback invoked once before the event loop exits.
    ///
    /// Returns `self` so calls can be chained. Like [`Self::on_startup`], registrations accumulate
    /// rather than replacing one another (APP-011).
    pub fn on_shutdown<F: FnOnce() + Send + 'static>(self, f: F) -> Self {
        with_shutdown(|callbacks| callbacks.push(Box::new(f)));
        self
    }

    /// Subscribe to application-level broadcast events (APP-012).
    ///
    /// The events are DPI changes, clipboard changes and drag/drop reports — facts about the
    /// application rather than about one control, so they do not travel the widget trigger queue.
    /// Returns `self` so calls can be chained; subscriptions are additive.
    pub fn on_application_event<F>(self, listener: F) -> Self
    where
        F: FnMut(&ApplicationEvent) + Send + 'static,
    {
        subscribe_application_events(listener);
        self
    }

    /// Bring the application's lifecycle in line with the phase the **platform** last reported
    /// (APP-010).
    ///
    /// # Why the app layer owns this translation
    ///
    /// Android, iOS and HarmonyOS report lifecycle transitions into
    /// [`platform::lifecycle`](crate::platform::lifecycle), which is a process-wide record rather
    /// than a callback channel. [`AppLifecycle`] tracks the same states but was driven only by the
    /// host calling `transition` by hand, so a real suspend never reached lifecycle listeners. This
    /// method is the join: `Active` maps to [`Foreground`](AppLifecycleState::Foreground),
    /// `Inactive` to [`Suspended`](AppLifecycleState::Suspended) and `Background` to
    /// [`Background`](AppLifecycleState::Background).
    ///
    /// Returns whether the application state changed. Calling it when the platform has reported
    /// nothing (`None`) leaves the state alone, so a desktop host that never reports a phase is
    /// unaffected.
    pub fn sync_platform_lifecycle(&mut self) -> bool {
        let Some(reported) = crate::platform::lifecycle::lifecycle_state() else {
            return false;
        };
        let next = match reported {
            crate::platform::MobileLifecycleState::Active => AppLifecycleState::Foreground,
            crate::platform::MobileLifecycleState::Inactive => AppLifecycleState::Suspended,
            crate::platform::MobileLifecycleState::Background => AppLifecycleState::Background,
        };
        let changed = self.lifecycle.state() != next;
        self.lifecycle.transition(next);
        changed
    }

    /// Emits a [`LifecycleEvent::MemoryWarning`] to every lifecycle listener (APP-010).
    ///
    /// The platform layer has no memory-warning notification to subscribe to, so the reachable
    /// channel is an explicit call: a host that receives the OS warning (or whose
    /// [`Self::check_memory_pressure`] crosses its threshold) reports it here. This is what makes
    /// the event reachable at all, instead of a variant nothing could ever emit.
    pub fn notify_memory_warning(&mut self) {
        self.lifecycle.emit_memory_warning();
    }

    /// Samples the platform's process memory utilization and emits a memory warning when it is at
    /// or above `threshold` (`0.0..=1.0`).
    ///
    /// Returns whether a warning was emitted. A backend that cannot measure memory answers `None`,
    /// and an unmeasurable value is **not** treated as zero pressure: nothing is emitted and
    /// `false` is returned, so "cannot tell" never looks like "plenty of memory".
    pub fn check_memory_pressure(&mut self, threshold: f32) -> bool {
        let Some(utilization) = crate::platform::get_platform().process_memory_utilization() else {
            return false;
        };
        if utilization >= threshold.clamp(0.0, 1.0) {
            self.lifecycle.emit_memory_warning();
            return true;
        }
        false
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

        // Menu effect policy runs one hardware probe (GPU / memory / battery). Doing it
        // here, at startup, keeps the first menu draw from paying for it. The policy is
        // on by default, so menus adapt to this host; a host that wants the pre-adaptive
        // rendering calls `menu_config::set_enabled(false)` (see `MenuConfig`).
        #[cfg(feature = "advanced-widgets")]
        crate::menu_config::init();

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

        // Enumerating installed fonts can take hundreds of milliseconds. Start it on a background
        // thread now so the first font chooser does not pay for it on the UI thread.
        crate::platform::fonts::prewarm_system_fonts();

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
        crate::platform::fonts::prewarm_system_fonts();
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

// ── Startup / shutdown callback sets (APP-011) ───────────────
// Using OnceLock + Mutex instead of thread_local! so callbacks
// are available across threads (e.g. in run_async).
//
// The slots are `Vec`s: a single `Option` made a second registration silently replace the first,
// so two components could not both observe startup. The order is registration order, and each
// callback fires once.

use crate::compat::Mutex;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::OnceLock;

type StartupCallback = Box<dyn FnOnce() + Send>;
type ShutdownCallback = Box<dyn FnOnce() + Send>;

static STARTUP: OnceLock<Mutex<Vec<StartupCallback>>> = OnceLock::new();
static SHUTDOWN: OnceLock<Mutex<Vec<ShutdownCallback>>> = OnceLock::new();

/// An application-level event that does not belong to one control (APP-012).
///
/// # Why these are not widget triggers
///
/// A DPI change, a clipboard write and a completed drop are facts about the **application**: they
/// have no single target widget, and a host may need to react even when no control is focused.
/// Reporting them through [`WidgetTriggerEvent`] would invent a widget id for a fact that has none,
/// so they travel this channel instead.
#[derive(Debug, Clone, PartialEq)]
pub enum ApplicationEvent {
    /// The display scale factor changed; `scale` is the new factor.
    DpiChanged {
        /// The new scale factor, as the platform reports it.
        scale: f32,
    },
    /// The clipboard's text was replaced through the library's own entry point.
    ClipboardChanged {
        /// The text that was written.
        text: crate::compat::String,
    },
    /// A drag operation was accepted by the backend.
    DragStarted {
        /// The widget the drag started from.
        source_widget_id: ObjectId,
        /// The payload's MIME type.
        mime: crate::compat::String,
    },
    /// A drop was completed and dequeued.
    DropCompleted {
        /// The completed drop event.
        event: crate::platform::DropEvent,
    },
}

/// Application-level listeners, process-wide because the facts are.
static APPLICATION_LISTENERS: OnceLock<Mutex<Vec<Box<dyn FnMut(&ApplicationEvent) + Send>>>> =
    OnceLock::new();

/// Subscribes `listener` to application-level events, returning the new subscription count.
///
/// `listener` is invoked for every event published after this call. A listener that re-subscribes
/// during dispatch is honoured on the next event: dispatch takes the current set out of the lock
/// before invoking, so the mutex is never held across a callback.
pub fn subscribe_application_events<F>(listener: F) -> usize
where
    F: FnMut(&ApplicationEvent) + Send + 'static,
{
    let lock = APPLICATION_LISTENERS.get_or_init(|| Mutex::new(Vec::new()));
    let mut listeners = lock.lock().unwrap_or_else(|error| error.into_inner());
    listeners.push(Box::new(listener));
    listeners.len()
}

/// Removes every application-level listener. For tests and for a host shutting its event loop down.
pub fn clear_application_event_listeners() {
    let lock = APPLICATION_LISTENERS.get_or_init(|| Mutex::new(Vec::new()));
    lock.lock().unwrap_or_else(|error| error.into_inner()).clear();
}

/// Publishes `event` to every current application-level listener, returning how many ran.
///
/// The listener set is taken out of the lock before dispatch, so a listener may subscribe (or
/// clear) without deadlocking. Listeners registered during dispatch are appended after the ones
/// that already ran, so a later event reaches every subscriber exactly once.
pub fn publish_application_event(event: &ApplicationEvent) -> usize {
    let lock = APPLICATION_LISTENERS.get_or_init(|| Mutex::new(Vec::new()));
    let mut listeners = {
        let mut guard = lock.lock().unwrap_or_else(|error| error.into_inner());
        core::mem::take(&mut *guard)
    };
    let count = listeners.len();
    for listener in listeners.iter_mut() {
        listener(event);
    }
    let mut guard = lock.lock().unwrap_or_else(|error| error.into_inner());
    let registered_during_dispatch = core::mem::take(&mut *guard);
    listeners.extend(registered_during_dispatch);
    *guard = listeners;
    count
}

/// The last DPI scale factor observed by [`publish_dpi_change_if_needed`].
static LAST_DPI_SCALE: OnceLock<Mutex<Option<f32>>> = OnceLock::new();

/// Compares the platform's current DPI scale factor with the last one and publishes a
/// [`ApplicationEvent::DpiChanged`] when it changed (APP-012).
///
/// The first call records the baseline and publishes nothing: there is no previous value to have
/// changed from, and reporting an initial "change" would make every application receive one event
/// at startup that no user action produced. Returns the new scale when an event was published.
pub fn publish_dpi_change_if_needed() -> Option<f32> {
    let scale = crate::dpi_scale_factor();
    let lock = LAST_DPI_SCALE.get_or_init(|| Mutex::new(None));
    let changed = {
        let mut last = lock.lock().unwrap_or_else(|error| error.into_inner());
        match *last {
            Some(previous) if (previous - scale).abs() > f32::EPSILON => {
                *last = Some(scale);
                true
            }
            Some(_) => false,
            None => {
                *last = Some(scale);
                false
            }
        }
    };
    if changed {
        publish_application_event(&ApplicationEvent::DpiChanged { scale });
        Some(scale)
    } else {
        None
    }
}

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
    F: FnOnce(&mut Vec<StartupCallback>) -> R,
{
    let lock = STARTUP.get_or_init(|| Mutex::new(Vec::new()));
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

fn with_shutdown<F, R>(f: F) -> R
where
    F: FnOnce(&mut Vec<ShutdownCallback>) -> R,
{
    let lock = SHUTDOWN.get_or_init(|| Mutex::new(Vec::new()));
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

/// Runs every registered startup callback, releasing the lock before invoking any of them.
///
/// The callbacks are taken out of the set and invoked **after** the guard is dropped. A startup
/// callback that re-registers a startup callback must not re-lock the same non-reentrant `Mutex`
/// while it is already held on this thread — that was a same-thread deadlock. A callback
/// re-registered here takes effect on the next [`fire_startup`] (i.e. the next `init`).
fn fire_startup() {
    let callbacks = with_startup(core::mem::take);
    for callback in callbacks {
        callback();
    }
}

/// Runs every registered shutdown callback, releasing the lock before invoking any of them.
///
/// Mirrors [`fire_startup`]: a shutdown callback that re-registers a shutdown callback takes
/// effect on the next [`fire_shutdown`] (i.e. the next `run`/`run_async`), and never deadlocks.
fn fire_shutdown() {
    let callbacks = with_shutdown(core::mem::take);
    for callback in callbacks {
        callback();
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
    use crate::app::lifecycle::LifecycleEvent;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    /// A startup callback that re-registers a startup callback must not deadlock.
    ///
    /// The old `init` invoked the callback while holding the global `STARTUP` mutex, so the
    /// re-registering `on_startup` re-locked the same non-reentrant mutex on the same thread and
    /// hung. `fire_startup` takes the slot and releases the lock before invoking, so the
    /// re-registered callback simply takes effect on the next startup.
    ///
    /// # Why this holds `app_init_test_lock()`
    ///
    /// `STARTUP` is one process-wide slot and `App::init` fires it, so a test that calls
    /// `App::init` concurrently *consumes this test's callback* and the first assertion below fails
    /// — measured before this lock was added: 1 failure in 25 runs of `app::app_core`, which is the
    /// "既有抖动" this file's race used to show in the full suite. The two tests must exclude each
    /// other because they share the slot, not because either one is wrong on its own.
    #[test]
    fn a_startup_callback_can_reenter_registration() {
        let _lock = app_init_test_lock();
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
    ///
    /// Holds the same lock as the startup reentrancy test: `SHUTDOWN` is one process-wide slot that
    /// `App::init`/`App::shutdown` also fire, so the two tests must not run concurrently.
    #[test]
    fn a_shutdown_callback_can_reenter_registration() {
        let _lock = app_init_test_lock();
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

    /// APP-011: a second startup subscription is added rather than replacing the first.
    #[test]
    fn every_startup_subscription_runs() {
        let _lock = app_init_test_lock();
        let first = Arc::new(AtomicU32::new(0));
        let second = Arc::new(AtomicU32::new(0));
        let f = Arc::clone(&first);
        let s = Arc::clone(&second);
        let _app = App::new()
            .on_startup(move || {
                f.fetch_add(1, Ordering::SeqCst);
            })
            .on_startup(move || {
                s.fetch_add(1, Ordering::SeqCst);
            });

        fire_startup();
        assert_eq!(first.load(Ordering::SeqCst), 1, "the first subscription must run");
        assert_eq!(second.load(Ordering::SeqCst), 1, "the second must not replace the first");
    }

    /// APP-010: the platform's reported phase drives the application lifecycle, and the memory
    /// warning has a reachable emitter.
    #[test]
    fn platform_lifecycle_drives_the_app_state_and_memory_warnings_are_reachable() {
        use crate::platform::MobileLifecycleState;
        let _guard = crate::compat::lock(&crate::platform::lifecycle::TEST_LOCK);
        let mut app = App::new();
        app.lifecycle_mut().transition(AppLifecycleState::Foreground);

        assert!(crate::platform::lifecycle::set_state(MobileLifecycleState::Background));
        assert!(app.sync_platform_lifecycle(), "a changed platform phase must transition the app");
        assert_eq!(app.lifecycle().state(), AppLifecycleState::Background);
        assert!(!app.sync_platform_lifecycle(), "a repeated phase is not a transition");

        let warned = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&warned);
        app.lifecycle_mut().add_listener(Box::new(move |event| {
            if event == LifecycleEvent::MemoryWarning {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }));
        app.notify_memory_warning();
        assert_eq!(
            warned.load(Ordering::SeqCst),
            1,
            "the memory-warning channel must be reachable"
        );

        // Restore the process-wide phase so another test does not inherit this one's background.
        assert!(crate::platform::lifecycle::set_state(MobileLifecycleState::Active));
    }

    /// APP-012: application-level facts reach every subscriber without a widget id.
    #[test]
    fn application_events_reach_every_subscriber() {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|error| error.into_inner());
        clear_application_event_listeners();

        let seen = Arc::new(AtomicU32::new(0));
        let first = Arc::clone(&seen);
        subscribe_application_events(move |_event| {
            first.fetch_add(1, Ordering::SeqCst);
        });
        let second = Arc::clone(&seen);
        subscribe_application_events(move |_event| {
            second.fetch_add(1, Ordering::SeqCst);
        });

        let published = publish_application_event(&ApplicationEvent::ClipboardChanged {
            text: crate::compat::String::from("copied"),
        });
        assert_eq!(published, 2);
        assert_eq!(seen.load(Ordering::SeqCst), 2, "both subscriptions receive one event each");

        assert!(publish_application_event(&ApplicationEvent::DpiChanged { scale: 2.0 }) == 2);
        assert_eq!(seen.load(Ordering::SeqCst), 4);

        // The DPI helper reports a change only after a baseline exists, so a second immediate read
        // publishes nothing. (The first call may publish if another test moved the platform value.)
        let _ = publish_dpi_change_if_needed();
        assert!(
            publish_dpi_change_if_needed().is_none(),
            "an unchanged DPI scale must not fabricate a change event"
        );

        clear_application_event_listeners();
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
    /// fix the optional-subsystem step is skipped.
    ///
    /// The assertion is on a **thread-local call counter**, not on `GLOBAL_I18N.is_none()`.
    /// `translate()` lazily populates the global slot by design, so any test that renders a
    /// translated label in parallel puts a manager there — which made the old global assertion
    /// fail intermittently for a reason that had nothing to do with this switch. The counter sees
    /// only the calls this test thread made.
    #[test]
    fn with_i18n_false_skips_i18n_initialisation() {
        let _lock = app_init_test_lock();
        let _i18n = crate::i18n::global_i18n_test_lock();
        let before = crate::i18n::init_call_count();

        let mut app = App::with_config(AppConfig::default().with_i18n(false));
        app.init();

        assert_eq!(
            crate::i18n::init_call_count(),
            before,
            "with_i18n(false) must not call i18n::init()"
        );
    }

    /// N-S-70: the default (and `with_i18n(true)`) still brings i18n up, so the switch
    /// changed behaviour rather than disabling it outright.
    #[test]
    fn with_i18n_true_initialises_i18n() {
        let _lock = app_init_test_lock();
        let _i18n = crate::i18n::global_i18n_test_lock();
        let before = crate::i18n::init_call_count();
        *crate::compat::lock(&crate::i18n::GLOBAL_I18N) = None;

        let mut app = App::new();
        app.init();

        assert!(
            crate::i18n::init_call_count() > before,
            "the default configuration must call i18n::init()"
        );

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
