// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MapView widget.

use crate::compat::HashMap;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_f32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use alloc::rc::Rc;
use core::cell::{Ref, RefCell};

/// A map marker's pin size: a 6x6 square centred on the marker's screen position.
///
/// Named because the paint, the label's offset and the "is this marker inside the pane?"
/// test all measure the pin, and three spellings of `6`/`3` are how the pin and its own hit
/// box drift apart.
const MAP_MARKER_SIZE: u32 = 6;

/// The default pin colour, and the colour of a cluster badge.
const MAP_MARKER_COLOR: Color = Color::rgb(59, 114, 190);

/// The colour of the selected marker's pin.
const MAP_MARKER_SELECTED_COLOR: Color = Color::rgb(230, 88, 76);

/// How one marker is painted, independent of where it sits.
///
/// # Why the style is per marker
///
/// The pin's colour encodes the marker's *kind* (a station, a vehicle, an alert), which is data
/// about the marker rather than a property of the map. A single map-wide colour, which is what the
/// control painted before, forced every caller to draw its own overlay for that distinction or to
/// drop it. `size` is a diameter in logical pixels; `icon` replaces the pin with real geometry when
/// the caller supplies one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkerStyle {
    /// Fill colour of the pin or the icon.
    pub color: Color,
    /// Pin diameter in logical pixels.
    pub size: u32,
    /// Optional icon painted instead of the pin.
    pub icon: Option<crate::widget::IconSource>,
}

impl Default for MarkerStyle {
    fn default() -> Self {
        Self { color: MAP_MARKER_COLOR, size: MAP_MARKER_SIZE, icon: None }
    }
}

impl MarkerStyle {
    /// Creates a pin style with `color` and `size` (floored at 2 pixels).
    pub fn new(color: Color, size: u32) -> Self {
        Self { color, size: size.max(2), icon: None }
    }

    /// Replaces the pin with `icon`, keeping the colour as the icon's ink.
    pub fn with_icon(mut self, icon: crate::widget::IconSource) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// The coordinate mapping the map paints with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapProjection {
    /// World coordinates are screen offsets at zoom 1: one world unit is one pixel.
    #[default]
    Linear,
    /// The y axis is Web Mercator latitude, in degree units, so north-south distances grow
    /// toward the poles the way a slippy map draws them. x is unchanged.
    Mercator,
}

impl MapProjection {
    /// The token this projection is published under.
    pub fn token(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Mercator => "mercator",
        }
    }

    /// Parses a published token; `None` for an unknown spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "linear" => Some(Self::Linear),
            "mercator" => Some(Self::Mercator),
            _ => None,
        }
    }
}

/// The latitude Web Mercator truncates at, so the projection stays finite.
const MERCATOR_MAX_LATITUDE: f32 = 85.051_13;

/// Hands out the identities the render layer cache uses to recognise an unchanged map grid.
///
/// Globally unique because the cache belongs to the surface: two maps that both started at `1`
/// would reuse each other's cached pixels.
fn next_map_key() -> u64 {
    use core::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Packs a colour into the low bits of a layer-cache key, so a theme change moves the key.
fn color_key(color: Color) -> u64 {
    (u64::from(color.r) << 24)
        | (u64::from(color.g) << 16)
        | (u64::from(color.b) << 8)
        | u64::from(color.a)
}

/// One map marker with logical world coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct MapMarker {
    /// Stable marker id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// World x coordinate in logical map units.
    pub x: f32,
    /// World y coordinate in logical map units (latitude in degrees under
    /// [`MapProjection::Mercator`]).
    pub y: f32,
    /// How the marker is painted.
    pub style: MarkerStyle,
}

impl MapMarker {
    /// Creates a map marker with the default pin style.
    pub fn new(id: impl Into<String>, label: impl Into<String>, x: f32, y: f32) -> Self {
        Self { id: id.into(), label: label.into(), x, y, style: MarkerStyle::default() }
    }

    /// Sets the marker's paint style.
    pub fn with_style(mut self, style: MarkerStyle) -> Self {
        self.style = style;
        self
    }
}

/// The declared shape of one map marker, as a record (BLUE26 B26.09).
///
/// This is the row shape of the `markers` property, and the control owns both ends of it: the
/// field names are [`MapMarker`]'s own fields plus [`MarkerStyle`]'s. `id`, `label`, `x` and `y`
/// are required because [`MapMarker::new`] requires them; the style fields are optional because a
/// marker with no style uses [`MarkerStyle::default`].
pub const MAP_MARKER_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "id",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "label",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "x",
                crate::widget::capability::PropertyValueKind::Float,
            ),
            crate::widget::capability::RecordField::required(
                "y",
                crate::widget::capability::PropertyValueKind::Float,
            ),
            crate::widget::capability::RecordField::optional(
                "color",
                crate::widget::capability::PropertyValueKind::Color,
            ),
            crate::widget::capability::RecordField::optional(
                "size",
                crate::widget::capability::PropertyValueKind::UInt,
            ),
            crate::widget::capability::RecordField::optional(
                "icon_source",
                crate::widget::capability::PropertyValueKind::IconSpec,
            ),
        ],
        id_field: "id",
        ids_unique: true,
        ids_required: true,
    };

/// One row of [`MAP_MARKER_SCHEMA`]; refuses a source with no document form.
fn map_marker_record(
    marker: &MapMarker,
) -> Result<crate::widget::capability::RecordValue, CapabilityAccessError> {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("id", CapabilityValue::String(marker.id.clone()))
        .with("label", CapabilityValue::String(marker.label.clone()))
        .with("x", CapabilityValue::Float(f64::from(marker.x)))
        .with("y", CapabilityValue::Float(f64::from(marker.y)))
        .with("color", CapabilityValue::Color(marker.style.color))
        .with("size", CapabilityValue::UInt(u64::from(marker.style.size)));
    match marker.style.icon {
        None => {}
        Some(crate::widget::IconSource::Builtin(name)) => {
            record = record.with(
                "icon_source",
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(name)),
            );
        }
        // A borrowed static geometry has no document form, so the read refuses rather than
        // reporting the row as icon-less.
        Some(crate::widget::IconSource::StaticGeometry(_)) => {
            return Err(CapabilityAccessError::IconSourceNotPersistable)
        }
    }
    Ok(record)
}

/// Rebuilds one [`MapMarker`] from a row of [`MAP_MARKER_SCHEMA`].
fn map_marker_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<MapMarker, CapabilityAccessError> {
    let text = |name: &str| match record.get(name) {
        Some(CapabilityValue::String(value)) => Ok(value.clone()),
        _ => Err(CapabilityAccessError::TypeMismatch),
    };
    let number = |name: &str| match record.get(name) {
        Some(CapabilityValue::Float(value)) => Ok(*value as f32),
        Some(CapabilityValue::Int(value)) => Ok(*value as f32),
        Some(CapabilityValue::UInt(value)) => Ok(*value as f32),
        _ => Err(CapabilityAccessError::TypeMismatch),
    };
    let mut style = MarkerStyle::default();
    match record.get("color") {
        None | Some(CapabilityValue::Null) => {}
        Some(CapabilityValue::Color(color)) => style.color = *color,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    }
    match record.get("size") {
        None | Some(CapabilityValue::Null) => {}
        Some(CapabilityValue::UInt(size)) => {
            style.size = u32::try_from(*size).map_err(|_| CapabilityAccessError::TypeMismatch)?;
        }
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    }
    match record.get("icon_source") {
        None | Some(CapabilityValue::Null) => {}
        Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(name))) => {
            style.icon = Some(crate::widget::IconSource::Builtin(*name));
        }
        // A resource descriptor needs the host's resolver, which a property write does not have.
        Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
            ..
        })) => return Err(CapabilityAccessError::ResourceResolutionRequired),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    }
    Ok(MapMarker {
        id: text("id")?,
        label: text("label")?,
        x: number("x")?,
        y: number("y")?,
        style,
    })
}

/// One painted cluster: the marker indices it groups and the point it is drawn at.
#[derive(Clone)]
struct MapCluster {
    /// The members' indices into [`MapView::markers`].
    indices: Vec<usize>,
    /// Screen position the badge is painted at.
    screen: (f32, f32),
    /// World position reported when the badge is clicked.
    world: (f32, f32),
}

/// Lightweight map view with pan/zoom and marker selection.
pub struct MapView {
    base: BaseWidget,
    center_x: f32,
    center_y: f32,
    zoom: f32,
    markers: Vec<MapMarker>,
    selected_marker: Option<usize>,
    /// The **fraction** of the way from the previous zoom to the current one, so the painted
    /// projection can be interpolated without pretending a `0..=1` progress driver is a general
    /// value.
    ///
    /// # Why a progress and not a second zoom value
    ///
    /// `PropertyDriver` clamps its target to `0.0..=1.0` (`Transition::tick`), because every other
    /// consumer of it animates a *fraction* — a reveal, a travel, a fade. Feeding it a zoom of
    /// `4.0` therefore asks for a target it can never reach: `is_moving()` stays true forever and
    /// the host schedules frames for the rest of the session. The fraction is the value the driver
    /// can actually hold, and the zoom it corresponds to is derived from the two endpoints.
    zoom_t: PropertyDriver,
    /// The zoom the current transition started from, so `zoom_t` has a left endpoint.
    zoom_from: f32,
    /// The last pointer position, used to anchor wheel zoom.
    last_pointer: Option<Point>,
    /// A pan in progress: `(press position, center at press)`.
    drag: Option<(Point, f32, f32)>,
    /// Whether nearby markers are grouped into one count badge.
    cluster_markers: bool,
    /// The y-axis mapping.
    projection: MapProjection,
    /// Emitted when center changes. Payload is (x, y).
    pub center_changed: Signal1<(f32, f32)>,
    /// Emitted when zoom changes.
    pub zoom_changed: Signal1<f32>,
    /// Emitted when marker is selected. Payload is marker id.
    pub marker_selected: Signal1<String>,
    /// Emitted when a marker is clicked. Payload is marker id.
    ///
    /// Distinct from [`Self::marker_selected`] because a click is an *act* and a selection is a
    /// *state*: a caller that re-selects the already-selected pin still deserves the click, which a
    /// selection-change signal by definition cannot deliver.
    pub marker_clicked: Signal1<String>,
    /// Emitted when a multi-marker cluster badge is clicked. Payload is the cluster's world centre.
    pub cluster_clicked: Signal1<(f32, f32)>,
    /// Emitted with each tile the map asks the host to fetch.
    ///
    /// The map never performs I/O: it computes the addresses the visible viewport needs and
    /// reports them, and the host answers through [`Self::deliver_tile`] once it holds the bytes.
    pub tile_requested: Signal1<TileRequest>,
    /// The tile source. It owns addresses and the delivered-pixel cache, not a network client.
    tile_loader: TileLoader,
    /// The `-`/`1:1`/`+` strip painted over the map.
    zoom_control: ZoomControl,
    /// The distance legend painted in the corner.
    scale_bar: ScaleBar,
    /// The north needle painted under the zoom strip.
    compass: Compass,
    /// Whether the three overlay controls are painted and hit-tested.
    show_map_controls: bool,
    /// Whether a paint asks the host for the tiles the viewport touches.
    ///
    /// The sweep itself runs from the camera writers ([`Self::set_center`], [`Self::set_zoom`]),
    /// not from the paint: a paint-time prefetch ran inside the frame it was painting and could
    /// re-enter the tile loader for a camera that had not moved (SP-P60). The accessor name is kept
    /// because that is the published property, and the behaviour it names — "ask for the visible
    /// tiles automatically" — is unchanged.
    prefetch_on_paint: bool,
    /// Bumped whenever the marker list changes; keys the cluster cache.
    markers_revision: u64,
    /// The projected clusters, shared by the paint and the hit test (SP-P58).
    cluster_cache: RefCell<Option<(ClusterKey, Rc<Vec<MapCluster>>)>>,
    /// The status line and the camera it was rendered for (SP-P59).
    ///
    /// `RefCell` because the paint refreshes it while it still holds the layout's shared style
    /// borrow.
    status_cache: RefCell<Option<StatusCache>>,
    /// The geometry the overlay controls were last laid out for.
    controls_layout: Option<Rect>,
    /// Identity of the painted grid; a fresh key rebuilds its cached pixels (SP-P59).
    grid_cache_key: u64,
}

