//! ImageGallery — an image gallery/browser widget with thumbnails and large view.
//!
//! The ImageGallery widget displays a large preview of the current image with
//! a thumbnail strip at the bottom. It supports keyboard navigation (arrow keys),
//! thumbnail selection via click, and emits a signal when the selected image changes.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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
            self.image_changed.emit(self.current_index);
            self.base.request_redraw();
        }
    }

    /// Returns the current image index.
    pub fn current_index(&self) -> usize {
        self.current_index
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
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 300)
    }
}

impl Draw for ImageGallery {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        if self.images.is_empty() {
            // Empty state.
            let bg = if !is_enabled {
                Color::rgba(230, 230, 230, 200)
            } else {
                Color::rgba(240, 240, 240, 255)
            };
            context.fill_rect(rect, bg);
            let font = Font::default();
            let text = "No images in gallery";
            let metrics = context.measure_text(text, &font);
            let text_x = rect.x + (rect.width as i32 - metrics.width as i32) / 2;
            let text_y = rect.y + rect.height as i32 / 2 + metrics.ascent as i32 / 2;
            context.draw_text(
                Point::new(text_x, text_y),
                text,
                &font,
                Color::rgba(160, 160, 160, 220),
                HorizontalAlignment::Left,
            );
            return;
        }

        // ── Large preview area ──────────────────────────────────────────
        let thumb_strip_height = if self.show_thumbnails { self.thumbnail_size + 28 } else { 0 };
        let preview_rect =
            Rect::new(rect.x, rect.y, rect.width, rect.height.saturating_sub(thumb_strip_height));

        let bg =
            if !is_enabled { Color::rgba(30, 30, 30, 200) } else { Color::rgba(30, 30, 30, 255) };
        context.fill_rect(preview_rect, bg);

