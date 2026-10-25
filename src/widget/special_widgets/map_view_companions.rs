// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Map companion controls (COMP-13): tile loading, zoom strip, scale bar and compass.

use crate::compat::HashMap;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{
    expect_bool, expect_f32, expect_f64, expect_string, expect_u32,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

// `MapView` owns no tile source on purpose: fetching a tile is a host concern and
// this layer performs no I/O. These four controls carry the state a map needs
// around it and report intent through signals.

/// Resolves the surface, border and ink a companion control paints with.
fn companion_palette(style: &crate::style::WidgetStyle, role: &str) -> (Color, Color, Color) {
    let theme = crate::style::resolved_theme_style_shared(role);
    let ink = style
        .text_color
        .or_else(|| theme.as_ref().and_then(|t| t.text_color))
        .unwrap_or(Color::rgb(45, 58, 74));
    let border = style
        .border_color
        .or_else(|| theme.as_ref().and_then(|t| t.border_color))
        .unwrap_or(Color::rgb(168, 184, 201));
    let surface = style
        .background_color
        .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        .unwrap_or(Color::rgb(235, 243, 250));
    (surface, border, ink)
}

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
        self.tile_failed.emit(key);
        self.base.request_redraw();
        true
    }

    /// Forgets every addressed tile; the host keeps whatever bytes it holds.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.order.clear();
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
        }
    }
}

impl Widget for TileLoader {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(160, 24)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
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
        let style = self.base.style();
        let (surface, border, ink) = companion_palette(style, "tile_loader");
        context.face_with_gradient(rect, surface, None, Default::default(), 0, Color::BLACK);
        context.draw_rect(rect, border);
        let status = format!(
            "Tiles {}/{}  z{}-{}",
            self.cached_count(),
            self.cache.len(),
            self.min_zoom,
            self.max_zoom
        );
        context.draw_text_fitted_cow(
            Rect::new(rect.x + 4, rect.y + 2, rect.width.saturating_sub(8), 14),
            &status,
            effective_font(style),
            ink,
            HorizontalAlignment::Left,
        );
    }
}

/// The multiplier one click of a [`ZoomControl`] applies.
const ZOOM_CONTROL_STEP: f32 = 2.0;

/// One button of a [`ZoomControl`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomButton {
    /// Halves the zoom level.
    Out,
    /// Returns to 1:1.
    Reset,
    /// Doubles the zoom level.
    In,
}

/// A three-button zoom strip: `-`, `1:1`, `+`.
///
/// The control owns a zoom level and reports every committed change through
/// [`Self::zoom_requested`]; it does not zoom a map itself, so a host can apply
/// the value to a `MapView`, a tile loader or another viewport.
pub struct ZoomControl {
    base: BaseWidget,
    zoom_level: f32,
    min: f32,
    max: f32,
    /// Emitted with the new level after a change.
    pub zoom_requested: Signal1<f32>,
}

