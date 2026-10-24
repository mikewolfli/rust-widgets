# C29 phase 2 — variable-font (`fvar`/`gvar`) instantiation — report

Source of truth: `docs/issues/issue-20261023.md` §4.3 P2-3 and `docs/issues/slices/final-c29-p3.md`
(section "C29 phase 2"). File scope followed as stated there: `src/render/text/**`, `src/core/font.rs`
(read only), `src/theme/manager.rs`, `src/platform/macos/fonts.rs` (not needed — see O-2),
`Cargo.toml`. Nothing under `src/render/mod.rs`, `src/render/core/**`, `src/render/backend/**`,
`src/render/pipeline/**`, `src/wgpu_backend/**`, `src/widget/capability/**`, or `docs/**` (except
this file) was touched.

## Verification run

| command | result |
|---|---|
| `cargo check --lib` | Finished (no warnings) |
| `cargo check --lib --features runtime-fonts` | Finished (no warnings) |
| `cargo check --lib --no-default-features --features mini` | Finished (variable module absent) |
| `cargo check --lib --no-default-features --features "mini,runtime-fonts"` | Finished (one pre-existing `glyph_run_for_family` dead-code warning in that profile, unrelated) |
| `cargo test --lib render::text::` | **92 passed, 0 failed** |
| `cargo test --lib` | **8805 passed, 0 failed** |
| `cargo test --test snapshot_tests` | **4 passed, 0 failed** (SVG bytes unchanged) |
| `cargo test --lib --features runtime-fonts -- the_system_ui_family_maps the_host_system_ui_face` | **2 passed** |
| `bash tools/check_font_data_is_opt_in.sh` | `fonts=11 payloads=11 findings=0` |
| `bash tools/check_font_licenses.sh` | `expected=11 checked=11 failed=0` |

## CLOSED

### 1. Documented variable-face API — CLOSED

New module `src/render/text/variable.rs`, re-exported at `render::text::`:

- `VariableFace::{new, for_family, is_variable, axes, weight_axis, clamp_weight, instantiate_at_weight, instantiate_at}`.
- `VariableAxis { tag, name, min, default, max, hidden }` — `tag` is the four bytes, `name` is read
  from the `name` table's Unicode record (decoded locally so the module compiles without the
  parser's `std` convenience accessor).
- `VariableInstance::{requested_weight, effective_weight, weight_axis, units_per_em, glyph_index, advance, outline_glyph}`.
- `VariableOutlineBuilder` re-export so a caller can implement the builder without adding the parser
  as a dependency.
- `WEIGHT_AXIS_TAG`, `MAX_INSTANCES`.

`instantiate_at` is the explicit coordinate-set form; `instantiate_at_weight` maps
`Font::weight()` onto `wght`, clamped to the axis's declared range. `gvar`/`avar`/`HVAR` are read by
the parser's own variation code — no deltas are re-implemented (requirement 1's "do not
re-implement glyph deltas").

Evidence: `render::text::variable::tests::the_fixture_reports_its_axes_with_names_and_ranges`,
`an_explicit_coordinate_set_instantiates_the_same_outline_as_a_weight`.

### 2. Weight mapping, clamping, missing-axis fallback — CLOSED

- Out-of-range requests clamp to the axis (`0 → min`, `1000 → max`); the outline at a clamped
  request is byte-identical to the outline at the bound.
- A variable face **without** a `wght` axis, a static face, and unreadable bytes all answer without
  panicking: no axis is reported, and every instantiation is the face's default instance.
- `Font::weight()` is already normalised to `100..=900`, so the API clamps only against the face.

Evidence: `a_weight_beyond_the_axis_is_clamped_to_the_axis_bounds`,
`a_variable_face_without_a_weight_axis_uses_its_default_instance`,
`unreadable_bytes_answer_none_instead_of_panicking`,
`a_static_face_is_unchanged_at_every_weight` (advance included).

### 3. Weight-aware outline and raster paths — CLOSED

- `raster::outline_at_weight`, `outline_by_coverage_at_weight`, `outline_glyph_at_weight`;
  the existing `outline`/`outline_by_coverage`/`outline_glyph` delegate at
  `Font::default_weight()`.
