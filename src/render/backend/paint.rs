// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

#![allow(clippy::chunks_exact_to_as_chunks)]
//! Paint backend trait and software implementation.
use super::batch::BatchState;
#[cfg(test)]
use crate::compat::vec;
use crate::compat::{Arc, Rc, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::render::pipeline::{
    fill_conic_ramp, ramp_lut_for_stops, sat_add_i32, sat_mul_i32, sat_neg_i32,
    u32_to_i32_saturating, ConicWrite,
};
use crate::render::{
    blend_pixel_with_mode, BlendMode, MatteMode, RenderCommand, ShapedText, SoftwareRenderConfig,
    SoftwareSurface, TextMetrics,
};

/// Pluggable paint backend strategy used by render scene composition.
///
/// A backend receives a frame's worth of [`RenderCommand`]s between
/// [`PaintBackend::begin_frame`] and [`PaintBackend::end_frame`] and is
/// responsible for rasterising them into some surface. Implementations may be
/// software (see [`SoftwarePaintBackend`]) or hardware. Commands arriving
/// outside a frame are outside the contract.
pub trait PaintBackend {
    /// Starts a frame, filling the target with `clear` first.
    ///
    /// Must be paired with [`PaintBackend::end_frame`]. Anything drawn before
    /// this call is not guaranteed to survive it.
    fn begin_frame(&mut self, clear: Color);
    /// Finishes the frame, making its contents available for presentation.
    ///
    /// For a software backend this is what flushes any pending batched work;
    /// reading the surface before this point may see a partially drawn frame.
    fn end_frame(&mut self);
    /// Reports `rect` as changed since the last present.
    ///
    /// A backend that can publish a region at a time implements this and
    /// [`PaintBackend::present_damage`] together. The default is a no-op, so a backend that only
    /// knows how to swap a whole frame stays correct: it simply publishes everything.
    fn mark_damage(&mut self, _rect: Rect) {}
    /// The damaged rectangles reported since the last present.
    fn damage_regions(&self) -> &[(i32, i32, u32, u32)] {
        &[]
    }
    /// Publishes only the reported damage.
    ///
    /// Returns `false` when the backend cannot publish a region (or nothing was reported), which
    /// tells the caller to fall back to [`PaintBackend::end_frame`].
    fn present_damage(&mut self) -> bool {
        false
    }
    /// Drops every cached layer foreground held by the backend.
    ///
    /// Layer cache identities are caller-supplied, so a caller that reuses one for different
    /// content must clear the cache. A backend with no layer cache ignores this.
    fn clear_layer_cache(&mut self) {}
    /// The number of cached layer foregrounds currently held (test observability).
    #[cfg(test)]
    fn cached_layer_count(&self) -> usize {
        0
    }
    /// Executes a single drawing command in the current frame.
    ///
    /// The command is applied in sequence, so ordering matters: a clip push
    /// affects every command until its matching pop.
    fn execute_command(&mut self, command: &RenderCommand);
    /// Draws `data` as an RGBA image at `(x, y)` scaled to `width` x `height` pixels.
    ///
    /// # Why this exists next to [`PaintBackend::execute_command`] (D09-RENDER-02)
    ///
    /// `RenderCommand::DrawImage` owns its pixel bytes, so routing an image through it forces
    /// the caller to copy the whole source buffer into the command. A backend that consumes the
    /// image **synchronously** — the software backend blits it immediately and retains nothing —
    /// gains nothing from that copy; it is pure cost proportional to the image size, paid on
    /// every frame of a static image and every frame of an animation.
    ///
    /// This entry point hands the pixels over by reference, so a backend that does not retain
    /// the command can draw straight from the caller's buffer. The **default implementation**
    /// reproduces the old behaviour exactly: it builds an owning `DrawImage` command (one copy)
    /// and executes it. A backend that keeps the command — or renders it later — must keep that
    /// default, because it really does need to own the bytes. Only an immediate backend should
    /// override it, and then the copy disappears.
    fn draw_image(&mut self, x: i32, y: i32, width: u32, height: u32, data: &[u8]) {
        self.execute_command(&RenderCommand::DrawImage {
            x,
            y,
            width,
            height,
            data: Arc::from(data),
        });
    }
    /// Fills borrowed contours using the non-zero winding rule.
    ///
    /// Immediate backends can consume the slices directly. The default preserves
    /// the owning-command contract required by deferred and recording backends.
    fn fill_path_non_zero(
        &mut self,
        points: &[crate::core::Point],
        contours: &[(usize, usize)],
        color: Color,
        offset_x: i32,
        offset_y: i32,
    ) -> Result<(), crate::render::CompoundPathError> {
        let translated = points
            .iter()
            .map(|point| crate::core::Point::new(point.x + offset_x, point.y + offset_y))
            .collect();
        let path = crate::render::CompoundPath::new(translated, contours.to_vec())?;
        self.execute_command(&RenderCommand::FillPathNonZero { path, color });
        Ok(())
    }
    /// Draws `text` with its origin at `origin` in `font` and `color`.
    ///
    /// # Why this is a trait method and not only a command
    ///
    /// [`RenderCommand::DrawText`] owns its `Arc<str>` and `Font`, so routing every label through
    /// the command hands the bytes over by refcount (G-12). This entry point hands the text over
    /// by reference; the **default implementation** is the old behaviour, so a recording or
    /// deferred backend is unchanged. Only an immediate backend should override it.
    fn draw_text(
        &mut self,
        origin: crate::core::Point,
        text: &str,
        font: &Font,
        color: Color,
        alignment: crate::core::HorizontalAlignment,
    ) {
        self.execute_command(&RenderCommand::DrawText {
            origin,
            text: crate::compat::Arc::from(text),
            font: font.clone(),
            color,
            alignment,
        });
    }
    /// Fills `rect` with `gradient`.
    ///
    /// The same shape as [`Self::draw_text`], and for the same reason: [`RenderCommand::DrawGradient`]
    /// owns an `Arc<Gradient>` (G-12), so a recording backend stores the same immutable paint
    /// server by refcount rather than deep-copying its `stops` vector. An immediate backend
    /// overrides this and paints straight from the borrowed gradient.
    fn fill_gradient(&mut self, rect: crate::core::Rect, gradient: &crate::style::Gradient) {
        #[cfg(test)]
        note_gradient_owned_command();
        self.execute_command(&RenderCommand::DrawGradient {
            rect,
            gradient: Arc::new(gradient.clone()),
        });
    }
    /// Returns the size of the target surface in logical (DPI-independent)
    /// units.
    fn size(&self) -> Size;
    /// Resizes the target surface, discarding its contents.
    fn set_size(&mut self, size: Size);
    /// Returns the device pixel ratio the backend rasterises at; `1.0` is the
    /// 96-DPI baseline.
    fn dpi_scale(&self) -> f32;
    /// Sets the device pixel ratio used for subsequent rasterisation.
    fn set_dpi_scale(&mut self, dpi_scale: f32);
    /// Measures `text` in `font`, returning advance and bounding-box metrics.
    ///
    /// Measurement is independent of the current frame, so it is valid to call
    /// outside `begin_frame`/`end_frame` — layout code relies on this.
    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics;
    /// Shapes `text` in `font`, returning positioned glyphs.
    ///
    /// Like [`PaintBackend::measure_text`], valid outside a frame. Backends
    /// without real shaping may return a trivial single-run result.
    fn shape_text(&self, text: &str, font: &Font) -> Rc<ShapedText>;
    /// Returns the final frame as tightly packed RGBA bytes, 8 bits per
    /// channel, row-major, top row first.
    ///
    /// The slice length is `width * height * 4` and the buffer is owned by the
    /// backend: it is invalidated by the next frame, resize, or mutable access.
    fn frame_rgba(&self) -> &[u8];
    /// Apply backend-specific render quality configuration.
    ///
    /// The default implementation ignores the config, which is the honest
    /// behaviour for a backend with no quality knobs; such a backend still
    /// accepts the call rather than failing, so callers need not feature-detect.
    fn apply_render_config(&mut self, _config: SoftwareRenderConfig) {}
    /// Read backend-specific render quality configuration.
    ///
    /// The default returns [`SoftwareRenderConfig::default`], which does **not**
    /// necessarily reflect what a non-software backend is actually doing — a
    /// backend with real quality settings should override both this and
    /// [`PaintBackend::apply_render_config`].
    fn render_config(&self) -> SoftwareRenderConfig {
        SoftwareRenderConfig::default()
    }
}
// Test-only counter: paints that adopted an owned frame instead of copying one in. The saving is
// the **absence** of a full-frame copy, which leaves no trace in the pixels, so the count is the
// only way a regression test can observe the optimisation.
#[cfg(test)]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static SEED_ADOPTIONS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Software implementation of the paint backend strategy.
///
/// Rasterises commands into an in-memory [`SoftwareSurface`]. This is the
/// reference implementation and the fallback when no accelerated backend is
/// available.
pub struct SoftwarePaintBackend {
    pub(crate) surface: SoftwareSurface,
    pub(crate) batch_state: BatchState,
}
impl SoftwarePaintBackend {
    /// Creates a software paint backend with a target size and DPI scale.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        Self { surface: SoftwareSurface::new(size, dpi_scale), batch_state: BatchState::new() }
    }
    /// Returns immutable access to the underlying software surface.
    pub fn surface(&self) -> &SoftwareSurface {
        &self.surface
    }
    /// Returns mutable access to the underlying software surface.
    pub fn surface_mut(&mut self) -> &mut SoftwareSurface {
        &mut self.surface
    }
    /// Apply software render quality configuration via backend facade.
    pub fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
        self.surface.apply_render_config(config);
    }
    /// Get software render quality configuration via backend facade.
    pub fn render_config(&self) -> SoftwareRenderConfig {
        self.surface.render_config()
    }

    /// Seeds the back buffer with a previously rendered frame.
    ///
    /// # Why this is needed for partial repaint
    ///
    /// `begin_frame` clears — that is what a full-frame render wants. A partial
    /// repaint must instead start from the previous frame and overwrite only the
    /// damaged pixels, so it needs a way to load that frame without clearing. This is
    /// that way.
    ///
    /// # Why the length is checked rather than trusted
    ///
    /// A caller can hand in a frame rendered at a different size — the widget was
    /// resized between frames, or the buffer came from somewhere else. Copying a
    /// short buffer would leave the tail of the surface holding whatever the clear
    /// left, so a mismatch is reported instead of silently producing a frame that is
    /// half stale. Returns `false` and leaves the surface untouched.
    pub fn seed_from(&mut self, frame: &[u8]) -> bool {
        let target = &mut self.surface.buffer.back;
        if frame.len() != target.len() {
            return false;
        }
        target.copy_from_slice(frame);
        true
    }

    /// Adopts `frame` as this backend's draw target instead of copying it in.
    ///
    /// # Why moving beats copying
    ///
    /// An incremental frame starts from the pixels the previous frame left and paints only the
    /// damaged regions. Copying them in is a full-frame `memcpy` — 4 MB at 1280×800 — which costs
    /// more than painting a control-sized damage region. When the caller owns the previous buffer
    /// outright (the common case: it has already presented and dropped it), the buffer can simply
    /// become the target.
    ///
    /// Returns `false` when `frame` is not the surface's size, which is the caller's cue to fall
    /// back to [`Self::seed_from`].
    pub fn seed_from_owned(&mut self, frame: crate::compat::Vec<u8>) -> bool {
        if frame.len() != self.surface.buffer.back.len() {
            return false;
        }
        self.surface.buffer.back = frame;
        #[cfg(test)]
        SEED_ADOPTIONS.with(|count| count.set(count.get() + 1));
        true
    }

    /// Resets and returns the adoption counter.
    ///
    /// Test-only, and the observable the incremental-frame regression test asserts on: the saving
    /// is the **absence** of a full-frame copy, and a copy leaves no other trace in the pixels.
    #[cfg(all(test, widgets_unstripped))]
    pub(crate) fn take_seed_adoptions() -> usize {
        SEED_ADOPTIONS.with(|count| count.replace(0))
    }

    /// Drops every cached layer foreground held by this backend's surface.
    ///
    /// Cache identities are caller-supplied, so a caller that reuses an identity for different
    /// content must clear the cache between uses. Resizing the surface also clears it.
    pub fn clear_layer_cache(&mut self) {
        self.surface.buffer.layer_cache.clear();
        self.surface.buffer.layer_cache_cursor = 0;
    }

    /// The number of cached layer foregrounds currently held.
    #[cfg(test)]
    pub(crate) fn cached_layer_count(&self) -> usize {
        self.surface.buffer.layer_cache.len()
    }

    /// Consumes the backend and returns the presented frame without copying it.
    ///
    /// # Why this is preferable to `frame_rgba().to_vec()`
    ///
    /// The three render entry points in `widget::runtime` own their backend and return
    /// the frame, so they can move the buffer instead of copying it. `to_vec` duplicated
    /// the whole `width * height * 4` buffer on every painted frame; at 1280×800 that is
    /// 4 MB of memcpy per frame on the UI thread.
    pub fn into_frame_rgba(self) -> crate::compat::Vec<u8> {
        self.surface.into_frame_rgba()
    }
}
impl PaintBackend for SoftwarePaintBackend {
    fn begin_frame(&mut self, clear: Color) {
        self.surface.begin_frame(clear);
    }
    fn end_frame(&mut self) {
        self.surface.end_frame();
    }
    fn mark_damage(&mut self, rect: Rect) {
        self.surface.mark_damage(rect);
    }
    fn damage_regions(&self) -> &[(i32, i32, u32, u32)] {
        self.surface.damage_regions()
    }
    fn present_damage(&mut self) -> bool {
        self.surface.present_damage()
    }
    fn clear_layer_cache(&mut self) {
        self.surface.buffer.layer_cache.clear();
        self.surface.buffer.layer_cache_cursor = 0;
    }
    #[cfg(test)]
    fn cached_layer_count(&self) -> usize {
        self.surface.buffer.layer_cache.len()
    }
    fn execute_command(&mut self, command: &RenderCommand) {
        route_command_to_surface(&mut self.surface, command);
    }
    /// Draws an image straight from the caller's slice — no owning command, no copy.
    ///
    /// # Why this override matters (D09-RENDER-02)
    ///
    /// The software backend rasterises an image the moment it is asked to, and keeps no
    /// reference to the bytes afterwards. The trait default would copy the whole source into a
    /// `RenderCommand::DrawImage` for that one synchronous call; this draws from the borrowed
    /// slice instead, so a widget that shows a static or animated image pays no per-frame
    /// full-image allocation or copy.
    fn draw_image(&mut self, x: i32, y: i32, width: u32, height: u32, data: &[u8]) {
        #[cfg(test)]
        note_image_bytes_drawn_without_copy(data.len());
        self.surface.draw_image(x, y, width, height, data);
    }
    /// Draws text straight from the caller's `&str` and `&Font`.
    ///
    /// The software backend rasterises immediately, so the owning `DrawText` command would be
    /// two allocations (text and font family) per label per frame for bytes it never keeps.
    fn draw_text(
        &mut self,
        origin: crate::core::Point,
        text: &str,
        font: &Font,
        color: Color,
        alignment: crate::core::HorizontalAlignment,
    ) {
        self.surface.draw_text(origin, text, font, color, alignment);
    }
    /// Fills `rect` with `gradient` without owning a copy of it.
    ///
    /// The gradient-paint counterpart of [`Self::draw_text`]: the software surface reads the stops
    /// and paints at once, so the owning command would copy the `stops` vector per fill for pixels
    /// it immediately discards.
    fn fill_gradient(&mut self, rect: crate::core::Rect, gradient: &crate::style::Gradient) {
        self.surface.fill_rect_gradient(rect, gradient);
    }
    fn fill_path_non_zero(
        &mut self,
        points: &[crate::core::Point],
        contours: &[(usize, usize)],
        color: Color,
        offset_x: i32,
        offset_y: i32,
    ) -> Result<(), crate::render::CompoundPathError> {
        self.surface.fill_path_non_zero_borrowed(points, contours, color, offset_x, offset_y)
    }
    fn size(&self) -> Size {
        self.surface.size()
    }
    fn set_size(&mut self, size: Size) {
        self.surface.resize(size);
    }
    fn dpi_scale(&self) -> f32 {
        self.surface.dpi_scale()
    }
    fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.surface.set_dpi_scale(dpi_scale);
    }
    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        self.surface.measure_text(text, font)
    }
    fn shape_text(&self, text: &str, font: &Font) -> Rc<ShapedText> {
        self.surface.shape_text(text, font)
    }
    fn frame_rgba(&self) -> &[u8] {
        self.surface.frame_rgba()
    }
    fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
        self.surface.apply_render_config(config);
    }
    fn render_config(&self) -> SoftwareRenderConfig {
        self.surface.render_config()
    }
}

