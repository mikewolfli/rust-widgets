// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Device resources the renderer keeps across calls: the grow-only buffer and texture caches,
//! their uniform arenas, and the byte-size helpers they share.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::{align_to, rasterize_draw_commands_into_rgba8};
use super::super::types::{PixelRect, Rgba8};
use std::cell::{Ref, RefCell};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

/// Upper bound on a single blocking GPU readback.
///
/// The readback is synchronous by design: every entry point returns owned pixels. Before this
/// bound existed, `device.poll` waited forever, so a lost device or a stuck submission hung the
/// caller instead of reporting an error (G-63).
pub(crate) const GPU_READBACK_TIMEOUT: Duration = Duration::from_secs(10);

/// Grow-only uniform buffer shared by every per-draw params write of one frame.
///
/// Each draw writes its params at a device-aligned offset and its bind group references that
/// subrange, so a frame of N draws allocates **one** buffer instead of N. `reset` rewinds only
/// the frame cursor; the allocation itself survives, and the queue's ordering makes the next
/// frame's writes land after the previous frame's submission.
#[derive(Default)]
pub(crate) struct ParamsScratch {
    buffer: Option<wgpu::Buffer>,
    capacity: u64,
    used: u64,
    stride: u64,
    /// Bumped whenever `buffer` is replaced, so bind groups built over the old handle can be
    /// rebuilt. A handle identity would do, but `wgpu::Buffer` does not expose one that is stable
    /// across backends.
    generation: u64,
}

impl ParamsScratch {
    /// The current buffer handle, or `None` before the first write of the process.
    pub(crate) fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    /// Identifies the current buffer allocation; changes when the buffer is replaced.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// Rewinds the frame cursor. Call once per encoded frame, before the first upload.
    pub(crate) fn reset(&mut self) {
        self.used = 0;
    }

    /// Reserves room for `slots` aligned params records before a frame is packed.
    ///
    /// Each draw's params occupy at most one [`Self::stride`] slice, so reserving
    /// `commands + 1` slots up front keeps the buffer from growing in the middle of a frame —
    /// which matters because all draws of a frame share one dynamic-offset bind group and a
    /// mid-frame replacement would leave earlier offsets pointing into the old allocation.
    pub(crate) fn reserve(&mut self, device: &wgpu::Device, slots: u64) {
        let stride = self.stride_for(device);
        let needed = slots.saturating_mul(stride).max(4);
        self.ensure_capacity(device, needed);
    }

    /// Writes `bytes` into the shared buffer and returns the buffer handle and its aligned offset.
    ///
    /// The offset is a multiple of the device's `min_uniform_buffer_offset_alignment` (256 on the
    /// common backends, but a device may report more), which is what makes a subrange binding over
    /// the shared buffer legal on every backend.
    pub(crate) fn write(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
    ) -> (wgpu::Buffer, u64) {
        let stride = self.stride_for(device);
        let offset = self.used.div_ceil(stride) * stride;
        let needed = offset + bytes.len() as u64;
        self.ensure_capacity(device, needed);
        let buffer = self.buffer.as_ref().expect("the buffer was just ensured").clone();
        queue.write_buffer(&buffer, offset, bytes);
        self.used = needed;
        (buffer, offset)
    }

    /// The alignment every params record is placed on, learned from the device once.
    fn stride_for(&mut self, device: &wgpu::Device) -> u64 {
        if self.stride == 0 {
            let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
            self.stride = alignment.max(4);
        }
        self.stride
    }

    /// Grows the buffer when `needed` does not fit, bumping the generation the bind groups key on.
    fn ensure_capacity(&mut self, device: &wgpu::Device, needed: u64) {
        if self.capacity >= needed {
            return;
        }
        let capacity =
            grown_buffer_capacity(self.capacity, needed, device.limits().max_buffer_size);
        self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw_params_scratch"),
            size: capacity,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.capacity = capacity;
        self.generation = self.generation.wrapping_add(1);
    }
}

/// The two dynamic-offset bind groups built over the shared params buffer.
///
/// `generation` names the buffer they were built for; a growth replaces the buffer and therefore
/// invalidates both groups.
pub(crate) struct ParamsGroups {
    pub(crate) generation: u64,
    pub(crate) shape: wgpu::BindGroup,
    pub(crate) path_cover: wgpu::BindGroup,
}

/// Which textured family a cached params bind group belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ParamsFamily {
    Image,
    Radial,
    Gradient,
}

/// Bind groups over the shared params arena, keyed by the texture they sample (RD2-01 / EX4-04).
///
/// A textured bind group can only be reused while the texture is the same, so the key pairs the
/// family with the upload's content [`TextureKey`]. The cache **persists across frames**: the
/// arena buffer is grow-only, so a group built over generation `g` stays valid until the buffer is
/// replaced, and the texture it samples is already kept alive by `TextureCache` under the same
/// content key. A still scene therefore builds each group once instead of once per frame.
///
/// `MAX_GROUPS` bounds the memory a long run with many distinct images can retain; reaching it
/// clears the cache rather than growing without limit. A params-arena growth changes `generation`
/// and drops every entry, because they bind the old buffer handle.
#[derive(Default)]
pub(crate) struct ParamsGroupCache {
    groups: GenerationMap<wgpu::BindGroup>,
}

/// How many cross-frame groups the cache keeps before it starts over.
const MAX_GROUPS: usize = 512;

/// The generation-tagged map behind [`ParamsGroupCache`].
///
/// Split from the `wgpu` handle so the retention policy — a stale arena generation drops every
/// entry, and `MAX_GROUPS` bounds a long run — is testable without a device. The policy is the
/// whole behaviour of the cache; the surrounding type only adds the clone on `get`.
struct GenerationMap<V> {
    generation: u64,
    entries: HashMap<(ParamsFamily, TextureKey), V>,
}

impl<V> Default for GenerationMap<V> {
    fn default() -> Self {
        Self { generation: 0, entries: HashMap::new() }
    }
}

impl<V> GenerationMap<V> {
    /// Keeps entries that still bind the live arena; a stale generation clears them.
    fn prepare(&mut self, generation: u64) {
        if self.generation != generation {
            self.entries.clear();
            self.generation = generation;
        }
    }

    fn get(&self, generation: u64, family: ParamsFamily, key: &TextureKey) -> Option<&V> {
        if self.generation != generation {
            return None;
        }
        self.entries.get(&(family, *key))
    }

    fn insert(&mut self, generation: u64, family: ParamsFamily, key: TextureKey, value: V) {
        if self.generation != generation {
            self.entries.clear();
            self.generation = generation;
        }
        if self.entries.len() >= MAX_GROUPS && !self.entries.contains_key(&(family, key)) {
            // A long-lived process that has drawn more distinct images than the cap clears the map
            // rather than retaining every group forever. The next frame rebuilds what it needs.
            self.entries.clear();
        }
        self.entries.insert((family, key), value);
    }
}

impl ParamsGroupCache {
    /// Prepares the cache for a frame whose arena has `generation`.
    ///
    /// Unlike a per-frame reset, this keeps entries that still bind the live arena. It is called at
    /// the start of every encoded frame so a generation change is noticed before any lookup; a
    /// stale generation clears the map because every group in it binds a dead buffer handle.
    pub(crate) fn prepare(&mut self, generation: u64) {
        self.groups.prepare(generation);
    }

    /// The group for `(family, key)`, when this frame already built one over the same arena.
    pub(crate) fn get(
        &self,
        generation: u64,
        family: ParamsFamily,
        key: &TextureKey,
    ) -> Option<wgpu::BindGroup> {
        self.groups.get(generation, family, key).cloned()
    }