impl ZoomControl {
    /// Creates a control at 1:1 with a `0.25 ..= 8.0` range.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ZoomControl, geometry, "ZoomControl"),
            zoom_level: 1.0,
            min: 0.25,
            max: 8.0,
            zoom_requested: Signal1::new(),
        }
    }

    /// Returns the current zoom level.
    pub fn zoom_level(&self) -> f32 {
        self.zoom_level
    }

    /// Returns the lower bound.
    pub fn min(&self) -> f32 {
        self.min
    }

    /// Returns the upper bound.
    pub fn max(&self) -> f32 {
        self.max
    }

    /// Sets the level directly, clamped to the range.
    ///
    /// Returns `false` when the value is non-finite or unchanged; a no-op emits
    /// nothing.
    pub fn set_zoom(&mut self, zoom_level: f32) -> bool {
        if !zoom_level.is_finite() {
            return false;
        }
        let level = zoom_level.clamp(self.min, self.max);
        if (self.zoom_level - level).abs() < f32::EPSILON {
            return false;
        }
        self.zoom_level = level;
        self.zoom_requested.emit(level);
        self.base.request_redraw();
        true
    }

    /// Doubles the level, clamped to the range.
    pub fn zoom_in(&mut self) -> bool {
        self.set_zoom(self.zoom_level * ZOOM_CONTROL_STEP)
    }

    /// Halves the level, clamped to the range.
    pub fn zoom_out(&mut self) -> bool {
        self.set_zoom(self.zoom_level / ZOOM_CONTROL_STEP)
    }

    /// Returns to 1:1 (clamped to the range).
    pub fn reset(&mut self) -> bool {
        self.set_zoom(1.0)
    }

    /// Sets the lower bound, kept at or below the upper bound, and re-clamps the level.
    pub fn set_min(&mut self, min: f32) {
        if !min.is_finite() {
            return;
        }
        let min = min.min(self.max);
        if (self.min - min).abs() < f32::EPSILON {
            return;
        }
        self.min = min;
        self.zoom_level = self.zoom_level.clamp(self.min, self.max);
        self.base.request_redraw();
    }

    /// Sets the upper bound, kept at or above the lower bound, and re-clamps the level.
    pub fn set_max(&mut self, max: f32) {
        if !max.is_finite() {
            return;
        }
        let max = max.max(self.min);
        if (self.max - max).abs() < f32::EPSILON {
            return;
        }
        self.max = max;
        self.zoom_level = self.zoom_level.clamp(self.min, self.max);
        self.base.request_redraw();
    }

    /// The rectangle one button occupies.
    pub fn button_rect(&self, button: ZoomButton) -> Rect {
        let rect = self.geometry();
        const GAP: i32 = 2;
        const COUNT: i32 = 3;
        let available = (rect.width as i32 - GAP * (COUNT - 1)).max(1);
        let width = (available / COUNT).max(1);
        let index = match button {
            ZoomButton::Out => 0,
            ZoomButton::Reset => 1,
            ZoomButton::In => 2,
        };
        Rect::new(rect.x + index * (width + GAP), rect.y, width as u32, rect.height)
    }

    /// The button under `point`, if any.
    pub fn hit_button(&self, point: Point) -> Option<ZoomButton> {
        [ZoomButton::Out, ZoomButton::Reset, ZoomButton::In].into_iter().find(|button| {
            let rect = self.button_rect(*button);
            point.x >= rect.x
                && point.x < rect.x + rect.width as i32
                && point.y >= rect.y
                && point.y < rect.y + rect.height as i32
        })
    }
}