        // Draw current image info in preview area.
        if let Some(image) = self.images.get(self.current_index) {
            let font = Font::default();

            // Draw image label / filename centered.
            let display_name = image
                .label
                .as_deref()
                .unwrap_or_else(|| image.path.rsplit('/').next().unwrap_or(&image.path));
            let name_metrics = context.measure_text(display_name, &font);
            let name_x =
                preview_rect.x + (preview_rect.width as i32 - name_metrics.width as i32) / 2;
            let name_y =
                preview_rect.y + preview_rect.height as i32 / 3 + name_metrics.ascent as i32 / 2;
            context.draw_text(
                Point::new(name_x, name_y),
                display_name,
                &font,
                Color::rgba(220, 220, 220, 230),
                HorizontalAlignment::Left,
            );

            // Draw dimensions.
            let dim_text = format!("{}x{}", image.width, image.height);
            let dim_metrics = context.measure_text(&dim_text, &font);
            let dim_x = preview_rect.x + (preview_rect.width as i32 - dim_metrics.width as i32) / 2;
            let dim_y =
                preview_rect.y + preview_rect.height as i32 * 2 / 3 + dim_metrics.ascent as i32 / 2;
            context.draw_text(
                Point::new(dim_x, dim_y),
                &dim_text,
                &font,
                Color::rgba(180, 180, 180, 200),
                HorizontalAlignment::Left,
            );

            // Image index indicator.
            let index_text = format!("{}/{}", self.current_index + 1, self.images.len());
            let index_metrics = context.measure_text(&index_text, &font);
            let index_x =
                preview_rect.x + preview_rect.width as i32 - index_metrics.width as i32 - 8;
            let index_y = preview_rect.y + 4 + index_metrics.ascent as i32;
            let pill_w = index_metrics.width as u32 + 8;
            let pill_h = index_metrics.height as u32 + 4;
            let pill_rect = Rect::new(index_x - 4, preview_rect.y + 2, pill_w, pill_h);
            context.fill_rounded_rect(pill_rect, 3, Color::rgba(0, 0, 0, 70));
            context.draw_text(Point::new(index_x, index_y), &index_text, &font, Color::WHITE, HorizontalAlignment::Left);

            // Navigation arrows.
            if self.has_previous() {
                let arrow_left = "◀";
                let arrow_metrics = context.measure_text(arrow_left, &font);
                let arrow_x = preview_rect.x + 8;
                let arrow_y = preview_rect.y
                    + preview_rect.height as i32 / 2
                    + arrow_metrics.ascent as i32 / 2;
                context.draw_text(
                    Point::new(arrow_x, arrow_y),
                    arrow_left,
                    &font,
                    Color::rgba(255, 255, 255, 180),
                    HorizontalAlignment::Left,
                );
            }

            if self.has_next() {
                let arrow_right = "▶";
                let arrow_metrics = context.measure_text(arrow_right, &font);
                let arrow_x =
                    preview_rect.x + preview_rect.width as i32 - arrow_metrics.width as i32 - 8;
                let arrow_y = preview_rect.y
                    + preview_rect.height as i32 / 2
                    + arrow_metrics.ascent as i32 / 2;
                context.draw_text(
                    Point::new(arrow_x, arrow_y),
                    arrow_right,
                    &font,
                    Color::rgba(255, 255, 255, 180),
                    HorizontalAlignment::Left,
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
        context.fill_rect(strip_rect, Color::rgba(50, 50, 50, 255));

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
            let thumb_bg = if is_selected {
                Color::rgba(80, 140, 220, 200)
            } else {
                Color::rgba(80, 80, 80, 200)
            };
            context.fill_rounded_rect(thumb_rect, 3, thumb_bg);

            // Draw a placeholder pattern inside the thumbnail.
            let inner_rect = Rect::new(
                thumb_rect.x + 2,
                thumb_rect.y + 2,
                thumb_rect.width - 4,
                thumb_rect.height - 4,
            );
            context.fill_rounded_rect(inner_rect, 2, Color::rgba(60, 60, 60, 200));

            // Draw image label under thumbnail.
            if let Some(image) = self.images.get(img_idx) {
                if let Some(ref label) = image.label {
                    let font = Font::default();
                    let label_text =
                        if label.len() > 10 { format!("{}..", &label[..8]) } else { label.clone() };
                    let label_metrics = context.measure_text(&label_text, &font);
                    let label_x =
                        thumb_x + (self.thumbnail_size as i32 - label_metrics.width as i32) / 2;
                    let label_y =
                        thumb_y + self.thumbnail_size as i32 + 2 + label_metrics.ascent as i32;
                    context.draw_text(
                        Point::new(label_x, label_y),
                        &label_text,
                        &font,
                        Color::rgba(200, 200, 200, 200),
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
            Event::MousePress { pos, button } => {
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

                    // Check if clicked on navigation arrows in preview area.
                    if pos.y >= preview_rect.y && pos.y < preview_rect.y + preview_height as i32 {
                        let font = Font::default();

                        if self.has_previous() {
                            let arrow_left = "◀";
                            let arrow_metrics =
                                context::private::measure_text_static(&font, arrow_left);
                            let arrow_x = preview_rect.x + 8;
                            let arrow_y = preview_rect.y
                                + preview_height as i32 / 2
                                + arrow_metrics.ascent as i32 / 2;
                            let arrow_w = arrow_metrics.width as i32 + 8;
                            let arrow_h = arrow_metrics.height as i32 + 8;
                            let arrow_rect = Rect::new(
                                arrow_x - 4,
                                arrow_y - arrow_metrics.ascent as i32 - 4,
                                arrow_w as u32,
                                arrow_h as u32,
                            );
                            if arrow_rect.contains_point(*pos) {
                                self.previous_image();
                                return;
                            }
                        }

                        if self.has_next() {
                            let arrow_right = "▶";
                            let arrow_metrics =
                                context::private::measure_text_static(&font, arrow_right);
                            let arrow_x = preview_rect.x + preview_rect.width as i32
                                - arrow_metrics.width as i32
                                - 8;
                            let arrow_y = preview_rect.y
                                + preview_height as i32 / 2
                                + arrow_metrics.ascent as i32 / 2;
                            let arrow_w = arrow_metrics.width as i32 + 8;
                            let arrow_h = arrow_metrics.height as i32 + 8;
                            let arrow_rect = Rect::new(
                                arrow_x - 4,
                                arrow_y - arrow_metrics.ascent as i32 - 4,
                                arrow_w as u32,
                                arrow_h as u32,
                            );
                            if arrow_rect.contains_point(*pos) {
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

/// Internal helper module for static text metrics (avoiding RenderContext dependency in event handler).
mod context {
    pub mod private {
        use crate::core::Font;
        use crate::render::TextMetrics;
        pub fn measure_text_static(_font: &Font, text: &str) -> TextMetrics {
            TextMetrics { width: (text.len() as u32) * 8, height: 16, ascent: 12, descent: 4 }
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
}
