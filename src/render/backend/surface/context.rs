use super::*;

/// How many `push_offset` levels a [`RenderContext`] saves without allocating.
///
/// A container that translates its child (a navigation stack sliding a page, a scroll area
/// moving its viewport) pushes one level; nesting two or three is ordinary. Eight covers the
/// steady-state depth of every control in this crate while keeping the inline array small.
pub(crate) const OFFSET_STACK_INLINE: usize = 8;
/// Render context for custom widget drawing.
///
/// Handed to widget draw code and wrapping a [`PaintBackend`]. Every primitive
/// is translated by the context's current offset *before* being forwarded to
/// the backend, which lets containers translate their children without
/// mutating child geometry. Unless stated otherwise, coordinates are physical
/// pixels in surface space and each draw call is clipped by whatever clip
/// rectangle is currently active on the backend.
pub struct RenderContext<'a> {
    pub(crate) backend: &'a mut dyn PaintBackend,
    /// Current translation applied to all subsequent draw primitives.
    pub(crate) offset_x: i32,
    pub(crate) offset_y: i32,
    /// Saved offsets for the first [`OFFSET_STACK_INLINE`] nesting levels.
    ///
    /// A context is built per frame, so an empty `Vec` for the saves allocated on the first
    /// `push_offset` of *every* frame — one allocation per frame for any control that translates
    /// a child (a navigation stack slides its pages, a scroll area moves its viewport). Shallow
    /// nesting is the whole steady-state case, so those saves live inline and the heap is only
    /// touched by a tree deeper than`OFFSET_STACK_INLINE`.
    pub(crate) offset_inline: [(i32, i32); OFFSET_STACK_INLINE],
    /// How many [`RenderContext::push_offset`] calls are active.
    pub(crate) offset_depth: usize,
    /// Saved offsets for nesting deeper than [`OFFSET_STACK_INLINE`].
    pub(crate) offset_stack: Vec<(i32, i32)>,
    /// When set, draw commands are recorded into control-level groups instead of being sent to
    /// the backend immediately. [`RenderContext::end_region_buckets`] replays them per region.
    pub(crate) buckets: Option<RegionBuckets>,
    /// How many region clips the last bucket flush issued, for tests and diagnostics.
    pub(crate) last_region_passes: usize,
}

/// One recorded control: the box it painted plus the commands it emitted, in order.
pub(crate) struct RegionGroup {
    /// The control's device-space bounds, or `None` when the command's own bounds are used.
    pub(crate) bounds: Option<Rect>,
    /// Commands emitted by the control, in paint order.
    pub(crate) commands: Vec<RenderCommand>,
}

/// A set of damage regions plus the control groups to replay into each one.
///
/// This is the data the one-traversal region pass needs: the widget tree is walked once and every
/// emitted control is stored with its bounds, so a later flush can replay a control into exactly
/// the regions it touches. Before this, the driver walked the tree once per region (G-04).
pub(crate) struct RegionBuckets {
    /// Device-space damage rectangles. The tracker has already merged them, so they are disjoint.
    pub(crate) regions: Vec<Rect>,
    /// Recorded controls in emission order.
    pub(crate) groups: Vec<RegionGroup>,
    /// The group index each open [`RenderContext::begin_control`] writes into.
    pub(crate) open_groups: Vec<usize>,
    /// Groups whose commands have already been replayed.
    ///
    /// A traversal records one group per ungrouped command and one per control, so a frame with
    /// many small draws used to allocate a fresh group for each of them every time. Returning a
    /// group here keeps its `commands` vector's capacity, and the next `record`/`begin_control`
    /// reuses it (G-03).
    pub(crate) free_groups: Vec<RegionGroup>,
    /// Whether a traversal is in progress.
    ///
    /// The buckets stay owned by the context between traversals so the free-group pool survives
    /// to the next frame; `dispatch` only records while this is true (G-03).
    pub(crate) active: bool,
    /// Frame arena for path vertices (G-802).
    ///
    /// Recording a path copies its translated vertices here instead of allocating an `Arc<[Point]>`
    /// per command; `flush` resolves the recorded slice and calls the backend by reference. The
    /// arena is cleared at the start of each traversal and keeps its capacity between frames, so a
    /// steady frame records paths without allocating.
    pub(crate) path_arena: Vec<Point>,
    /// Frame arena for packed contour ranges; indices are relative to `path_arena`.
    pub(crate) contour_arena: Vec<(usize, usize)>,
    /// Frame arena for recorded label bytes.
    pub(crate) text_arena: String,
}

impl RegionBuckets {
    fn new(regions: Vec<Rect>) -> Self {
        Self {
            regions,
            groups: Vec::new(),
            open_groups: Vec::new(),
            free_groups: Vec::new(),
            active: true,
            path_arena: Vec::new(),
            contour_arena: Vec::new(),
            text_arena: String::new(),
        }
    }

    /// Starts a new traversal on an existing bucket set, retaining its pooled groups (G-03) and
    /// arena capacity (G-802).
    fn reset(&mut self, regions: &[Rect], offset: (i32, i32)) {
        // Translated in place: the caller used to build a second `Vec` per bucketed frame, only for
        // this method to drop it on assignment.
        self.regions.clear();
        self.regions.extend(regions.iter().map(|region| {
            Rect::new(region.x + offset.0, region.y + offset.1, region.width, region.height)
        }));
        self.recycle_groups();
        self.path_arena.clear();
        self.contour_arena.clear();
        self.text_arena.clear();
        self.active = true;
    }

    /// Records a path whose vertices go into the frame arena (G-802).
    fn record_path(
        &mut self,
        points: &[Point],
        offset: (i32, i32),
        closed: bool,
        color: Color,
        filled: bool,
        width: u32,
    ) {
        let start = self.path_arena.len();
        self.path_arena
            .extend(points.iter().map(|point| Point::new(point.x + offset.0, point.y + offset.1)));
        let bounds =
            crate::render::core::command::bounds_of_points(&self.path_arena[start..], width);
        self.record(RenderCommand::DrawPathRecorded {
            start,
            len: points.len(),
            bounds,
            closed,
            color,
            filled,
            width,
        });
    }

    /// Records a dashed path whose vertices go into the frame arena (G-802).
    fn record_path_dashed(
        &mut self,
        points: &[Point],
        offset: (i32, i32),
        closed: bool,
        color: Color,
        width: u32,
        dash: crate::render::core::command::DashPattern,
    ) {
        let start = self.path_arena.len();
        self.path_arena
            .extend(points.iter().map(|point| Point::new(point.x + offset.0, point.y + offset.1)));
        let bounds =
            crate::render::core::command::bounds_of_points(&self.path_arena[start..], width);
        self.record(RenderCommand::DrawPathDashedRecorded {
            start,
            len: points.len(),
            bounds,
            closed,
            color,
            width,
            dash,
        });
    }

    /// Records a non-zero fill whose points and contours go into the frame arenas (G-802).
    fn record_fill_path_non_zero(
        &mut self,
        points: &[Point],
        contours: &[(usize, usize)],
        offset: (i32, i32),
        color: Color,
    ) -> Result<(), crate::render::CompoundPathError> {
        crate::render::CompoundPath::validate_ranges(points.len(), contours)?;
        let point_start = self.path_arena.len();
        self.path_arena
            .extend(points.iter().map(|point| Point::new(point.x + offset.0, point.y + offset.1)));
        let contour_start = self.contour_arena.len();
        self.contour_arena.extend_from_slice(contours);
        let bounds =
            crate::render::core::command::bounds_of_points(&self.path_arena[point_start..], 0);
        self.record(RenderCommand::FillPathNonZeroRecorded {
            point_start,
            point_len: points.len(),
            contour_start,
            contour_len: contours.len(),
            bounds,
            color,
        });
        Ok(())
    }

    /// Records a label whose bytes go into the frame text arena (G-802).
    #[allow(clippy::too_many_arguments)]
    fn record_text(
        &mut self,
        origin: Point,
        text: &str,
        font: &crate::core::Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) {
        let text_start = self.text_arena.len();
        self.text_arena.push_str(text);
        let bounds = crate::render::core::command::bounds_of_text(origin, text, font, alignment);
        self.record(RenderCommand::DrawTextRecorded {
            origin,
            text_start,
            text_len: text.len(),
            bounds,
            font: font.clone(),
            color,
            alignment,
        });
    }

    /// Recycles every recorded group, keeping the capacity of its command vector (G-03).
    fn recycle_groups(&mut self) {
        for mut group in self.groups.drain(..) {
            group.bounds = None;
            group.commands.clear();
            self.free_groups.push(group);
        }
        self.open_groups.clear();
    }

    /// Takes a group from the free pool (or a fresh one) with `bounds` and no commands.
    fn take_group(&mut self, bounds: Option<Rect>) -> RegionGroup {
        let mut group =
            self.free_groups.pop().unwrap_or(RegionGroup { bounds: None, commands: Vec::new() });
        group.bounds = bounds;
        group.commands.clear();
        group
    }

    fn begin_control(&mut self, bounds: Rect) {
        let index = self.groups.len();
        let group = self.take_group(Some(bounds));
        self.groups.push(group);
        self.open_groups.push(index);
    }

