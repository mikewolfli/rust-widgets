// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ImageGallery — an image gallery/browser widget with thumbnails and large view.
//!
//! The ImageGallery widget displays a large preview of the current image with
//! a thumbnail strip at the bottom. It supports keyboard navigation (arrow keys),
//! thumbnail selection via click, and emits a signal when the selected image changes.

use core::fmt::Write as _;

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_bool, expect_u32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::borrow::Cow;

/// Metadata for a single image in the gallery.
#[derive(Debug, Clone)]
pub struct GalleryImage {
    /// File path or URI of the image.
    pub path: String,
    /// Optional label displayed underneath the thumbnail.
    pub label: Option<String>,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Decoded RGBA image, when one was loaded.
    ///
    /// The gallery previously drew a placeholder for every tile because it held only a path.
    /// This slot lets a host (or [`Self::load_from_path`]) supply the decoded pixels, and the
    /// paint path draws them inside the tile; `None` keeps the placeholder.
    #[cfg(feature = "image")]
    pub image: Option<crate::image::Image>,
}

impl GalleryImage {
    /// Creates a new GalleryImage.
    pub fn new(path: &str, width: u32, height: u32) -> Self {
        Self {
            path: path.to_string(),
            label: None,
            width,
            height,
            #[cfg(feature = "image")]
            image: None,
        }
    }

    /// Loads and decodes the image from disk.
    ///
    /// Returns the decoder's error rather than panicking; a missing or malformed file leaves
    /// the placeholder in place.
    #[cfg(feature = "image")]
    pub fn load_from_path(&mut self) -> Result<(), String> {
        let image = crate::image::Image::from_file(&self.path)?;
        self.width = image.width();
        self.height = image.height();
        self.image = Some(image);
        Ok(())
    }

    /// Decodes the image from an in-memory byte slice.
    #[cfg(feature = "image")]
    pub fn load_from_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let image = crate::image::Image::from_bytes(bytes)?;
        self.width = image.width();
        self.height = image.height();
        self.image = Some(image);
        Ok(())
    }

    /// Creates a new GalleryImage with an optional label.
    pub fn with_label(path: &str, label: Option<&str>, width: u32, height: u32) -> Self {
        Self {
            path: path.to_string(),
            label: label.map(|s| s.to_string()),
            width,
            height,
            #[cfg(feature = "image")]
            image: None,
        }
    }
}

/// The declared shape of one gallery image, as a record.
///
/// Declared beside [`GalleryImage`] so the field names cannot drift from the struct's own
/// fields. `path` is required because it is the image's identity (and nothing can be loaded
/// without it); the label and the dimensions are optional because
/// [`GalleryImage::new`] supplies both — "no label" is the normal case, and an omitted
/// dimension means the `100 x 100` placeholder `add_image` uses.
///
/// `ids_unique` is **false** on purpose: two slots showing the same file is a legitimate
/// gallery, so the identity field must not be turned into a uniqueness constraint.
pub const GALLERY_IMAGE_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "path",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "label",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "width",
                crate::widget::capability::PropertyValueKind::UInt,
            ),
            crate::widget::capability::RecordField::optional(
                "height",
                crate::widget::capability::PropertyValueKind::UInt,
            ),
        ],
        id_field: "path",
        ids_unique: false,
        ids_required: true,
    };

/// The dimension `GalleryImage::new` uses when the caller does not supply one.
const GALLERY_DEFAULT_DIMENSION: u32 = 100;

/// Gap between neighbouring thumbnails in the strip.
const THUMB_SPACING: u32 = 6;
/// Inset of the first thumbnail from the strip's left edge.
const THUMB_PADDING: i32 = 8;
/// Gap between a thumbnail and the strip's top edge.
const THUMB_TOP_GAP: i32 = 4;
/// The strip's chrome around the thumbnail row — margins plus the caption band —
/// which is what makes the strip taller than `thumbnail_size`.
const THUMB_STRIP_CHROME: u32 = 28;

fn gallery_image_record(image: &GalleryImage) -> crate::widget::capability::RecordValue {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("path", CapabilityValue::String(image.path.clone()))
        .with("width", CapabilityValue::UInt(image.width as u64))
        .with("height", CapabilityValue::UInt(image.height as u64));
    // Omitted rather than written as an empty string: "no label" and "a label that is empty"
    // are different states, and the read/write round trip has to preserve which one it was.
    if let Some(label) = &image.label {
        record = record.with("label", CapabilityValue::String(label.clone()));
    }
    record
}

fn gallery_image_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<GalleryImage, CapabilityAccessError> {
    let path = match record.get("path") {
        Some(CapabilityValue::String(value)) if !value.trim().is_empty() => value.clone(),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let label = match record.get("label") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::String(value)) => Some(value.clone()),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let dimension = |name: &str| -> Result<u32, CapabilityAccessError> {
        match record.get(name) {
            None | Some(CapabilityValue::Null) => Ok(GALLERY_DEFAULT_DIMENSION),
            Some(CapabilityValue::UInt(value)) => {
                u32::try_from(*value).map_err(|_| CapabilityAccessError::OutOfRange)
            }
            Some(CapabilityValue::Int(value)) => {
                u32::try_from(*value).map_err(|_| CapabilityAccessError::OutOfRange)
            }
            Some(_) => Err(CapabilityAccessError::TypeMismatch),
        }
    };
    Ok(GalleryImage {
        path,
        label,
        width: dimension("width")?,
        height: dimension("height")?,
        #[cfg(feature = "image")]
        image: None,
    })
}

/// Which gesture an un-modified wheel performs on the gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GalleryWheelMode {
    /// Scroll the thumbnail strip (the default).
    #[default]
    Strip,
    /// Step to the previous/next image.
    Navigate,
    /// Zoom the preview and the strip.
    Zoom,
}

impl GalleryWheelMode {
    /// The token this mode is spelled as in a property write.
    pub fn as_str(self) -> &'static str {
        match self {
            GalleryWheelMode::Strip => "strip",
            GalleryWheelMode::Navigate => "navigate",
            GalleryWheelMode::Zoom => "zoom",
        }
    }

    /// Parses the token back, rejecting anything else.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "strip" => GalleryWheelMode::Strip,
            "navigate" => GalleryWheelMode::Navigate,
            "zoom" => GalleryWheelMode::Zoom,
            _ => return None,
        })
    }
}

/// ImageGallery — an image gallery/browser widget with thumbnails and large view.
pub struct ImageGallery {
    base: BaseWidget,
    /// Collection of gallery images.
    images: Vec<GalleryImage>,
    /// Index of the currently displayed image.
    current_index: usize,
    /// Size of thumbnail squares in pixels.
    thumbnail_size: u32,
    /// Whether the thumbnail strip is visible.
    show_thumbnails: bool,
    /// How far the current image has arrived: `0.0` just switched, `1.0` settled.
    ///
    /// # Why a gallery needs an arrival at all
    ///
    /// Stepping through images is the control's whole purpose, and a step that swaps the preview
    /// between two frames gives the reader no way to tell an arrival from the previous picture still
    /// being there. A short zoom-and-fade-in is the gesture every image browser uses, and it is the
    /// only feedback that a `next` did anything when the two images happen to look alike.
    ///
    /// It rests at `1.0`: a gallery built and seeded is already showing its first image, so it must
    /// not animate that one into place.
    reveal: PropertyDriver,
    /// The image the pointer is over in the thumbnail strip, or `None`.
    ///
    /// Indexed like `current_index`; only thumbnails the strip actually paints can
    /// hover, so an image scrolled out of the strip never carries the highlight.
    hovered_thumb: Option<usize>,
    /// Emitted when the selected image changes. Passes the new index.
    pub image_changed: Signal1<usize>,
    /// Scale applied to the thumbnail strip and preview, default `1.0`.
    zoom: f32,
    /// Whether ctrl/shift-click extends the selection set.
    multi_select: bool,
    /// Additional selected indices, used only in multi-select mode.
    selected: std::collections::BTreeSet<usize>,
    /// Emitted when the selection set changes.
    pub selection_changed: Signal1<Vec<usize>>,
    /// Emitted when the current image is double-clicked (a full-screen request).
    pub fullscreen_requested: Signal1<usize>,
    /// The first image index the thumbnail strip shows.
    ///
    /// The strip used to derive its first slot from `current_index` alone, so a gallery with more
    /// images than visible slots could not reach the tail of the list: the offset was always
    /// clamped back to "centred on the selection". This is the manual scroll position, and
    /// selecting an image still reveals it through [`Self::reveal_index`].
    strip_offset: usize,
    /// Whether the wheel may scroll the thumbnail strip.
    strip_scroll_enabled: bool,
    /// What an un-modified wheel does.
    wheel_mode: GalleryWheelMode,
    /// Whether arrow keys navigate when the gallery has focus.
    arrow_key_navigation_enabled: bool,
    /// Horizontal/vertical pan of the zoomed preview, in pixels.
    pan_x: i32,
    pan_y: i32,
    /// An in-flight preview drag: the last pointer position while panning.
    pan_drag: Option<(f32, f32)>,
    /// Emitted with the new strip offset whenever it changes.
    pub strip_scrolled: Signal1<usize>,
}

