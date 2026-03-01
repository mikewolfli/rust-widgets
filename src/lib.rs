//! rust_widgets - cross-platform native GUI architecture in pure Rust.

pub mod core;
pub mod object;
pub mod event;
pub mod signal;
pub mod widget;
pub mod layout;
pub mod render_engine;
#[cfg(not(feature = "embedded"))]
pub mod xml;
#[cfg(not(feature = "embedded"))]
pub mod i18n;
pub mod platform;
#[cfg(not(feature = "embedded"))]
pub mod theme;
pub mod style;
#[cfg(not(feature = "embedded"))]
pub mod bindings;

#[cfg(feature = "print")]
pub mod print;

#[cfg(feature = "pdf")]
pub mod pdf;

#[cfg(feature = "chart")]
pub mod chart;

/// Initialize global platform and i18n subsystems.
pub fn init() {
    render_engine::default_render_engine().init();
    init_i18n_runtime();
}

/// Run platform main event loop.
pub fn run() {
    render_engine::default_render_engine().run();
}

/// Request platform event loop shutdown.
pub fn quit() {
    render_engine::default_render_engine().quit();
}

#[cfg(not(feature = "embedded"))]
fn init_i18n_runtime() {
    i18n::init();
}

#[cfg(feature = "embedded")]
fn init_i18n_runtime() {}
