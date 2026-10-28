// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ImageView widget — displays an Image as a widget (BLUE13 R2.12).

//!
//! # `enabled`
//!
//! `redraw` / `zoom_redraw` from `attach_zoom_pan` are **deliberately not gated** by `enabled`: a
//! disabled view is still painted, so the companion's change must repaint it. They announce no
//! user action.
#[cfg(full_widgets)]
use crate::compat::{lock, Arc, Mutex};
use crate::compat::{String, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::image::decoder::{decode_async, ImageDecodeJob};
use crate::image::{Image, ImageFormat};
use crate::render::RenderContext;
#[cfg(full_widgets)]
use crate::signal::ConnectionScope;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_f32, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(full_widgets)]
use crate::widget::companions::ZoomPanController;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// How the image is rotated before it is laid out.
///
/// A quarter turn swaps the oriented width and height, so the layout, the size hint and the
/// destination box all read `oriented_dimensions` rather than the source dimensions directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageRotation {
    /// No rotation.
    #[default]
    None,
    /// A quarter turn clockwise.
    Deg90,
    /// A half turn.
    Deg180,
    /// A quarter turn counter-clockwise (three quarters clockwise).
    Deg270,
}

impl ImageRotation {
    /// The token this rotation is published as.
    pub const fn token(self) -> &'static str {
        match self {
            Self::None => "0",
            Self::Deg90 => "90",
            Self::Deg180 => "180",
            Self::Deg270 => "270",
        }
    }

    /// Parses a published token. `None` for a spelling this control does not publish.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "0" => Some(Self::None),
            "90" => Some(Self::Deg90),
            "180" => Some(Self::Deg180),
            "270" => Some(Self::Deg270),
            _ => None,
        }
    }
}

/// The oriented dimensions of a `width` × `height` source under `rotation`.
fn oriented_dimensions(width: u32, height: u32, rotation: ImageRotation) -> (u32, u32) {
    match rotation {
        ImageRotation::None | ImageRotation::Deg180 => (width, height),
        ImageRotation::Deg90 | ImageRotation::Deg270 => (height, width),
    }
}

/// Copies `data` into an oriented RGBA buffer, applying rotation then the two flips.
///
/// The transform is a pure function of the source bytes and the three flags, so the caller caches
/// its result. `(dx, dy)` is the destination pixel a source pixel lands on; flips are applied in
/// the *oriented* space, which is what a viewer expects ("mirror the picture I am looking at").
fn orient_pixels(
    data: &[u8],
    width: u32,
    height: u32,
    rotation: ImageRotation,
    flip_horizontal: bool,
    flip_vertical: bool,
) -> (crate::compat::Vec<u8>, u32, u32) {
    let (oriented_width, oriented_height) = oriented_dimensions(width, height, rotation);
    let mut out = crate::compat::vec![0u8; (oriented_width * oriented_height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let (mut dx, mut dy) = match rotation {
                ImageRotation::None => (x, y),
                ImageRotation::Deg90 => (height - 1 - y, x),
                ImageRotation::Deg180 => (width - 1 - x, height - 1 - y),
                ImageRotation::Deg270 => (y, width - 1 - x),
            };
            if flip_horizontal {
                dx = oriented_width - 1 - dx;
            }
            if flip_vertical {
                dy = oriented_height - 1 - dy;
            }
            let source = ((y * width + x) * 4) as usize;
            let destination = ((dy * oriented_width + dx) * 4) as usize;
            out[destination..destination + 4].copy_from_slice(&data[source..source + 4]);
        }
    }
    (out, oriented_width, oriented_height)
}

/// The largest side of the pyramid level used while a zoom or pan gesture is in flight.
///
/// Interaction resamples at every pointer step; a source larger than this is first reduced to
/// a preview once, and the moving view samples that instead. The rest frame always samples the
/// full-resolution buffer, so the settled picture is not softened by the preview.
const PREVIEW_MAX_SIDE: u32 = 512;

/// How long after a wheel notch the preview level stays in use, in milliseconds.
///
/// A notch has no release event, so it is treated as a gesture that settles on its own. The
/// draw pass keeps submitting frames while this window is open, which is what lets the final
/// full-resolution frame be painted without a second input event.
const WHEEL_PREVIEW_HOLD_MS: u64 = 150;

/// How far one wheel notch changes the zoom. The same factors `ZoomPanController` publishes.
const WHEEL_ZOOM_IN: f32 = 1.25;
const WHEEL_ZOOM_OUT: f32 = 0.8;

/// The transformed, full-resolution pixel grid of one source.
struct OrientedCache {
    source_len: usize,
    rotation: ImageRotation,
    flip_horizontal: bool,
    flip_vertical: bool,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// The reduced pyramid level used while a gesture is in flight.
struct PreviewCache {
    source_len: usize,
    rotation: ImageRotation,
    flip_horizontal: bool,
    flip_vertical: bool,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// Builds the full-resolution transformed grid for one source and transform.
fn build_oriented(
    data: &[u8],
    width: u32,
    height: u32,
    rotation: ImageRotation,
    flip_horizontal: bool,
    flip_vertical: bool,
) -> OrientedCache {
    let identity = rotation == ImageRotation::None && !flip_horizontal && !flip_vertical;
    let (pixels, oriented_width, oriented_height) = if identity {
        (data.to_vec(), width, height)
    } else {
        orient_pixels(data, width, height, rotation, flip_horizontal, flip_vertical)
    };
    OrientedCache {
        source_len: data.len(),
        rotation,
        flip_horizontal,
        flip_vertical,
        width: oriented_width,
        height: oriented_height,
        pixels,
    }
}

/// Builds the reduced pyramid level from the oriented grid, or reuses it when it is current.
fn build_preview(oriented: Option<&OrientedCache>) -> Option<PreviewCache> {
    let oriented = oriented?;
    let longest = oriented.width.max(oriented.height);
    let (width, height) = if longest > PREVIEW_MAX_SIDE {
        let scale = PREVIEW_MAX_SIDE as f32 / longest as f32;
        (
            ((oriented.width as f32 * scale).round() as u32).max(1),
            ((oriented.height as f32 * scale).round() as u32).max(1),
        )
    } else {
        (oriented.width, oriented.height)
    };
    let pixels = if width == oriented.width && height == oriented.height {
        oriented.pixels.clone()
    } else {
        resample_bilinear(&oriented.pixels, oriented.width, oriented.height, width, height)
    };
    Some(PreviewCache {
        source_len: oriented.source_len,
        rotation: oriented.rotation,
        flip_horizontal: oriented.flip_horizontal,
        flip_vertical: oriented.flip_vertical,
        width,
        height,
        pixels,
    })
}

/// A transformed and resampled copy of the image for one destination size.
///
/// The cache key includes the source length, the three transform flags and the destination
/// size, so a still viewer paints from the buffer and only a real change recomputes it.
struct ScaledImageCache {
    source_len: usize,
    rotation: ImageRotation,
    flip_horizontal: bool,
    flip_vertical: bool,
    /// Whether the buffer was sampled from the preview level rather than the full image.
    preview: bool,
    width: u32,
    height: u32,
    pixels: crate::compat::Vec<u8>,
}

/// Resamples a straight-RGBA grid to `dest_width` × `dest_height` bilinearly.
///
/// The source is offset by half a texel so an equal-size request would be the identity (it is
/// not reached — `pixels_for` borrows the source for that case), and edge texels are clamped so
/// a sample at the boundary reads the edge rather than wrapping around.
fn resample_bilinear(
    data: &[u8],
    source_width: u32,
    source_height: u32,
    dest_width: u32,
    dest_height: u32,
) -> crate::compat::Vec<u8> {
    // Both products are checked *before* they become an allocation or a guard: the
    // destination rectangle comes from layout arithmetic, and a bare `u32 * u32 * 4`
    // would wrap on a 32-bit target (the same family as `MediaFrame`'s length check).
    // An impossible size degrades to an empty buffer, which the caller already treats
    // as "nothing to paint".
    let Some(out_len) = (dest_width as usize)
        .checked_mul(dest_height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return crate::compat::Vec::new();
    };
    let mut out = crate::compat::vec![0u8; out_len];
    if source_width == 0 || source_height == 0 || dest_width == 0 || dest_height == 0 {
        return out;
    }
    let Some(source_len) = (source_width as usize)
        .checked_mul(source_height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return out;
    };
    if data.len() < source_len {
        return out;
    }
    let max_x = source_width as i32 - 1;
    let max_y = source_height as i32 - 1;
    let fetch = |x: usize, y: usize| -> [u8; 4] {
        let index = (y * source_width as usize + x) * 4;
        [data[index], data[index + 1], data[index + 2], data[index + 3]]
    };
    for dy in 0..dest_height {
        // Destination pixel centre → source coordinate in texels, then to the two texels that
        // bracket it and the weight between them.
        let fy = ((dy as f64 + 0.5) * source_height as f64 / dest_height as f64 - 0.5) as f32;
        let y0 = fy.floor();
        let ty = fy - y0;
        let y0i = (y0 as i32).clamp(0, max_y) as usize;
        let y1i = (y0 as i32 + 1).clamp(0, max_y) as usize;
        for dx in 0..dest_width {
            let fx = ((dx as f64 + 0.5) * source_width as f64 / dest_width as f64 - 0.5) as f32;
            let x0 = fx.floor();
            let tx = fx - x0;
            let x0i = (x0 as i32).clamp(0, max_x) as usize;
            let x1i = (x0 as i32 + 1).clamp(0, max_x) as usize;
            let top_left = fetch(x0i, y0i);
            let top_right = fetch(x1i, y0i);
            let bottom_left = fetch(x0i, y1i);
            let bottom_right = fetch(x1i, y1i);
            let base = ((dy * dest_width + dx) as usize) * 4;
            for channel in 0..4 {
                let top = top_left[channel] as f32
                    + (top_right[channel] as f32 - top_left[channel] as f32) * tx;
                let bottom = bottom_left[channel] as f32
                    + (bottom_right[channel] as f32 - bottom_left[channel] as f32) * tx;
                let value = top + (bottom - top) * ty;
                out[base + channel] = value.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// How the decoded image is mapped into the widget's box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageAspect {
    /// Scale the image to the box, independently on each axis.
    ///
    /// This is the historic behaviour and the default, so an `ImageView`
    /// that never asks for an aspect renders exactly as it always did.
    #[default]
    Stretch,
    /// Scale the image uniformly until it fits inside the box, leaving
    /// margins (letterboxing) on the axis that has room to spare.
    Fit,
    /// Scale the image uniformly until it covers the box, cropping the
    /// overflow. The crop is centred on the source image.
    Fill,
    /// Repeat the image at its natural size across the box.
    Tile,
}

impl ImageAspect {
    /// Returns the token this aspect is published under.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stretch => "stretch",
            Self::Fit => "fit",
            Self::Fill => "fill",
            Self::Tile => "tile",
        }
    }

    /// Parses an [`ImageAspect`] from its published token, or `None` when the
    /// token names no aspect.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "stretch" => Some(Self::Stretch),
            "fit" => Some(Self::Fit),
            "fill" => Some(Self::Fill),
            "tile" => Some(Self::Tile),
            _ => None,
        }
    }
}

/// Where the image sits inside the widget's box when it does not fill it.
///
/// Alignment is consulted by [`ImageAspect::Stretch`] and [`ImageAspect::Fit`].
/// [`ImageAspect::Fill`] always crops around the source image's centre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageAlignment {
    /// Top-left corner (the default, matching the historic placement).
    #[default]
    TopLeft,
    /// Top edge, horizontally centred.
    Top,
    /// Top-right corner.
    TopRight,
    /// Left edge, vertically centred.
    Left,
    /// Centred on both axes.
    Center,
    /// Right edge, vertically centred.
    Right,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom edge, horizontally centred.
    Bottom,
    /// Bottom-right corner.
    BottomRight,
}

