# group-f-render — report

Scope: `src/render/**`, `src/wgpu_backend/**`, the `3d` item API in `src/widget/base.rs`, the
`device_profile` gate in `src/widget/runtime.rs`, the feature wiring in `Cargo.toml`, and the
R461 entries in `.github/workflows/ci.yml`.

R441–R450 are macOS rows whose implementation lives outside this slice's file scope. They are
listed as verified because their mechanisms exist on disk and were reported by
`group-e-cross-report.md`; R446 is now also implemented and registered in the shared worktree by
the macOS widget slice. No file outside this slice's scope was edited for them.

CLOSED: R433, R434, R435, R436, R437, R438, R439, R440, R441, R442, R443, R444, R445, R446, R447, R448, R449, R450, R451, R452, R453, R454, R455, R456, R457, R458, R459, R460, R461

OPEN: none.

No `SCHEMA | …` rows: every addition here is a render command or backend API, not a widget
property.

## Row notes

- R433 — verified closed (previously reported by `group-e-cross-report.md`): `render::math3d` is
  a real matrix module (`src/render/math3d.rs`, `#![cfg(feature = "3d")]`), `TransformStack` is
  composed by `BatchCommand::PushTransform`/`PopTransform`, and widgets carry an item `Mat3`
  (`src/widget/base.rs`). `projection` is still the presentation-mode controller, so the naming
  collision is resolved rather than hidden.
- R434 — closed: `ClipPath` no longer approximates with its bounding box.
  `SoftwareSurface::push_path_clip` (`src/render/pipeline/containers.rs`) pushes a rectangle clip
  plus a 4x-subsampled non-zero-winding coverage mask and captures the backdrop; `pop_clip`
  applies the mask, restoring uncovered pixels and compositing partial coverage
  (`apply_path_clip`). `RenderCommand::ClipPath` routes there (`src/render/backend/paint.rs`), and
  the SVG backend opens a clip group even for a degenerate path so `PopClip` stays balanced.
  Tests: `containers::tests::a_path_clip_keeps_only_the_pixels_the_path_covers`,
  `popping_a_path_clip_restores_the_backdrop_for_the_next_draw`,
  `a_degenerate_path_clip_draws_nothing`.
- R435 — verified closed (previously reported): coverage antialiasing for circles, rounded rects,
  strokes and glyph coverage is the paint path (`src/render/pipeline/pixel_ops.rs`,
  `src/render/pipeline/primitives.rs`); assessment row, no change required.
- R436 — closed: `BackBuffer` owns a damage list and a region present
  (`mark_damage`/`damage_regions`/`clear_damage`/`present_damage`,
  `src/render/backend/surface.rs`), forwarded by `SoftwareSurface`
  (`src/render/pipeline/containers.rs`) and by `PaintBackend`
  (`src/render/backend/paint.rs`). A full-surface region falls back to the O(1) swap; every other
  present copies only the reported rows. Tests:
  `surface::tests::present_damage_copies_only_the_reported_region`,
  `present_damage_with_no_regions_is_a_no_op`, `mark_damage_clips_and_deduplicates`,
  `whole_surface_damage_presents_by_swapping`, `a_surface_presents_a_damaged_region`.
- R437 — closed: `RenderCommand::PushLayerGroup { bounds, effect, opacity, cache_key }`
  (`src/render/core/command.rs`) adds subtree opacity and a caller-keyed replay cache.
  `begin_layer`/`pop_layer` (`src/render/backend/paint.rs`) isolate the foreground, scale its alpha
  as one unit, restore a fully transparent layer's backdrop, and store/reuse the foreground in
  `BackBuffer::layer_cache` (bounded, cleared on resize). Suppressed commands still count nested
  push/pop pairs so the matching pop is found without disturbing the clip stack. The SVG backend
  emits a group `opacity`. Tests:
  `paint::tests::a_layer_group_applies_its_opacity_to_the_subtree`,
  `a_cached_layer_reuses_its_foreground_until_the_key_changes`,
  `changing_a_cached_layer_opacity_redraws_it`,
  `clearing_the_layer_cache_forces_a_redraw`,
  `a_fully_transparent_layer_keeps_its_backdrop`.