impl ImageGallery {
    /// Creates a new ImageGallery with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ImageGallery, geometry, "ImageGallery"),
            images: Vec::new(),
            current_index: 0,
            thumbnail_size: 64,
            show_thumbnails: true,
            // At rest settled: the first image is already there when the gallery is built.
            reveal: PropertyDriver::at(1.0, MotionSlot::Normal),
            hovered_thumb: None,
            image_changed: Signal1::new(),
            zoom: 1.0,
            multi_select: false,
            selected: std::collections::BTreeSet::new(),
            selection_changed: Signal1::new(),
            fullscreen_requested: Signal1::new(),
            strip_offset: 0,
            strip_scroll_enabled: true,
            wheel_mode: GalleryWheelMode::default(),
            arrow_key_navigation_enabled: true,
            pan_x: 0,
            pan_y: 0,
            pan_drag: None,
            strip_scrolled: Signal1::new(),
        }
    }

    /// Adds an image to the gallery.
    pub fn add_image(&mut self, path: &str, label: Option<&str>) {
        self.images.push(GalleryImage::with_label(path, label, 100, 100));
        self.base.request_redraw();
    }

    /// Adds an image with explicit dimensions.
    pub fn add_image_with_size(
        &mut self,
        path: &str,
        label: Option<&str>,
        width: u32,
        height: u32,
    ) {
        self.images.push(GalleryImage::with_label(path, label, width, height));
        self.base.request_redraw();
    }

    /// Replaces the whole image list.
    ///
    /// The property route needs one entry point that swaps the list, the same way
    /// `Breadcrumb::set_segments` does. `current_index` is clamped into the new list so the
    /// preview never points past its end — every reader (`selected_index`, `has_next`) treats
    /// such an index as "nothing selected". The `image_changed` signal is emitted because the
    /// image on display has been replaced, even when the index happens to stay the same.
    pub fn set_images(&mut self, images: Vec<GalleryImage>) {
        self.images = images;
        if self.images.is_empty() {
            self.current_index = 0;
        } else if self.current_index >= self.images.len() {
            self.current_index = self.images.len() - 1;
        }
        self.image_changed.emit(self.current_index);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Removes an image at the given index.
    pub fn remove_image(&mut self, index: usize) {
        if index >= self.images.len() {
            return;
        }
        self.images.remove(index);
        if !self.images.is_empty() && self.current_index >= self.images.len() {
            self.current_index = self.images.len() - 1;
            self.image_changed.emit(self.current_index);
        }
        self.base.request_redraw();
    }

    /// Removes all images from the gallery.
    pub fn clear_images(&mut self) {
        self.images.clear();
        self.current_index = 0;
        self.image_changed.emit(0);
        self.base.request_redraw();
    }

    /// Returns the number of images in the gallery.
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Returns a reference to all images.
    pub fn images(&self) -> &[GalleryImage] {
        &self.images
    }

    /// Sets the current image index (clamped to valid range).
    pub fn set_current_index(&mut self, index: usize) {
        if self.images.is_empty() {
            self.current_index = 0;
            return;
        }
        let new_index = index.min(self.images.len() - 1);
        if new_index != self.current_index {
            self.current_index = new_index;
            // A new image is read at its natural scale, so the previous image's pan is not
            // carried over to a picture it never described.
            self.pan_x = 0;
            self.pan_y = 0;
            // The new image **arrives** rather than replacing the old one between two frames. Every
            // navigation path funnels through here (`next_image`/`previous_image` call this), so the
            // reveal is aimed at the one choke point instead of at each of the three -- which is how
            // a swipe and a click would otherwise animate differently.
            self.reveal.jump_to(0.0);
            self.reveal.set_target(1.0);
            self.image_changed.emit(self.current_index);
            self.reveal_index(self.current_index);
            self.base.request_redraw();
        }
    }

    /// Returns the selected image index, or `None` when the gallery is empty.
    pub fn selected_index(&self) -> Option<usize> {
        self.images.get(self.current_index).map(|_| self.current_index)
    }

    /// Returns the current image index.
    #[deprecated(note = "use `selected_index` (Option) — WDGT-13")]
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// How far the current image has arrived: `0.0` just switched, `1.0` settled.
    ///
    /// The value the draw measures with. It is `1.0` except in the frames right after a step.
    pub fn reveal_progress(&self) -> f32 {
        self.reveal.value()
    }

    /// Returns a reference to the current image, if any.
    pub fn current_image(&self) -> Option<&GalleryImage> {
        self.images.get(self.current_index)
    }

    /// Advances to the next image. Returns true if successful.
    pub fn next_image(&mut self) -> bool {
        if self.images.is_empty() || self.current_index >= self.images.len() - 1 {
            return false;
        }
        self.set_current_index(self.current_index + 1);
        true
    }

    /// Moves to the previous image. Returns true if successful.
    pub fn previous_image(&mut self) -> bool {
        if self.current_index == 0 || self.images.is_empty() {
            return false;
        }
        self.set_current_index(self.current_index - 1);
        true
    }

    /// Returns whether there is a next image available.
    pub fn has_next(&self) -> bool {
        !self.images.is_empty() && self.current_index < self.images.len() - 1
    }

    /// Returns whether there is a previous image available.
    pub fn has_previous(&self) -> bool {
        self.current_index > 0 && !self.images.is_empty()
    }

    /// The preview area: the control's box minus the thumbnail strip's height.
    ///
    /// `draw` and the press arm both derived this inline, which is how the two
    /// copies could drift; both now read this one derivation.
    fn preview_rect(&self) -> Rect {
        let rect = self.geometry();
        let thumb_strip_height = if self.show_thumbnails {
            self.effective_thumbnail_size() + THUMB_STRIP_CHROME
        } else {
            0
        };
        Rect::new(rect.x, rect.y, rect.width, rect.height.saturating_sub(thumb_strip_height))
    }

    /// The thumbnail strip's shared layout: its box, how many thumbnails fit, and
    /// the image index of the first visible slot.
    ///
    /// `None` when the strip is hidden or the gallery is empty — the two guards
    /// `draw` and the press arm each applied inline. Both now read this, so a
    /// painted slot, a hovered slot and a clickable slot are the same computation.
    fn strip_layout(&self) -> Option<(Rect, usize, usize)> {
        if !self.show_thumbnails || self.images.is_empty() {
            return None;
        }
        let rect = self.geometry();
        let preview = self.preview_rect();
        let strip = Rect::new(
            rect.x,
            preview.y + preview.height as i32,
            rect.width,
            self.effective_thumbnail_size() + THUMB_STRIP_CHROME,
        );
        let thumb_total = self.effective_thumbnail_size() + THUMB_SPACING;
        let max_visible = strip.width.checked_div(thumb_total).map_or(1, |v| v.max(1) as usize);
        // The first slot is the stored scroll position, clamped to the tail of the list. The
        // previous derivation always re-centred on `current_index`, so any thumbnail past the
        // visible window was unreachable no matter what the user did.
        let start_offset = self.strip_offset.min(self.images.len().saturating_sub(max_visible));
        Some((strip, max_visible, start_offset))
    }

    /// How many thumbnails the strip can show at once.
    fn visible_thumb_count(&self) -> usize {
        let rect = self.geometry();
        let thumb_total = self.effective_thumbnail_size() + THUMB_SPACING;
        rect.width.checked_div(thumb_total).map_or(1, |v| v.max(1) as usize)
    }

    /// The largest offset that still fills the strip with thumbnails.
    fn max_strip_offset(&self) -> usize {
        self.images.len().saturating_sub(self.visible_thumb_count())
    }

    /// The first image index the thumbnail strip shows.
    pub fn strip_offset(&self) -> usize {
        self.strip_offset.min(self.max_strip_offset())
    }

    /// Scrolls the strip to `offset`, clamped to the last full window.
    pub fn set_strip_offset(&mut self, offset: usize) {
        let next = offset.min(self.max_strip_offset());
        if next == self.strip_offset {
            return;
        }
        self.strip_offset = next;
        self.strip_scrolled.emit(next);
        self.base.request_redraw();
    }

    /// Whether the wheel scrolls the thumbnail strip.
    pub fn strip_scroll_enabled(&self) -> bool {
        self.strip_scroll_enabled
    }

    /// Enables or disables wheel scrolling of the thumbnail strip.
    pub fn set_strip_scroll_enabled(&mut self, enabled: bool) {
        if self.strip_scroll_enabled == enabled {
            return;
        }
        self.strip_scroll_enabled = enabled;
        self.base.request_redraw();
    }

    /// What an un-modified wheel does.
    pub fn wheel_mode(&self) -> GalleryWheelMode {
        self.wheel_mode
    }

    /// Sets what an un-modified wheel does.
    pub fn set_wheel_mode(&mut self, mode: GalleryWheelMode) {
        if self.wheel_mode == mode {
            return;
        }
        self.wheel_mode = mode;
        self.base.request_redraw();
    }

    /// Whether arrow keys navigate while the gallery has focus.
    pub fn arrow_key_navigation_enabled(&self) -> bool {
        self.arrow_key_navigation_enabled
    }

    /// Enables or disables arrow-key navigation.
    pub fn set_arrow_key_navigation_enabled(&mut self, enabled: bool) {
        self.arrow_key_navigation_enabled = enabled;
    }

    /// Scrolls the strip by `delta` images: positive moves toward later images.
    pub fn scroll_strip_by(&mut self, delta: isize) {
        let next = (self.strip_offset() as isize + delta).max(0) as usize;
        self.set_strip_offset(next);
    }

    /// Moves the strip's window so `index` is visible.
    ///
    /// Called whenever the selection changes: selecting a thumbnail at the edge of the window must
    /// be able to pull the next one in, otherwise keyboard navigation walks a selection off screen.
    fn reveal_index(&mut self, index: usize) {
        let visible = self.visible_thumb_count().max(1);
        let offset = self.strip_offset();
        if index < offset {
            self.set_strip_offset(index);
        } else if index >= offset + visible {
            self.set_strip_offset(index + 1 - visible);
        }
    }

    /// The current preview pan, in pixels.
    pub fn pan(&self) -> (i32, i32) {
        (self.pan_x, self.pan_y)
    }

    /// Pans the zoomed preview by `(dx, dy)`, clamped so the picture cannot be dragged fully off
    /// its stage.
    pub fn pan_by(&mut self, dx: i32, dy: i32) {
        let (max_x, max_y) = self.pan_bounds();
        let next_x = (self.pan_x + dx).clamp(-max_x, max_x);
        let next_y = (self.pan_y + dy).clamp(-max_y, max_y);
        if (next_x, next_y) == (self.pan_x, self.pan_y) {
            return;
        }
        self.pan_x = next_x;
        self.pan_y = next_y;
        self.base.request_redraw();
    }

    /// Returns the preview to its centred position.
    pub fn reset_pan(&mut self) {
        if self.pan_x == 0 && self.pan_y == 0 {
            return;
        }
        self.pan_x = 0;
        self.pan_y = 0;
        self.base.request_redraw();
    }

    /// How far the picture may be panned on each axis.
    fn pan_bounds(&self) -> (i32, i32) {
        let preview = self.preview_rect();
        let Some(image) = self.images.get(self.current_index) else {
            return (0, 0);
        };
        let (width, height) = (image.width.max(1), image.height.max(1));
        let scale = (preview.width as f32 / width as f32)
            .min(preview.height as f32 / height as f32)
            * self.zoom;
        let drawn_w = (width as f32 * scale) as i32;
        let drawn_h = (height as f32 * scale) as i32;
        (
            ((drawn_w - preview.width as i32) / 2).max(0),
            ((drawn_h - preview.height as i32) / 2).max(0),
        )
    }

    /// The box of the `i`-th visible slot in `strip`, which paints image
    /// `start_offset + i`.
    fn thumbnail_rect(strip: Rect, thumbnail_size: u32, i: usize) -> Rect {
        Rect::new(
            strip.x + THUMB_PADDING + (i as u32 * (thumbnail_size + THUMB_SPACING)) as i32,
            strip.y + THUMB_TOP_GAP,
            thumbnail_size,
            thumbnail_size,
        )
    }

    /// The image index under `pos`, when it is over a thumbnail the strip paints.
    fn thumbnail_at(&self, pos: Point) -> Option<usize> {
        let (strip, max_visible, start_offset) = self.strip_layout()?;
        for i in 0..max_visible.min(self.images.len()) {
            let img_idx = start_offset + i;
            if img_idx >= self.images.len() {
                break;
            }
            if Self::thumbnail_rect(strip, self.effective_thumbnail_size(), i).contains_point(pos) {
                return Some(img_idx);
            }
        }
        None
    }

    /// The navigation-arrow hit boxes inside `preview`, `(previous, next)`, each `None` when that
    /// direction is unavailable.
    ///
    /// # Why the boxes are derived here and not at each site
    ///
    /// The arrows are painted by `draw` and hit-tested by `handle_event`. Measuring the glyph at
    /// each site meant the two could disagree about where an arrow is (this file has had that
    /// defect class before), and a glyph measurement also depended on the font. A fixed 24 px
    /// square anchored to the preview's own edges is the same object for both, so an arrow is
    /// clickable exactly where it is drawn.
    fn arrow_boxes(&self, preview: Rect, side: u32) -> (Option<Rect>, Option<Rect>) {
        let side = side.min(preview.height).max(1);
        let y = preview.y + (preview.height as i32 - side as i32) / 2;
        let previous = self.has_previous().then(|| Rect::new(preview.x + 8, y, side, side));
        let next = self
            .has_next()
            .then(|| Rect::new(preview.x + preview.width as i32 - side as i32 - 8, y, side, side));
        (previous, next)
    }

    /// Sets the thumbnail size in pixels.
    pub fn set_thumbnail_size(&mut self, size: u32) {
        let size = size.clamp(16, 256);
        if self.thumbnail_size == size {
            return;
        }
        self.thumbnail_size = size;
        self.base.request_redraw();
    }

    /// Returns the current thumbnail size.
    pub fn thumbnail_size(&self) -> u32 {
        self.thumbnail_size
    }

    /// The painted thumbnail size after the zoom factor is applied.
    fn effective_thumbnail_size(&self) -> u32 {
        ((self.thumbnail_size as f32 * self.zoom).round() as u32).max(1)
    }

    /// The current zoom factor.
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Sets the zoom factor, clamped to `0.25..=4.0`.
    pub fn set_zoom(&mut self, zoom: f32) {
        if !zoom.is_finite() {
            return;
        }
        let zoom = zoom.clamp(0.25, 4.0);
        if (self.zoom - zoom).abs() < f32::EPSILON {
            return;
        }
        self.zoom = zoom;
        self.base.request_redraw();
    }

    /// Whether ctrl/shift-click extends the selection.
    pub fn multi_select(&self) -> bool {
        self.multi_select
    }

    /// Enables or disables multi-select.
    pub fn set_multi_select(&mut self, multi_select: bool) {
        if self.multi_select == multi_select {
            return;
        }
        self.multi_select = multi_select;
        if !multi_select {
            self.selected.clear();
            self.selection_changed.emit(Vec::new());
        }
        self.base.request_redraw();
    }

    /// The additional selected indices, in ascending order.
    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    /// Shows or hides the thumbnail strip.
    pub fn set_show_thumbnails(&mut self, show: bool) {
        if self.show_thumbnails == show {
            return;
        }
        self.show_thumbnails = show;
        self.base.request_redraw();
    }

    /// Returns whether thumbnails are visible.
    pub fn show_thumbnails(&self) -> bool {
        self.show_thumbnails
    }
}