// Test-only counter of source image bytes the software backend drew **without** copying, so a
// regression test can prove the borrowed path is the one taken and that the owned-command path
// copies exactly once when a caller builds the command itself (D09-RENDER-02). Kept out of
// release builds (principle #28).
#[cfg(test)]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static IMAGE_BYTES_DRAWN_WITHOUT_COPY: core::cell::Cell<usize> =
        const { core::cell::Cell::new(0) };
}

/// Adds `bytes` to the test-only "drawn without copy" counter. (D09-RENDER-02)
#[cfg(test)]
pub(crate) fn note_image_bytes_drawn_without_copy(bytes: usize) {
    IMAGE_BYTES_DRAWN_WITHOUT_COPY.with(|cell| cell.set(cell.get() + bytes));
}

/// Resets and returns the test-only "drawn without copy" counter. (D09-RENDER-02)
#[cfg(test)]
pub(crate) fn take_image_bytes_drawn_without_copy() -> usize {
    IMAGE_BYTES_DRAWN_WITHOUT_COPY.with(|cell| cell.replace(0))
}

// Test-only count of gradient fills that went through the **owning** command, i.e. the trait's
// default implementation. A backend that paints immediately overrides `fill_gradient` and never
// builds the command, so zero is the evidence that the override is the one being taken — the
// counter moves only when a backend falls back to the default.
#[cfg(test)]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static GRADIENT_OWNED_COMMANDS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Records one gradient fill routed through the owning `RenderCommand`.
#[cfg(test)]
fn note_gradient_owned_command() {
    GRADIENT_OWNED_COMMANDS.with(|cell| cell.set(cell.get() + 1));
}

/// Resets and returns that counter.
#[cfg(test)]
fn take_gradient_owned_command_count() -> usize {
    GRADIENT_OWNED_COMMANDS.with(|cell| cell.replace(0))
}

/// Route one [`RenderCommand`] into a bare [`SoftwareSurface`].
///
/// # Why this is a free function (D09-RENDER-05)
///
/// `PaintBackend::execute_command` for [`SoftwarePaintBackend`] is exactly this
/// routing over `self.surface`. Batch replay has to paint while a *different* field
/// of the backend (`batch_state`) is borrowed immutably, and the borrow checker
/// rejects borrowing `self` both ways — which is why replay used to deep-`clone()`
/// the whole batch state. Splitting the routing out lets the replay path paint
/// through the surface alone. Both paths share this one function, so the command
/// routing cannot drift between them.
pub(super) fn route_command_to_surface(surface: &mut SoftwareSurface, command: &RenderCommand) {
    // A cache hit suppresses the enclosed commands, but their push/pop pairs still have to be
    // counted so the matching `PopLayer` can be recognised. Only balancing commands are seen while
    // suppressed; everything that would paint or change frame state is discarded.
    if surface.buffer.suppress_depth > 0 {
        match command {
            RenderCommand::PushClip { .. }
            | RenderCommand::PushClipRoundedRect { .. }
            | RenderCommand::PushLayer { .. }
            | RenderCommand::PushLayerGroup { .. }
            | RenderCommand::PushMatte { .. } => {
                surface.buffer.suppress_depth += 1;
                return;
            }
            RenderCommand::PopClip | RenderCommand::PopMatte => {
                surface.buffer.suppress_depth -= 1;
                return;
            }
            RenderCommand::PopLayer => {
                surface.buffer.suppress_depth -= 1;
                if surface.buffer.suppress_depth > 0 {
                    return;
                }
                // The outer cached layer's pop: fall through so `pop_layer` composites the cached
                // foreground over the live backdrop.
            }
            _ => return,
        }
    }
    match command {
        RenderCommand::FillRect { rect, color } => surface.fill_rect(*rect, *color),
        #[cfg(feature = "3d")]
        RenderCommand::FillQuad { corners, color } => {
            // The surface's existing scanline polygon fill is the software expression of the
            // same quadrilateral. It is deliberately not reduced to a bounding box: a rotated
            // item must keep its corners.
            surface.draw_path(&corners[..], true, *color, true, 0);
        }
        #[cfg(feature = "3d")]
        RenderCommand::FillQuadAA { corners, color } => surface.fill_quad_aa(*corners, *color),
        #[cfg(feature = "3d")]
        RenderCommand::DrawMesh3D { mesh, model, camera, viewport } => {
            surface.draw_mesh_3d(mesh, model, camera, *viewport);
        }
        #[cfg(feature = "3d")]
        RenderCommand::StrokeQuad { corners, color, width } => {
            // Four connected edges, each drawn by the same line primitive a `DrawLineStroke`
            // uses. A closed polygon outline would also work, but its joins are mitered, and the
            // affine-mapped rectangle a widget produces has corners the caller positioned: a
            // miter there would push the stroke outside the geometry the caller asked for.
            if *width > 0 {
                for index in 0..4 {
                    surface.draw_line_with_width(
                        corners[index],
                        corners[(index + 1) % 4],
                        *color,
                        *width,
                    );
                }
            }
        }
        #[cfg(feature = "3d")]
        RenderCommand::DrawImageQuad { corners, width, height, data } => {
            surface.draw_image_quad(*corners, *width, *height, data);
        }
        RenderCommand::DrawRect { rect, color } => surface.draw_rect(*rect, *color),
        RenderCommand::DrawRectStroke { rect, color, width } => {
            surface.draw_rect_with_width(*rect, *color, *width)
        }
        RenderCommand::FillRoundedRect { rect, radii, color } => {
            surface.fill_rounded_rect_radii(*rect, *radii, *color)
        }
        RenderCommand::FillRoundedRectAA { rect, radii, color } => {
            surface.fill_rounded_rect_radii_aa(*rect, *radii, *color)
        }
        RenderCommand::DrawRoundedRectStroke { rect, radii, color, width } => {
            surface.draw_rounded_rect_radii_with_width(*rect, *radii, *color, *width)
        }
        RenderCommand::DrawRoundedRectStrokeAA { rect, radii, color, width } => {
            surface.draw_rounded_rect_radii_aa_with_width(*rect, *radii, *color, *width)
        }
        RenderCommand::DrawLine { from, to, color } => surface.draw_line(*from, *to, *color),
        RenderCommand::DrawLineAA { from, to, color } => surface.draw_line_aa(*from, *to, *color),
        RenderCommand::DrawLineStrokeAA { from, to, color, width } => {
            surface.draw_line_aa_with_width(*from, *to, *color, *width)
        }
        RenderCommand::DrawLineStroke { from, to, color, width } => {
            surface.draw_line_with_width(*from, *to, *color, *width)
        }
        RenderCommand::FillCircle { center, radius, color } => {
            surface.fill_circle(*center, *radius, *color)
        }
        RenderCommand::FillCircleAA { center, radius, color } => {
            surface.fill_circle_aa(*center, *radius, *color)
        }
        RenderCommand::DrawCircle { center, radius, color } => {
            surface.draw_circle(*center, *radius, *color)
        }
        RenderCommand::DrawCircleStroke { center, radius, color, width } => {
            surface.draw_circle_with_width(*center, *radius, *color, *width)
        }
        RenderCommand::DrawText { origin, text, font, color, alignment } => {
            surface.draw_text(*origin, text, font, *color, *alignment)
        }
        RenderCommand::DrawTextDecorated { origin, text, font, color, alignment, decoration } => {
            draw_text_decorated_on_surface(
                surface,
                *origin,
                text,
                font,
                *color,
                *alignment,
                *decoration,
            );
        }
        RenderCommand::DrawImage { x, y, width, height, data } => {
            surface.draw_image(*x, *y, *width, *height, data)
        }
        RenderCommand::PushClip { x, y, width, height } => {
            surface.push_clip(*x, *y, *width, *height)
        }
        RenderCommand::PushClipRoundedRect { x, y, width, height, radii } => {
            // The rounded clip reuses the exact path machinery a `ClipPath` push
            // uses: the polygon below approximates the rounded outline, and
            // `push_path_clip` turns it into a bounding clip plus a coverage mask
            // (G-015). The sample count trades polygon vertices for mask bytes;
            // six segments per corner keeps the quarter arc within half a pixel
            // of the true circle at any radius a UI uses.
            let rect = crate::core::Rect::new(*x, *y, *width, *height);
            let outline = rounded_rect_outline_points(rect, *radii);
            surface.push_path_clip(&outline);
        }
        RenderCommand::PopClip => surface.pop_clip(),
        RenderCommand::PushLayer { bounds, effect } => push_layer(surface, *bounds, *effect),
        RenderCommand::PushLayerGroup { bounds, effect, opacity, cache_key } => {
            push_layer_group(surface, *bounds, *effect, *opacity, *cache_key)
        }
        RenderCommand::PopLayer => pop_layer(surface),
        RenderCommand::PushMatte { bounds, mode } => push_matte(surface, *bounds, *mode),
        RenderCommand::PopMatte => pop_matte(surface),
        RenderCommand::EndMatte => end_matte(surface),
        RenderCommand::DrawGradient { rect, gradient } => {
            surface.fill_rect_gradient(*rect, gradient);
        }
        RenderCommand::DrawArc { center, radius, start_angle, end_angle, color, filled } => {
            surface.draw_arc(*center, *radius, *start_angle, *end_angle, *color, *filled);
        }
        RenderCommand::DrawPath { points, closed, color, filled, width } => {
            surface.draw_path(points, *closed, *color, *filled, *width);
        }
        RenderCommand::DrawPathDashed { points, closed, color, width, dash } => {
            // One dash kernel, shared with the border's dashed edges (G-014): the
            // polyline is expanded here, on the immediate path, into the same
            // `DrawLineStroke` primitives a solid stroke uses.
            for (from, to) in
                crate::render::backend::surface::dash_line_segments(points, *closed, *dash)
            {
                surface.draw_line_with_width(from, to, *color, *width);
            }
        }
        RenderCommand::FillPathNonZero { path, color } => {
            surface.fill_path_non_zero(path, *color);
        }
        RenderCommand::BoxShadow { rect, color, offset_x, offset_y, blur_radius, spread } => {
            // Render shadow rect with offset and optional spread. Every term is `i32` and the
            // operands come from a public command, so the arithmetic is saturating: a wrapped
            // `spread` used to flip a large shadow into a tiny one (or a negative width that
            // `.max(0)` silently collapsed), which is a wrong picture rather than a rejected one.
            let spread_rect = crate::core::Rect::new(
                sat_add_i32(sat_add_i32(rect.x, *offset_x), sat_neg_i32(*spread)),
                sat_add_i32(sat_add_i32(rect.y, *offset_y), sat_neg_i32(*spread)),
                sat_mul_i32(*spread, 2).saturating_add(u32_to_i32_saturating(rect.width)).max(0)
                    as u32,
                sat_mul_i32(*spread, 2).saturating_add(u32_to_i32_saturating(rect.height)).max(0)
                    as u32,
            );
            let shadow_color = *color;
            surface.fill_rect(spread_rect, shadow_color);
            // Apply box blur to the shadow region if blur_radius > 0
            if *blur_radius > 0 {
                blur_rect_on_surface(surface, spread_rect, *blur_radius);
            }
        }
        RenderCommand::BoxShadowRounded {
            rect,
            radii,
            color,
            offset_x,
            offset_y,
            blur_radius,
            spread,
        } => {
            // The rounded twin of the `BoxShadow` arm above: the same offset/spread
            // arithmetic, but the blur source is the rounded rectangle (G-016), so
            // the shadow's corners follow the caster's instead of staying square.
            // The radii are resolved per CSS `border-radius` first, matching the
            // filled rounded rect the caster itself paints through.
            let spread_rect = crate::core::Rect::new(
                sat_add_i32(sat_add_i32(rect.x, *offset_x), sat_neg_i32(*spread)),
                sat_add_i32(sat_add_i32(rect.y, *offset_y), sat_neg_i32(*spread)),
                sat_mul_i32(*spread, 2).saturating_add(u32_to_i32_saturating(rect.width)).max(0)
                    as u32,
                sat_mul_i32(*spread, 2).saturating_add(u32_to_i32_saturating(rect.height)).max(0)
                    as u32,
            );
            let shadow_color = *color;
            surface.fill_rounded_rect_radii(
                spread_rect,
                crate::render::pipeline::rounded_rect_effective_radii(spread_rect, *radii),
                shadow_color,
            );
            if *blur_radius > 0 {
                blur_rect_on_surface(surface, spread_rect, *blur_radius);
            }
        }
        RenderCommand::Blur { radius } => {
            let r = (*radius).min(100) as usize;
            if r == 0 {
                return;
            }
            let size = surface.size();
            let w = size.width as usize;
            let h = size.height as usize;
            if w == 0 || h == 0 {
                return;
            }
            // A blur is expensive per pixel, so run it only over the clip that can publish the
            // result; the whole surface remains the no-clip case (G-03).
            let (bx, by, bw, bh) = match surface.current_clip() {
                Some((clip_x, clip_y, clip_w, clip_h)) => {
                    let x0 = clip_x.max(0) as usize;
                    let y0 = clip_y.max(0) as usize;
                    let x1 = (clip_x as i64 + i64::from(clip_w)).clamp(0, w as i64) as usize;
                    let y1 = (clip_y as i64 + i64::from(clip_h)).clamp(0, h as i64) as usize;
                    (
                        x0.min(w),
                        y0.min(h),
                        x1.saturating_sub(x0.min(w)),
                        y1.saturating_sub(y0.min(h)),
                    )
                }
                None => (0, 0, w, h),
            };
            let mut scratch = surface.take_blur_scratch();
            {
                let back = surface.buffer.back_mut();
                box_blur_region(back, w, h, (bx, by, bw, bh), r, &mut scratch);
            }
            surface.return_blur_scratch(scratch);
        }
        RenderCommand::ClipPath { points } => {
            // Exact path clipping: the surface restricts drawing to the path's bounds and records a
            // coverage mask plus the backdrop it covered. The mask is applied when the matching
            // `PopClip` arrives, so every primitive between the two — geometry, glyphs, images,
            // blurs — is clipped by the path rather than by its bounding box. A degenerate path
            // pushes an empty region, which draws nothing instead of silently not clipping.
            surface.push_path_clip(points);
        }
        RenderCommand::SetBlendMode { mode } => {
            // Frame state, like the clip stack: it applies to everything drawn until it changes.
            // The writes it governs each go through the surface's blend-aware pixel path.
            surface.set_blend_mode(*mode);
        }
        RenderCommand::DrawConicGradient { center, start_angle, stops } => {
            if stops.is_empty() {
                return;
            }
            let size = surface.size();
            let w = size.width;
            let h = size.height;
            if w == 0 || h == 0 {
                return;
            }
            // Restrict the loop to the clip instead of evaluating the whole surface. The old loop
            // wrote every pixel unconditionally, so a conic fill inside a small clip still paid
            // `atan2` plus a stop search over the full frame (G-03).
            let (x0, y0, x1, y1) = match surface.current_clip() {
                Some((clip_x, clip_y, clip_w, clip_h)) => {
                    let x0 = clip_x.max(0) as u32;
                    let y0 = clip_y.max(0) as u32;
                    let x1 = (clip_x as i64 + i64::from(clip_w)).clamp(0, w as i64) as u32;
                    let y1 = (clip_y as i64 + i64::from(clip_h)).clamp(0, h as i64) as u32;
                    (x0.min(w), y0.min(h), x1, y1)
                }
                None => (0, 0, w, h),
            };
            if x1 <= x0 || y1 <= y0 {
                return;
            }
            // One content-cached ramp and one kernel, both shared with `fill_rect_gradient`'s
            // conic branch, so the two software paths cannot drift (G-203). The command arm keeps
            // its historical verbatim write; `fill_rect_gradient` keeps the blend-aware one.
            let lut = ramp_lut_for_stops(stops);
            fill_conic_ramp(
                surface.buffer.back_mut(),
                size.width,
                (x0, y0, x1, y1),
                *center,
                *start_angle,
                &lut,
                ConicWrite::Replace,
            );
        }
    }
}