- R438 — closed: a real separable Gaussian pass on the GPU. `BLUR_VS`/`BLUR_FS`
  (`src/wgpu_backend/shaders.rs`) run horizontal then vertical over two cached textures
  (`render_blur_rgba8`, `src/wgpu_backend/renderer.rs`); `render_backdrop_blur_rgba8` now calls it
  instead of the software box blur. Tests:
  `renderer::tests::a_zero_radius_blur_returns_the_input`,
  `a_uniform_field_survives_the_gpu_blur`, `the_gpu_blur_spreads_an_edge_symmetrically`.
- R439 — closed (opt-in, default off so the committed SVG snapshots stay byte-identical): the
  shaper can shape with the face's standard ligatures
  (`RustybuzzShaper::shape_face_with_ligatures`, `glyph_run_for_family` in
  `src/render/text/shaping.rs`) and the rasteriser can outline a **glyph id**
  (`outline_glyph`, `src/render/text/raster.rs`). `SoftwareSurface::set_glyph_run_text(true)` makes
  `draw_text` paint the shaped run glyph by glyph through the non-zero fill
  (`src/render/pipeline/primitives.rs`), so a ligature is one shape and the shaper's mark offsets
  are honoured. Tests: `surface::tests::glyph_run_text_paints_the_shapers_ligature`,
  `glyph_run_text_falls_back_for_an_unknown_family`.
- R440 — closed as an index row: its two named bottlenecks are R436 (damage regions) and R437
  (layer cache/opacity), both implemented above.
- R441 — verified closed: `themes/macos-light.json` and `themes/macos-dark.json` exist and
  `themes/generate.sh` registers them.
- R442 — verified closed: `Theme::macos_light`/`macos_dark`, `SystemAccent::ALL`, and
  `Platform::accent_color` exist (`src/theme/manager.rs`, `src/theme/types.rs`,
  `src/platform/types/traits.rs`).
- R443 — verified closed: `Motion { fast, normal, slow, easing, curve }` with
  `MotionCurve::{Easing, Spring}` (`src/theme/types.rs`, `src/style/animation.rs`).
- R444 — verified closed: `Environment` carries `system_accent` and reduced transparency, and the
  macOS backends implement both (`src/style/environment.rs`, `src/platform/macos/environment.rs`).
- R445 — verified closed: `ControlMetricsProfile::{Material, Macos, Cupertino}` and
  `profile_metrics` exist (`src/widget/metrics.rs`), and button/switch/text-field read them.
