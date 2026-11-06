// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//!
//! Tile addressing, the host-facing load state and the `TileLoader` control.

use super::common::companion_palette;
use rust_widgets::compat::HashMap;
use rust_widgets::core::{Color, HorizontalAlignment, Rect};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::Signal1;
use rust_widgets::widget::capability::coercion::{expect_string, expect_u32};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::effective_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

/// The highest zoom level a [`TileLoader`] accepts.
pub const MAX_TILE_ZOOM: u8 = 22;

/// Addresses one tile in the slippy-map pyramid.
///
/// Derives `Ord`/`Hash` because the crate's map alias is a `BTreeMap` in frugal
/// builds and a `HashMap` elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileKey {
    /// Zoom level the tile belongs to.
    pub zoom: u8,
    /// Column, counted from the west edge of the world.
    pub x: u32,
    /// Row, counted from the north edge of the world.
    pub y: u32,
}

impl TileKey {
    /// Creates a tile address.
    pub fn new(zoom: u8, x: u32, y: u32) -> Self {
        Self { zoom, x, y }
    }

    /// How many tiles fit on one axis at `zoom`; `0` when the level is unaddressable.
    pub fn tiles_per_axis(zoom: u8) -> u32 {
        1u32.checked_shl(u32::from(zoom)).unwrap_or(0)
    }
}

/// Why a tile request could not be produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileLoadError {
    /// The zoom level lies outside the loader's configured range.
    ZoomOutOfRange,
    /// The column or row does not exist at this zoom.
    TileOutOfWorld,
}

/// A tile the host should fetch, or reuse from the cache.
///
/// The loader never performs the transfer: the host resolves the request and
/// reports back through [`TileLoader::deliver_tile`] or [`TileLoader::reject_tile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileRequest {
    /// The tile's address.
    pub key: TileKey,
    /// The URL built from the loader's `url_template`.
    pub url: String,
    /// Whether this tile was already requested.
    pub cached: bool,
}

/// A rectangular window onto the tile grid, in tile units at one zoom level.
///
/// `x`/`y` are fractional tile coordinates of the top-left corner; keeping the
/// unit explicit means `prefetch` never has to guess what a `Rect` measures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileViewport {
    /// Zoom level the window is expressed in.
    pub zoom: u8,
    /// Fractional tile column of the west edge.
    pub x: f64,
    /// Fractional tile row of the north edge.
    pub y: f64,
    /// Viewport width in logical pixels.
    pub width: u32,
    /// Viewport height in logical pixels.
    pub height: u32,
}

impl TileViewport {
    /// Creates a tile window.
    pub fn new(zoom: u8, x: f64, y: f64, width: u32, height: u32) -> Self {
        Self { zoom, x, y, width, height }
    }
}

/// What the loader remembers about one addressed tile.
struct TileEntry {
    /// Pixels the host supplied, or `None` while the fetch is outstanding.
    payload: Option<Vec<u8>>,
}

/// Builds tile addresses and remembers which of them the host has fetched.
///
/// `load_tile` returns a [`TileRequest`] and `prefetch` a batch of them; only the
/// host's answer makes a tile available. The cache is bounded by `cache_limit`
/// entries (not bytes) and evicts oldest-first.
pub struct TileLoader {
    base: BaseWidget,
    url_template: String,
    min_zoom: u8,
    max_zoom: u8,
    tile_size: u32,
    cache_limit: usize,
    cache: HashMap<TileKey, TileEntry>,
    /// Insertion order, used for oldest-first eviction.
    order: alloc::collections::VecDeque<TileKey>,
    /// Revision of the cache contents, so the status label moves only when the counts can (SP-120).
    cache_revision: u64,
    /// The cached status label and the key it was built for: revision and zoom range.
    pub(super) status_cache: Option<(u64, u8, u8, String)>,
    /// Emitted when the host supplies pixels for a tile.
    pub tile_loaded: Signal1<TileKey>,
    /// Emitted when the host reports that a tile could not be fetched.
    pub tile_failed: Signal1<TileKey>,
}

