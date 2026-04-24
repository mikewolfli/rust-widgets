//! Widget-specific rendering controls (reserved for future pipeline integration).
//!
//! Contains 14 files across `basic/` (button, label, checkbox), `input/` (textbox, slider, spinbox),
//! and `special/` (progress bar, scrollbar) rendering implementations.
//! All are `#[allow(dead_code)]` pending pipeline activation — this is intentional
//! architectural scaffolding, not dead code to be removed.
pub mod basic;
pub mod input;
pub mod special;