/// How many straight chords approximate one quarter arc of a rounded clip (G-015).
///
/// Six chords keep the sampled polygon within half a pixel of the true quarter
/// circle for any radius a UI clip uses, while costing only 24 extra vertices
/// per clip; the coverage mask those vertices produce is what actually clips.
const ROUNDED_CLIP_CORNER_SEGMENTS: usize = 6;

/// Runs the box blur the shadow commands share over `region` (G-03/G-016).
///
/// Extracted from the `BoxShadow` arm so [`RenderCommand::BoxShadow`] and
/// [`RenderCommand::BoxShadowRounded`] cannot drift on how the blur region is
/// clipped: both fill their own source shape and then hand it to this one
/// routine, which intersects the region with the active clip and the surface
/// bounds exactly as the inline block always did.
fn blur_rect_on_surface(surface: &mut SoftwareSurface, region: Rect, blur_radius: u32) {
    let size = surface.size();
    let w = size.width as usize;
    let h = size.height as usize;
    if w == 0 || h == 0 {
        return;
    }
    let radius = blur_radius.min(100) as usize;
    let clip = surface.current_clip().unwrap_or((0, 0, w as u32, h as u32));
    let blur_x0 = region.x.max(clip.0).max(0) as usize;
    let blur_y0 = region.y.max(clip.1).max(0) as usize;
    let clip_right = clip.0 as i64 + i64::from(clip.2);
    let clip_bottom = clip.1 as i64 + i64::from(clip.3);
    let blur_right =
        (region.x as i64 + i64::from(region.width)).min(clip_right).min(w as i64).max(0);
    let blur_bottom =
        (region.y as i64 + i64::from(region.height)).min(clip_bottom).min(h as i64).max(0);
    let blur_w = (blur_right - blur_x0 as i64).max(0) as usize;
    let blur_h = (blur_bottom - blur_y0 as i64).max(0) as usize;
    if blur_w > 0 && blur_h > 0 {
        let mut scratch = surface.take_blur_scratch();
        {
            let back = surface.buffer.back_mut();
            box_blur_region(back, w, h, (blur_x0, blur_y0, blur_w, blur_h), radius, &mut scratch);
        }
        surface.return_blur_scratch(scratch);
    }
}

/// The closed outline of a rounded rectangle as a polygon of sample points.
///
/// Each corner is walked with [`ROUNDED_CLIP_CORNER_SEGMENTS`] straight chords,
/// starting and ending on the tangent points where the arc meets the edges.
/// Feeding this to [`SoftwareSurface::push_path_clip`] gives the clip the same
/// coverage machinery a `ClipPath` command gets (G-015); the chords cut inside
/// the true arc by at most half a pixel at the sample count chosen, which is
/// the accuracy a UI clip needs.
///
/// The radii are resolved per CSS `border-radius` first, so an edge-overrun
/// cannot produce a self-intersecting polygon — the same guarantee a filled
/// rounded rect's coverage path has.
fn rounded_rect_outline_points(rect: Rect, radii: [u32; 4]) -> Vec<crate::core::Point> {
    use crate::core::Point;
    use crate::render::pipeline::rounded_rect_effective_radii;
    let [tl, tr, br, bl] = rounded_rect_effective_radii(rect, radii);
    let left = rect.x;
    let top = rect.y;
    let right = rect.x + rect.width as i32;
    let bottom = rect.y + rect.height as i32;
    let segments = ROUNDED_CLIP_CORNER_SEGMENTS;
    let mut points = Vec::with_capacity(4 * segments + 4);
    // Perimeter order (top-left → top-right → bottom-right → bottom-left), the
    // order every other outline the command set carries uses.
    let arc = |points: &mut Vec<Point>, cx: i32, cy: i32, radius: u32, start: f32| {
        if radius == 0 {
            points.push(Point::new(cx, cy));
            return;
        }
        let radius = radius as f32;
        for step in 0..=segments {
            let angle = start + core::f32::consts::FRAC_PI_2 * (step as f32 / segments as f32);
            points.push(Point::new(
                cx + (radius * angle.cos()).round() as i32,
                cy + (radius * angle.sin()).round() as i32,
            ));
        }
    };
    // Top edge runs left to right, so the top-left arc sweeps from 180° to 270°.
    arc(&mut points, left + tl as i32, top + tl as i32, tl, core::f32::consts::PI);
    // The top-right arc starts where the top edge ends: 270° (its tangent
    // point) through 360°.
    arc(&mut points, right - tr as i32, top + tr as i32, tr, -core::f32::consts::FRAC_PI_2);
    // Bottom-right: 0° through 90°.
    arc(&mut points, right - br as i32, bottom - br as i32, br, 0.0);
    // Bottom-left: 90° through 180°.
    arc(&mut points, left + bl as i32, bottom - bl as i32, bl, core::f32::consts::FRAC_PI_2);
    points
}

/// Draws `text` and then the decorations `decoration` asks for (G-013).
///
/// The glyphs go down through the same [`SoftwareSurface::draw_text`] the plain
/// command routes to, so the ink is identical; the decoration rules are then
/// laid out around the shaped run with these concrete formulas, documented here
/// because they are new rendering geometry:
///
/// * the baseline sits `metrics.ascent` below `origin.y`, where `metrics` is the
///   shared [`TextMetrics::for_font`] line box at the surface's DPI scale — the
///   same numbers [`SoftwareSurface::draw_text`] rasterises against;
/// * the underline runs `underline_offset = max(1, descent / 2)` below the
///   baseline, centring the rule in the space between baseline and line-box
///   bottom;
/// * the strikethrough runs at `origin.y + ascent / 2`, the middle of the
///   ascender region where cap height and x-height both cross;
/// * the wavy underline shares the underline's offset and zigzags between
///   one and two offsets below it, with a two-pixel half period;
/// * every rule's thickness is `max(1, round(em / 12))` device pixels (em =
///   `font.size() * scale`), so it grows with the font instead of vanishing at
///   display sizes;
/// * every rule spans the run's own width, adjusted for `alignment` exactly as
///   the glyphs were.
fn draw_text_decorated_on_surface(
    surface: &mut SoftwareSurface,
    origin: Point,
    text: &str,
    font: &Font,
    color: Color,
    alignment: HorizontalAlignment,
    decoration: crate::render::core::command::TextDecoration,
) {
    use crate::render::core::command::TextDecoration;
    surface.draw_text(origin, text, font, color, alignment);
    if decoration.is_empty() {
        return;
    }
    let scale = surface.dpi_scale();
    let metrics = TextMetrics::for_font(font, scale);
    let advance = surface.shape_text(text, font).advance().round().max(0.0) as u32;
    if advance == 0 {
        return;
    }
    let start_x = match alignment {
        HorizontalAlignment::Left => origin.x,
        HorizontalAlignment::Center => origin.x - (advance / 2) as i32,
        HorizontalAlignment::Right => origin.x - advance as i32,
    };
    let end_x = start_x + advance as i32;
    let baseline = origin.y + metrics.ascent as i32;
    let thickness = ((font.size() * scale) / 12.0).round().max(1.0) as u32;
    let underline_offset = (metrics.descent as i32 / 2).max(1);
    let rule = |surface: &mut SoftwareSurface, y: i32| {
        surface.draw_line_with_width(
            Point::new(start_x, y),
            Point::new(end_x, y),
            color,
            thickness,
        );
    };
    if decoration.contains(TextDecoration::UNDERLINE.0) {
        rule(surface, baseline + underline_offset);
    }
    if decoration.contains(TextDecoration::STRIKETHROUGH.0) {
        rule(surface, origin.y + (metrics.ascent / 2) as i32);
    }
    if decoration.contains(TextDecoration::WAVY.0) {
        // A triangle wave: half period 2 px, amplitude from one to two offsets
        // below the baseline, so the crest never climbs onto the descenders.
        let crest = baseline + underline_offset;
        let trough = baseline + underline_offset * 2;
        let mut x = start_x;
        let mut low = false;
        while x < end_x {
            let next = (x + 2).min(end_x);
            let from_y = if low { trough } else { crest };
            let to_y = if low { crest } else { trough };
            surface.draw_line_with_width(
                Point::new(x, from_y),
                Point::new(next, to_y),
                color,
                thickness,
            );
            x = next;
            low = !low;
        }
    }
}

