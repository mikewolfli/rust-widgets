use super::*;

/// Software raster surface with quality controls and RGBA frame output.
///
/// Wraps a double-buffered [`BackBuffer`]: drawing commands and
/// [`SoftwareSurface::begin_frame`] act on the back buffer, and
/// [`SoftwareSurface::end_frame`] presents it. Reads via
/// [`SoftwareSurface::frame_rgba`] therefore return the last presented frame
/// (the front buffer), which is one frame behind the buffer currently being
/// drawn into.
///
/// Pixels are 8-bit RGBA, tightly packed at 4 bytes per pixel regardless of
/// DPI scale; `dpi_scale` only affects text metrics and geometry that opts in.
pub struct SoftwareSurface {
    pub(crate) buffer: BackBuffer,
    pub(crate) aa_samples_per_axis: u8,
    /// Active clip rectangle stack. An empty stack means no clipping.
    pub(crate) clip_stack: Vec<(i32, i32, u32, u32)>,
    /// Scratch space reused by compound-path scan conversion.
    pub(crate) nonzero_intersections: Vec<(f32, i32)>,
    /// Scratch coverage buffer reused by the text raster path.
    ///
    /// `draw_text` needs `cell.width * cell.height` bytes per glyph to hold the rasterised
    /// coverage before blending. Allocating that inside the loop allocated once per glyph —
    /// tens of thousands of allocations a second on a text-heavy window — while the buffer is
    /// discarded immediately after. Keeping it on the surface makes the allocation per line at
    /// worst, and usually per surface.
    pub(crate) text_coverage_scratch: Vec<u8>,
    /// Active path clips, parallel to [`Self::clip_stack`].
    ///
    /// Every rectangle push records one entry here (`None` for a plain rectangle clip), so the two
    /// stacks stay the same height and a `PopClip` always releases exactly one of each. A `Some`
    /// entry carries the path's coverage mask and the backdrop it must restore on pop.
    pub(crate) path_clips: Vec<Option<PathClip>>,
    /// The blend mode the next pixel write composites through.
    ///
    /// Set by [`SoftwareSurface::set_blend_mode`] (driven by
    /// [`crate::render::RenderCommand::SetBlendMode`]) and read by every pixel write. It lives on
    /// the surface rather than on a primitive because a mode applies to *everything drawn until it
    /// changes* — a sequential fact, exactly like the clip stack — and because every opaque and
    /// antialiased primitive funnels its writes through [`SoftwareSurface::write_pixel`].
    pub(crate) blend_mode: BlendMode,
    /// Monotonic frame counter, advanced by [`SoftwareSurface::begin_frame`] (G-020).
    ///
    /// The glyph atlas' epoch used to be advanced by an approximate lookup count, which drifted
    /// from the real frame boundary and made eviction timing depend on how much text a frame
    /// happened to draw. The counter is the true frame boundary: one `begin_frame`, one epoch.
    pub(crate) frame_number: u64,
    /// Whether text is drawn glyph by glyph through a shaped run.
    ///
    /// Off by default, which keeps the cluster raster path (and every existing snapshot)
    /// unchanged. When on and a face matches `Font::family`, [`SoftwareSurface::draw_text`]
    /// shapes with the face's standard ligatures enabled and paints the resulting glyph ids, so a
    /// ligature or a contextual form is drawn as the shaper chose it rather than as its individual
    /// characters.
    pub(crate) glyph_run_text: bool,
    /// Depth buffer for the `3d` mesh raster path, one `f32` per pixel.
    ///
    /// Depth is normalised `0..=1`, with `0` at the camera's near plane and `1` at its far plane.
    /// The buffer lives on the surface rather than on one draw so that two mesh commands in a frame
    /// are ordered by depth rather than by recording order; [`Self::begin_frame`] marks it stale and
    /// the next mesh draw refills it with the far value. A 2D command never reads it.
    #[cfg(feature = "3d")]
    pub(crate) depth_buffer: Vec<f32>,
    /// Whether [`Self::depth_buffer`] still holds the previous frame's depths and must be cleared.
    #[cfg(feature = "3d")]
    pub(crate) depth_dirty: bool,
    /// Scratch space reused by [`Self::draw_mesh_3d`] for the projected triangles.
    ///
    /// The projection used to allocate a fresh `Vec` on every mesh draw — once per 3D control per
    /// frame. Keeping it on the surface, exactly as [`Self::text_coverage_scratch`] is kept for the
    /// text path, makes the allocation per surface rather than per draw; the vector's capacity only
    /// grows, so a mesh no larger than the last one pays nothing.
    #[cfg(feature = "3d")]
    pub(crate) projected_triangles_scratch: Vec<crate::render::mesh3d::ProjectedTriangle>,
}

/// How many reusable backdrop buffers a surface keeps (G-06).
///
/// Four covers a modal scrim, its card and a nested group without pinning an unbounded amount of
/// pixel memory; deeper nesting falls back to an allocation, which is the same cost as before.
pub(crate) const LAYER_BACKDROP_POOL_LIMIT: usize = 4;