- `raster::rasterize_face_at_weight` (the atlas path); `GlyphAtlas` now passes `GlyphKey.weight`.
- The coverage cache key includes the weight, so 400 and 700 cannot share an entry.
- `glyph_source::paint_active_at_weight` is the coverage paint entry the pipeline can adopt (see O-1).
- Every static face takes the pre-existing cached parse unchanged
  (`variable::with_weighted_face` checks for a `wght` axis first).

Evidence: `raster::tests::the_weight_instance_rasterises_the_outline_it_reports` — at 400 and 700 the
rasterised coverage box and the outline box agree within one pixel, the 700 instance is wider and
starts higher, and the two coverage buffers differ;
`raster::tests::the_weighted_outline_matches_the_default_one_for_a_static_face`;
`variable::tests::a_static_face_is_unchanged_at_every_weight`;
`raster::tests::a_static_face_rasterises_identically_at_every_weight`.

### 4. Weight-aware advance (measuring) path — CLOSED at the text layer

- `RustybuzzShaper::{shape_face_at_weight, shape_face_with_ligatures_at_weight}`.
- `shaping::glyph_run_for_family_at_weight`.
- `variable::with_shaper_face_at_weight` — the shaper-side instance cache, clamped by the same rule
  as the outline instantiation, only used for faces that declare a `wght` axis.

Evidence: `variable::tests::the_shaper_measures_a_variable_face_at_the_requested_weight` (700
measures wider than 400, same glyph ids); `the_shaper_instance_and_the_outline_instance_move_the_axis_together`
(shaper outline == outline instance outline, shaper advance == instance advance, both after the same
clamp). The unweighted `shape_face` is explicitly the face's **own** instance, so the always-on
renderer keeps one instance for measurement and drawing (see O-1).

### 5. Feature gating and byte-identical default build — CLOSED

- `variable` is `#[cfg(feature = "text-shaping")]` (that feature is what brings the parser in);
  `text::paint_active_at_weight` and the variable re-exports carry the same gate.
- `cargo check --no-default-features --features mini` compiles with none of it.
- All shipped faces are static, so every weighted entry point delegates to the original cached
  parse: `cargo test --lib` (8805) and all 4 SVG snapshots pass unchanged.

### 6. macOS system UI face: one variable slot, honest fallback — CLOSED (pre-existing, verified)

The registration already exists in this tree (previous phase), in
`src/platform/macos/environment.rs` (Cocoa) and `src/platform/macos_objc2/native.rs` (objc2):
`SYSTEM_UI_FAMILY = "SF Pro"`, one `register_face` call, bytes read once, `None` on every failure
path. `Theme::macos_light`/`macos_dark` only name the family when the call returns `Some`.

Verified on this host:

- `/System/Library/Fonts/SFNS.ttf` parses as a variable face with four axes —
  `wdth 30..150/100`, `opsz 17..96/28`, `GRAD 400..1000/400`, `wght 1..1000/400`.
- `render::text::variable::tests::the_host_system_ui_face_declares_a_weight_axis` asserts the
  `wght` axis, its range, its default (400) and that the 400 and 700 instances draw different
  outlines. This test also exercises the face's `avar` table implicitly (SFNS declares one), which
  is the "honour `avar` implicitly" evidence.
- `theme::manager::tests::the_system_ui_family_maps_every_text_role_and_preserves_weights` asserts
  one family moves all eight text roles, each role's weight is preserved (bold stays 700), and the
  monospace role is untouched.

### 7. `gvar` tuple-capacity capability fix — CLOSED

**Finding:** `ttf-parser` 0.25.1's default feature set caps a glyph's gvar tuples at 32 unless
`gvar-alloc` is enabled. SFNS exceeds that, so `outline_glyph` returned `None` for **26 of 26**
Latin glyphs while the raw `glyf` outline worked — variable fonts would have been "supported" but
drawn nothing. `Cargo.toml` now requests `features = ["gvar-alloc"]`; the same probe then reported
26/26, and the host-face test above passes.

This is a real behaviour change only for faces whose variation needs the heap (the shipped static
faces have no `gvar`), so the default build's bytes are unaffected (snapshots pass).

### 8. Tests and fixtures — CLOSED