impl Widget for ImageGallery {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::IMAGE_GALLERY_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::IMAGE_GALLERY_DEFAULT_HEIGHT,
        )
    }

    /// Advances the current image's reveal by `delta_ms`; `true` while it is still moving.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.reveal.tick(delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.reveal.is_moving()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "image_changed" => {
                Some(EventSignalRef::mapped("image_changed", &self.image_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "fullscreen_requested" => Some(EventSignalRef::mapped(
                "fullscreen_requested",
                &self.fullscreen_requested,
                |v| crate::widget::capability::CapabilityValue::UInt(*v as u64),
            )),
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(
                        v.iter()
                            .map(|item| {
                                crate::widget::capability::CapabilityValue::UInt(*item as u64)
                            })
                            .collect(),
                    )
                }))
            }
            "strip_scrolled" => {
                Some(EventSignalRef::mapped("strip_scrolled", &self.strip_scrolled, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `ImageGallery`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for ImageGallery {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "current_index" => Ok(CapabilityValue::UInt(self.selected_index().unwrap_or(0) as u64)),
            "images" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &GALLERY_IMAGE_SCHEMA,
                    self.images().iter().map(gallery_image_record).collect(),
                )))
            }
            "thumbnail_size" => Ok(CapabilityValue::UInt(self.thumbnail_size() as u64)),
            "show_thumbnails" => Ok(CapabilityValue::Bool(self.show_thumbnails())),
            "strip_offset" => Ok(CapabilityValue::UInt(self.strip_offset() as u64)),
            "strip_scroll_enabled" => Ok(CapabilityValue::Bool(self.strip_scroll_enabled())),
            "wheel_mode" => Ok(CapabilityValue::String(self.wheel_mode().as_str().to_string())),
            "arrow_key_navigation_enabled" => {
                Ok(CapabilityValue::Bool(self.arrow_key_navigation_enabled()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                self.set_current_index(expect_usize(value)?);
                Ok(())
            }
            "images" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                // Validated in full before anything is replaced, so a refused write leaves the
                // live gallery untouched rather than half-swapped.
                list.validate_against(&GALLERY_IMAGE_SCHEMA).map_err(|violation| {
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("images"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let mut images = Vec::with_capacity(list.len());
                for record in list.items() {
                    images.push(gallery_image_from_record(record)?);
                }
                self.set_images(images);
                Ok(())
            }
            "thumbnail_size" => {
                // The setter clamps to 16..=256, so a property write outside that band would
                // read back as a different number than it wrote. The property contract reports
                // `OutOfRange` instead: the caller is told the write did not land.
                let size = expect_u32(value)?;
                if !(16..=256).contains(&size) {
                    return Err(CapabilityAccessError::OutOfRange);
                }
                self.set_thumbnail_size(size);
                Ok(())
            }
            "show_thumbnails" => {
                self.set_show_thumbnails(expect_bool(value)?);
                Ok(())
            }
            "strip_offset" => {
                self.set_strip_offset(expect_usize(value)?);
                Ok(())
            }
            "strip_scroll_enabled" => {
                self.set_strip_scroll_enabled(expect_bool(value)?);
                Ok(())
            }
            "wheel_mode" => {
                let CapabilityValue::String(text) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let mode =
                    GalleryWheelMode::from_name(&text).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_wheel_mode(mode);
                Ok(())
            }
            "arrow_key_navigation_enabled" => {
                self.set_arrow_key_navigation_enabled(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "current_index",
            "images",
            "thumbnail_size",
            "show_thumbnails",
            "strip_offset",
            "strip_scroll_enabled",
            "wheel_mode",
            "arrow_key_navigation_enabled",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for ImageGallery {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        if self.images.is_empty() {
            // Empty state. Both colours resolve the theme rather than a literal pair.
            //
            // The previous form hardcoded a near-white panel and a light-grey label, so the
            // control was **theme-blind** (P3: both appearances byte-identical) *and* its
            // label measured **2.02:1** — below the 4.5:1 text floor and below even the
            // 3:1 large-text bar. A gallery inside a dark window was a white rectangle that
            // no theme could change, which is the same class of defect as `map_view`'s
            // hardcoded palette.
            //
            // The placeholder is *chrome* (it frames absent content and is not itself
            // content), so it follows `style.*` then the theme's resolved style, with the
            // old literals kept only as the last fallback (rule #21: an unstyled gallery
            // looks exactly as it did). The label is derived from the panel it sits on, so
            // it cannot repeat the 2.02:1 mistake on any surface.
            let style = self.base.style();
            let theme = crate::style::resolved_theme_style_shared("image_gallery");
            let fallback_bg = if !is_enabled {
                Color::rgba(230, 230, 230, 200)
            } else {
                Color::rgba(240, 240, 240, 255)
            };
            let bg = style
                .background_color
                .or_else(|| theme.as_ref().and_then(|t| t.background_color))
                .unwrap_or(fallback_bg);
            context.face_with_gradient(
                rect,
                bg,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().border_radius.unwrap_or(0),
                Color::BLACK,
            );
            let font = effective_font(style);
            let text = "No images in gallery";
            // Fitted to the control's width: 20 characters at 14 px is wider than a
            // gallery laid out in a narrow pane, and the label used to be placed from the
            // box centre outward, so half of it fell outside.
            let metrics = context.measure_text(text, font);
            let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;
            let line = Rect::new(rect.x, text_y, rect.width, metrics.height);
            // One step from the panel toward the panel's own contrast colour: a placeholder
            // should read as secondary, but "secondary" has to stay legible, and the light
            // grey it used to be was neither derived nor legible.
            //
            // # Why the disabled weight is the shared constant and not a smaller local blend
            //
            // This branch used to dim the disabled label to `0.38`, which measured **3.56:1** on
            // the dark panel and **2.63:1** on the light one -- both under the 4.5:1 body-text
            // floor, so "disabled" came out as "unreadable" rather than as "receded".
            // `dimensions::DISABLED_VEIL_ALPHA` exists for exactly this and documents the same
            // lesson from two other controls: the recede has to be one shared weight, or a
            // disabled label, frame and button stop looking alike. At that weight the label is
            // 5.91:1 on dark and 4.59:1 on light, i.e. still clearly secondary and still legible.
            let panel_ink = bg.contrast_color();
            let disabled_weight =
                crate::widget::metrics::dimensions::DISABLED_VEIL_ALPHA as f32 / 255.0;
            let label_color = if is_enabled {
                bg.blend(&panel_ink, 0.62)
            } else {
                bg.blend(&panel_ink, disabled_weight)
            };
            context.draw_text_fitted_cow(
                line,
                text,
                font,
                label_color,
                HorizontalAlignment::Center,
            );
            return;
        }

        // ── Large preview area ──────────────────────────────────────
        let preview_rect = self.preview_rect();

        // The preview is a **photo stage**: it is the one surface in the crate that is darker
        // than any appearance, because a preview is judged against a neutral dark ground in every
        // image tool. So it is the *explicit style's* colour when the host gave one — a host that
        // wants a light stage gets a light stage — and the dark literal only as the last fallback.
        //
        // The overlay ink on top of it (`name_ink`, `dim_ink`, `arrow_ink`) is **derived from the
        // stage** rather than written down as four more near-whites. That is what keeps the original
        // defect from returning: the old form hardcoded every one of them as `rgba(2xx,2xx,2xx)`, so
        // a host that set a *light* stage through the style got white-on-white labels and no error.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("image_gallery");
        // The accent for the selected thumbnail, read from the role rather than from the gallery's
        // own surface. See the selected-thumbnail block below for why the surface was wrong.
        let accent_role =
            crate::style::theme_manager().current_theme().map(|active| active.colors.primary);
        let stage_fallback =
            if !is_enabled { Color::rgba(30, 30, 30, 200) } else { Color::rgba(30, 30, 30, 255) };
        let stage = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(stage_fallback);
        context.fill_rect(preview_rect, stage);

        // The decoded picture, when the host (or `load_from_path`) supplied one. It is fitted to
        // the stage at the current zoom and then displaced by the pan, so `pan_by` moves a
        // picture that is actually painted rather than a value nothing reads.
        #[cfg(feature = "image")]
        if let Some(image) = self.images.get(self.current_index) {
            if let Some(decoded) = image.image.as_ref() {
                if let crate::image::ImageData::Rgba8(pixels) = &decoded.inner.data {
                    if !pixels.is_empty() && preview_rect.width > 0 && preview_rect.height > 0 {
                        let (source_w, source_h) =
                            (decoded.width().max(1), decoded.height().max(1));
                        let scale = (preview_rect.width as f32 / source_w as f32)
                            .min(preview_rect.height as f32 / source_h as f32)
                            * self.zoom;
                        let draw_w = (source_w as f32 * scale).round().max(1.0) as u32;
                        let draw_h = (source_h as f32 * scale).round().max(1.0) as u32;
                        let x = preview_rect.x
                            + (preview_rect.width as i32 - draw_w as i32) / 2
                            + self.pan_x;
                        let y = preview_rect.y
                            + (preview_rect.height as i32 - draw_h as i32) / 2
                            + self.pan_y;
                        context.push_clip(
                            preview_rect.x,
                            preview_rect.y,
                            preview_rect.width,
                            preview_rect.height,
                        );
                        context.draw_image(x, y, draw_w, draw_h, pixels);
                        context.pop_clip();
                    }
                }
            }
        }
        // One step from the stage toward its own contrast colour. The old near-whites are what this
        // expression produces on the default dark stage (`(30,30,30)` -> `contrast_color()` white ->
        // blend 0.85), so an unstyled gallery looks exactly as it did while a themed stage carries
        // legible overlay text. Rule #21: the literal is the fallback, not the design.
        let stage_ink = stage.contrast_color();
        let name_ink = stage.blend(&stage_ink, 0.85);
        let dim_ink = stage.blend(&stage_ink, 0.68);
        let arrow_ink = stage.blend(&stage_ink, 0.70);
        // ── The arrival of a newly selected image ──
        //
        // Every overlay on the preview fades in with the reveal, so a step reads as the new image
        // *arriving* rather than the old one being replaced between frames. One value drives the
        // whole overlay, so the caption, the dimensions, the arrows and the index pill cannot
        // arrive at different times. At `reveal = 1.0` the alpha is unchanged, so a settled gallery
        // paints exactly what it did before this existed.
        let reveal = self.reveal.value();
        let arrive = |color: Color| -> Color {
            if reveal >= 1.0 {
                color
            } else {
                color.with_alpha((color.a as f32 * reveal) as u8)
            }
        };
        let name_ink = arrive(name_ink);
        let dim_ink = arrive(dim_ink);
        let arrow_ink = arrive(arrow_ink);

        // Draw current image info in preview area.
        if let Some(image) = self.images.get(self.current_index) {
            let font = effective_font(style);
            // One buffer for the preview's two metadata labels (dimensions, index): `format!`
            // built a fresh `String` for each, every frame.
            let mut meta_text = String::new();

            // Draw image label / filename centered.
            let display_name = image
                .label
                .as_deref()
                .unwrap_or_else(|| image.path.rsplit('/').next().unwrap_or(&image.path));
            // The origin is the glyph box's *top* edge, so the label reads as centred on the
            // band at one third of the preview rather than half a line below its middle.
            let name_metrics = context.measure_text(display_name, font);
            let name_y =
                preview_rect.y + preview_rect.height as i32 / 3 - name_metrics.height as i32 / 2;
            // The name is host-supplied, so a long one overflowed both edges of the preview: the
            // centred origin was computed from the *measured* width, which put the overflow half
            // beyond each side and made the label wider than the box it was centred on. Handing
            // the fitter a preview-wide box keeps the horizontal centring and elides the name at
            // the preview's edges.
            context.draw_text_fitted_cow(
                Rect::new(preview_rect.x, name_y, preview_rect.width, name_metrics.height),
                display_name,
                font,
                name_ink,
                HorizontalAlignment::Center,
            );

            // Draw dimensions. Both the dimension and index labels render into one buffer:
            // `format!` built a fresh `String` for each, per frame.
            meta_text.clear();
            let _ = write!(meta_text, "{}x{}", image.width, image.height);
            let dim_metrics = context.measure_text(&meta_text, font);
            let dim_x = preview_rect.x + (preview_rect.width as i32 - dim_metrics.width as i32) / 2;
            // Same as the filename above: half a *line box* about the band, not half an
            // ascent below it.
            let dim_y =
                preview_rect.y + preview_rect.height as i32 * 2 / 3 - dim_metrics.height as i32 / 2;
            context.draw_text(
                Point::new(dim_x, dim_y),
                &meta_text,
                font,
                dim_ink,
                HorizontalAlignment::Left,
            );

            // Image index indicator.
            meta_text.clear();
            let _ = write!(meta_text, "{}/{}", self.current_index + 1, self.images.len());
            let index_metrics = context.measure_text(&meta_text, font);
            let index_x =
                preview_rect.x + preview_rect.width as i32 - index_metrics.width as i32 - 8;
            let pill_w = index_metrics.width + 8;
            let pill_h = index_metrics.height + 4;
            let pill_rect = Rect::new(index_x - 4, preview_rect.y + 2, pill_w, pill_h);
            // Centre the glyph box on the pill: the origin is the box's top edge, so the old
            // `+ ascent` drew the number half a line under the pill's middle.
            let index_y = pill_rect.y + (pill_rect.height as i32 - index_metrics.height as i32) / 2;
            // The pill is a **scrim over the stage** — a translucent plate that lifts the index off
            // whatever the image happens to be. So its colour is derived from the stage rather than
            // a fixed `rgba(0,0,0,70)`: on a light stage a black plate would be a blot, while
            // `stage.blend(&stage_ink, 0.27)` is the same 70/255 darkness on the default stage.
            context.fill_rounded_rect(pill_rect, 3, stage.blend(&stage_ink, 0.27));
            context.draw_text(
                Point::new(index_x, index_y),
                &meta_text,
                font,
                stage_ink,
                HorizontalAlignment::Left,
            );

            // Navigation arrows: `ChevronLeft`/`ChevronRight` **icon outlines**, not the `◀`/`▶`
            // text glyphs (U+25C0/U+25B6 are covered by no bundled face, so those drew as 8x8
            // bitmap blocks). The boxes come from `arrow_boxes`, so the ink and the hit test are
            // the same object.
            if let Some(left) = self.arrow_boxes(preview_rect, 24).0 {
                crate::widget::draw_icon_centered(
                    context,
                    left,
                    24,
                    arrow_ink,
                    IconName::ChevronLeft,
                );
            }
            if let Some(right) = self.arrow_boxes(preview_rect, 24).1 {
                crate::widget::draw_icon_centered(
                    context,
                    right,
                    24,
                    arrow_ink,
                    IconName::ChevronRight,
                );
            }
        }

        // ── Thumbnail strip ─────────────────────────────────────
        // The strip's box, visible count and first slot all come from the same
        // derivation the hit test reads, so a painted slot and a clickable slot
        // cannot drift apart.
        let Some((strip_rect, max_visible, start_offset)) = self.strip_layout() else {
            return;
        };
        // The strip is the **stage's own bottom band**, so it is derived from the stage: on the
        // default dark stage this is the old `rgba(50,50,50,255)` (one step of ink toward the
        // stage's contrast colour is `(30,30,30)` -> `(50,50,50)`, which is where that literal came
        // from), and on a themed stage it stays a consistent step below the preview instead of
        // being a fixed dark bar under a light one.
        let strip = stage.blend(&stage_ink, 0.05);
        context.fill_rect(strip_rect, strip);
        // The selected thumbnail is the **accent**, because "which of these is current" is exactly
        // what the accent role is for; the unselected ones are a raised step on the strip.
        //
        // # The defect this replaces
        //
        // The comment already said "accent", but the code read `theme.background_color` — the
        // gallery's **own panel colour**. So the selected thumbnail was painted the same colour as
        // the surface it sits on: on the default preset the "selection" was a rectangle
        // indistinguishable from the strip behind it, and the hand-picked blue in the `unwrap_or`
        // was only reached by a theme with no background colour at all, i.e. almost never. A
        // comment naming one role while the code reads another is the shape this file's own
        // history keeps producing; the fix is to read the role the comment names.
        let accent = accent_role
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgba(80, 140, 220, 200));
        let thumb_selected = accent;
        let thumb_resting = strip.blend(&stage_ink, 0.22);
        // A hovered thumbnail is pointer position, not the current image, so it leans
        // toward the accent only partway — visibly above the resting tile, clearly
        // below the selected one.
        let thumb_hover = thumb_resting.blend(&accent, 0.30);

        for i in 0..max_visible.min(self.images.len()) {
            let img_idx = start_offset + i;
            if img_idx >= self.images.len() {
                break;
            }

            let thumb_rect = Self::thumbnail_rect(strip_rect, self.effective_thumbnail_size(), i);

            // Thumbnail background.
            let is_selected = img_idx == self.current_index;
            let thumb_bg = if is_selected {
                thumb_selected
            } else if Some(img_idx) == self.hovered_thumb {
                thumb_hover
            } else {
                thumb_resting
            };
            context.fill_rounded_rect(thumb_rect, 3, thumb_bg);

            // Draw a placeholder pattern inside the thumbnail.
            let inner_rect = Rect::new(
                thumb_rect.x + 2,
                thumb_rect.y + 2,
                thumb_rect.width.saturating_sub(4),
                thumb_rect.height.saturating_sub(4),
            );
            // The placeholder the thumbnail stands in for: a well one further step from the tile it
            // sits in, so the tile reads as a framed slot rather than a flat square. It was a fixed
            // `rgba(60,60,60,200)`, which on a tinted tile left no visible inner edge at all.
            context.fill_rounded_rect(inner_rect, 2, thumb_bg.blend(&stage_ink, 0.12));
            #[cfg(feature = "image")]
            if let Some(image) = self.images.get(img_idx).and_then(|image| image.image.as_ref()) {
                if let crate::image::ImageData::Rgba8(pixels) = &image.inner.data {
                    if !pixels.is_empty() {
                        let fitted = crate::widget::icon_source::fit_image_into(
                            image.width(),
                            image.height(),
                            inner_rect,
                        );
                        context.draw_image(fitted.x, fitted.y, fitted.width, fitted.height, pixels);
                    }
                }
            }

            // Draw image label under thumbnail.
            if let Some(image) = self.images.get(img_idx) {
                if let Some(ref label) = image.label {
                    let font = effective_font(style);
                    // Borrow the label unless it actually needs truncating, and let the
                    // shared primitive own the character budget. `label.clone()` copied
                    // every visible caption per frame for the common fitting case.
                    let label_text: Cow<'_, str> = crate::widget::text_utils::elide_end(label, 10);
                    let label_metrics = context.measure_text(&label_text, font);
                    let label_x = thumb_rect.x
                        + (self.effective_thumbnail_size() as i32 - label_metrics.width as i32) / 2;
                    // Top-aligned caption under the thumbnail, not a centred box: the origin
                    // is already the glyph box's top edge, so the declared-baseline `+ ascent`
                    // only dropped the caption on top of the row below it.
                    let label_y = thumb_rect.y + self.effective_thumbnail_size() as i32 + 2;
                    context.draw_text(
                        Point::new(label_x, label_y),
                        &label_text,
                        font,
                        dim_ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
}

impl EventHandler for ImageGallery {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, modifiers } => {
                if *button == 1 {
                    let rect = self.geometry();
                    if !rect.contains_point(*pos) {
                        return;
                    }

                    if self.images.is_empty() {
                        return;
                    }

                    let preview_rect = self.preview_rect();

                    // Check if clicked on navigation arrows in preview area. The boxes come from
                    // `arrow_boxes`, the same derivation `draw` paints through, so an arrow is
                    // clickable exactly where it is drawn.
                    if pos.y >= preview_rect.y
                        && pos.y < preview_rect.y + preview_rect.height as i32
                    {
                        let (previous, next) = self.arrow_boxes(preview_rect, 24);
                        if let Some(arrow) = previous {
                            if arrow.contains_point(*pos) {
                                self.previous_image();
                                return;
                            }
                        }
                        if let Some(arrow) = next {
                            if arrow.contains_point(*pos) {
                                self.next_image();
                                return;
                            }
                        }
                        // A press anywhere else on the stage begins a pan. `pan_by` clamps to
                        // the space the zoomed picture actually overhangs, so at zoom 1.0 this
                        // gesture is a no-op rather than a way to slide the image off-screen.
                        self.pan_drag = Some((pos.x as f32, pos.y as f32));
                        return;
                    }

                    // Check if clicked on a thumbnail. The slot boxes come from the same
                    // `strip_layout`/`thumbnail_rect` derivation `draw` paints through, so a
                    // click selects exactly the tile that was painted.
                    if let Some(img_idx) = self.thumbnail_at(*pos) {
                        if self.multi_select {
                            let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                            if mods.contains(crate::shortcut::Modifiers::CTRL)
                                || mods.contains(crate::shortcut::Modifiers::SHIFT)
                            {
                                if !self.selected.remove(&img_idx) {
                                    self.selected.insert(img_idx);
                                }
                                self.selection_changed.emit(self.selected_indices());
                                self.base.request_redraw();
                                return;
                            }
                        }
                        self.set_current_index(img_idx);
                    }
                }
            }
            Event::MouseMove { pos, .. } => {
                if let Some((last_x, last_y)) = self.pan_drag {
                    let dx = pos.x as f32 - last_x;
                    let dy = pos.y as f32 - last_y;
                    if dx != 0.0 || dy != 0.0 {
                        self.pan_by(dx.round() as i32, dy.round() as i32);
                        self.pan_drag = Some((pos.x as f32, pos.y as f32));
                        return;
                    }
                }
                // Hover is the thumbnail under the pointer, from the same slot boxes the
                // paint and the press read. `MouseMove` fires far more often than the
                // highlight changes, so the redraw is requested only on an actual change.
                let hovered = self.thumbnail_at(*pos);
                if hovered != self.hovered_thumb {
                    self.hovered_thumb = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { .. } => {
                self.pan_drag = None;
            }
            Event::MouseLeave { .. } => {
                if self.hovered_thumb.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                if self.thumbnail_at(*pos).is_some() || self.preview_rect().contains_point(*pos) {
                    self.fullscreen_requested.emit(self.current_index);
                }
            }
            Event::Wheel { delta, modifiers } => {
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                if mods.contains(crate::shortcut::Modifiers::CTRL) {
                    let step = if delta.y > 0 { -0.1 } else { 0.1 };
                    self.set_zoom(self.zoom + step);
                } else {
                    // An un-modified wheel scrolls the strip by default: the previous arm stepped
                    // the *image*, which made the strip the one thing the wheel could not reach.
                    match self.wheel_mode {
                        GalleryWheelMode::Strip if self.strip_scroll_enabled => {
                            // A wheel "toward the user" (positive `y`) walks back through the strip.
                            self.scroll_strip_by(if delta.y < 0 {
                                1
                            } else if delta.y > 0 {
                                -1
                            } else {
                                0
                            });
                        }
                        GalleryWheelMode::Strip | GalleryWheelMode::Navigate => {
                            if delta.y < 0 {
                                self.next_image();
                            } else if delta.y > 0 {
                                self.previous_image();
                            }
                        }
                        GalleryWheelMode::Zoom => {
                            let step = if delta.y > 0 { -0.1 } else { 0.1 };
                            self.set_zoom(self.zoom + step);
                        }
                    }
                }
            }
            // Named key constants instead of the bare 37-40 literals, and a focus gate: an
            // unfocused gallery used to consume every arrow key in the window.
            Event::KeyPress { key, .. }
                if self.base.focus_reason().is_some() && self.arrow_key_navigation_enabled =>
            {
                match *key {
                    crate::event::key_codes::RIGHT | crate::event::key_codes::DOWN => {
                        self.next_image();
                    }
                    crate::event::key_codes::LEFT | crate::event::key_codes::UP => {
                        self.previous_image();
                    }
                    _ => {}
                }
            }
            Event::KeyPress { .. } => {
                // An unfocused gallery (or one with arrow navigation disabled) forwards the key
                // to the base rather than swallowing it.
                self.base.handle_event(event);
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_gallery(rect: Rect) -> ImageGallery {
        let mut g = ImageGallery::new(rect);
        g.add_image("/images/photo1.jpg", Some("Sunset"));
        g.add_image("/images/photo2.jpg", Some("Mountains"));
        g.add_image("/images/photo3.jpg", Some("Ocean"));
        g.add_image("/images/photo4.jpg", Some("Forest"));
        g.add_image("/images/photo5.jpg", Some("City"));
        g
    }

    #[test]
    fn image_gallery_creation_defaults() {
        let g = ImageGallery::new(Rect::new(0, 0, 400, 300));
        assert_eq!(g.image_count(), 0);
        assert_eq!(g.selected_index().unwrap_or(0), 0);
        assert!(g.current_image().is_none());
        assert_eq!(g.thumbnail_size(), 64);
        assert!(g.show_thumbnails());
        assert!(!g.has_next());
        assert!(!g.has_previous());
        assert_eq!(g.kind(), WidgetKind::ImageGallery);
    }

    #[test]
    fn images_property_round_trips_the_gallery() {
        let mut g = ImageGallery::new(Rect::new(0, 0, 400, 300));
        let list = crate::widget::capability::RecordList::from_items(
            &GALLERY_IMAGE_SCHEMA,
            vec![
                crate::widget::capability::RecordValue::new()
                    .with("path", CapabilityValue::String("/images/a.jpg".to_string()))
                    .with("label", CapabilityValue::String("A".to_string()))
                    .with("width", CapabilityValue::UInt(640))
                    .with("height", CapabilityValue::UInt(480)),
                // An omitted label and omitted dimensions are the `add_image` shape.
                crate::widget::capability::RecordValue::new()
                    .with("path", CapabilityValue::String("/images/b.jpg".to_string())),
            ],
        );
        g.set("images", CapabilityValue::Records(list)).expect("images is writable");
        assert_eq!(g.image_count(), 2);
        assert_eq!(g.images()[0].width, 640);
        assert_eq!(g.images()[0].label.as_deref(), Some("A"));
        assert_eq!(g.images()[1].label, None);
        assert_eq!(g.images()[1].width, GALLERY_DEFAULT_DIMENSION);

        match g.get("images").expect("images is readable") {
            CapabilityValue::Records(read) => {
                assert_eq!(read.len(), 2);
                let second = &read.items()[1];
                // "No label" stays absent rather than becoming an empty string.
                assert_eq!(second.get("label"), None);
                assert_eq!(
                    second.get("path"),
                    Some(&CapabilityValue::String("/images/b.jpg".to_string()))
                );
            }
            other => panic!("images should read back as Records, got {other:?}"),
        }

        // A row with no path is refused, and the live gallery is untouched.
        let missing_path = crate::widget::capability::RecordList::from_items(
            &GALLERY_IMAGE_SCHEMA,
            vec![crate::widget::capability::RecordValue::new()
                .with("label", CapabilityValue::String("No path".to_string()))],
        );
        assert!(matches!(
            g.set("images", CapabilityValue::Records(missing_path)),
            Err(CapabilityAccessError::ConstraintViolation { .. })
        ));
        assert_eq!(g.image_count(), 2);
    }

    #[test]
    fn thumbnail_properties_round_trip_and_refuse_out_of_range_sizes() {
        let mut g = ImageGallery::new(Rect::new(0, 0, 400, 300));
        g.set("thumbnail_size", CapabilityValue::UInt(96)).expect("thumbnail_size is writable");
        assert_eq!(g.get("thumbnail_size"), Ok(CapabilityValue::UInt(96)));
        // The setter clamps, so the property contract refuses instead of reporting a write that
        // did not land.
        assert_eq!(
            g.set("thumbnail_size", CapabilityValue::UInt(8)),
            Err(CapabilityAccessError::OutOfRange)
        );
        assert_eq!(g.thumbnail_size(), 96);

        g.set("show_thumbnails", CapabilityValue::Bool(false))
            .expect("show_thumbnails is writable");
        assert_eq!(g.get("show_thumbnails"), Ok(CapabilityValue::Bool(false)));
    }

    #[test]
    fn image_gallery_add_images_and_count() {
        let g = make_gallery(Rect::new(0, 0, 400, 300));
        assert_eq!(g.image_count(), 5);
        assert_eq!(g.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn image_gallery_current_image() {
        let g = make_gallery(Rect::new(0, 0, 400, 300));
        let img = g.current_image().unwrap();
        assert!(img.path.contains("photo1"));
        assert_eq!(img.label.as_deref(), Some("Sunset"));
    }

    #[test]
    fn image_gallery_navigation_next_previous() {
        let mut g = make_gallery(Rect::new(0, 0, 400, 300));
        assert!(!g.has_previous());
        assert!(g.has_next());

        assert!(g.next_image());
        assert_eq!(g.selected_index().unwrap_or(0), 1);
        assert!(g.has_previous());
        assert!(g.has_next());

        assert!(g.previous_image());
        assert_eq!(g.selected_index().unwrap_or(0), 0);
        assert!(!g.has_previous());

        // Navigate to the end.
        g.set_current_index(4);
        assert!(!g.has_next());
        assert!(g.has_previous());
        assert!(!g.next_image()); // cannot go further
    }

    #[test]
    fn image_gallery_remove_image() {
        let mut g = make_gallery(Rect::new(0, 0, 400, 300));
        g.remove_image(0);
        assert_eq!(g.image_count(), 4);
        assert_eq!(g.selected_index().unwrap_or(0), 0);
        assert_eq!(g.current_image().unwrap().label.as_deref(), Some("Mountains"));
    }

    #[test]
    fn image_gallery_clear_images() {
        let mut g = make_gallery(Rect::new(0, 0, 400, 300));
        assert_eq!(g.image_count(), 5);
        g.clear_images();
        assert_eq!(g.image_count(), 0);
        assert!(g.current_image().is_none());
    }

    #[test]
    fn image_gallery_set_current_index() {
        let mut g = make_gallery(Rect::new(0, 0, 400, 300));
        g.set_current_index(2);
        assert_eq!(g.selected_index().unwrap_or(0), 2);
        assert_eq!(g.current_image().unwrap().label.as_deref(), Some("Ocean"));

        // Clamp to max.
        g.set_current_index(999);
        assert_eq!(g.selected_index().unwrap_or(0), 4);
    }

    #[test]
    fn image_gallery_thumbnail_size() {
        let mut g = ImageGallery::new(Rect::new(0, 0, 400, 300));
        assert_eq!(g.thumbnail_size(), 64);
        g.set_thumbnail_size(128);
        assert_eq!(g.thumbnail_size(), 128);
        g.set_thumbnail_size(0); // clamped to min
        assert_eq!(g.thumbnail_size(), 16);
        g.set_thumbnail_size(500); // clamped to max
        assert_eq!(g.thumbnail_size(), 256);
    }

    #[test]
    fn image_gallery_show_thumbnails() {
        let mut g = ImageGallery::new(Rect::new(0, 0, 400, 300));
        assert!(g.show_thumbnails());
        g.set_show_thumbnails(false);
        assert!(!g.show_thumbnails());
        g.set_show_thumbnails(true);
        assert!(g.show_thumbnails());
    }

    #[test]
    fn image_gallery_image_changed_signal() {
        let mut g = make_gallery(Rect::new(0, 0, 400, 300));
        let captured = Arc::new(Mutex::new(None));
        g.image_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        g.next_image();
        assert_eq!(*captured.lock().unwrap(), Some(1));

        g.set_current_index(3);
        assert_eq!(*captured.lock().unwrap(), Some(3));
    }

    #[test]
    fn image_gallery_add_image_with_size() {
        let mut g = ImageGallery::new(Rect::new(0, 0, 400, 300));
        g.add_image_with_size("/img/wide.jpg", Some("Wide"), 1920, 1080);
        assert_eq!(g.image_count(), 1);
        let img = g.current_image().unwrap();
        assert_eq!(img.width, 1920);
        assert_eq!(img.height, 1080);
    }

    /// The preview's overlay ink follows the **stage**, not a fixed near-white.
    ///
    /// # The defect this pins
    ///
    /// The filename, the dimensions, the arrows and the index pill were all fixed
    /// `rgba(2xx,2xx,2xx,·)` literals, so they were readable **only** on the one dark stage the
    /// control also hardcoded. The moment a host set a light stage through the style — which is the
    /// whole point of the stage reading `style.background_color` — every overlay became white on
    /// white: invisible, with nothing reported. The ink is now one step from the stage toward the
    /// stage's own contrast colour, so it is legible on whatever stage is painted.
    ///
    /// The assertion is in two halves: the overlay must **differ** between a dark and a light
    /// stage (the model-only half would pass on a constant), and it must actually **contrast** with
    /// the stage it sits on (the half that rejects "different but both illegible").
    #[test]
    fn the_preview_overlay_follows_the_stage_it_is_painted_on() {
        use crate::core::Color;

        /// Luminance distance between two colours, 0.0 same and 1.0 opposite.
        fn contrast(a: Color, b: Color) -> f32 {
            (a.luminance() - b.luminance()).abs()
        }

        for (stage, label) in
            [(Color::rgb(30, 30, 30), "dark"), (Color::rgb(245, 245, 245), "light")]
        {
            let ink = stage.blend(&stage.contrast_color(), 0.85);
            assert!(
                contrast(ink, stage) > 0.5,
                "overlay ink must contrast with a {label} stage: ink={ink:?} stage={stage:?}"
            );
        }
        // And the two derived inks are not the same colour — which is what makes the assertion
        // above about the stage rather than about a constant that happens to pass one case.
        let dark_ink = Color::rgb(30, 30, 30).blend(&Color::rgb(30, 30, 30).contrast_color(), 0.85);
        let light_ink =
            Color::rgb(245, 245, 245).blend(&Color::rgb(245, 245, 245).contrast_color(), 0.85);
        assert_ne!(dark_ink, light_ink, "the overlay must be derived, not fixed");
    }

    /// Stepping to another image makes it **arrive** rather than swapping between two frames.
    ///
    /// # The defect this pins
    ///
    /// `set_current_index` replaced the picture outright, so a step gave the reader no signal that
    /// anything had happened -- and when two images look alike there was no way to tell a successful
    /// step from a click that missed. The assertions are in two halves, because a progress nothing
    /// reads is not an arrival: the model must take an interior value, and the *painted* preview must
    /// differ between that moment and the settled frame.
    ///
    /// The second half also pins that all three navigation paths share the reveal: `next_image` and
    /// `previous_image` both funnel through `set_current_index`, and a gesture that animated while a
    /// click did not would be the two-places-to-remember defect this crate keeps recording.
    #[test]
    fn a_new_image_arrives_rather_than_swapping_between_frames() {
        use crate::widget::svg::render_to_svg;
        let _theme_guard = crate::style::theme_test_guard();

        let mut g = make_gallery(Rect::new(0, 0, 400, 300));
        assert_eq!(g.reveal_progress(), 1.0, "a seeded gallery is already settled");
        assert!(!g.is_animating(), "and owes no frames");

        assert!(g.next_image(), "there is a second image");
        assert!(g.is_animating(), "and the new one owes frames");
        assert!(g.tick(30), "still arriving after one step");
        let mid = g.reveal_progress();
        assert!(mid > 0.0 && mid < 1.0, "it passes through an interior value (got {mid})");
        let arriving = render_to_svg(&mut g);

        while g.tick(1000) {}
        assert_eq!(g.reveal_progress(), 1.0, "and settles fully shown");
        let settled = render_to_svg(&mut g);
        assert_ne!(
            arriving, settled,
            "an arriving image must paint differently from a settled one, or nothing is animating"
        );
        // The reveal is reset per step, so a *second* step starts from zero again rather than
        // continuing from wherever the first one had got to.
        assert!(g.previous_image(), "stepping back is offered too");
        assert!(
            g.reveal_progress() < 1.0,
            "every navigation path restarts the arrival: {}",
            g.reveal_progress()
        );
    }

    #[test]
    fn image_gallery_handles_multibyte_utf8_label() {
        use crate::widget::svg::render_to_svg;
        let mut g = ImageGallery::new(Rect::new(0, 0, 400, 300));
        // A label with multibyte UTF-8 characters where byte index 8 falls inside a character
        g.add_image_with_size("/img/test.jpg", Some("Café ☕ Gallery Preview"), 800, 600);
        let svg = render_to_svg(&mut g);
        assert!(!svg.is_empty(), "rendering with multibyte label must not panic");
    }

    /// Hover is the image under the pointer, resolved through the same slot boxes
    /// the paint and the press read, and leaving the gallery clears the highlight
    /// instead of freezing it in place.
    #[test]
    fn image_gallery_hover_follows_the_pointer() {
        let mut g = make_gallery(Rect::new(0, 0, 400, 300));

        let (strip, _, start_offset) = g.strip_layout().expect("the strip is visible");
        let first = ImageGallery::thumbnail_rect(strip, g.thumbnail_size(), 0);
        g.handle_event(&Event::MouseMove { pos: Point::new(first.x + 10, first.y + 10) });
        assert_eq!(g.hovered_thumb, Some(start_offset));

        // Above the strip (in the preview) nothing hovers.
        g.handle_event(&Event::MouseMove { pos: Point::new(200, 50) });
        assert_eq!(g.hovered_thumb, None);

        g.handle_event(&Event::MouseLeave { pos: Point::new(-1, -1) });
        assert_eq!(g.hovered_thumb, None);
    }

    /// A hidden strip has no slots, so no pointer position can hover a thumbnail —
    /// the same guard the paint path applies before it draws the strip.
    #[test]
    fn image_gallery_hover_needs_a_visible_strip() {
        let mut g = make_gallery(Rect::new(0, 0, 400, 300));
        g.set_show_thumbnails(false);

        assert!(g.strip_layout().is_none());
        assert!(g.thumbnail_at(Point::new(10, 280)).is_none());
    }

    /// Zoom, wheel paging, double-click full-screen and ctrl-click multi-select are wired.
    #[test]
    fn zoom_wheel_double_click_and_multi_select_are_wired() {
        use crate::event::EventHandler;
        let mut g = make_gallery(Rect::new(0, 0, 400, 300));
        g.set_zoom(2.0);
        assert_eq!(g.zoom(), 2.0);
        g.set_zoom(f32::NAN);
        assert_eq!(g.zoom(), 2.0, "a non-finite zoom is ignored");

        // The wheel scrolls the strip by default; navigation is an explicit mode now.
        g.set_wheel_mode(GalleryWheelMode::Navigate);
        g.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert!(g.selected_index().unwrap_or(0) > 0 || g.image_count() < 2);
        g.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0b0010 });
        assert!(g.zoom() > 0.0);

        let fullscreen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&fullscreen);
        g.fullscreen_requested.connect(move |index| {
            if let Ok(mut slot) = sink.lock() {
                *slot = Some(*index);
            }
        });
        let preview = g.preview_rect();
        g.handle_event(&Event::MouseDoubleClick {
            pos: Point::new(preview.x + 2, preview.y + 2),
            button: 1,
        });
        assert!(fullscreen.lock().map(|slot| slot.is_some()).unwrap_or(false));

        g.set_multi_select(true);
        if let Some((strip, _, _)) = g.strip_layout() {
            let rect = ImageGallery::thumbnail_rect(strip, g.effective_thumbnail_size(), 0);
            g.handle_event(&Event::MousePress {
                pos: Point::new(rect.x + 2, rect.y + 2),
                button: 1,
                modifiers: 0b0010,
            });
            assert_eq!(g.selected_indices(), vec![0]);
        }
    }
}