/// Begin a layered region: capture its backdrop, clear it, and clip the foreground to it.
///
/// # Why the region is cleared
///
/// The foreground and the backdrop have to be separable at pop, and the only exact way to
/// separate two source-over composites is to draw them one at a time. Clearing the region
/// means everything drawn between `PushLayer` and `PopLayer` is foreground with its own
/// alpha, which the pop can then composite over the processed backdrop. Leaving the backdrop
/// in place would make the foreground an opaque mixture of the two, and no amount of blurring
/// could recover the layer's own alpha.
///
/// # Why the clip is pushed before the capture
///
/// The layer may only paint inside `bounds`, and the capture must cover exactly the pixels
/// the layer can touch. Reading `current_clip()` after the push gives the intersection of
/// `bounds` with the caller's own clip, so both operations agree about the region by
/// construction.
fn push_layer(
    surface: &mut SoftwareSurface,
    bounds: Rect,
    effect: Option<crate::render::core::command::LayerEffect>,
) {
    begin_layer(surface, bounds, effect, 1.0, None);
}

/// Begins a layer with a subtree opacity and an optional replay cache.
///
/// See [`RenderCommand::PushLayerGroup`]: the opacity is applied to the layer's foreground as a
/// whole, and the cache key is a caller assertion that the enclosed commands are unchanged.
fn push_layer_group(
    surface: &mut SoftwareSurface,
    bounds: Rect,
    effect: Option<crate::render::core::command::LayerEffect>,
    opacity: f32,
    cache_key: Option<u64>,
) {
    begin_layer(surface, bounds, effect, opacity, cache_key);
}

/// The shared implementation of both layer push forms.
fn begin_layer(
    surface: &mut SoftwareSurface,
    bounds: Rect,
    effect: Option<crate::render::core::command::LayerEffect>,
    opacity: f32,
    cache_key: Option<u64>,
) {
    let opacity = if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { 1.0 };
    surface.push_clip(bounds.x, bounds.y, bounds.width, bounds.height);
    let size = surface.size();
    let Some(rect) = clipped_region(surface.current_clip(), size) else {
        surface.buffer.layers.push(super::surface::LayerCapture {
            rect: (0, 0, 0, 0),
            effect,
            backdrop: crate::compat::Vec::new(),
            isolated: false,
            opacity,
            cache_key,
            cached_foreground: None,
        });
        return;
    };
    // A plain layer with no effect and full opacity draws straight into the surface, so its pop
    // has nothing to separate. Opacity or a cache key both need the foreground on its own.
    let isolated = effect.is_some() || opacity < 1.0 || cache_key.is_some();
    let cached_foreground = match cache_key {
        Some(key) => {
            let cursor = surface.buffer.layer_cache_cursor;
            let cache = &mut surface.buffer.layer_cache;
            // Touch on hit: move the entry to the newest slot so eviction follows *use* order. The
            // old list was evicted from the front in insertion order, so a hot layer could be
            // pushed out by a cold one registered later (G-13). A swap keeps the cost bounded;
            // it is ring-relative once the cache is full (G-05).
            let position = cache.iter().position(|(cached_key, entry)| {
                *cached_key == key && entry.rect == rect && entry.opacity == opacity
            });
            position.map(|position| {
                let newest = newest_cache_slot(cache.len(), cursor);
                if position != newest {
                    cache.swap(position, newest);
                }
                Arc::clone(&cache[newest].1.foreground)
            })
        }
        None => None,
    };
    let backdrop = if isolated {
        // Capture and clear in one sweep: the previous form read the region out and then made a
        // second pass to zero it, while a reusable buffer removes the per-layer allocation (G-06).
        let mut buffer = surface.take_layer_backdrop();
        capture_and_clear_region_into(
            surface.buffer.back_mut(),
            size.width as usize,
            rect,
            &mut buffer,
        );
        buffer
    } else {
        crate::compat::Vec::new()
    };
    if cached_foreground.is_some() {
        // The caller asserted the enclosed commands are unchanged; discard them and reuse the
        // previous frame's foreground.
        surface.buffer.suppress_depth += 1;
    }
    surface.buffer.layers.push(super::surface::LayerCapture {
        rect,
        effect,
        backdrop,
        isolated,
        opacity,
        cache_key,
        cached_foreground,
    });
}

/// Begins a track matte: captures the backdrop and clears the region for the matte drawing.
///
/// The clip stack is used exactly as [`begin_layer`] uses it, so the captured region is the
/// intersection of `bounds` with the caller's own clip and the matching [`end_matte`] releases
/// that same clip. A matte is three phases rather than two: the commands after this push draw
/// the **matte source**, [`pop_matte`] converts that drawing into a coverage mask, and the
/// commands after the pop draw the **matted layer**, which [`end_matte`] composites through the
/// mask.
fn push_matte(surface: &mut SoftwareSurface, bounds: Rect, mode: MatteMode) {
    surface.push_clip(bounds.x, bounds.y, bounds.width, bounds.height);
    let size = surface.size();
    let Some(rect) = clipped_region(surface.current_clip(), size) else {
        surface.buffer.mattes.push(super::surface::MatteCapture {
            rect: (0, 0, 0, 0),
            backdrop: Vec::new(),
            mode,
            mask: Vec::new(),
        });
        return;
    };
    // Capture and clear in one sweep, into a pooled buffer (G-06).
    let mut backdrop = surface.take_layer_backdrop();
    capture_and_clear_region_into(
        surface.buffer.back_mut(),
        size.width as usize,
        rect,
        &mut backdrop,
    );
    surface.buffer.mattes.push(super::surface::MatteCapture {
        rect,
        backdrop,
        mode,
        mask: Vec::new(),
    });
}

/// Converts the drawing since [`push_matte`] into coverage, then clears the region for the
/// matted layer.
fn pop_matte(surface: &mut SoftwareSurface) {
    let Some(mut capture) = surface.buffer.mattes.pop() else { return };
    let (_, _, width, height) = capture.rect;
    if width == 0 || height == 0 {
        surface.buffer.mattes.push(capture);
        return;
    }
    let surface_width = surface.size().width as usize;
    let mut drawn = surface.take_layer_foreground_scratch();
    // The matte source has served its purpose; the matted layer starts from a clean region, and
    // both happen in one read-and-zero sweep (G-06).
    capture_and_clear_region_into(
        surface.buffer.back_mut(),
        surface_width,
        capture.rect,
        &mut drawn,
    );
    capture.mask = matte_coverage(&drawn, capture.mode);
    surface.return_layer_foreground_scratch(drawn);
    surface.buffer.mattes.push(capture);
}

/// Composites the matted layer through the captured coverage and restores the backdrop.
fn end_matte(surface: &mut SoftwareSurface) {
    let Some(capture) = surface.buffer.mattes.pop() else { return };
    surface.pop_clip();
    let (_, _, width, height) = capture.rect;
    if width == 0 || height == 0 {
        surface.return_layer_backdrop(capture.backdrop);
        return;
    }
    let pixel_count = width as usize * height as usize;
    if capture.backdrop.len() != pixel_count * 4 || capture.mask.len() != pixel_count {
        surface.return_layer_backdrop(capture.backdrop);
        return;
    }
    let surface_width = surface.size().width as usize;
    let mut matted = surface.take_layer_foreground_scratch();
    capture_region_into(surface.buffer.back_mut(), surface_width, capture.rect, &mut matted);
    for (index, pixel) in matted.chunks_exact_mut(4).enumerate() {
        let coverage = capture.mask[index] as u32;
        pixel[3] = ((pixel[3] as u32 * coverage) / 255) as u8;
    }
    composite_foreground(
        surface.buffer.back_mut(),
        surface_width,
        capture.rect,
        &capture.backdrop,
        &matted,
        1.0,
    );
    surface.return_layer_foreground_scratch(matted);
    surface.return_layer_backdrop(capture.backdrop);
}

/// Per-pixel coverage for a captured matte drawing.
///
/// `Alpha` is the source's own alpha; `Luma` is its luminance times its alpha, so a transparent
/// pixel contributes no coverage regardless of its colour. The inverted modes complement the
/// result, which is what a Lottie `tt` layer with the invert flag asks for.
fn matte_coverage(source: &[u8], mode: MatteMode) -> Vec<u8> {
    let mut mask = Vec::with_capacity(source.len() / 4);
    for pixel in source.chunks_exact(4) {
        let alpha = pixel[3] as u32;
        // Rec. 709 luminance, scaled by 10000 to stay in integer arithmetic.
        let luma =
            (2126 * pixel[0] as u32 + 7152 * pixel[1] as u32 + 722 * pixel[2] as u32) / 10_000;
        let coverage = match mode {
            MatteMode::Alpha => alpha,
            MatteMode::AlphaInverted => 255 - alpha,
            MatteMode::Luma => luma * alpha / 255,
            MatteMode::LumaInverted => 255 - luma * alpha / 255,
        };
        mask.push(coverage.min(255) as u8);
    }
    mask
}

/// How many cached layer foregrounds a surface keeps.
///
/// A cache key is caller-defined and a screen has a bounded number of independently invalidated
/// subtrees, so a small fixed ceiling is enough. When it is reached the oldest entry is dropped,
/// which turns the next use of that key into a redraw rather than an unbounded memory growth.
const LAYER_CACHE_CAPACITY: usize = 64;

/// End the innermost layer, apply its effect to the captured backdrop, and composite the
/// foreground over the result.
///
/// An unmatched pop is ignored on purpose: the clip stack belongs to the caller too, and
/// popping it for a layer that was never pushed would widen or narrow an unrelated clip.
///
/// The captured region is blurred as its own image, so a pixel on the region's boundary clamps
/// to that boundary rather than sampling the surface outside it. That is the same edge rule
/// the full-surface `Blur` command follows, and it keeps the layer independent of pixels the
/// material does not cover.
fn pop_layer(surface: &mut SoftwareSurface) {
    if surface.buffer.layers.is_empty() {
        return;
    }
    let Some(layer) = surface.buffer.layers.pop() else {
        return;
    };
    surface.pop_clip();
    if !layer.isolated {
        return;
    }
    let (_, _, width, height) = layer.rect;
    if width == 0 || height == 0 {
        return;
    }
    let expected = width as usize * height as usize * 4;
    let surface_width = surface.size().width as usize;
    let was_cached = layer.cached_foreground.is_some();
    let mut cached = layer.cached_foreground.filter(|foreground| foreground.len() == expected);
    // Capture into the reusable foreground slot. The `Arc` is built only when the bytes are going
    // into the cache, so an uncached layer never allocates one (G-06).
    let mut scratch = Vec::new();
    let mut scratch_owned = false;
    if cached.is_none() {
        let mut buffer = surface.take_layer_foreground_scratch();
        capture_region_into(surface.buffer.back_mut(), surface_width, layer.rect, &mut buffer);
        if let (Some(key), false) = (layer.cache_key, was_cached) {
            let shared = Arc::new(buffer);
            store_cached_layer(surface, key, layer.rect, layer.opacity, Arc::clone(&shared));
            cached = Some(shared);
        } else {
            scratch = buffer;
            scratch_owned = true;
        }
    }
    let foreground: &[u8] = match &cached {
        Some(foreground) => foreground.as_slice(),
        None => scratch.as_slice(),
    };
    let mut backdrop = layer.backdrop;
    // A fully transparent subtree shows neither its foreground nor its backdrop effect, but the
    // backdrop itself must come back: the layer cleared the region at push, and leaving it cleared
    // would punch a transparent hole through everything behind the group.
    if layer.opacity <= 0.0 {
        if backdrop.len() == expected {
            restore_region(surface.buffer.back_mut(), surface_width, layer.rect, &backdrop);
        }
        surface.return_layer_backdrop(backdrop);
        if scratch_owned {
            surface.return_layer_foreground_scratch(scratch);
        }
        return;
    }
    if backdrop.len() != expected {
        surface.return_layer_backdrop(backdrop);
        if scratch_owned {
            surface.return_layer_foreground_scratch(scratch);
        }
        return;
    }
    if let Some(effect) = layer.effect {
        // The same clamp `RenderCommand::Blur` applies: a radius beyond the kernel's useful reach
        // only costs time, and the command's range is already a public, untrusted input.
        let radius = effect.radius().min(100) as usize;
        if radius > 0 {
            let mut blur = surface.take_blur_scratch();
            box_blur_region(
                &mut backdrop,
                width as usize,
                height as usize,
                (0, 0, width as usize, height as usize),
                radius,
                &mut blur,
            );
            surface.return_blur_scratch(blur);
        }
        if let Some((tint, weight)) = effect.tint() {
            tint_region(&mut backdrop, tint, weight);
        }
    }
    composite_foreground(
        surface.buffer.back_mut(),
        surface_width,
        layer.rect,
        &backdrop,
        foreground,
        layer.opacity,
    );
    surface.return_layer_backdrop(backdrop);
    if scratch_owned {
        surface.return_layer_foreground_scratch(scratch);
    }
}

/// Physical slot of the newest entry in `layer_cache` (G-05).
///
/// While the cache is still growing the physical order is the use order, so the newest entry is
/// last. Once full the vector is a ring whose oldest slot is `layer_cache_cursor`, which makes the
/// slot before the cursor the newest.
fn newest_cache_slot(len: usize, cursor: usize) -> usize {
    if len < LAYER_CACHE_CAPACITY {
        len.saturating_sub(1)
    } else {
        (cursor + len - 1) % LAYER_CACHE_CAPACITY
    }
}