New fixtures under `src/render/text/font_assets/` (`variable_test.ttf`, `variable_test_wdth.ttf`,
declared in `variable_fixtures.rs` behind `cfg(test)`, keeping
`tools/check_font_data_is_opt_in.sh` green). They are this crate's own work, built from two
interpolation masters each: a four-point `A` box plus `.notdef`, with `fvar`, `gvar`, `HVAR` and
`STAT`. The weight fixture's 700 master widens the box's right edge by 100 units, raises its top by
100 units and advances 60 units further; the width fixture has a `wdth` axis and **no** `wght`
axis.

Covered: axis enumeration (tag, name, min/default/max), 400-vs-700 outline difference, advance
difference, clamping at both bounds, missing-axis fallback, unreadable bytes, explicit coordinate
sets, cache/fresh agreement, shaper/outline agreement, static-face neutrality, and the host face.

## Resolved after the first report (previously OPEN)

### O-1 — CLOSED: the whole pipeline now carries `Font::weight()`

`pipeline/pixel_ops.rs` (`GlyphDrawConfig.weight` → `paint_active_at_weight`),
`pipeline/primitives.rs` (`glyph_run_for_family_at_weight` + `outline_glyph_at_weight`),
`svg/backend.rs` (`outline_by_coverage_at_weight`) and `text/line.rs`'s `shape_line`
(`cluster_advances` through `with_shaper_face_at_weight`) now request the same instance, so the
advance a label reserves and the ink it paints cannot come from two different masters.

Evidence: `render::text::raster::tests::the_weight_instance_rasterises_the_outline_it_reports`,
`render::text::variable::tests::the_instance_advance_moves_with_the_weight`, and the SVG snapshot
bytes (all faces this crate ships are static, so the default pictures are unchanged).

### O-2 — CLOSED as already provided

The byte supply lives in `platform/macos/environment.rs::register_system_ui_face` and its
`macos_objc2` sibling, each with a `'static` one-slot cache and an honest `None`. A new
`platform/macos/fonts.rs` would have been a second implementation of one rule, which the project's
own single-derivation rule forbids. The stale "default instance" sentences in those two modules
were updated to describe the weighted path.

### O-3 — CLOSED: synthetic `avar` assertion added

`font_assets/variable_test_avar.ttf` is the weight fixture plus an `avar` segment map
(`0.2 → 0.6`, with `0.6` held fixed). The test
`render::text::variable::tests::avar_remaps_the_weight_coordinate_before_the_deltas_are_applied`
asserts that user 500 and user 700 reach one outline (within one font unit, because the map is
stored as F2Dot14), that the advance agrees, and that the linear face does not.

### O-4 — CLOSED: registry isolation in the font dialog tests

`font_dialog`'s tests now take `runtime_fonts_test_guard()` and clear the registry on drop
(`FaceRegistryGuard`), so a selection no longer shadows the compiled Latin face for concurrent
tests. `cargo test --lib --features runtime-fonts` is 0 failed.

## OPEN

None.

## Capability schema rows

**None required.** The weight is already the serialized `Font.weight` field (the theme JSON role
font's `weight`, defaulting to 400), and this change is a library-level text-rendering capability
with no new widget property, command, event, or constructor. Nothing in
`src/widget/capability/**` needs a row; editing those files was out of scope in any case.

If a host later wants to browse axes through the capability layer, the minimal shape would be a
read-only `font_axes` property on the font-preview control (a list of
`{tag, name, min, default, max}`), backed by `VariableFace::axes`. That is a separate proposal, not
a requirement of this item.

## Files

- `Cargo.toml` — `ttf-parser` now enables `gvar-alloc` (with the measured rationale).
- `src/render/text/variable.rs` (new) — axes, instantiation, caches, tests.
- `src/render/text/raster.rs` — `_at_weight` outline entries, weighted coverage key, atlas raster
  entry, tests.
- `src/render/text/shaping.rs` — shaper weighted forms, `glyph_run_for_family_at_weight`.
- `src/render/text/glyph_source.rs` — `paint_active_at_weight`.
- `src/render/text/atlas.rs` — passes `GlyphKey.weight` to the rasteriser.
- `src/render/text/mod.rs` — module + re-exports.
- `src/render/text/font_assets/mod.rs`, `variable_fixtures.rs` (new), two `.ttf` fixtures (new).
- `src/theme/manager.rs` — corrected the stale "no instancing" doc; role-mapping test.
- `docs/issues/slices/final-c29-report.md` (this file).