    /// Stores a group for the rest of this arena generation.
    pub(crate) fn insert(
        &mut self,
        generation: u64,
        family: ParamsFamily,
        key: TextureKey,
        group: wgpu::BindGroup,
    ) {
        self.groups.insert(generation, family, key, group);
    }
}

/// One persistent colour-uniform arena shared by every draw of one call.
///
/// WebGPU requires a dynamic uniform offset to be a multiple of the device's
/// `min_uniform_buffer_offset_alignment` (256 on the common backends, but a
/// device may report more), so each rectangle's 16-byte colour is padded to a
/// stride read from the device rather than assumed. One buffer and one bind
/// group serve the whole call instead of one of each per rectangle (REND-02).
pub(crate) struct UniformArena {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) capacity: u64,
    /// Dynamic-offset stride: the device's `min_uniform_buffer_offset_alignment`,
    /// raised to hold the 16-byte colour, so every offset stays legal on any backend.
    ///
    /// Only the per-rectangle path reads this; an instanced run carries colour in its instance
    /// records instead.
    pub(crate) stride: u64,
}

/// The offscreen render target for one `(width, height)`.
pub(crate) struct TargetCache {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
}

/// The stencil attachment the 2D passes carry, cached per `(width, height)`.
///
/// WebGPU requires *every* pipeline used in a pass to declare a depth-stencil state matching the
/// attachment's format, so this target exists for all 2D passes even though only path fills read or
/// write it. The view owns its texture, so the handle itself does not need to be stored.
pub(crate) struct StencilTarget {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) view: wgpu::TextureView,
}

/// A grow-only vertex buffer reused across calls.
///
/// The same cache holds both the per-rectangle vertex positions and the instanced quad records:
/// both uses are `VERTEX`-usage buffers written once per call, and sharing one grow-only
/// allocation is what keeps an instanced run from allocating once per quad.
pub(crate) struct VertexCache {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) capacity: u64,
}

/// A grow-only readback buffer reused across calls.
pub(crate) struct ReadbackCache {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) size: u64,
}

/// The two ping-pong targets and per-direction bind groups of the blur pass.
///
/// The pass reads a texture and writes one that is not also bound as its attachment, so two
/// textures are required. Both are `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_*`: the first is
/// uploaded into and read back from, the second is only ever the horizontal pass's target. Cached
/// per `(width, height)` and rebuilt when the size changes (REND-02's reuse rule).
pub(crate) struct BlurTargets {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) source: wgpu::Texture,
    pub(crate) source_view: wgpu::TextureView,
    pub(crate) scratch_view: wgpu::TextureView,
    pub(crate) horizontal: wgpu::BindGroup,
    pub(crate) vertical: wgpu::BindGroup,
    pub(crate) horizontal_params: wgpu::Buffer,
    pub(crate) vertical_params: wgpu::Buffer,
}

/// One blurred-shadow layer: the offscreen target its body is painted into, the two directional
/// blur bind groups and the composite bind group that paints the result back.
///
/// The layer is allocated at exactly the extent the CPU rasteriser uses — the shadow body grown by
/// the blur radius and clamped to the frame — so the box blur's clamped window is bounded by the
/// same edges on both paths (see [`super::super::shaders::SHADOW_BLUR_FS`]).
///
/// `body_uniform` carries the layer's own pixel-to-clip matrix, which is what lets the body be
/// packed in layer-local coordinates; `scratch_view` is the ping-pong partner of the horizontal
/// pass, the same size as the layer so one fullscreen triangle covers it exactly.
pub(crate) struct ShadowLayer {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) view: wgpu::TextureView,
    pub(crate) scratch_view: wgpu::TextureView,
    pub(crate) body_uniform: TransformedUniform,
    pub(crate) horizontal: wgpu::BindGroup,
    pub(crate) horizontal_params: wgpu::Buffer,
    pub(crate) vertical: wgpu::BindGroup,
    pub(crate) vertical_params: wgpu::Buffer,
    /// The blurred layer composited over the frame at its own rectangle: the image pipeline's
    /// params uniform plus the layer texture.
    pub(crate) composite: wgpu::BindGroup,
    /// The params buffer `composite` binds. The caller writes it, because only the caller can see
    /// the queue.
    pub(crate) composite_params: wgpu::Buffer,
}

/// The mirrored shadow layers of one call, one per [`DrawStep::Shadow`].
pub(crate) struct ShadowTargets {
    pub(crate) layers: Vec<ShadowLayer>,
}

/// One in-place frame blur: the two textures its six box passes ping-pong through, and the bind group
/// each pass reads with.
///
/// Both textures need `COPY_SRC` and `COPY_DST` as well as the render and binding usages, because the
/// region arrives by a texture-to-texture copy from the frame and leaves the same way — a pass cannot
/// read the target it writes, so the region is copied out, blurred, and copied back.
pub(crate) struct FrameBlurTarget {
    pub(crate) rect: PixelRect,
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) scratch_view: wgpu::TextureView,
    /// The six passes in order: horizontal then vertical for each of the three box widths. Entry `i`
    /// reads the texture an even `i` writes to, which is what makes the ping-pong implicit.
    pub(crate) passes: Vec<(wgpu::BindGroup, wgpu::Buffer)>,
}

/// The frame-blur targets of one call, one per [`DrawStep::BlurRegion`].
pub(crate) struct FrameBlurTargets {
    pub(crate) targets: Vec<FrameBlurTarget>,
}

/// One command drawn under a blend mode other than `Normal`.
///
/// The command's own steps stay in the frame's plan, immediately after its
/// [`DrawStep::Blend`] marker, so the renderer can replay exactly those steps into an offscreen
/// layer without re-deciding what the command was. `bbox` bounds both the backdrop copy and the
/// composite, so a blended command costs its own rectangle, not the whole frame.
pub(crate) struct BlendRun {
    pub(crate) bbox: PixelRect,
    pub(crate) mode: crate::render::BlendMode,
    pub(crate) kind: BlendKind,
}

/// How the run's layer was written, which decides what the composite does with it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlendKind {
    /// Straight colour: the `REPLACE` rectangle class.
    Straight,
    /// Premultiplied over a transparent layer: an image or a gradient.
    Premultiplied,
    /// Straight colour with the glyph coverage in the alpha channel: text, whose mode is applied to
    /// the colour and then composited by that coverage.
    Coverage,
}

/// One blend run's GPU handles: the composite bind group and the mode uniform it binds.
pub(crate) struct BlendUnit {
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) params: wgpu::Buffer,
}

/// The offscreen pair every blend run shares.
///
/// Both textures are the size of the frame, which is what lets the layer pass carry the shared
/// stencil attachment and reuse the frame's own mvp: the geometry packed for the frame is drawn
/// unchanged into the layer. `units` holds one composite bind group per run, because a pass cannot
/// read a uniform another pass has already overwritten.
pub(crate) struct BlendTargets {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) layer_view: wgpu::TextureView,
    pub(crate) backdrop: wgpu::Texture,
    pub(crate) units: Vec<BlendUnit>,
}

/// The model-view-projection uniform used by the instanced quad pipeline.
///
/// One matrix serves a whole call, so this is not a dynamic-offset arena. The 64-byte size is
/// `mat4x4f`'s uniform footprint.
pub(crate) struct TransformedUniform {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
}

/// The colour-and-depth pair the `3d` mesh pipeline renders into.
///
/// The view owns its texture, so the texture handle itself does not need to be stored. Cached per
/// `(width, height)` like the other targets (REND-02) and rebuilt when the size changes.
#[cfg(feature = "3d")]
pub(crate) struct DepthTarget {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) view: wgpu::TextureView,
}

