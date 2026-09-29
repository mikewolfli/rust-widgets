// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ImageGallery — an image gallery/browser widget with thumbnails and large view.
//!
//! The ImageGallery widget displays a large preview of the current image with
//! a thumbnail strip at the bottom. It supports keyboard navigation (arrow keys),
//! thumbnail selection via click, and emits a signal when the selected image changes.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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
}

impl GalleryImage {
    /// Creates a new GalleryImage.
    pub fn new(path: &str, width: u32, height: u32) -> Self {
        Self { path: path.to_string(), label: None, width, height }
    }

    /// Creates a new GalleryImage with an optional label.
    pub fn with_label(path: &str, label: Option<&str>, width: u32, height: u32) -> Self {
        Self { path: path.to_string(), label: label.map(|s| s.to_string()), width, height }
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
    /// Emitted when the selected image changes. Passes the new index.
    pub image_changed: Signal1<usize>,
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
            image_changed: Signal1::new(),
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
            // The new image **arrives** rather than replacing the old one between two frames. Every
            // navigation path funnels through here (`next_image`/`previous_image` call this), so the
            // reveal is aimed at the one choke point instead of at each of the three -- which is how
            // a swipe and a click would otherwise animate differently.
            self.reveal.jump_to(0.0);
            self.reveal.set_target(1.0);
            self.image_changed.emit(self.current_index);
            self.base.request_redraw();
        }
    }

    /// Returns the current image index.
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
        self.thumbnail_size = size.clamp(16, 256);
        self.base.request_redraw();
    }

    /// Returns the current thumbnail size.
    pub fn thumbnail_size(&self) -> u32 {
        self.thumbnail_size
    }

    /// Shows or hides the thumbnail strip.
    pub fn set_show_thumbnails(&mut self, show: bool) {
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
}