/// Inserts or replaces one cache entry, dropping the oldest when the cache is full.
fn store_cached_layer(
    surface: &mut SoftwareSurface,
    key: u64,
    rect: (i32, i32, u32, u32),
    opacity: f32,
    foreground: Arc<Vec<u8>>,
) {
    let capacity = LAYER_CACHE_CAPACITY;
    let cursor = surface.buffer.layer_cache_cursor;
    let entry = super::surface::CachedLayer { rect, opacity, foreground };
    let next_cursor = {
        let cache = &mut surface.buffer.layer_cache;
        if let Some(position) = cache.iter().position(|(cached_key, _)| *cached_key == key) {
            // A refreshed entry becomes the most recently used one.
            let newest = newest_cache_slot(cache.len(), cursor);
            if position != newest {
                cache.swap(position, newest);
            }
            cache[newest].1 = entry;
            cursor
        } else if cache.len() < capacity {
            cache.push((key, entry));
            cursor
        } else {
            // Full: overwrite the oldest slot in place instead of shifting every entry down. The
            // written slot becomes the newest and the cursor advances to the next-oldest (G-05).
            cache[cursor] = (key, entry);
            (cursor + 1) % capacity
        }
    };
    surface.buffer.layer_cache_cursor = next_cursor;
}

/// The part of `clip` that lies inside `size`, or `None` when there is no visible part.
///
/// The arithmetic is done in `i64`: both a negative origin and a `u32::MAX` extent are legal
/// command inputs, and adding them in `i32` would wrap the right/bottom edge into the region.
fn clipped_region(clip: Option<(i32, i32, u32, u32)>, size: Size) -> Option<(i32, i32, u32, u32)> {
    let (x, y, width, height) = clip?;
    let x0 = x.max(0) as i64;
    let y0 = y.max(0) as i64;
    let x1 = (x as i64 + width as i64).min(size.width as i64);
    let y1 = (y as i64 + height as i64).min(size.height as i64);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some((x0 as i32, y0 as i32, (x1 - x0) as u32, (y1 - y0) as u32))
}

/// Copies a clipped region out of `back` into a tightly packed RGBA buffer.
///
/// Exposed at crate visibility so `pipeline::containers` can reuse this implementation instead of
/// keeping a second copy (G-201); the destination-owning form below is what the paint paths use.
pub(crate) fn capture_region(
    back: &[u8],
    surface_width: usize,
    rect: (i32, i32, u32, u32),
) -> crate::compat::Vec<u8> {
    let mut out = crate::compat::Vec::new();
    capture_region_into(back, surface_width, rect, &mut out);
    out
}

/// Copies a clipped region out of `back` into `out`, reusing its allocation (G-06).
fn capture_region_into(
    back: &[u8],
    surface_width: usize,
    rect: (i32, i32, u32, u32),
    out: &mut crate::compat::Vec<u8>,
) {
    let (x, y, width, height) = rect;
    let row_bytes = width as usize * 4;
    out.resize(row_bytes * height as usize, 0);
    for row in 0..height as usize {
        let src = (y as usize + row) * surface_width * 4 + x as usize * 4;
        let dst = row * row_bytes;
        if let (Some(source), Some(target)) =
            (back.get(src..src + row_bytes), out.get_mut(dst..dst + row_bytes))
        {
            target.copy_from_slice(source);
        } else if let Some(target) = out.get_mut(dst..dst + row_bytes) {
            // A recycled buffer holds the previous capture; a row the source cannot supply must
            // read as transparent black, exactly as the old zero-initialised buffer did.
            target.fill(0);
        }
    }
}

/// Copies a clipped region out of `back` and zeroes it in the same sweep (G-06).
///
/// A capture followed by a clear reads and then writes every row. Both operations touch exactly
/// the same bytes, so folding them into one loop halves the passes over the region.
fn capture_and_clear_region_into(
    back: &mut [u8],
    surface_width: usize,
    rect: (i32, i32, u32, u32),
    out: &mut crate::compat::Vec<u8>,
) {
    let (x, y, width, height) = rect;
    let row_bytes = width as usize * 4;
    out.resize(row_bytes * height as usize, 0);
    for row in 0..height as usize {
        let src = (y as usize + row) * surface_width * 4 + x as usize * 4;
        let dst = row * row_bytes;
        if let Some(source) = back.get_mut(src..src + row_bytes) {
            if let Some(target) = out.get_mut(dst..dst + row_bytes) {
                target.copy_from_slice(source);
            }
            source.fill(0);
        } else if let Some(target) = out.get_mut(dst..dst + row_bytes) {
            // See `capture_region_into`: a row the source cannot supply is transparent black.
            target.fill(0);
        }
    }
}

/// Overwrites a clipped region of `back` with a tightly packed RGBA buffer.
fn restore_region(
    back: &mut [u8],
    surface_width: usize,
    rect: (i32, i32, u32, u32),
    source: &[u8],
) {
    let (x, y, width, height) = rect;
    let row_bytes = width as usize * 4;
    for row in 0..height as usize {
        let dst = (y as usize + row) * surface_width * 4 + x as usize * 4;
        let src = row * row_bytes;
        if let (Some(target), Some(pixels)) =
            (back.get_mut(dst..dst + row_bytes), source.get(src..src + row_bytes))
        {
            target.copy_from_slice(pixels);
        }
    }
}

/// Composites a layer's foreground over `backdrop` and writes the result into the region.
///
/// The foreground is passed in rather than read back out of `back` so a cached foreground (from a
/// previous frame) and a freshly drawn one go through exactly the same composition. `opacity`
/// scales the foreground's own alpha, which is what makes a subtree fade as one unit: a child's
/// antialiased edge and its semi-transparent background both keep their relative weights.
fn composite_foreground(
    back: &mut [u8],
    surface_width: usize,
    rect: (i32, i32, u32, u32),
    backdrop: &[u8],
    foreground: &[u8],
    opacity: f32,
) {
    restore_region(back, surface_width, rect, backdrop);
    let (x, y, width, height) = rect;
    let opacity = if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { 1.0 };
    for row in 0..height as usize {
        for column in 0..width as usize {
            let index = (row * width as usize + column) * 4;
            let Some(pixel) = foreground.get(index..index + 4) else { continue };
            if pixel[3] == 0 {
                continue;
            }
            let alpha = (pixel[3] as f32 * opacity).round().clamp(0.0, 255.0) as u8;
            if alpha == 0 {
                continue;
            }
            let color = Color::rgba(pixel[0], pixel[1], pixel[2], alpha);
            blend_pixel_with_mode(
                back,
                surface_width as u32,
                x.max(0) as u32 + column as u32,
                y.max(0) as u32 + row as u32,
                color,
                1.0,
                BlendMode::Normal,
            );
        }
    }
}