/// Grow-only uniform arena for the SDF circle pipeline (G-219).
///
/// Each circle consumes two 16-byte slots at the dynamic-offset stride — the fill colour at
/// `2 * i` and the radius/centre SDF parameters at `2 * i + 1` — so one buffer and one bind
/// group serve every circle of a frame. The bind group binds the same buffer at both
/// entries; the per-draw dynamic offsets select the slots.
pub(crate) struct CircleUniformArena {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) capacity: u64,
    pub(crate) bind_group: wgpu::BindGroup,
}

/// Grow-only CPU frame storage for one render path (G-221).
///
/// The rasteriser zero-fills the frame, so a reused allocation is byte-identical to a fresh
/// one; only the capacity is retained. A separate cell per path lets a caller keep one frame
/// borrowed (for an upload) without blocking a different path's borrow.
pub(crate) struct CpuFrame {
    pub(crate) bytes: RefCell<Vec<u8>>,
}

impl CpuFrame {
    pub(crate) fn new() -> Self {
        Self { bytes: RefCell::new(Vec::new()) }
    }

    /// Rasterises `commands` into this path's buffer and lends the frame back to the caller.
    ///
    /// The buffer grows only. `bytes` is re-borrowed immutably for the returned guard after
    /// the rasteriser has finished, so the caller cannot observe a half-drawn frame.
    pub(crate) fn rasterize(
        &self,
        width: u32,
        height: u32,
        commands: &[WgpuDrawCommand],
    ) -> Result<Ref<'_, [u8]>, String> {
        let byte_len = rgba_buffer_len(width, height)?;
        {
            let mut bytes = self.bytes.borrow_mut();
            if bytes.len() < byte_len {
                bytes.resize(byte_len, 0);
            }
            rasterize_draw_commands_into_rgba8(width, height, commands, &mut bytes[..byte_len])?;
        }
        Ok(Ref::map(self.bytes.borrow(), |bytes| &bytes[..byte_len]))
    }
}

/// One encoded draw of the command-stream pass (G-218).
///
/// The dispatcher packs the frame's geometry first and records the draw order here, so the
/// render pass replays painter order exactly without re-deciding anything while encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrawStep {
    /// Sets the scissor rectangle for the draws that follow.
    ///
    /// Rectangle runs set the full frame themselves (their clipping is baked into the geometry),
    /// so this step is emitted only before draws whose clipping cannot be expressed as geometry:
    /// circles and images.
    Scissor(PixelRect),
    /// Full-screen repaint through the clear pipeline; the payload is the colour slot index
    /// in the shared uniform arena.
    Clear(usize),
    /// Instanced quad run over the instance range `[start, end)` of the packed instance
    /// region.
    Rects(usize, usize),
    /// One SDF circle draw over the bounding quad at `quad_index`.
    Circle(usize),
    /// One image draw: the `index`th entry of the frame's image-draw table (a texture and its
    /// params uniform).
    Image(usize),
    /// One text box: the `index`th entry of the frame's text-draw table (a coverage texture and
    /// its params uniform).
    Text(usize),
    /// One ring / wedge / rounded outline: the `index`th entry of the frame's shape-draw table.
    Shape(usize),
    /// One radial gradient: the `index`th entry of the frame's radial-gradient table.
    Radial(usize),
    /// Blur the frame region described by the `index`th entry of the frame's blur table, in place.
    ///
    /// Like [`DrawStep::Blend`], this marker splits the frame's pass: it has to stop the pass, copy
    /// the region out, run its own passes, and copy the result back before anything else is drawn.
    BlurRegion(usize),
    /// One general gradient (linear in any direction, radial or conic): the `index`th entry of the
    /// frame's gradient table.
    Gradient(usize),
    /// One filled path: the `index`th entry of the frame's path table.
    Path(usize),
    /// One blurred shadow: the `index`th entry of the frame's shadow table. The blurred layer is
    /// rendered before the main pass (see `super::encode`), so this step only composites it.
    Shadow(usize),
    /// A command drawn under a non-`Normal` blend mode: `run` indexes the frame's blend table and
    /// `steps` is how many of the following plan entries belong to it. The renderer replays those
    /// `steps` into an offscreen layer and composites it, instead of recording them in place.
    Blend {
        /// Index into the frame's blend table.
        run: usize,
        /// Number of following plan entries that make up this run.
        steps: usize,
    },
}

/// One blurred shadow of a frame: the layer it is rendered into, the instance that paints its
/// body, and the kernel radius.
///
/// The body instance lives in the frame's `shadow_instances` region rather than in the main
/// instance run, because it is drawn into the layer before the frame's pass begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShadowDraw {
    /// The layer's rectangle in frame pixels; the layer texture is this size.
    pub(crate) layer: PixelRect,
    /// Index of the body's instance record in the packed `shadow_instances` region.
    pub(crate) body_instance: usize,
    pub(crate) blur_radius: u32,
}

/// The `ShadowBlurParams` uniform of one directional pass: direction, radius and one unused word.
///
/// The 32-byte size is the one `blur_bind_group_layout` declares; the trailing word is padding
/// the shader never reads.
pub(crate) fn shadow_blur_params(direction: [f32; 2], radius: u32) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[0..4].copy_from_slice(&direction[0].to_le_bytes());
    bytes[4..8].copy_from_slice(&direction[1].to_le_bytes());
    bytes[8..12].copy_from_slice(&(radius as f32).to_le_bytes());
    bytes
}

/// CPU-side packing buffers reused across calls (G-216).
///
/// Every entry point used to build a fresh `Vec` for the bytes it uploads: instance records,
/// per-rect NDC vertices, uniform staging, mesh vertices and indices. The packing itself is
/// unavoidable — the GPU consumes a byte layout — but the allocation is not: these buffers
/// are cleared and refilled each call, so the capacity the peak frame required is retained
/// across frames instead of being freed and re-grown.
#[derive(Default)]
pub(crate) struct PackScratch {
    /// Instanced quad records, `INSTANCE_BYTES` per instance.
    pub(crate) instances: Vec<u8>,
    /// Per-rect NDC vertices, 48 bytes (twelve `f32`) per rectangle.
    pub(crate) vertices: Vec<u8>,
    /// Colour slots staged for the shared uniform arena, one aligned slot per rectangle.
    pub(crate) uniform: Vec<u8>,
    /// SDF circle bounding quads, six 8-byte vertices per circle (G-219).
    pub(crate) circle_vertices: Vec<u8>,
    /// SDF circle uniform slots, two aligned slots per circle (G-219).
    pub(crate) circle_uniforms: Vec<u8>,
    /// Path fan vertices, two `f32` per point, appended after the instances and circle quads.
    pub(crate) path_vertices: Vec<u8>,
    /// The triangle fan of the path command being encoded.
    ///
    /// The encoder used to build a fresh `Vec<(f32, f32)>` per path command and drop it once the
    /// vertices reached `path_vertices`; this keeps that scratch across commands and frames, so a
    /// steady frame encodes a path without allocating (RD6-02).
    pub(crate) path_fan: Vec<(f32, f32)>,
    /// The instanced records of the shadow bodies, one `INSTANCE_BYTES` record per blurred shadow,
    /// appended after the path vertices. They are painted into offscreen layers, never into the
    /// frame, so they are kept out of the main instance run.
    pub(crate) shadow_instances: Vec<u8>,
    /// The commands drawn under a non-`Normal` blend mode, one entry per run.
    pub(crate) blend_runs: Vec<BlendRun>,
    /// The command-stream dispatcher's draw order (G-218).
    pub(crate) plan: Vec<DrawStep>,
    /// The clip rectangles still open while the stream is packed.
    ///
    /// Stream state, not per-command data: it is cleared where `plan` is and keeps its capacity,
    /// so a frame with nested clips does not allocate a fresh stack on every encode.
    pub(crate) clip_stack: Vec<PixelRect>,
    /// Mesh vertices, 32 bytes per vertex (`3d`).
    #[cfg(feature = "3d")]
    pub(crate) mesh_vertices: Vec<u8>,
    /// Mesh indices as little-endian `u32` (`3d`).
    #[cfg(feature = "3d")]
    pub(crate) mesh_indices: Vec<u8>,
}

