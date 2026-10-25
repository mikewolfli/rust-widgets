//! The finance demo's application module, exposed as a library.
//!
//! `main` stays a stub, and the offscreen preview binary needs the same `build_screen` the window
//! shows — a second copy of the screen layout in the preview would be free to drift, and a preview
//! of a drifted layout proves nothing.

pub mod app;