impl TileLoader {
    /// Creates a loader with the default template and a `0 ..= 19` zoom range.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TileLoader, geometry, "TileLoader"),
            url_template: "tiles/{z}/{x}/{y}.png".to_string(),
            min_zoom: 0,
            max_zoom: 19,
            tile_size: 256,
            cache_limit: 256,
            cache: HashMap::new(),
            order: alloc::collections::VecDeque::new(),
            cache_revision: 0,
            status_cache: None,
            tile_loaded: Signal1::new(),
            tile_failed: Signal1::new(),
        }
    }

    /// Returns the URL template.
    pub fn url_template(&self) -> &str {
        &self.url_template
    }

    /// Sets the URL template; an empty template is ignored.
    pub fn set_url_template(&mut self, template: impl Into<String>) {
        let template = template.into();
        if template.trim().is_empty() {
            return;
        }
        self.url_template = template;
        self.base.request_redraw();
    }

    /// Returns the lowest accepted zoom level.
    pub fn min_zoom(&self) -> u8 {
        self.min_zoom
    }

    /// Returns the highest accepted zoom level.
    pub fn max_zoom(&self) -> u8 {
        self.max_zoom
    }

    /// Sets the accepted zoom range, clamped to `0 ..= MAX_TILE_ZOOM`.
    ///
    /// A range never inverts: `max_zoom < min_zoom` raises the maximum to the
    /// minimum instead of producing an empty range.
    pub fn set_zoom_range(&mut self, min_zoom: u8, max_zoom: u8) {
        let min = min_zoom.min(MAX_TILE_ZOOM);
        let max = max_zoom.clamp(min, MAX_TILE_ZOOM);
        if self.min_zoom == min && self.max_zoom == max {
            return;
        }
        self.min_zoom = min;
        self.max_zoom = max;
        self.base.request_redraw();
    }

    /// Returns the tile edge length in logical pixels.
    pub fn tile_size(&self) -> u32 {
        self.tile_size
    }

    /// Sets the tile edge length, clamped to `16 ..= 1024`.
    pub fn set_tile_size(&mut self, tile_size: u32) {
        let size = tile_size.clamp(16, 1024);
        if self.tile_size == size {
            return;
        }
        self.tile_size = size;
        self.base.request_redraw();
    }

    /// Returns the maximum number of remembered tiles.
    pub fn cache_limit(&self) -> usize {
        self.cache_limit
    }

    /// Sets the cache ceiling, clamped to `1 ..= 65536`, and trims to it.
    pub fn set_cache_limit(&mut self, cache_limit: usize) {
        self.cache_limit = cache_limit.clamp(1, 65536);
        self.evict_over_limit();
        self.base.request_redraw();
    }

    /// Expands `{z}`, `{x}` and `{y}` in the template for `key`.
    pub fn tile_url(&self, key: TileKey) -> String {
        self.url_template
            .replace("{z}", &key.zoom.to_string())
            .replace("{x}", &key.x.to_string())
            .replace("{y}", &key.y.to_string())
    }

    /// Requests one tile, returning the address and URL to fetch.
    ///
    /// No I/O happens here. A cache miss is recorded as pending so a second call
    /// reports `cached: true` and the host does not issue a duplicate transfer.
    pub fn load_tile(&mut self, zoom: u8, x: u32, y: u32) -> Result<TileRequest, TileLoadError> {
        if zoom < self.min_zoom || zoom > self.max_zoom {
            return Err(TileLoadError::ZoomOutOfRange);
        }
        let per_axis = TileKey::tiles_per_axis(zoom);
        if per_axis == 0 || x >= per_axis || y >= per_axis {
            return Err(TileLoadError::TileOutOfWorld);
        }
        let key = TileKey::new(zoom, x, y);
        let cached = self.cache.contains_key(&key);
        let url = self.tile_url(key);
        if cached {
            // LRU, not insertion order: a tile the camera keeps asking for is the one to keep when
            // the cache is full, and a hot level used to be evicted by a colder one registered
            // later.
            if let Some(position) = self.order.iter().position(|queued| *queued == key) {
                self.order.remove(position);
                self.order.push_back(key);
            }
        } else {
            self.cache.insert(key, TileEntry { payload: None });
            self.order.push_back(key);
            self.evict_over_limit();
            self.cache_revision = self.cache_revision.wrapping_add(1);
            self.base.request_redraw();
        }
        Ok(TileRequest { key, url, cached })
    }

    /// Requests every tile the viewport touches, row-major.
    ///
    /// The batch is capped at `cache_limit` entries so prefetching cannot evict
    /// the tiles it just asked for. Tiles outside the world are skipped, and a
    /// non-finite or degenerate viewport requests nothing.
    pub fn prefetch(&mut self, viewport: TileViewport) -> Vec<TileRequest> {
        if !viewport.x.is_finite() || !viewport.y.is_finite() {
            return Vec::new();
        }
        if viewport.width == 0 || viewport.height == 0 {
            return Vec::new();
        }
        let per_axis = TileKey::tiles_per_axis(viewport.zoom) as i64;
        if per_axis <= 0 {
            return Vec::new();
        }
        let tile = self.tile_size.max(1) as f64;
        let first_col = viewport.x.floor();
        let first_row = viewport.y.floor();
        let cols = (viewport.width as f64 / tile).ceil().max(1.0) as i64 + 1;
        let rows = (viewport.height as f64 / tile).ceil().max(1.0) as i64 + 1;
        let limit = self.cache_limit.max(1);
        let mut requests = Vec::new();
        for row in 0..rows {
            for col in 0..cols {
                if requests.len() >= limit {
                    return requests;
                }
                let x = first_col as i64 + col;
                let y = first_row as i64 + row;
                if x < 0 || y < 0 || x >= per_axis || y >= per_axis {
                    continue;
                }
                if let Ok(request) = self.load_tile(viewport.zoom, x as u32, y as u32) {
                    requests.push(request);
                }
            }
        }
        requests
    }

    /// Records the pixels a host fetched for `key` and reports success.
    ///
    /// Returns `false` when the address was never requested, so a stray answer is
    /// refused instead of silently growing the cache.
    pub fn deliver_tile(&mut self, key: TileKey, payload: Vec<u8>) -> bool {
        let Some(entry) = self.cache.get_mut(&key) else {
            return false;
        };
        entry.payload = Some(payload);
        self.cache_revision = self.cache_revision.wrapping_add(1);
        self.tile_loaded.emit(key);
        self.base.request_redraw();
        true
    }

    /// Drops a requested tile and reports that the host could not fetch it.
    pub fn reject_tile(&mut self, key: TileKey) -> bool {
        if self.cache.remove(&key).is_none() {
            return false;
        }
        self.order.retain(|queued| *queued != key);
        self.cache_revision = self.cache_revision.wrapping_add(1);
        self.tile_failed.emit(key);
        self.base.request_redraw();
        true
    }

    /// Forgets every addressed tile; the host keeps whatever bytes it holds.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.order.clear();
        self.cache_revision = self.cache_revision.wrapping_add(1);
        self.base.request_redraw();
    }

    /// Returns how many tiles the loader remembers, pending or delivered.
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// Whether `key` has pixels supplied by the host.
    pub fn is_cached(&self, key: TileKey) -> bool {
        self.cache.get(&key).is_some_and(|entry| entry.payload.is_some())
    }

    /// The pixels the host supplied for `key`, or `None` while the tile is outstanding.
    ///
    /// The bytes are the host's own decoded RGBA8 tile, `tile_size`×`tile_size`; the map blits
    /// them by reference, so rendering a delivered tile copies nothing. Eviction drops the
    /// payload with the entry, so the memory a delivered tile holds is released with it.
    pub fn tile_pixels(&self, key: TileKey) -> Option<&[u8]> {
        self.cache.get(&key).and_then(|entry| entry.payload.as_deref())
    }

    fn cached_count(&self) -> usize {
        self.cache.values().filter(|entry| entry.payload.is_some()).count()
    }

    /// Trims the cache from the front of the access order until it fits.
    ///
    /// The order is least-recently-used first (see `load_tile`), so eviction drops the tile the
    /// camera has gone longest without asking for — together with its payload, which is what keeps
    /// a long session's memory bounded (SP-P60).
    fn evict_over_limit(&mut self) {
        while self.cache.len() > self.cache_limit {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.cache.remove(&oldest);
            self.cache_revision = self.cache_revision.wrapping_add(1);
        }
    }

    /// The status label, rebuilt only when the cache revision or the zoom range moves (SP-120).
    fn status_text(&mut self) -> &str {
        let key = (self.cache_revision, self.min_zoom, self.max_zoom);
        let stale = self
            .status_cache
            .as_ref()
            .is_none_or(|(revision, min, max, _)| (*revision, *min, *max) != key);
        if stale {
            let status = format!(
                "Tiles {}/{}  z{}-{}",
                self.cached_count(),
                self.cache.len(),
                self.min_zoom,
                self.max_zoom
            );
            self.status_cache = Some((key.0, key.1, key.2, status));
        }
        self.status_cache.as_ref().map(|(_, _, _, text)| text.as_str()).unwrap_or("")
    }
}