impl Widget for ZoomControl {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(96, 28)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "zoom_requested" => {
                Some(EventSignalRef::mapped("zoom_requested", &self.zoom_requested, |level| {
                    CapabilityValue::Float(f64::from(*level))
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for ZoomControl {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "zoom_level" => Ok(CapabilityValue::Float(f64::from(self.zoom_level))),
            "min" => Ok(CapabilityValue::Float(f64::from(self.min))),
            "max" => Ok(CapabilityValue::Float(f64::from(self.max))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "zoom_level" => {
                self.set_zoom(expect_f32(value)?);
                Ok(())
            }
            "min" => {
                self.set_min(expect_f32(value)?);
                Ok(())
            }
            "max" => {
                self.set_max(expect_f32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["zoom_level", "min", "max", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for ZoomControl {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => match self.hit_button(*pos) {
                Some(ZoomButton::Out) => {
                    self.zoom_out();
                }
                Some(ZoomButton::Reset) => {
                    self.reset();
                }
                Some(ZoomButton::In) => {
                    self.zoom_in();
                }
                None => { /* A press outside the three buttons changes nothing. */ }
            },
            Event::Wheel { delta, .. } => {
                if delta.y < 0 {
                    self.zoom_in();
                } else if delta.y > 0 {
                    self.zoom_out();
                }
            }
            _ => { /* Other events are not relevant. */ }
        }
    }
}

impl Draw for ZoomControl {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let (surface, border, ink) = companion_palette(style, "zoom_control");
        context.face_with_gradient(rect, surface, None, Default::default(), 0, Color::BLACK);
        context.draw_rect(rect, border);
        for button in [ZoomButton::Out, ZoomButton::Reset, ZoomButton::In] {
            let button_rect = self.button_rect(button);
            context.face_with_gradient(
                button_rect,
                surface,
                None,
                Default::default(),
                2,
                Color::BLACK,
            );
            context.draw_rect(button_rect, border);
            let label = match button {
                ZoomButton::Out => "-",
                ZoomButton::Reset => "1:1",
                ZoomButton::In => "+",
            };
            let font = effective_font(style);
            let metrics = context.measure_text(label, font);
            context.draw_text(
                Point::new(
                    button_rect.x + (button_rect.width as i32 - metrics.width as i32) / 2,
                    button_rect.y + (button_rect.height as i32 - metrics.height as i32) / 2,
                ),
                label,
                font,
                ink,
                HorizontalAlignment::Left,
            );
        }
    }
}

/// The unit system a [`ScaleBar`] labels its distance in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleUnits {
    /// Metres and kilometres.
    #[default]
    Metric,
    /// Feet and miles.
    Imperial,
}

impl ScaleUnits {
    /// The stable property token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Imperial => "imperial",
        }
    }

    /// Parses a property token.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "metric" => Some(Self::Metric),
            "imperial" => Some(Self::Imperial),
            _ => None,
        }
    }
}

/// How a [`ScaleBar`] frames its bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleBarStyle {
    /// A bare bar and label, for drawing directly on a map.
    #[default]
    Plain,
    /// A bordered box behind the bar, for use over busy tile imagery.
    Boxed,
}

impl ScaleBarStyle {
    /// The stable property token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Boxed => "boxed",
        }
    }

    /// Parses a property token.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "plain" => Some(Self::Plain),
            "boxed" => Some(Self::Boxed),
            _ => None,
        }
    }
}

/// Padding kept clear at both ends of a [`ScaleBar`].
const SCALE_BAR_PADDING: f64 = 8.0;

/// A distances legend for a map.
///
/// The bar length comes from `meters_per_pixel`, rounded down to a 1/2/5 × 10ⁿ
/// value so the label is a number a reader can hold in their head.
pub struct ScaleBar {
    base: BaseWidget,
    meters_per_pixel: f64,
    units: ScaleUnits,
    style: ScaleBarStyle,
}

impl ScaleBar {
    /// Creates a metric, plain-styled bar at 1 metre per pixel.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ScaleBar, geometry, "ScaleBar"),
            meters_per_pixel: 1.0,
            units: ScaleUnits::Metric,
            style: ScaleBarStyle::Plain,
        }
    }

    /// Returns the current scale.
    pub fn meters_per_pixel(&self) -> f64 {
        self.meters_per_pixel
    }

    /// Sets the map scale. Non-finite and non-positive values are ignored.
    pub fn set_scale(&mut self, meters_per_pixel: f64) {
        if !meters_per_pixel.is_finite() || meters_per_pixel <= 0.0 {
            return;
        }
        if (self.meters_per_pixel - meters_per_pixel).abs() < f64::EPSILON {
            return;
        }
        self.meters_per_pixel = meters_per_pixel;
        self.base.request_redraw();
    }

    /// Returns the unit system.
    pub fn units(&self) -> ScaleUnits {
        self.units
    }

    /// Sets the unit system.
    pub fn set_units(&mut self, units: ScaleUnits) {
        if self.units == units {
            return;
        }
        self.units = units;
        self.base.request_redraw();
    }

    /// Returns the visual style.
    pub fn style(&self) -> ScaleBarStyle {
        self.style
    }

    /// Sets the visual style.
    pub fn set_style(&mut self, style: ScaleBarStyle) {
        if self.style == style {
            return;
        }
        self.style = style;
        self.base.request_redraw();
    }

    /// The largest 1/2/5 × 10ⁿ distance whose bar fits in `max_px`.
    ///
    /// Returns `0.0` for a degenerate or non-finite width; a caller drawing the
    /// result should skip the bar rather than paint a zero-length one.
    pub fn bar_distance(&self, max_px: f64) -> f64 {
        if !max_px.is_finite() || max_px <= 0.0 {
            return 0.0;
        }
        nice_floor(self.meters_per_pixel * max_px)
    }

    /// The label a caller would draw, derived from the control's width.
    pub fn label(&self) -> String {
        let available = (f64::from(self.geometry().width) - 2.0 * SCALE_BAR_PADDING).max(16.0);
        format_scale_distance(self.units, self.bar_distance(available))
    }

    /// The width of the bar in pixels at the current scale.
    pub fn bar_pixels(&self) -> u32 {
        let available = (f64::from(self.geometry().width) - 2.0 * SCALE_BAR_PADDING).max(16.0);
        let distance = self.bar_distance(available);
        if distance <= 0.0 {
            return 0;
        }
        (distance / self.meters_per_pixel).round().clamp(0.0, available) as u32
    }
}

