//! rust_widgets - cross-platform native GUI architecture in pure Rust.

pub mod core;
pub mod object;
pub mod event;
pub mod signal;
pub mod widget;
pub mod layout;
pub mod render_engine;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
pub mod xml;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
pub mod i18n;
pub mod platform;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
pub mod theme;
pub mod style;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
pub mod bindings;

#[cfg(feature = "print")]
pub mod print;

#[cfg(feature = "pdf")]
pub mod pdf;

#[cfg(feature = "chart")]
pub mod chart;

/// Initialize global platform and i18n subsystems.
pub fn init() {
    trace_runtime_route("init");
    init_runtime_backend();
    init_i18n_runtime();
}

/// Run platform main event loop.
pub fn run() {
    trace_runtime_route("run");
    run_runtime_backend();
}

/// Request platform event loop shutdown.
pub fn quit() {
    trace_runtime_route("quit");
    quit_runtime_backend();
}

fn trace_runtime_route(stage: &str) {
    if std::env::var("RUST_WIDGETS_TRACE_RUNTIME").ok().as_deref() == Some("1") {
        eprintln!("[rust_widgets] stage={stage} route={}", runtime_route_name());
    }
}

#[cfg(not(feature = "embedded"))]
fn runtime_route_name() -> &'static str {
    "native-platform"
}

#[cfg(feature = "embedded")]
fn runtime_route_name() -> &'static str {
    "embedded-render-engine"
}

#[cfg(not(feature = "embedded"))]
fn init_runtime_backend() {
    platform::init();
}

#[cfg(feature = "embedded")]
fn init_runtime_backend() {
    render_engine::default_render_engine().init();
}

#[cfg(not(feature = "embedded"))]
fn run_runtime_backend() {
    platform::run();
}

#[cfg(feature = "embedded")]
fn run_runtime_backend() {
    render_engine::default_render_engine().run();
}

#[cfg(not(feature = "embedded"))]
fn quit_runtime_backend() {
    platform::quit();
}

#[cfg(feature = "embedded")]
fn quit_runtime_backend() {
    render_engine::default_render_engine().quit();
}

#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
fn init_i18n_runtime() {
    i18n::init();
}

#[cfg(any(feature = "embedded", not(feature = "desktop-runtime")))]
fn init_i18n_runtime() {}
