# final-p3 — true 3D (mesh / camera / depth buffer) — report

Source of truth: `docs/issues/slices/final-c29-p3.md` §P3 and `docs/issues/issue-20261023.md`
PART 4 §4.3 (3D) + decision **C6** ("本文件实施过程中酌情完整实现"). Scope respected:
`src/render/**` (not `src/render/text/**`), `src/wgpu_backend/**`, and gated additions in
`src/widget/base.rs`. `Cargo.toml`, `src/theme/**`, `src/platform/**`,
`src/widget/capability/**` and other docs were not edited.

CLOSED: P3-1, P3-2, P3-3, P3-4, P3-5, P3-6, P3-7

OPEN: none against the requested scope. Documented residuals are listed at the end; they are
limits of the chosen design, not missing work items.

No `SCHEMA | …` rows: the 3D capability is a render command plus a gated `BaseWidget` API, not a
widget property. No control outside `base.rs` advertises it, so nothing in
`src/widget/capability/**` needs a new row.

## What changed

| # | Deliverable | Files |
|---|---|---|
| P3-1 | Mesh/camera data types: `Vec3`, `Vertex3D` (position + colour), validated indexed `Mesh3D`, `Camera` (eye/target/up/fov/near/far) with `view`/`projection`/`view_projection`, plus the shared `project_point`/`project_mesh`/`viewport_matrix` | `src/render/mesh3d.rs` (new), `src/render/math3d.rs` (`Mat4::transform_point`), `src/render/mod.rs` |
| P3-2 | Storable command `RenderCommand::DrawMesh3D { mesh, model, camera, viewport }` under `#[cfg(feature = "3d")]` | `src/render/core/command.rs` |
| P3-3 | Depth-buffered software rasteriser: clip-space transform, perspective divide, barycentric coverage, interpolated depth, documented back-face policy | `src/render/pipeline/mesh.rs` (new), `src/render/backend/paint.rs`, `src/render/backend/surface.rs`, `src/render/pipeline/containers.rs` |
| P3-4 | SVG backend: shared projection + far-to-near triangle sort, one flat-shaded `<path>` per triangle | `src/render/svg/backend.rs` |
| P3-5 | wgpu: `MESH3D_VS`/`MESH3D_FS`, a `Depth32Float` pipeline (`Less`, clear `1.0`), cached vertex/index/mvp/depth resources, `render_mesh_3d_gpu` | `src/wgpu_backend/shaders.rs`, `src/wgpu_backend/renderer.rs` |
| P3-6 | Gated camera/mesh API on `BaseWidget` and a cube-drawing test | `src/widget/base.rs` |
| P3-7 | Tests: cube non-empty, near/far occlusion in both triangle and command order, known-point perspective, 3D-off byte identity | per-file test modules below |

## Design statements (the facts a reviewer needs)

**Projection conventions.** `Camera` is right-handed, looking down its own `-z`.
`Mat4::perspective` is the existing conventional `-1..=1` depth projection.
`viewport_matrix(viewport, target)` maps NDC onto the viewport rectangle **and** remaps depth to
`0..=1`. That remap is required by the WebGPU depth attachment and is used by the software
z-buffer too, so "smaller depth is nearer" is one statement for every backend. The software
rasteriser maps NDC to top-left-origin surface pixels (`y` flipped); the SVG backend consumes the
same `ProjectedTriangle` values.

**Clip policy.** A triangle is skipped whole when any vertex has `w <= 1e-6`; geometry is not
clipped against the near plane. Per-pixel fragments outside `0..=1` depth are rejected. This is
documented on `project_mesh` and `draw_mesh_3d`.

**Back-face policy.** No triangle is culled, in software, SVG or wgpu. The depth test decides
visibility. This keeps a mesh with inconsistent winding (or a deliberately two-sided surface)
correct, and removes the need for the renderer and the mesh author to agree about winding. A
closed cube therefore shows its three near faces and hides its three far faces.

**Occlusion and draw order.** The software surface owns one `f32` depth buffer per frame
(`SoftwareSurface::depth_buffer`), marked stale in `begin_frame`/`resize` and refilled with the
far value on the first mesh draw. Two mesh commands in one frame are therefore ordered by depth,
not recording order. A 2D command after a mesh paints over it in painter order because flat
drawing has no depth to compare; this is documented on the command.

**Depth interpolation.** For a standard perspective matrix, NDC depth is affine in screen space,
so the same barycentric weights that determine coverage interpolate depth exactly. No separate
perspective-correct attribute pass is needed because the mesh carries no texture coordinates.

**Flat shading.** `ProjectedTriangle::color` is the mean of the triangle's vertex colours. Both
the software rasteriser and the SVG backend paint that one colour per triangle, so a snapshot and
a frame agree pixel-for-pixel on the visible triangles. The GPU interpolates per-vertex colour;
for a flat-shaded mesh (e.g. `Mesh3D::cube`) the pictures are identical.