/// The last rendered status line and the camera it names.
struct StatusCache {
    /// The camera the text was built for.
    key: ((f32, f32), f32),
    /// The rendered line.
    text: String,
}

/// The inputs the cluster cache depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ClusterKey {
    painted_zoom: f32,
    center: (f32, f32),
    rect: Rect,
    markers_revision: u64,
    projection: MapProjection,
}

impl MapView {
    /// Creates an empty map view.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Canvas, geometry, "MapView"),
            center_x: 0.0,
            center_y: 0.0,
            zoom: 1.0,
            zoom_t: PropertyDriver::at(1.0, MotionSlot::Normal),
            zoom_from: 1.0,
            last_pointer: None,
            drag: None,
            cluster_markers: false,
            projection: MapProjection::Linear,
            markers: Vec::new(),
            selected_marker: None,
            center_changed: Signal1::new(),
            zoom_changed: Signal1::new(),
            marker_selected: Signal1::new(),
            marker_clicked: Signal1::new(),
            cluster_clicked: Signal1::new(),
            tile_requested: Signal1::new(),
            tile_loader: TileLoader::new(Rect::new(0, 0, 0, 0)),
            zoom_control: ZoomControl::new(Rect::new(0, 0, 0, 0)),
            scale_bar: ScaleBar::new(Rect::new(0, 0, 0, 0)),
            compass: Compass::new(Rect::new(0, 0, 0, 0)),
            show_map_controls: true,
            prefetch_on_paint: true,
            markers_revision: 1,
            cluster_cache: RefCell::new(None),
            status_cache: RefCell::new(None),
            controls_layout: None,
            grid_cache_key: next_map_key(),
        }
    }

    /// Drops the cached status line so the next paint rebuilds it.
    fn invalidate_status(&mut self) {
        *self.status_cache.get_mut() = None;
    }

    /// The tile source: its template, zoom range and delivered-tile cache.
    pub fn tile_loader(&self) -> &TileLoader {
        &self.tile_loader
    }

    /// The tile source, for a host that delivers pixels or changes the template.
    pub fn tile_loader_mut(&mut self) -> &mut TileLoader {
        &mut self.tile_loader
    }

    /// The zoom strip painted over the map.
    pub fn zoom_control(&self) -> &ZoomControl {
        &self.zoom_control
    }

    /// The zoom strip, for a host that wants its own range or signal.
    pub fn zoom_control_mut(&mut self) -> &mut ZoomControl {
        &mut self.zoom_control
    }

    /// The distance legend painted over the map.
    pub fn scale_bar(&self) -> &ScaleBar {
        &self.scale_bar
    }

    /// The distance legend, for a host that changes units or style.
    pub fn scale_bar_mut(&mut self) -> &mut ScaleBar {
        &mut self.scale_bar
    }

    /// The north needle painted over the map.
    pub fn compass(&self) -> &Compass {
        &self.compass
    }

    /// The north needle, for a host that listens for its reset request.
    pub fn compass_mut(&mut self) -> &mut Compass {
        &mut self.compass
    }

    /// Whether the zoom strip, compass and scale bar are painted.
    pub fn map_controls_visible(&self) -> bool {
        self.show_map_controls
    }

    /// Shows or hides the three overlay controls.
    pub fn set_map_controls_visible(&mut self, visible: bool) {
        if self.show_map_controls == visible {
            return;
        }
        self.show_map_controls = visible;
        self.base.request_redraw();
    }

    /// Whether a paint asks the host for the tiles the viewport touches.
    pub fn prefetch_on_paint(&self) -> bool {
        self.prefetch_on_paint
    }

    /// Enables or disables the automatic prefetch a paint performs.
    pub fn set_prefetch_on_paint(&mut self, enabled: bool) {
        self.prefetch_on_paint = enabled;
    }

    /// Records pixels a host fetched for `key`.
    pub fn deliver_tile(&mut self, key: TileKey, payload: Vec<u8>) -> bool {
        let delivered = self.tile_loader.deliver_tile(key, payload);
        if delivered {
            self.base.request_redraw();
        }
        delivered
    }

    /// Reports that the host could not fetch `key`.
    pub fn reject_tile(&mut self, key: TileKey) -> bool {
        let rejected = self.tile_loader.reject_tile(key);
        if rejected {
            self.base.request_redraw();
        }
        rejected
    }

    /// The tile window the current camera and pane cover.
    ///
    /// The map's world unit is one logical pixel at zoom 1, and a tile is `tile_size` world units
    /// at tile zoom 0. The tile zoom follows the painted zoom's `log2`, so a fractional pinch does
    /// not ask for a new level on every intermediate frame.
    pub fn tile_viewport(&self) -> TileViewport {
        let rect = self.geometry();
        let painted = self.painted_zoom().max(0.0001);
        let raw_zoom = (painted.log2().round() as i32)
            .clamp(self.tile_loader.min_zoom() as i32, self.tile_loader.max_zoom() as i32);
        let zoom = raw_zoom as u8;
        let scale = 2f64.powi(zoom as i32);
        let tile_world = (self.tile_loader.tile_size().max(1) as f64 / scale).max(1.0);
        let world_w = rect.width as f64 / painted as f64;
        let world_h = rect.height as f64 / painted as f64;
        let x = (self.center_x as f64 - world_w / 2.0) / tile_world;
        let y = (self.project_world_y(self.center_y) as f64 - world_h / 2.0) / tile_world;
        TileViewport::new(zoom, x, y, rect.width, rect.height)
    }

    /// Asks the tile source for every tile the visible viewport touches.
    ///
    /// A request the loader already remembers reports `cached: true`; only a first request is
    /// re-emitted through [`Self::tile_requested`], so a host can fetch exactly once per address.
    pub fn prefetch_tiles(&mut self) -> Vec<TileRequest> {
        let viewport = self.tile_viewport();
        let requests = self.tile_loader.prefetch(viewport);
        for request in &requests {
            if !request.cached {
                self.tile_requested.emit(request.clone());
            }
        }
        requests
    }

    /// Places the three overlay controls inside the map's rectangle and syncs their state.
    ///
    /// The geometry is recomputed only when the map's rectangle changes: both the event path and
    /// the paint path call this, and a still frame used to re-derive all three boxes twice per
    /// frame (SP-P59).
    fn layout_map_controls(&mut self) {
        let rect = self.geometry();
        if self.controls_layout == Some(rect) {
            return;
        }
        self.controls_layout = Some(rect);
        let zoom_size = self.zoom_control.size_hint();
        let compass_size = self.compass.size_hint();
        let scale_size = self.scale_bar.size_hint();
        let zoom_rect = Rect::new(
            rect.x + rect.width as i32 - zoom_size.width as i32 - 8,
            rect.y + 8,
            zoom_size.width,
            zoom_size.height,
        );
        let compass_rect = Rect::new(
            zoom_rect.x + (zoom_rect.width as i32 - compass_size.width as i32) / 2,
            zoom_rect.y + zoom_rect.height as i32 + 6,
            compass_size.width,
            compass_size.height,
        );
        let scale_rect = Rect::new(
            rect.x + 8,
            rect.y + rect.height as i32 - scale_size.height as i32 - 8,
            scale_size.width,
            scale_size.height,
        );
        if self.zoom_control.base().geometry() != zoom_rect {
            self.zoom_control.base_mut().set_geometry(zoom_rect);
        }
        if self.compass.base().geometry() != compass_rect {
            self.compass.base_mut().set_geometry(compass_rect);
        }
        if self.scale_bar.base().geometry() != scale_rect {
            self.scale_bar.base_mut().set_geometry(scale_rect);
        }
    }

    /// Mirrors the map's camera into the overlay controls, once per paint.
    fn sync_map_controls(&mut self) {
        // The strip is a view of the map's zoom, not a second source of truth: a wheel or a key
        // that changed the map is reflected here, and a press here changes the map.
        if (self.zoom_control.zoom_level() - self.zoom).abs() > f32::EPSILON {
            self.zoom_control.set_zoom(self.zoom);
        }
        let meters_per_pixel = 1.0 / self.painted_zoom().max(0.0001) as f64;
        self.scale_bar.set_scale(meters_per_pixel);
    }

    /// Returns map center.
    pub fn center(&self) -> (f32, f32) {
        (self.center_x, self.center_y)
    }

    /// Sets map center.
    pub fn set_center(&mut self, x: f32, y: f32) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        if (self.center_x - x).abs() < f32::EPSILON && (self.center_y - y).abs() < f32::EPSILON {
            return;
        }
        self.center_x = x;
        self.center_y = y;
        self.invalidate_status();
        self.center_changed.emit((x, y));
        self.refresh_tiles();
        self.base.request_redraw();
    }

    /// Asks the tile source for the viewport the camera now covers.
    ///
    /// Called from the camera writers rather than from `draw`, so the paint never performs the
    /// sweep and a still map issues no requests at all (SP-P60).
    fn refresh_tiles(&mut self) {
        if self.prefetch_on_paint {
            let _ = self.prefetch_tiles();
        }
    }

    /// Pans map center by world delta.
    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        if !dx.is_finite() || !dy.is_finite() {
            return;
        }
        self.set_center(self.center_x + dx, self.center_y + dy);
    }

    /// Returns zoom level.
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Enables grouping markers that are close together on screen into one count badge.
    ///
    /// Clustering is opt-in: a map with a handful of pins reads better ungrouped, and the
    /// grouped form is a different picture rather than a correction of one.
    pub fn set_cluster_markers(&mut self, cluster: bool) {
        if self.cluster_markers == cluster {
            return;
        }
        self.cluster_markers = cluster;
        self.base.request_redraw();
    }

    /// Returns whether markers are clustered.
    pub fn cluster_markers(&self) -> bool {
        self.cluster_markers
    }

    /// The y coordinate the projection paints with.
    ///
    /// `Linear` is the identity, so a map that never asks for a projection behaves exactly as it
    /// did. `Mercator` maps a latitude in degrees onto its Web Mercator ordinate (also in degrees),
    /// which makes the same trip cover more screen height near the poles — the reason a slippy map's
    /// equator-to-45° band is smaller than its 45°-to-60° band. x is untouched by either.
    fn project_world_y(&self, y: f32) -> f32 {
        match self.projection {
            MapProjection::Linear => y,
            MapProjection::Mercator => {
                let latitude = y.clamp(-MERCATOR_MAX_LATITUDE, MERCATOR_MAX_LATITUDE).to_radians();
                (core::f32::consts::FRAC_PI_4 + latitude / 2.0).tan().ln().to_degrees()
            }
        }
    }

    /// The inverse of [`Self::project_world_y`], so a pointer position can be turned back into a
    /// world latitude before it becomes the map centre.
    fn unproject_world_y(&self, projected: f32) -> f32 {
        match self.projection {
            MapProjection::Linear => projected,
            MapProjection::Mercator => {
                let ordinate = projected.to_radians();
                // `2·atan(e^y) - π/2` is the inverse Gudermannian.
                (2.0 * ordinate.exp().atan() - core::f32::consts::FRAC_PI_2).to_degrees()
            }
        }
    }

    /// Returns the coordinate mapping.
    pub fn projection(&self) -> MapProjection {
        self.projection
    }

    /// Sets the coordinate mapping.
    pub fn set_projection(&mut self, projection: MapProjection) {
        if self.projection == projection {
            return;
        }
        self.projection = projection;
        self.base.request_redraw();
    }

    /// Adds one marker, returning its index.
    ///
    /// [`Self::set_markers`] replaces the whole list and was the only way to change the marker set;
    /// adding or removing one pin therefore meant cloning the entire list, which also dropped the
    /// selection whenever the replace happened to reorder it.
    pub fn add_marker(&mut self, marker: MapMarker) -> usize {
        let index = self.markers.len();
        self.markers.push(marker);
        self.markers_revision = self.markers_revision.wrapping_add(1);
        self.base.request_redraw();
        index
    }

    /// Removes the marker with `id`, returning it when it was present.
    ///
    /// The selected index is adjusted rather than cleared: removing a pin *above* the selected one
    /// shifts it down by one, and clearing the selection there would report a selection the caller
    /// never changed.
    pub fn remove_marker(&mut self, id: &str) -> Option<MapMarker> {
        let index = self.markers.iter().position(|marker| marker.id == id)?;
        let removed = self.markers.remove(index);
        self.markers_revision = self.markers_revision.wrapping_add(1);
        self.selected_marker = match self.selected_marker {
            Some(selected) if selected == index => None,
            Some(selected) if selected > index => Some(selected - 1),
            other => other,
        };
        self.base.request_redraw();
        Some(removed)
    }

    /// Sets the paint style of the marker with `id`, returning whether it exists.
    pub fn set_marker_style(&mut self, id: &str, style: MarkerStyle) -> bool {
        let Some(marker) = self.markers.iter_mut().find(|marker| marker.id == id) else {
            return false;
        };
        marker.style = style;
        self.markers_revision = self.markers_revision.wrapping_add(1);
        self.base.request_redraw();
        true
    }

    /// The pin diameter of the marker at `index`.
    fn marker_size(&self, index: usize) -> u32 {
        self.markers.get(index).map_or(MAP_MARKER_SIZE, |marker| marker.style.size.max(2))
    }

    /// Zooms by `factor` while keeping the world point under `anchor` fixed on screen.
    fn zoom_at(&mut self, factor: f32, anchor: Point) {
        let rect = self.geometry();
        let zoom_before = self.painted_zoom().max(0.0001);
        let center_px = rect.x as f32 + rect.width as f32 / 2.0;
        let center_py = rect.y as f32 + rect.height as f32 / 2.0;
        // The anchor is converted into projected space, so a Mercator map keeps the latitude under
        // the pointer fixed rather than the raw world value (which would slide it vertically).
        let world_x = self.center_x + (anchor.x as f32 - center_px) / zoom_before;
        let world_y =
            self.project_world_y(self.center_y) + (anchor.y as f32 - center_py) / zoom_before;
        let before = self.zoom;
        self.zoom_by(factor);
        if (self.zoom - before).abs() < f32::EPSILON {
            return;
        }
        let zoom_after = self.painted_zoom().max(0.0001);
        self.center_x = world_x - (anchor.x as f32 - center_px) / zoom_after;
        self.center_y =
            self.unproject_world_y(world_y - (anchor.y as f32 - center_py) / zoom_after);
        self.invalidate_status();
        self.center_changed.emit((self.center_x, self.center_y));
        // The centre moved after `set_zoom` already asked for tiles, so the viewport the host is
        // asked for is refreshed once more at the final camera (SP-P60).
        self.refresh_tiles();
        self.base.request_redraw();
    }

    /// Sets zoom level.
    pub fn set_zoom(&mut self, zoom: f32) {
        if !zoom.is_finite() {
            return;
        }
        let next = zoom.clamp(0.25, 8.0);
        if (self.zoom - next).abs() < f32::EPSILON {
            return;
        }
        // # Why the "from" endpoint is read before `self.zoom` moves
        //
        // `painted_zoom` interpolates between `zoom_from` and `self.zoom`. Reading it *after*
        // assigning the new target makes the two endpoints the same number, so the interpolation
        // is constant and the transition paints its destination on every frame — the control
        // animates (`is_animating` is true, `tick` advances) while the picture never moves. That is
        // the same split `segmented_control`'s teleporting pill had, and the grid-line count was
        // the measurement that caught it: 28 lines before, 8 immediately, 8 forever.
        self.zoom_from = self.painted_zoom();
        self.zoom = next;
        // Resume from wherever the picture currently is, so a second zoom mid-flight does not
        // snap back to the previous target — the continuity rule `segmented_control` follows.
        self.zoom_t.jump_to(0.0);
        self.zoom_t.set_target(1.0);
        self.invalidate_status();
        self.zoom_changed.emit(next);
        self.refresh_tiles();
        self.base.request_redraw();
    }

    /// Zooms around current center.
    pub fn zoom_by(&mut self, factor: f32) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        self.set_zoom(self.zoom * factor);
    }

    /// Replaces map markers.
    pub fn set_markers(&mut self, markers: Vec<MapMarker>) {
        self.markers = markers;
        self.markers_revision = self.markers_revision.wrapping_add(1);
        if let Some(index) = self.selected_marker {
            if index >= self.markers.len() {
                self.selected_marker = None;
            }
        }
        self.base.request_redraw();
    }

    /// Returns marker list.
    pub fn markers(&self) -> &[MapMarker] {
        &self.markers
    }

    /// Returns selected marker id.
    pub fn selected_marker_id(&self) -> Option<&str> {
        let index = self.selected_marker?;
        self.markers.get(index).map(|marker| marker.id.as_str())
    }

    /// The status line, rebuilt only when the camera it names changes (SP-P59).
    fn status_line(&self) -> Ref<'_, str> {
        let key = ((self.center_x, self.center_y), self.zoom);
        let stale = !matches!(self.status_cache.borrow().as_ref(), Some(cache) if cache.key == key);
        if stale {
            let text = format!(
                "Center ({:.1}, {:.1})  Zoom {:.2}x",
                self.center_x, self.center_y, self.zoom
            );
            *self.status_cache.borrow_mut() = Some(StatusCache { key, text });
        }
        Ref::map(self.status_cache.borrow(), |cache| match cache.as_ref() {
            Some(cache) => cache.text.as_str(),
            // Unreachable: the block above always stores an entry.
            None => "",
        })
    }

    /// Paints every delivered tile the viewport covers, with a parent-level fallback.
    ///
    /// A missing tile falls back to the tile one zoom level out, scaled up: that is the same
    /// low-resolution stand-in a slippy map uses while a level loads, and it fills the place the
    /// current level has not arrived for (SP-52). A miss at every level leaves the placeholder grid
    /// showing.
    fn draw_tiles(&self, context: &mut RenderContext, rect: Rect) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let viewport = self.tile_viewport();
        let zoom = viewport.zoom;
        let per_axis = TileKey::tiles_per_axis(zoom) as i64;
        if per_axis <= 0 {
            return;
        }
        let tile_size = self.tile_loader.tile_size().max(1) as f64;
        let tile_world = tile_size / 2f64.powi(i32::from(zoom));
        let painted = self.painted_zoom().max(0.0001) as f64;
        let center_proj_y = self.project_world_y(self.center_y) as f64;
        let center_screen_x = rect.x as f64 + rect.width as f64 / 2.0;
        let center_screen_y = rect.y as f64 + rect.height as f64 / 2.0;
        let tile_px = (tile_world * painted).round().max(1.0) as u32;

        let first_col = viewport.x.floor() as i64;
        let first_row = viewport.y.floor() as i64;
        let cols = (viewport.width as f64 / tile_size).ceil() as i64 + 2;
        let rows = (viewport.height as f64 / tile_size).ceil() as i64 + 2;
        // Parents already painted this frame, so a fallback tile is blitted once rather than once
        // per child that needs it.
        let mut painted_parents: Vec<TileKey> = Vec::new();

        for row in 0..rows {
            for col in 0..cols {
                let x = first_col + col;
                let y = first_row + row;
                if x < 0 || y < 0 || x >= per_axis || y >= per_axis {
                    continue;
                }
                let screen_x =
                    center_screen_x + (x as f64 * tile_world - f64::from(self.center_x)) * painted;
                let screen_y = center_screen_y + (y as f64 * tile_world - center_proj_y) * painted;
                let screen_x = screen_x.round() as i32;
                let screen_y = screen_y.round() as i32;
                let key = TileKey::new(zoom, x as u32, y as u32);
                if let Some(pixels) = self.tile_loader.tile_pixels(key) {
                    context.draw_image(screen_x, screen_y, tile_px, tile_px, pixels);
                    continue;
                }
                if zoom == 0 {
                    continue;
                }
                let parent = TileKey::new(zoom - 1, (x / 2) as u32, (y / 2) as u32);
                if painted_parents.contains(&parent) {
                    continue;
                }
                let Some(pixels) = self.tile_loader.tile_pixels(parent) else {
                    continue;
                };
                painted_parents.push(parent);
                // The parent covers a 2×2 block of the child level, so it is drawn at twice the
                // size, anchored at the parent's own origin.
                let parent_world = tile_world * 2.0;
                let parent_x = center_screen_x
                    + (f64::from(parent.x) * parent_world - f64::from(self.center_x)) * painted;
                let parent_y = center_screen_y
                    + (f64::from(parent.y) * parent_world - center_proj_y) * painted;
                let size = (tile_px as f64 * 2.0).round().max(1.0) as u32;
                context.draw_image(
                    parent_x.round() as i32,
                    parent_y.round() as i32,
                    size,
                    size,
                    pixels,
                );
            }
        }
    }

    /// The zoom the projection is actually painted at.
    ///
    /// Interpolates between the zoom the transition started from and the stored target by the
    /// driver's fraction. At rest (`zoom_t == 1.0`, its settled value) this is exactly `self.zoom`,
    /// so a settled map is byte-identical to the un-animated one.
    fn painted_zoom(&self) -> f32 {
        let t = self.zoom_t.value();
        self.zoom_from + (self.zoom - self.zoom_from) * t
    }

    fn world_to_screen(&self, world_x: f32, world_y: f32) -> (f32, f32) {
        let rect = self.geometry();
        // The **painted** zoom, not the stored target: see `painted_zoom`. Everything below derives
        // its scale from this one value, so the pane, the grid and the markers cannot disagree
        // about how big the map currently is.
        let zoom = self.painted_zoom();
        let sx = rect.x as f32 + rect.width as f32 / 2.0 + (world_x - self.center_x) * zoom;
        // Both ends go through the projection: comparing a projected marker against an unprojected
        // centre would paint a Mercator map at the mercator ordinate of the *offset* rather than as
        // an offset from the centre, so every pin would sit off its place except at the equator.
        let sy = rect.y as f32
            + rect.height as f32 / 2.0
            + (self.project_world_y(world_y) - self.project_world_y(self.center_y)) * zoom;
        (sx, sy)
    }

    /// Whether a marker's screen position falls inside the pane.
    ///
    /// # Why a marker can be outside at all
    ///
    /// `world_to_screen` is an affine map, so a marker the user has panned or zoomed away from
    /// lands anywhere — including outside the pane. Nothing clips a widget at this layer, so a
    /// marker painted there draws over whatever the layout put beside the map. Both the paint
    /// and the hit test therefore ask this first, which is what keeps the ink and the clickable
    /// area the same set: a marker outside the pane has neither a pin nor a hit box.
    fn marker_is_inside(&self, index: usize) -> bool {
        let rect = self.geometry();
        let Some(marker) = self.markers.get(index) else {
            return false;
        };
        let (sx, sy) = self.world_to_screen(marker.x, marker.y);
        let (px, py) = (sx.round() as i32, sy.round() as i32);
        // The pin is the marker's own diameter across and centred on the point, so the whole pin —
        // not just its centre — must fit inside the pane.
        let half = self.marker_size(index) as i32 / 2;
        px - half >= rect.x
            && (px + half) < rect.x + rect.width as i32
            && py - half >= rect.y
            && (py + half) < rect.y + rect.height as i32
    }

    fn hit_marker_index(&self, pos: Point) -> Option<usize> {
        let px = pos.x as f32;
        let py = pos.y as f32;
        for index in 0..self.markers.len() {
            if !self.marker_is_inside(index) {
                continue;
            }
            let Some(marker) = self.markers.get(index) else {
                continue;
            };
            let (sx, sy) = self.world_to_screen(marker.x, marker.y);
            let dx = sx - px;
            let dy = sy - py;
            // The hit radius follows the pin's own size: a 24 px icon with an 8 px hit box is a
            // target the user can see and cannot press.
            let reach = (self.marker_size(index) as f32 / 2.0).max(2.0) + 2.0;
            if dx * dx + dy * dy <= reach * reach {
                return Some(index);
            }
        }
        None
    }

    /// Groups markers into screen-space grid buckets.
    ///
    /// # Why a grid instead of pair-wise greedy grouping
    ///
    /// The previous grouping compared every unclaimed marker with every later one, which is
    /// quadratic in the number of pins and ran on every frame — hundreds of markers made the map's
    /// own paint the slowest thing on the frame. A bucket per `CELL` by `CELL` pixel square is one
    /// pass: each marker falls into exactly one cell, and a cell's members are its cluster. The
    /// cell is derived from the painted zoom, so zooming in separates markers naturally. Markers
    /// are not re-grouped incrementally: the projection changes every frame of a zoom animation, so
    /// a cache keyed on the camera would be invalidated as often as it was used.
    fn clusters(&self) -> Rc<Vec<MapCluster>> {
        let key = ClusterKey {
            painted_zoom: self.painted_zoom(),
            center: (self.center_x, self.center_y),
            rect: self.geometry(),
            markers_revision: self.markers_revision,
            projection: self.projection,
        };
        if let Some((cached, clusters)) = self.cluster_cache.borrow().as_ref() {
            if *cached == key {
                // The paint and the hit test share one projection pass (SP-P58).
                return Rc::clone(clusters);
            }
        }
        let clusters = Rc::new(self.compute_clusters());
        *self.cluster_cache.borrow_mut() = Some((key, Rc::clone(&clusters)));
        clusters
    }

    /// Projects the markers into screen-space buckets, without consulting the cache.
    fn compute_clusters(&self) -> Vec<MapCluster> {
        const CELL: f32 = 36.0;
        let mut order: Vec<Vec<usize>> = Vec::new();
        let mut cells: HashMap<(i32, i32), usize> = HashMap::new();
        for index in 0..self.markers.len() {
            if !self.marker_is_inside(index) {
                continue;
            }
            let Some(marker) = self.markers.get(index) else {
                continue;
            };
            let (sx, sy) = self.world_to_screen(marker.x, marker.y);
            let key = ((sx / CELL).floor() as i32, (sy / CELL).floor() as i32);
            match cells.get(&key).copied() {
                Some(bucket) => order[bucket].push(index),
                None => {
                    cells.insert(key, order.len());
                    order.push(vec![index]);
                }
            }
        }
        order
            .into_iter()
            .filter_map(|indices| {
                let mut screen = (0.0f32, 0.0f32);
                let mut world = (0.0f32, 0.0f32);
                for index in &indices {
                    let marker = self.markers.get(*index)?;
                    let (sx, sy) = self.world_to_screen(marker.x, marker.y);
                    screen.0 += sx;
                    screen.1 += sy;
                    world.0 += marker.x;
                    world.1 += marker.y;
                }
                let count = indices.len() as f32;
                Some(MapCluster {
                    indices,
                    screen: (screen.0 / count, screen.1 / count),
                    world: (world.0 / count, world.1 / count),
                })
            })
            .collect()
    }

    /// The multi-marker cluster under `pos`, if the pointer is on its badge.
    fn cluster_at(&self, pos: Point) -> Option<MapCluster> {
        if !self.cluster_markers {
            return None;
        }
        let px = pos.x as f32;
        let py = pos.y as f32;
        self.clusters().iter().find_map(|cluster| {
            if cluster.indices.len() < 2 {
                return None;
            }
            let dx = cluster.screen.0 - px;
            let dy = cluster.screen.1 - py;
            // The badge is drawn with a 9 px radius; the hit box adds the pointer's own reach.
            (dx * dx + dy * dy <= 12.0 * 12.0).then(|| cluster.clone())
        })
    }

    fn select_marker(&mut self, index: usize) -> bool {
        if index >= self.markers.len() {
            return false;
        }
        if self.selected_marker == Some(index) {
            // A click on the already-selected pin is still a click: the state did not change, but
            // the act did.
            if let Some(marker) = self.markers.get(index) {
                self.marker_clicked.emit(marker.id.clone());
            }
            return true;
        }
        self.selected_marker = Some(index);
        if let Some(marker) = self.markers.get(index) {
            self.marker_selected.emit(marker.id.clone());
            self.marker_clicked.emit(marker.id.clone());
        }
        self.base.request_redraw();
        true
    }
}