impl ImageAlignment {
    /// Returns the token this alignment is published under.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::Top => "top",
            Self::TopRight => "top_right",
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::BottomLeft => "bottom_left",
            Self::Bottom => "bottom",
            Self::BottomRight => "bottom_right",
        }
    }

    /// Parses an [`ImageAlignment`] from its published token, or `None` when the
    /// token names no alignment.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "top_left" => Some(Self::TopLeft),
            "top" => Some(Self::Top),
            "top_right" => Some(Self::TopRight),
            "left" => Some(Self::Left),
            "center" => Some(Self::Center),
            "right" => Some(Self::Right),
            "bottom_left" => Some(Self::BottomLeft),
            "bottom" => Some(Self::Bottom),
            "bottom_right" => Some(Self::BottomRight),
            _ => None,
        }
    }

    /// The horizontal offset of an image `image_width` wide inside `box_width`.
    fn horizontal_offset(self, box_width: i32, image_width: i32) -> i32 {
        match self {
            Self::TopLeft | Self::Left | Self::BottomLeft => 0,
            Self::Top | Self::Center | Self::Bottom => (box_width - image_width) / 2,
            Self::TopRight | Self::Right | Self::BottomRight => box_width - image_width,
        }
    }

    /// The vertical offset of an image `image_height` tall inside `box_height`.
    fn vertical_offset(self, box_height: i32, image_height: i32) -> i32 {
        match self {
            Self::TopLeft | Self::Top | Self::TopRight => 0,
            Self::Left | Self::Center | Self::Right => (box_height - image_height) / 2,
            Self::BottomLeft | Self::Bottom | Self::BottomRight => box_height - image_height,
        }
    }
}

/// The live link between an [`ImageView`] and a [`ZoomPanController`] (chapter 3, gap 12).
///
/// A widget cannot hold a borrow of another, so the controller's `zoom_changed`/`pan_changed`
/// slots park the latest values in shared cells and [`ImageView::poll_zoom_pan`] applies them on
/// the next paint or interaction. That is the same host-free shape the data grid uses for its
/// companion bindings.
#[cfg(full_widgets)]
struct ZoomPanBinding {
    /// The latest level the controller reported, or `None` when nothing is pending.
    zoom: Arc<Mutex<Option<f32>>>,
    /// The latest pan the controller reported, or `None` when nothing is pending.
    pan: Arc<Mutex<Option<Point>>>,
    /// The subscriptions, dropped with the binding so a detached view leaves no slot behind.
    _scope: ConnectionScope,
}

/// Widget that displays an Image.
pub struct ImageView {
    base: BaseWidget,
    image: Image,
    scaled: bool,
    aspect: ImageAspect,
    image_alignment: ImageAlignment,
    margin: u32,
    background: Option<Color>,
    /// A resampled copy of the image for the destination size the last paint used.
    ///
    /// # Why the control resamples instead of the renderer
    ///
    /// The drawing API a control reaches (`RenderContext::draw_image`) is told the destination
    /// rectangle and the pixel bytes but **not** the source grid, so a backend can only copy — a
    /// downscale became a top-left crop and an upscale drew nothing at all. The control is the one
    /// party that knows both sizes, so it produces a buffer of exactly the destination size. The
    /// cache is keyed by `(source pixels, destination size)` so a still control resamples once.
    scaled_image: Option<ScaledImageCache>,
    /// The full-resolution transformed grid, rebuilt only when the transform or source changes.
    ///
    /// `orient_pixels` walks every source pixel, and the previous code ran it again for every
    /// destination size — a zoom gesture therefore re-oriented the whole image per step.
    oriented_cache: Option<OrientedCache>,
    /// The reduced pyramid level used while a gesture is in flight.
    preview_cache: Option<PreviewCache>,
    /// The last pointer position of an ongoing pan, if any.
    drag_last: Option<Point>,
    /// When a wheel gesture's preview window closes; see [`WHEEL_PREVIEW_HOLD_MS`].
    interaction_until: Option<crate::compat::Instant>,
    /// A decode started by [`ImageView::load_path_async`], polled by `Widget::tick`.
    pending_decode: Option<ImageDecodeJob>,
    /// The image's quarter-turn rotation. Defaults to none.
    rotation: ImageRotation,
    /// Whether the image is mirrored horizontally (after rotation).
    flip_horizontal: bool,
    /// Whether the image is mirrored vertically (after rotation).
    flip_vertical: bool,
    /// The view's zoom, where `1.0` is the unzoomed layout. Defaults to `1.0`.
    zoom_level: f32,
    /// The view's pan, in pixels, applied after zoom. Defaults to the origin.
    pan_offset: Point,
    /// The zoom/pan controller this view follows, when one is attached (chapter 3, gap 12).
    #[cfg(full_widgets)]
    zoom_pan: Option<ZoomPanBinding>,
    /// Emitted whenever [`ImageView::zoom_level`] or [`ImageView::pan_offset`] changes.
    ///
    /// The payload is the same `(f32, Point)` pair a `ZoomPanController` produces, so the two can
    /// be wired without a second projection.
    pub view_changed: Signal1<(f32, Point)>,
    /// Emitted after [`ImageView::set_image`] installs a new picture.
    pub image_changed: GenericSignal,
    /// Emitted with the decoder's message when a bytes/file load fails.
    pub load_failed: Signal1<String>,
    /// Emitted when the picture is double-clicked inside the control.
    pub double_clicked: GenericSignal,
}

