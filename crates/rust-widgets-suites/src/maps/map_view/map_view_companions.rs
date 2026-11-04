// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Map companion controls (COMP-13): tile loading, zoom strip, scale bar and compass.
//!
//! Each control has its own file under this module's folder; this file only declares and
//! re-exports them, so the historical `map_view_companions::TileLoader`-style paths keep
//! resolving while the family is one folder.
//!
//! # `enabled`
//!
//! `tile_loaded` / `tile_failed` report an **asynchronous platform fact**, and `redraw` /
//! `zoom_requested` are chrome plumbing; all four are **deliberately not gated** by `enabled`. A
//! disabled map still shows the tiles that arrived, and the host still owns the request/answer
//! pair it started.

mod common;
mod compass;
mod scale_bar;
mod tile_loader;
mod zoom_control;

pub use compass::Compass;
pub use scale_bar::{ScaleBar, ScaleBarStyle, ScaleUnits};
pub use tile_loader::{
    TileKey, TileLoadError, TileLoader, TileRequest, TileViewport, MAX_TILE_ZOOM,
};
pub use zoom_control::{ZoomButton, ZoomControl};
