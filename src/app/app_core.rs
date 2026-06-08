//! Core `App` type — lifecycle wrapper with configuration and callbacks.

use crate::core::ObjectId;
use crate::WidgetTriggerEvent;

use super::handle::{dispatch_trigger, WindowHandle};

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
///     .with_organization("Acme Corp");
/// ```
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Human-readable application name (used for window titles, etc.).
    pub app_name: String,
    /// Organization or vendor name.
    pub organization: String,
    /// Whether to initialise the i18n subsystem (default: `true`).
    pub enable_i18n: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { app_name: String::new(), organization: String::new(), enable_i18n: true }
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

    /// Enable or disable i18n initialisation.
    pub fn with_i18n(mut self, enable: bool) -> Self {
        self.enable_i18n = enable;
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
/// ```ignore
/// use rust_widgets::app::App;
///
/// let app = App::new();
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
/// ```ignore
/// use rust_widgets::app::{App, AppConfig};
///
/// let app = App::with_config(
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
/// // … create widgets …
/// app.run();
/// ```
pub struct App {
    config: AppConfig,
}

impl App {
    /// Create a new application handle with default configuration.
    pub fn new() -> Self {
        Self { config: AppConfig::default() }
    }

    /// Create a new application handle with a custom configuration.
    pub fn with_config(config: AppConfig) -> Self {
        Self { config }
    }

    /// Return a reference to the current configuration.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Register a callback invoked once after [`init`](App::init) completes.
    ///
    /// Returns `self` so calls can be chained.
    pub fn on_startup<F: FnOnce() + 'static>(self, f: F) -> Self {
        STARTUP.with(|s| {
            *s.borrow_mut() = Some(Box::new(f));
        });
        self
    }

    /// Register a callback invoked once before the event loop exits.
    ///
    /// Returns `self` so calls can be chained.
    pub fn on_shutdown<F: FnOnce() + 'static>(self, f: F) -> Self {
        SHUTDOWN.with(|s| {
            *s.borrow_mut() = Some(Box::new(f));
        });
        self
    }

    /// Initialize global platform and (optionally) i18n subsystems.
    ///
    /// Call this once before creating windows or running the loop.
    pub fn init(&self) {
        trace_runtime_route("app::init");
        crate::init();

        // Fire the startup callback after everything is initialised.
        STARTUP.with(|s| {
            if let Some(cb) = s.borrow_mut().take() {
                cb();
            }
        });
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

        SHUTDOWN.with(|s| {
            if let Some(cb) = s.borrow_mut().take() {
                cb();
            }
        });
    }

    /// Request the event loop to shut down.
    pub fn quit(&self) {
        crate::quit();
    }

    /// Create a top-level window and return a type-safe handle.
    pub fn new_window(&self, title: &str, x: i32, y: i32, w: u32, h: u32) -> WindowHandle {
        WindowHandle::from_raw(crate::create_window(title, x, y, w, h))
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

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

// ── Startup / shutdown one-shot callbacks ─────────────────────

use std::cell::RefCell;

thread_local! {
    static STARTUP: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
    static SHUTDOWN: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

fn trace_runtime_route(stage: &str) {
    if std::env::var("RUST_WIDGETS_TRACE_RUNTIME").ok().as_deref() == Some("1") {
        eprintln!("[rust_widgets.app] stage={}", stage);
    }
}