impl SoftwareSurface {
    /// Takes the reusable blur buffers, or an empty set when none are pooled yet (G-07).
    pub(crate) fn take_blur_scratch(&mut self) -> BoxBlurScratch {
        self.buffer.blur_scratch.take().unwrap_or_default()
    }

    /// Returns the blur buffers for the next call. The vectors keep their capacity (G-07).
    pub(crate) fn return_blur_scratch(&mut self, scratch: BoxBlurScratch) {
        self.buffer.blur_scratch = Some(scratch);
    }

    /// Takes the reusable projected-triangle scratch, or an empty vector when none is pooled.
    #[cfg(feature = "3d")]
    pub(crate) fn take_projected_triangles_scratch(
        &mut self,
    ) -> Vec<crate::render::mesh3d::ProjectedTriangle> {
        core::mem::take(&mut self.projected_triangles_scratch)
    }

    /// Returns the projected-triangle scratch for the next mesh draw.
    #[cfg(feature = "3d")]
    pub(crate) fn return_projected_triangles_scratch(
        &mut self,
        scratch: Vec<crate::render::mesh3d::ProjectedTriangle>,
    ) {
        self.projected_triangles_scratch = scratch;
    }

    /// Takes the reusable foreground capture buffer (G-06).
    pub(crate) fn take_layer_foreground_scratch(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.buffer.layer_foreground_scratch)
    }

    /// Returns a foreground capture buffer; its capacity is retained for the next pop (G-06).
    pub(crate) fn return_layer_foreground_scratch(&mut self, buffer: Vec<u8>) {
        self.buffer.layer_foreground_scratch = buffer;
    }

    /// Takes a reusable backdrop buffer, or an empty one when the pool is empty (G-06).
    pub(crate) fn take_layer_backdrop(&mut self) -> Vec<u8> {
        self.buffer.layer_backdrop_pool.pop().unwrap_or_default()
    }

    /// Returns a backdrop buffer to the pool, dropping it when the pool is full (G-06).
    pub(crate) fn return_layer_backdrop(&mut self, buffer: Vec<u8>) {
        if self.buffer.layer_backdrop_pool.len() < LAYER_BACKDROP_POOL_LIMIT {
            self.buffer.layer_backdrop_pool.push(buffer);
        }
    }

    /// Rasterises `text` rotated about `pivot` and composites it.
    ///
    /// The coverage is the reference renderer's own shaped mask, rotated once, so a rotated label
    /// is the same ink and weight as an unrotated one. `origin`, `pivot` and the text metrics are
    /// in the same space [`Self::draw_text`] uses.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_text_rotated_coverage(
        &mut self,
        origin: Point,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
        pivot: Point,
        radians: f32,
    ) {
        let Some(run) = crate::render::text::shaped_text_coverage_rotated(
            origin,
            text,
            font,
            alignment,
            self.dpi_scale(),
            pivot,
            radians,
        ) else {
            return;
        };
        self.blend_text_coverage_mask(
            Rect::new(run.x, run.y, run.width, run.height),
            &run.bytes,
            color,
        );
    }

    /// Composites one pre-rasterised coverage mask over the frame, multiplied by `color`.
    ///
    /// The bytes are the display-encoded alpha the reference compositor would have used, so this
    /// multiplies them by the colour directly instead of running them through
    /// `glyph_coverage_weight` a second time. The clip and the frame bounds are resolved into a
    /// visible span once, exactly as the glyph path does.
    ///
    /// Returns whether any pixel was written.
    pub(crate) fn blend_text_coverage_mask(
        &mut self,
        rect: Rect,
        coverage: &[u8],
        color: Color,
    ) -> bool {
        let size = self.buffer.size();
        if rect.width == 0 || rect.height == 0 || size.width == 0 || size.height == 0 {
            return false;
        }
        let (clip_x, clip_y, clip_w, clip_h) =
            self.current_clip().unwrap_or((0, 0, size.width, size.height));
        let clip_right = clip_x.saturating_add(clip_w as i32);
        let clip_bottom = clip_y.saturating_add(clip_h as i32);
        let mut wrote = false;
        for row in 0..rect.height {
            let y = rect.y + row as i32;
            if y < clip_y || y >= clip_bottom || y < 0 || y >= size.height as i32 {
                continue;
            }
            for col in 0..rect.width {
                let index = row as usize * rect.width as usize + col as usize;
                let Some(&coverage_byte) = coverage.get(index) else { break };
                if coverage_byte == 0 {
                    continue;
                }
                let x = rect.x + col as i32;
                if x < clip_x || x >= clip_right || x < 0 || x >= size.width as i32 {
                    continue;
                }
                crate::render::pipeline::blend_pixel(
                    &mut self.buffer.back,
                    size.width,
                    x as u32,
                    y as u32,
                    color,
                    f32::from(coverage_byte) / 255.0,
                );
                wrote = true;
            }
        }
        wrote
    }
}