    fn end_control(&mut self) {
        let Some(child) = self.open_groups.pop() else {
            return;
        };
        // A nested control's commands become part of its parent's group, because the parent's
        // bounds already contain them and replaying the parent as one unit keeps emission order.
        let Some(&parent) = self.open_groups.last() else {
            return;
        };
        if parent == child {
            return;
        }
        // The child is the last group pushed while it was open, so popping it keeps the parent's
        // index stable. `append` moves the commands without allocating, and the (now empty) child
        // vector keeps its capacity in the free pool (G-03).
        if child + 1 == self.groups.len() {
            let mut child_group =
                self.groups.pop().unwrap_or(RegionGroup { bounds: None, commands: Vec::new() });
            self.groups[parent].commands.append(&mut child_group.commands);
            child_group.bounds = None;
            child_group.commands.clear();
            self.free_groups.push(child_group);
            return;
        }
        // Defensive fallback for an out-of-order close; it preserves the old behaviour.
        let mut commands = core::mem::take(&mut self.groups[child].commands);
        self.groups[parent].commands.append(&mut commands);
    }

    fn record(&mut self, command: RenderCommand) {
        if let Some(&index) = self.open_groups.last() {
            self.groups[index].commands.push(command);
            return;
        }
        let bounds = command.bounds();
        let mut group = self.take_group(bounds);
        group.commands.push(command);
        self.groups.push(group);
    }

    /// Replays every group into each region it intersects, and returns the number of region clips
    /// issued.
    fn flush(&mut self, backend: &mut dyn PaintBackend) -> usize {
        for region in &self.regions {
            backend.execute_command(&RenderCommand::PushClip {
                x: region.x,
                y: region.y,
                width: region.width,
                height: region.height,
            });
            for group in &self.groups {
                let touches = group.bounds.is_none_or(|bounds| bounds.intersects(region));
                if !touches {
                    continue;
                }
                for command in &group.commands {
                    match command {
                        RenderCommand::DrawPathRecorded {
                            start,
                            len,
                            closed,
                            color,
                            filled,
                            width,
                            ..
                        } => backend.draw_path_ref(
                            &self.path_arena[*start..*start + *len],
                            *closed,
                            *color,
                            *filled,
                            *width,
                        ),
                        RenderCommand::DrawPathDashedRecorded {
                            start,
                            len,
                            closed,
                            color,
                            width,
                            dash,
                            ..
                        } => backend.draw_path_dashed_ref(
                            &self.path_arena[*start..*start + *len],
                            *closed,
                            *color,
                            *width,
                            *dash,
                        ),
                        RenderCommand::FillPathNonZeroRecorded {
                            point_start,
                            point_len,
                            contour_start,
                            contour_len,
                            color,
                            ..
                        } => backend.fill_path_non_zero_ref(
                            &self.path_arena[*point_start..*point_start + *point_len],
                            &self.contour_arena[*contour_start..*contour_start + *contour_len],
                            *color,
                        ),
                        RenderCommand::DrawTextRecorded {
                            origin,
                            text_start,
                            text_len,
                            font,
                            color,
                            alignment,
                            ..
                        } => backend.draw_text(
                            *origin,
                            &self.text_arena[*text_start..*text_start + *text_len],
                            font,
                            *color,
                            *alignment,
                        ),
                        other => backend.execute_command(other),
                    }
                }
            }
            backend.execute_command(&RenderCommand::PopClip);
        }
        let passes = self.regions.len();
        self.recycle_groups();
        passes
    }
}
impl<'a> RenderContext<'a> {
    /// Wraps `backend` in a context whose translation offset starts at `(0, 0)`.
    ///
    /// The context borrows the backend mutably; it does not take ownership.
    pub fn new(backend: &'a mut dyn PaintBackend) -> Self {
        Self {
            backend,
            offset_x: 0,
            offset_y: 0,
            offset_inline: [(0, 0); OFFSET_STACK_INLINE],
            offset_depth: 0,
            offset_stack: Vec::new(),
            buckets: None,
            last_region_passes: 0,
        }
    }

    /// Whether a region-bucket traversal is currently recording commands.
    ///
    /// The bucket set is retained between traversals so its free-group pool survives (G-03), so
    /// the `Option` being `Some` is not the same as a traversal being active.
    fn bucket_recording(&self) -> bool {
        self.buckets.as_ref().is_some_and(|buckets| buckets.active)
    }

    /// Sends one command to the backend, or records it when region bucketing is active.
    fn dispatch(&mut self, command: &RenderCommand) {
        match self.buckets.as_mut() {
            Some(buckets) if buckets.active => buckets.record(command.clone()),
            _ => self.backend.execute_command(command),
        }
    }

    /// The owned form of [`Self::dispatch`], which an owning caller can hand over without a clone.
    fn dispatch_owned(&mut self, command: RenderCommand) {
        match self.buckets.as_mut() {
            Some(buckets) if buckets.active => buckets.record(command),
            _ => self.backend.execute_command(&command),
        }
    }

    /// Starts recording the traversal into `regions` instead of painting it immediately.
    ///
    /// Every draw command issued until [`Self::end_region_buckets`] is stored with the bounds of
    /// the control that emitted it. The end call then replays each control into the regions it
    /// touches, so the tree is walked **once** regardless of how many regions are damaged (G-04).
    ///
    /// Regions are in the caller's coordinate space and are translated by the context's current
    /// offset, exactly like a clip or a primitive.
    pub fn begin_region_buckets(&mut self, regions: &[Rect]) {
        // Reuse the bucket set when one exists so its free-group pool carries over to this
        // traversal; a nested traversal still replaces the in-progress one (G-03).
        let buckets = self.buckets.get_or_insert_with(|| RegionBuckets::new(Vec::new()));
        buckets.reset(regions, (self.offset_x, self.offset_y));
        self.last_region_passes = 0;
    }

    /// Flushes a bucket traversal recorded by [`Self::begin_region_buckets`].
    ///
    /// A no-op when no traversal is active. Nested bucket traversals are not supported; a caller
    /// that starts one while another is active replaces it.
    pub fn end_region_buckets(&mut self) {
        let Some(buckets) = self.buckets.as_mut() else {
            return;
        };
        if !buckets.active {
            return;
        }
        buckets.active = false;
        self.last_region_passes = buckets.flush(self.backend);
    }

    /// Opens a control group so every command until [`Self::end_control`] is bucketed together.
    ///
    /// A container that draws its children in its own `draw` body should bracket each child with
    /// this pair; a control that does not call it is still bucketed, at its commands' own bounds.
    /// Calling it outside a bucket traversal is a no-op.
    pub fn begin_control(&mut self, bounds: Rect) {
        let bounds = self.offset_rect(bounds);
        if let Some(buckets) = self.buckets.as_mut() {
            if buckets.active {
                buckets.begin_control(bounds);
            }
        }
    }

    /// Closes the innermost control group. A no-op when none is open or bucket mode is off.
    pub fn end_control(&mut self) {
        if let Some(buckets) = self.buckets.as_mut() {
            if buckets.active {
                buckets.end_control();
            }
        }
    }