/// Creates a 64-byte matrix uniform and the bind group that exposes it at group 0, binding 0.
///
/// Both the instanced quad pipeline and the mesh pipeline use one matrix per call rather than a
/// dynamic-offset arena, so this is the one shape their resources share. The 64-byte size is
/// `mat4x4f`'s uniform footprint.
pub(crate) fn create_matrix_uniform(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &str,
) -> TransformedUniform {
    let buffer_label = format!("{label}_ubo");
    let bind_group_label = format!("{label}_bind_group");
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(buffer_label.as_str()),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(bind_group_label.as_str()),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(64),
            }),
        }],
    });
    TransformedUniform { buffer, bind_group }
}

/// Device resources a renderer keeps across calls.
///
/// # Why this exists (REND-02)
///
/// Every entry point used to allocate its uniform buffer, bind group, vertex
/// buffer, offscreen texture and full-frame readback buffer per call. Those
/// allocations are not free — a `fill_rect` batch of 40 rectangles created 40
/// uniform buffers and 40 bind groups — and the per-call texture made the GPU
/// treat every call as a new render target. This cache keeps one of each, grown
/// on demand and overwritten with `write_buffer`, because the API is called at
/// frame rate by a host that renders offscreen. The data is fully rewritten on
/// every call, so no stale content can leak between calls.
///
/// `RefCell` rather than `&mut self`: the public entry points take `&self` (and
/// `GpuRenderer` calls them that way), so the cache is the only interior-mutable
/// piece. It is not `Sync`, and the renderer is not shared across threads.
/// Identity of one uploaded texture: the source allocation plus the grid it describes.
///
/// The source `Arc` is retained by the cache entry, so the address cannot be recycled by a
/// different image while the entry exists — the same rule the baked-image cache in
/// `render/backend/batch.rs` states.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TextureKey {
    ptr: usize,
    len: usize,
    width: u32,
    height: u32,
}

impl TextureKey {
    /// Keys `source` laid out as `width` x `height` pixels.
    pub(crate) fn new(source: &Arc<[u8]>, width: u32, height: u32) -> Self {
        Self { ptr: source.as_ptr() as usize, len: source.len(), width, height }
    }
}

/// One cached upload: the view the bind groups sample, plus what keeps it alive.
pub(crate) struct CachedTexture {
    /// The key's source, retained so the pointer identity stays valid.
    _source: Arc<[u8]>,
    /// The texture the view was made from (the view alone already keeps it alive).
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

/// Content-keyed uploads of draw inputs whose pixels come from a shared `Arc<[u8]>` (RD2-01).
///
/// Before this cache every image, gradient ramp and radial stop table was re-created and
/// re-uploaded per draw, per frame. Those bytes do not change between frames — an `Arc` image is
/// stable content — so the upload happens once and later draws only create their bind group.
///
/// Bounded LRU: a host that streams video frames through distinct `Arc`s allocates one upload per
/// frame and the least recently used entry is dropped, exactly like the crate's other content
/// caches. LRU rather than FIFO because a working set that cycles through N+1 textures (a carousel,
/// a video thumbnail strip, a screen with 33+ icons) would otherwise evict the entry that is about
/// to be drawn next and re-upload every texture every frame.
#[derive(Default)]
pub(crate) struct TextureCache {
    entries: HashMap<TextureKey, CachedTexture>,
    order: VecDeque<TextureKey>,
}

impl TextureCache {
    /// How many uploads one renderer keeps before evicting the least recently used one.
    const LIMIT: usize = 64;

    /// The cached view for `key`, when the upload is still resident.
    ///
    /// A hit marks the entry as most recently used (see [`Self::touch`]).
    pub(crate) fn view(&mut self, key: &TextureKey) -> Option<wgpu::TextureView> {
        let view = self.entries.get(key)?.view.clone();
        self.touch(key);
        Some(view)
    }

    /// Moves `key` to the back of the recency queue, where eviction never reaches it next.
    fn touch(&mut self, key: &TextureKey) {
        let Some(position) = self.order.iter().position(|candidate| candidate == key) else {
            return;
        };
        // Already the most recent: moving it would only churn the queue.
        if position + 1 == self.order.len() {
            return;
        }
        if let Some(touched) = self.order.remove(position) {
            self.order.push_back(touched);
        }
    }

    /// Stores `texture`/`view` under `key`, retaining `source` and evicting the least recently
    /// used entry when the cache is full.
    pub(crate) fn insert(
        &mut self,
        key: TextureKey,
        source: Arc<[u8]>,
        texture: wgpu::Texture,
        view: wgpu::TextureView,
    ) {
        if self
            .entries
            .insert(key, CachedTexture { _source: source, _texture: texture, view })
            .is_none()
        {
            self.order.push_back(key);
        } else {
            // Re-inserting an existing key replaces the upload, so it also becomes the most
            // recently used one.
            self.touch(&key);
        }
        while self.order.len() > Self::LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

/// Identity of one rasterised text coverage mask (RD3-04).
///
/// The bytes of a coverage texture depend on the text (or, for the shaped path, the mask bytes
/// themselves), the box it was laid out in, and **which face answered each character** — the last
/// of which changes when a host calls `register_face` without any text changing. All four are
/// therefore part of the key. `digest` plus `len` is the probe; the entry keeps the original
/// content, so a hash collision costs a comparison and never draws the wrong glyphs.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TextCoverageKey {
    digest: u64,
    len: usize,
    width: u32,
    height: u32,
    face_generation: u32,
}

impl TextCoverageKey {
    pub(crate) fn new(text: &str, width: u32, height: u32) -> Self {
        Self::digest_bytes(text.as_bytes(), text.len(), width, height)
    }

    /// The key for a caller-supplied coverage mask, which has no text behind it.
    ///
    /// The shaped-text path hands the renderer the already-rasterised mask, so its identity is
    /// the bytes themselves plus the box and face (W6-05).
    pub(crate) fn coverage(bytes: &[u8], width: u32, height: u32) -> Self {
        Self::digest_bytes(bytes, bytes.len(), width, height)
    }

