//! The control demo's application module, exposed as a library.
//!
//! # Why the demo has a library target at all
//!
//! `main` stays a stub (project convention), and the offscreen preview binary needs the *same*
//! window construction `run` uses. A second copy of that layout in the preview would be free to
//! drift, and a preview of a drifted layout proves nothing. Exposing the module is what lets
//! both callers share one `build_window`.

pub mod app;