- R446 — verified closed in the shared worktree (implemented by the macOS widget slice, outside
  this slice's files): `src/widget/macos/**` defines `MacSegmentedControl`, `MacSourceList`,
  `MacStepper`, `MacToolbar` and `MacWindowChrome`, and
  `src/widget/capability/registration.rs:250-254` registers all five with factory constructors
  (`constructors.rs:1303+`) and capability rows (`properties.rs:2968+`).
- R447 — verified closed: `FocusRingStyle::{Inner, Halo}` with `FOCUS_HALO_BLUR_RADIUS`
  (`src/widget/metrics.rs`).
- R448 — verified closed: `ScrollBarStyle::{Arrows, Overlay}`
  (`src/widget/display_widgets/scrollbar.rs`).
- R449 — verified closed: the popover shadow uses `draw_box_shadow`, and vibrancy has a real
  primitive (`RenderCommand::PushLayer` + `LayerEffect::BackdropBlur{,Tint}`), which R438 now backs
  with a GPU pass.
- R450 — verified closed: `THEME_CROSSFADE_DEFAULT_MS = 220` and
  `set_theme_with_crossfade`/`tick_theme_crossfade` (`src/theme/manager.rs`).
- R451 — verified closed: the 3D capability now exists without a depth buffer by design —
  `Mat3`/`Mat4`, `FillQuad`/`StrokeQuad`/`DrawImageQuad`, the instanced `uMVP` pipeline
  (`FILL_RECT_VS_TRANSFORMED`, `src/wgpu_backend/shaders.rs`), and the batch transform stack;
  control-level occlusion stays with scene-layer z order (R456 P3 defers a depth attachment).
- R452 — verified closed: `render::math3d` exists under `3d`; `projection` remains the
  presentation-mode controller.
- R453 — verified closed: rectangles are instanced (`encode_quads_instanced`,
  `src/wgpu_backend/renderer.rs`) with one instance buffer and one draw per run.
- R454 — verified closed: `Bevel Raised/Inset`, gradients, real box-shadow blur and
  `SceneLayer.z_index` exist, and `TransformState` carries the affine stack
  (`src/render/backend/batch.rs`).
- R455 — closed: ① (matrix chain), ③ (painter-order depth), ④ (layers) and ⑤
  (bevel/gradient lighting) were already present. ② is now implemented: a projective fill is
  emitted as `RenderCommand::FillQuadAA`, and `SoftwareSurface::fill_quad_aa`
  (`src/render/pipeline/primitives.rs`) rasterises the two triangles with 4x4 per-pixel coverage
  from the projected edges (top-left rule so a translucent quad has no double-blended diagonal),
  not from the bounding box. Tests:
  `surface::tests::a_projectively_divided_quad_antialiases_its_projected_edge`,
  `batch::tests::a_projective_fill_reaches_a_pixel_and_keeps_its_perspective`.
  ⑥ **documented residual**: text under a projective transform stays an approximation. The batch
  replay anchors the run at the projectively mapped origin and scales its point size by the
  projected unit-square edge length (`TransformState::projective_length_scale`), so it moves and
  resizes with the card; glyph outlines are **not** warped, the baseline stays upright, and text
  is not rasterised into a layer. Closing that residual means rasterising the text run into an
  offscreen layer and transforming the layer (R456 P1's layer pipeline, reused rather than a new
  one).
- R456 — closed: P0 (affine stack in batch/software/wgpu) and P1 (`PushLayer`/`PopLayer` + backdrop
  blur) were implemented. P2 is now implemented:
  `BaseWidget::set_rotation(axis, angle_radians, origin)` under `#[cfg(feature = "3d")]` builds a
  `Mat4` that turns the control's plane in front of a fixed eye
  (`ITEM_PERSPECTIVE_DISTANCE`), projects it with `Mat4::perspective`, and divides by `w`
  (`src/widget/base.rs`, `item_projective_matrix`). `item_rotation()` returns the settled
  `(axis, angle, origin)`, `item_projective_transform()` returns the composite, and the composition
  order is documented and fixed: **affine first, then the rotation about `origin`**, each setter
  leaving the other's parameters untouched. `BatchCommand::PushProjectiveTransform { matrix }` /
  `PopProjectiveTransform` compose it into `TransformState`, which maps corners through
  `Mat4::project_row` + divide and emits `FillQuadAA` for fills, `StrokeQuad` for strokes, a
  homography-mapped `DrawImageQuad` for images, and drops geometry whose `w <= 0`
  (`src/render/backend/batch.rs`). Images whose corners are not a parallelogram are mapped with a
  unit-square homography (`inverse_quad_homography`, `src/render/pipeline/primitives.rs`).
  Tests: `base::tests::item_rotation_maps_corners_with_a_perspective_divide`,
  `a_zero_item_rotation_is_exactly_the_affine_transform`,
  `item_transform_and_rotation_compose_without_erasing_each_other`,
  `item_rotation_rejects_a_non_finite_angle`,
  `batch::tests::a_projective_fill_reaches_a_pixel_and_keeps_its_perspective`,
  `an_identity_projective_push_paints_like_the_2d_path`,
  `a_projective_fill_behind_the_eye_is_dropped`, `an_affine_push_still_emits_a_hard_quad`,
  `surface::tests::a_projective_quad_image_follows_its_narrow_edge`. P3 remains a documented
  non-goal (no depth buffer/mesh/camera).
- R457 — verified closed: the whole capability is behind `3d`, which is not in `default`.
- R458 — verified closed: `3d = []` (`Cargo.toml`) is absent from `default = ["desktop"]`.
- R459 — verified closed: `math3d` is both module-gated (`src/render/mod.rs`) and self-gated
  (`#![cfg(feature = "3d")]`), and every batch/wgpu intrusion is `#[cfg(feature = "3d")]` (112 cfg
  sites across `src/render` and `src/wgpu_backend`, plus the item API in `src/widget/base.rs`).
  `PushLayer`/`PopLayer` are deliberately **not** gated: they are a 2D material-composition
  capability that R449 needs in default builds, and `src/render/core/command.rs` documents that
  decision.
- R460 — verified closed: the 2D path is unchanged with `3d` off.
  `batch::tests::transform_state_is_byte_identical_when_3d_is_off` asserts the 12-byte state, and
  `tests/snapshot_tests.rs` passes byte-identically in both matrices (4/4 in each).
- R461 — closed: the `3d` gate cannot rot in either direction.
  - `src/widget/runtime.rs` now gates the theme-crossfade advance on `#[cfg(device_profile)]`, so
    `cargo check --no-default-features` compiles (it previously failed at
    `crate::theme::global_theme_manager`).
  - `Cargo.toml` adds `all-desktop-features`, the curated "everything a desktop host can use"
    target (`desktop`, `3d`, `gpu-wgpu`, `projection`, `runtime-fonts`, the two `unstable-*`
    flags). The system-library codecs and other device profiles are excluded with a reason.
  - `.github/workflows/ci.yml` has the dual matrix as explicit adjacent steps
    (`--no-default-features --features desktop` and `--features "desktop,3d"`, both
    `--all-targets`) plus an `all-desktop-features` step.
  - Decision C5 is recorded in `Cargo.toml` and in the CI comment: `--all-features` is **not a
    supported command**. Device profiles are mutually exclusive, so it enables `mini` (no_std)
    together with `desktop` and fails with ~970 `String`/`Vec`-not-in-scope errors by construction.
    `all-desktop-features` is the honest all-on target and is what CI builds instead.

## Validation

- `cargo check --lib` — clean.
- `cargo check --no-default-features` — compiles (only the pre-existing unused-import/unused-fn
  warnings in `event`, `widget/base_widgets` and `widget/runtime`; no errors).
- `cargo check --features "desktop,3d"` — clean.
- `cargo check --no-default-features --features desktop --all-targets` — clean.
- `cargo check --no-default-features --features "desktop,3d" --all-targets` — clean.
- `cargo check --no-default-features --features all-desktop-features --all-targets` — clean.
- `cargo test --lib render` — 519 passed, 0 failed.
- `cargo test --lib 3d` — 2 passed, 0 failed (the default build's `3d`-named gate tests).
- `cargo test --lib --features "desktop,3d" render` — 546 passed, 0 failed.
- `cargo test --lib --features "desktop,3d" 3d` — 14 passed, 0 failed.
- `cargo test --lib --features "desktop,3d" item_rotation` — 3 passed, 0 failed.
- `cargo test --lib --features "desktop,3d" widget::base` — 380 passed, 0 failed.
- `cargo test --lib --features "desktop,3d" projective` — 4 passed, 0 failed.
- `cargo test --lib --features "desktop,gpu-wgpu" wgpu_backend::` — 44 passed, 0 failed (real GPU
  adapter; the GPU blur tests ran rather than skipping).
- `cargo test --no-default-features --features desktop --test snapshot_tests` — 4 passed.
- `cargo test --no-default-features --features "desktop,3d" --test snapshot_tests` — 4 passed,
  same baselines (zero-diff evidence for R460).
- `cargo clippy --lib --no-default-features --features "desktop,3d" -- -D warnings` — no findings in
  `src/render`, `src/wgpu_backend`, `src/widget/base.rs` or `src/widget/runtime.rs`. One pre-existing
  finding remains in `src/widget/base_widgets/toggle_button.rs`, which this slice does not own.
- Unrelated pre-existing failure observed while running the broad suite (unchanged by this slice):
  `widget::runtime::tests::a_widget_enabled_by_the_auto_decision_tracks_damage` expects
  `set_geometry` not to count as "ever requested a redraw", but `BaseWidget::set_geometry` calls
  `request_redraw`. It is owned by `src/widget/runtime.rs`'s test fixture, not by the render layer.