**SVG ordering.** SVG has no depth buffer, so `emit_mesh_3d` sorts triangles by mean projected
depth, farthest first (painter's algorithm). This is exact for a closed, non-intersecting mesh
such as a cube; it is an approximation for interpenetrating triangles, where the software path is
the reference.

**Goldens / feature off.** Every new item is behind `#[cfg(feature = "3d")]` (module
declarations, command variant, surface fields, rasteriser module, wgpu pipeline/shaders/resources,
`BaseWidget` fields and methods). With `3d` off the code does not exist, so the 2D picture cannot
change. The committed snapshots were run in both matrices (below) and matched byte for byte.

## Validation (actual commands and results)

Build/type-check:

- `cargo check --lib` — clean (`Finished dev profile`).
- `cargo check --lib --no-default-features --features "desktop,3d"` — clean.
- `cargo check --lib --no-default-features --features "desktop,3d,gpu-wgpu"` — clean; this is the
  configuration that actually compiles `src/wgpu_backend/**` (note: `desktop` alone enables
  `wgpu = ["gpu"]`, **not** `gpu-wgpu`, so the mesh pipeline needed the explicit third feature to
  be type-checked).
- `cargo check --lib --features "gpu-wgpu"` — clean (3D off, GPU 2D path unchanged).
- `cargo check --no-default-features --features "desktop,3d" --all-targets` — clean (the CI
  "3d on" command; it compiles tests and examples as well).

Targeted tests, `--no-default-features --features "desktop,3d"`:

- `render::mesh3d` — **8 passed, 0 failed**
  (`cube_has_twelve_triangles_and_valid_indices`, `mesh_rejects_malformed_index_lists`,
  `a_degenerate_cube_is_still_a_valid_mesh`,
  `camera_view_falls_back_to_identity_for_a_degenerate_basis`,
  `perspective_projection_puts_a_known_point_at_the_expected_pixel`,
  `projection_lands_inside_the_requested_viewport`,
  `a_triangle_behind_the_eye_is_dropped_whole`,
  `a_triangles_flat_colour_is_the_mean_of_its_vertices`).
- `render::pipeline::mesh` — **10 passed, 0 failed**
  (`a_nearer_triangle_occludes_a_farther_one_in_either_triangle_order`,
  `a_nearer_mesh_occludes_a_farther_one_in_either_command_order`,
  `a_far_triangle_alone_is_visible`,
  `a_cube_draws_a_non_empty_picture_through_the_command_path`,
  `a_mesh_outside_its_viewport_paints_nothing_inside_the_viewport_elsewhere`,
  `a_two_dimensional_command_after_a_mesh_paints_over_it`,
  `a_mesh_is_clipped_by_the_surface_clip_stack`, `an_empty_mesh_or_viewport_draws_nothing`,
  `a_point_outside_the_triangle_is_not_painted`, `the_viewport_origin_moves_the_picture`).
- `render::svg::backend::tests::svg_backend_paints_a_depth_sorted_cube` — 1 passed
  (12 `<path>` elements; the far `-Z` face precedes the near `+Z` face in the document).
- `render::svg::backend::tests::every_render_command_variant_reaches_the_svg_backend` — 1 passed
  (the exhaustive `name_of` guard now names `DrawMesh3D` and the fixture list drives it).
- `render::math3d::tests::transform_point_returns_all_four_clip_coordinates` — 1 passed.
- `widget::base::tests::a_base_widget_draws_its_mesh_through_its_camera` — 1 passed (the P3-6
  example: a `BaseWidget` cube reaches the surface through `mesh_3d_command`).
- `widget::base::tests::mesh_3d_is_empty_until_a_mesh_and_a_camera_are_set`,
  `mesh_and_camera_setters_request_repaint_only_on_change` — 2 passed.

GPU (real adapter on this host, not a skip):

- `cargo test --lib --no-default-features --features "desktop,3d,gpu-wgpu"
  the_gpu_mesh_pipeline` — **1 passed**; the test asserts a non-empty painted area from the
  depth-tested indexed pipeline.

Feature-off / snapshot identity:

- `cargo test --lib render::` (default build, 3D off) — **464 passed, 0 failed**.
- `cargo test --test snapshot_tests` (default) — **4 passed**.
- `cargo test --test snapshot_tests --no-default-features --features "desktop,3d"` — **4 passed**
  against the same committed baselines (zero diff with the feature on, hence also with it off).

Clippy:

- `cargo clippy --lib --no-default-features --features "desktop,3d,gpu-wgpu" -- -D warnings` —
  no findings in `src/render/**` (except `src/render/text/**`), `src/wgpu_backend/**` or
  `src/widget/base.rs`. One remaining error is in `src/render/text/variable.rs`, owned by the
  parallel C29 slice.

Broad suite in the shared worktree:

- `cargo test --lib --no-default-features --features "desktop,3d"` — 8860 passed; the only failure
  was `theme::manager::tests::a_theme_mutation_arms_a_full_repaint` (a repaint-count assertion in
  `src/theme/manager.rs`, which the parallel C29 slice is editing concurrently). The earlier run
  failed the same way in `src/render/text/variable.rs`; both files are outside this slice and were
  not touched here.

## Documented residuals (design limits, not open work)

1. No geometric near-plane clipping: a triangle with any vertex behind the view plane is dropped
   whole rather than clipped. Sufficient for a mesh in front of the camera; a large world-space
   mesh that intersects the eye loses the offending triangles.
2. Mesh edges are hard (no antialiasing) in the software rasteriser. The brief made AA optional
   and occlusion mandatory; occlusion is exact.
3. Software/SVG shading is flat (mean vertex colour). The GPU interpolates per-vertex colour; a
   mesh with a colour gradient across a face therefore differs between the GPU and the two
   reference backends, while a flat-shaded mesh (including `Mesh3D::cube`) agrees.
4. The SVG backend uses painter's algorithm, which fails for interpenetrating triangles; the
   software z-buffer does not.
5. A 2D command drawn after a mesh paints over it. There is no combined 2D/3D depth interaction;
   only mesh triangles are depth-ordered.
6. No texture coordinates, lighting, materials or per-control 3D clip shape. None were requested.