    fn digest_bytes(bytes: &[u8], len: usize, width: u32, height: u32) -> Self {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut digest = FNV_OFFSET;
        for byte in bytes {
            digest ^= u64::from(*byte);
            digest = digest.wrapping_mul(FNV_PRIME);
        }
        Self {
            digest,
            len,
            width,
            height,
            face_generation: crate::render::text::font_assets::face_generation(),
        }
    }
}

/// One cached coverage upload: the content it was rasterised from, plus the texture.
pub(crate) struct CachedTextCoverage {
    /// The content, retained so a hash collision cannot reuse the wrong mask.
    content: CachedCoverageContent,
    /// The texture the view was made from (the view alone already keeps it alive).
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

/// What a cached coverage entry was built from.
///
/// The text path keys on a string and the shaped path on the mask bytes; keeping the original
/// value lets a lookup reject a digest collision on the exact content rather than on a weaker
/// proxy.
enum CachedCoverageContent {
    Text(Arc<str>),
    Bytes(Arc<[u8]>),
}

/// Content-keyed text coverage uploads (RD3-04).
///
/// The GPU text path rasterises glyph coverage on the CPU and uploads it as an `R8Unorm` texture
/// per draw. Unlike an `Arc` image, the same caption is drawn again on every frame with identical
/// coverage — colour and position are shader inputs, not texture content — so the upload is
/// reusable until the text, the box or the face changes. This is the text-side counterpart of
/// [`TextureCache`].
#[derive(Default)]
pub(crate) struct TextCoverageCache {
    entries: HashMap<TextCoverageKey, CachedTextCoverage>,
    order: VecDeque<TextCoverageKey>,
    /// How many coverage textures this cache has uploaded (misses, not lookups).
    ///
    /// A counter rather than an inferred number: the RD3-04 property is "a repeated caption
    /// uploads once", and a test that only checked the cache length could pass while the
    /// renderer kept rasterising and uploading the same coverage. Test-only so the release
    /// build carries no bookkeeping for a measurement it does not publish.
    #[cfg(test)]
    uploads: u64,
}

impl TextCoverageCache {
    /// How many coverage uploads one renderer keeps before evicting the least recently used one.
    ///
    /// A screen paints more distinct captions than distinct images, and a coverage texel is one
    /// byte rather than four, so the bound is higher than [`TextureCache`]'s.
    const LIMIT: usize = 128;

    /// The cached view for `key`, when the entry describes exactly this text.
    ///
    /// A hit marks the entry as most recently used (see [`Self::touch`]).
    pub(crate) fn view(&mut self, key: &TextCoverageKey, text: &str) -> Option<wgpu::TextureView> {
        let entry = self.entries.get(key)?;
        match &entry.content {
            CachedCoverageContent::Text(cached) if cached.as_ref() == text => {}
            _ => return None,
        }
        let view = entry.view.clone();
        self.touch(key);
        Some(view)
    }

    /// The cached view for a shaped mask, when the entry holds exactly these bytes.
    pub(crate) fn view_coverage(
        &mut self,
        key: &TextCoverageKey,
        coverage: &[u8],
    ) -> Option<wgpu::TextureView> {
        let entry = self.entries.get(key)?;
        match &entry.content {
            CachedCoverageContent::Bytes(cached) if cached.as_ref() == coverage => {}
            _ => return None,
        }
        let view = entry.view.clone();
        self.touch(key);
        Some(view)
    }

    /// Moves `key` to the back of the recency queue, where eviction never reaches it next.
    fn touch(&mut self, key: &TextCoverageKey) {
        let Some(position) = self.order.iter().position(|candidate| candidate == key) else {
            return;
        };
        if position + 1 == self.order.len() {
            return;
        }
        if let Some(touched) = self.order.remove(position) {
            self.order.push_back(touched);
        }
    }

    /// Stores `texture`/`view` under `key`, evicting the least recently used entry when the cache
    /// is full.
    pub(crate) fn insert(
        &mut self,
        key: TextCoverageKey,
        text: Arc<str>,
        texture: wgpu::Texture,
        view: wgpu::TextureView,
    ) {
        self.insert_entry(key, CachedCoverageContent::Text(text), texture, view);
    }

    /// Stores a shaped mask's texture under `key`, evicting the least recently used entry when
    /// the cache is full.
    pub(crate) fn insert_coverage(
        &mut self,
        key: TextCoverageKey,
        coverage: Arc<[u8]>,
        texture: wgpu::Texture,
        view: wgpu::TextureView,
    ) {
        self.insert_entry(key, CachedCoverageContent::Bytes(coverage), texture, view);
    }

    fn insert_entry(
        &mut self,
        key: TextCoverageKey,
        content: CachedCoverageContent,
        texture: wgpu::Texture,
        view: wgpu::TextureView,
    ) {
        #[cfg(test)]
        {
            self.uploads = self.uploads.saturating_add(1);
        }
        if self
            .entries
            .insert(key, CachedTextCoverage { content, _texture: texture, view })
            .is_none()
        {
            self.order.push_back(key);
        } else {
            self.touch(&key);
        }
        while self.order.len() > Self::LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    /// The number of coverage uploads made so far (cache misses).
    #[cfg(test)]
    pub(crate) fn uploads(&self) -> u64 {
        self.uploads
    }
}

#[derive(Default)]
pub(crate) struct RenderCache {
    pub(crate) uniform: Option<UniformArena>,
    pub(crate) target: Option<TargetCache>,
    /// Stencil attachment shared by every 2D pass, sized with the colour target.
    pub(crate) stencil: Option<StencilTarget>,
    pub(crate) upload_target: Option<TargetCache>,
    pub(crate) vertices: Option<VertexCache>,
    pub(crate) readback: Option<ReadbackCache>,
    /// Ping-pong targets and bind groups for the separable blur pass.
    pub(crate) blur: Option<BlurTargets>,
    /// The frame regions this frame blurs in place, rebuilt when their extents change.
    pub(crate) frame_blur: Option<FrameBlurTargets>,
    /// The offscreen layers of the frame's blurred shadows, rebuilt when their extents change.
    pub(crate) shadow: Option<ShadowTargets>,
    /// The offscreen pair the frame's blend runs share, rebuilt when the size or run count changes.
    pub(crate) blend: Option<BlendTargets>,
    /// Mvp uniform and bind group for the instanced quad pipeline.
    pub(crate) transformed_uniform: Option<TransformedUniform>,
    /// Grow-only uniform arena for the SDF circle pipeline (G-219).
    pub(crate) circle_uniforms: Option<CircleUniformArena>,
    /// CPU-side packing buffers refilled each call instead of reallocated (G-216).
    pub(crate) pack: PackScratch,
    /// The command-stream packer's per-frame side tables, refilled each call (RD-02).
    pub(crate) tables: super::pack_tables::PackTables,
    /// The `Depth32Float` attachment for mesh draws.
    #[cfg(feature = "3d")]
    pub(crate) depth_target: Option<DepthTarget>,
    /// Mvp uniform and bind group for the mesh pipeline.
    #[cfg(feature = "3d")]
    pub(crate) mesh_uniform: Option<TransformedUniform>,
    /// Grow-only index buffer for mesh draws.
    #[cfg(feature = "3d")]
    pub(crate) mesh_indices: Option<VertexCache>,
}

// Retain headroom without exceeding the device limit; buffers remain grow-only.
pub(crate) fn grown_buffer_capacity(current: u64, needed: u64, limit: u64) -> u64 {
    let grown = current.saturating_add(current / 2).max(needed).max(4);
    let rounded = grown.checked_add(65_535).map(|size| size / 65_536 * 65_536).unwrap_or(limit);
    rounded.min(limit).max(needed.max(4))
}

impl RenderCache {
    /// The dynamic-offset stride for the colour arena: the device's
    /// `min_uniform_buffer_offset_alignment`, raised to hold the 16-byte colour
    /// and rounded back up to a legal multiple. A hard-coded 256 would make every
    /// draw a validation error on a device that reports 512.
    pub(crate) fn uniform_stride(device: &wgpu::Device) -> u64 {
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        alignment.max(16).next_multiple_of(alignment.max(1))
    }

