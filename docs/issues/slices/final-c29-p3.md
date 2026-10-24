# Final items: C29 phase 2 + P3 true 3D

Source of truth: `docs/issues/issue-20261023.md` §4.3 P2-3 and PART 4 §4.3 (3D).
Both are gated/opt-in; the default build must stay byte-identical.

## C29 phase 2 — variable-font (fvar/gvar) instantiation

The static-weight phase is already in place (runtime face registration, `Font.weight`,
nearest-weight selection, `GlyphKey.weight`). What remains is eating a variable face such
as the system UI font (`SFNS.ttf`), which is a single face with a `wght` axis.

Required:

1. A `VariableFace`-level API that:
   - lists `fvar` axes for a family (tag, min/default/max, name) — `ttf_parser::Face::variation_axes`;
   - instantiates a parsed face at a normalized coordinate set before outlining/measuring;
   - maps `Font::weight()` (100..=900) onto the `wght` axis, clamped to the axis range;
   - honours `avar`/`gvar` implicitly through `ttf-parser` (do not re-implement glyph deltas).
2. Wire the instance through the real paint path:
   - `outline`/`outline_glyph`/rasterisation must apply the weight variation before
     `outline_glyph`;
   - shaping/advance must use the same instance, or the measured advance and the drawn
     outline disagree (a test must pin this).
3. System UI font supply on macOS: register the variable face once (one slot, not four),
   with an honest fallback when the file is absent. No fabricated family name on failure.
4. Feature gating: absent `ttf-parser` (no `text-shaping`) or absent runtime fonts must still
   compile, and the default build must not change a single pixel.
5. Tests:
   - a synthetic variable face (build a minimal fvar/gvar fixture in the test, or parse a
     small committed fixture) reports its axis and produces different outlines at 400 vs 700;
   - weight clamping at the axis bounds;
   - missing axis/face answers `None`/fallback rather than panicking;
   - the existing SVG snapshots stay identical with the feature off.

Exclusive file scope: `src/render/text/**`, `src/core/font.rs`, `src/theme/manager.rs`,
`src/platform/macos/fonts.rs` (new allowed), `Cargo.toml`.
Do NOT touch `src/render/mod.rs`, `src/render/core/**`, `src/render/backend/**`,
`src/render/pipeline/**`, `src/wgpu_backend/**`, `src/widget/capability/**`.

## P3 — true 3D (depth buffer / mesh / camera), behind `3d`

Required, all `#[cfg(feature = "3d")]` except any pure-data type that can live ungated:

1. `mesh3d`: vertex (position + colour) and indexed triangle mesh; a camera with
   look-at + perspective producing a view-projection `Mat4`; model transform.
2. A storable render command for a 3D mesh (`RenderCommand::DrawMesh3D` or equivalent).
3. Software rasteriser: clip-space transform, perspective divide, triangle setup,
   **z-buffer** depth test with correct interpolated depth, and back-face handling that is
   documented. AA is optional; correct occlusion is mandatory.
4. SVG backend: project the mesh and paint depth-sorted triangles (painter's algorithm),
   so a headless snapshot shows a real 3D object.
5. wgpu backend: a depth-texture-backed pipeline for the mesh command. If the wgpu feature
   is not exercised on this host, the code must still type-check under `gpu-wgpu`.
6. Camera API on the widget/base layer (gated) so a control can be drawn from a 3D camera.
7. Tests:
   - a cube/quad at a known camera renders non-empty pixels;
   - a nearer triangle occludes a farther one (depth test), including the reversed draw order;
   - perspective divide puts a point at the expected screen position;
   - with `3d` off, nothing compiles in and every snapshot is byte-identical.

Exclusive file scope: `src/render/**` except `src/render/text/**`, plus
`src/wgpu_backend/**`, `src/widget/base.rs` (gated additions only).
Do NOT touch `Cargo.toml`, `src/render/text/**`, `src/theme/**`, `src/platform/**`,
`src/widget/capability/**`, or any control outside `base.rs`.

## Both

- English comments only; no third-party toolkit names anywhere.
- No `todo!`/`unimplemented!`/empty bodies; `unsafe` needs `SAFETY:`.
- Every public item documented; new behaviour gets a unit test in an existing test module.
- Setters early-return and request a redraw when pixels can change.
- Run `cargo check --lib`, the targeted tests, and (for P3) both `3d` on/off test runs.
- Write `docs/issues/slices/final-<track>-report.md` with CLOSED/OPEN evidence and any
  schema rows needed (the main thread owns `widget/capability/**`).