    /// The number of region clips the last [`Self::end_region_buckets`] issued.
    ///
    /// Zero when no bucket pass has run. This is the observable that distinguishes "painted the
    /// three damaged regions" from "repainted the whole frame", and it is what the dirty-region
    /// tests assert on.
    pub fn region_bucket_passes(&self) -> usize {
        self.last_region_passes
    }
    /// Returns the wrapped backend, for operations not exposed by the context.
    ///
    /// Commands issued directly on the backend bypass this context's
    /// translation offset and must be offset by the caller if required.
    pub fn backend(&mut self) -> &mut dyn PaintBackend {
        self.backend
    }
    /// Returns the target surface size in physical pixels.
    pub fn size(&self) -> Size {
        self.backend.size()
    }
    /// Returns the backend's logical DPI scale; `1.0` means unscaled.
    pub fn dpi_scale(&self) -> f32 {
        self.backend.dpi_scale()
    }
    /// Returns the currently active translation offset `(dx, dy)`.
    pub fn offset(&self) -> (i32, i32) {
        (self.offset_x, self.offset_y)
    }
    /// Pushes a translation offset that is applied to every draw primitive
    /// (including nested clips) until the matching [`RenderContext::pop_offset`]
    /// restores the previous offset. Offsets may be nested.
    ///
    /// Used by viewport containers such as `ScrollArea` to translate child
    /// content by the negative scroll position.
    pub fn push_offset(&mut self, dx: i32, dy: i32) {
        let saved = (self.offset_x, self.offset_y);
        if self.offset_depth < OFFSET_STACK_INLINE {
            self.offset_inline[self.offset_depth] = saved;
        } else {
            self.offset_stack.push(saved);
        }
        self.offset_depth += 1;
        self.offset_x += dx;
        self.offset_y += dy;
    }
    /// Restores the translation offset in effect before the matching
    /// [`RenderContext::push_offset`]. No-op when no offset is pushed.
    pub fn pop_offset(&mut self) {
        if self.offset_depth == 0 {
            return;
        }
        self.offset_depth -= 1;
        let previous = if self.offset_depth < OFFSET_STACK_INLINE {
            self.offset_inline[self.offset_depth]
        } else {
            self.offset_stack.pop().unwrap_or((0, 0))
        };
        self.offset_x = previous.0;
        self.offset_y = previous.1;
    }
    fn offset_rect(&self, rect: Rect) -> Rect {
        let mut translated = rect;
        translated.x += self.offset_x;
        translated.y += self.offset_y;
        translated
    }
    fn offset_point(&self, point: Point) -> Point {
        Point::new(point.x + self.offset_x, point.y + self.offset_y)
    }
    fn offset_points(&self, points: &[Point]) -> Vec<Point> {
        points.iter().map(|point| self.offset_point(*point)).collect()
    }
    /// Fills `rect` with a solid `color`, ignoring any alpha blending
    /// limitations of the backend's fill primitive.
    ///
    /// `rect` is translated by the current offset before drawing.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::FillRect { rect, color });
    }

    /// Draws the shadow behind a raised face.
    ///
    /// The command, not a plain rectangle: `BoxShadow` is the one primitive both backends already
    /// implement with a blur (the software backend runs a three-pass box blur over the region, the
    /// SVG backend emits a `feGaussianBlur` filter), and `SurfaceShadow::blur` was written by the
    /// theme and then **dropped** at this point — every raised face cast an unblurred rectangle of
    /// its own size, which reads as a 6 px hard bar under a control rather than as a shadow.
    ///
    /// `rect` is the face's own rectangle; `offset_x`/`offset_y` move the shadow behind it and
    /// `spread` grows (or shrinks) it. The colour's alpha is the shadow's weight — see the
    /// backends' `BoxShadow` arm for why the command no longer halves it.
    pub fn draw_box_shadow(
        &mut self,
        rect: Rect,
        color: Color,
        offset_x: i32,
        offset_y: i32,
        blur_radius: u32,
        spread: i32,
    ) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::BoxShadow {
            rect,
            color,
            offset_x,
            offset_y,
            blur_radius,
            spread,
        });
    }

    /// Draws the shadow behind a rounded face (G-016).
    ///
    /// The blur source is the rounded rectangle itself — the corner `radii`
    /// belong to the shadow caster — so a rounded card no longer spills square
    /// shadow corners past its outline. The offset/spread/blur parameters mean
    /// exactly what [`Self::draw_box_shadow`]'s mean; `radii` is resolved per CSS
    /// `border-radius` before use. A face with all-zero radii should call
    /// [`Self::draw_box_shadow`], which emits the historical command unchanged.
    pub fn draw_box_shadow_rounded(
        &mut self,
        rect: Rect,
        radii: [u32; 4],
        color: Color,
        offset_x: i32,
        offset_y: i32,
        blur_radius: u32,
        spread: i32,
    ) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::BoxShadowRounded {
            rect,
            radii,
            color,
            offset_x,
            offset_y,
            blur_radius,
            spread,
        });
    }

    /// Fills `rect` with a `gradient`.
    ///
    /// # Why this is a separate call rather than a flag on `fill_rect`
    ///
    /// A gradient is not "a colour with an extra" — it is a different paint server, and the two
    /// backends route it differently (`SoftwareSurface::fill_rect_gradient` versus the SVG backend's
    /// `<linearGradient>`/`<radialGradient>`/conic fan). Keeping it a distinct call is what lets the
    /// two stay honest about what they can express, and lets a caller fall back to a solid fill when
    /// a gradient is absent.
    ///
    /// `rect` is translated by the current offset before drawing.
    pub fn fill_gradient(&mut self, rect: Rect, gradient: &crate::style::Gradient) {
        let rect = self.offset_rect(rect);
        // Straight to the backend, which may paint it without owning a copy; a bucket traversal
        // needs an owning handle, and an immutable gradient is shared by refcount (G-12).
        if self.bucket_recording() {
            self.dispatch_owned(RenderCommand::DrawGradient {
                rect,
                gradient: crate::compat::Arc::new(gradient.clone()),
            });
        } else {
            self.backend.fill_gradient(rect, gradient);
        }
    }

    /// Paints a control's background: the `gradient` when the style declares one, otherwise the
    /// solid `color`.
    ///
    /// # Why this exists
    ///
    /// `WidgetStyle::background_gradient` was settable, mergeable and CSS-expressible, but **no
    /// painter ever read it**: a caller that set a gradient got a solid `background_color` (or the
    /// control's own default) and the gradient was silently dropped. This is the one-line call that
    /// closes that gap, so a control draws the gradient it was given and falls back to the colour it
    /// always drew. The identity case is `fill_rect`: a style with no gradient emits exactly what it
    /// did before.
    pub fn fill_background(
        &mut self,
        rect: Rect,
        color: Color,
        gradient: Option<&crate::style::Gradient>,
    ) {
        match gradient {
            Some(gradient) => self.fill_gradient(rect, gradient),
            None => self.fill_rect(rect, color),
        }
    }
    /// Draws a one-pixel-wide outline of `rect` (alias-aliased) in `color`.
    ///
    /// `rect` is translated by the current offset before drawing.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::DrawRect { rect, color });
    }
    /// Draws an outline of `rect` in `color` with the given stroke `width` in
    /// pixels.
    ///
    /// `rect` is translated by the current offset before drawing.
    pub fn draw_rect_stroke(&mut self, rect: Rect, color: Color, width: u32) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::DrawRectStroke { rect, color, width });
    }
    /// Fills `rect` with `color` using rounded corners of `radius` pixels.
    ///
    /// The non-anti-aliased variant; use [`RenderContext::fill_rounded_rect_aa`]
    /// for smoother edges. `radius` is typically clamped by the backend to half
    /// the shorter side of `rect`.
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::FillRoundedRect { rect, radii: [radius; 4], color });
    }

    /// [`Self::fill_rounded_rect`] with per-corner radii (STY-01).
    ///
    /// `radii` is ordered `[top_left, top_right, bottom_right, bottom_left]`,
    /// matching [`crate::style::CornerRadii`]'s field order. Four equal values
    /// produce exactly the command the scalar method emits.
    pub fn fill_rounded_rect_corners(&mut self, rect: Rect, radii: [u32; 4], color: Color) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::FillRoundedRect { rect, radii, color });
    }

    /// Paints a control's own **face**: its shadow, then its fill.
    ///
    /// # Why this exists beside `fill_rect`
    ///
    /// A control's face is not "a rectangle of colour". It is a rectangle whose *shape* and
    /// *depth* come from the theme: a `border_radius` the theme declares, a material (opaque or
    /// tinted glass), and an elevation whose shadow sits behind it. Writing
    /// `fill_rect(rect, bg)` states only the colour, so the other three were unreachable --
    /// measured, **88 controls** declared a radius the theme never gave them and **no control at
    /// all** ever drew the shadow its role resolved (BLUE24 §10A).
    ///
    /// This is the one-line replacement for that call. Its **identity case is `fill_rect`**: a
    /// solid face of radius 0 at elevation 0 emits exactly `FillRect`, so a control that has not
    /// been taught about surfaces draws byte-for-byte what it did before.
    ///
    /// # Why it takes the two resolved values rather than the style
    ///
    /// Because a call site has them and taking `&WidgetStyle` would not borrow-check at every one:
    /// some `draw` bodies hold the style by value, some by reference, and some only in an inner
    /// scope. `SurfaceStyle` and `u32` are both `Copy`, so passing them cannot conflict with a
    /// borrow the caller is still using -- which is what makes this a **mechanical** rewrite
    /// rather than one that has to reason about each control's borrows.
    ///
    /// The **outline is deliberately not drawn here**: a control's edge is often a semantic colour
    /// (a validation error, a checked state) that lives in the control rather than in the face, so
    /// a control keeps its own `draw_rect_stroke` call. [`crate::render::SurfaceStyle::paint`] is
    /// the fuller form for a control that wants the bevel and the edge derived too.
    ///
    /// `shadow_tint` is the shadow's hue; each elevation level owns its own alpha.
    ///
    /// This is [`Self::face_with_gradient`] with no gradient: it is the identity case for a face
    /// that declares a solid fill, so a control calling it emits exactly what it always did.
    pub fn face(
        &mut self,
        rect: Rect,
        fill: Color,
        surface: crate::render::SurfaceStyle,
        radius: u32,
        shadow_tint: Color,
    ) {
        self.face_with_gradient(rect, fill, None, surface, radius, shadow_tint);
    }

    /// [`Self::face`] with per-corner radii (STY-01).
    ///
    /// `radii` is ordered `[top_left, top_right, bottom_right, bottom_left]`,
    /// matching [`crate::style::CornerRadii`] — pass
    /// `style.resolved_corner_radii().to_array()`. Four equal values emit exactly
    /// the command [`Self::face`] emits.
    pub fn face_corners(
        &mut self,
        rect: Rect,
        fill: Color,
        surface: crate::render::SurfaceStyle,
        radii: [u32; 4],
        shadow_tint: Color,
    ) {
        self.face_with_gradient_corners(rect, fill, None, surface, radii, shadow_tint);
    }

    /// [`Self::face`], but the fill is a `gradient` when the style declares one.
    ///
    /// # Why this exists beside `face`
    ///
    /// `WidgetStyle::background_gradient` was settable, mergeable and CSS-expressible, but only
    /// `Button` ever read it: every other control painted a solid `background_color` and silently
    /// dropped a declared gradient (issue §9 item #2). The gradient is a paint server rather than a
    /// colour, so it cannot be folded into `fill` (a [`Color`]); passing it as a separate
    /// `Option` keeps the identity exact — `None` emits precisely the solid face `face` always did,
    /// so a control that declares no gradient is unchanged.
    ///
    /// The shadow, the materialised fill and the radius are identical to `face`; only the fill step
    /// differs.
    ///
    /// # Why a rounded gradient is still a rectangular ramp
    ///
    /// A gradient is a paint server with no corner rounding. The command set does
    /// now carry a rounded clip (G-015) that could intersect the ramp with, but
    /// turning it on here changes every gradient face's rendered edge, which is a
    /// snapshot decision rather than a mechanical one — so the ramp still spans
    /// the full rectangle and the declared `radius` is honoured by the face's
    /// **outline** (which the control draws itself), exactly as
    /// [`SurfaceStyle::paint_with_gradient`](crate::render::SurfaceStyle::paint_with_gradient) does
    /// for a button. That keeps one gradient story in the crate rather than two.
    ///
    /// # Which controls route through here, and which do not
    ///
    /// Every control that paints its **own background face** calls this helper — the 100-plus call
    /// sites that used to call `face` (`label`, `frame`, the dialogs, the view widgets, `lineedit`,
    /// `textedit`, `listbox`, `combobox`, …). A handful of controls deliberately do **not**, and the
    /// rule is the same one principle #51 states: share only where the concept is actually shared.
    ///
    /// * `checkbox` / `radiobutton` paint only their small checked **indicator**, not a
    ///   control-sized background, so a `background_gradient` has no face to apply to.
    /// * `slider` / `progressbar` paint a centred **track band** inside their rectangle, not the
    ///   rectangle as a face; a widget-level gradient applied to a 4-pixel groove would be the wrong
    ///   shape rather than the right one.
    ///
    /// Those four keep their existing `fill_rect` / `fill_rounded_rect` calls. A future control
    /// whose background really is a face inherits gradient and radius by calling this helper, which
    /// is the point of the shared path.
    pub fn face_with_gradient(
        &mut self,
        rect: Rect,
        fill: Color,
        gradient: Option<&crate::style::Gradient>,
        surface: crate::render::SurfaceStyle,
        radius: u32,
        shadow_tint: Color,
    ) {
        self.face_with_gradient_corners(rect, fill, gradient, surface, [radius; 4], shadow_tint);
    }

    /// [`Self::face_with_gradient`] with per-corner radii (STY-01).
    ///
    /// The one-line face replacement for a control whose style can express
    /// distinct corners: pass `style.resolved_corner_radii().to_array()` and the
    /// face, its rounded fill and its shadow all follow the resolved shape. Four
    /// equal values emit exactly the commands the scalar form has always emitted,
    /// byte for byte.
    pub fn face_with_gradient_corners(
        &mut self,
        rect: Rect,
        fill: Color,
        gradient: Option<&crate::style::Gradient>,
        surface: crate::render::SurfaceStyle,
        radii: [u32; 4],
        shadow_tint: Color,
    ) {
        // 1. The shadow, behind the face. `draws_shadow` is false at `Elevation::Flat`, so a face
        //    on the flat preset emits nothing here.
        if let Some(shadow) = surface.shadow(shadow_tint) {
            // The box-shadow command carries the blur the theme declared; a plain fill dropped it.
            // A rounded caster carries its radii through (G-016) so the blur source is the rounded
            // shape, not the bounding box; a square face keeps the historical command.
            if radii.iter().any(|r| *r > 0) {
                self.draw_box_shadow_rounded(
                    rect,
                    radii,
                    shadow.color,
                    shadow.x,
                    shadow.y,
                    shadow.blur,
                    0,
                );
            } else {
                self.draw_box_shadow(rect, shadow.color, shadow.x, shadow.y, shadow.blur, 0);
            }
        }
        // 2. The fill. Without a gradient this is the solid, radius-clipped face `face` always
        //    drew; with one the ramp spans the full rectangle, matching `paint_with_gradient`.
        let materialised = surface.apply_fill(fill);
        match gradient {
            Some(gradient) => self.fill_gradient(rect, gradient),
            None => {
                if radii.iter().any(|r| *r > 0) {
                    self.fill_rounded_rect_corners(rect, radii, materialised);
                } else {
                    self.fill_rect(rect, materialised);
                }
            }
        }
    }
    /// Anti-aliased equivalent of [`RenderContext::fill_rounded_rect`].
    ///
    /// Costs more per call: the corners are sampled on the surface's
    /// configured `aa_samples_per_axis` grid.
    pub fn fill_rounded_rect_aa(&mut self, rect: Rect, radius: u32, color: Color) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::FillRoundedRectAA { rect, radii: [radius; 4], color });
    }

    /// Anti-aliased [`Self::fill_rounded_rect_corners`].
    pub fn fill_rounded_rect_corners_aa(&mut self, rect: Rect, radii: [u32; 4], color: Color) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::FillRoundedRectAA { rect, radii, color });
    }
    /// Strokes the rounded-rectangle outline of `rect` in `color` with the
    /// given `width` in pixels.
    ///
    /// The non-anti-aliased variant; use
    /// [`RenderContext::draw_rounded_rect_stroke_aa`] for smoother edges.
    pub fn draw_rounded_rect_stroke(&mut self, rect: Rect, radius: u32, color: Color, width: u32) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::DrawRoundedRectStroke {
            rect,
            radii: [radius; 4],
            color,
            width,
        });
    }

    /// [`Self::draw_rounded_rect_stroke`] with per-corner radii (STY-01).
    pub fn draw_rounded_rect_stroke_corners(
        &mut self,
        rect: Rect,
        radii: [u32; 4],
        color: Color,
        width: u32,
    ) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::DrawRoundedRectStroke { rect, radii, color, width });
    }
    /// Anti-aliased equivalent of
    /// [`RenderContext::draw_rounded_rect_stroke`].
    pub fn draw_rounded_rect_stroke_aa(
        &mut self,
        rect: Rect,
        radius: u32,
        color: Color,
        width: u32,
    ) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::DrawRoundedRectStrokeAA {
            rect,
            radii: [radius; 4],
            color,
            width,
        });
    }

    /// Anti-aliased [`Self::draw_rounded_rect_stroke_corners`].
    pub fn draw_rounded_rect_stroke_corners_aa(
        &mut self,
        rect: Rect,
        radii: [u32; 4],
        color: Color,
        width: u32,
    ) {
        let rect = self.offset_rect(rect);
        self.dispatch_owned(RenderCommand::DrawRoundedRectStrokeAA { rect, radii, color, width });
    }

    /// Draws the border a style declares, honouring its line style (G-404/STY-01).
    ///
    /// This is the one entry a control's own border painting should use: it is
    /// where `border-style` becomes ink. `Solid` is the historical stroke;
    /// `None` draws nothing even when a width and colour are set — the
    /// semantics [`crate::style::BorderStyle::None`]'s doc has always promised;
    /// `Dashed` and `Dotted` run the pattern along each edge's **straight**
    /// section only, leaving rounded corner arcs unstroked (a dashed arc needs
    /// an arc primitive the command set does not have — the boundary is
    /// deliberate and tested, not an oversight). A zero-width border draws
    /// nothing, as everywhere else.
    ///
    /// The pattern periods follow the reference toolkits' defaults: a dash is
    /// three times the width with a gap of two; a dot's diameter is the width
    /// and dots sit three radii apart, centre to centre.
    pub fn draw_rounded_rect_border(
        &mut self,
        rect: Rect,
        radii: [u32; 4],
        color: Color,
        width: u32,
        style: crate::style::BorderStyle,
    ) {
        use crate::style::BorderStyle;
        if width == 0 {
            return;
        }
        let rect = self.offset_rect(rect);
        match style {
            BorderStyle::None => {}
            BorderStyle::Solid => {
                self.dispatch_owned(RenderCommand::DrawRoundedRectStroke {
                    rect,
                    radii,
                    color,
                    width,
                });
            }
            BorderStyle::Dashed => self.draw_edge_pattern(rect, radii, color, width, true),
            BorderStyle::Dotted => self.draw_edge_pattern(rect, radii, color, width, false),
        }
    }

    /// The dash/dot worker behind [`Self::draw_rounded_rect_border`].
    ///
    /// Walks each edge's straight section — inset by the corner radii the CSS
    /// resolution produces — emitting [`RenderCommand::DrawLineStroke`] dashes
    /// or [`RenderCommand::FillCircle`] dots. `dashes` picks the pattern. The
    /// rect is already offset, so the commands are dispatched directly rather
    /// than through the offset-applying helpers.
    ///
    /// The dash walk is [`dash_line_segments`], the same kernel
    /// [`RenderCommand::DrawPathDashed`] expands through, so the border's dashes
    /// and a dashed path cannot disagree on where a mark starts (G-014). The
    /// per-edge buffer is reused across the four edges (ENG-B1). A unit
    /// test in this module pins the kernel's output to the historical inline
    /// walk on identical inputs; the dots branch stays inline because a dot is a
    /// disc, not a mark/gap pair.
    fn draw_edge_pattern(
        &mut self,
        rect: Rect,
        radii: [u32; 4],
        color: Color,
        width: u32,
        dashes: bool,
    ) {
        use crate::render::pipeline::rounded_rect_effective_radii;
        let [tl, tr, br, bl] = rounded_rect_effective_radii(rect, radii);
        let left = rect.x;
        let top = rect.y;
        let right = rect.x + rect.width as i32;
        let bottom = rect.y + rect.height as i32;
        let (mark, gap) = if dashes {
            ((width * 3).max(1), (width * 2).max(1))
        } else {
            (0, (width / 2).max(1) * 3)
        };
        let edges = [
            ((left + tl as i32, top), (right - tr as i32, top)),
            ((right, top + tr as i32), (right, bottom - br as i32)),
            ((left + bl as i32, bottom), (right - br as i32, bottom)),
            ((left, top + tl as i32), (left, bottom - bl as i32)),
        ];
        for (from, to) in edges {
            let (from, to) = (Point::new(from.0, from.1), Point::new(to.0, to.1));
            if dashes {
                // One straight edge per kernel call: the pattern restarts at the
                // edge, exactly as the inline loop it replaced did.
                for_each_dash_segment(
                    &[from, to],
                    false,
                    crate::render::core::command::DashPattern { mark, gap },
                    |a, b| {
                        self.dispatch_owned(RenderCommand::DrawLineStroke {
                            from: a,
                            to: b,
                            color,
                            width,
                        });
                    },
                );
            } else {
                let dx = i64::from(to.x - from.x);
                let dy = i64::from(to.y - from.y);
                let length = (dx * dx + dy * dy).isqrt();
                if length <= 0 {
                    continue;
                }
                let point_at = |t: i64| {
                    Point::new(
                        from.x + ((dx * t) / length) as i32,
                        from.y + ((dy * t) / length) as i32,
                    )
                };
                let mut position = 0i64;
                while position < length {
                    self.dispatch_owned(RenderCommand::FillCircle {
                        center: point_at(position),
                        radius: (width / 2).max(1),
                        color,
                    });
                    position += i64::from(gap);
                }
            }
        }
    }
    /// Draws a one-pixel-wide, non-anti-aliased line from `from` to `to` in
    /// `color`.
    ///
    /// Both endpoints are translated by the current offset.
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        let from = self.offset_point(from);
        let to = self.offset_point(to);
        self.dispatch_owned(RenderCommand::DrawLine { from, to, color });
    }
    /// Anti-aliased equivalent of [`RenderContext::draw_line`].
    pub fn draw_line_aa(&mut self, from: Point, to: Point, color: Color) {
        let from = self.offset_point(from);
        let to = self.offset_point(to);
        self.dispatch_owned(RenderCommand::DrawLineAA { from, to, color });
    }
    /// Draws a line from `from` to `to` in `color` with the given `width` in
    /// pixels, without anti-aliasing.
    pub fn draw_line_stroke(&mut self, from: Point, to: Point, color: Color, width: u32) {
        let from = self.offset_point(from);
        let to = self.offset_point(to);
        self.dispatch_owned(RenderCommand::DrawLineStroke { from, to, color, width });
    }
    /// Anti-aliased equivalent of [`RenderContext::draw_line_stroke`].
    pub fn draw_line_stroke_aa(&mut self, from: Point, to: Point, color: Color, width: u32) {
        let from = self.offset_point(from);
        let to = self.offset_point(to);
        self.dispatch_owned(RenderCommand::DrawLineStrokeAA { from, to, color, width });
    }
    /// Draws a stroked line whose ends and corners follow `cap`/`join` (G-017).
    ///
    /// A single segment has no interior corner, so `join` only matters to
    /// polyline callers ([`crate::render::Bevel::stroke`] applies it at shared
    /// vertices); on this segment it is accepted for call-site symmetry.
    ///
    /// # How each cap is expressed
    ///
    /// The command set has no cap parameter on `DrawLineStroke`, so the caps are
    /// expanded into the primitives that already exist, all in this recording
    /// layer:
    ///
    /// * `LineCap::Butt` is the plain stroke — the historical picture, byte for
    ///   byte, which is also what the `Default` impl produces;
    /// * `LineCap::Round` adds one [`RenderCommand::FillCircle`] of radius
    ///   `width / 2` centred on each endpoint, so the stroke ends in half-discs;
    /// * `LineCap::Square` extends the segment by `width / 2` past each
    ///   endpoint along its own direction, so the stroke ends in a square the
    ///   width of the line.
    pub fn draw_line_stroke_styled(
        &mut self,
        from: Point,
        to: Point,
        color: Color,
        width: u32,
        cap: crate::render::core::command::LineCap,
        join: crate::render::core::command::LineJoin,
    ) {
        use crate::render::core::command::LineCap;
        let _ = join; // A join needs a shared vertex; see the doc comment above.
        if width == 0 {
            return;
        }
        match cap {
            LineCap::Butt => {
                self.draw_line_stroke(from, to, color, width);
            }
            LineCap::Round => {
                self.draw_line_stroke(from, to, color, width);
                let radius = width / 2;
                self.fill_circle(from, radius, color);
                self.fill_circle(to, radius, color);
            }
            LineCap::Square => {
                let dx = i64::from(to.x - from.x);
                let dy = i64::from(to.y - from.y);
                let length = (dx * dx + dy * dy).isqrt();
                let extend = i64::from(width / 2);
                if length <= 0 {
                    // A degenerate segment cannot define a direction to extend
                    // along; draw the plain stroke (a dot of the brush) and stop.
                    self.draw_line_stroke(from, to, color, width);
                    return;
                }
                let extended_from = Point::new(
                    from.x - ((dx * extend) / length) as i32,
                    from.y - ((dy * extend) / length) as i32,
                );
                let extended_to = Point::new(
                    to.x + ((dx * extend) / length) as i32,
                    to.y + ((dy * extend) / length) as i32,
                );
                self.draw_line_stroke(extended_from, extended_to, color, width);
            }
        }
    }
    /// Fills a circle centred on `center` with `radius` pixels, without
    /// anti-aliasing.
    ///
    /// The centre is translated by the current offset.
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        let center = self.offset_point(center);
        self.dispatch_owned(RenderCommand::FillCircle { center, radius, color });
    }
    /// Anti-aliased equivalent of [`RenderContext::fill_circle`].
    pub fn fill_circle_aa(&mut self, center: Point, radius: u32, color: Color) {
        let center = self.offset_point(center);
        self.dispatch_owned(RenderCommand::FillCircleAA { center, radius, color });
    }
    /// Draws the one-pixel outline of a circle centred on `center` with
    /// `radius` pixels, without anti-aliasing.
    pub fn draw_circle(&mut self, center: Point, radius: u32, color: Color) {
        let center = self.offset_point(center);
        self.dispatch_owned(RenderCommand::DrawCircle { center, radius, color });
    }
    /// Draws a circular outline of `width` pixels centred on `center` with
    /// `radius` pixels, without anti-aliasing.
    pub fn draw_circle_stroke(&mut self, center: Point, radius: u32, color: Color, width: u32) {
        let center = self.offset_point(center);
        self.dispatch_owned(RenderCommand::DrawCircleStroke { center, radius, color, width });
    }
    /// The shared translated vertex slice for `points` (RD3-05).
    ///
    /// [`Self::draw_path`] already reused an arc for a polygon painted every frame;
    /// [`Self::draw_path_dashed`] rebuilt one unconditionally for the same geometry. Both now use
    /// this lookup, whose key is the complete geometry: the translation, the vertex contents and
    /// the closed flag. Colour, fill, width and the dash pattern are command fields and are not
    /// part of the shared slice, so a solid stroke and its dashed neighbour share one arc. Bounded
    /// by [`PATH_POINT_CACHE_DEPTH`].
    ///
    /// # Cost
    ///
    /// A hit is one `Arc` clone and no allocation. A miss allocates the translated slice (no
    /// intermediate `Vec`) and one vector for the key, because the comparison is against the
    /// vertices as the caller wrote them. The old comment here claimed the miss allocated
    /// nothing at all, which the `points.to_vec()` key contradicted.
    fn translated_path_points(&self, points: &[Point], closed: bool) -> Arc<[Point]> {
        // Linear scan with an exact vertex comparison, bounded by `PATH_POINT_CACHE_DEPTH` (16).
        //
        // R-15 flagged this as a "linear scan + full comparison" hot spot. The bound is what makes
        // the shape right: a frame paints a handful of small static polygons (arrows, chevrons,
        // gauge arcs), so the worst case is 16 comparisons against paths of a few points each. An
        // index keyed by (offset, closed, first point) would trade that for a hash and a second
        // lookup table, and the comparison would still be needed to prove the hit — a net loss at
        // this depth. Revisit only with a measured frame time, per principle #51.
        let offset_x = self.offset_x;
        let offset_y = self.offset_y;
        let cached = PATH_POINT_CACHE.with(|cache| {
            cache
                .borrow()
                .iter()
                .find(|(x, y, vertices, cached_closed, _)| {
                    *x == offset_x
                        && *y == offset_y
                        && *cached_closed == closed
                        && vertices.as_slice() == points
                })
                .map(|(_, _, _, _, arc)| Arc::clone(arc))
        });
        match cached {
            Some(arc) => arc,
            None => {
                // A hit costs no allocation. A miss collects the translated slice directly into
                // the `Arc<[Point]>` the command carries — no intermediate `Vec` for the result —
                // and also allocates one key vector: the cache compares the *untranslated*
                // vertices it was handed, so it must keep a copy of them for the next lookup.
                // That key copy is the price of the comparison being exact rather than
                // positional, and it is paid only on a miss (G-02).
                let arc: Arc<[Point]> =
                    points.iter().map(|point| self.offset_point(*point)).collect();
                PATH_POINT_CACHE.with(|cache| {
                    let mut cache = cache.borrow_mut();
                    if cache.len() >= PATH_POINT_CACHE_DEPTH {
                        cache.remove(0);
                    }
                    cache.push((offset_x, offset_y, points.to_vec(), closed, Arc::clone(&arc)));
                });
                arc
            }
        }
    }
    /// Draws a polyline/polygon path from a list of points.
    ///
    /// When `filled` is true the path is closed implicitly and filled with
    /// `color`; otherwise it is stroked with `width` and closed only when
    /// `closed` is true. Supported by the software (scanline fill) and
    /// SVG backends.
    pub fn draw_path(
        &mut self,
        points: &[Point],
        closed: bool,
        color: Color,
        filled: bool,
        width: u32,
    ) {
        // Recorded paths copy their vertices into the frame arena instead of allocating an
        // `Arc<[Point]>` per command (G-802). The flush resolves the slice and calls the
        // backend by reference.
        if self.bucket_recording() {
            let offset = (self.offset_x, self.offset_y);
            if let Some(buckets) = self.buckets.as_mut() {
                if buckets.active {
                    buckets.record_path(points, offset, closed, color, filled, width);
                    return;
                }
            }
        }
        // A three-vertex path fits inline, so it skips the shared-slice copy entirely (GV4-10).
        // The bucket recorder above already stores vertices in the frame arena, which is why this
        // branch is only on the immediate path.
        if points.len() == 3 {
            self.dispatch_owned(RenderCommand::DrawPath3 {
                a: self.offset_point(points[0]),
                b: self.offset_point(points[1]),
                c: self.offset_point(points[2]),
                closed,
                color,
                filled,
                width,
            });
            return;
        }
        // Collect straight into the shared slice the command now carries, so the immediate path
        // does not allocate an intermediate `Vec` (G-02). The translated slice is cached by its
        // complete geometry, so a polygon that did not move this frame reuses its arc instead of
        // rebuilding the vertices.
        let points = self.translated_path_points(points, closed);
        self.dispatch_owned(RenderCommand::DrawPath { points, closed, color, filled, width });
    }
    /// Strokes a polyline with a repeating dash pattern (G-014).
    ///
    /// The stroke is the same width and colour [`Self::draw_path`] would lay
    /// down, broken into alternating `dash.mark` inked and `dash.gap` open runs
    /// along each segment; a `closed` path also dashes the closing edge. The
    /// pattern restarts at each vertex — the convention the border dash uses per
    /// edge — and both walks share one kernel, so they cannot disagree about
    /// where a mark lands.
    ///
    /// A zero `mark` draws nothing (no ink is possible); a zero `gap` degenerates
    /// to the solid stroke, for which [`Self::draw_path`] is the cheaper spelling.
    pub fn draw_path_dashed(
        &mut self,
        points: &[Point],
        closed: bool,
        color: Color,
        width: u32,
        dash: crate::render::core::command::DashPattern,
    ) {
        if self.bucket_recording() {
            let offset = (self.offset_x, self.offset_y);
            if let Some(buckets) = self.buckets.as_mut() {
                if buckets.active {
                    buckets.record_path_dashed(points, offset, closed, color, width, dash);
                    return;
                }
            }
        }
        let points = self.translated_path_points(points, closed);
        self.dispatch_owned(RenderCommand::DrawPathDashed { points, closed, color, width, dash });
    }
    /// Fills packed contours together using the non-zero winding rule.
    ///
    /// Unlike [`RenderContext::draw_path`], this treats the contours as one shape, preserving
    /// counters (holes) in compound icon outlines.
    pub fn fill_path_non_zero(
        &mut self,
        points: &[Point],
        contours: &[(usize, usize)],
        color: Color,
    ) -> Result<(), crate::render::CompoundPathError> {
        if self.bucket_recording() {
            let offset = (self.offset_x, self.offset_y);
            if let Some(buckets) = self.buckets.as_mut() {
                if buckets.active {
                    return buckets.record_fill_path_non_zero(points, contours, offset, color);
                }
            }
            let translated = self.offset_points(points);
            let path = crate::render::CompoundPath::new(translated, contours.to_vec())?;
            self.dispatch_owned(RenderCommand::FillPathNonZero { path, color });
            Ok(())
        } else {
            self.backend.fill_path_non_zero(points, contours, color, self.offset_x, self.offset_y)
        }
    }
    /// Draws `text` with its origin (baseline start, per `alignment`) at
    /// `origin`, using `font` and `color`.
    ///
    /// The origin is translated by the current offset. `alignment` selects how
    /// the text is positioned horizontally relative to `origin`.
    pub fn draw_text(
        &mut self,
        origin: Point,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) {
        let origin = self.offset_point(origin);
        if self.bucket_recording() {
            if let Some(buckets) = self.buckets.as_mut() {
                if buckets.active {
                    buckets.record_text(origin, text, font, color, alignment);
                    return;
                }
            }
            self.dispatch_owned(RenderCommand::DrawText {
                origin,
                text: crate::compat::Arc::from(text),
                font: font.clone(),
                color,
                alignment,
            });
        } else {
            self.backend.draw_text(origin, text, font, color, alignment);
        }
    }
    /// Draws one line of text rotated about `pivot`.
    ///
    /// `radians` is clockwise in screen coordinates (y grows down). The glyph run, its
    /// measured box and its alignment are exactly [`Self::draw_text`]'s; only the finished
    /// box is rotated, so a caller that already fits a label into a box keeps that box and
    /// asks for the orientation separately.
    pub fn draw_text_rotated(
        &mut self,
        origin: Point,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
        pivot: Point,
        radians: f32,
    ) {
        let origin = self.offset_point(origin);
        let pivot = self.offset_point(pivot);
        if self.bucket_recording() {
            // The region buckets carry the four unrotated text fields; a rotated run keeps
            // the owning command so a bucket replay still paints it.
            self.dispatch_owned(RenderCommand::DrawTextRotated {
                origin,
                text: crate::compat::Arc::from(text),
                font: font.clone(),
                color,
                alignment,
                pivot,
                radians,
            });
        } else {
            self.backend.draw_text_rotated(origin, text, font, color, alignment, pivot, radians);
        }
    }

    /// Draws a single line of text with baseline decorations (G-013).
    ///
    /// The glyphs go down exactly as [`Self::draw_text`] puts them; `decoration`
    /// then asks for an underline, a strikethrough and/or a wavy underline in the
    /// same `color`, drawn by the backend around the shaped run. `TextDecoration::NONE`
    /// emits the plain [`RenderCommand::DrawText`] command, so the default costs
    /// nothing and changes nothing.
    ///
    /// # Why a separate entry point rather than a flag on `draw_text`
    ///
    /// The command variant is additive for the same reason — every existing
    /// construction and destructure site stays valid — and the recording entry
    /// point mirrors that: a caller states decoration only when it wants it.
    pub fn draw_text_decorated(
        &mut self,
        origin: Point,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
        decoration: crate::render::core::command::TextDecoration,
    ) {
        if decoration.is_empty() {
            self.draw_text(origin, text, font, color, alignment);
            return;
        }
        let origin = self.offset_point(origin);
        self.dispatch_owned(RenderCommand::DrawTextDecorated {
            origin,
            text: crate::compat::Arc::from(text),
            font: font.clone(),
            color,
            alignment,
            decoration,
        });
    }
    /// Returns the measured bounds of `text` in `font` under the backend's DPI
    /// scale.
    ///
    /// The result is not affected by the current translation offset, so it is
    /// an unscaled layout measurement and carries no `(x, y)` origin.
    pub fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        self.backend.measure_text(text, font)
    }

    /// The line box a single line of `font` occupies, centred in `band`.
    ///
    /// The convenience form of [`text_line`] for the common case, and the entry point a
    /// control should reach for instead of writing `band.y + band.height / 2` — that
    /// expression places the glyph box's *top* edge on the band's middle line, which draws
    /// the label half a line too low (see [`VerticalAlignment`]).
    ///
    /// Use [`Self::text_line_aligned`] when the line belongs at the top or bottom of the
    /// band rather than centred.
    pub fn text_line(&self, band: Rect, font: &Font) -> Rect {
        text_line(band, font, VerticalAlignment::Center, self)
    }

    /// As [`Self::text_line`], with an explicit [`VerticalAlignment`].
    pub fn text_line_aligned(&self, band: Rect, font: &Font, alignment: VerticalAlignment) -> Rect {
        text_line(band, font, alignment, self)
    }

    /// Draws `text` fitted inside `bounds`, truncating with an ellipsis when it is wider.
    ///
    /// # Why this exists as a context method
    ///
    /// `draw_text` places a string and never asks whether it fits. A glyph's advance is
    /// `font.size()` pixels per cluster, so a 14 px font advances 14 px per character: a
    /// label longer than its control simply kept going, and the trailing glyphs landed
    /// outside the control's own rectangle. The software backend clips at the canvas edge,
    /// so the overflow was invisible there; the SVG backend emits absolute coordinates, so
    /// the same label appeared to run out of the picture. Around forty controls drew a
    /// placeholder, a file name or a status line wider than themselves.
    ///
    /// Naming the *rectangle* rather than a maximum width is the point: every call site
    /// already has the control's (or a cell's) rectangle in hand, and each was re-deriving
    /// "how much room is left" — or forgetting to. Passing the box makes the bound
    /// impossible to leave out by accident.
    ///
    /// `alignment` positions the *fitted* string, so a centred label stays centred inside
    /// `bounds` after truncation rather than drifting. The returned string is what was
    /// drawn, which lets a caller reserve the width it actually occupies.
    pub fn draw_text_fitted(
        &mut self,
        bounds: Rect,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) -> crate::compat::String {
        self.draw_text_fitted_cow(bounds, text, font, color, alignment).into_owned()
    }

    /// [`Self::draw_text_fitted`], returning the fitted text without materialising a `String`.
    ///
    /// # Why this entry point exists
    ///
    /// The public method promises the drawn text to its caller, which is a `String` by signature
    /// and therefore copies even when the label already fits. Almost every call site in this crate
    /// ignores that result: it draws a label and moves on. This crate-internal form returns the
    /// fitted text as a [`Cow`], so the common case — a label narrower than its box — borrows from
    /// the caller's own `&str` and allocates nothing. Truncation still allocates, because the
    /// ellipsised string has to be built, but that is the case that was always going to allocate.
    ///
    /// The public method delegates here, so the two can never disagree about what is drawn.
    pub fn draw_text_fitted_cow<'t>(
        &mut self,
        bounds: Rect,
        text: &'t str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) -> Cow<'t, str> {
        let inset = TEXT_FIT_MARGIN as i32;
        // Both ends are inset, so the width a string may occupy is the box less one inset at
        // each end. The old form subtracted a single inset and then placed the result in the
        // full box, which is what shifted every centred label left; see `fitted_origin`.
        let available = (bounds.width as i32 - 2 * inset).max(0) as f32;
        let (fitted, measured) = fit_text_to_width_measured(text, available, font, self.backend);
        let advance = match measured {
            Some(width) => width as i32,
            None => self.measure_text(&fitted, font).width as i32,
        };
        let origin = fitted_origin(bounds, advance, alignment);
        self.draw_text(origin, &fitted, font, color, HorizontalAlignment::Left);
        fitted
    }

    /// Draws `text` fitted inside `bounds`, without building the elided string.
    ///
    /// [`Self::draw_text_fitted_cow`] must return what it drew, so the truncated case assembles
    /// `prefix + `…`` into an owned string. Paint loops that only want the pixels paid that
    /// allocation on every elided label, every frame. This form draws the prefix and the ellipsis
    /// as two adjacent runs and allocates nothing; the fitted origin is computed from the two
    /// advances, so alignment matches the `Cow` form.
    pub fn draw_text_fitted_ellipsis(
        &mut self,
        bounds: Rect,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) {
        let inset = TEXT_FIT_MARGIN as i32;
        let available = (bounds.width as i32 - 2 * inset).max(0) as f32;
        let parts = fit_text_parts(text, available, font, self.backend);
        if !parts.ellipsis {
            // The label fits: draw the caller's own slice and allocate nothing to do it.
            let advance = parts.advance.unwrap_or(0.0) as i32;
            let origin = fitted_origin(bounds, advance, alignment);
            self.draw_text(origin, parts.prefix, font, color, HorizontalAlignment::Left);
            return;
        }
        let available_bits = available.to_bits();
        let dpi_bits = self.dpi_scale().to_bits();
        FITTED_LABEL_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if let Some(index) = cache.iter().position(|entry| {
                entry.source == text
                    && entry.available_bits == available_bits
                    && entry.font == *font
                    && entry.dpi_bits == dpi_bits
            }) {
                let entry = &cache[index];
                let origin = fitted_origin(bounds, entry.advance, alignment);
                self.draw_text(origin, &entry.fitted, font, color, HorizontalAlignment::Left);
                return;
            }
            // A truncated label is assembled once. The parts already name the prefix that fits;
            // the ellipsis is the one character appended to it, exactly as the `Cow` form builds
            // the answer.
            let mut fitted = String::with_capacity(parts.prefix.len() + 3);
            fitted.push_str(parts.prefix);
            fitted.push('\u{2026}');
            let advance = match parts.advance {
                Some(width) => width as i32,
                None => self.measure_text(&fitted, font).width as i32,
            };
            let origin = fitted_origin(bounds, advance, alignment);
            self.draw_text(origin, &fitted, font, color, HorizontalAlignment::Left);
            if cache.len() >= FITTED_LABEL_CACHE_DEPTH {
                cache.remove(0);
            }
            cache.push(FittedLabelEntry {
                source: String::from(text),
                available_bits,
                font: font.clone(),
                dpi_bits,
                fitted,
                advance,
            });
        });
    }

    /// Draws a single line of `text` fitted inside `bounds` and centred vertically in it.
    ///
    /// The vertical half of what [`Self::draw_text_fitted`] deliberately does not do. That
    /// method's contract is *fit horizontally, align horizontally* — its origin is
    /// `bounds.y` unchanged — so a caller that handed it a padded content box (a dialog's
    /// button row, a keyboard's key cap, a popover's empty state) got a label pinned to the
    /// box's top edge. Around twenty call sites read it as though it centred both ways; this
    /// entry point is the one that does, and naming it separately keeps the fit/align
    /// contract of the original intact for the callers that rely on it.
    ///
    /// `baseline.y` is ignored; the line box is derived from `bounds`. The horizontal half
    /// behaves exactly as in [`Self::draw_text_fitted`], including the fit inset and the
    /// truncation ellipsis.
    pub fn draw_text_line(
        &mut self,
        bounds: Rect,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) -> crate::compat::String {
        self.draw_text_line_cow(bounds, text, font, color, alignment).into_owned()
    }

    /// [`Self::draw_text_line`], returning the fitted text without materialising a `String`.
    ///
    /// See [`Self::draw_text_fitted_cow`]: the same allocation argument applies, and the public
    /// method delegates here so the two forms draw identically.
    pub fn draw_text_line_cow<'t>(
        &mut self,
        bounds: Rect,
        text: &'t str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) -> Cow<'t, str> {
        let line = self.text_line(bounds, font);
        self.draw_text_fitted_cow(line, text, font, color, alignment)
    }

    /// [`Self::draw_text_line`] without building the elided string.
    ///
    /// See [`Self::draw_text_fitted_ellipsis`]; the vertical half is identical to
    /// [`Self::draw_text_line_cow`].
    pub fn draw_text_line_ellipsis(
        &mut self,
        bounds: Rect,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) {
        let line = self.text_line(bounds, font);
        self.draw_text_fitted_ellipsis(line, text, font, color, alignment);
    }
    /// Splits `text` into visual clusters (grapheme-like units) with per-cluster
    /// advances, as used for hit testing and caret placement.
    ///
    /// Unaffected by the current translation offset.
    pub fn shape_text(&self, text: &str, font: &Font) -> Rc<ShapedText> {
        self.backend.shape_text(text, font)
    }
    /// Pushes a clip rectangle onto the backend's clip stack, translated by the
    /// current offset.
    ///
    /// Subsequent draws are restricted to this rectangle intersected with any
    /// enclosing clip. Every push must be matched by a
    /// [`RenderContext::pop_clip`]; the offsets pushed via
    /// [`RenderContext::push_offset`] also apply to it.
    pub fn push_clip(&mut self, x: i32, y: i32, width: u32, height: u32) {
        let x = x + self.offset_x;
        let y = y + self.offset_y;
        self.dispatch_owned(RenderCommand::PushClip { x, y, width, height });
    }
    /// Pops the innermost clip rectangle from the backend's clip stack.
    ///
    /// A pop with an empty stack is a no-op on the software backend; unbalanced
    /// pops would otherwise widen the clip unexpectedly.
    pub fn pop_clip(&mut self) {
        self.dispatch_owned(RenderCommand::PopClip);
    }
    /// Pushes a rounded-rectangle clip onto the clip stack (G-015).
    ///
    /// Everything drawn until [`Self::pop_clip`] is masked to the rounded
    /// region: corners outside the radius are cut away exactly as
    /// [`Self::fill_rounded_rect_corners`] cuts them, because the software
    /// backend computes the mask's coverage from the same path machinery a
    /// filled rounded rect uses.
    ///
    /// The push is translated by the current offset like [`Self::push_clip`],
    /// and pops from the same stack, so the two clip forms interleave freely.
    /// Radii that overrun an edge are resolved per CSS `border-radius` before
    /// use.
    pub fn push_clip_rounded_rect(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        radii: [u32; 4],
    ) {
        let x = x + self.offset_x;
        let y = y + self.offset_y;
        self.dispatch_owned(RenderCommand::PushClipRoundedRect { x, y, width, height, radii });
    }

    /// Begins a composed layer inside `bounds`, translated by the current offset.
    ///
    /// Everything drawn until [`RenderContext::pop_layer`] becomes the layer's foreground. The
    /// backdrop captured at the push is processed by `effect` and the foreground is composited
    /// over it.
    ///
    /// This is the crate-facing form of [`RenderCommand::PushLayer`]; widget code that resolved
    /// a material through the theme should use [`RenderContext::push_material_layer`], which
    /// takes the plain `(radius, tint)` pair that resolution returns.
    fn push_layer(&mut self, bounds: Rect, effect: Option<LayerEffect>) {
        let bounds = self.offset_rect(bounds);
        self.dispatch_owned(RenderCommand::PushLayer { bounds, effect });
    }

    /// Begins a material layer inside `bounds` from a plain blur radius and optional tint.
    ///
    /// This is the widget/window-facing entry point: a control asks the theme for a material
    /// and receives `(blur_radius, tint)` (or an opaque colour when transparency is reduced),
    /// then hands that answer straight to this method. The blur radius is the material's
    /// 2-sigma point, so no unit conversion is needed.
    pub fn push_material_layer(
        &mut self,
        bounds: Rect,
        blur_radius: u32,
        tint: Option<(Color, f32)>,
    ) {
        let effect = match tint {
            Some((tint, tint_weight)) => {
                Some(LayerEffect::BackdropBlurTint { radius: blur_radius, tint, tint_weight })
            }
            None if blur_radius == 0 => None,
            None => Some(LayerEffect::BackdropBlur { radius: blur_radius }),
        };
        self.push_layer(bounds, effect);
    }

    /// Ends the innermost `RenderContext::push_layer` / [`RenderContext::push_material_layer`]
    /// and composites it.
    pub fn pop_layer(&mut self) {
        self.dispatch_owned(RenderCommand::PopLayer);
    }

    /// Begins a track matte inside `bounds`; see [`RenderCommand::PushMatte`].
    ///
    /// The commands until [`Self::pop_matte`] draw the matte source (the Lottie “matte layer”),
    /// the commands after it draw the matted content, and [`Self::end_matte`] composites the
    /// content through the captured coverage. This is the offscreen primitive Lottie's `tt`
    /// layers needed: a partial-coverage mask cannot be expressed as a rectangular clip.
    pub fn push_matte(&mut self, bounds: Rect, mode: MatteMode) {
        let bounds = self.offset_rect(bounds);
        self.dispatch_owned(RenderCommand::PushMatte { bounds, mode });
    }

    /// Ends the matte drawing and starts the matted layer's foreground.
    pub fn pop_matte(&mut self) {
        self.dispatch_owned(RenderCommand::PopMatte);
    }

    /// Composites the matted layer through the mask and restores the backdrop.
    pub fn end_matte(&mut self) {
        self.dispatch_owned(RenderCommand::EndMatte);
    }

    /// Begins a composed layer with subtree opacity and an optional replay cache.
    ///
    /// This is the [`RenderCommand::PushLayerGroup`] form: `opacity` fades the whole layer (a
    /// child and its own background together), and a `cache_key` lets the backend reuse the
    /// layer's foreground when the caller guarantees the enclosed commands are unchanged. Pass
    /// `None` for `cache_key` to redraw every frame.
    pub fn push_layer_group(
        &mut self,
        bounds: Rect,
        effect: Option<LayerEffect>,
        opacity: f32,
        cache_key: Option<u64>,
    ) {
        let bounds = self.offset_rect(bounds);
        self.dispatch_owned(RenderCommand::PushLayerGroup { bounds, effect, opacity, cache_key });
    }

    /// Drops every cached layer foreground.
    ///
    /// A cache entry is keyed by a caller-supplied identity, so a caller that reuses identities
    /// for different content must clear the cache when it reuses them. The surface also clears it
    /// on resize, when the cached regions no longer describe the buffer.
    pub fn clear_layer_cache(&mut self) {
        self.backend.clear_layer_cache();
    }

    /// Draws `data` as an image at `(x, y)` scaled to `width` x `height` pixels.
    ///
    /// `data` is interpreted as 8-bit RGBA, four bytes per pixel, tightly
    /// packed, and is expected to hold at least `width * height * 4` bytes;
    /// a shorter slice is cropped by the backend. The position is translated by
    /// the current offset.
    ///
    /// # Borrowed, not copied (D09-RENDER-02)
    ///
    /// This routes through [`PaintBackend::draw_image`], which hands the pixels over by
    /// reference. A backend that rasterises the image immediately — the software backend does —
    /// therefore draws straight from `data` with no copy. A backend that retains the command
    /// still gets an owning copy from that method's default implementation, so the caller's
    /// slice never needs to outlive the call on any backend. The earlier form always built an
    /// owning `RenderCommand::DrawImage` (a `to_vec()`), which cost one full-image allocation and
    /// copy per draw even when nothing retained it.
    ///
    /// A region-bucketed pass must record an owning command, but it records the copy only once:
    /// `recorded_image` reuses the `Arc` it built for the same unchanged buffer on the next
    /// frame, so a steady frame copies nothing and the GPU backend keeps sampling the texture it
    /// already uploaded for that allocation.
    pub fn draw_image(&mut self, x: i32, y: i32, width: u32, height: u32, data: &[u8]) {
        let x = x + self.offset_x;
        let y = y + self.offset_y;
        if self.bucket_recording() {
            self.dispatch_owned(RenderCommand::DrawImage {
                x,
                y,
                width,
                height,
                data: recorded_image(data, width, height),
            });
        } else {
            self.backend.draw_image(x, y, width, height, data);
        }
    }

    /// Draws `data` (`source_width` x `source_height` RGBA8) **resampled** into the
    /// `width` x `height` rectangle.
    ///
    /// # Why this is a separate entry from [`Self::draw_image`]
    ///
    /// `draw_image` blits a buffer that already matches its rectangle: a shorter buffer is ignored
    /// and a longer one is cropped, so a control that hands over native pixels for a differently
    /// sized slot paints nothing (smaller source) or the top-left corner (larger source). Fitting
    /// an icon into a slot or a photo into a frame means resampling, and this is the call that
    /// says so. The identity case delegates to `draw_image`, so an image already at its slot size
    /// is byte-identical and allocation-free.
    ///
    /// A region-bucketed pass (partial repaint) records an owning
    /// [`RenderCommand::DrawImageScaled`], which carries the source grid and resamples when the
    /// bucket replays.
    pub fn draw_image_scaled(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        source_width: u32,
        source_height: u32,
        data: &[u8],
    ) {
        if width == 0 || height == 0 || source_width == 0 || source_height == 0 {
            return;
        }
        if width == source_width && height == source_height {
            self.draw_image(x, y, width, height, data);
            return;
        }
        let x = x + self.offset_x;
        let y = y + self.offset_y;
        if self.bucket_recording() {
            // The command carries the source grid and the pixels, so a bucket replay resamples
            // once per region at most — and pays one owning copy, not a resampled temporary. The
            // copy is shared with the previous frame through `recorded_image`, so the identity is
            // stable and the GPU backend's content-keyed upload actually hits (RD2-01).
            self.dispatch_owned(RenderCommand::DrawImageScaled {
                x,
                y,
                width,
                height,
                source_width,
                source_height,
                data: recorded_image(data, source_width, source_height),
            });
        } else {
            self.backend.draw_image_scaled(x, y, width, height, source_width, source_height, data);
        }
    }

    /// Execute an arbitrary render command directly.
    ///
    /// Useful for widgets like `Canvas` that store a command buffer
    /// and replay it during the draw pass.
    pub fn execute_command(&mut self, command: RenderCommand) {
        self.dispatch_owned(command);
    }

    /// Executes a command that the caller still owns.
    ///
    /// A display list that is replayed every frame (a canvas, a mini canvas) keeps its
    /// commands for its whole lifetime, so handing each one to [`Self::execute_command`]
    /// forced a deep clone per command per frame — `DrawText` owns a `Font`, and the path commands
    /// carry their point lists. Text and paths are shared by refcount (G-02/G-12), but a
    /// `Font` is still an owned value. The backends already take `&RenderCommand`, so the owned
    /// entry point above was the only reason the clone existed.
    pub fn execute_command_ref(&mut self, command: &RenderCommand) {
        self.dispatch(command);
    }
}