/// Blends `tint` over every pixel of a tightly packed region.
///
/// `weight` scales the tint's own alpha, so a tint declared with `a = 128` at weight `0.5`
/// contributes half of half opacity. A non-finite weight is treated as no tint: `clamp`
/// propagates `NaN`, which would poison every channel it touched.
#[allow(clippy::chunks_exact_to_as_chunks)]
fn tint_region(region: &mut [u8], tint: Color, weight: f32) {
    if !weight.is_finite() {
        return;
    }
    let weight = weight.clamp(0.0, 1.0);
    if weight <= 0.0 {
        return;
    }
    let tint_alpha = (tint.a as f32 / 255.0) * weight;
    if tint_alpha <= 0.0 {
        return;
    }
    let tint_rgb = [tint.r as f32 / 255.0, tint.g as f32 / 255.0, tint.b as f32 / 255.0];
    for pixel in region.chunks_exact_mut(4) {
        let dst_alpha = pixel[3] as f32 / 255.0;
        let out_alpha = tint_alpha + dst_alpha * (1.0 - tint_alpha);
        if out_alpha <= f32::EPSILON {
            pixel.fill(0);
            continue;
        }
        for channel in 0..3 {
            let dst = pixel[channel] as f32 / 255.0;
            let blended =
                (tint_rgb[channel] * tint_alpha + dst * dst_alpha * (1.0 - tint_alpha)) / out_alpha;
            pixel[channel] = (blended * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        pixel[3] = (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }
}

/// Apply a **Gaussian-approximating** blur to a rectangular region of an RGBA pixel buffer.
///
/// # Why a box blur is not what the command asks for
///
/// A `BoxShadow` declares a `blur_radius`, and the only blur a Gaussian has is a *standard
/// deviation*. One pass of a box blur is not that: it produces a flat plateau with hard corners, so
/// a shadow reads as a rectangle with a halo rather than as a falloff — the wider the radius, the
/// more visible the plateau. The crate's snapshots never showed it because no control drew a shadow
/// until the surface channel was wired (`render::surface`), which is exactly when this had to be
/// fixed: a shadow that is *drawn* is a shadow whose quality is visible.
///
/// # The approximation, and why it is the right one here
///
/// Three successive box blurs approximate a Gaussian to within a few percent (the central limit
/// theorem, applied deliberately). Ivan Kutskir's box-width formulas give the three widths whose
/// iterated result has the requested standard deviation, and they are computed for the **first**
/// box so that all three differ by at most one pixel — which is what keeps the result symmetric.
///
/// # Why this is also **faster**, not a cost paid for quality
///
/// The naive implementation this replaces summed `2r + 1` pixels per output pixel per pass, i.e.
/// `O(w * h * r)`, and it re-derived each sum from scratch. A box blur is a convolution with a
/// constant kernel, so the sum is a **sliding window**: leave one pixel, enter one pixel. That makes
/// each pass `O(w * h)` *regardless of radius* across all three boxes — so the Gaussian is not
/// merely comparable to the box blur it replaces, it is asymptotically better and the cost does not
/// grow with the blur the caller asks for.
///
/// # Edge handling
///
/// The region is expanded by the total kernel reach in each direction before the passes run, so an
/// edge pixel is blurred against its neighbours rather than against a shortened window (which would
/// darken or lighten the border). The window itself clamps its indices for the outermost pixels, so
/// the expansion only affects *which* pixels are written, never the arithmetic.
fn box_blur_region(
    back: &mut [u8],
    w: usize,
    h: usize,
    region: (usize, usize, usize, usize),
    radius: usize,
    scratch_buffers: &mut super::surface::BoxBlurScratch,
) {
    let (region_x, region_y, region_w, region_h) = region;
    if w == 0 || h == 0 || region_w == 0 || region_h == 0 || radius == 0 {
        return;
    }
    // The three box widths whose iteration approximates `sigma = radius / 2`.
    //
    // The divisor is the convention a Gaussian blur's radius follows: `radius` is the point at
    // which the kernel has decayed to roughly `1 / e^2` of its peak, which is about `2 sigma`. Using
    // `radius` directly as a standard deviation would make every existing shadow roughly four times
    // too soft, so the caller's unit is preserved and only the *shape* is corrected.
    let sigma = (radius as f32) / 2.0;
    let (b0, b1, b2) = box_blur_widths(sigma);
    let reach = b0.div_ceil(2) + b1.div_ceil(2) + b2.div_ceil(2);

    // Expand region by the kernel's total reach in all directions (clamped to surface bounds).
    let ex0 = region_x.saturating_sub(reach);
    let ey0 = region_y.saturating_sub(reach);
    let ex1 = (region_x + region_w + reach).min(w);
    let ey1 = (region_y + region_h + reach).min(h);
    let ew = ex1 - ex0;
    let eh = ey1 - ey0;
    if ew == 0 || eh == 0 {
        return;
    }

    // One scratch row and one scratch column, reused across all three passes.
    //
    // A sliding window does not need a copy of the region — it needs somewhere to stage the values
    // a horizontal pass is about to overwrite while the pass is still reading them (one row, and one
    // column for the vertical pass). These buffers live on the surface and are taken/returned by the
    // caller, so they grow to the largest region once and then cost no allocation at all (G-07).
    let row_len = ew * 4;
    let col_len = eh * 4;
    let copy_len = ew.max(eh) * 4;
    scratch_buffers.row.resize(row_len, 0);
    scratch_buffers.col.resize(col_len, 0);
    scratch_buffers.copy.resize(copy_len, 0);
    let row = &mut scratch_buffers.row[..row_len];
    let col = &mut scratch_buffers.col[..col_len];
    // The untouched row/column being blurred, kept because a pass writes into the same buffer it
    // slides its window over (see `blur_row`).
    let copy = &mut scratch_buffers.copy[..copy_len];

    let stride = w * 4;
    for width in [b0, b1, b2] {
        if width < 2 {
            continue;
        }
        // Horizontal pass: stage each row, blur it, write it back.
        for y in ey0..ey1 {
            let base = y * stride + ex0 * 4;
            row.copy_from_slice(&back[base..base + row_len]);
            copy[..row_len].copy_from_slice(row);
            blur_row(row, ew, width, &copy[..row_len]);
            back[base..base + row_len].copy_from_slice(row);
        }
        // Vertical pass: stage each column, blur it, write it back.
        for x in ex0..ex1 {
            for y in 0..eh {
                let si = (ey0 + y) * stride + x * 4;
                col[y * 4..y * 4 + 4].copy_from_slice(&back[si..si + 4]);
            }
            copy[..col_len].copy_from_slice(&col[..col_len]);
            blur_row(col, eh, width, &copy[..col_len]);
            for y in 0..eh {
                let di = (ey0 + y) * stride + x * 4;
                back[di..di + 4].copy_from_slice(&col[y * 4..y * 4 + 4]);
            }
        }
    }
}

/// The three box widths whose successive application approximates a Gaussian of `sigma`.
///
/// Ivan Kutskir's derivation, which is the standard one for this approximation. Three boxes of
/// widths `wl`, `wl`, `wu` give a variance of `(2 wl^2 + wu^2 - 3) / 12`, so solving for the `wl`
/// that lands nearest the requested `sigma` and taking the next odd width up as `wu` is the whole
/// of it. The two differ by exactly two, which keeps the iterated kernel symmetric — a set like
/// `(2, 5, 2)` would have a different falloff on each side.
///
/// # The per-pass share, which this used to omit
///
/// Variances **add** across independent passes, so the three boxes together must carry `sigma^2`,
/// i.e. each one carries `sigma^2 / 3` and therefore has a width that solves
/// `(w^2 - 1) / 12 = sigma^2 / 3`, i.e. `w = sqrt(12 * (sigma / sqrt(3))^2 + 1)`.
///
/// The width was computed as `sqrt(12 * sigma^2 + 1)` instead — the *whole* variance put into each
/// of the three passes — so the kernel came out `sqrt(3)` times too wide. Measured against the
/// intended standard deviation (the code's own `sigma = radius / 2`):
///
/// | radius | intended | produced | over-blur |
/// |---|---|---|---|
/// | 2 px | 1.0 | 1.83 | 1.83x |
/// | 6 px | 3.0 | 4.83 | 1.61x |
/// | 12 px | 6.0 | 9.83 | 1.64x |
/// | 24 px | 12.0 | 20.83 | 1.74x |
///
/// Every shadow a theme drew was therefore roughly **1.6–1.8x softer than its own elevation said**,
/// which also made `Theme::elevation`'s levels less distinguishable from one another than the ladder
/// intends. With the share applied, the realised variance lands within the integer-width quantisation
/// of the target at every radius (`sigma=3`: 8.00 against 9.00, `sigma=6`: 34.00 against 36.00,
/// `sigma=12`: 140.00 against 144.00).
///
/// Returns `(0, 0, 0)` for a non-positive `sigma`, which the caller reads as "no blur": a zero
/// width is a no-op pass rather than a division by zero.
fn box_blur_widths(sigma: f32) -> (usize, usize, usize) {
    if sigma <= 0.0 {
        return (0, 0, 0);
    }
    // Each of the three passes carries an equal share of the total variance.
    const PASSES: f32 = 3.0;
    let per_pass = sigma / PASSES.sqrt();
    // The width that contributes exactly that share: `variance = (w^2 - 1) / 12`, so
    // `w = sqrt(12 * per_pass^2 + 1)`. Rounded down, then forced odd so the box has a true centre.
    let ideal = (12.0 * per_pass * per_pass + 1.0).sqrt();
    let wl = match ideal.floor() as usize {
        w if w % 2 == 1 => w,
        w => w.saturating_sub(1),
    };
    (wl, wl, wl + 2)
}

/// One box-blur pass over a single row (or column) of interleaved RGBA pixels.
///
/// The window sums are maintained incrementally: each step subtracts the pixel leaving the window
/// and adds the one entering it, so the pass costs one add and one subtract per channel per pixel
/// rather than `2r + 1` of each. Indices are clamped at both ends, which is what makes the edge
/// pixels blur against the image rather than against a shorter (and therefore darker) window.
///
/// # Why the sum reads from ``source`` and not from ``pixels``
///
/// A blur output pixel must be a function of the **input** pixels around it, and this pass writes its
/// output into the same row it reads. Sliding the window over `pixels` would therefore subtract a
/// pixel it had already overwritten, and the error compounds along the row — the *first* output
/// pixel is correct (nothing has been written yet) and every one after it drifts, which is the exact
/// shape of the failure the agreement test caught. `source` is the untouched row, held by the caller;
/// `pixels` is only ever written to.
///
/// `pixels` is the single row/column, `len` its pixel count, `width` the box width in pixels
/// (odd, and at least 2 by the caller's check), `source` the pre-pass copy of `pixels`.
fn blur_row(pixels: &mut [u8], len: usize, width: usize, source: &[u8]) {
    if len == 0 || width < 2 {
        return;
    }
    let half = width / 2;

    // The window for output pixel `x` spans the **clamped** index range `lo..=hi`.
    //
    // Deriving the divisor from the same two bounds the sum comes from is what makes an edge pixel
    // exact rather than approximately right; and `lo`/`hi` are also the two pixels the window
    // exchanges when it slides, so the sum and the slide cannot disagree about where the window is.
    let lo_of = |x: usize| x.saturating_sub(half);
    let hi_of = |x: usize| (x + half).min(len - 1);

    // Signed accumulators.
    //
    // The slide adds one pixel and subtracts another *before* dividing, and the subtracted one can be
    // the brighter of the two — so a `u32` sum underflows on the pixel where the window stops growing
    // and starts sliding, which is every real image. `i64` makes the intermediate honest and the
    // final value is provably in `0..=255` because it is a mean of bytes.
    let mut sums = [0i64; 4];
    // The window for pixel 0, which is `[0, half]` with the upper end clamped.
    for i in 0..=hi_of(0) {
        let idx = i * 4;
        for c in 0..4 {
            sums[c] += source[idx + c] as i64;
        }
    }

    for x in 0..len {
        let count = (hi_of(x) - lo_of(x) + 1) as i64;
        let out = x * 4;
        for c in 0..4 {
            pixels[out + c] = (sums[c] / count) as u8;
        }
        if x + 1 == len {
            break;
        }
        // Slide to the window for `x + 1`.
        //
        // # Why neither side of the slide is unconditional
        //
        // A window resting against an edge does not slide, it **resizes**, and treating it as a slide
        // is wrong in both directions:
        //
        // * Near the **left** edge the window grows: at `x = 0` it is `[0, half]` and at `x = 1` it is
        //   `[0, half + 1]`, so a pixel enters and none leaves. Subtracting `lo_of(x)` there removes a
        //   pixel that is still inside the window.
        // * Near the **right** edge it shrinks: the last pixel enters when `x + 1 + half` reaches
        //   `len - 1`, and after that each step only drops one from the left. Adding `hi_of(x + 1)`
        //   there adds the same pixel a second time.
        //
        // Both errors are *constant* once made rather than cumulative, so they read as a uniform shift
        // and a slightly bright tail rather than as an obvious tear — the reason this survived a naive
        // box-blur-to-box-blur comparison and only fell out against a direct convolution.
        //
        // A pixel enters while the window is still growing, and a pixel leaves once the left edge can
        // advance: `x >= half` is exactly when the left edge stops being pinned at 0, and
        // `x + 1 + half <= len - 1` is exactly when the right edge has not yet been pinned.
        let mut delta = [0i64; 4];
        if x + 1 + half < len {
            let entering = (x + 1 + half) * 4;
            for c in 0..4 {
                delta[c] += source[entering + c] as i64;
            }
        }
        if x >= half {
            let leaving = lo_of(x) * 4;
            for c in 0..4 {
                delta[c] -= source[leaving + c] as i64;
            }
        }
        for c in 0..4 {
            sums[c] += delta[c];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // `Vec` from `compat` rather than the std prelude. These tests build pixel buffers, and on the
    // `ohos`/`android` cross targets there is no `std` prelude to supply `Vec` -- which is how they
    // broke `check_harmony_cross.sh` (`cannot find type Vec in this scope`) while every host build
    // stayed green. `compat` re-exports `alloc::vec::Vec`, so importing it makes the tests mean the
    // same thing on every target rather than only on the ones with a `std`.
    use crate::compat::Vec;
    use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};

    // ── SoftwarePaintBackend construction ───────────────────────────────

    #[test]
    fn software_paint_backend_new_creates_surface() {
        let size = Size::new(100, 100);
        let backend = SoftwarePaintBackend::new(size, 1.0);
        assert_eq!(backend.size(), size);
        assert!((backend.dpi_scale() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn software_paint_backend_zero_size() {
        let backend = SoftwarePaintBackend::new(Size::new(0, 0), 1.0);
        assert_eq!(backend.size(), Size::new(0, 0));
        let rgba = backend.frame_rgba();
        assert!(rgba.is_empty());
    }

    #[test]
    fn software_paint_backend_high_dpi() {
        let backend = SoftwarePaintBackend::new(Size::new(50, 50), 2.0);
        assert!((backend.dpi_scale() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn software_paint_backend_minimum_dpi_scale() {
        let backend = SoftwarePaintBackend::new(Size::new(10, 10), 0.0);
        assert!((backend.dpi_scale() - 0.1).abs() < 1e-6);
    }

    // ── SoftwarePaintBackend surface access ─────────────────────────────

    /// The software backend paints a gradient without building the owning command.
    ///
    /// `fill_gradient`'s trait default constructs `RenderCommand::DrawGradient`, which owns a copy
    /// of the paint server — `stops` vector and all — for pixels an immediate backend is about to
    /// paint and forget. The software surface overrides it, so the counter must not move; the pixel
    /// assertions keep the override honest about *painting* what the default would have painted.
    #[test]
    fn the_software_backend_fills_a_gradient_without_building_the_owning_command() {
        use crate::style::{Gradient, GradientStop};

        let size = Size::new(16, 16);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let _ = take_gradient_owned_command_count();

        let gradient = Gradient::linear(Point::new(0, 0), Point::new(16, 0)).with_stops(vec![
            GradientStop::new(0.0, Color::RED),
            GradientStop::new(1.0, Color::BLUE),
        ]);
        backend.fill_gradient(Rect::new(0, 0, 16, 16), &gradient);
        backend.end_frame();

        assert_eq!(
            take_gradient_owned_command_count(),
            0,
            "the software backend must paint the gradient itself rather than copy it into a command"
        );

        let rgba = backend.frame_rgba();

        // Reference: the same gradient through the owning command, which is the default the
        // override replaced. Equal buffers prove the override paints what the default painted;
        // the WHITE-only backend proves it painted *something* (an equality check against an
        // untouched frame would otherwise pass for a no-op).
        let mut through_command = SoftwarePaintBackend::new(size, 1.0);
        through_command.begin_frame(Color::WHITE);
        through_command.execute_command(&RenderCommand::DrawGradient {
            rect: Rect::new(0, 0, 16, 16),
            gradient: Arc::new(gradient.clone()),
        });
        through_command.end_frame();
        let mut untouched = SoftwarePaintBackend::new(size, 1.0);
        untouched.begin_frame(Color::WHITE);
        untouched.end_frame();

        assert_eq!(
            rgba,
            through_command.frame_rgba(),
            "the override must paint the command's pixels"
        );
        assert_ne!(rgba, untouched.frame_rgba(), "the gradient must actually paint");
    }

    #[test]
    fn software_paint_backend_surface_accessor() {
        let backend = SoftwarePaintBackend::new(Size::new(30, 30), 1.0);
        let surface = backend.surface();
        assert_eq!(surface.size(), Size::new(30, 30));
    }

    #[test]
    fn software_paint_backend_surface_mut_accessor() {
        let mut backend = SoftwarePaintBackend::new(Size::new(40, 40), 1.0);
        {
            let surface = backend.surface_mut();
            assert_eq!(surface.size(), Size::new(40, 40));
        }
    }

    // ── PaintBackend trait - frame lifecycle ────────────────────────────

    #[test]
    fn paint_backend_begin_end_frame_clears() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);

        backend.begin_frame(Color::RED);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255); // R
            assert_eq!(chunk[1], 0); // G
            assert_eq!(chunk[2], 0); // B
            assert_eq!(chunk[3], 255); // A
        }
    }

    #[test]
    fn paint_backend_execute_fill_rect() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(2, 2, 10, 10),
            color: Color::BLUE,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let idx = 5 * stride + 5 * 4;
        assert_eq!(rgba[idx], 0); // R
        assert_eq!(rgba[idx + 1], 0); // G
        assert_eq!(rgba[idx + 2], 255); // B
    }

    /// The `3d` feature's quad command fills the quadrilateral, not its bounding box.
    ///
    /// The fixture is a diamond: the centre and the four mid-edges are inside it, while the
    /// corners of its bounding box are outside. A backend that collapsed the command to
    /// `Rect(min, min, width, height)` would paint those corners too.
    #[cfg(feature = "3d")]
    #[test]
    fn paint_backend_execute_fill_quad_fills_the_quadrilateral() {
        let size = Size::new(24, 24);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillQuad {
            corners: [Point::new(12, 2), Point::new(22, 12), Point::new(12, 22), Point::new(2, 12)],
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 24 * 4;
        let center = 12 * stride + 12 * 4;
        assert_eq!(&rgba[center..center + 4], &[255, 0, 0, 255], "the centre is inside");

        let box_corner = 3 * stride + 3 * 4;
        assert_eq!(rgba[box_corner], 255, "the bounding-box corner is outside the diamond");
        assert_eq!(rgba[box_corner + 1], 255);
        assert_eq!(rgba[box_corner + 2], 255);
    }

    #[test]
    fn paint_backend_execute_draw_rect_stroke() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawRectStroke {
            rect: Rect::new(0, 0, 20, 20),
            color: Color::GREEN,
            width: 1,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 0); // R
        assert_eq!(rgba[1], 255); // G
        assert_eq!(rgba[2], 0); // B
    }

    #[test]
    fn paint_backend_execute_draw_line() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawLine {
            from: Point::new(0, 0),
            to: Point::new(9, 9),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[3], 255); // A
    }

    #[test]
    fn paint_backend_execute_push_pop_clip() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::PushClip { x: 0, y: 0, width: 5, height: 5 });
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 10, 10),
            color: Color::RED,
        });
        backend.execute_command(&RenderCommand::PopClip);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 10 * 4;
        let idx = 2 * stride + 2 * 4;
        assert_eq!(rgba[idx], 255); // R
        assert_eq!(rgba[idx + 3], 255); // A
    }

    #[test]
    fn paint_backend_execute_fill_circle() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillCircleAA {
            center: Point::new(10, 10),
            radius: 5,
            color: Color::BLUE,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let idx = 10 * stride + 10 * 4;
        assert_eq!(rgba[idx], 0); // R
        assert_eq!(rgba[idx + 2], 255); // B
    }

    #[test]
    fn paint_backend_size_set_size() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        assert_eq!(backend.size(), Size::new(10, 10));

        let new_size = Size::new(50, 50);
        backend.set_size(new_size);
        assert_eq!(backend.size(), new_size);
    }

    #[test]
    fn paint_backend_dpi_scale_set_dpi_scale() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.set_dpi_scale(1.5);
        assert!((backend.dpi_scale() - 1.5).abs() < 1e-6);
    }

    // ── SoftwarePaintBackend config ─────────────────────────────────────

    #[test]
    fn paint_backend_render_config_default() {
        let backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let config = backend.render_config();
        assert_eq!(config.aa_samples_per_axis, 4);
    }

    #[test]
    fn paint_backend_apply_render_config() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let config = SoftwareRenderConfig { aa_samples_per_axis: 2 };
        backend.apply_render_config(config);
        assert_eq!(backend.render_config().aa_samples_per_axis, 2);
    }

    #[test]
    fn paint_backend_apply_render_config_clamps_to_normalized_range() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let config = SoftwareRenderConfig { aa_samples_per_axis: 99 };
        backend.apply_render_config(config);
        assert_eq!(backend.render_config().aa_samples_per_axis, 8);

        let config = SoftwareRenderConfig { aa_samples_per_axis: 0 };
        backend.apply_render_config(config);
        assert_eq!(backend.render_config().aa_samples_per_axis, 1);
    }

    #[test]
    fn paint_backend_default_render_config() {
        let config = <SoftwarePaintBackend as PaintBackend>::render_config(
            &SoftwarePaintBackend::new(Size::new(1, 1), 1.0),
        );
        assert_eq!(config, SoftwareRenderConfig::default());
    }

    #[test]
    fn paint_backend_execute_draw_text_does_not_panic() {
        let size = Size::new(100, 100);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        let font = Font::simple("Arial", 12.0);
        backend.execute_command(&RenderCommand::DrawText {
            origin: Point::new(10, 20),
            text: Arc::from("Hello"),
            font,
            color: Color::BLACK,
            alignment: HorizontalAlignment::Left,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn paint_backend_execute_draw_image() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        let data = vec![
            255, 0, 0, 255, // red
            0, 255, 0, 255, // green
            0, 0, 255, 255, // blue
            255, 255, 0, 255, // yellow
        ];
        backend.execute_command(&RenderCommand::DrawImage {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            data: data.into(),
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[1], 0); // G
        assert_eq!(rgba[2], 0); // B
    }

    #[test]
    fn paint_backend_execute_draw_rect() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawRect {
            rect: Rect::new(0, 0, 10, 10),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[3], 255); // A
    }

    #[test]
    fn paint_backend_execute_fill_rounded_rect() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRoundedRect {
            rect: Rect::new(2, 2, 16, 16),
            radii: [4, 4, 4, 4],
            color: Color::GREEN,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let idx = 10 * stride + 10 * 4;
        assert_eq!(rgba[idx + 1], 255); // G
    }

    #[test]
    fn paint_backend_execute_draw_circle_stroke() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawCircleStroke {
            center: Point::new(10, 10),
            radius: 5,
            color: Color::RED,
            width: 2,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    // ── PaintBackend measure_text / shape_text ──────────────────────────

    #[test]
    fn paint_backend_measure_text_returns_metrics() {
        let backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        let font = Font::simple("Arial", 12.0);
        let metrics = backend.measure_text("Hello", &font);
        assert!(metrics.width > 0);
        assert!(metrics.height > 0);
    }

    #[test]
    fn paint_backend_shape_text_returns_shaped() {
        let backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        let font = Font::simple("Arial", 12.0);
        let shaped = backend.shape_text("Hi", &font);
        assert!(!shaped.clusters.is_empty());
    }

    #[test]
    fn paint_backend_frame_rgba_after_clear() {
        let size = Size::new(5, 5);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::rgb(128, 64, 32));
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let expected_len = 5 * 5 * 4;
        assert_eq!(rgba.len(), expected_len);

        assert_eq!(rgba[0], 128);
        assert_eq!(rgba[1], 64);
        assert_eq!(rgba[2], 32);
        assert_eq!(rgba[3], 255);
    }

    // ── Edge cases ──────────────────────────────────────────────────────

    #[test]
    fn paint_backend_execute_fill_rect_out_of_bounds() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(100, 100, 50, 50),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255);
            assert_eq!(chunk[1], 255);
            assert_eq!(chunk[2], 255);
        }
    }

    #[test]
    fn paint_backend_fill_rect_zero_size() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 0, 0),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255);
        }
    }

    #[test]
    fn paint_backend_multiple_commands_in_frame() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 5, 10),
            color: Color::RED,
        });
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(5, 0, 5, 10),
            color: Color::BLUE,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 10 * 4;
        let left_idx = 2 * stride + 2 * 4;
        assert_eq!(rgba[left_idx], 255); // R
        assert_eq!(rgba[left_idx + 2], 0); // B

        let right_idx = 5 * stride + 7 * 4;
        assert_eq!(rgba[right_idx], 0); // R
        assert_eq!(rgba[right_idx + 2], 255); // B
    }

    /// The blur's cost must not grow with the radius — that is the whole point of the sliding
    /// window, and it is a *timing* claim, so the measurement has to be built to survive a busy
    /// machine.
    ///
    /// Two things make it fragile if written naively, and both have been observed: a single sample
    /// of a fast blur is short enough that scheduler jitter is a large fraction of it, and
    /// measuring *all* of one radius before starting the other lets a burst of load inflate only
    /// the second. So the two radii are **interleaved** (load that arrives hits a sample of each)
    /// and each result is the **minimum** of several samples (the sample least disturbed by load).
    ///
    /// The threshold is a *shape* boundary, not a benchmark. Measured on this 400×400 surface in the
    /// debug profile this suite runs:
    ///
    ///   * sliding window (correct):      40 px costs ~2.9x a 2 px
    ///   * recomputed window (the bug):   40 px costs ~16.4x a 2 px
    ///
    /// `6.0` sits ~2x above the correct shape and ~2.7x below the bug, so the verdict does not
    /// depend on how fast the machine is. 4.0 was too close to 2.9: a single unlucky sample was
    /// enough to read a working blur as broken.
    #[test]
    fn the_blur_cost_does_not_grow_with_the_radius() {
        use core::time::Duration;
        use std::time::Instant;

        let (w, h) = (400usize, 400usize);
        // One buffer, reused. The blur's cost depends on the region and the radius, never on the
        // pixel values, so re-blurring the same bytes measures the same work — and it keeps the
        // fill (which `sin`s 640k values) out of the neighbour of the timed region, where it used
        // to be the larger term.
        let mut buf: Vec<u8> =
            (0..w * h * 4).map(|i| ((i as f32 * 0.37).sin().abs() * 255.0) as u8).collect();

        // One scratch set, reused exactly as the surface does at runtime (G-07).
        let mut scratch = crate::render::backend::surface::BoxBlurScratch::default();
        let mut blur = |radius: usize| {
            let start = Instant::now();
            box_blur_region(&mut buf, w, h, (0, 0, w, h), radius, &mut scratch);
            start.elapsed()
        };

        const SAMPLES: usize = 5;
        let mut small = Duration::MAX;
        let mut large = Duration::MAX;
        for _ in 0..SAMPLES {
            small = small.min(blur(2));
            large = large.min(blur(40));
        }

        let ratio = large.as_secs_f64() / small.as_secs_f64().max(1e-9);
        assert!(
            ratio < 6.0,
            "a 40 px blur took {ratio:.1}x a 2 px one ({large:?} vs {small:?}, minimum of \
             {SAMPLES} samples) — the cost is still growing with the radius, so the window is not \
             sliding"
        );
    }

    // ── The Gaussian-approximating blur ─────────────────────────────────

    /// The sliding window must agree with the naive sum it replaced.
    ///
    /// This is the whole risk of the optimisation: an incremental window that adds and subtracts the
    /// wrong pixels is *plausible* — it produces a smooth-looking image — and wrong. So it is checked
    /// against a direct convolution, which is the definition, on a signal with no symmetry that could
    /// hide a mirrored index error.
    #[test]
    fn the_sliding_window_agrees_with_a_direct_convolution() {
        // A row of distinct values, so an off-by-one in either direction changes the output.
        let len = 23usize;
        let width = 5usize;
        let half = width / 2;
        let source: Vec<u8> =
            (0..len * 4).map(|i| ((i as f32 * 37.0).sin().abs() * 200.0 + 20.0) as u8).collect();
        let mut blurred = source.clone();
        blur_row(&mut blurred, len, width, &source);

        for x in 0..len {
            for c in 0..4 {
                // The reference: clamp the window at both ends *and* divide by the number of pixels
                // actually accumulated, which is what `blur_row` documents it does.
                let lo = x.saturating_sub(half);
                let hi = (x + half).min(len - 1);
                let sum: u32 = (lo..=hi).map(|k| source[k * 4 + c] as u32).sum();
                let expected = (sum / (hi - lo + 1) as u32) as u8;
                assert_eq!(
                    blurred[x * 4 + c],
                    expected,
                    "pixel {x} channel {c}: the window and the direct sum must agree"
                );
            }
        }
    }

    /// The window count must be exact, not merely close.
    ///
    /// A divisor that is off by one at an edge produces a subtle gradient rather than an obvious
    /// band, and the agreement test above would catch it only because it re-derives the same count.
    /// Pinning the count directly is what makes that test meaningful rather than circular.
    #[test]
    fn a_constant_field_survives_the_edge_clamping() {
        // A flat field must stay flat: every clamped window sums `count` copies of the same value, so
        // a divisor larger than `count` would darken the border and one smaller would lighten it.
        let len = 16usize;
        let mut row = vec![200u8; len * 4];
        blur_row(&mut row, len, 7, &vec![200u8; len * 4]);
        assert!(
            row.iter().all(|&v| v == 200),
            "a constant field must be unchanged by a blur, including at the edges"
        );
    }

    /// The three box widths must number three, be odd, and grow with the requested sigma.
    ///
    /// Odd widths give each box a true centre pixel, which is what makes the iterated kernel
    /// symmetric; a monotonic response is what makes `radius` mean something to a caller.
    #[test]
    fn the_box_widths_are_odd_and_monotonic() {
        let mut previous = 0usize;
        for radius in [1usize, 2, 4, 8, 16, 32, 64] {
            let (a, b, c) = box_blur_widths(radius as f32 / 2.0);
            assert!(a % 2 == 1 && b % 2 == 1 && c % 2 == 1, "widths must be odd: {a} {b} {c}");
            assert_eq!(a, b, "the pair must match, or the kernel is asymmetric");
            let reach = a + c;
            assert!(reach >= previous, "radius {radius} must not blur less than a smaller one");
            previous = reach;
        }
        assert_eq!(box_blur_widths(0.0), (0, 0, 0), "a zero sigma is not a blur");
        assert_eq!(box_blur_widths(-1.0), (0, 0, 0), "and neither is a negative one");
    }

    /// The three boxes together must realise the requested `sigma`, not three times its variance.
    ///
    /// # The defect this pins
    ///
    /// `box_blur_widths` solved each width from `variance = (w^2 - 1) / 12 == sigma^2`, i.e. it put
    /// the **whole** requested variance into **each** of the three passes. Variances add, so the
    /// kernel that came out had three times the intended variance -- about `sqrt(3)` times too wide,
    /// which measured out as every shadow being 1.6-1.8x softer than its own elevation said:
    ///
    /// | radius | intended sigma | produced |
    /// |---|---|---|
    /// | 2 px | 1.0 | 1.83 |
    /// | 6 px | 3.0 | 4.83 |
    /// | 12 px | 6.0 | 9.83 |
    /// | 24 px | 12.0 | 20.83 |
    ///
    /// The widths are still *odd* and still *monotonic* with such a bug present, which is exactly
    /// why `the_box_widths_are_odd_and_monotonic` above cannot see it: it asserts the shape of the
    /// response, not its scale. This test asserts the scale.
    ///
    /// # Why the radii below are the ladder's own, and why the band is wide at the small end
    ///
    /// A box width is an integer, so the realised variance can only land where some `(w^2 - 1) / 12`
    /// falls. The residual is therefore quantisation, and it shrinks as a fraction of the target as
    /// sigma grows. Measured after the fix, over exactly the four radii
    /// [`crate::render::default_shadow`] uses (`blur` 3/6/12/24):
    ///
    /// | blur | sigma | realised / target | standard deviation |
    /// |---|---|---|---|
    /// | 3 | 1.5 | 1.48 | 1.22x |
    /// | 6 | 3.0 | 0.889 | 0.94x |
    /// | 12 | 6.0 | 0.944 | 0.97x |
    /// | 24 | 12.0 | 0.972 | 0.99x |
    ///
    /// So the band is `0.6..=1.6` on variance (0.77x-1.26x on standard deviation): it admits the
    /// quantisation at `blur: 3`, and it still rejects the `sqrt(3)`-scale error by a wide margin --
    /// that bug produces 2.59-3.67x, i.e. outside the band at every radius.
    ///
    /// # Why `radius: 1` is deliberately not in the list
    ///
    /// Because at that radius the test **cannot tell the fix from the bug**, and a case that passes
    /// either way is not evidence. `box_blur_widths(0.5)` is `(1, 1, 3)` both before and after the
    /// fix -- every width floors to the same odd integers -- so both kernels realise 0.67 against a
    /// target of 0.25, a ratio of 2.667. Asserting it would either fail the *correct* code or force a
    /// band wide enough to admit the *broken* one, and both of those make the test worse than not
    /// having it.
    ///
    /// The smallest radius where a separating integer kernel exists is 2 (`sigma` 1.0), and the
    /// separation is complete from there up: fixed gives 0.667/1.48/0.83/0.89/0.92/0.94/0.97/0.99,
    /// buggy gives 3.33/3.56/3.67/2.59/2.92/2.69/3.01/3.04. Excluding one radius for a stated
    /// reason is the honest form of this exclusion; silently widening the band to cover it would
    /// have hidden exactly the defect the test exists for.
    #[test]
    fn the_three_boxes_realise_the_requested_sigma_not_three_times_it() {
        let variance_of = |w: usize| {
            if w == 0 {
                0.0
            } else {
                (w * w - 1) as f32 / 12.0
            }
        };

        // `default_shadow`'s four levels, plus a finer and a coarser radius so a regression that
        // only shows at one scale cannot hide behind the ladder's particular values. `radius: 1` is
        // excluded on purpose -- see this test's rustdoc.
        for radius in [2usize, 3, 4, 6, 8, 12, 24, 48, 96] {
            let sigma = radius as f32 / 2.0;
            let (a, b, c) = box_blur_widths(sigma);
            let realised = variance_of(a) + variance_of(b) + variance_of(c);
            let target = sigma * sigma;
            let ratio = realised / target;
            assert!(
                (0.6..=1.6).contains(&ratio),
                "radius {radius}: the kernel realises variance {realised:.2} against a target of \
                 {target:.2} (ratio {ratio:.3}), i.e. a standard deviation {:.2}x the one asked for. \
                 Widths were ({a}, {b}, {c}). A ratio near 3 means the per-pass variance share was \
                 dropped again.",
                ratio.sqrt()
            );
        }
    }

    /// The blur is separable: three boxes in x and three in y must equal the same work done the
    /// other way round.
    ///
    /// This is the property that lets the implementation do a row pass and a column pass instead of
    /// a two-dimensional kernel, and it is the one an accidental transposition would break.
    #[test]
    fn the_blur_is_separable() {
        let (w, h) = (9usize, 7usize);
        let source: Vec<u8> =
            (0..w * h * 4).map(|i| ((i as f32 * 91.0).cos().abs() * 180.0 + 30.0) as u8).collect();

        let mut horizontal_first = source.clone();
        box_blur_region(
            &mut horizontal_first,
            w,
            h,
            (0, 0, w, h),
            6,
            &mut crate::render::backend::surface::BoxBlurScratch::default(),
        );

        // The same region blurred after a vertical translation of the *source* by zero pixels is
        // the only honest comparison: `box_blur_region` always runs x then y, so "y then x" is
        // expressed by blurring a transposed copy and transposing back.
        let mut transposed = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                transposed[(x * h + y) * 4..(x * h + y) * 4 + 4]
                    .copy_from_slice(&source[(y * w + x) * 4..(y * w + x) * 4 + 4]);
            }
        }
        box_blur_region(
            &mut transposed,
            h,
            w,
            (0, 0, h, w),
            6,
            &mut crate::render::backend::surface::BoxBlurScratch::default(),
        );

        // A box blur is symmetric, so transposing the result back is a reference for the original.
        // Tolerated by one level: the three boxes are applied in a different order, so integer
        // truncation can land on either side of the exact value.
        for y in 0..h {
            for x in 0..w {
                for c in 0..4 {
                    let a = horizontal_first[(y * w + x) * 4 + c] as i32;
                    let b = transposed[(x * h + y) * 4 + c] as i32;
                    assert!(
                        (a - b).abs() <= 1,
                        "({x},{y}) channel {c}: {a} vs {b} — the passes are not separable"
                    );
                }
            }
        }
    }

    /// A backdrop-blur layer blurs the pixels that were already there and keeps the region's
    /// own drawing out of the blur source.
    #[test]
    fn a_backdrop_blur_layer_blurs_what_is_behind_it() {
        use crate::render::core::command::LayerEffect;

        let mut backend = SoftwarePaintBackend::new(Size::new(40, 20), 1.0);
        backend.begin_frame(Color::BLACK);
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 20, 20),
            color: Color::rgb(255, 0, 0),
        });
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(20, 0, 20, 20),
            color: Color::rgb(0, 0, 255),
        });
        backend.execute_command(&RenderCommand::PushLayer {
            bounds: Rect::new(0, 0, 40, 20),
            effect: Some(LayerEffect::BackdropBlur { radius: 8 }),
        });
        backend.execute_command(&RenderCommand::PopLayer);
        backend.end_frame();

        let frame = backend.frame_rgba();
        let at = |x: usize, y: usize| -> [u8; 4] {
            let index = (y * 40 + x) * 4;
            [frame[index], frame[index + 1], frame[index + 2], frame[index + 3]]
        };
        assert_eq!(at(0, 10), [255, 0, 0, 255], "the untouched edge stays red");
        assert_eq!(at(39, 10), [0, 0, 255, 255], "and the other edge stays blue");
        let left = at(19, 10);
        let right = at(20, 10);
        assert!(left[2] > 0, "blue must bleed across the hard edge: {left:?}");
        assert!(right[0] > 0, "red must bleed across the hard edge: {right:?}");
    }

    /// The foreground is composited **over** the processed backdrop, not blurred with it.
    #[test]
    fn a_layer_composites_its_foreground_over_the_tinted_backdrop() {
        use crate::render::core::command::LayerEffect;

        let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
        backend.begin_frame(Color::TRANSPARENT);
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 20, 20),
            color: Color::WHITE,
        });
        backend.execute_command(&RenderCommand::PushLayer {
            bounds: Rect::new(0, 0, 20, 20),
            effect: Some(LayerEffect::BackdropBlurTint {
                radius: 0,
                tint: Color::rgb(255, 0, 0),
                tint_weight: 1.0,
            }),
        });
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(4, 4, 4, 4),
            color: Color::rgb(0, 255, 0),
        });
        backend.execute_command(&RenderCommand::PopLayer);
        backend.end_frame();

        let frame = backend.frame_rgba();
        let at = |x: usize, y: usize| -> [u8; 4] {
            let index = (y * 20 + x) * 4;
            [frame[index], frame[index + 1], frame[index + 2], frame[index + 3]]
        };
        assert_eq!(at(10, 10), [255, 0, 0, 255], "the backdrop takes the tint");
        assert_eq!(at(5, 5), [0, 255, 0, 255], "the foreground stays on top of it");
    }

    /// Layer composition is a 2D capability: the same commands produce the same pixels in a
    /// build with and without the `3d` feature.
    ///
    /// This test is deliberately **not** `cfg`-gated. It runs in both matrices and asserts a
    /// concrete picture, so a future change that accidentally puts the layer path behind `3d`
    /// would either fail to compile in the 2D matrix or produce different pixels here.
    #[test]
    fn a_layer_snapshot_is_the_same_with_and_without_the_3d_feature() {
        use crate::render::core::command::LayerEffect;

        let mut backend = SoftwarePaintBackend::new(Size::new(16, 16), 1.0);
        backend.begin_frame(Color::rgb(0, 0, 0));
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 8, 16),
            color: Color::rgb(200, 40, 20),
        });
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(8, 0, 8, 16),
            color: Color::rgb(20, 40, 200),
        });
        backend.execute_command(&RenderCommand::PushLayer {
            bounds: Rect::new(0, 0, 16, 16),
            effect: Some(LayerEffect::BackdropBlur { radius: 6 }),
        });
        backend.execute_command(&RenderCommand::PopLayer);
        backend.end_frame();

        let frame = backend.frame_rgba();
        let index = (8 * 16 + 8) * 4;
        let red = frame[index];
        let blue = frame[index + 2];
        assert!(
            red > 0 && blue > 0,
            "the seam must be a blend of both halves regardless of the 3d feature: {red}, {blue}"
        );
    }

    /// A layer group's opacity fades the whole subtree, not each primitive separately.
    ///
    /// The group paints an opaque red fill over a white backdrop at 50% opacity. Compositing the
    /// subtree as one unit leaves the red channel at full strength (both colours agree there) and
    /// mixes the green and blue channels halfway, which per-primitive blending could not do.
    #[test]
    fn a_layer_group_applies_its_opacity_to_the_subtree() {
        let mut backend = SoftwarePaintBackend::new(Size::new(4, 4), 1.0);
        backend.begin_frame(Color::WHITE);
        backend.execute_command(&RenderCommand::PushLayerGroup {
            bounds: Rect::new(0, 0, 4, 4),
            effect: None,
            opacity: 0.5,
            cache_key: None,
        });
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 4, 4),
            color: Color::RED,
        });
        backend.execute_command(&RenderCommand::PopLayer);
        backend.end_frame();

        let frame = backend.frame_rgba();
        assert_eq!(
            &frame[..4],
            &[255, 127, 127, 255],
            "the group is halfway between red and white"
        );
    }

    /// A fully transparent layer must not clear its region.
    ///
    /// The layer isolates its foreground by clearing the backdrop at push. At zero opacity the
    /// composite is a no-op, so the backdrop has to be restored — otherwise the group would be a
    /// transparent hole through the surface.
    #[test]
    fn a_fully_transparent_layer_keeps_its_backdrop() {
        let mut backend = SoftwarePaintBackend::new(Size::new(4, 4), 1.0);
        backend.begin_frame(Color::WHITE);
        backend.execute_command(&RenderCommand::PushLayerGroup {
            bounds: Rect::new(0, 0, 4, 4),
            effect: None,
            opacity: 0.0,
            cache_key: None,
        });
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 4, 4),
            color: Color::RED,
        });
        backend.execute_command(&RenderCommand::PopLayer);
        backend.end_frame();
        assert_eq!(
            &backend.frame_rgba()[..4],
            &[255, 255, 255, 255],
            "an invisible group must leave the backdrop exactly as it was"
        );
    }

    /// A cache hit reuses the previous frame's foreground instead of replaying the subtree.
    ///
    /// Frame two draws a blue rectangle into the *same* cached group; the blue must not appear,
    /// because the cache hit discards the enclosed commands. Frame three changes the key, which
    /// is the documented way to say "the content changed", and the blue must appear.
    #[test]
    fn a_cached_layer_reuses_its_foreground_until_the_key_changes() {
        let mut backend = SoftwarePaintBackend::new(Size::new(4, 4), 1.0);
        let group = |cache_key: u64| RenderCommand::PushLayerGroup {
            bounds: Rect::new(0, 0, 4, 4),
            effect: None,
            opacity: 1.0,
            cache_key: Some(cache_key),
        };

        backend.begin_frame(Color::WHITE);
        backend.execute_command(&group(7));
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 4, 4),
            color: Color::RED,
        });
        backend.execute_command(&RenderCommand::PopLayer);
        backend.end_frame();
        assert_eq!(backend.cached_layer_count(), 1, "the first frame fills the cache");

        backend.begin_frame(Color::WHITE);
        backend.execute_command(&group(7));
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 4, 4),
            color: Color::BLUE,
        });
        backend.execute_command(&RenderCommand::PopLayer);
        backend.end_frame();
        assert_eq!(
            &backend.frame_rgba()[..4],
            &[255, 0, 0, 255],
            "the cached red foreground must replace the suppressed blue fill"
        );

        backend.begin_frame(Color::WHITE);
        backend.execute_command(&group(8));
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 4, 4),
            color: Color::BLUE,
        });
        backend.execute_command(&RenderCommand::PopLayer);
        backend.end_frame();
        assert_eq!(
            &backend.frame_rgba()[..4],
            &[0, 0, 255, 255],
            "a new key must redraw the subtree"
        );
    }

    /// An opacity change invalidates a cache entry with the same key.
    #[test]
    fn changing_a_cached_layer_opacity_redraws_it() {
        let mut backend = SoftwarePaintBackend::new(Size::new(2, 2), 1.0);
        let group = |opacity: f32| RenderCommand::PushLayerGroup {
            bounds: Rect::new(0, 0, 2, 2),
            effect: None,
            opacity,
            cache_key: Some(3),
        };
        backend.begin_frame(Color::WHITE);
        backend.execute_command(&group(1.0));
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 2, 2),
            color: Color::RED,
        });
        backend.execute_command(&RenderCommand::PopLayer);
        backend.end_frame();

        backend.begin_frame(Color::WHITE);
        backend.execute_command(&group(0.5));
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 2, 2),
            color: Color::BLUE,
        });
        backend.execute_command(&RenderCommand::PopLayer);
        backend.end_frame();
        assert_eq!(
            &backend.frame_rgba()[..4],
            &[127, 127, 255, 255],
            "the new opacity must be applied to the newly drawn foreground"
        );
    }

    /// Clearing the layer cache forces the next frame to redraw.
    #[test]
    fn clearing_the_layer_cache_forces_a_redraw() {
        let mut backend = SoftwarePaintBackend::new(Size::new(2, 2), 1.0);
        let group = RenderCommand::PushLayerGroup {
            bounds: Rect::new(0, 0, 2, 2),
            effect: None,
            opacity: 1.0,
            cache_key: Some(11),
        };
        backend.begin_frame(Color::WHITE);
        backend.execute_command(&group);
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 2, 2),
            color: Color::RED,
        });
        backend.execute_command(&RenderCommand::PopLayer);
        backend.end_frame();
        assert_eq!(backend.cached_layer_count(), 1);
        backend.clear_layer_cache();
        assert_eq!(backend.cached_layer_count(), 0);
    }
}
