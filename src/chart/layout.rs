//! Chart layout helpers re-exported from `charts.rs` for module consistency.
//!
//! This module re-exports the cartesian layout computation and axis/legend
//! drawing functions that are defined in `charts.rs` so that the
//! `pub mod layout;` declaration in `chart/mod.rs` provides a stable path.

pub use crate::chart::charts::{
    compute_cartesian_layout, draw_cartesian_axes, draw_legend, draw_x_ticks, draw_y_ticks,
    truncate_legend_label, CartesianLayout,
};