    /// Ensures the colour arena can hold `slots` colours.
    pub(crate) fn ensure_uniform(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        slots: usize,
    ) -> Result<(), String> {
        let stride = Self::uniform_stride(device);
        let needed = stride
            .checked_mul(slots.max(1) as u64)
            .ok_or_else(|| format!("{slots} colour slots overflow the uniform buffer size"))?;
        let needs_new = self.uniform.as_ref().map(|arena| arena.capacity < needed).unwrap_or(true);
        if needs_new {
            if needed > device.limits().max_buffer_size {
                return Err(format!("{needed} uniform bytes exceed the device buffer limit"));
            }
            let capacity = grown_buffer_capacity(
                self.uniform.as_ref().map_or(0, |arena| arena.capacity),
                needed,
                device.limits().max_buffer_size,
            );
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rect_color_ubo"),
                size: capacity,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rect_bind_group"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(16),
                    }),
                }],
            });
            self.uniform = Some(UniformArena { buffer, bind_group, capacity, stride });
        }
        Ok(())
    }

    /// Ensures the offscreen target matches `width × height`.
    pub(crate) fn ensure_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let matches = self
            .target
            .as_ref()
            .map(|target| target.width == width && target.height == height)
            .unwrap_or(false);
        if !matches {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("wgpu_offscreen_target"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                // `TEXTURE_BINDING` so a caller of `with_draw_commands_texture` can sample the
                // picture, and `COPY_DST` so a stream the dispatcher refuses can be rasterised on
                // the CPU and uploaded into the same texture.
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.target = Some(TargetCache { width, height, texture, view });
        }
    }

    /// Ensures the stencil attachment matches `width × height`.
    ///
    /// `Stencil8` is the smallest format the path fills can use: even-odd needs one bit, and the
    /// non-zero rule needs a signed count, which eight bits carry with wrapped increments.
    pub(crate) fn ensure_stencil(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let matches = self
            .stencil
            .as_ref()
            .map(|target| target.width == width && target.height == height)
            .unwrap_or(false);
        if !matches {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("wgpu_2d_stencil"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Stencil8,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.stencil = Some(StencilTarget { width, height, view });
        }
    }

    /// Ensures the upload-side texture matches `width × height`.
    ///
    /// Separate from [`Self::ensure_target`] because this one is writable
    /// (`COPY_DST`): `queue.write_texture` uploads the caller's bytes into it before the copy
    /// back, which a render-attachment-only texture cannot accept.
    pub(crate) fn ensure_upload_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let matches = self
            .upload_target
            .as_ref()
            .map(|target| target.width == width && target.height == height)
            .unwrap_or(false);
        if !matches {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("rw_wgpu_offscreen_texture"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.upload_target = Some(TargetCache { width, height, texture, view });
        }
    }

    /// Ensures the vertex buffer can hold `bytes`.
    pub(crate) fn ensure_vertices(&mut self, device: &wgpu::Device, bytes: u64) {
        let needs_new = self.vertices.as_ref().map(|cache| cache.capacity < bytes).unwrap_or(true);
        if needs_new {
            let capacity = grown_buffer_capacity(
                self.vertices.as_ref().map_or(0, |cache| cache.capacity),
                bytes,
                device.limits().max_buffer_size,
            );
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rect_vertex_buffer"),
                size: capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertices = Some(VertexCache { buffer, capacity });
        }
    }

    /// Ensures the readback buffer can hold `size` bytes.
    pub(crate) fn ensure_readback(&mut self, device: &wgpu::Device, size: u64) {
        let needs_new = self.readback.as_ref().map(|cache| cache.size < size).unwrap_or(true);
        if needs_new {
            let capacity = grown_buffer_capacity(
                self.readback.as_ref().map_or(0, |cache| cache.size),
                size,
                device.limits().max_buffer_size,
            );
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("wgpu_readback"),
                size: capacity,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.readback = Some(ReadbackCache { buffer, size: capacity });
        }
    }

    /// Ensures the blur ping-pong targets and both directional bind groups match `width × height`.
    pub(crate) fn ensure_blur_targets(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        width: u32,
        height: u32,
    ) {
        let matches = self
            .blur
            .as_ref()
            .map(|targets| targets.width == width && targets.height == height)
            .unwrap_or(false);
        if matches {
            return;
        }
        let make_texture = |label: &str| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let source = make_texture("wgpu_blur_source");
        let scratch = make_texture("wgpu_blur_scratch");
        let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
        let scratch_view = scratch.create_view(&wgpu::TextureViewDescriptor::default());
        let make_params = |label: &str| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let horizontal_params = make_params("wgpu_blur_horizontal_params");
        let vertical_params = make_params("wgpu_blur_vertical_params");
        let make_bind_group = |label: &str, view: &wgpu::TextureView, buffer: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(32),
                        }),
                    },
                ],
            })
        };
        let horizontal = make_bind_group("wgpu_blur_horizontal", &source_view, &horizontal_params);
        let vertical = make_bind_group("wgpu_blur_vertical", &scratch_view, &vertical_params);
        self.blur = Some(BlurTargets {
            width,
            height,
            source,
            source_view,
            scratch_view,
            horizontal,
            vertical,
            horizontal_params,
            vertical_params,
        });
    }

    /// Ensures one offscreen layer exists per entry of `layers`.
    ///
    /// The layers are reused when the count and every extent still match, which is the common case
    /// for a host that redraws the same widget tree. A changed extent rebuilds the whole set rather
    /// than patching one entry: the bind groups of a rebuilt layer differ in size, so a partial
    /// update would need the same work as a full one and would leave mismatched pairs if it
    /// aborted.
    ///
    /// Nothing is written into the layers here — the caller owns the queue and writes the body
    /// matrix, the blur radii and the composite params before it encodes.
    pub(crate) fn ensure_shadow_targets(
        &mut self,
        device: &wgpu::Device,
        blur_layout: &wgpu::BindGroupLayout,
        image_layout: &wgpu::BindGroupLayout,
        mvp_layout: &wgpu::BindGroupLayout,
        layers: &[PixelRect],
    ) {
        let reusable = self
            .shadow
            .as_ref()
            .map(|targets| {
                targets.layers.len() == layers.len()
                    && targets.layers.iter().zip(layers).all(|(layer, rect)| {
                        layer.width == rect.width && layer.height == rect.height
                    })
            })
            .unwrap_or(false);
        if reusable {
            return;
        }
        let mut built = Vec::with_capacity(layers.len());
        for (index, rect) in layers.iter().enumerate() {
            built.push(create_shadow_layer(
                device,
                blur_layout,
                image_layout,
                mvp_layout,
                *rect,
                index,
            ));
        }
        self.shadow = Some(ShadowTargets { layers: built });
    }

    /// Ensures one blur target per requested region and kernel.
    ///
    /// Rebuilt when the count, any rectangle, or any pass count changes, for the same reason the
    /// shadow layers are: a partially updated set would leave bind groups pointing at textures of
    /// the wrong size, and a stale pass count would leave the encode loop indexing widths that the
    /// current kernel no longer has. A radius below two produces fewer than three box widths, so the
    /// pass count really does vary with the kernel — it is not a constant six. The per-pass radius
    /// lives in each pass's own uniform, because all of them are encoded into one submission and a
    /// shared buffer would show every pass the last value written.
    pub(crate) fn ensure_frame_blur_targets(
        &mut self,
        device: &wgpu::Device,
        blur_layout: &wgpu::BindGroupLayout,
        specs: &[(PixelRect, u32)],
    ) {
        let reusable = self
            .frame_blur
            .as_ref()
            .map(|targets| {
                targets.targets.len() == specs.len()
                    && targets.targets.iter().zip(specs).all(|(target, (rect, radius))| {
                        target.rect == *rect
                            && target.passes.len() == frame_blur_pass_count(*radius) * 2
                    })
            })
            .unwrap_or(false);
        if reusable {
            return;
        }
        let built = specs
            .iter()
            .enumerate()
            .map(|(index, (rect, radius))| {
                create_frame_blur_target(device, blur_layout, *rect, *radius, index)
            })
            .collect();
        self.frame_blur = Some(FrameBlurTargets { targets: built });
    }

    /// Ensures the blend pair and one composite bind group per run.
    ///
    /// The pair is rebuilt when the frame size or the run count changes, for the same reason the
    /// shadow layers are: a partially updated set would leave bind groups pointing at textures of
    /// the wrong size. Each run gets its own 16-byte mode uniform, because all of a frame's runs are
    /// encoded into one submission and a single shared buffer would show every pass the last value
    /// written.
    pub(crate) fn ensure_blend_targets(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        width: u32,
        height: u32,
        runs: usize,
    ) {
        let reusable = self
            .blend
            .as_ref()
            .map(|targets| {
                targets.width == width && targets.height == height && targets.units.len() == runs
            })
            .unwrap_or(false);
        if reusable {
            return;
        }
        let make_texture = |label: &str| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let layer_view =
            make_texture("wgpu_blend_layer").create_view(&wgpu::TextureViewDescriptor::default());
        let backdrop = make_texture("wgpu_blend_backdrop");
        let backdrop_view = backdrop.create_view(&wgpu::TextureViewDescriptor::default());
        let mut units = Vec::with_capacity(runs);
        for index in 0..runs {
            let params = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("wgpu_blend_params"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(format!("wgpu_blend_composite_{index}").as_str()),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &params,
                            offset: 0,
                            size: wgpu::BufferSize::new(16),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&layer_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&backdrop_view),
                    },
                ],
            });
            units.push(BlendUnit { bind_group, params });
        }
        self.blend = Some(BlendTargets { width, height, layer_view, backdrop, units });
    }

    /// Ensures the instanced-quad mvp uniform and its bind group exist.
    ///
    /// The buffer never grows: every call writes one 64-byte matrix. It is created once per
    /// renderer, matching the rest of this cache (REND-02).
    pub(crate) fn ensure_transformed_uniform(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) {
        if self.transformed_uniform.is_some() {
            return;
        }
        self.transformed_uniform = Some(create_matrix_uniform(device, layout, "transformed_mvp"));
    }

    /// Ensures the mesh mvp uniform and its bind group exist.
    ///
    /// Separate from [`Self::ensure_transformed_uniform`] so a 2D transformed draw and a mesh draw
    /// never race for the same 64-byte buffer within one frame.
    #[cfg(feature = "3d")]
    pub(crate) fn ensure_mesh_uniform(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) {
        if self.mesh_uniform.is_some() {
            return;
        }
        self.mesh_uniform = Some(create_matrix_uniform(device, layout, "mesh_mvp"));
    }

    /// Ensures the `Depth32Float` attachment matches `(width, height)`.
    #[cfg(feature = "3d")]
    pub(crate) fn ensure_depth_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let matches = self
            .depth_target
            .as_ref()
            .map(|target| target.width == width && target.height == height)
            .unwrap_or(false);
        if matches {
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mesh_depth_texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.depth_target = Some(DepthTarget { width, height, view });
    }

    /// Ensures the mesh index buffer holds at least `bytes`.
    #[cfg(feature = "3d")]
    pub(crate) fn ensure_mesh_indices(&mut self, device: &wgpu::Device, bytes: u64) {
        if self.mesh_indices.as_ref().map(|cache| cache.capacity >= bytes).unwrap_or(false) {
            return;
        }
        let capacity = bytes.max(256).next_power_of_two();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh_indices"),
            size: capacity,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.mesh_indices = Some(VertexCache { buffer, capacity });
    }

    /// Ensures the circle arena can hold `circles` two-slot records (G-219).
    ///
    /// The two bind-group entries point at the same buffer; the per-draw dynamic offsets
    /// select the colour and parameter slots. The stride is the device's dynamic-offset
    /// alignment, exactly like the colour arena, so every offset stays legal on any backend.
    pub(crate) fn ensure_circle_uniforms(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        circles: usize,
    ) -> Result<(), String> {
        let stride = Self::uniform_stride(device);
        let slots = (circles.saturating_mul(2)).max(1) as u64;
        let needed = stride.checked_mul(slots).ok_or_else(|| {
            format!("{slots} circle uniform slots overflow the uniform buffer size")
        })?;
        let needs_new =
            self.circle_uniforms.as_ref().map(|arena| arena.capacity < needed).unwrap_or(true);
        if needs_new {
            if needed > device.limits().max_buffer_size {
                return Err(format!("{needed} uniform bytes exceed the device buffer limit"));
            }
            let capacity = grown_buffer_capacity(
                self.circle_uniforms.as_ref().map_or(0, |arena| arena.capacity),
                needed,
                device.limits().max_buffer_size,
            );
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("circle_sdf_ubo"),
                size: capacity,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("circle_sdf_bind_group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(16),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(16),
                        }),
                    },
                ],
            });
            self.circle_uniforms = Some(CircleUniformArena { buffer, capacity, bind_group });
        }
        Ok(())
    }
}