impl Widget for TileLoader {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("tile_loader")
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> rust_widgets::core::Size {
        rust_widgets::core::Size::new(160, 24)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "tile_loaded" => {
                Some(EventSignalRef::mapped("tile_loaded", &self.tile_loaded, |key| {
                    tile_key_value(*key)
                }))
            }
            "tile_failed" => {
                Some(EventSignalRef::mapped("tile_failed", &self.tile_failed, |key| {
                    tile_key_value(*key)
                }))
            }
            _ => None,
        }
    }
}

/// The published payload shape for a tile event: `(zoom, x, y)`.
fn tile_key_value(key: TileKey) -> CapabilityValue {
    CapabilityValue::Tuple(alloc::vec![
        CapabilityValue::Int(i64::from(key.zoom)),
        CapabilityValue::Int(i64::from(key.x)),
        CapabilityValue::Int(i64::from(key.y)),
    ])
}

impl WidgetProperties for TileLoader {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "url_template" => Ok(CapabilityValue::String(self.url_template().to_string())),
            "min_zoom" => Ok(CapabilityValue::UInt(u64::from(self.min_zoom))),
            "max_zoom" => Ok(CapabilityValue::UInt(u64::from(self.max_zoom))),
            "tile_size" => Ok(CapabilityValue::UInt(u64::from(self.tile_size))),
            "cache_limit" => Ok(CapabilityValue::UInt(self.cache_limit as u64)),
            "cache_size" => Ok(CapabilityValue::UInt(self.cache_len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "url_template" => {
                self.set_url_template(expect_string(value)?);
                Ok(())
            }
            "min_zoom" => {
                let min = expect_u32(value)?.min(u32::from(MAX_TILE_ZOOM)) as u8;
                self.set_zoom_range(min, self.max_zoom);
                Ok(())
            }
            "max_zoom" => {
                let max = expect_u32(value)?.min(u32::from(MAX_TILE_ZOOM)) as u8;
                self.set_zoom_range(self.min_zoom, max);
                Ok(())
            }
            "tile_size" => {
                self.set_tile_size(expect_u32(value)?);
                Ok(())
            }
            "cache_limit" => {
                self.set_cache_limit(expect_u32(value)? as usize);
                Ok(())
            }
            "cache_size" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "url_template",
            "min_zoom",
            "max_zoom",
            "tile_size",
            "cache_limit",
            "cache_size",
            BASE_PROPERTY_NAMES
        ]
    }

    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_cache" => {
                self.clear_cache();
                Ok(())
            }
            _ => self.default_command(name),
        }
    }
}

impl EventHandler for TileLoader {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

impl Draw for TileLoader {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // The label is resolved before the style borrow so the cached string can be read
        // immutably while the style is borrowed (SP-120).
        self.status_text();
        let style = self.base.style();
        let (surface, border, ink) = companion_palette(style, "tile_loader");
        context.face_with_gradient(rect, surface, None, Default::default(), 0, Color::BLACK);
        rust_widgets::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );
        let status = self.status_cache.as_ref().map(|(_, _, _, text)| text.as_str()).unwrap_or("");
        context.draw_text_fitted_ellipsis(
            Rect::new(rect.x + 4, rect.y + 2, rect.width.saturating_sub(8), 14),
            status,
            effective_font(style),
            ink,
            HorizontalAlignment::Left,
        );
    }
}