/// Rounds `value` down to the nearest 1, 2 or 5 times a power of ten.
fn nice_floor(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 0.0;
    }
    let magnitude = 10f64.powf(value.log10().floor());
    let mantissa = value / magnitude;
    let step = if mantissa >= 5.0 {
        5.0
    } else if mantissa >= 2.0 {
        2.0
    } else {
        1.0
    };
    step * magnitude
}

/// Formats `meters` for `units` with the precision the magnitude deserves.
fn format_scale_distance(units: ScaleUnits, meters: f64) -> String {
    if !meters.is_finite() || meters <= 0.0 {
        return String::new();
    }
    match units {
        ScaleUnits::Metric => {
            if meters >= 1000.0 {
                format_scaled(meters / 1000.0, "km")
            } else {
                format_scaled(meters, "m")
            }
        }
        ScaleUnits::Imperial => {
            let feet = meters * 3.280_839_895;
            if feet >= 5280.0 {
                format_scaled(feet / 5280.0, "mi")
            } else if feet >= 1.0 {
                format_scaled(feet, "ft")
            } else {
                format_scaled(feet * 12.0, "in")
            }
        }
    }
}

/// Prints a whole number without a decimal point and a fraction with one.
fn format_scaled(value: f64, unit: &str) -> String {
    if (value - value.round()).abs() < 0.05 {
        format!("{:.0} {unit}", value)
    } else {
        format!("{:.1} {unit}", value)
    }
}

