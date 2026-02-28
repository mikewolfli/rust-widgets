//! rust_widgets - cross-platform native GUI architecture in pure Rust.

pub mod core;
pub mod object;
pub mod event;
pub mod signal;
pub mod widget;
pub mod layout;
pub mod xml;
pub mod i18n;
pub mod platform;
pub mod theme;
pub mod style;
pub mod bindings;

#[cfg(feature = "print")]
pub mod print;

#[cfg(feature = "pdf")]
pub mod pdf;

#[cfg(feature = "chart")]
pub mod chart;

/// Initialize the GUI library
pub fn init() {
    platform::init();
    i18n::init();
}

/// Run the main event loop
pub fn run() {
    platform::run();
}

/// Quit the application
pub fn quit() {
    platform::quit();
}