impl MapView {
    /// Paints one marker: its pin (or icon) and its label.
    ///
    /// The paint and the hit test read the same `marker_is_inside` and `marker_size`, so a pin's ink
    /// and its clickable area cannot disagree about either position or extent.
    fn draw_marker(
        &self,
        context: &mut RenderContext,
        index: usize,
        style: &crate::style::WidgetStyle,
        ink: Color,
    ) {
        // A marker panned or zoomed out of the pane is not drawn: its pin would land over the
        // neighbouring widget, since nothing clips a widget at this layer. The hit test asks the
        // same question, so an invisible pin is also not clickable.
        if !self.marker_is_inside(index) {
            return;
        }
        let Some(marker) = self.markers.get(index) else {
            return;
        };
        let rect = self.geometry();
        let (sx, sy) = self.world_to_screen(marker.x, marker.y);
        let size = self.marker_size(index);
        let half = size as i32 / 2;
        // Selection is a state of the map, so it overrides the marker's own colour; the marker's
        // colour is its kind, which the user still needs to read from an unselected pin.
        let color = if self.selected_marker == Some(index) {
            MAP_MARKER_SELECTED_COLOR
        } else {
            marker.style.color
        };
        if let Some(icon) = marker.style.icon {
            let slot = Rect::new(sx as i32 - half, sy as i32 - half, size, size);
            if let Err(error) = crate::widget::paint_icon(context, icon.into(), slot, color) {
                log::warn!("map marker '{}' icon failed: {error:?}", marker.id);
            }
        } else {
            // A pin, not a square: a circle over a tapering tip points at the place the marker
            // names, which a square centred on the coordinate does not. The tip is a three-point path
            // from a stack array, so a marker does not allocate its own vector per frame (SP-P59).
            let head = Point::new(sx as i32, sy as i32 - half);
            context.fill_circle(head, half.max(2) as u32, color);
            let tip = [
                Point::new(sx as i32 - half / 2, sy as i32 - 1),
                Point::new(sx as i32 + half / 2, sy as i32 - 1),
                Point::new(sx as i32, sy as i32 + half),
            ];
            context.draw_path(&tip, true, color, true, 0);
        }
        // A marker label is bounded by the pane: it is placed beside a point that may sit
        // anywhere, so unbounded it could start inside and end past the edge.
        let label_left = (sx as i32) + half + 3;
        let label_width = (rect.x + rect.width as i32 - label_left).max(0) as u32;
        if label_width > 0 {
            context.draw_text_fitted_cow(
                Rect::new(label_left, sy as i32 - half, label_width, 14),
                &marker.label,
                effective_font(style),
                ink,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl Widget for MapView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(320, 240)
    }

    /// Advances the painted zoom toward its target; `true` while it still owes frames.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.zoom_t.tick(delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.zoom_t.is_moving()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "center_changed" => {
                Some(EventSignalRef::mapped("center_changed", &self.center_changed, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::Float(v.0 as f64),
                        CapabilityValue::Float(v.1 as f64),
                    ])
                }))
            }
            "zoom_changed" => {
                Some(EventSignalRef::mapped("zoom_changed", &self.zoom_changed, |v| {
                    CapabilityValue::Float(*v as f64)
                }))
            }
            "marker_selected" => {
                Some(EventSignalRef::mapped("marker_selected", &self.marker_selected, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "marker_clicked" => {
                Some(EventSignalRef::mapped("marker_clicked", &self.marker_clicked, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "cluster_clicked" => {
                Some(EventSignalRef::mapped("cluster_clicked", &self.cluster_clicked, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::Float(v.0 as f64),
                        CapabilityValue::Float(v.1 as f64),
                    ])
                }))
            }
            "tile_requested" => {
                Some(EventSignalRef::mapped("tile_requested", &self.tile_requested, |request| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(request.key.zoom as u64),
                        CapabilityValue::UInt(request.key.x as u64),
                        CapabilityValue::UInt(request.key.y as u64),
                        CapabilityValue::String(request.url.clone()),
                    ])
                }))
            }
            _ => None,
        }
    }
}