impl Widget for ScaleBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(120, 28)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for ScaleBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "meters_per_pixel" => Ok(CapabilityValue::Float(self.meters_per_pixel)),
            "units" => Ok(CapabilityValue::String(self.units.token().to_string())),
            "style" => Ok(CapabilityValue::String(self.style.token().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "meters_per_pixel" => {
                self.set_scale(expect_f64(value)?);
                Ok(())
            }
            "units" => {
                let token = expect_string(value)?;
                let units =
                    ScaleUnits::from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_units(units);
                Ok(())
            }
            "style" => {
                let token = expect_string(value)?;
                let style =
                    ScaleBarStyle::from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_style(style);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["meters_per_pixel", "units", "style", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for ScaleBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

impl Draw for ScaleBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let (surface, border, ink) = companion_palette(style, "scale_bar");
        if self.style == ScaleBarStyle::Boxed {
            context.face_with_gradient(rect, surface, None, Default::default(), 3, Color::BLACK);
            context.draw_rect(rect, border);
        }
        let width = self.bar_pixels();
        if width == 0 {
            return;
        }
        let left = rect.x + SCALE_BAR_PADDING as i32;
        let bottom = rect.y + rect.height as i32 - 6;
        let right = left + width as i32;
        context.draw_line(Point::new(left, bottom), Point::new(right, bottom), ink);
        for x in [left, right] {
            context.draw_line(Point::new(x, bottom - 3), Point::new(x, bottom), ink);
        }
        let label = self.label();
        if !label.is_empty() {
            context.draw_text(
                Point::new(left, bottom - 16),
                &label,
                effective_font(style),
                ink,
                HorizontalAlignment::Left,
            );
        }
    }
}

/// A north needle that reports a reset request when clicked.
///
/// The compass displays a bearing but does not own one: `bearing_deg` is written
/// by the host, and a click emits [`Self::reset_north_requested`] without changing
/// it — the component that owns the rotation is the only one that can undo it.
pub struct Compass {
    base: BaseWidget,
    bearing_deg: f32,
    clickable: bool,
    /// Emitted when a clickable compass is pressed.
    pub reset_north_requested: GenericSignal,
}

impl Compass {
    /// Creates a clickable compass pointing north.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Compass, geometry, "Compass"),
            bearing_deg: 0.0,
            clickable: true,
            reset_north_requested: GenericSignal::new(),
        }
    }

    /// Returns the displayed bearing in `0 .. 360` degrees, clockwise from north.
    pub fn bearing_deg(&self) -> f32 {
        self.bearing_deg
    }

    /// Sets the displayed bearing, normalised into `0 .. 360`.
    ///
    /// Non-finite values are ignored; `-90` becomes `270` and `450` becomes `90`.
    pub fn set_bearing(&mut self, bearing_deg: f32) {
        if !bearing_deg.is_finite() {
            return;
        }
        let normalized = bearing_deg.rem_euclid(360.0);
        if (self.bearing_deg - normalized).abs() < f32::EPSILON {
            return;
        }
        self.bearing_deg = normalized;
        self.base.request_redraw();
    }

    /// Whether a press reports a reset request.
    pub fn clickable(&self) -> bool {
        self.clickable
    }

    /// Enables or disables the click target.
    pub fn set_clickable(&mut self, clickable: bool) {
        if self.clickable == clickable {
            return;
        }
        self.clickable = clickable;
        self.base.request_redraw();
    }

    /// Whether `point` falls inside the dial.
    pub fn hit_dial(&self, point: Point) -> bool {
        let rect = self.geometry();
        let center = Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2);
        let radius = i64::from(self.radius());
        let dx = i64::from(point.x) - i64::from(center.x);
        let dy = i64::from(point.y) - i64::from(center.y);
        dx * dx + dy * dy <= radius * radius
    }

    /// The dial's radius in pixels.
    fn radius(&self) -> u32 {
        let rect = self.geometry();
        (rect.width.min(rect.height) as i32 / 2 - 2).max(4) as u32
    }
}

impl Widget for Compass {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(48, 48)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "reset_north_requested" => {
                Some(EventSignalRef::unit("reset_north_requested", &self.reset_north_requested))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for Compass {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "bearing_deg" => Ok(CapabilityValue::Float(f64::from(self.bearing_deg))),
            "clickable" => Ok(CapabilityValue::Bool(self.clickable)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "bearing_deg" => {
                self.set_bearing(expect_f32(value)?);
                Ok(())
            }
            "clickable" => {
                self.set_clickable(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["bearing_deg", "clickable", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for Compass {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if self.clickable && self.hit_dial(*pos) {
                self.reset_north_requested.emit();
                self.base.request_redraw();
            }
        }
    }
}

impl Draw for Compass {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let (surface, border, ink) = companion_palette(style, "compass");
        let center = Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2);
        let radius = self.radius();
        context.fill_circle(center, radius, surface);
        context.draw_circle(center, radius, border);
        context.draw_text(
            Point::new(center.x - 3, rect.y + 2),
            "N",
            effective_font(style),
            ink,
            HorizontalAlignment::Left,
        );
        // `bearing_deg` is clockwise from north, so the needle tip is `sin` across
        // and `-cos` up — the same convention a map rotation uses.
        let angle = self.bearing_deg.to_radians();
        let length = radius as f32 - 2.0;
        let tip = Point::new(
            center.x + (angle.sin() * length) as i32,
            center.y - (angle.cos() * length) as i32,
        );
        context.draw_line(center, tip, ink);
        let head = (radius as f32 * 0.35).max(4.0);
        for offset in [0.45f32, -0.45] {
            let wing = angle + offset;
            context.draw_line(
                tip,
                Point::new(tip.x - (wing.sin() * head) as i32, tip.y + (wing.cos() * head) as i32),
                ink,
            );
        }
    }
}
