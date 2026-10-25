// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ImageView widget — displays an Image as a widget (BLUE13 R2.12).
use crate::compat::String;
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::image::{Image, ImageFormat};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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
    let mut out = crate::compat::vec![0u8; dest_width as usize * dest_height as usize * 4];
    if source_width == 0
        || source_height == 0
        || dest_width == 0
        || dest_height == 0
        || data.len() < source_width as usize * source_height as usize * 4
    {
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
}

impl ImageAspect {
    /// Returns the token this aspect is published under.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stretch => "stretch",
            Self::Fit => "fit",
            Self::Fill => "fill",
        }
    }

    /// Parses an [`ImageAspect`] from its published token, or `None` when the
    /// token names no aspect.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "stretch" => Some(Self::Stretch),
            "fit" => Some(Self::Fit),
            "fill" => Some(Self::Fill),
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
    scaled_image: Option<(usize, u32, u32, crate::compat::Vec<u8>)>,
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
        self.base.request_redraw();
        self.image_changed.emit();
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
                self.load_failed.emit(error.clone());
                Err(error)
            }
        }
    }

    /// Returns a reference to the current image.
    pub fn image(&self) -> &Image {
        &self.image
    }

    /// The image bytes to hand the painter for a `dest_width` × `dest_height` rectangle.
    ///
    /// # Why this exists
    ///
    /// The draw API receives the destination rectangle and the pixel bytes but not the source
    /// grid, so a backend can only copy: a downscale became the image's top-left crop and an
    /// upscale drew nothing (`data.len() < width * height * 4`). This control is the one party
    /// that knows both sizes, so it hands over a buffer that already matches the rectangle.
    ///
    /// The resample is cached against the source dimensions **and** the destination size, so a
    /// still control pays once and a moving one only when its box changes. A matching size is
    /// borrowed from the image itself, so an unscaled paint allocates nothing.
    fn pixels_for(&mut self, dest_width: u32, dest_height: u32) -> crate::compat::Cow<'_, [u8]> {
        let source_width = self.image.width();
        let source_height = self.image.height();
        let data = self.image.rgba8_data().unwrap_or(&[]);
        if dest_width == source_width && dest_height == source_height {
            return crate::compat::Cow::Borrowed(data);
        }
        let cached = self
            .scaled_image
            .as_ref()
            .is_some_and(|(_, width, height, _)| *width == dest_width && *height == dest_height);
        if !cached {
            let resampled =
                resample_bilinear(data, source_width, source_height, dest_width, dest_height);
            self.scaled_image = Some((data.len(), dest_width, dest_height, resampled));
        }
        match &self.scaled_image {
            Some((_, _, _, pixels)) => crate::compat::Cow::Borrowed(pixels.as_slice()),
            None => crate::compat::Cow::Borrowed(&[]),
        }
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
        let image_width = self.image.width();
        let image_height = self.image.height();
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

        Some((content, Rect::new(dest_x, dest_y, dest_width, dest_height)))
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
        if self.image.width() > 0 && self.image.height() > 0 {
            Size::new(self.image.width(), self.image.height())
        } else {
            Size::new(100, 100)
        }
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
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "scaled" => {
                self.set_scaled(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["scaled", BASE_PROPERTY_NAMES]
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
        self.base.handle_event(event);
        // A double click is the gesture a host uses to open a viewer or replace the picture. It
        // is reported only when it lands on the control and the control is enabled.
        if let Event::MouseDoubleClick { pos, button: 1 } = event {
            if self.base.is_enabled() && self.geometry().contains_point(*pos) {
                self.double_clicked.emit();
            }
        }
    }
}

impl Draw for ImageView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        if self.image.format() == ImageFormat::Rgba8
            && self.image.width() > 0
            && self.image.height() > 0
            && self.image.rgba8_data().is_some_and(|data| !data.is_empty())
        {
            // No room left for the image after its margin: nothing to paint.
            let Some((content, dest)) = self.image_layout() else {
                return;
            };
            // A covering image is larger than its box, so the painted copy has
            // to be bounded to it or it spills onto whatever is behind the
            // control. `Stretch` and `Fit` never exceed the box and need no
            // clip, which keeps their command stream identical to the old one.
            let covering = self.aspect == ImageAspect::Fill;
            if covering {
                context.push_clip(content.x, content.y, content.width, content.height);
            }
            // Paint a buffer that is already the destination size. The draw API takes the bytes
            // and the destination rectangle but no source grid, so handing it a differently sized
            // image is a crop (downscale) or nothing at all (upscale) on the raster backend; the
            // control is the only party that knows both sizes.
            let pixels = self.pixels_for(dest.width, dest.height);
            context.draw_image(dest.x, dest.y, dest.width, dest.height, pixels.as_ref());
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
            context.face_with_gradient(
                rect,
                bg,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().border_radius.unwrap_or(0),
                Color::BLACK,
            );

            // Draw a border to make the placeholder visible.
            let border = self.style().border_color.unwrap_or(Color::rgb(200, 200, 200));
            context.draw_rect_stroke(rect, border, 1);

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
}