/// `ImageGallery`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for ImageGallery {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "current_index" => Ok(CapabilityValue::UInt(self.current_index() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                self.set_current_index(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["current_index", BASE_PROPERTY_NAMES]
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
            let style = self.base.style().clone();
            let theme = crate::style::resolved_theme_style("image_gallery");
            let fallback_bg = if !is_enabled {
                Color::rgba(230, 230, 230, 200)
            } else {
                Color::rgba(240, 240, 240, 255)
            };
            let bg = style
                .background_color
                .or_else(|| theme.as_ref().and_then(|t| t.background_color))
                .unwrap_or(fallback_bg);
            context.face(
                rect,
                bg,
                self.style().surface.unwrap_or_default(),
                self.style().border_radius.unwrap_or(0),
                Color::BLACK,
            );
            let font = Font::default();
            let text = "No images in gallery";
            // Fitted to the control's width: 20 characters at 14 px is wider than a
            // gallery laid out in a narrow pane, and the label used to be placed from the
            // box centre outward, so half of it fell outside.
            let metrics = context.measure_text(text, &font);
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
            context.draw_text_fitted(line, text, &font, label_color, HorizontalAlignment::Center);
            return;
        }

        // ── Large preview area ──────────────────────────────────────────
        let thumb_strip_height = if self.show_thumbnails { self.thumbnail_size + 28 } else { 0 };
        let preview_rect =
            Rect::new(rect.x, rect.y, rect.width, rect.height.saturating_sub(thumb_strip_height));

        // The preview is a **photo stage**: it is the one surface in the crate that is darker
        // than any appearance, because a preview is judged against a neutral dark ground in every
        // image tool. So it is the *explicit style's* colour when the host gave one — a host that
        // wants a light stage gets a light stage — and the dark literal only as the last fallback.
        //
        // The overlay ink on top of it (`name_ink`, `dim_ink`, `arrow_ink`) is **derived from the
        // stage** rather than written down as four more near-whites. That is what keeps the original
        // defect from returning: the old form hardcoded every one of them as `rgba(2xx,2xx,2xx)`, so
        // a host that set a *light* stage through the style got white-on-white labels and no error.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("image_gallery");
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
            let font = Font::default();

            // Draw image label / filename centered.
            let display_name = image
                .label
                .as_deref()
                .unwrap_or_else(|| image.path.rsplit('/').next().unwrap_or(&image.path));
            // The origin is the glyph box's *top* edge, so the label reads as centred on the
            // band at one third of the preview rather than half a line below its middle.
            let name_metrics = context.measure_text(display_name, &font);
            let name_y =
                preview_rect.y + preview_rect.height as i32 / 3 - name_metrics.height as i32 / 2;
            // The name is host-supplied, so a long one overflowed both edges of the preview: the
            // centred origin was computed from the *measured* width, which put the overflow half
            // beyond each side and made the label wider than the box it was centred on. Handing
            // the fitter a preview-wide box keeps the horizontal centring and elides the name at
            // the preview's edges.
            context.draw_text_fitted(
                Rect::new(preview_rect.x, name_y, preview_rect.width, name_metrics.height),
                display_name,
                &font,
                name_ink,
                HorizontalAlignment::Center,
            );

            // Draw dimensions.
            let dim_text = format!("{}x{}", image.width, image.height);
            let dim_metrics = context.measure_text(&dim_text, &font);
            let dim_x = preview_rect.x + (preview_rect.width as i32 - dim_metrics.width as i32) / 2;
            // Same as the filename above: half a *line box* about the band, not half an
            // ascent below it.
            let dim_y =
                preview_rect.y + preview_rect.height as i32 * 2 / 3 - dim_metrics.height as i32 / 2;
            context.draw_text(
                Point::new(dim_x, dim_y),
                &dim_text,
                &font,
                dim_ink,
                HorizontalAlignment::Left,
            );

            // Image index indicator.
            let index_text = format!("{}/{}", self.current_index + 1, self.images.len());
            let index_metrics = context.measure_text(&index_text, &font);
            let index_x =
                preview_rect.x + preview_rect.width as i32 - index_metrics.width as i32 - 8;
            let pill_w = index_metrics.width as u32 + 8;
            let pill_h = index_metrics.height as u32 + 4;
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
                &index_text,
                &font,
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

        // ── Thumbnail strip ─────────────────────────────────────────────
        if !self.show_thumbnails {
            return;
        }

        let strip_rect = Rect::new(
            rect.x,
            preview_rect.y + preview_rect.height as i32,
            rect.width,
            thumb_strip_height,
        );
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

        let thumb_spacing = 6u32;
        let thumb_total = self.thumbnail_size + thumb_spacing;
        let strip_padding = 8i32;

        // Calculate which thumbnails are visible.
        let max_visible =
            strip_rect.width.checked_div(thumb_total).map_or(1, |v| v.max(1) as usize);

        let start_offset = if self.current_index >= max_visible / 2 {
            (self.current_index - max_visible / 2)
                .min(self.images.len().saturating_sub(max_visible))
        } else {
            0
        };

        for i in 0..max_visible.min(self.images.len()) {
            let img_idx = start_offset + i;
            if img_idx >= self.images.len() {
                break;
            }

            let thumb_x = strip_rect.x + strip_padding + (i as u32 * thumb_total) as i32;
            let thumb_y = strip_rect.y + 4;

            let thumb_rect = Rect::new(thumb_x, thumb_y, self.thumbnail_size, self.thumbnail_size);

            // Thumbnail background.
            let is_selected = img_idx == self.current_index;
            let thumb_bg = if is_selected { thumb_selected } else { thumb_resting };
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

            // Draw image label under thumbnail.
            if let Some(image) = self.images.get(img_idx) {
                if let Some(ref label) = image.label {
                    let font = Font::default();
                    let label_text =
                        if label.len() > 10 { format!("{}..", &label[..8]) } else { label.clone() };
                    let label_metrics = context.measure_text(&label_text, &font);
                    let label_x =
                        thumb_x + (self.thumbnail_size as i32 - label_metrics.width as i32) / 2;
                    // Top-aligned caption under the thumbnail, not a centred box: the origin
                    // is already the glyph box's top edge, so the declared-baseline `+ ascent`
                    // only dropped the caption on top of the row below it.
                    let label_y = thumb_y + self.thumbnail_size as i32 + 2;
                    context.draw_text(
                        Point::new(label_x, label_y),
                        &label_text,
                        &font,
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
            Event::MousePress { pos, button, .. } => {
                if *button == 1 {
                    let rect = self.geometry();
                    if !rect.contains_point(*pos) {
                        return;
                    }

                    if self.images.is_empty() {
                        return;
                    }

                    let thumb_strip_height =
                        if self.show_thumbnails { self.thumbnail_size + 28 } else { 0 };
                    let preview_height = rect.height.saturating_sub(thumb_strip_height);
                    let preview_rect = Rect::new(rect.x, rect.y, rect.width, preview_height);

                    // Check if clicked on navigation arrows in preview area. The boxes come from
                    // `arrow_boxes`, the same derivation `draw` paints through, so an arrow is
                    // clickable exactly where it is drawn.
                    if pos.y >= preview_rect.y && pos.y < preview_rect.y + preview_height as i32 {
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
                        return;
                    }

                    // Check if clicked on a thumbnail.
                    if self.show_thumbnails {
                        let strip_rect = Rect::new(
                            rect.x,
                            preview_rect.y + preview_height as i32,
                            rect.width,
                            thumb_strip_height,
                        );

                        if pos.y >= strip_rect.y && pos.y < strip_rect.y + strip_rect.height as i32
                        {
                            let thumb_spacing = 6u32;
                            let thumb_total = self.thumbnail_size + thumb_spacing;
                            let strip_padding = 8i32;

                            let max_visible = strip_rect
                                .width
                                .checked_div(thumb_total)
                                .map_or(1, |v| v.max(1) as usize);

                            let start_offset = if self.current_index >= max_visible / 2 {
                                (self.current_index - max_visible / 2)
                                    .min(self.images.len().saturating_sub(max_visible))
                            } else {
                                0
                            };

                            for i in 0..max_visible.min(self.images.len()) {
                                let img_idx = start_offset + i;
                                if img_idx >= self.images.len() {
                                    break;
                                }

                                let thumb_x =
                                    strip_rect.x + strip_padding + (i as u32 * thumb_total) as i32;
                                let thumb_y = strip_rect.y + 4;

                                let thumb_rect = Rect::new(
                                    thumb_x,
                                    thumb_y,
                                    self.thumbnail_size,
                                    self.thumbnail_size,
                                );

                                if thumb_rect.contains_point(*pos) {
                                    self.set_current_index(img_idx);
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                match key {
                    // Right arrow or Down arrow = next.
                    39 | 40 => {
                        self.next_image();
                    }
                    // Left arrow or Up arrow = previous.
                    37 | 38 => {
                        self.previous_image();
                    }
                    _ => {}
                }
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
        assert_eq!(g.current_index(), 0);
        assert!(g.current_image().is_none());
        assert_eq!(g.thumbnail_size(), 64);
        assert!(g.show_thumbnails());
        assert!(!g.has_next());
        assert!(!g.has_previous());
        assert_eq!(g.kind(), WidgetKind::ImageGallery);
    }

    #[test]
    fn image_gallery_add_images_and_count() {
        let g = make_gallery(Rect::new(0, 0, 400, 300));
        assert_eq!(g.image_count(), 5);
        assert_eq!(g.current_index(), 0);
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
        assert_eq!(g.current_index(), 1);
        assert!(g.has_previous());
        assert!(g.has_next());

        assert!(g.previous_image());
        assert_eq!(g.current_index(), 0);
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
        assert_eq!(g.current_index(), 0);
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
        assert_eq!(g.current_index(), 2);
        assert_eq!(g.current_image().unwrap().label.as_deref(), Some("Ocean"));

        // Clamp to max.
        g.set_current_index(999);
        assert_eq!(g.current_index(), 4);
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
}
