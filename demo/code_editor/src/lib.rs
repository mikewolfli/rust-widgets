//! The code-editor demo's application modules, exposed as a library.
//!
//! # Why the demo has a library target at all
//!
//! `main` stays a stub (project convention), and the offscreen preview binary needs the *same*
//! window construction `run` uses. A second copy of that composition in the preview would be
//! free to drift — the split layout, the tree-before-editor mount order, the appearance applied
//! before any control exists — and a preview of a drifted layout proves nothing. Exposing the
//! modules is what lets both callers share one [`app::render_offscreen`] path.

pub mod app;
pub mod commands;
