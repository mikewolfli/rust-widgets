// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Avatar widget — a circular or square user avatar/image placeholder with initials fallback.
//!
//! The Avatar widget displays a colored circle (or rounded square) with centered
//! initials text, commonly used for user profile pictures, contact avatars, and
//! identity placeholders in modern UI design.

use crate::compat::Vec;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Avatar widget for displaying user profile images or initials-based placeholders.
///
/// By default the avatar renders as a filled circle with the background color and
/// white bold initials centered inside. Setting `square` to `true` produces a
/// rounded rectangle instead.
pub struct Avatar {
    base: BaseWidget,
    /// Initials text displayed inside the avatar (typically 1-2 characters).
    text: String,
    /// Optional source path or URL of the avatar image.
    ///
    /// Read by [`Avatar::paint_image_source`], which draws the decoded picture over the disc and
    /// falls back to the initials when it cannot be loaded. An empty source is the initials-only
    /// avatar the widget has always had.
    image_source: String,
    /// RGBA8 pixels for [`Self::image_source`], decoded once per source.
    ///
    /// `None` is cached too, so a source that failed to load is not re-read every frame.
    decoded_source: Option<alloc::sync::Arc<Vec<u8>>>,
    /// The source string [`Self::decoded_source`] was produced for, so a change to the source
    /// invalidates the pixels without the caller having to clear them.
    decoded_source_key: Option<String>,
    /// Background fill color of the avatar.
    bg_color: Color,
    /// Whether the fill was chosen by the caller through [`Avatar::set_bg_color`].
    ///
    /// The caller's choice must win over the active theme, but the colour the
    /// constructor seeds must not: without this flag a theme switch could never
    /// reach an avatar that was never explicitly coloured, which is exactly the
    /// hardcoded-chrome defect the rendering census reports.
    bg_color_is_explicit: bool,
    /// When `true`, renders as a rounded square instead of a circle.
    square: bool,
    /// Diameter (circle) or side length (square) in logical pixels.
    size: u32,
}

impl Avatar {
    /// Creates a new Avatar widget with the given geometry.
    ///
    /// Defaults to a 40×40 circle with [`Color::PRIMARY`] background and empty initials.
    /// If `geometry` has zero width or height, a default 40×40 size is used.
    pub fn new(geometry: Rect) -> Self {
        let sz = if geometry.width > 0 && geometry.height > 0 {
            geometry.width.min(geometry.height)
        } else {
            40
        };
        Self {
            base: BaseWidget::new(
                WidgetKind::Avatar,
                Rect::new(geometry.x, geometry.y, sz, sz),
                "Avatar",
            ),
            text: String::new(),
            image_source: String::new(),
            decoded_source: None,
            decoded_source_key: None,
            bg_color: Color::PRIMARY,
            bg_color_is_explicit: false,
            square: false,
            size: sz,
        }
    }

    /// Sets the initials text displayed inside the avatar.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current initials text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the configured avatar image source, or an empty string when the
    /// avatar falls back to its initials.
    pub fn image_source(&self) -> &str {
        &self.image_source
    }

    /// Sets the source path or URL of the avatar image.
    ///
    /// An empty string clears the source and restores the initials fallback. Both the pixels and
    /// the key they were produced for are dropped together: leaving the key would make the next
    /// paint believe the cache belonged to the new source.
    pub fn set_image_source(&mut self, source: &str) {
        if self.image_source == source {
            return;
        }
        self.image_source = source.to_string();
        self.decoded_source = None;
        self.decoded_source_key = None;
        self.base.request_redraw();
    }

    /// Sets whether the avatar renders as a rounded square (instead of a circle).
    pub fn set_square(&mut self, square: bool) {
        self.square = square;
        self.base.request_redraw();
    }

    /// Returns `true` if the avatar renders as a rounded square.
    pub fn is_square(&self) -> bool {
        self.square
    }

    /// Sets the background fill color of the avatar.
    ///
    /// An explicit colour wins over the active theme, so this overrides the
    /// theme-resolved fill until it is set again.
    pub fn set_bg_color(&mut self, color: Color) {
        self.bg_color = color;
        self.bg_color_is_explicit = true;
        self.base.request_redraw();
    }

    /// Returns the current background fill color.
    pub fn bg_color(&self) -> Color {
        self.bg_color
    }

