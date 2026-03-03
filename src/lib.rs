//! rust_widgets - cross-platform native GUI architecture in pure Rust.

/// Action/command system.
pub mod action;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
/// C ABI bindings for desktop runtime.
pub mod bindings;
/// Clipboard helpers.
pub mod clipboard;
/// Control backend abstraction for native/custom control implementations.
pub mod control_backend;
/// Core types and shared contracts.
pub mod core;
/// Event types and dispatch helpers.
pub mod event;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
/// Internationalization module for desktop runtime.
pub mod i18n;
/// Layout managers.
pub mod layout;
/// Object tree and object utilities.
pub mod object;
/// Platform abstraction and backend adapters.
pub mod platform;
/// Rendering traits and primitives.
pub mod render;
/// Runtime render-engine abstraction.
pub mod render_engine;
/// Signal-slot utilities.
pub mod signal;
/// Style system primitives.
pub mod style;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
/// Theme management for desktop runtime.
pub mod theme;
#[cfg(feature = "gpu-wgpu")]
/// Optional WGPU GPU acceleration backend.
pub mod wgpu_backend;
/// Widget definitions and widget helpers.
pub mod widget;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
/// XML utilities for desktop runtime.
pub mod xml;

#[cfg(feature = "print")]
/// Print and preview support.
pub mod print;

#[cfg(feature = "pdf")]
/// PDF rendering/export support.
pub mod pdf;

#[cfg(feature = "chart")]
/// Charting primitives.
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
        eprintln!(
            "[rust_widgets.runtime] stage={stage} profile={} backend={} route={}",
            runtime_profile_name(),
            platform::get_platform().backend_name(),
            runtime_route_name()
        );
    }
}

#[cfg(not(feature = "embedded"))]
fn runtime_profile_name() -> &'static str {
    "full"
}

#[cfg(feature = "embedded")]
fn runtime_profile_name() -> &'static str {
    "embedded"
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