/// `MapView`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `MapView` reports
/// `WidgetKind::Canvas`, shared with `Canvas`; dispatching on the concrete type
/// here is what keeps the two contracts separate.
impl WidgetProperties for MapView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The whole marker list as records (BLUE26 B26.09): `set_markers` takes a list the bare
            // command channel cannot carry, so the list is the property and `set_markers` is its
            // published command form.
            "markers" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &MAP_MARKER_SCHEMA,
                    self.markers()
                        .iter()
                        .map(map_marker_record)
                        .collect::<Result<Vec<_>, CapabilityAccessError>>()?,
                )))
            }
            "center_x" => Ok(CapabilityValue::Float(self.center().0 as f64)),
            "center_y" => Ok(CapabilityValue::Float(self.center().1 as f64)),
            "zoom" => Ok(CapabilityValue::Float(self.zoom() as f64)),
            "marker_count" => Ok(CapabilityValue::UInt(self.markers().len() as u64)),
            "selected_marker_id" => match self.selected_marker_id() {
                Some(id) => Ok(CapabilityValue::String(id.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            "projection" => Ok(CapabilityValue::String(self.projection().token().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "center_x" => {
                let x = expect_f32(value)?;
                let (_, y) = self.center();
                self.set_center(x, y);
                Ok(())
            }
            "center_y" => {
                let y = expect_f32(value)?;
                let (x, _) = self.center();
                self.set_center(x, y);
                Ok(())
            }
            "zoom" => {
                self.set_zoom(expect_f32(value)?);
                Ok(())
            }
            "projection" => {
                let token = match value {
                    CapabilityValue::String(token) => token,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                let projection =
                    MapProjection::from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_projection(projection);
                Ok(())
            }
            // Derived counts and the selection identity are computed from the marker
            // list, so they are refused as read-only rather than reported as names
            // this control does not know.
            "marker_count" | "selected_marker_id" => Err(CapabilityAccessError::ReadOnlyProperty),
            // A whole-list replacement through the control's own `set_markers`, which bumps the
            // marker revision for the changed signal.
            "markers" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&MAP_MARKER_SCHEMA).map_err(|violation| {
                    log::warn!("map_view.markers refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("markers"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let markers = list.items().iter().map(map_marker_from_record).collect::<Result<
                    Vec<_>,
                    CapabilityAccessError,
                >>(
                )?;
                self.set_markers(markers);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "markers",
            "center_x",
            "center_y",
            "zoom",
            "marker_count",
            "selected_marker_id",
            "projection",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Reports the one published command that names its payload through a property.
    ///
    /// `set_markers` replaces the whole marker list through [`Self::set_markers`]; the list
    /// travels through the `markers` records property (B26.03), so a bare invocation reports
    /// that the value is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_markers" => Err(CapabilityAccessError::OutOfRange),
            _ => self.default_command(name),
        }
    }
}

impl EventHandler for MapView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        // The overlay controls sit on top of the map: their own boxes are resolved before the map
        // considers a pan, a cluster or a marker, so a press on `+` never also starts a drag. Only
        // the geometry is ensured here; the camera mirror runs once per paint (SP-P59).
        self.layout_map_controls();

        match event {
            Event::Wheel { delta, .. } => {
                // Zoom around the pointer rather than the centre: the place under the cursor is
                // what the user is looking at, and a centre-anchored zoom slides it away.
                let anchor = self.last_pointer.unwrap_or_else(|| {
                    let rect = self.geometry();
                    Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2)
                });
                if delta.y < 0 {
                    self.zoom_at(1.15, anchor);
                } else if delta.y > 0 {
                    self.zoom_at(0.87, anchor);
                }
            }
            Event::MouseMove { pos } => {
                self.last_pointer = Some(*pos);
                if let Some((origin, center_x, center_y)) = self.drag {
                    let zoom = self.painted_zoom().max(0.0001);
                    // `center_y` is held in projected space for the duration of the drag, so a
                    // Mercator map pans smoothly instead of accelerating as the pointer climbs.
                    self.set_center(
                        center_x - (pos.x - origin.x) as f32 / zoom,
                        self.unproject_world_y(center_y - (pos.y - origin.y) as f32 / zoom),
                    );
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                self.drag = None;
            }
            // A two-finger pinch is a zoom around the gesture's midpoint. The recogniser already
            // supplies the anchor, so the same pointer-anchored path the wheel uses applies.
            #[cfg(feature = "touch")]
            Event::Pinch { scale, center, .. } => {
                if scale.is_finite() && *scale > 0.0 {
                    self.last_pointer = Some(*center);
                    self.zoom_at(*scale, *center);
                }
            }
            Event::MouseLeave { .. } => {
                self.drag = None;
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                37 => self.pan_by(-20.0 / self.zoom, 0.0),
                38 => self.pan_by(0.0, -20.0 / self.zoom),
                39 => self.pan_by(20.0 / self.zoom, 0.0),
                40 => self.pan_by(0.0, 20.0 / self.zoom),
                187 | 107 => self.zoom_by(1.12),
                189 | 109 => self.zoom_by(0.9),
                _ => { /* Other keys are not relevant */ }
            },
            Event::MousePress { pos, button: 1, .. } => {
                if self.show_map_controls {
                    if self.zoom_control.base().geometry().contains_point(*pos) {
                        let before = self.zoom_control.zoom_level();
                        self.zoom_control.handle_event(event);
                        let after = self.zoom_control.zoom_level();
                        if (after - before).abs() > f32::EPSILON {
                            self.set_zoom(after);
                        }
                        return;
                    }
                    if self.compass.base().geometry().contains_point(*pos) {
                        self.compass.handle_event(event);
                        // The compass's reset request means north-up; the map has no rotation of
                        // its own, so the needle returning to 0 is the whole effect here.
                        self.compass.set_bearing(0.0);
                        return;
                    }
                }
                // A cluster badge is a target of its own: pressing one reports the group it stands
                // for instead of selecting whichever member happens to be nearest.
                if let Some(cluster) = self.cluster_at(*pos) {
                    self.cluster_clicked.emit(cluster.world);
                    self.base.request_redraw();
                    return;
                }
                if let Some(index) = self.hit_marker_index(*pos) {
                    let _ = self.select_marker(index);
                } else {
                    // A press on empty map starts a pan; a press on a pin selects instead.
                    self.drag = Some((*pos, self.center_x, self.project_world_y(self.center_y)));
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for MapView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // The pane, its border, the grid and the status line are **chrome** — they frame the
        // geographic content rather than being it — so they resolve the theme instead of
        // four literals. The previous literals were all light, which is why a map in a dark
        // window was a white rectangle: the control could not follow the appearance at all.
        // The tile grid and the markers keep their own colours, because those encode place
        // and selection (rule #108 ③).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("map_view");
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(45, 58, 74));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgb(168, 184, 201));
        // A pane whose resolved fill is the window's own would be indistinguishable from the
        // frame behind it, so a surface equal to the theme background is stepped toward the
        // ink — the same distinction `splash_screen` makes.
        let window_fill =
            crate::style::theme_manager().current_theme().map(|active| active.colors.background);
        let surface = match window_fill {
            Some(window_fill) if window_fill == ink => ink.blend(&border, 0.5),
            _ => style
                .background_color
                .or_else(|| theme.as_ref().and_then(|t| t.background_color))
                .unwrap_or(Color::rgb(235, 243, 250)),
        };
        let surface = match window_fill {
            Some(window_fill) if surface == window_fill => window_fill.blend(&ink, 0.08),
            _ => surface,
        };
        let grid = surface.blend(&ink, 0.12);
        context.face_with_gradient_corners(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        // Draw coarse map grid for pan/zoom visual feedback.
        //
        // # Why the grid reads the *painted* zoom
        //
        // It read `self.zoom` — the stored target — so the one piece of this control's chrome that
        // exists to show the scale snapped to the destination on the first frame while
        // `is_animating` reported a transition in flight. That is the same split
        // `segmented_control`'s teleporting pill had, and it is why the grid is the quantity the
        // animation test samples: it is the only mark on the pane whose geometry follows the zoom.
        //
        // The step's floor keeps the lines from crowding into a solid field at low zoom; its
        // **ceiling** is above the 8.0 `set_zoom` allows, because clamping it at 2.0 made the grid
        // stop changing part-way through a zoom-in — the marks froze while the map was still
        // scaling, which is the very stillness this line exists to avoid. The first version of this
        // fix read the painted zoom but kept the old ceiling, and the probe still saw identical
        // frames: a measurement that named the right quantity and then threw it away on a clamp.
        let step = (40.0 * self.painted_zoom().clamp(0.5, 8.0)) as i32;
        if step > 8 {
            // The grid is a fixed background: it changes only with the geometry, the step or the
            // resolved colours, so it is painted inside a render-cached layer and reused while
            // none of those move (SP-P59).
            let key =
                self.grid_cache_key ^ color_key(grid) ^ color_key(surface) ^ ((step as u64) << 44);
            context.push_layer_group(rect, None, 1.0, Some(key));
            let mut x = rect.x;
            while x < rect.x + rect.width as i32 {
                context.draw_line(
                    Point::new(x, rect.y),
                    Point::new(x, rect.y + rect.height as i32),
                    grid,
                );
                x += step;
            }

            let mut y = rect.y;
            while y < rect.y + rect.height as i32 {
                context.draw_line(
                    Point::new(rect.x, y),
                    Point::new(rect.x + rect.width as i32, y),
                    grid,
                );
                y += step;
            }
            context.pop_layer();
        }

        // Tiles the host has delivered are painted over the placeholder grid. They were cached and
        // never drawn at all — the map was a plain grid with pins regardless of how many tiles the
        // host had supplied (SP-52).
        self.draw_tiles(context, rect);

        if self.cluster_markers {
            // Grid-bucket grouping in screen space: each marker belongs to one 36 px cell and a
            // cell's members are its badge. See `compute_clusters` for why this replaced pair-wise
            // greedy grouping. A one-member cluster is drawn as an ordinary pin, so clustering
            // never hides a marker that is nowhere near another. The list is the shared cache the
            // hit test reads (SP-P58).
            let clusters = self.clusters();
            for cluster in clusters.iter() {
                let (sx, sy) = cluster.screen;
                if cluster.indices.len() > 1 {
                    context.fill_circle(Point::new(sx as i32, sy as i32), 9, MAP_MARKER_COLOR);
                    let count = cluster.indices.len().to_string();
                    let metrics = context.measure_text(&count, effective_font(style));
                    context.draw_text(
                        Point::new(sx as i32 - metrics.width as i32 / 2, sy as i32 - 6),
                        &count,
                        effective_font(style),
                        Color::WHITE,
                        HorizontalAlignment::Left,
                    );
                } else if let Some(index) = cluster.indices.first().copied() {
                    self.draw_marker(context, index, style, ink);
                }
            }
        } else {
            for index in 0..self.markers.len() {
                self.draw_marker(context, index, style, ink);
            }
        }

        // Scoped so the cache borrow is released before the overlay controls take `&mut self`.
        {
            let status = self.status_line();
            context.draw_text_fitted_cow(
                Rect::new(rect.x + 8, rect.y + 4, rect.width.saturating_sub(16), 14),
                &status,
                effective_font(style),
                ink,
                HorizontalAlignment::Left,
            );
        }

        // The controls are chrome over the geographic content, so they are painted last and read
        // the same resolved appearance the pane does.
        self.layout_map_controls();
        self.sync_map_controls();
        if self.show_map_controls {
            self.zoom_control.draw(context);
            self.compass.draw(context);
            self.scale_bar.draw(context);
        }
    }
}

// The four companions live in a sibling file so this one stays under the
// 2000-line production limit (principle #9). Re-exporting keeps their public
// path unchanged: `map_view::TileLoader` and friends still resolve.
#[path = "map_view_companions.rs"]
mod companions;
pub use companions::{
    Compass, ScaleBar, ScaleBarStyle, ScaleUnits, TileKey, TileLoadError, TileLoader, TileRequest,
    TileViewport, ZoomButton, ZoomControl, MAX_TILE_ZOOM,
};

// The anchored information window lives in a sibling file for the same size reason as the four
// companions above; re-exporting keeps its public path stable.
#[path = "map_marker_detail.rs"]
mod marker_detail;
pub use marker_detail::{DetailLine, MapMarkerDetail};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn pan_and_zoom_update_state() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        map.pan_by(15.0, -8.0);
        map.zoom_by(1.5);

        let (cx, cy) = map.center();
        assert!((cx - 15.0).abs() < 0.001);
        assert!((cy + 8.0).abs() < 0.001);
        assert!((map.zoom() - 1.5).abs() < 0.001);
    }

    #[test]
    fn wheel_event_changes_zoom() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        let before = map.zoom();
        map.handle_event(&Event::wheel(0, -120, 0));
        let after_in = map.zoom();
        assert!(after_in > before);

        map.handle_event(&Event::wheel(0, 120, 0));
        let after_out = map.zoom();
        assert!(after_out < after_in);
    }

    #[test]
    fn marker_selection_emits_signal() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        map.set_markers(vec![MapMarker::new("m1", "Alpha", 0.0, 0.0)]);

        let selected = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = selected.clone();
        map.marker_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        map.handle_event(&Event::mouse_press(200, 120, 1));

        assert_eq!(map.selected_marker_id(), Some("m1"));
        let got = selected.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["m1".to_string()]);
    }

    #[test]
    fn new_creates_default_state() {
        let map = MapView::new(Rect::new(0, 0, 800, 600));
        assert_eq!(map.center(), (0.0, 0.0));
        assert_eq!(map.zoom(), 1.0);
        assert!(map.markers().is_empty());
        assert_eq!(map.selected_marker_id(), None);
    }

    #[test]
    fn set_center_delta_guard() {
        let mut map = MapView::new(Rect::new(0, 0, 800, 600));
        map.set_center(10.0, 20.0);
        assert_eq!(map.center(), (10.0, 20.0));

        // Same value should be no-op
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        map.center_changed.connect(move |_| {
            if let Ok(mut guard) = sink.lock() {
                guard.push("changed".to_string());
            }
        });
        map.set_center(10.0, 20.0);
        let got = emitted.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(got.len(), 0);
    }

    #[test]
    fn pan_by_zero_is_no_op() {
        let mut map = MapView::new(Rect::new(0, 0, 800, 600));
        map.set_center(5.0, 5.0);
        map.pan_by(0.0, 0.0);
        assert_eq!(map.center(), (5.0, 5.0));
    }

    #[test]
    fn set_zoom_clamps_lower_bound() {
        let mut map = MapView::new(Rect::new(0, 0, 800, 600));
        map.set_zoom(0.1);
        assert!((map.zoom() - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn set_zoom_clamps_upper_bound() {
        let mut map = MapView::new(Rect::new(0, 0, 800, 600));
        map.set_zoom(12.0);
        assert!((map.zoom() - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn set_zoom_guard_no_op() {
        let mut map = MapView::new(Rect::new(0, 0, 800, 600));
        map.set_zoom(2.0);
        let emitted = Arc::new(Mutex::new(Vec::<f32>::new()));
        let sink = emitted.clone();
        map.zoom_changed.connect(move |z| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*z);
            }
        });
        map.set_zoom(2.0);
        let got = emitted.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(got.len(), 0);
    }

    #[test]
    fn zoom_by_negative_factor_does_nothing() {
        let mut map = MapView::new(Rect::new(0, 0, 800, 600));
        let before = map.zoom();
        map.zoom_by(-1.0);
        assert!((map.zoom() - before).abs() < f32::EPSILON);
    }

    #[test]
    fn set_markers_replaces_and_clears_invalid_selection() {
        let mut map = MapView::new(Rect::new(0, 0, 800, 600));
        map.set_markers(vec![MapMarker::new("m1", "A", 0.0, 0.0)]);
        // Select marker index 0
        let _ = map.select_marker(0);
        assert_eq!(map.selected_marker_id(), Some("m1"));

        // Replace with different markers - selection should clear
        map.set_markers(vec![MapMarker::new("m2", "B", 10.0, 10.0)]);
        // Old selection index (0) is still valid, but it's a different marker now
        // Let's actually test that old selection index is still 0
        assert_eq!(map.selected_marker_id(), Some("m2"));
    }

    #[test]
    fn empty_markers_no_crash() {
        let mut map = MapView::new(Rect::new(0, 0, 800, 600));
        assert!(map.markers().is_empty());
        assert_eq!(map.selected_marker_id(), None);
        // Selecting index on empty markers should not panic
        assert!(!map.select_marker(0));
    }

    #[test]
    fn select_marker_invalid_index() {
        let mut map = MapView::new(Rect::new(0, 0, 800, 600));
        map.set_markers(vec![MapMarker::new("m1", "A", 0.0, 0.0)]);
        assert!(!map.select_marker(5));
        assert_eq!(map.selected_marker_id(), None);
    }

    #[test]
    fn center_changed_signal_emits_on_pan() {
        let mut map = MapView::new(Rect::new(0, 0, 800, 600));
        let centers = Arc::new(Mutex::new(Vec::<(f32, f32)>::new()));
        let sink = centers.clone();
        map.center_changed.connect(move |c| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*c);
            }
        });
        map.pan_by(10.0, -5.0);
        let got = centers.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(got, vec![(10.0, -5.0)]);
    }

    #[test]
    fn zoom_changed_signal_emits_on_set() {
        let mut map = MapView::new(Rect::new(0, 0, 800, 600));
        let zooms = Arc::new(Mutex::new(Vec::<f32>::new()));
        let sink = zooms.clone();
        map.zoom_changed.connect(move |z| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*z);
            }
        });
        map.set_zoom(2.5);
        let got = zooms.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert!((got[0] - 2.5).abs() < f32::EPSILON);
    }

    /// A marker panned outside the pane is neither drawn nor clickable.
    ///
    /// The defect this pins: `world_to_screen` is affine, so a marker a long way from the
    /// centre lands outside the pane, and the pin was painted there unconditionally — over
    /// whatever the layout put beside the map, since nothing clips a widget at this layer.
    /// The hit test asks the same question, so an invisible pin has no hit box either.
    #[test]
    fn a_marker_outside_the_pane_has_neither_pin_nor_hit_box() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        map.set_zoom(1.0);
        map.set_markers(vec![
            MapMarker::new("centre", "Centre", 0.0, 0.0),
            // Far outside at this zoom: (5000, -4000) maps past the pane's edges.
            MapMarker::new("far", "Far away", 5000.0, -4000.0),
        ]);

        assert!(map.marker_is_inside(0), "the centred marker is in the pane");
        assert!(!map.marker_is_inside(1), "the far marker is not");
        // A press at the point the far marker would project to must not select it.
        let (sx, sy) = map.world_to_screen(5000.0, -4000.0);
        assert!(
            map.hit_marker_index(Point::new(sx as i32, sy as i32)).is_none(),
            "a marker outside the pane must not be selectable"
        );
    }

    #[test]
    fn non_finite_inputs_rejected() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        map.set_center(f32::NAN, 10.0);
        assert_eq!(map.center(), (0.0, 0.0));
        map.set_center(10.0, f32::INFINITY);
        assert_eq!(map.center(), (0.0, 0.0));

        map.set_zoom(f32::NAN);
        assert_eq!(map.zoom(), 1.0);
        map.set_zoom(f32::INFINITY);
        assert_eq!(map.zoom(), 1.0);

        map.zoom_by(f32::NAN);
        assert_eq!(map.zoom(), 1.0);
        map.pan_by(f32::NAN, 0.0);
        assert_eq!(map.center(), (0.0, 0.0));
    }

    #[test]
    fn add_and_remove_marker_keeps_the_selected_id() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        map.add_marker(MapMarker::new("a", "A", 0.0, 0.0));
        map.add_marker(MapMarker::new("b", "B", 20.0, 0.0));
        assert!(map.select_marker(1));
        assert_eq!(map.selected_marker_id(), Some("b"));

        // Removing a marker above the selection shifts it down, it does not clear it.
        let removed = map.remove_marker("a").expect("a exists");
        assert_eq!(removed.id, "a");
        assert_eq!(map.selected_marker_id(), Some("b"));
        assert_eq!(map.markers().len(), 1);

        // Removing the selected marker clears the selection.
        assert!(map.remove_marker("b").is_some());
        assert_eq!(map.selected_marker_id(), None);
    }

    #[test]
    fn marker_click_emits_even_when_already_selected() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        map.set_markers(vec![MapMarker::new("m1", "Alpha", 0.0, 0.0)]);
        let clicks = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = clicks.clone();
        map.marker_clicked.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        map.handle_event(&Event::mouse_press(200, 120, 1));
        map.handle_event(&Event::mouse_press(200, 120, 1));

        let got = clicks.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["m1".to_string(), "m1".to_string()]);
    }

    #[test]
    fn mercator_projection_pushes_northern_latitudes_away_from_the_centre() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        map.set_markers(vec![MapMarker::new("north", "North", 0.0, 45.0)]);
        let (_, linear_y) = map.world_to_screen(0.0, 45.0);

        map.set_projection(MapProjection::Mercator);
        let (_, mercator_y) = map.world_to_screen(0.0, 45.0);
        assert!(
            mercator_y > linear_y,
            "mercator must stretch the y axis: linear {linear_y}, mercator {mercator_y}"
        );

        // Centring on the latitude puts it back on the middle line, which is what makes the
        // inverse projection correct rather than merely monotone.
        map.set_center(0.0, 45.0);
        let (_, centred_y) = map.world_to_screen(0.0, 45.0);
        assert!((centred_y - 120.0).abs() < 0.5, "centred marker y was {centred_y}");
    }

    #[test]
    fn projection_property_round_trips() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        assert_eq!(map.get("projection").unwrap(), CapabilityValue::String("linear".into()));
        map.set("projection", CapabilityValue::String("mercator".into()))
            .expect("mercator is a published token");
        assert_eq!(map.projection(), MapProjection::Mercator);
        assert!(map.set("projection", CapabilityValue::String("globe".into())).is_err());
    }

    #[test]
    fn cluster_badge_reports_its_world_centre() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        map.set_markers(vec![
            MapMarker::new("a", "A", 0.0, 0.0),
            MapMarker::new("b", "B", 10.0, 10.0),
        ]);
        map.set_cluster_markers(true);

        let clusters = map.clusters();
        assert_eq!(clusters.len(), 1, "nearby pins must form one bucket");
        assert_eq!(clusters[0].indices.len(), 2);

        let centres = Arc::new(Mutex::new(Vec::<(f32, f32)>::new()));
        let sink = centres.clone();
        map.cluster_clicked.connect(move |centre| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*centre);
            }
        });

        let (sx, sy) = clusters[0].screen;
        map.handle_event(&Event::mouse_press(sx as i32, sy as i32, 1));
        let got = centres.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got.len(), 1);
        assert!((got[0].0 - 5.0).abs() < 0.01 && (got[0].1 - 5.0).abs() < 0.01);
    }

    #[test]
    fn marker_style_size_widens_the_hit_box() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        map.set_markers(vec![MapMarker::new("big", "Big", 0.0, 0.0)
            .with_style(MarkerStyle::new(Color::rgb(1, 2, 3), 24))]);
        // 10 px from the centre: outside the default 6 px pin, inside the 24 px one.
        assert!(map.hit_marker_index(Point::new(210, 120)).is_some());
        assert!(map.set_marker_style("big", MarkerStyle::new(Color::rgb(1, 2, 3), 6)));
        assert!(map.hit_marker_index(Point::new(210, 120)).is_none());
    }

    // ── COMP-13: map companions ────────────────────────────────────────────

    #[test]
    fn tile_loader_validates_zoom_and_world_bounds() {
        let mut loader = TileLoader::new(Rect::new(0, 0, 160, 24));
        assert_eq!(TileLoader::new(Rect::new(0, 0, 1, 1)).max_zoom(), 19);
        assert_eq!(loader.load_tile(25, 0, 0), Err(TileLoadError::ZoomOutOfRange));
        // At zoom 2 the world is 4x4, so column 4 is outside it.
        assert_eq!(loader.load_tile(2, 4, 0), Err(TileLoadError::TileOutOfWorld));
        assert_eq!(loader.load_tile(2, 0, 4), Err(TileLoadError::TileOutOfWorld));
        assert!(loader.load_tile(2, 3, 3).is_ok());

        loader.set_zoom_range(5, 3);
        assert_eq!((loader.min_zoom(), loader.max_zoom()), (5, 5));
    }

    #[test]
    fn tile_loader_formats_urls_and_reports_cache_hits() {
        let mut loader = TileLoader::new(Rect::new(0, 0, 160, 24));
        loader.set_url_template("https://example.test/{z}/{x}/{y}.png");

        let first = loader.load_tile(2, 1, 3).expect("zoom 2 is in range");
        assert_eq!(first.url, "https://example.test/2/1/3.png");
        assert!(!first.cached);

        let second = loader.load_tile(2, 1, 3).expect("repeat request");
        assert!(second.cached, "a pending tile is not requested twice");
        assert_eq!(loader.cache_len(), 1);
        assert!(!loader.is_cached(second.key));

        let loaded = Arc::new(Mutex::new(Vec::<TileKey>::new()));
        let sink = loaded.clone();
        loader.tile_loaded.connect(move |key| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*key);
            }
        });
        assert!(loader.deliver_tile(second.key, vec![1, 2, 3]));
        assert!(loader.is_cached(second.key));
        assert_eq!(
            loaded.lock().ok().map(|guard| guard.clone()).unwrap_or_default(),
            vec![TileKey::new(2, 1, 3)]
        );

        // An answer for an address never requested is refused.
        assert!(!loader.deliver_tile(TileKey::new(2, 0, 0), vec![0]));
    }

    #[test]
    fn tile_loader_reports_failures_and_clears_the_cache() {
        let mut loader = TileLoader::new(Rect::new(0, 0, 160, 24));
        let failed = Arc::new(Mutex::new(Vec::<TileKey>::new()));
        let sink = failed.clone();
        loader.tile_failed.connect(move |key| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*key);
            }
        });
        let request = loader.load_tile(1, 0, 0).expect("zoom 1 is in range");
        assert!(loader.reject_tile(request.key));
        assert_eq!(
            failed.lock().ok().map(|guard| guard.clone()).unwrap_or_default(),
            vec![TileKey::new(1, 0, 0)]
        );
        assert_eq!(loader.cache_len(), 0);
        assert!(!loader.reject_tile(request.key));

        let _ = loader.load_tile(1, 1, 1);
        loader.clear_cache();
        assert_eq!(loader.cache_len(), 0);
    }

    #[test]
    fn tile_loader_evicts_least_recently_used_beyond_the_limit() {
        let mut loader = TileLoader::new(Rect::new(0, 0, 160, 24));
        loader.set_cache_limit(2);
        assert!(!loader.load_tile(2, 0, 0).expect("in range").cached);
        assert!(!loader.load_tile(2, 1, 0).expect("in range").cached);
        // Re-asking for `0` makes `1` the least recently used tile.
        assert!(loader.load_tile(2, 0, 0).expect("in range").cached);
        // The third address has to evict something, and LRU eviction keeps the tile the camera
        // has just asked for.
        assert!(!loader.load_tile(2, 2, 0).expect("in range").cached);
        assert_eq!(loader.cache_len(), 2);
        assert!(
            loader.load_tile(2, 0, 0).expect("in range").cached,
            "the most recently used tile survived"
        );
        assert!(
            !loader.load_tile(2, 1, 0).expect("in range").cached,
            "the least recently used tile was evicted"
        );
    }

    #[test]
    fn tile_loader_prefetch_covers_the_viewport() {
        let mut loader = TileLoader::new(Rect::new(0, 0, 160, 24));
        loader.set_tile_size(256);
        // A 256 px window whose top-left sits half a tile into the world crosses
        // both tile boundaries, so four tiles are touched.
        let requests = loader.prefetch(TileViewport::new(2, 0.5, 0.5, 256, 256));
        assert_eq!(requests.len(), 4);
        assert!(requests.iter().all(|request| request.key.zoom == 2));
        let mut keys: Vec<(u32, u32)> =
            requests.iter().map(|request| (request.key.x, request.key.y)).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec![(0, 0), (0, 1), (1, 0), (1, 1)]);

        // A window starting past the world edge requests only what exists.
        assert!(loader.prefetch(TileViewport::new(2, 4.0, 4.0, 256, 256)).is_empty());
        assert!(loader.prefetch(TileViewport::new(2, 0.0, 0.0, 0, 256)).is_empty());
    }

    #[test]
    fn tile_loader_properties_round_trip() {
        let mut loader = TileLoader::new(Rect::new(0, 0, 160, 24));
        loader
            .set("url_template", CapabilityValue::String("t/{z}/{x}/{y}".into()))
            .expect("string template");
        loader.set("tile_size", CapabilityValue::UInt(512)).expect("size");
        loader.set("cache_limit", CapabilityValue::UInt(4)).expect("limit");
        assert_eq!(
            loader.get("url_template").unwrap(),
            CapabilityValue::String("t/{z}/{x}/{y}".into())
        );
        assert_eq!(loader.get("tile_size").unwrap(), CapabilityValue::UInt(512));
        assert_eq!(loader.get("cache_limit").unwrap(), CapabilityValue::UInt(4));
        assert!(loader.set("cache_size", CapabilityValue::UInt(1)).is_err());
        assert!(loader.command("set_tile_size").is_err(), "a command carries no payload");
        assert!(loader.command("clear_cache").is_ok());
    }

    #[test]
    fn zoom_control_steps_and_reports() {
        let mut control = ZoomControl::new(Rect::new(0, 0, 96, 28));
        let requested = Arc::new(Mutex::new(Vec::<f32>::new()));
        let sink = requested.clone();
        control.zoom_requested.connect(move |level| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*level);
            }
        });

        assert!(control.zoom_in());
        assert!((control.zoom_level() - 2.0).abs() < f32::EPSILON);
        assert!(control.zoom_out());
        assert!((control.zoom_level() - 1.0).abs() < f32::EPSILON);
        assert!(control.set_zoom(4.0));
        assert!(control.reset());
        assert!((control.zoom_level() - 1.0).abs() < f32::EPSILON);
        assert!(!control.reset(), "resetting an unchanged level is a no-op");
        assert!(!control.set_zoom(f32::NAN), "non-finite levels are refused");

        let got = requested.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![2.0, 1.0, 4.0, 1.0]);
    }

    #[test]
    fn zoom_control_clamps_level_to_its_range() {
        let mut control = ZoomControl::new(Rect::new(0, 0, 96, 28));
        control.set_zoom(100.0);
        assert!((control.zoom_level() - 8.0).abs() < f32::EPSILON);
        control.set_zoom(0.01);
        assert!((control.zoom_level() - 0.25).abs() < f32::EPSILON);

        control.set_min(1.0);
        assert!((control.min() - 1.0).abs() < f32::EPSILON);
        control.set_max(0.5);
        assert!((control.max() - 1.0).abs() < f32::EPSILON, "the range never inverts");
        assert!((control.zoom_level() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn zoom_control_buttons_are_hit_and_do_their_jobs() {
        let mut control = ZoomControl::new(Rect::new(0, 0, 96, 28));
        assert!(matches!(control.hit_button(Point::new(5, 14)), Some(ZoomButton::Out)));
        assert!(matches!(control.hit_button(Point::new(48, 14)), Some(ZoomButton::Reset)));
        assert!(matches!(control.hit_button(Point::new(92, 14)), Some(ZoomButton::In)));
        assert!(control.hit_button(Point::new(200, 14)).is_none());

        control.handle_event(&Event::mouse_press(92, 14, 1));
        assert!((control.zoom_level() - 2.0).abs() < f32::EPSILON);
        control.handle_event(&Event::mouse_press(5, 14, 1));
        assert!((control.zoom_level() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scale_bar_picks_a_round_distance() {
        let mut bar = ScaleBar::new(Rect::new(0, 0, 400, 28));
        bar.set_scale(10.0);
        // 400 - 2*8 = 384 px * 10 m = 3840 m, rounded down to 2 km.
        assert!((bar.bar_distance(384.0) - 2000.0).abs() < f64::EPSILON);
        assert_eq!(bar.label(), "2 km");
        assert_eq!(bar.bar_pixels(), 200);

        bar.set_units(ScaleUnits::Imperial);
        assert!(bar.label().ends_with("ft") || bar.label().ends_with("mi"));

        bar.set_scale(0.0);
        assert!((bar.meters_per_pixel() - 10.0).abs() < f64::EPSILON, "invalid scales are ignored");
        bar.set_scale(f64::NAN);
        assert!((bar.meters_per_pixel() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scale_bar_properties_round_trip() {
        let mut bar = ScaleBar::new(Rect::new(0, 0, 200, 28));
        bar.set("meters_per_pixel", CapabilityValue::Float(2.5)).expect("scale");
        bar.set("units", CapabilityValue::String("imperial".into())).expect("units");
        bar.set("style", CapabilityValue::String("boxed".into())).expect("style");
        assert_eq!(bar.units(), ScaleUnits::Imperial);
        assert_eq!(bar.style(), ScaleBarStyle::Boxed);
        assert!((bar.meters_per_pixel() - 2.5).abs() < f64::EPSILON);
        assert!(bar.set("units", CapabilityValue::String("nautical".into())).is_err());
    }

    #[test]
    fn compass_emits_a_reset_request_only_when_clickable() {
        let mut compass = Compass::new(Rect::new(0, 0, 48, 48));
        let resets = Arc::new(Mutex::new(0usize));
        let sink = resets.clone();
        compass.reset_north_requested.connect(move || {
            if let Ok(mut guard) = sink.lock() {
                *guard += 1;
            }
        });

        compass.handle_event(&Event::mouse_press(24, 24, 1));
        assert_eq!(resets.lock().map(|guard| *guard).unwrap_or(0), 1);
        assert!((compass.bearing_deg() - 0.0).abs() < f32::EPSILON, "the host owns the bearing");

        compass.set_clickable(false);
        compass.handle_event(&Event::mouse_press(24, 24, 1));
        assert_eq!(resets.lock().map(|guard| *guard).unwrap_or(0), 1);

        compass.set_clickable(true);
        compass.handle_event(&Event::mouse_press(-50, -50, 1));
        assert_eq!(resets.lock().map(|guard| *guard).unwrap_or(0), 1, "outside the dial");
    }

    #[test]
    fn compass_normalises_its_bearing() {
        let mut compass = Compass::new(Rect::new(0, 0, 48, 48));
        compass.set_bearing(-90.0);
        assert!((compass.bearing_deg() - 270.0).abs() < 0.001);
        compass.set_bearing(450.0);
        assert!((compass.bearing_deg() - 90.0).abs() < 0.001);
        compass.set_bearing(f32::NAN);
        assert!((compass.bearing_deg() - 90.0).abs() < 0.001);
        compass.set("bearing_deg", CapabilityValue::Float(180.0)).expect("bearing property");
        assert!((compass.bearing_deg() - 180.0).abs() < 0.001);
    }

    /// The map mounts the zoom strip, compass and scale bar: a press on the strip's `+` cell zooms
    /// the map itself, and the compass' reset request returns the needle to north.
    #[test]
    fn map_controls_are_mounted_and_drive_the_map() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 300));
        assert!(map.map_controls_visible());

        // First event places the companions; the strip is 96 px wide at the top-right, so the `+`
        // cell is the third of three 30 px buttons.
        let before = map.zoom();
        map.handle_event(&Event::mouse_press(370, 12, 1));
        assert!(map.zoom() > before, "the + cell zooms the map");
        assert!((map.zoom_control().zoom_level() - map.zoom()).abs() < 0.0001);

        map.compass_mut().set_bearing(90.0);
        let dial = map.compass().base().geometry();
        map.handle_event(&Event::mouse_press(
            dial.x + dial.width as i32 / 2,
            dial.y + dial.height as i32 / 2,
            1,
        ));
        assert_eq!(map.compass().bearing_deg(), 0.0, "the needle returns to north");
        assert!(map.scale_bar().meters_per_pixel() > 0.0);
    }

    /// Prefetch reports the viewport's tiles without any network activity, and the host's delivery
    /// answer lands in the same loader the map asked through.
    #[test]
    fn map_prefetches_tiles_and_accepts_host_delivery() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 300));
        let requests = map.prefetch_tiles();
        assert!(!requests.is_empty(), "the viewport covers at least one tile");
        assert!(requests.iter().all(|request| !request.url.is_empty()));

        let first = requests[0].clone();
        assert!(map.deliver_tile(first.key, vec![1, 2, 3]));
        assert!(map.tile_loader().is_cached(first.key));
        assert!(!map.reject_tile(TileKey::new(19, 0, 0)), "an unrequested tile is refused");

        map.set_map_controls_visible(false);
        assert!(!map.map_controls_visible());
    }

    /// `markers` round-trips through the record property, including the style fields.
    ///
    /// This is the payload channel `set_markers` never had (B26.03).
    #[test]
    fn markers_round_trip_through_the_property() {
        use crate::widget::capability::{RecordList, RecordValue, EMPTY_RECORD_SCHEMA};

        let mut map = MapView::new(Rect::new(0, 0, 320, 240));
        let list = RecordList::from_items(
            &EMPTY_RECORD_SCHEMA,
            vec![
                RecordValue::new()
                    .with("id", CapabilityValue::String(String::from("a")))
                    .with("label", CapabilityValue::String(String::from("Alpha")))
                    .with("x", CapabilityValue::Float(12.5))
                    .with("y", CapabilityValue::Float(-3.0))
                    .with("size", CapabilityValue::UInt(18)),
                RecordValue::new()
                    .with("id", CapabilityValue::String(String::from("b")))
                    .with("label", CapabilityValue::String(String::from("Beta")))
                    .with("x", CapabilityValue::Float(1.0))
                    .with("y", CapabilityValue::Float(2.0))
                    .with("color", CapabilityValue::Color(Color::rgb(10, 20, 30))),
            ],
        );
        map.set("markers", CapabilityValue::Records(list)).expect("well-formed markers");

        assert_eq!(map.markers().len(), 2);
        assert_eq!(map.markers()[0].label, "Alpha");
        assert_eq!(map.markers()[0].style.size, 18);
        assert_eq!(map.markers()[1].style.color, Color::rgb(10, 20, 30));

        let CapabilityValue::Records(read_back) = map.get("markers").expect("reads back") else {
            panic!("`markers` must read as a record list");
        };
        assert_eq!(read_back.len(), 2);
        assert!(matches!(
            read_back.items()[0].get("x"),
            Some(CapabilityValue::Float(x)) if (*x - 12.5).abs() < f64::EPSILON
        ));

        // A malformed row is refused and changes nothing.
        let broken = RecordList::from_items(
            &EMPTY_RECORD_SCHEMA,
            vec![RecordValue::new().with("id", CapabilityValue::String(String::from("a")))],
        );
        assert!(matches!(
            map.set("markers", CapabilityValue::Records(broken)),
            Err(CapabilityAccessError::ConstraintViolation { .. })
        ));
        assert_eq!(map.markers().len(), 2, "a refused write changes nothing");
    }

    /// SP-021: a pinch zooms around its midpoint, leaving the world point under it in place.
    #[cfg(feature = "touch")]
    #[test]
    fn pinch_zooms_around_the_gesture_midpoint() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 300));
        let anchor = Point::new(150, 140);
        let before_zoom = map.zoom();
        let rect = map.geometry();
        let zoom = before_zoom.max(0.0001);
        let center_px = rect.x as f32 + rect.width as f32 / 2.0;
        let world_before = map.center_x + (anchor.x as f32 - center_px) / zoom;

        map.handle_event(&Event::pinch_at(1.5, anchor.x, anchor.y));

        assert!(map.zoom() > before_zoom, "a spread zooms in");
        let zoom_after = map.painted_zoom().max(0.0001);
        let world_after = map.center_x + (anchor.x as f32 - center_px) / zoom_after;
        assert!(
            (world_before - world_after).abs() < 0.5,
            "the anchored world point stays put: {world_before} vs {world_after}"
        );
    }
}