    /// The colour the constructor seeds when the caller sets none.
    ///
    /// Named rather than inlined so [`Draw`] can tell "the caller chose this colour"
    /// (keep it, even across a theme switch) from "nothing chose it yet" (let the
    /// theme decide).
    fn default_bg() -> Color {
        Color::PRIMARY
    }

    /// Sets the diameter (circle) or side length (square) of the avatar.
    /// Updates the widget geometry to match.
    pub fn set_size(&mut self, size: u32) {
        self.size = size;
        let rect = self.geometry();
        self.set_geometry(Rect::new(rect.x, rect.y, size, size));
        self.base.request_redraw();
    }

    /// Returns the current diameter or side length of the avatar.
    pub fn size(&self) -> u32 {
        self.size
    }
}

impl Widget for Avatar {
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
        crate::core::Size::new(dimensions::AVATAR_SIZE, dimensions::AVATAR_SIZE)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Avatar`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `initials` reports the
/// widget's own placeholder text and `image_source` its configured image.
impl WidgetProperties for Avatar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "initials" => Ok(CapabilityValue::String(self.text().to_string())),
            "image_source" => Ok(CapabilityValue::String(self.image_source().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "initials" => {
                self.set_text(&expect_string(value)?);
                Ok(())
            }
            "image_source" => {
                self.set_image_source(&expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["initials", "image_source", BASE_PROPERTY_NAMES]
    }
}

impl Draw for Avatar {
    fn draw(&mut self, context: &mut RenderContext) {
        // ── The disc actually painted ──
        //
        // An avatar is a **fixed-size square** affordance centred in the area it was given.
        // Deriving the disc from `rect`'s top-left corner drew it half-clipped at the left
        // edge of any rectangle wider than the disc: the census hands this control a 240x120
        // cell, so the old code drew `circle cx=60 cy=60 r=60`, running from x 0 to x 120 with
        // the whole left half of the cell empty — a half-visible circle rather than a centred
        // avatar. The height is `size_hint`'s, so the drawn shape and the reported size cannot
        // disagree; `center_in` clamps *down* rather than up, so a control laid out smaller
        // than its hint still paints inside the rectangle it was given.
        let rect = ControlMetrics::centered_square(self.geometry(), self.size_hint().width);
        let size = rect.width.min(rect.height);
        let center = Point::new(rect.x + (size as i32) / 2, rect.y + (size as i32) / 2);

        // The disc is chrome: it resolves explicit style first, then the theme's resolved
        // style for this control, and only then falls back to the constructor's brand
        // colour. Without the theme step a light/dark switch would change nothing on
        // screen, because the fill was previously hardcoded.
        //
        // The theme reads are separate manager locks, each taken and released inside
        // `resolved_theme_style`, so none is held across the draw or across another
        // accessor — the global manager's mutex is not re-entrant.
        //
        // The precedence itself lives in `disc_fill`, because the corner squares that mask a
        // square picture over a circular avatar have to be painted in exactly this colour -- two
        // copies would let a mask square differ from the disc it continues.
        let disc_color = self.disc_fill();

        // Draw the avatar shape (circle or rounded square)
        if self.square {
            let corner_radius = size / 4;
            context.fill_rounded_rect(rect, corner_radius, disc_color);
        } else {
            context.fill_circle(center, size / 2, disc_color);
        }

        // ── The picture, when the caller supplied one ──
        //
        // `image_source` was carried as state and published as a property, and the widget's own
        // field comment recorded the gap honestly: "the widget has no image pipeline yet, so this is
        // carried as state ... the initials remain the rendered fallback until a loader is wired
        // in." The pipeline exists (`crate::image::decoder`), so the loader is wired in here.
        //
        // The picture is drawn over the disc rather than instead of it, so the disc is what shows
        // through a picture with transparency -- and so a source that fails to load still leaves a
        // shaped, coloured avatar rather than a hole. The initials are skipped when a picture is
        // actually painted, because two identities on one disc read as neither.
        let painted = self.paint_image_source(context, rect, center, size);

        // Draw centered initials text
        if !painted && !self.text.is_empty() {
            // Use a bold font sized relative to the avatar size
            let font_size = (size as f32 * 0.45).max(8.0);
            let font = Font::bold("Arial", font_size);

            let metrics = context.measure_text(&self.text, &font);
            let text_width = metrics.width as i32;

            // The origin is the glyph box's *top* edge, so centring is half the line box:
            // the old `(size - ascent - descent)/2 + ascent` started the box half a line below
            // the disc's middle and clipped the initials against the bottom edge.
            let text_x = rect.x + (rect.width as i32 - text_width) / 2;
            let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;

            context.draw_text(
                Point::new(text_x.max(rect.x), text_y.max(rect.y)),
                &self.text,
                &font,
                disc_color.contrast_color(),
                HorizontalAlignment::Left,
            );
        }
    }
}

impl Avatar {
    /// Draws the avatar's configured picture over its disc, and reports whether one was painted.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `image_source` was stored, published (getter, setter, schema row, round-trip test) and read
    /// by nothing: `draw` never looked at it, so setting a source changed no pixel. The field's own
    /// comment said so -- "the widget has no image pipeline yet" -- which made it read as a known
    /// gap rather than as a broken promise.
    ///
    /// # Why the load is cached in the field's own cell
    ///
    /// Decoding is expensive and `draw` runs once a frame, so reading the file on every paint would
    /// spend the whole budget on I/O for a picture that has not changed. The cache is keyed on the
    /// *source string*, so a caller who sets a new path gets a new picture and one who re-sets the
    /// same path does not re-read it. A failure is cached as `None` for the same reason: a missing
    /// file must not be stat-ed sixty times a second.
    ///
    /// # Why a failure draws the initials rather than an error
    ///
    /// An avatar is a placeholder affordance -- that is what the initials exist for. A path that
    /// does not resolve is the caller's mistake, and the honest rendering is the fallback the widget
    /// already has, not a broken-image glyph in a 40 px disc. On desktop the failure is logged so
    /// the caller can find it.
    fn paint_image_source(
        &mut self,
        context: &mut RenderContext,
        rect: Rect,
        center: Point,
        size: u32,
    ) -> bool {
        if self.image_source.is_empty() || size == 0 {
            return false;
        }
        let Some(pixels) = self.resolved_image_source() else {
            return false;
        };
        // The picture is clipped to the avatar's own shape before it is drawn, so a square bitmap
        // does not put corners outside a circular avatar. The clip is a rectangle because that is
        // the primitive the renderer has; a rounded-square avatar's own radius is close enough to
        // the rectangle that clipping to it would only remove pixels the radius already rounds off.
        context.push_clip(rect.x, rect.y, rect.width, rect.height);
        context.draw_image(rect.x, rect.y, rect.width, rect.height, &pixels);
        context.pop_clip();
        // A square clip still leaves the corners of a picture outside a *circle*, so those four
        // corner squares are repainted in the disc's colour. Four rectangles rather than a mask,
        // because the renderer has no per-pixel mask and the corners are cheap.
        if !self.square {
            let radius = (size / 2) as i32;
            let corner = (size as f32 * 0.1464).ceil() as u32;
            let disc_color = self.disc_fill();
            for (dx, dy) in [(0i32, 0i32), (1, 0), (0, 1), (1, 1)] {
                let x = center.x - radius + dx * (size as i32 - corner as i32);
                let y = center.y - radius + dy * (size as i32 - corner as i32);
                context.fill_rect(Rect::new(x, y, corner, corner), disc_color);
            }
        }
        true
    }

    /// The RGBA8 pixels for the current [`Self::image_source`], shared process-wide.
    ///
    /// Returns `None` when there is no source, when the file cannot be read, or when the bytes do
    /// not decode -- see [`Self::paint_image_source`] for why that is not an error to render.
    ///
    /// # Two layers, for two different questions
    ///
    /// `decoded_source_key` answers "has *this avatar* already resolved this source", and it is a
    /// string compare -- the cheapest possible check, which matters because this runs on every frame.
    /// [`crate::image::cache`] answers "has *anybody in this process* already decoded these bytes",
    /// and it is what makes a list of avatars sharing one picture decode it once instead of once per
    /// row. The first is the fast path; the second is the reason the fast path is not the only thing
    /// there is.
    ///
    /// What is *stored* is now the shared [`Arc<Vec<u8>>`](alloc::sync::Arc) rather than a `Vec` this
    /// control owns, so the pixels are the cache's allocation and a hit here hands back the same one
    /// an `image_view` of the same file would get.
    fn resolved_image_source(&mut self) -> Option<alloc::sync::Arc<Vec<u8>>> {
        #[cfg(all(feature = "image", not(alloc_frugal)))]
        {
            if self.decoded_source_key.as_deref() != Some(self.image_source.as_str()) {
                // One entry point for "a file's RGBA8 pixels", shared by every control that wants
                // one -- so an avatar and an image view of the same file cannot disagree about what
                // the file looks like, and cannot decode it twice.
                self.decoded_source = crate::image::cache::file_rgba8_or_none(&self.image_source);
                // The key is set whether or not the decode succeeded, so a bad path is not re-read
                // every frame -- an avatar that cannot load must not make every frame a disk read.
                self.decoded_source_key = Some(self.image_source.clone());
            }
            self.decoded_source.clone()
        }
        #[cfg(not(all(feature = "image", not(alloc_frugal))))]
        {
            // Without the image feature there is no decoder to call, so there is nothing to paint.
            // An empty source means the same thing, so one arm states both rather than each reader
            // re-deriving it.
            None
        }
    }

    /// The colour the disc is painted in, for the corner squares that mask a square picture.
    ///
    /// One derivation, read by the disc fill *and* by the mask: two copies of the precedence would
    /// leave mask squares a different colour from the disc they are meant to continue.
    fn disc_fill(&self) -> Color {
        let window_background = crate::style::resolved_theme_style("avatar")
            .as_ref()
            .and_then(|theme| theme.background_color);
        let raised =
            crate::style::resolved_theme_style("button").and_then(|button| button.background_color);
        let disc_background = self
            .base
            .style()
            .background_color
            .filter(|colour| Some(*colour) != window_background)
            .or(raised)
            .or(window_background)
            .unwrap_or_else(Self::default_bg);
        if self.bg_color_is_explicit {
            self.bg_color
        } else {
            disc_background
        }
    }
}

impl EventHandler for Avatar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::widget::WidgetKind;

    #[test]
    fn avatar_default_state() {
        let avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        assert_eq!(avatar.text(), "");
        assert!(!avatar.is_square());
        assert_eq!(avatar.bg_color(), Color::PRIMARY);
        assert_eq!(avatar.size(), 40);
        assert_eq!(avatar.kind(), WidgetKind::Avatar);
    }

    #[test]
    fn avatar_default_size_when_zero_geometry() {
        let avatar = Avatar::new(Rect::new(10, 20, 0, 0));
        assert_eq!(avatar.size(), 40);
        assert_eq!(avatar.geometry().width, 40);
        assert_eq!(avatar.geometry().height, 40);
    }

    #[test]
    fn avatar_set_text() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        avatar.set_text("JD");
        assert_eq!(avatar.text(), "JD");
    }

    #[test]
    fn avatar_set_text_clears() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        avatar.set_text("AB");
        avatar.set_text("");
        assert_eq!(avatar.text(), "");
    }

    #[test]
    fn avatar_set_bg_color() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        let custom = Color::rgb(255, 0, 0);
        avatar.set_bg_color(custom);
        assert_eq!(avatar.bg_color(), custom);
    }

    #[test]
    fn avatar_set_square() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        assert!(!avatar.is_square());
        avatar.set_square(true);
        assert!(avatar.is_square());
        avatar.set_square(false);
        assert!(!avatar.is_square());
    }

    #[test]
    fn avatar_set_size() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        avatar.set_size(64);
        assert_eq!(avatar.size(), 64);
        assert_eq!(avatar.geometry().width, 64);
        assert_eq!(avatar.geometry().height, 64);
    }

    #[test]
    fn avatar_set_size_preserves_position() {
        let mut avatar = Avatar::new(Rect::new(10, 20, 40, 40));
        avatar.set_size(56);
        assert_eq!(avatar.geometry().x, 10);
        assert_eq!(avatar.geometry().y, 20);
        assert_eq!(avatar.geometry().width, 56);
        assert_eq!(avatar.geometry().height, 56);
    }

    #[test]
    fn avatar_widget_trait_geometry() {
        let mut avatar = Avatar::new(Rect::new(5, 10, 48, 48));
        assert_eq!(avatar.geometry(), Rect::new(5, 10, 48, 48));
        avatar.set_geometry(Rect::new(0, 0, 64, 64));
        assert_eq!(avatar.geometry(), Rect::new(0, 0, 64, 64));
    }

    #[test]
    fn avatar_widget_trait_kind() {
        let avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        assert_eq!(avatar.kind(), WidgetKind::Avatar);
    }

    #[test]
    fn avatar_handle_event_no_panic() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        // EventHandler should not panic for any event type
        avatar.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        avatar.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        avatar.handle_event(&Event::MouseMove { pos: Point::new(20, 20) });
        avatar.handle_event(&Event::KeyPress { key: 0x41, modifiers: 0 });
        avatar.handle_event(&Event::KeyRelease { key: 0x41, modifiers: 0 });
    }

    #[test]
    fn avatar_disabled_blocks_events() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        avatar.set_enabled(false);
        // Should not panic
        avatar.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        avatar.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
    }

    #[test]
    fn avatar_svg_output() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        avatar.set_text("MW");
        let svg = crate::widget::svg::render_to_svg(&mut avatar);
        assert!(svg.starts_with("<svg"), "SVG output should start with <svg, got: {svg:.80}");
    }

    #[test]
    fn avatar_square_svg_output() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 48, 48));
        avatar.set_text("AB");
        avatar.set_square(true);
        let svg = crate::widget::svg::render_to_svg(&mut avatar);
        assert!(
            svg.starts_with("<svg"),
            "Square SVG output should start with <svg, got: {svg:.80}"
        );
    }

    #[test]
    fn avatar_different_sizes() {
        let small = Avatar::new(Rect::new(0, 0, 24, 24));
        assert_eq!(small.size(), 24);

        let large = Avatar::new(Rect::new(0, 0, 96, 96));
        assert_eq!(large.size(), 96);
    }

    #[test]
    fn avatar_size_min_of_width_and_height() {
        let avatar = Avatar::new(Rect::new(0, 0, 60, 40));
        // size should be min(60, 40) = 40
        assert_eq!(avatar.size(), 40);
        assert_eq!(avatar.geometry().width, 40);
        assert_eq!(avatar.geometry().height, 40);
    }

    // ── `image_source` loads and paints a picture ──

    /// A real PNG in the avatar's cell paints, and the initials stop being drawn.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `image_source` was stored, published (getter, setter, schema row, round-trip test) and read
    /// by nothing. The widget's own field comment recorded it: "the widget has no image pipeline
    /// yet, so this is carried as state ... the initials remain the rendered fallback until a loader
    /// is wired in". A round-trip test therefore passed while a source changed no pixel.
    ///
    /// # Why a real file rather than injected pixels
    ///
    /// The claim is that *a source path becomes a picture*, so the test writes the smallest real
    /// PNG it can and points the widget at it. Injecting RGBA bytes would test the draw and skip the
    /// loader, which is the half that did not exist.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn a_real_image_source_is_loaded_and_painted_over_the_disc() {
        use crate::widget::svg::render_to_svg;

        // A 2x2 PNG with one fully-opaque red pixel and three transparent ones, written by hand so
        // the fixture does not depend on an encoder being compiled in.
        //
        // # Why the filename carries a per-test suffix
        //
        // [`crate::image::cache`] is process-wide and keyed on content, so two tests that write the
        // *same path* with *different bytes* can observe each other: the decoder reads whatever the
        // other test's `write` left, which made `an_unloadable_source_falls_back_to_the_initials`
        // pass alone and fail under the full suite. A name no other test uses is what makes each
        // test's I/O its own.
        let path = std::env::temp_dir().join("rw_avatar_source_real_image.png");
        std::fs::write(&path, MINIMAL_PNG).expect("the fixture is writable");
        let source = path.to_str().expect("a UTF-8 temp path").to_string();

        let initials_only = {
            let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
            avatar.set_text("MW");
            render_to_svg(&mut avatar)
        };
        let with_picture = {
            let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
            avatar.set_text("MW");
            avatar.set_image_source(&source);
            render_to_svg(&mut avatar)
        };
        assert_ne!(
            initials_only, with_picture,
            "a real source must change the rendering -- that was the dead state"
        );
        // The picture replaces the initials rather than being drawn behind them: two identities on
        // one disc read as neither. `text_ink_boxes` (not `text_subpath_count`) because the latter
        // counts *every* path's subpaths, and the disc's own shapes are paths too.
        let picture_runs = crate::widget::svg::text_ink_boxes(&with_picture).len();
        let initials_runs = crate::widget::svg::text_ink_boxes(&initials_only).len();
        assert!(
            initials_runs > 0,
            "the fixture must draw initials without a source, or this proves nothing"
        );
        assert_eq!(
            picture_runs, 0,
            "a painted picture must suppress the initials fallback ({picture_runs} text runs)"
        );
        // And the picture itself is reported as an `<image>` element, which is what the SVG backend
        // emits for a non-empty `draw_image` -- not as glyphs and not as a placeholder rectangle.
        assert!(
            with_picture.contains("<image"),
            "the decoded pixels must reach the document as an embedded image"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// A source that cannot be loaded falls back to the initials instead of blanking the avatar.
    ///
    /// The alternative -- painting nothing, or an error glyph -- would leave a hole where an
    /// identity belongs, and the initials are exactly the fallback this widget exists to provide.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn an_unloadable_source_falls_back_to_the_initials() {
        use crate::widget::svg::{render_to_svg, text_ink_boxes};

        let build = |source: &str| {
            let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
            avatar.set_text("MW");
            if !source.is_empty() {
                avatar.set_image_source(source);
            }
            render_to_svg(&mut avatar)
        };

        let missing = build("/definitely/not/a/real/avatar-file.png");
        let no_source = build("");
        assert_eq!(
            missing, no_source,
            "an unloadable source renders exactly the initials fallback, not a hole"
        );
        assert!(!text_ink_boxes(&missing).is_empty(), "the initials are still drawn");

        // A file that exists but is not an image is the same case, so a caller who points at a text
        // file gets the fallback rather than a panic or a blank disc.
        let not_an_image =
            std::env::temp_dir().join(format!("rw_avatar_not_an_image_{}.txt", std::process::id()));
        std::fs::write(&not_an_image, b"this is not a picture").expect("writable");
        let garbage = build(not_an_image.to_str().expect("UTF-8 temp path"));
        assert_eq!(garbage, no_source, "undecodable bytes fall back too");
        let _ = std::fs::remove_file(&not_an_image);
    }

    /// Re-setting the same source does not re-read the file, and changing it drops the cached pixels.
    ///
    /// The cache is what keeps `draw` from doing I/O once a frame; the invalidation is what keeps a
    /// new source from painting the old picture, which would be worse than no cache at all.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn the_decoded_source_is_cached_and_invalidated_by_the_source() {
        let mut avatar = Avatar::new(Rect::new(0, 0, 40, 40));
        assert!(avatar.decoded_source.is_none(), "nothing is decoded before a source is set");

        let path = std::env::temp_dir().join("rw_avatar_cache_test.png");
        std::fs::write(&path, MINIMAL_PNG).expect("the fixture is writable");
        let source = path.to_str().expect("a UTF-8 temp path").to_string();

        avatar.set_image_source(&source);
        let first = avatar.resolved_image_source();
        assert!(first.is_some(), "the fixture decodes");
        assert_eq!(
            avatar.decoded_source_key.as_deref(),
            Some(source.as_str()),
            "the cache records which source its pixels belong to"
        );
        let pixels = avatar.decoded_source.clone();
        // A second resolve with the same key returns the same pixels without touching the file.
        assert_eq!(avatar.resolved_image_source(), pixels);

        // A new source drops both halves of the cache, so the next resolve cannot answer with the
        // picture the old one produced.
        avatar.set_image_source("/somewhere/else.png");
        assert!(avatar.decoded_source.is_none(), "pixels are dropped with the source");
        assert!(avatar.decoded_source_key.is_none(), "and so is the key they belong to");

        let _ = std::fs::remove_file(&path);
    }

    /// The smallest valid PNG: a 2x2 truecolour-with-alpha image whose first pixel is opaque red.
    ///
    /// Written out as bytes rather than produced by an encoder so the fixture works wherever the
    /// `image` feature is on, including builds without the encoder side.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    const MINIMAL_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
        0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR, 13 data bytes
        0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, // width 2, height 2
        0x08, 0x06, 0x00, 0x00, 0x00, 0x72, 0xb6, 0x0d, // 8-bit, colour type 6 (RGBA), CRCs
        0x24, 0x00, 0x00, 0x00, 0x0f, 0x49, 0x44, 0x41, // IDAT, 15 data bytes
        0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0x00, // zlib stream
        0x44, 0x48, 0x00, 0x00, 0x1e, 0xf3, 0x01, 0xff, // ...
        0x6a, 0x37, 0x5d, 0xad, 0x00, 0x00, 0x00, 0x00, // IDAT CRC
        0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82, // IEND
    ];
}