impl ImageView {
    /// Creates a new ImageView with the given image and geometry.
    pub fn new(image: Image, rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ImageView, rect, "ImageView"),
            image,
            scaled: false,
            aspect: ImageAspect::Stretch,
            image_alignment: ImageAlignment::TopLeft,
            margin: 0,
            background: None,
            scaled_image: None,
            oriented_cache: None,
            preview_cache: None,
            drag_last: None,
            interaction_until: None,
            pending_decode: None,
            rotation: ImageRotation::None,
            flip_horizontal: false,
            flip_vertical: false,
            zoom_level: 1.0,
            pan_offset: Point::new(0, 0),
            #[cfg(full_widgets)]
            zoom_pan: None,
            view_changed: Signal1::new(),
            image_changed: GenericSignal::new(),
            load_failed: Signal1::new(),
            double_clicked: GenericSignal::new(),
        }
    }

    /// Sets a new image to display.
    pub fn set_image(&mut self, image: Image) {
        self.image = image;
        // The cached resample belongs to the previous image. Keying it by the source *size* is
        // not enough: two images of the same size have different pixels.
        self.scaled_image = None;
        self.oriented_cache = None;
        self.preview_cache = None;
        self.base.request_redraw();
        if !self.base.is_enabled() {
            // The image is stored either way; a disabled view simply does not announce it.
            return;
        }
        self.image_changed.emit();
    }

    /// The source dimensions after [`Self::rotation`] has been applied.
    fn oriented_size(&self) -> (u32, u32) {
        oriented_dimensions(self.image.width(), self.image.height(), self.rotation)
    }

    /// The image's quarter-turn rotation.
    pub fn rotation(&self) -> ImageRotation {
        self.rotation
    }

    /// Rotates the image by a quarter turn.
    ///
    /// A 90° or 270° rotation swaps the oriented width and height, so the layout, the size hint
    /// and the destination box follow it.
    pub fn set_rotation(&mut self, rotation: ImageRotation) {
        if self.rotation == rotation {
            return;
        }
        self.rotation = rotation;
        self.scaled_image = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the image is mirrored horizontally.
    pub fn flip_horizontal(&self) -> bool {
        self.flip_horizontal
    }

    /// Sets the horizontal mirror.
    pub fn set_flip_horizontal(&mut self, flip: bool) {
        if self.flip_horizontal == flip {
            return;
        }
        self.flip_horizontal = flip;
        self.scaled_image = None;
        self.base.request_redraw();
    }

    /// Whether the image is mirrored vertically.
    pub fn flip_vertical(&self) -> bool {
        self.flip_vertical
    }

    /// Sets the vertical mirror.
    pub fn set_flip_vertical(&mut self, flip: bool) {
        if self.flip_vertical == flip {
            return;
        }
        self.flip_vertical = flip;
        self.scaled_image = None;
        self.base.request_redraw();
    }

    /// The view's zoom factor. `1.0` is the unzoomed layout.
    pub fn zoom_level(&self) -> f32 {
        self.zoom_level
    }

    /// Sets the view's zoom factor, clamped to a usable range.
    ///
    /// A non-finite or non-positive write is refused rather than clamped: a zero zoom would
    /// collapse the picture to nothing. A real change requests a repaint and publishes
    /// [`Self::view_changed`].
    pub fn set_zoom_level(&mut self, zoom: f32) {
        if !zoom.is_finite() || zoom <= 0.0 {
            return;
        }
        let zoom = zoom.clamp(0.05, 20.0);
        if (self.zoom_level - zoom).abs() <= f32::EPSILON {
            return;
        }
        self.zoom_level = zoom;
        self.base.request_redraw();
        if !self.base.is_enabled() {
            // The zoom is stored; a disabled view does not announce a view the user cannot reach.
            return;
        }
        self.view_changed.emit((self.zoom_level, self.pan_offset));
    }

    /// The view's pan offset, in pixels.
    pub fn pan_offset(&self) -> Point {
        self.pan_offset
    }

    /// Sets the view's pan offset, in pixels.
    pub fn set_pan_offset(&mut self, offset: Point) {
        if self.pan_offset == offset {
            return;
        }
        self.pan_offset = offset;
        self.base.request_redraw();
        if !self.base.is_enabled() {
            // Same rule as `set_zoom_level`.
            return;
        }
        self.view_changed.emit((self.zoom_level, self.pan_offset));
    }

    /// Follows `controller`'s zoom and pan (chapter 3, gap 12).
    ///
    /// The binding is one-way, controller → view: the controller is the authority for the
    /// transform and this view mirrors it through [`Self::poll_zoom_pan`], which the paint and
    /// event paths call. Standalone behaviour is unchanged — with no controller attached the view
    /// is driven only by its own setters and gestures, exactly as before. Attaching replaces any
    /// previous controller, so calling this twice leaves one subscription rather than two.
    ///
    /// # Range
    ///
    /// Both ends express zoom as a factor whose `1.0` is the unzoomed layout, so the level maps
    /// across unchanged. The controller accepts `0.05 ..= 32.0` while this view accepts
    /// `0.05 ..= 20.0`, so a level above `20.0` arrives as `20.0`: [`Self::set_zoom_level`]'s own
    /// clamp is the narrowing conversion, and nothing here silently rescales the picture. The pan
    /// maps unchanged too — both ends are pixels.
    ///
    /// A gesture on the view itself is not pushed back into the controller, because a widget
    /// cannot hold a borrow of another; a host that wants both directions forwards
    /// [`Self::view_changed`] to the controller (or re-attaches).
    #[cfg(full_widgets)]
    pub fn attach_zoom_pan(&mut self, controller: &ZoomPanController) {
        let zoom = Arc::new(Mutex::new(None));
        let pan = Arc::new(Mutex::new(None));
        let scope = ConnectionScope::new();
        let redraw = self.base.redraw_requested_signal().clone();

        let zoom_sink = Arc::clone(&zoom);
        let zoom_redraw = redraw.clone();
        controller.zoom_changed.connect_scoped(&scope, move |level| {
            *lock(&zoom_sink) = Some(*level);
            // A controller gesture redraws the view, so the poll that applies it happens next
            // frame rather than on whatever event arrives afterwards.
            zoom_redraw.emit();
        });
        let pan_sink = Arc::clone(&pan);
        controller.pan_changed.connect_scoped(&scope, move |offset| {
            *lock(&pan_sink) = Some(*offset);
            redraw.emit();
        });

        self.zoom_pan = Some(ZoomPanBinding { zoom, pan, _scope: scope });
        // Adopt the controller's current transform now, so attaching does not wait for the next
        // controller gesture before the two agree.
        self.apply_controller_view(controller.zoom_level(), controller.pan_offset());
    }

    /// Stops following the attached controller; the last applied transform stays.
    #[cfg(full_widgets)]
    pub fn detach_zoom_pan(&mut self) {
        if self.zoom_pan.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a [`ZoomPanController`] is attached.
    #[cfg(full_widgets)]
    pub fn is_zoom_pan_attached(&self) -> bool {
        self.zoom_pan.is_some()
    }

    /// Applies the zoom and pan the attached controller reported since the last poll.
    ///
    /// Called from `draw` and `handle_event`, so a controller gesture is reflected by the next
    /// frame. Returns whether anything was pending; a reported value is applied even when it
    /// already matches, which keeps this a drain rather than a change detector.
    #[cfg(full_widgets)]
    pub fn poll_zoom_pan(&mut self) -> bool {
        let (zoom, pan) = {
            let Some(binding) = self.zoom_pan.as_ref() else {
                return false;
            };
            (lock(&binding.zoom).take(), lock(&binding.pan).take())
        };
        if let Some(zoom) = zoom {
            self.set_zoom_level(zoom);
        }
        if let Some(pan) = pan {
            self.set_pan_offset(pan);
        }
        zoom.is_some() || pan.is_some()
    }

    /// Writes a controller transform through the view's own setters, so clamps and the
    /// `view_changed` announcement are the same as a host-driven write.
    #[cfg(full_widgets)]
    fn apply_controller_view(&mut self, zoom: f32, pan: Point) {
        self.set_zoom_level(zoom);
        self.set_pan_offset(pan);
    }

    /// The zoom that fits the oriented image inside the control's content box.
    ///
    /// A degenerate box or image answers `1.0`, so a double-click has a sane other end even
    /// before the control has been laid out.
    fn fit_zoom(&self) -> f32 {
        let Some(content) = self.content_box() else {
            return 1.0;
        };
        let (width, height) = self.oriented_size();
        if width == 0 || height == 0 {
            return 1.0;
        }
        let scale_x = content.width as f32 / width as f32;
        let scale_y = content.height as f32 / height as f32;
        scale_x.min(scale_y).clamp(0.05, 20.0)
    }

    /// Decodes `data` and installs it as the displayed picture.
    ///
    /// This is the failure-reporting counterpart of [`Self::set_image`]: a decode error is emitted
    /// through [`Self::load_failed`] *and* returned, so a host that connects the signal and a
    /// caller that inspects the result both see it. A successful decode emits
    /// [`Self::image_changed`] through `set_image`.
    pub fn set_image_from_bytes(&mut self, data: &[u8]) -> Result<(), String> {
        match Image::from_bytes(data) {
            Ok(image) => {
                self.set_image(image);
                Ok(())
            }
            Err(error) => {
                if !self.base.is_enabled() {
                    // A failed decode still returns the error to the caller; the signal is what a
                    // disabled view withholds.
                    return Err(error);
                }
                self.load_failed.emit(error.clone());
                Err(error)
            }
        }
    }

    /// Returns a reference to the current image.
    pub fn image(&self) -> &Image {
        &self.image
    }

    /// Loads an image from a file path through the crate's decoder.
    ///
    /// This is the path counterpart of [`Self::set_image_from_bytes`]: a decode failure is
    /// emitted through [`Self::load_failed`] *and* returned, so both a connected host and a
    /// caller see it. The read and decode happen on the calling thread; use
    /// [`Self::load_path_async`] to keep a large decode off the frame that requested it.
    pub fn load_path(&mut self, path: &str) -> Result<(), String> {
        match Image::from_file(path) {
            Ok(image) => {
                self.set_image(image);
                Ok(())
            }
            Err(error) => {
                if self.base.is_enabled() {
                    self.load_failed.emit(error.clone());
                }
                Err(error)
            }
        }
    }

    /// Starts a background decode of `path` and returns immediately.
    ///
    /// The file is read here (that is the cheap half), then the decode runs on the image
    /// decoder's worker thread. [`Widget::tick`] polls the job once per frame; a finished image
    /// is installed through [`Self::set_image`], which emits [`Self::image_changed`], so a host
    /// that connected that signal gets the callback on the frame the picture arrives. A decode
    /// failure is emitted through [`Self::load_failed`].
    pub fn load_path_async(&mut self, path: &str) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|error| {
            crate::compat::format!(
                "image file '{path}' could not be read: {error} (check that the path exists \
                 and is readable)"
            )
        })?;
        self.pending_decode = Some(decode_async(bytes));
        self.base.request_redraw();
        Ok(())
    }

    /// Whether a background decode is still waiting for its result.
    pub fn is_loading(&self) -> bool {
        self.pending_decode.is_some()
    }

    /// Takes a finished background decode, if one is ready.
    fn poll_decode(&mut self) -> bool {
        let Some(job) = self.pending_decode.take() else {
            return false;
        };
        match job.try_take() {
            None => {
                // Still decoding: put the job back and ask for another frame.
                self.pending_decode = Some(job);
                self.base.request_redraw();
                false
            }
            Some(Ok(decoded)) => {
                self.set_image(Image::from_decoded(decoded));
                true
            }
            Some(Err(error)) => {
                if self.base.is_enabled() {
                    self.load_failed.emit(error);
                }
                true
            }
        }
    }

    /// Whether a gesture is currently using the reduced preview level.
    fn preview_active(&self) -> bool {
        if self.drag_last.is_some() {
            return true;
        }
        self.interaction_until.is_some_and(|until| crate::compat::Instant::now() < until)
    }

    /// Paints the decoded image into `dest`, resampling from the appropriate pyramid level.
    fn paint_pixels(&mut self, context: &mut RenderContext, dest: Rect) {
        let Some(data) = self.image.rgba8_data().filter(|data| !data.is_empty()) else {
            return;
        };
        let source_width = self.image.width();
        let source_height = self.image.height();
        let identity =
            self.rotation == ImageRotation::None && !self.flip_horizontal && !self.flip_vertical;
        if identity && dest.width == source_width && dest.height == source_height {
            // An untransformed, same-size paint borrows the decoded pixels.
            context.draw_image(dest.x, dest.y, dest.width, dest.height, data);
            return;
        }

        let fresh_oriented = self.oriented_cache.as_ref().is_some_and(|cache| {
            cache.source_len == data.len()
                && cache.rotation == self.rotation
                && cache.flip_horizontal == self.flip_horizontal
                && cache.flip_vertical == self.flip_vertical
        });
        if !fresh_oriented {
            self.oriented_cache = Some(build_oriented(
                data,
                source_width,
                source_height,
                self.rotation,
                self.flip_horizontal,
                self.flip_vertical,
            ));
            self.preview_cache = None;
            self.scaled_image = None;
        }

        let interactive = self.preview_active();
        if interactive {
            let preview_fresh = self.preview_cache.as_ref().is_some_and(|cache| {
                self.oriented_cache.as_ref().is_some_and(|oriented| {
                    cache.source_len == oriented.source_len
                        && cache.rotation == oriented.rotation
                        && cache.flip_horizontal == oriented.flip_horizontal
                        && cache.flip_vertical == oriented.flip_vertical
                })
            });
            if !preview_fresh {
                self.preview_cache = build_preview(self.oriented_cache.as_ref());
            }
        }
        let use_preview = interactive
            && self
                .preview_cache
                .as_ref()
                .is_some_and(|preview| preview.width < dest.width || preview.height < dest.height);
        if interactive && !use_preview {
            // The request is not larger than the preview, so there is nothing to gain; end the
            // window so the next frame samples precisely.
            self.interaction_until = None;
        }

        let fresh = self.scaled_image.as_ref().is_some_and(|cache| {
            cache.source_len == data.len()
                && cache.rotation == self.rotation
                && cache.flip_horizontal == self.flip_horizontal
                && cache.flip_vertical == self.flip_vertical
                && cache.preview == use_preview
                && cache.width == dest.width
                && cache.height == dest.height
        });
        if !fresh {
            let (source, width, height) = match (use_preview, self.preview_cache.as_ref()) {
                (true, Some(preview)) => (preview.pixels.as_slice(), preview.width, preview.height),
                _ => {
                    let oriented = self.oriented_cache.as_ref();
                    match oriented {
                        Some(oriented) => {
                            (oriented.pixels.as_slice(), oriented.width, oriented.height)
                        }
                        None => return,
                    }
                }
            };
            let resampled = if dest.width == width && dest.height == height {
                source.to_vec()
            } else {
                resample_bilinear(source, width, height, dest.width, dest.height)
            };
            self.scaled_image = Some(ScaledImageCache {
                source_len: data.len(),
                rotation: self.rotation,
                flip_horizontal: self.flip_horizontal,
                flip_vertical: self.flip_vertical,
                preview: use_preview,
                width: dest.width,
                height: dest.height,
                pixels: resampled,
            });
        }
        if let Some(cache) = self.scaled_image.as_ref() {
            context.draw_image(dest.x, dest.y, dest.width, dest.height, &cache.pixels);
        }

        // A wheel has no release event, so the preview window closes on its own. Keep asking
        // for frames while it is open, which is what paints the precise frame when it expires.
        if interactive {
            self.base.request_redraw();
        }
    }

    /// Paints the tiled form of the image across `content`.
    fn paint_tiles(&mut self, context: &mut RenderContext, content: Rect) {
        let (oriented_width, oriented_height) = self.oriented_size();
        if oriented_width == 0 || oriented_height == 0 {
            return;
        }
        let zoom = self.zoom_level;
        let tile_w = ((oriented_width as f32 * zoom).round() as u32).max(1);
        let tile_h = ((oriented_height as f32 * zoom).round() as u32).max(1);
        let step_x = tile_w as i32;
        let step_y = tile_h as i32;
        // The pattern's origin follows the pan, wrapped into one tile so a pan looks like
        // moving a continuous sheet rather than shifting a single picture.
        let mut y = content.y + self.pan_offset.y.rem_euclid(step_y.max(1)) - step_y;
        context.push_clip(content.x, content.y, content.width, content.height);
        while y < content.y + content.height as i32 {
            let mut x = content.x + self.pan_offset.x.rem_euclid(step_x.max(1)) - step_x;
            while x < content.x + content.width as i32 {
                self.paint_pixels(context, Rect::new(x, y, tile_w, tile_h));
                x += step_x;
            }
            y += step_y;
        }
        context.pop_clip();
    }

    /// Sets whether the image should be scaled to fill the widget rect.
    pub fn set_scaled(&mut self, scaled: bool) {
        if self.scaled == scaled {
            return;
        }
        self.scaled = scaled;
        // `draw` branches on this to choose between the image's own size and the widget's, so the
        // geometry on screen changes with it and the setter owes a frame.
        self.base.request_redraw();
    }

    /// Returns whether the image is scaled to fill the widget rect.
    pub fn is_scaled(&self) -> bool {
        self.scaled
    }

    /// Returns how the image is mapped into the widget's box.
    pub fn aspect(&self) -> ImageAspect {
        self.aspect
    }

    /// Sets how the image is mapped into the widget's box.
    ///
    /// [`ImageAspect::Stretch`] still honours [`Self::is_scaled`] so the
    /// historic geometry is preserved; `Fit` and `Fill` always scale the image
    /// into the box because that is what the two modes mean.
    pub fn set_aspect(&mut self, aspect: ImageAspect) {
        if self.aspect == aspect {
            return;
        }
        self.aspect = aspect;
        self.base.request_redraw();
    }

    /// Returns where the image sits when it does not fill the widget's box.
    pub fn image_alignment(&self) -> ImageAlignment {
        self.image_alignment
    }

    /// Sets where the image sits when it does not fill the widget's box.
    pub fn set_image_alignment(&mut self, alignment: ImageAlignment) {
        if self.image_alignment == alignment {
            return;
        }
        self.image_alignment = alignment;
        self.base.request_redraw();
    }

    /// Returns the blank border kept around the image, in pixels.
    pub fn margin(&self) -> u32 {
        self.margin
    }

    /// Sets the blank border kept around the image, in pixels.
    ///
    /// The margin shrinks the box the image is laid out in, so it applies to
    /// every aspect and alignment. The default is `0`.
    pub fn set_margin(&mut self, margin: u32) {
        if self.margin == margin {
            return;
        }
        self.margin = margin;
        self.base.request_redraw();
    }

    /// The widget box reduced by [`Self::margin`], or `None` when nothing is
    /// left to paint into.
    fn content_box(&self) -> Option<Rect> {
        let rect = self.geometry();
        let doubled = self.margin.saturating_mul(2);
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        if doubled >= rect.width || doubled >= rect.height {
            return None;
        }
        Some(Rect::new(
            rect.x + self.margin as i32,
            rect.y + self.margin as i32,
            rect.width - doubled,
            rect.height - doubled,
        ))
    }

    /// The box the image is laid out in and the destination rectangle it is
    /// drawn at, or `None` when there is no image or no room to paint it.
    ///
    /// `Stretch` reproduces the historic geometry: the whole box when the
    /// image is scaled, otherwise the image's own size clamped to the box.
    /// `Fit` letterboxes and `Fill` covers and centre-crops.
    fn image_layout(&self) -> Option<(Rect, Rect)> {
        let content = self.content_box()?;
        let (image_width, image_height) = self.oriented_size();
        if image_width == 0 || image_height == 0 {
            return None;
        }

        let (dest_width, dest_height) = match self.aspect {
            ImageAspect::Stretch if self.scaled => (content.width, content.height),
            ImageAspect::Stretch => {
                (image_width.min(content.width), image_height.min(content.height))
            }
            ImageAspect::Fit => {
                // Compare the cross products rather than the two scale factors:
                // integers keep the decision exact, so a square image in a
                // square box is limited by both axes and not by rounding.
                let width_limited = content.width as u64 * image_height as u64
                    <= content.height as u64 * image_width as u64;
                if width_limited {
                    let height = (content.width as u64 * image_height as u64 / image_width as u64)
                        .max(1) as u32;
                    (content.width, height.min(content.height))
                } else {
                    let width = (content.height as u64 * image_width as u64 / image_height as u64)
                        .max(1) as u32;
                    (width.min(content.width), content.height)
                }
            }
            ImageAspect::Fill => {
                // Cover: the limiting axis reaches the box exactly and the other
                // overflows. Rounding up keeps the overflow cover complete.
                let width_limited = content.width as u64 * image_height as u64
                    >= content.height as u64 * image_width as u64;
                if width_limited {
                    let height =
                        (content.width as u64 * image_height as u64).div_ceil(image_width as u64);
                    (content.width, height.max(content.height as u64) as u32)
                } else {
                    let width =
                        (content.height as u64 * image_width as u64).div_ceil(image_height as u64);
                    (width.max(content.width as u64) as u32, content.height)
                }
            }
            // A tile is drawn at its natural size and repeated; the repeat count follows
            // from the content box in the paint path.
            ImageAspect::Tile => (image_width, image_height),
        };

        let dest_x = if self.aspect == ImageAspect::Fill {
            // `Fill` crops around the source image's centre, so the overflow is
            // split evenly and the visible picture is never biased to a corner.
            content.x + (content.width as i32 - dest_width as i32) / 2
        } else {
            content.x
                + self.image_alignment.horizontal_offset(content.width as i32, dest_width as i32)
        };
        let dest_y = if self.aspect == ImageAspect::Fill {
            content.y + (content.height as i32 - dest_height as i32) / 2
        } else {
            content.y
                + self.image_alignment.vertical_offset(content.height as i32, dest_height as i32)
        };
        let mut dest = Rect::new(dest_x, dest_y, dest_width, dest_height);

        // Zoom and pan are applied around the laid-out picture's centre, so `zoom == 1.0` and a
        // zero pan reproduce the historical box exactly. Tiling is its own geometry: the tile
        // size and origin are derived in the paint path so the pattern repeats rather than
        // being one enlarged picture.
        if self.aspect != ImageAspect::Tile
            && ((self.zoom_level - 1.0).abs() > f32::EPSILON || self.pan_offset != Point::new(0, 0))
        {
            let scaled_width = (dest.width as f32 * self.zoom_level).round().max(1.0) as u32;
            let scaled_height = (dest.height as f32 * self.zoom_level).round().max(1.0) as u32;
            let centre_x = dest.x + (dest.width as i32) / 2;
            let centre_y = dest.y + (dest.height as i32) / 2;
            dest = Rect::new(
                centre_x - (scaled_width as i32) / 2 + self.pan_offset.x,
                centre_y - (scaled_height as i32) / 2 + self.pan_offset.y,
                scaled_width,
                scaled_height,
            );
        }

        Some((content, dest))
    }
}