/// Byte length of an RGBA8 image of `width × height` pixels.
///
/// Computed in `usize` with checked multiplication: in `u32` the product wraps
/// (e.g. 65536×16384 = 2^32), which would hand the GPU a buffer one byte long for
/// a 1 GiB image. Returns an explicit error on overflow, a zero dimension, or a
/// size beyond the largest supported framebuffer (N-S-66).
pub(crate) fn rgba_buffer_len(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 {
        return Err(format!("width/height must be > 0, got {width}x{height}"));
    }
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| format!("image {width}x{height} pixel count overflows usize"))?;
    let bytes = pixels
        .checked_mul(4)
        .ok_or_else(|| format!("image {width}x{height} RGBA size overflows usize"))?;
    if bytes > u32::MAX as usize {
        return Err(format!(
            "image {width}x{height} needs {bytes} bytes, which exceeds the largest supported \
             framebuffer ({} bytes)",
            u32::MAX
        ));
    }
    Ok(bytes)
}

/// Builds one shadow layer at `rect`'s extent: the two textures, the body matrix uniform, the two
/// directional blur bind groups and the composite bind group.
///
/// `rect` is in frame pixels and its origin rides in the composite params, so the layer itself only
/// ever knows its own local grid. That is what allows the same box-blur shader to clamp its window
/// to the layer's edges: those edges are exactly the CPU rasteriser's layer edges.
/// The box widths a frame-region blur actually runs, in pass order.
///
/// The reference skips a pass whose width is below two (`box_blur_region`'s `if width < 2`), which
/// happens for a radius so small that `box_blur_widths` returns `(1, 1, 3)`. The expansion is
/// computed from **all three** widths regardless, because `box_blur_region` sums their reaches.
pub(crate) fn frame_blur_pass_widths(radius: u32) -> impl Iterator<Item = usize> {
    let (b0, b1, b2) = crate::render::box_blur_widths(radius as f32 / 2.0);
    [b0, b1, b2].into_iter().filter(|width| *width >= 2)
}

/// How many passes [`frame_blur_pass_widths`] yields.
///
/// Derived from the same filter, so a caller that needs only the count does not build a
/// `Vec` just to read its length (W5-43).
pub(crate) fn frame_blur_pass_count(radius: u32) -> usize {
    frame_blur_pass_widths(radius).count()
}

