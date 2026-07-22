//! ImageView widget — displays an Image as a widget (BLUE13 R2.12).
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::image::{Image, ImageFormat};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Widget that displays an Image.
pub struct ImageView {
    base: BaseWidget,
    image: Image,
    scaled: bool,
    background: Option<Color>,
}

impl ImageView {
    /// Creates a new ImageView with the given image and geometry.
    pub fn new(image: Image, rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ImageView, rect, "ImageView"),
            image,
            scaled: false,
            background: None,
        }
    }

    /// Sets a new image to display.
    pub fn set_image(&mut self, image: Image) {
        self.image = image;
        self.base.request_redraw();
    }

    /// Returns a reference to the current image.
    pub fn image(&self) -> &Image {
        &self.image
    }

    /// Sets whether the image should be scaled to fill the widget rect.
    pub fn set_scaled(&mut self, scaled: bool) {
        self.scaled = scaled;
    }

    /// Returns whether the image is scaled to fill the widget rect.
    pub fn is_scaled(&self) -> bool {
        self.scaled
    }
}

impl Widget for ImageView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        if self.image.width > 0 && self.image.height > 0 {
            Size::new(self.image.width, self.image.height)
        } else {
            Size::new(100, 100)
        }
    }
}

impl EventHandler for ImageView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

impl Draw for ImageView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        if self.image.format == ImageFormat::Rgba8
            && self.image.width > 0
            && self.image.height > 0
            && !self.image.data.is_empty()
        {
            let (draw_w, draw_h) = if self.scaled {
                (rect.width, rect.height)
            } else {
                (self.image.width.min(rect.width), self.image.height.min(rect.height))
            };
            context.draw_image(rect.x, rect.y, draw_w, draw_h, &self.image.data);
        } else {
            // Draw placeholder rectangle with "?" text.
            let bg = self
                .style()
                .background_color
                .or(self.background)
                .unwrap_or(Color::rgb(240, 240, 240));
            context.fill_rect(rect, bg);

            // Draw a border to make the placeholder visible.
            let border = self.style().border_color.unwrap_or(Color::rgb(200, 200, 200));
            context.draw_rect_stroke(rect, border, 1);

            // Draw "?" centered in the placeholder.
            let fg = self.style().text_color.unwrap_or(Color::rgb(120, 120, 120));
            let default_font = crate::core::Font::default();
            let font = self.style().font.as_ref().unwrap_or(&default_font);
            let text_x = rect.x + rect.width as i32 / 2 - 4;
            let text_y = rect.y + rect.height as i32 / 2 - 8;
            context.draw_text(
                Point::new(text_x, text_y),
                "?",
                font,
                fg,
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

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

        let rgba = Image::from_rgba(vec![255; 16 * 4], 4, 4);
        view.set_image(rgba.clone());
        assert_eq!(view.image().width, 4);
        assert_eq!(view.image().height, 4);
        assert_eq!(view.image().format, ImageFormat::Rgba8);
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
        let rgba = Image::from_rgba(vec![255; 64 * 64 * 4], 64, 64);
        let view = ImageView::new(rgba, Rect::new(0, 0, 200, 200));
        assert_eq!(view.size_hint(), Size::new(64, 64));
    }
}