impl Widget for ImageView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        let (width, height) = self.oriented_size();
        if width > 0 && height > 0 {
            Size::new(width, height)
        } else {
            Size::new(100, 100)
        }
    }

    /// Polls a background decode once per frame; a finished picture is installed here.
    fn tick(&mut self, _delta_ms: u32) -> bool {
        self.poll_decode()
    }

    /// A background decode owes frames until its result arrives.
    fn is_animating(&self) -> bool {
        self.pending_decode.is_some()
    }

    /// Resolves a published event name to the signal the view emits.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "image_changed" => Some(EventSignalRef::unit("image_changed", &self.image_changed)),
            "load_failed" => Some(EventSignalRef::mapped(
                "load_failed",
                &self.load_failed,
                |message: &String| CapabilityValue::String(message.clone()),
            )),
            "double_clicked" => Some(EventSignalRef::unit("double_clicked", &self.double_clicked)),
            "view_changed" => Some(EventSignalRef::mapped(
                "view_changed",
                &self.view_changed,
                |(zoom, pan): &(f32, Point)| {
                    CapabilityValue::Tuple(crate::compat::vec![
                        CapabilityValue::Float(f64::from(*zoom)),
                        CapabilityValue::Int(i64::from(pan.x)),
                        CapabilityValue::Int(i64::from(pan.y)),
                    ])
                },
            )),
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ImageView`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch.
impl WidgetProperties for ImageView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "scaled" => Ok(CapabilityValue::Bool(self.is_scaled())),
            "rotation" => Ok(CapabilityValue::String(self.rotation().token().to_string())),
            "flip_horizontal" => Ok(CapabilityValue::Bool(self.flip_horizontal())),
            "flip_vertical" => Ok(CapabilityValue::Bool(self.flip_vertical())),
            "zoom_level" => Ok(CapabilityValue::Float(f64::from(self.zoom_level()))),
            // A point is not a `CapabilityValue`; the `x,y` spelling is the same one the
            // geometry property uses for a pair of numbers.
            "pan_offset" => Ok(CapabilityValue::String(format!(
                "{},{}",
                self.pan_offset().x,
                self.pan_offset().y
            ))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "scaled" => {
                self.set_scaled(expect_bool(value)?);
                Ok(())
            }
            "rotation" => {
                let token = expect_string(value)?;
                let rotation =
                    ImageRotation::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_rotation(rotation);
                Ok(())
            }
            "flip_horizontal" => {
                self.set_flip_horizontal(expect_bool(value)?);
                Ok(())
            }
            "flip_vertical" => {
                self.set_flip_vertical(expect_bool(value)?);
                Ok(())
            }
            "zoom_level" => {
                self.set_zoom_level(expect_f32(value)?);
                Ok(())
            }
            "pan_offset" => {
                let raw = expect_string(value)?;
                let mut parts = raw.split(',');
                let x = parts
                    .next()
                    .and_then(|part| part.trim().parse::<i32>().ok())
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                let y = parts
                    .next()
                    .and_then(|part| part.trim().parse::<i32>().ok())
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_pan_offset(Point::new(x, y));
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "scaled",
            "rotation",
            "flip_horizontal",
            "flip_vertical",
            "zoom_level",
            "pan_offset",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs the two published commands that cannot travel as a property value.
    ///
    /// # Why this control needs an override
    ///
    /// The default [`WidgetProperties::command`] implements the `set_foo` convention
    /// by resolving `foo` against [`Self::property_names`]. That works for scalar
    /// properties, but an `Image` is not a [`CapabilityValue`] variant, so there is no
    /// `image_source` property to route through — the payload is set with the control's
    /// own [`Self::set_image`]. Naming the command after a property that does not exist
    /// would leave the caller with an instruction they cannot carry out, which is
    /// exactly what `set_image` used to be.
    ///
    /// `set_scaled` is handled here too rather than left to the default: it *does*
    /// have a property, but this control already owns the typed setter, so the command
    /// route and the property route end at the same place deliberately.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            // A bare invocation has no image to install; the caller supplies one
            // through `set_image`. Reporting `OutOfRange` is the same answer a
            // payload-carrying property command gives.
            "set_image" => Err(CapabilityAccessError::OutOfRange),
            "set_scaled" => {
                self.set_scaled(true);
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ImageView {
    fn handle_event(&mut self, event: &Event) {
        // A controller gesture is a shared-cell write, so it is drained before routing: the frame
        // that follows already shows the transform the controller reported (chapter 3, gap 12).
        #[cfg(full_widgets)]
        self.poll_zoom_pan();
        self.base.handle_event(event);
        // A double click is the gesture a host uses to open a viewer or replace the picture. It
        // is reported only when it lands on the control and the control is enabled. It is also
        // the view's own zoom toggle: `1.0` and the fit scale are the two ends a viewer expects,
        // and the change is published through `view_changed`.
        if let Event::MouseDoubleClick { pos, button: 1 } = event {
            if self.base.is_enabled() && self.geometry().contains_point(*pos) {
                self.double_clicked.emit();
                let fit = self.fit_zoom();
                let target = if (self.zoom_level - 1.0).abs() <= f32::EPSILON { fit } else { 1.0 };
                self.set_zoom_level(target);
                self.set_pan_offset(Point::new(0, 0));
            }
            return;
        }
        if !self.base.is_enabled() {
            return;
        }
        match event {
            // Dragging pans the picture. A press arms it, each move applies the pointer's
            // own delta, and a release (or a departure, whose release never arrives) ends it
            // and restores full-resolution sampling.
            Event::MousePress { pos, button: 1, .. } if self.geometry().contains_point(*pos) => {
                self.drag_last = Some(*pos);
                self.interaction_until = None;
            }
            Event::MouseMove { pos } if self.drag_last.is_some() => {
                if let Some(last) = self.drag_last {
                    let dx = pos.x - last.x;
                    let dy = pos.y - last.y;
                    if dx != 0 || dy != 0 {
                        self.set_pan_offset(Point::new(
                            self.pan_offset.x.saturating_add(dx),
                            self.pan_offset.y.saturating_add(dy),
                        ));
                    }
                }
                self.drag_last = Some(*pos);
            }
            Event::MouseRelease { button: 1, .. } => {
                if self.drag_last.take().is_some() {
                    self.interaction_until = None;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.drag_last.take().is_some() {
                    self.interaction_until = None;
                    self.base.request_redraw();
                }
            }
            // One wheel notch zooms by a fixed factor, around the picture's own centre. The
            // preview level is used for a short settle window because a notch carries no
            // release to mark the end of the gesture.
            Event::Wheel { delta, .. } if delta.y != 0 => {
                let factor = if delta.y < 0 { WHEEL_ZOOM_IN } else { WHEEL_ZOOM_OUT };
                self.set_zoom_level(self.zoom_level * factor);
                self.interaction_until = Some(
                    crate::compat::Instant::now()
                        + core::time::Duration::from_millis(WHEEL_PREVIEW_HOLD_MS),
                );
                self.base.request_redraw();
            }
            _ => {}
        }
    }
}

impl Draw for ImageView {
    fn draw(&mut self, context: &mut RenderContext) {
        // Drain a transform the bound controller reported, so the paint shows the state the
        // controller is in rather than the one from before its gesture (chapter 3, gap 12).
        #[cfg(full_widgets)]
        self.poll_zoom_pan();
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // One read of the decoded bytes: the decision below and the painting use the same
        // reference, instead of walking the decoder's accessor twice per frame.
        let data = if self.image.format() == ImageFormat::Rgba8 {
            self.image.rgba8_data().filter(|data| !data.is_empty())
        } else {
            None
        };
        if data.is_some() {
            // No room left for the image after its margin: nothing to paint.
            let Some((content, dest)) = self.image_layout() else {
                return;
            };
            if self.aspect == ImageAspect::Tile {
                self.paint_tiles(context, content);
                return;
            }
            // A covering image, a zoomed one or a panned one is larger than its box, so the
            // painted copy has to be bounded to it or it spills onto whatever is behind the
            // control. Stretch and Fit at rest never exceed the box and need no clip, which
            // keeps their command stream identical to the old one.
            let covering = self.aspect == ImageAspect::Fill
                || (self.zoom_level - 1.0).abs() > f32::EPSILON
                || self.pan_offset != Point::new(0, 0);
            if covering {
                context.push_clip(content.x, content.y, content.width, content.height);
            }
            self.paint_pixels(context, dest);
            if covering {
                context.pop_clip();
            }
        } else {
            // Draw placeholder rectangle with "?" text.
            let bg = self
                .style()
                .background_color
                .or(self.background)
                .unwrap_or(Color::rgb(240, 240, 240));
            context.face_with_gradient_corners(
                rect,
                bg,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().resolved_corner_radii().to_array(),
                Color::BLACK,
            );

            // Draw a border to make the placeholder visible.
            let border = self.style().border_color.unwrap_or(Color::rgb(200, 200, 200));
            crate::widget::draw::stroke_surface_border(
                context,
                rect,
                self.style().resolved_corner_radii().to_array(),
                border,
                1,
            );

            // Draw "?" centered in the placeholder.
            let fg = self.style().text_color.unwrap_or(Color::rgb(120, 120, 120));
            let default_font = crate::core::Font::default();
            let font = self.style().font.as_ref().unwrap_or(&default_font);
            // Horizontal centre from the measured glyph, vertical from the shared line box.
            // The previous `rect.y + rect.height / 2 - 8` put the box's top edge on the
            // placeholder's middle line, with a constant standing in for the missing calc.
            let glyph_metrics = context.measure_text("?", font);
            let line = context.text_line(rect, font);
            let text_x = rect.x + (rect.width as i32 - glyph_metrics.width as i32) / 2;
            let text_y = line.y;
            context.draw_text(Point::new(text_x, text_y), "?", font, fg, HorizontalAlignment::Left);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};
    use crate::widget::svg::render_to_svg;

    #[test]
    fn image_view_creation() {
        let image = Image::new();
        let view = ImageView::new(image, Rect::new(0, 0, 100, 100));
        assert_eq!(view.kind(), WidgetKind::ImageView);
        assert_eq!(view.geometry(), Rect::new(0, 0, 100, 100));
        assert!(!view.is_scaled());
    }

    #[test]
    fn image_view_set_image() {
        let empty = Image::new();
        let mut view = ImageView::new(empty, Rect::new(0, 0, 100, 100));
        assert!(view.image().is_empty());

        let rgba = Image::try_from_rgba(vec![255; 16 * 4], 4, 4)
            .expect("the buffer matches its declared size");
        view.set_image(rgba.clone());
        assert_eq!(view.image().width(), 4);
        assert_eq!(view.image().height(), 4);
        assert_eq!(view.image().format(), ImageFormat::Rgba8);
    }

    #[test]
    fn image_view_draw_no_panic() {
        let image = Image::new();
        let mut view = ImageView::new(image, Rect::new(0, 0, 100, 100));

        let mut backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        view.draw(&mut context);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn image_view_scaled_flag() {
        let image = Image::new();
        let mut view = ImageView::new(image, Rect::new(0, 0, 100, 100));
        assert!(!view.is_scaled());

        view.set_scaled(true);
        assert!(view.is_scaled());

        view.set_scaled(false);
        assert!(!view.is_scaled());
    }

    #[test]
    fn image_view_size_hint_empty() {
        let image = Image::new();
        let view = ImageView::new(image, Rect::new(0, 0, 100, 100));
        assert_eq!(view.size_hint(), Size::new(100, 100));
    }

    #[test]
    fn image_view_size_hint_with_image() {
        let rgba = Image::try_from_rgba(vec![255; 64 * 64 * 4], 64, 64)
            .expect("the buffer matches its declared size");
        let view = ImageView::new(rgba, Rect::new(0, 0, 200, 200));
        assert_eq!(view.size_hint(), Size::new(64, 64));
    }

    #[test]
    fn setting_scaled_repaints() {
        let mut view = ImageView::new(crate::widget::Image::new(), Rect::new(0, 0, 200, 200));
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        view.base.redraw_requested.connect({
            let seen = std::sync::Arc::clone(&seen);
            move || {
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        view.set_scaled(true);
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "`draw` branches on this to size the image"
        );
        view.set_scaled(true);
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the same value is not a change"
        );
    }

    /// The two new enums publish and parse the tokens their properties use, and
    /// their defaults keep the historic picture.
    #[test]
    fn image_view_aspect_and_alignment_tokens_round_trip() {
        assert_eq!(ImageAspect::default(), ImageAspect::Stretch);
        for aspect in [ImageAspect::Stretch, ImageAspect::Fit, ImageAspect::Fill] {
            assert_eq!(ImageAspect::from_token(aspect.as_str()), Some(aspect));
        }
        assert_eq!(ImageAspect::from_token("cover"), None, "an unknown token is refused");

        assert_eq!(ImageAlignment::default(), ImageAlignment::TopLeft);
        for alignment in [
            ImageAlignment::TopLeft,
            ImageAlignment::Top,
            ImageAlignment::TopRight,
            ImageAlignment::Left,
            ImageAlignment::Center,
            ImageAlignment::Right,
            ImageAlignment::BottomLeft,
            ImageAlignment::Bottom,
            ImageAlignment::BottomRight,
        ] {
            assert_eq!(ImageAlignment::from_token(alignment.as_str()), Some(alignment));
        }
        assert_eq!(ImageAlignment::from_token("middle"), None);
    }

    /// The destination rectangle follows the aspect, alignment and margin: the
    /// historic stretch fills the box, fit letterboxes, and fill centre-crops.
    #[test]
    fn image_view_aspects_lay_the_image_out_in_the_box() {
        let image = Image::try_from_rgba(vec![255; 4 * 2 * 4], 4, 2)
            .expect("the buffer matches its declared size");

        let mut stretch = ImageView::new(image.clone(), Rect::new(0, 0, 100, 100));
        stretch.set_scaled(true);
        let svg = render_to_svg(&mut stretch);
        assert!(
            svg.contains(r#"width="100" height="100""#),
            "stretch still fills the widget: {svg}"
        );

        let mut fit = ImageView::new(image.clone(), Rect::new(0, 0, 100, 100));
        fit.set_aspect(ImageAspect::Fit);
        let svg = render_to_svg(&mut fit);
        assert!(
            svg.contains(r#"x="0" y="0" width="100" height="50""#),
            "a 4x2 image fits as 100x50 at the top-left: {svg}"
        );

        // Centring moves the letterboxed image down by half the spare height.
        fit.set_image_alignment(ImageAlignment::Center);
        let svg = render_to_svg(&mut fit);
        assert!(
            svg.contains(r#"x="0" y="25" width="100" height="50""#),
            "centre alignment splits the spare height: {svg}"
        );

        let mut fill = ImageView::new(image, Rect::new(0, 0, 100, 100));
        fill.set_aspect(ImageAspect::Fill);
        let svg = render_to_svg(&mut fill);
        assert!(
            svg.contains(r#"x="-50" y="0" width="200" height="100""#),
            "fill covers the box and centre-crops the overflow: {svg}"
        );
        assert!(svg.contains("clip-path"), "the overhanging copy is clipped to the box: {svg}");
    }

    /// The margin shrinks the box the image is laid out in, and a margin that
    /// would leave no room paints nothing.
    #[test]
    fn image_view_margin_shrinks_the_box() {
        let image = Image::try_from_rgba(vec![255; 4 * 2 * 4], 4, 2)
            .expect("the buffer matches its declared size");
        let mut view = ImageView::new(image, Rect::new(0, 0, 100, 100));
        view.set_scaled(true);
        assert_eq!(view.margin(), 0);

        view.set_margin(10);
        assert_eq!(view.margin(), 10);
        let svg = render_to_svg(&mut view);
        assert!(
            svg.contains(r#"x="10" y="10" width="80" height="80""#),
            "the margin insets the stretched image on every side: {svg}"
        );

        view.set_margin(50);
        assert!(
            !render_to_svg(&mut view).contains("<image"),
            "a margin that consumes the box leaves nothing to paint"
        );
    }

    /// Each new setter repaints, and rewriting the same value does not.
    #[test]
    fn image_view_layout_setters_repaint_only_on_change() {
        let mut view = ImageView::new(Image::new(), Rect::new(0, 0, 200, 200));
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        view.base.redraw_requested.connect({
            let seen = std::sync::Arc::clone(&seen);
            move || {
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        view.set_aspect(ImageAspect::Fit);
        view.set_image_alignment(ImageAlignment::BottomRight);
        view.set_margin(4);
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 3);

        view.set_aspect(ImageAspect::Fit);
        view.set_image_alignment(ImageAlignment::BottomRight);
        view.set_margin(4);
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            3,
            "unchanged values do not request frames"
        );
    }

    /// A scaled image reaches the **raster** backend, not only the vector one.
    ///
    /// The draw API takes the destination rectangle and the pixels but not the source grid, so a
    /// control that handed it a differently sized buffer was cropped (downscale) or skipped
    /// entirely (upscale: the backend refuses a buffer shorter than `w * h * 4`). The control now
    /// resamples first, and this is the assertion that its pixels arrive.
    #[test]
    fn a_scaled_image_paints_on_the_software_backend() {
        use crate::render::{PaintBackend, SoftwarePaintBackend};

        // A 2x2 opaque red source stretched into a 4x4 box.
        let red = [255u8, 0, 0, 255];
        let image = Image::try_from_rgba(red.repeat(4), 2, 2).expect("the buffer matches its size");
        let mut view = ImageView::new(image, Rect::new(0, 0, 4, 4));
        view.set_scaled(true);

        let mut backend = SoftwarePaintBackend::new(Size::new(4, 4), 1.0);
        backend.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut backend);
            <ImageView as Draw>::draw(&mut view, &mut context);
        }
        backend.end_frame();

        let frame = backend.into_frame_rgba();
        // Row 1, column 1 of a 4-pixel-wide frame: the centre of the stretched image.
        let index = (4 + 1) * 4;
        assert_eq!(
            &frame[index..index + 3],
            &[255, 0, 0],
            "a stretched image must cover the box, not vanish or leave a crop"
        );
    }

    /// Rotation swaps the oriented hint for a quarter turn.
    #[test]
    fn rotation_tokens_round_trip_and_swap_the_hint() {
        for rotation in [
            ImageRotation::None,
            ImageRotation::Deg90,
            ImageRotation::Deg180,
            ImageRotation::Deg270,
        ] {
            assert_eq!(ImageRotation::from_token(rotation.token()), Some(rotation));
        }
        assert_eq!(ImageRotation::from_token("45"), None);

        let image = Image::try_from_rgba(vec![255; 4 * 2 * 4], 4, 2).expect("valid fixture");
        let mut view = ImageView::new(image, Rect::new(0, 0, 100, 100));
        assert_eq!(view.size_hint(), Size::new(4, 2));
        view.set_rotation(ImageRotation::Deg90);
        assert_eq!(view.size_hint(), Size::new(2, 4), "a quarter turn swaps the axes");
        view.set_rotation(ImageRotation::Deg180);
        assert_eq!(view.size_hint(), Size::new(4, 2));
    }

    /// The orientation transform maps every source pixel to the expected corner.
    #[test]
    fn the_orientation_transform_rotates_then_flips() {
        // A 2x1 image: red on the left, blue on the right.
        let data = [255u8, 0, 0, 255, 0, 0, 255, 255];
        let (out, width, height) = orient_pixels(&data, 2, 1, ImageRotation::Deg90, false, false);
        assert_eq!((width, height), (1, 2));
        // A clockwise turn puts the left pixel at the top.
        assert_eq!(&out[0..4], &[255, 0, 0, 255]);
        assert_eq!(&out[4..8], &[0, 0, 255, 255]);

        let (flipped, _, _) = orient_pixels(&data, 2, 1, ImageRotation::None, true, false);
        assert_eq!(&flipped[0..4], &[0, 0, 255, 255], "a mirror swaps the ends");
    }

    /// Zoom and pan are readable, clamped and published; the default is the historical view.
    #[test]
    fn zoom_and_pan_are_published_when_they_change() {
        let image = Image::try_from_rgba(vec![255; 4 * 4 * 4], 4, 4).expect("valid fixture");
        let mut view = ImageView::new(image, Rect::new(0, 0, 100, 100));
        assert_eq!(view.zoom_level(), 1.0);
        assert_eq!(view.pan_offset(), Point::new(0, 0));

        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = std::sync::Arc::clone(&seen);
        view.view_changed.connect(move |value: std::sync::Arc<(f32, Point)>| {
            sink.lock().unwrap().push(*value);
        });

        view.set_zoom_level(2.0);
        view.set_pan_offset(Point::new(5, -3));
        assert_eq!(view.zoom_level(), 2.0);
        assert_eq!(view.pan_offset(), Point::new(5, -3));
        assert_eq!(*seen.lock().unwrap(), vec![(2.0, Point::new(0, 0)), (2.0, Point::new(5, -3))]);

        // A same-value write and a non-finite write are not changes.
        view.set_zoom_level(2.0);
        view.set_zoom_level(f32::NAN);
        assert_eq!(view.zoom_level(), 2.0);
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    /// A double click toggles between the fitted view and the unzoomed one.
    #[test]
    fn a_double_click_toggles_between_fit_and_actual_size() {
        let image = Image::try_from_rgba(vec![255; 10 * 10 * 4], 10, 10).expect("valid fixture");
        let mut view = ImageView::new(image, Rect::new(0, 0, 100, 100));
        let fit = view.fit_zoom();
        assert!(fit > 1.0, "a small image in a large box fits larger: {fit}");

        view.handle_event(&Event::MouseDoubleClick { pos: Point::new(20, 20), button: 1 });
        assert!((view.zoom_level() - fit).abs() < 1e-4, "got {}", view.zoom_level());
        view.handle_event(&Event::MouseDoubleClick { pos: Point::new(20, 20), button: 1 });
        assert_eq!(view.zoom_level(), 1.0);
    }

    /// A still transformed image reuses its resampled buffer.
    #[test]
    fn the_transformed_buffer_is_cached_between_frames() {
        let image = Image::try_from_rgba(vec![255; 8 * 8 * 4], 8, 8).expect("valid fixture");
        let mut view = ImageView::new(image, Rect::new(0, 0, 40, 40));
        view.set_rotation(ImageRotation::Deg90);
        let mut backend = SoftwarePaintBackend::new(Size::new(40, 40), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        view.draw(&mut context);
        let ptr = view.scaled_image.as_ref().expect("a rotated paint caches").pixels.as_ptr();
        view.draw(&mut context);
        assert_eq!(
            view.scaled_image.as_ref().unwrap().pixels.as_ptr(),
            ptr,
            "an unchanged transform reuses the buffer"
        );
        backend.end_frame();
    }

    /// The Tile form repeats the picture across the box instead of stretching it.
    #[test]
    fn tile_repeats_the_image_across_the_box() {
        let image = Image::try_from_rgba(vec![180; 10 * 10 * 4], 10, 10).expect("valid fixture");
        let mut view = ImageView::new(image, Rect::new(0, 0, 40, 40));
        view.set_aspect(ImageAspect::Tile);
        let svg = render_to_svg(&mut view);
        assert!(
            svg.matches("<image ").count() >= 16,
            "a 10 px tile in a 40 px box repeats on both axes: {}",
            svg.matches("<image ").count()
        );
        assert_eq!(ImageAspect::from_token("tile"), Some(ImageAspect::Tile));
        assert_eq!(ImageAspect::Tile.as_str(), "tile");
    }

    /// Dragging pans the picture by the pointer's own movement.
    #[test]
    fn dragging_pans_the_picture() {
        let image = Image::try_from_rgba(vec![90; 20 * 20 * 4], 20, 20).expect("valid fixture");
        let mut view = ImageView::new(image, Rect::new(0, 0, 100, 100));
        view.set_aspect(ImageAspect::Stretch);
        view.set_scaled(true);
        view.set_zoom_level(2.0);

        view.handle_event(&Event::mouse_press(50, 50, 1));
        view.handle_event(&Event::mouse_move(60, 55));
        assert_eq!(view.pan_offset(), Point::new(10, 5));
        view.handle_event(&Event::mouse_release(60, 55, 1));

        // A move after the release must not keep panning.
        view.handle_event(&Event::mouse_move(90, 90));
        assert_eq!(view.pan_offset(), Point::new(10, 5));
    }

    /// A wheel notch zooms in and out around the current level.
    #[test]
    fn the_wheel_zooms_the_picture() {
        let image = Image::try_from_rgba(vec![90; 20 * 20 * 4], 20, 20).expect("valid fixture");
        let mut view = ImageView::new(image, Rect::new(0, 0, 100, 100));
        assert_eq!(view.zoom_level(), 1.0);

        view.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert!((view.zoom_level() - WHEEL_ZOOM_IN).abs() < 1e-6);
        view.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert!((view.zoom_level() - 1.0).abs() < 1e-6);
    }

    /// A gesture samples the reduced pyramid; the rest frame samples the full image again.
    #[test]
    fn a_gesture_uses_the_preview_and_the_rest_frame_does_not() {
        let image =
            Image::try_from_rgba(vec![120; 520 * 520 * 4], 520, 520).expect("valid fixture");
        let mut view = ImageView::new(image, Rect::new(0, 0, 520, 520));
        view.set_scaled(true);
        view.set_zoom_level(1.2);
        let mut backend = SoftwarePaintBackend::new(Size::new(520, 520), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);

        view.handle_event(&Event::mouse_press(10, 10, 1));
        view.draw(&mut context);
        assert!(
            view.scaled_image.as_ref().is_some_and(|cache| cache.preview),
            "a gesture must sample the preview level"
        );

        view.handle_event(&Event::mouse_release(10, 10, 1));
        view.draw(&mut context);
        assert!(
            view.scaled_image.as_ref().is_some_and(|cache| !cache.preview),
            "the resting frame samples the full-resolution image"
        );
        backend.end_frame();
    }

    /// A path that cannot be read reports through the signal and the result.
    #[test]
    fn load_path_reports_a_missing_file() {
        let mut view = ImageView::new(Image::new(), Rect::new(0, 0, 100, 100));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let sink = std::sync::Arc::clone(&seen);
        view.load_failed.connect(move |message| {
            if let Ok(mut guard) = sink.lock() {
                *guard = Some((*message).clone());
            }
        });

        let missing = "definitely/not/a/real/image-file.png";
        assert!(view.load_path(missing).is_err());
        assert!(
            seen.lock().unwrap().as_deref().is_some_and(|message| message.contains(missing)),
            "the decoder's message names the path"
        );

        // The asynchronous entry point reads the same path and fails the same way.
        assert!(view.load_path_async(missing).is_err());
        assert!(!view.is_loading());
    }

    /// Chapter 3, gap 12: a controller zoom and fit reach the view, and detaching stops it.
    #[cfg(full_widgets)]
    #[test]
    fn a_controller_zoom_and_fit_reach_the_view() {
        use crate::widget::companions::ZoomPanController;

        let mut view = ImageView::new(Image::new(), Rect::new(0, 0, 200, 100));
        let mut controller = ZoomPanController::new(Rect::new(0, 0, 200, 100));
        // A 50 px wide picture in a 200 px viewport fits at 4.0.
        controller.set_content_size(Size::new(50, 25));

        view.attach_zoom_pan(&controller);
        assert!(view.is_zoom_pan_attached());
        assert_eq!(view.zoom_level(), 1.0, "attaching adopts the controller's 1:1 view");

        controller.set_zoom_level(2.0);
        assert!(view.poll_zoom_pan(), "the reported level is drained");
        assert!((view.zoom_level() - 2.0).abs() < 1e-6, "the controller zoom reaches the view");
        assert!(!view.poll_zoom_pan(), "a drained slot reports nothing on the next poll");

        controller.set_pan_offset(Point::new(12, -4));
        assert!(view.poll_zoom_pan());
        assert_eq!(view.pan_offset(), Point::new(12, -4), "the controller pan reaches the view");

        controller.fit_to_width();
        assert!(view.poll_zoom_pan(), "a fit is reported like any other change");
        assert!((view.zoom_level() - 4.0).abs() < 1e-6, "fit width drives the view's zoom");
        assert_eq!(view.pan_offset(), Point::new(0, 0), "fit recentres the pan as well");

        view.detach_zoom_pan();
        assert!(!view.is_zoom_pan_attached());
        controller.set_zoom_level(3.0);
        assert!(!view.poll_zoom_pan(), "a detached controller reports nothing");
        assert!((view.zoom_level() - 4.0).abs() < 1e-6, "the last applied transform stays");
    }
}