/// How far a frame-region blur reaches beyond its region, in pixels.
pub(crate) fn frame_blur_reach(radius: u32) -> u32 {
    let (b0, b1, b2) = crate::render::box_blur_widths(radius as f32 / 2.0);
    (b0.div_ceil(2) + b1.div_ceil(2) + b2.div_ceil(2)) as u32
}

/// Builds the two textures and six passes one in-place frame blur needs.
///
/// The passes read the texture the *previous* pass wrote, so entry `i` binds `view` for an even `i`
/// and `scratch_view` for an odd one: horizontal, vertical, horizontal, vertical, horizontal,
/// vertical. Each pass carries its own uniform because all six are encoded into one submission.
fn create_frame_blur_target(
    device: &wgpu::Device,
    blur_layout: &wgpu::BindGroupLayout,
    rect: PixelRect,
    radius: u32,
    index: usize,
) -> FrameBlurTarget {
    let make_texture = |label: String| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label.as_str()),
            size: wgpu::Extent3d {
                width: rect.width,
                height: rect.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            // `COPY_SRC` + `COPY_DST` are what make this an in-place region blur: the region arrives
            // by a texture copy from the frame and leaves the same way.
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    };
    let texture = make_texture(format!("wgpu_frame_blur_{index}"));
    // The scratch texture is owned by its view, which is what the passes bind and render into, so it
    // needs no field of its own.
    let scratch_texture = make_texture(format!("wgpu_frame_blur_scratch_{index}"));
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let scratch_view = scratch_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut passes = Vec::with_capacity(6);
    for _width in frame_blur_pass_widths(radius) {
        for _direction in [[1.0f32, 0.0], [0.0, 1.0]] {
            let params = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("wgpu_frame_blur_params"),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let source = if passes.len() % 2 == 0 { &view } else { &scratch_view };
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("wgpu_frame_blur_group"),
                layout: blur_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &params,
                            offset: 0,
                            size: wgpu::BufferSize::new(32),
                        }),
                    },
                ],
            });
            passes.push((bind_group, params));
        }
    }
    FrameBlurTarget { rect, texture, view, scratch_view, passes }
}

fn create_shadow_layer(
    device: &wgpu::Device,
    blur_layout: &wgpu::BindGroupLayout,
    image_layout: &wgpu::BindGroupLayout,
    mvp_layout: &wgpu::BindGroupLayout,
    rect: PixelRect,
    index: usize,
) -> ShadowLayer {
    let make_texture = |label: String| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label.as_str()),
            size: wgpu::Extent3d {
                width: rect.width,
                height: rect.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    };
    let view = make_texture(format!("wgpu_shadow_layer_{index}"))
        .create_view(&wgpu::TextureViewDescriptor::default());
    let scratch_view = make_texture(format!("wgpu_shadow_scratch_{index}"))
        .create_view(&wgpu::TextureViewDescriptor::default());
    let body_uniform =
        create_matrix_uniform(device, mvp_layout, &format!("wgpu_shadow_body_{index}"));

    let make_params = |label: String| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label.as_str()),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    };
    let horizontal_params = make_params(format!("wgpu_shadow_horizontal_params_{index}"));
    let vertical_params = make_params(format!("wgpu_shadow_vertical_params_{index}"));
    let make_blur_group = |label: String, source: &wgpu::TextureView, buffer: &wgpu::Buffer| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label.as_str()),
            layout: blur_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(32),
                    }),
                },
            ],
        })
    };
    let horizontal =
        make_blur_group(format!("wgpu_shadow_horizontal_{index}"), &view, &horizontal_params);
    let vertical =
        make_blur_group(format!("wgpu_shadow_vertical_{index}"), &scratch_view, &vertical_params);

    // The composite draws the layer at its own frame rectangle and reads it 1:1: the image
    // shader maps `local * source / dest`, and both are the layer's grid.
    let composite_params = make_params(format!("wgpu_shadow_composite_params_{index}"));
    let composite = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(format!("wgpu_shadow_composite_{index}").as_str()),
        layout: image_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &composite_params,
                    offset: 0,
                    size: wgpu::BufferSize::new(32),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
        ],
    });

    ShadowLayer {
        width: rect.width,
        height: rect.height,
        view,
        scratch_view,
        body_uniform,
        horizontal,
        horizontal_params,
        vertical,
        vertical_params,
        composite,
        composite_params,
    }
}

/// Quantises a unit-interval colour to the 8-bit RGBA the rasteriser consumes.
pub(crate) fn rgba8_from_unit(color: [f64; 4]) -> Rgba8 {
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0) as u8;
    Rgba8 { r: channel(color[0]), g: channel(color[1]), b: channel(color[2]), a: channel(color[3]) }
}

/// Bytes per row of an RGBA8 image, aligned to WebGPU's copy requirement.
pub(crate) fn padded_row_bytes(width: u32) -> Result<u32, String> {
    let unpadded = width
        .checked_mul(4)
        .ok_or_else(|| format!("row byte width for {width} pixels overflows u32"))?;
    Ok(align_to(unpadded, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
}

/// Total bytes of a padded RGBA8 readback buffer.
pub(crate) fn readback_frame_bytes(width: u32, height: u32) -> Result<u64, String> {
    let padded = padded_row_bytes(width)?;
    (padded as u64)
        .checked_mul(height as u64)
        .ok_or_else(|| format!("readback buffer for {width}x{height} overflows u64"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(ptr: usize) -> TextureKey {
        TextureKey { ptr, len: 4, width: 1, height: 1 }
    }

    #[test]
    fn a_second_pass_over_the_same_generation_keeps_the_groups_it_built() {
        let mut map = GenerationMap::<u32>::default();
        map.prepare(7);
        map.insert(7, ParamsFamily::Image, key(1), 11);
        map.prepare(7);
        assert_eq!(map.entries.len(), 1, "the same arena must not drop its groups");
        assert_eq!(map.get(7, ParamsFamily::Image, &key(1)), Some(&11));
    }

    #[test]
    fn a_new_arena_generation_drops_the_groups_that_bound_the_old_buffer() {
        let mut map = GenerationMap::<u32>::default();
        map.prepare(7);
        map.insert(7, ParamsFamily::Gradient, key(2), 22);
        map.prepare(8);
        assert!(map.entries.is_empty(), "a rebuilt arena invalidates every group");
        assert_eq!(map.get(7, ParamsFamily::Gradient, &key(2)), None);
        assert_eq!(map.get(8, ParamsFamily::Gradient, &key(2)), None);
    }

    #[test]
    fn reaching_the_cap_starts_over_only_for_a_new_key() {
        let mut map = GenerationMap::<usize>::default();
        map.prepare(1);
        for index in 0..MAX_GROUPS {
            map.insert(1, ParamsFamily::Image, key(index), index);
        }
        assert_eq!(map.entries.len(), MAX_GROUPS);

        // Replacing a key the map already holds does not trip the cap.
        map.insert(1, ParamsFamily::Image, key(0), 99);
        assert_eq!(map.entries.len(), MAX_GROUPS);
        assert_eq!(map.get(1, ParamsFamily::Image, &key(0)), Some(&99));

        // A genuinely new key at the cap clears the map before storing it.
        map.insert(1, ParamsFamily::Radial, key(MAX_GROUPS), MAX_GROUPS);
        assert_eq!(map.entries.len(), 1);
        assert_eq!(map.get(1, ParamsFamily::Image, &key(0)), None);
    }
}
