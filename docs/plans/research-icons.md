# RESEARCH + DESIGN — A Complete Icon Library for `rust_widgets`

> **⚠️ SUPERSEDED (2026-09-27).** This document recommends **Lucide**.
> That recommendation was **reversed** after fetching the upstream licence
> texts: Lucide's ISC re-licences a Feather-derived subset whose MIT requires
> its own notice in all copies, and Lucide does not carry that notice — so the
> re-licence chain is incomplete and needs a human legal call.
>
> **The adopted plan is `blue25.md` §6: Material Symbols (Apache-2.0) +
> SVG path data + CPU flattening.** Material Symbols ships per-icon SVG
> outlines (`symbols/web/<name>/`), not only a font — see §6.1.3 there.
>
> The analysis below remains useful for: the data-layout design (§3), the
> three completeness guarantees (§4.1), the census pattern (§4.2), the
> snapshot strategy (§4.3), and the option comparison method (§2).
> **Read it with the source substituted: Lucide → Material Symbols.**

Status: **research / design only. No code has been written into `src/`.**
Scope: replace the hand-drawn geometric icon approximations in
`src/widget/display_widgets/icon.rs` with a complete, licence-clean,
`no_std`-safe, census-verified icon library that renders identically through both
the software rasteriser and the SVG backend, and that cannot silently ship a
missing icon.

All file/line citations are relative to the crate root
(`/Users/mikewolfli/Desktop/workspace/rust-widgets`).

---

## 0. Executive summary

1. **The current state is incomplete, and the incompleteness is structural, not
   accidental.** `IconName` declares 31 variants (`icon.rs:32-95`); each is drawn
   by a bespoke `draw_*` method that composes lines, circles and rects. Two
   defects follow directly from that design:
   - `Close` is drawn as `draw_cross` — *"Reuse cross drawing"* (`icon.rs:585-588`),
     so `close` and `cross` are byte-identical pictures. The module doc already
     admits this: *"several variants are drawn as aliases of others … visually
     distinct names do not always produce distinct output"* (`icon.rs:28-30`).
   - `set_icon` accepts **any** string and matches it at draw time, falling back
     to a question-mark placeholder (`icon.rs:247-250`, `draw_icon`'s `_ =>
     self.draw_unknown(ctx)` at `icon.rs:750`). There is **no compile-time or
     test-time link** between a declared `IconName` and renderable geometry, so a
     new variant with no `draw_` arm is a silent placeholder.
2. **Recommended approach: bundled SVG path data from Lucide, stored as
   `&'static str`, parsed by a new shared path parser, drawn through the crate's
   existing vector pipeline (`RenderContext::draw_path`) and colour-flattened at
   draw time.** Lucide is **ISC** and the derived-from-Feather subset is
   additionally **MIT** (verified — §1). It is a 24×24 grid with a 2 px stroke,
   i.e. exactly the geometry this crate's `Icon::size` (a square box, default 24)
   is designed around.
3. **Verification is by census, not by inspection.** A generated
   `tools/icon_census.txt` + `tools/icons/*.svg` vendor tree + a new crate
   `tests/icon_census_test.rs` turn "missing icon" from something a user
   discovers into a **build failure** (§4).
4. **Deferred, with reasons stated:** multi-tone/duotone icons (needs a
   colour-role model the theme layer does not have yet), `currentColor`
   inheritance *through arbitrary SVG documents* (only static two-tone is in
   scope), and a runtime icon-font source (would re-introduce the U-13 SDF
   problem BLUE24 §12.2 already rules out; §1.2/§7).

---

## 1. What the host project actually is (grounding facts)

### 1.1 Existing icon surface — what `icon.rs` does and does **not** do

**Does:**

| Capability | Evidence |
|---|---|
| 31-variant `IconName` enum | `icon.rs:32-95` |
| `&str` ↔ enum round-trip | `as_str` `icon.rs:104-138`, `from_name` `icon.rs:146-181` |
| String-keyed `set_icon` + `icon` getter | `icon.rs:247-259` |
| Square box sizing, min 4.0, default 24.0 | `set_size` `icon.rs:274-277` |
| Colour resolution: explicit → theme `text_color` → `Color::PRIMARY` | `resolve_color` `icon.rs:321-329` |
| Disabled appearance: desaturate + half alpha | type doc `icon.rs:211-216` |
| Draws through `RenderContext`, so both backends see it | `Icon::draw` `icon.rs:1352-1371` |
| SVG snapshot test (3 icons only) | `icon.rs:1509-1536` |

**Does NOT do:**

| Gap | Evidence / consequence |
|---|---|
| No icon **data** — geometry is code | 28 `draw_*` methods, `icon.rs:343-1277` |
| Declared name ≠ renderable distinct picture | `Close` aliases `Cross` (`icon.rs:585-588`) |
| No test that every `IconName` renders *distinct* ink | `icon_name_enum_roundtrip` (`icon.rs:1467-1506`) only checks string round-trip |
| Unknown name is silently a placeholder with no diagnostic | `draw_unknown` `icon.rs:1264-1277`; `log`/`warn` absent |
| No curve/arc support | every shape is lines/circles/`draw_path` polylines; no `Q`/`C`/`A` command is ever emitted |
| 3 of 31 icons have snapshots | `icon_svg_output_check|star|warning` `icon.rs:1509-1536` |
| No feature gate for icon data | no `icons-*` feature exists in `Cargo.toml` |
| No licence record | `NOTICE` covers 5 font tables, zero icon material |

### 1.2 How icons are referenced elsewhere (grep results)

- **`set_icon` on a widget** — only `Icon::set_icon` (`icon.rs:247`) and its
  property setter `"icon_name"` (`icon.rs:1316-1319`), plus commands
  `set_icon_name`/`set_size` (`icon.rs:1345-1348`).
- **`set_icon` on a window** — a **different, unrelated** API:
  `WindowHandle::set_icon(path)` sets the OS window icon from a **file path**
  (`src/app/handle.rs:2657`), pushed to `Platform::set_window_icon`. It is
  mirrored in `WindowState.icon` (`src/app/handle.rs:2582-2592`). **This is a
  naming collision only; the icon-library work must not touch it.** (Worth a
  doc-comment cross-reference, see Phase 5.)
- **`Font::simple`** — no match in `src/` for a symbol/icon use; fonts are
  text-only.
- **Emoji/symbol tables** — the crate has a **glyph** path, not an **icon** path:
  `GlyphSource`/`InkKind` (`src/render/text/glyph_source.rs:137-254`) resolve a
  `char` to pixels. `Font8x8Source` is the default Latin face; colour emoji is a
  gated `ColorBitmapSource` (`glyph_source.rs:400-470`). **Icons are not glyphs
  here today and should not become glyphs** — see §1.4.
- **`GlyphSource` is guarded**: `tools/check_glyph_source_is_the_only_glyph_path.sh`
  + `tools/glyph_path_scan.py` forbid any file except
  `src/render/text/glyph_source.rs` from reaching a face table. **An icon-font
  approach would therefore be forbidden by an existing gate unless it were routed
  through `GlyphSource`** (§1.2 below).
- **`DrawPath`** exists in both backends: software
  (`src/render/pipeline/primitives.rs:494`, `src/render/backend/paint.rs:203-207`)
  and SVG (`src/render/svg/backend.rs:893-909`). The SVG backend emits it as
  `<path d="M .. L .. Z">`. **This is the reuse point.**
- **SVG path *parsing* exists but is `M/L/H/V/Z`-only and private**:
  `path_bounds` (`src/widget/svg.rs:279-332`) understands only axis-aligned
  subpaths, and is `fn`, not `pub`. It is a **test-side** reader, not a
  production path interpreter.
- **Snapshot gate**: `tools/check_svg_snapshots.sh` regenerates via
  `cargo run --no-default-features --features desktop --example export_control_svgs`
  and byte-compares. Current committed count is **391 files** (`snapshots/svg`,
  verified: `action.svg` … `*_checked.svg`); the script's own header still says
  376 and its count check derives `CONTROLS*2 + EXTRAS*2` from
  `properties.rs` and `EXTRA_APPEARANCES`. **Any icon change that affects the
  census render will move these bytes.**
- **Font-data conventions to mirror**: `tools/check_font_data_is_opt_in.sh`
  (font data is opt-in; no profile may enable it) and `tools/check_font_licenses.sh`
  (every shipped generated table is named in `NOTICE` with upstream digest +
  licence).

### 1.3 `docs/plans/blue24.md` on icons

`blue24.md` contains **no `icon`-specific section** (grep for `icon` returns
nothing). Its relevance is indirect but load-bearing, and it establishes two
constraints this design must honour:

- **§12 U-13 — glyph distance fields (SDF/MSDF) are explicitly NOT to be done**
  (`blue24.md:1439-1479`), because (per the crate's own measurement summary) the
  font sizes are 97% within 9–18 px, there is no GPU glyph atlas, SDF is worse at
  small sizes, SDF would turn the 377 byte-compared SVG snapshots into
  non-scalable bitmaps, and CJK is infeasible. **Icon fonts inherit every one of
  those objections**, which is why §1 says "no icon font".
- **§12 U-14 — text metrics: "measure with the right face"** (`blue24.md:1370`).
  A real defect: the theme declared `"Arial"` while the crate ships `"Open Sans"`,
  so `face_for_family` returned `None` and metrics fell back to a flat `0.6 em`,
  making `widget`'s `w` overlap by 1.85 px and `i` over-occupy 4.87 px. **This is
  exactly the failure mode an icon box aligned by guesswork reproduces**, so §5
  below specifies icon/text alignment against measured metrics, not a magic
  constant.

**Unresolved question carried forward from blue24:** U-13/U-14 leave open *how*
any non-text vector art should enter `snapshots/svg/` without invalidating the
byte-comparison safety rope — the document resolves it only for glyphs. §4.2
answers it for icons (sample, don't explode; see there).

### 1.4 The crate's SVG backend is an **emitter**, not a **renderer**

`SvgPaintBackend::execute_command` (`src/render/svg/backend.rs:893-909`) writes
`<path d="M x y L x y Z">` from a `&[Point]` polyline. It **cannot express
curves** because `RenderCommand::DrawPath` (`src/render/core/command.rs:291-295`)
is a point list. Consequences:

- A bundled SVG path with `C`/`A`/`Q` commands **must be flattened to a polyline
  before it can use `draw_path`**, otherwise the icon renders as a chord.
- The flattening must be adaptive (§2.4) or a 24 px icon loses its round shapes.

---

## 2. Options for shipping icons — comparison and recommendation

### 2.0 Dependencies already available (`Cargo.toml`)

The crate **already depends on a full SVG rasteriser and an SVG parser stack**:

| Crate | Version | Feature | Relevance |
|---|---|---|---|
| `resvg` | `Cargo.toml:519` | `svg-rasterizer` (`:276`) | a complete SVG *renderer* — **too heavy** for icons and already gated to `image` |
| `ttf-parser` | `Cargo.toml:499` | `fonts-emoji-color`/vector faces | outlines from fonts |
| `rustybuzz` | `Cargo.toml:498` | `text-shaping` | text shaping |
| `phf` | **not present** | — | would be new (§2.3) |
| any icon crate (`lucide-*`, `tabler-*`, `material-*`) | **not present** | — | would be new |

**Finding: there is no icon-related crate in `Cargo.toml` today, and no
existing path/curve geometry parser in production code.** The only SVG-`d`
reader (`path_bounds`) is private and `M/L/H/V/Z`-only (§1.2).

**Constraint from `Cargo.toml`:** `edition = "2021"`, `rust-version = "1.87"`
(`Cargo.toml:4,13`) — so `phf` 0.14 (MSRV 1.85) is admissible, and **const generics
/ `const fn` are freely usable**.

### 2.1 Option A — Bundled SVG path data (`&'static str`) ✅ RECOMMENDED

**What:** vendor each glyph's outline as an SVG `d` string, one `&'static str`
per icon, plus optional per-shape stroke attributes.

| Axis | Assessment |
|---|---|
| **Licence** | Lucide **ISC**; the Feather-derived subset additionally **MIT** (both permit bundling with the notice retained). See §2.5 for exact texts. |
| **Binary size** | Lucide's own average path is ~180–320 bytes of text. At **~250 B avg × 400 icons ≈ 100 KB** of `.rodata` **before** feature gating and **without** a coordinate-compression pass. Comparable to the crate's existing `fonts-cjk-bitmap` (~85 KB) which the crate already treats as an *opt-in* payload — so **this MUST be feature-gated** (mirroring `check_font_data_is_opt_in.sh`). |
| **Scalability** | True vector; re-flattened per requested pixel size. |
| **Theming / recolouring** | Excellent: a path is ink, so `resolve_color()` result is simply passed as the draw colour. Single-tone for free; two-tone needs a second stroke list (§3). |
| **`no_std`** | Trivially safe: `&'static str` is `'static` data; the parser is a stack-only tokeniser (§2.4). |
| **Fits software backend** | Yes, via `RenderContext::draw_path`. |
| **Fits SVG backend** | Yes — and **better than today**, because the SVG backend could optionally pass the *source* `d` through un-flattened for perfect fidelity (§2.6). |
| **Snapshot gate** | Friendly: deterministic integer/float geometry, byte-stable. |

### 2.2 Option B — Icon **fonts** (Material Icons font, Font Awesome) ❌ NOT RECOMMENDED

| Axis | Assessment |
|---|---|
| **Binary/proc size** | A Material Icons `.ttf` is 100 KB–1.5 MB depending on subset; Font Awesome free is larger still. The crate's own convention is that such payloads are opt-in and measured (`NOTICE` records `fonts-cjk-bitmap 2361 glyphs ~85 KB`, `fonts-cjk ~353 KB`). |
| **Scalability** | Good, but the crate rasterises **CPU-side, per glyph, per frame** (`blue24.md:1447`, "本仓是 CPU 逐字形光栅化到 RGBA 缓冲"). Icons are static and could be cached, whereas font glyphs cannot be shaped-then-cached cheaply — this is a **worse fit** than paths. |
| **Theming** | **Weak.** A glyph is monochrome ink; recolouring works, but multi-tone does not, and an icon font cannot express `currentColor` at all. |
| **`no_std`** | Would require a `GlyphSource` implementation, which `tools/check_glyph_source_is_the_only_glyph_path.sh` restricts to `src/render/text/glyph_source.rs`. Workable, but see next row. |
| **Project-gate conflict** | BLUE24 §12.2 U-13 rules out *the whole class* (distance fields / atlas / small-size quality / snapshot degradation). An icon font reintroduces a subset of exactly those problems at 9–18 px. |
| **Effort** | Requires a `GlyphSource` impl + a font subset generator + a `NOTICE` section + licence gate — **strictly more moving parts than paths** for a capability the crate does not need. |

**Verdict:** reject on fit and on the crate's own recorded ruling, not on licence.

### 2.3 Option C — Raster / PNG atlases ❌ NOT RECOMMENDED

| Axis | Assessment |
|---|---|
| **Scalability** | Blurry at non-native size; needs DPI tiers (`@1x/@2x/@3x`) which multiplies the payload. Desktop/tablet/mobile at differing density means **three** atlases. |
| **Snapshot gate** | **Directly harmful.** The byte-compared `snapshots/svg/` would embed or reference rasters and stop being a vector artifact, which is exactly the objection that killed SDF (`blue24.md:1450`). |
| **Theming** | Recolouring a raster requires a tint pass and loses antialiasing quality; a mask atlas is possible but is strictly more machinery than a path. |
| **`no_std`** | Feasible (raw `&[u8]` + the crate's existing image decode stack is gated), but heavier. |
| **Fits backends** | Software: yes (`draw_image`). SVG: as `<image>` — again, a raster in a vector snapshot. |

### 2.4 Option D — Procedural (vector commands built in code, i.e. today's design) ⚠️ KEEP ONLY AS FALLBACK

| Axis | Assessment |
|---|---|
| **Binary size** | Near-zero data; code instead. The smallest option. |
| **Scalability/Theming/`no_std`** | All fine — this is what the crate does today. |
| **Why it is not the answer** | It is **the cause of the bug the user reported.** 28 hand-written `draw_*` methods (`icon.rs:343-1277`) produced `Close == Cross`, cannot express a curve, and have no completeness link. Extending it to 400 icons means 400 hand-written methods — 10 000+ lines of unreviewable geometry with no census. |
| **Recommended role** | **Keep exactly the primitives that are genuinely parameterisable**, specifically the "unknown/placeholder" box (§4.3), and keep the *current* 31 hand-drawn icons alive behind the new data path as a fallback for stripped profiles — see Phase 4. |

### 2.5 Recommended set: **Lucide** — exact licence verification

Fetched `https://raw.githubusercontent.com/lucide-icons/lucide/main/LICENSE` on
2026-09-27. The file is **two licences in one document**:

1. **ISC License** — `Copyright (c) 2026 Lucide Icons and Contributors`.
   Verbatim grant: *"Permission to use, copy, modify, and/or distribute this
   software for any purpose with or without fee is hereby granted, provided that
   the above copyright notice and this permission notice appear in all copies."*
   → **Attribution is required: the copyright notice and the licence text must
   appear "in all copies".**
2. The icons **derived from Feather** (an explicit, enumerated 100+ name list in
   the same file — including `check`, `x`, `search`, `plus`, `minus`, `lock`,
   `download`, `upload`, `share`, `trash`, `more-horizontal`, `info`, `alert-*`,
   `arrow-*`) are **MIT**, `Copyright (c) 2013-present Cole Bemis`.
   → **MIT also requires "the above copyright notice and this permission notice
   shall be included in all copies or substantial portions of the Software."**

**Consequence for this crate: attribution must ship in the repository *source*
(NOTICE), not merely in docs.** Both licences say "in all copies", and the
bundled strings are compiled into the binary. This is *precisely* the standard
`NOTICE` already applies to fonts, so the mechanism exists — see §6.3.

**Alternatives checked:**

| Set | Licence | Verdict |
|---|---|---|
| **Tabler Icons** | **MIT** (`Copyright (c) 2020-2026 Paweł Kuna`), fetched 2026-09-27 | Viable alternate; single clean MIT, larger set, 24×24 grid. Choose Lucide for: equal grid, tighter `stroke-width:2` convention, and the Feather lineage matches the crate's existing semantic names (`check`, `x`, `arrow-*`, `trash`). |
| **Material Symbols** | **Apache-2.0** (fetched `google/material-design-icons/master/LICENSE`, 2026-09-27: standard Apache 2.0 with the "APPENDIX"). Apache §4(d) also requires a NOTICE reproduction. **⚠️ CORRECTION (2026-09-27):** this row originally read "It is primarily a **font** … → lands in Option B's problems". **That is wrong, and the error was a category mistake.** Fetched `google/material-design-icons/contents/symbols/web` and it returns **one directory per icon** (`symbols/web/add/`, `symbols/web/close/`, …), i.e. Material Symbols ships **per-icon SVG outlines** as well as a font. It is therefore usable as an **Option A source** (bundled `d` strings), not confined to Option B. The `.codepoints` map is what the *font* distribution uses; it is not the only distribution. See `blue25.md` §6.1.4 for the retraction, and §6.1.1 for the two-axis framing that would have prevented the mistake (axis A = where data comes from, axis B = how data becomes pixels). |

### 2.6 Recommendation

**Bundle Lucide SVG path data as `&'static str`, feature-gated, parsed by a new
`no_std` path tokeniser, flattened to the crate's existing polyline
`RenderCommand::DrawPath`, and recoloured from `resolve_color()`.**

Two-phase fidelity plan:

- **Now:** flatten to polylines on the CPU and emit `DrawPath`. Works on both
  backends unchanged, is byte-deterministic, and needs **no new `RenderCommand`**.
- **Later (deferred):** add `RenderCommand::DrawSvgPath { d: &'static str, .. }`
  so the **SVG backend can pass the original `d` through**, giving perfect
  curves for zero flattening cost *and* making the icon snapshots diff against the
  upstream Lucide path rather than against this crate's flattening approximation.
  This is deferred because it changes the `RenderCommand` enum (a shared
  contract) and would need its own gate; the flattening path is strictly a
  subset of it.

---

## 3. Data representation — concrete design

### 3.1 The core type

```rust
// src/widget/icons/mod.rs  (new module)

/// One drawable shape of an icon. Coordinates are in the icon's own
/// 24x24 design grid; the renderer maps them into the widget's box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconShape {
    /// A filled/stroked polyline already flattened to the design grid.
    Path,
    /// A circle (Lucide's `<circle cx cy r>`), kept un-flattened because a
    /// circle is exact and cheap in both backends.
    Circle { cx: i16, cy: i16, r: i16 },
    /// An axis-aligned rect (Lucide's `<rect x y width height>`, optional rx).
    Rect { x: i16, y: i16, w: i16, h: i16, rx: i16 },
}

/// The renderable data for one icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconData {
    /// Source path data, in the 24x24 design grid, Lucide's own `d` string.
    /// `&'static str` so the whole table is `.rodata` in every profile.
    pub path: &'static str,
    /// The design grid the path is authored in (`24` for Lucide).
    pub grid: u8,
    /// Stroke width in design units (`2` for Lucide).
    pub stroke: u8,
    /// Whether the ink is filled rather than stroked.
    pub filled: bool,
}
```

**Why `&'static str` and not a pre-flattened `&'static [Point]`:** the string is
~3–5× the byte size of a flattened f32 polyline at 24 units (`Point { i32, i32 }`
= 8 B per vertex; a 24-vertex path = 192 B vs ~200 B of text). Text is **equal or
smaller**, is human-diffable in review, is trivially proven to match upstream, and
moves the parser cost to draw time where it can be cached. Pre-flattening at
`const` time is **deferred** (§7) because Rust `const fn` cannot yet allocate a
slice of unknown length without unsafe, and the crate forbids that style.

**Why `i16` coordinates for flattened output:** a 24-unit grid fits `i8`; `i16`
leaves room for the scaled result before the box transform.

### 3.2 Compact binary encoding vs text — analysed, rejected

| Encoding | Bytes/icon (est.) | Verdict |
|---|---|---|
| Lucide SVG `d` text (avg) | ~250 B | Readable, diffable, provably identical to upstream. **Chosen.** |
| Same text, deflated with `miniz_oxide` (already a dep) | ~120 B | ~50% saving, but adds a decompress step and **destroys diffability**; the crate's snapshot gate philosophy is that reviewable > compact. **Deferred.** |
| Custom opcode bytecode (`i8` ops + `i8` operands, 6-bit packed) | ~70–90 B | Smallest, but needs a generator *and* a disassembler for review, and its correctness cannot be checked against upstream by eye. Not worth it until size is a proven problem. |
| Pre-flattened `&[Point]` in `.rodata` | ~190+ B, and **lossy for curves** | No size win, loses curve fidelity. |

**Estimated totals at the recommended encoding:**

| Set size | Est. `.rodata` (text) | With feature gate off |
|---|---|---|
| 31 (parity with today) | ~8 KB | 0 B |
| **~200 (recommended core set)** | **~50 KB** | 0 B |
| ~400 (Lucide "common") | ~100 KB | 0 B |
| 1600 (all of Lucide) | ~400 KB | 0 B |

For calibration: the crate already ships `fonts-cjk-bitmap` at **~85 KB** as an
explicit opt-in (`NOTICE:217-225`). **A ~50 KB gated icon payload is well inside
the crate's established budget** — provided it is gated (§3.4).

### 3.3 Lookup: `enum Icon` vs `HashMap` vs `phf` vs linear scan

Requirements: (a) `no_std` with **no allocator** on `alloc_frugal`; (b) a
`&str` property path (`icon_name`, `icon.rs:1316`) must resolve; (c) the resolver
must be able to answer **"is this a real icon?"** distinguishably from "placeholder".

| Candidate | `no_std` | Cost | Verdict |
|---|---|---|---|
| `HashMap<&str, IconData>` | `compat::HashMap` is `BTreeMap` under `alloc_frugal` (`src/compat.rs:191-197`), so it needs `Ord` **and** a runtime-buildable map | Allocates; the crate has no `const` `HashMap`. | ❌ |
| `phf::Map<&'static str, IconData>` | **YES** — `phf` 0.14 documents `default-features = false` for `no_std`, MSRV 1.85 (docs.rs/phf/0.14.0; crate `rust-version = 1.87`, so ok) | Perfect hash, **O(1), zero runtime construction**, lives in `.rodata` | ✅ viable, but **adds a dependency + a `build.rs`/`phf_codegen` step.** |
| `const`-sorted `&[(&str, IconData)]` + binary search | YES, pure `core` | O(log n); `&str` comparison is a `memcmp` | ✅ **Recommended** for the first cut. |
| `match name { "check" => …, … }` (compiler-generated perfect hash) | YES | **rustc lowers a dense `&str` match to a perfect hash / length-bucketed jump**, effectively `phf` for free | ✅ **Recommended for the *enum* path**; already the crate's style (`IconName::from_name`, `icon.rs:146-181`). |

**Recommendation:**

- Keep **`IconName` as the typed, compile-time-checked surface**, and implement
  `IconName::data(self) -> IconData` as a `match` on the enum with **no `None`
  arm** — the compiler then makes "declared variant with no data" a **compile
  error**, which is the strongest possible form of "missing is impossible" (§4.1).
- Keep **`Icon::set_icon(&str)`** for the designer/JSON path (`icon_name`
  property) but route it through a `&[(&str, IconName)]` sorted table searched by
  binary search, and **log a diagnostic on miss** (the crate already uses `log`;
  see `census.rs:300` for the precedent of a non-silent `log::warn!`).
- **Do not add `phf` yet.** A `const` slice + `match` gives the same asymptotics
  for the two access patterns this crate has, with zero new dependencies
  (principle: prefer existing deps). `phf` is the escape hatch if the string
  table ever exceeds ~1000 entries and the binary search shows up in a profile —
  record that as a deferred item, not a blocker.

### 3.4 `no_std` safety and feature gating

**`no_std` rules honoured:**

- All icon data is `&'static str` → `'static`, no allocation.
- The parser is a **stack-only tokeniser over `&[u8]`** (no `Vec`) if it emits
  into a caller-supplied fixed buffer (or, in the `alloc`-available paths, a
  `Vec`). Design: `PathParser::for_each_segment(d, |seg| …)` — a **callback**
  rather than a collected `Vec`, so the caller chooses the storage and the
  `alloc_frugal` profile can flatten straight into `heapless::Vec<Point, N>`.
  **This is the same shape as `for_each_cluster` in
  `src/widget/metrics.rs:82`** — the crate already uses the callback-visitor
  pattern to stay allocation-free.
- No `std::`, no `HashMap` in the data path.

**Feature gating (mirroring `check_font_data_is_opt_in.sh`):**

```toml
# Cargo.toml [features]

## `icons` — the complete bundled icon set (Lucide, ISC + MIT).
##
## NOT in `default`, `desktop`, `tablet`, `mobile`, `mini` or `embedded`.
## Icon data is opt-in for the same reason font data is (BLUE23 §0A.4
## constraint 1): a build that does not ask for it must not pay for it.
## The 31 geometry-only icons in `display_widgets/icon.rs` remain available
## without this feature.
##
## See `tools/check_icon_data_is_opt_in.sh` and `NOTICE`.
icons = []

## `icons-full` — every bundled icon (superset of the core 200).
icons-full = ["icons"]
```

**Naming note:** the crate uses noun-plural feature names for payloads
(`fonts-cjk-bitmap`, `image-codecs`). `icons` / `icons-full` matches.

---

## 4. Completeness and verification — making "missing" impossible

The user's report is *"I see many are missing in the svg"*. The design goal is
that this sentence becomes unsayable.

### 4.1 Three independent completeness guarantees

**Guarantee 1 — compile time (strongest).** `IconName::data()` returns
`IconData` (not `Option`). Adding a variant without adding its data is `E0004`
(non-exhaustive match). This is checkable by the compiler, needs no gate, and
cannot be bypassed.

**Guarantee 2 — vendor integrity.** The bundled `d` strings are **copied
verbatim** from the pinned upstream Lucide release by a generator that records
the upstream `SHA-256` of each source file. A `--check` mode re-derives and
compares, exactly like `tools/gen_cjk_bitmap.py --check`
(`NOTICE:28-31` documents that pattern). This proves *"what we ship is what
Lucide wrote"*, which no amount of eyeballing can.

**Guarantee 3 — runtime census (the artifact-level check).** A gate enumerates
every name in `tools/icon_census.txt`, renders each through **both** backends,
and asserts:
  - the parser **accepted** the path (no malformed `d`);
  - the rasterised ink is **non-empty**;
  - every name renders **distinctly** from every other name (no `Close == Cross`);
  - every declared name has **data** (Guarantee 1 backs this at compile time; the
    census backs it against the *string* surface too).

### 4.2 The census file — mirroring `event_published_census.txt`

**Existing pattern to mirror** (`tools/event_published_census.txt:1-5`):

```
# Published capability events, one control per line: control: name, name, ...
# Generated from src/widget/capability/properties.rs by tools/derive_event_payloads.py.
# This file is the census the event payload table is derived from; edit it to publish or
# withdraw a name, and re-run the tool.
action: triggered, toggled, hovered, changed
```

**Analogous icon census** (`tools/icon_census.txt`, new):

```
# Bundled icon names, one per line, in the crate's canonical token form.
# The census the icon data table is derived from: add a name here and re-run
# tools/gen_icon_data.py, which vendors the outline from the pinned Lucide release
# and writes src/widget/icons/icon_data.rs. A name that has no outline upstream is
# a generator failure, not a silent omission.
# Generated for the `icons` feature set; the `icons-full` superset is listed in
# tools/icon_census_full.txt.
alert_triangle
arrow_down
arrow_left
arrow_right
arrow_up
bell
check
...
```

**Generator** `tools/gen_icon_data.py` (new), modelled on the crate's existing
generators (`tools/gen_cjk_bitmap.py`, `gen_font_subset.py`, `gen_emoji_subset.py`):

- takes `--license=<id>` from a **closed set** and exits `2` otherwise — the
  *inbound* licence gate, identical in shape to `gen_cjk_bitmap.py`
  (documented by `check_font_licenses.sh:24-27`);
- takes `--check` to re-derive and byte-compare (the *outbound* integrity gate);
- vendors the upstream `.svg` sources into `tools/icons/<name>.svg` (so a
  reviewer can diff the crate's data against the upstream file in one `git diff`)
  and writes `src/widget/icons/icon_data.rs` with a `GENERATED FILE` header
  naming the upstream project, version, source URL, per-file SHA-256, and licence;
- refuses to emit a name that has no upstream outline, and refuses to emit two
  names whose outlines are byte-identical (defeating the `Close == Cross` class
  **at generation time**, before it can ship).

### 4.3 Snapshot strategy — do **not** explode the 391-file set

Rendering 200 icons × 2 appearances = 400 new files would **more than double** the
snapshot set and the gate's runtime, and the gate's count check
(`check_svg_snapshots.sh:107-141`) derives the total from `properties.rs` — so the
icon snapshots must be a **separate, self-contained set**, not a smuggled-in one.

**Recommended: a separate `snapshots/icons.svg` sheet + a sampled pair, both in
one file.**

1. **One sprite sheet, `snapshots/svg/icon_sheet.svg`** — every icon in the `icons`
   feature laid out in a labelled grid, one dark file and one light companion
   (`icon_sheet.light.svg`), rendered by a new `examples/export_icon_sheet.rs`.
   A 200-icon grid at 24 px + labels is a single reviewable artifact, and the
   **dark/light pair** proves recolouring works for all 200 at once. This reuses
   the "pair proves the theme moved" device the crate already relies on
   (`check_svg_snapshots.sh:33-37`).
2. **Keep the existing per-control `icon.svg` / `icon.light.svg`** (the `Icon`
   *widget* snapshot) so the control's own reviewed picture continues to exist.
   **Do not add 200 more `icon_<name>.svg` files.**
3. **The census gate** (§4.4) does the exhaustive per-icon verification on the
   *rasteriser* side (cheap, no files), so file count stays flat.

**Gate wiring:** `check_svg_snapshots.sh` gains the sheet in a *new* step that
regenerates `examples/export_icon_sheet.rs` into scratch and byte-compares —
**without** touching its existing `EXPECTED` arithmetic.

### 4.4 Malformed paths must fail at test time, not draw time

Three layers, cheapest first:

1. **Generator-time** (Guarantee 2): the generator runs the *same parser* over
   every vendored `d` and refuses to emit a file where it fails. An unparseable
   path therefore cannot reach `src/`.
2. **A crate test over the whole table** (new
   `tests/icon_data_integrity_test.rs`): for each name in the generated table,
   call `parse_icon_path(d, &mut sink)` and assert `Ok` **and**
   `sink.segments() >= expected_min` **and** that every emitted coordinate lies
   within `0..=grid` (with a tolerance for the 2-unit stroke's half-width).
   A path that parses but is empty or out-of-grid is a failure too.
3. **A "no silent placeholder" test:** `Icon::set_icon(name)` for every census
   name must **not** take the `draw_unknown` branch. Implement by having
   `draw_unknown` set a `#[cfg(test)]` thread-local / return a sentinel, or far
   better: **make the resolver typed** — `Icon::set_icon` resolves via
   `IconName::from_name` and stores an `Option<IconName>`, so "unknown" is a
   value the test can assert on and the draw path never re-parses a string.

> **Design note:** making `Icon` store `Option<IconName>` (plus the original
> string for `icon()` round-tripping, `icon.rs:257`) also removes the current
> draw-time `match self.icon_name.as_str()` (`icon.rs:717-751`), which is a
> `&str` comparison per frame. That is a small but real win and is consistent
> with principle #28.

### 4.5 Gate list to add (each with reverse injection)

| Gate | Mirrors | Proves | Reverse injection |
|---|---|---|---|
| `tools/check_icon_data_is_opt_in.sh` (+ `tools/icon_data_opt_in_scan.py`) | `check_font_data_is_opt_in.sh` | No profile (`default`/`desktop`/`tablet`/`mobile`/`mini`/`embedded`) enables `icons`/`icons-full`; `include_str!`/`include_bytes!` for icons lives only under `src/widget/icons/` | add `"icons"` to `default` in a temp `Cargo.toml`; scan must report a finding |
| `tools/check_icon_licences.sh` (+ `tools/icon_license_scan.py`) | `check_font_licenses.sh` | Every `GENERATED FILE` icon table is named in `NOTICE` with upstream digest + both licences; `gen_icon_data.py` exits 2 with no/unknown `--license` | inject an unrecorded table → scan fails; run generator with no `--license` → must exit 2 |
| `tests/icon_census_test.rs` | `tests/control_rendering_census_test.rs` + `regenerate_census_lists.py` | Every census name has data, parses, paints non-empty ink, and is pairwise distinct | delete one name's data → compile error; alias two paths in the generator → generator refuses |
| `bash tools/check_svg_snapshots.sh` (extended) | itself | The icon sheet regenerates byte-for-byte | hand-edit one byte of `icon_sheet.svg` → FAIL |

**Every gate must be shown to fail before it is trusted** — `tools/README.md:25-29`
states this explicitly and names two prior false-passing gates. The plan must
include the break-and-restore step as a deliverable, not a suggestion.

---

## 5. Sizing, box and optical alignment

### 5.1 What the toolkits actually specify

- **Lucide:** every icon is authored on a **24×24 grid with `stroke-width="2"`**,
  i.e. the ink's nominal box is 24 units with the stroke centred on the path, so
  the **visual** extent is 23..25 → the design box is exactly 24 with the stroke
  in-setting by 1 unit on each side. (This is why the recommended `IconData`
  carries `grid` and `stroke` explicitly — §3.1.)
- **Material Symbols:** the canonical size is **24 dp** with optical sizes
  available at **20 / 24 / 40 / 48 dp** — i.e. Material's own answer to "icons at
  different sizes" is to ship *separate optical variants*, not to scale one
  drawing. That is a font-first decision and is **not** what this crate should
  copy (§7, deferred).
- **Universal practice:** an icon sits on the **text cap-height / x-height box**,
  not the full line box; the icon's *optical* centre is the cap-height centre, not
  the em centre.

### 5.2 What the crate has to measure against (the U-14 lesson)

The crate's glyph geometry derives the baseline from a **face-derived** split,
`ASCENT_SHARE = 0.82` of the cell height (`src/render/text/raster.rs:99-106,
618-624`), and the crate's own U-14 record (`blue24.md:1370`) shows what happens
when a metric is *assumed* rather than *measured*: a wrong face lookup silently
produced a flat `0.6 em` and real collisions.

**Therefore the icon box must be derived from the same metrics the text uses**, not
from a hardcoded 0.82 or a hardcoded 24.

### 5.3 Recommended specification

```rust
// src/widget/icons/mod.rs

/// How an icon box is derived from the text it sits beside.
///
/// `cap_height_ratio` comes from the *measured* face metrics the crate already
/// resolves through `crate::render::text` — never a literal. U-14
/// (docs/plans/blue24.md §12) is the record of what a literal costs: a wrong
/// face lookup produced a flat 0.6 em and text that overlapped by 1.85 px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconBox {
    /// The requested square box, in logical px. `Icon::size` today.
    pub side: f32,
    /// Vertical alignment of the box against the accompanying text run.
    pub align: IconAlignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconAlignment {
    /// Box centred on the text's cap-height band (the default for inline icons).
    CapCentred,
    /// Box centred on the full line box (for leading icons in a list row).
    LineCentred,
    /// Box bottom-aligned to the text baseline (for icons that hang from the
    /// baseline, e.g. a superscript marker).
    BaselineAligned,
}
```

**Alignment rule (concrete):**

- Let `asc`/`desc` be the text ascender/descender the crate's metrics already
  expose for the resolved face. Cap height `cap = asc * CAP_RATIO` where
  `CAP_RATIO` is read from the face where available and otherwise falls back to
  the typographic constant **0.70** (documented as a fallback, not as truth).
- `CapCentred`: `icon_top = baseline - cap - (side - cap)/2`.
  This makes a 16 px icon beside 14 px text sit on the cap band, which is what
  every design system actually does; centring on the *line box* is what makes
  icons look "low".
- Clamp the box to the widget geometry exactly as today's `icon_rect`
  (`icon.rs:334-340`) does, preserving the documented "clip, do not scale"
  behaviour (`icon.rs:203-209`) so existing `Icon` snapshots do not move for
  this reason.

**Optical correction:** Lucide's ink touches `y=2..22`, not `0..24`, so a
`CapCentred` box mapped 1:1 to the design grid is **2 units of dead space** top and
bottom. The mapping must therefore scale the **ink extent** (derived once from the
path bounds, cacheable) into the box, keeping the grid's aspect ratio — i.e.
`scale = side / (grid - 2*stroke_half)`. Getting this wrong is exactly the
"icon looks 2 px small" defect class.

**Delivery:** expose it through the existing style surface rather than a new API —
`Icon::set_size` stays (`icon.rs:274`), and alignment is added as a **style
property** with a default of `CapCentred`, so the declarative/JSON designer path
gets it for free (principles #95-#98, `principle.md:110-118`).

---

## 6. Concrete phased implementation plan (each phase ends in a gate)

File layout targets are real paths in this repo. **Nothing below is written
until its phase is approved.**

### Phase 0 — Pin the source and write the licence record *(gate: nothing ships un-attributed)*

- **Create `tools/icons/LICENSE-lucide.txt`** — a verbatim copy of the upstream
  `LICENSE` file (ISC + the Feather/MIT section). This is the single source the
  NOTICE quotes.
- **Create `tools/icons/VERSION`** — the pinned upstream tag/commit.
- **Create `tools/gen_icon_data.py`** — generator skeleton with `--license=<id>`
  closed set, `--check`, `--outline=<name>`; refuses to run without a licence.
- **Edit `NOTICE`** — append one clearly delimited section (the file's own
  instruction, `NOTICE:1-9`: *"Append a new clearly-delimited section rather
  than editing an existing one."*), containing: included files, origin
  (project/version/source URL), **per-file SHA-256**, a "What it is" paragraph,
  and a **Licence** block naming **ISC** *and* **MIT** with the exact
  "attribution must appear in all copies" requirement. **Yes — attribution is
  required in the repository source, not only in docs**, because both licences say
  "in all copies" and the strings are compiled in.
- **Gate:** `bash tools/check_icon_licences.sh` reports `failed=0` **and** its
  reverse injection fails.

### Phase 1 — The parser (no icons yet) *(gate: unit tests over adversarial input)*

- **Create `src/widget/icons/path.rs`** — `no_std` tokeniser for the SVG subset
  Lucide uses: `M m L l H h V v C c S s Q q T t A a Z z`, absolute + relative,
  implicit repeated coordinates, and the `1.5.5` / `-1-1` number-splitting rules.
  Emits via a visitor callback (allocation-free by construction), **not** a
  `Vec<Point>`.
- **Curve flattening:** adaptive subdivision with a flatness tolerance derived
  from the requested pixel size (the same idea principle #50 states for
  cross-abstraction adapters: *"when the source primitive is missing (arc/ellipse),
  approximate with polylines and let the precision adapt to the size"*,
  `principle.md:79`).
- **Tests in the same file**, including: every command form, relative/absolute,
  implicit repeat, malformed input (`M` with one number) returns `Err` **not**
  a panic, and a curve that flattens to ≥ N segments at 24 px.
- **Gate:** `cargo test --no-default-features --features mini icon::path` and the
  same for `desktop` — proving `no_std`/`alloc_frugal` compile.

### Phase 2 — The data module and the typed enum *(gate: compile-time completeness)*

- **Create `src/widget/icons/mod.rs`** — `IconData`, `IconShape`, the `&[(&str,
  IconName)]` sorted table, `IconName::data(self) -> IconData` (**no `None` arm**),
  and the box/alignment types (§5.3).
- **Create `src/widget/icons/icon_data.rs`** — `GENERATED FILE`, written by
  `gen_icon_data.py`. Starts with the **31 names the crate already declares**
  (parity), so Phase 3 is a behaviour change with a known-diff.
- **Edit `src/widget/display_widgets/icon.rs`** — `IconName` **stays where it is**
  (it is the public enum), but gains `data()` delegating to the new module.
  This honours principle #54 (`principle.md:87`): one definition per semantic
  enum, `pub use` rather than a second copy.
- **Edit `src/widget/mod.rs` / `src/widget/display_widgets/mod.rs`** — declare the
  module under `#[cfg(feature = "icons")]`.
- **Create `tools/icon_census.txt`** + **`tools/icon_census_full.txt`**.
- **Gate:** `cargo check --no-default-features --features desktop` and
  `--features mini`; a temporary variant added to `IconName` in the test proves
  the match is exhaustive (then reverted).

### Phase 3 — Draw through the new data path *(gate: snapshots move deliberately)*

- **Edit `src/widget/display_widgets/icon.rs`** — replace `draw_icon`'s
  `match self.icon_name.as_str()` (`icon.rs:717-751`) with a typed
  `match self.icon { Some(name) => self.draw_data(name.data()), None =>
  self.draw_unknown(ctx) }`. Keep the 28 `draw_*` methods **only** as the
  `stripped_widgets` fallback (principle #21, forward compatibility: no API
  break), gated behind `#[cfg(not(feature = "icons"))]`.
- **Store `Option<IconName>`** on `Icon` (§4.4) while preserving `icon()`
  round-tripping of the original string.
- **Regenerate snapshots**: `cargo run --no-default-features --features
  desktop --example export_control_svgs`.
- **Gate:** `bash tools/check_svg_snapshots.sh` — expect **`icon.svg` and
  `icon.light.svg` to be the only two files that change**, which is itself the
  evidence that the change is contained.

### Phase 4 — Widen the set, vendor the sheet *(gates: census + sheet bytes)*

- **Edit `tools/icon_census.txt`** to the recommended **~200-name core set**;
  run `python3 tools/gen_icon_data.py --license=lucide-isc-mit`.
- **Create `examples/export_icon_sheet.rs`** (mirrors
  `examples/export_control_svgs.rs`'s structure and `GENERATED_MARKER`
  convention) and **commit `snapshots/svg/icon_sheet.svg` +
  `icon_sheet.light.svg`**.
- **Edit `tools/check_svg_snapshots.sh`** — add a step that regenerates the
  sheet into scratch and byte-compares, **without** altering the existing
  `CONTROLS`/`EXTRAS` arithmetic (`check_svg_snapshots.sh:107-141`).
- **Add `tests/icon_census_test.rs`** — the pairwise-distinctness + non-empty-ink
  census over `tools/icon_census.txt`.
- **Gate:** census test green; sheet bytes stable across two runs; a
  deliberately-duplicated path in the generator is refused.

### Phase 5 — Property, theme and docs *(gates: capability + docs match code)*

- **Edit `icon.rs`** — add `icon_alignment` (and, if needed, `icon_grid`) to
  `property_names` / `get` / `set` (`icon.rs:1334-1348`), keeping the
  `CapabilityAccessError` behaviour for genuine type errors.
- **Theme:** map `IconAlignment::CapCentred` to the theme's text colour path
  unchanged (the existing `resolve_color`, `icon.rs:321-329`, is already correct);
  add an `IconRole`-style token **only if** a second role is genuinely needed —
  otherwise do not invent one (principle #51: a shared abstraction must eliminate
  real duplication, `principle.md:81`).
- **Docs:** update the `Icon` type doc to delete the two now-false statements —
  *"rather than a glyph from an icon font, so the rendered result is a plain-shape
  approximation"* (`icon.rs:22-30`) and *"Close and Cross … visually distinct names
  do not always produce distinct output"* — because principle #18 forbids docs
  that contradict code (`principle.md:23`). Add a cross-reference from
  `WindowHandle::set_icon` (`src/app/handle.rs:2657`) explaining that it is the
  *OS window* icon and unrelated to `IconName`.
- **Gate:** `bash tools/check_capability_manifest_roundtrip.sh`(or the event
  analogue), `check_control_has_tests.sh`, and the full
  `bash tools/run_all_gates.sh` once, at the end (principle #55/#56:
  `principle.md:91-92` — do not run gates per round; one full pass at the end).

### Phase 6 — Deferred items register *(gate: documented, not silently dropped)*

Write the deferred list (§7) into `docs/plans/FUTURE.md` so nothing here is
mistaken for done.

---

## 7. Explicitly deferred, with the condition that unblocks each

| # | Deferred | Why now | Trigger to revisit |
|---|---|---|---|
| D-1 | **`RenderCommand::DrawSvgPath` passthrough** (SVG backend emits upstream `d` un-flattened) | Changes a shared enum contract; the flattened path is a strict subset and already ships (§2.6) | When a reviewer rejects a snapshot because flattening is visible at 24 px |
| D-2 | **Multi-tone / duotone icons** | Needs a *role* model (primary/secondary ink) that the theme layer does not have; inventing one for icons would be a second colour path (principle #101, `principle.md:124`) | When a real screen requires two-tone (e.g. a filled badge on an outline glyph) |
| D-3 | **`currentColor` inheritance through arbitrary SVG documents** | Out of scope: only static, crate-resolved ink is in scope; the crate has no CSS cascade | When the crate gains an SVG *document* renderer distinct from its emitter |
| D-4 | **Runtime-loaded icon packs / `.svg` from disk** | `no_std` has no filesystem; the crate deliberately keeps OS knowledge in `src/platform/` (principle #36) | Never for the library; a *host* tool may do it with `Platform` |
| D-5 | **Icon fonts as a source** | BLUE24 §12.2 rules out the whole small-size/atlas/snapshot class | Only if a continuous-size glyph path appears (U-13's own three conditions) |
| D-6 | **`phf` lookup** | `const` slice + `match` has the same asymptotics at ≤1000 entries with no new dep (§3.3) | When the table exceeds ~1000 names and binary search appears in a profile |
| D-7 | **Deflate / bytecode compression of the data** | Costs reviewability; ~50 KB gated is inside the crate's existing budget (§3.2) | When a size gate proves the gated payload exceeds the ~85 KB the crate already accepts for `fonts-cjk-bitmap` |
| D-8 | **Material Symbol optical sizes (20/24/40/48)** | Font-first design; would multiply the data 4× | Only if the crate adopts optical sizes for text too |

---

## 8. Verification evidence collected while researching (for the plan's §0)

| Claim | How it was verified |
|---|---|
| 31 declared icons; `Close` aliases `Cross` | read `icon.rs:32-95`, `:585-588` |
| Unknown name → silent placeholder | read `icon.rs:247-250`, `:750`, `:1264-1277` |
| No icon data exists; geometry is code | 28 `draw_*` in the outline of `icon.rs` |
| Both backends support polylines but not curves | `RenderCommand::DrawPath` (`command.rs:291-295`), SVG emit (`svg/backend.rs:893-909`), software (`primitives.rs:494`) |
| Committed SVG snapshots = **391** files | `ls snapshots/svg \| wc -l` → 391 |
| Gate derives its count (does not hardcode) | `check_svg_snapshots.sh:107-141` |
| Font data must be opt-in | `check_font_data_is_opt_in.sh:26-33`, `:74-87` |
| Every shipped generated table must be in `NOTICE` with digest | `check_font_licenses.sh:34-52`, `:132-143` |
| Census pattern exists | `tools/event_published_census.txt:1-5`, `tools/derive_event_payloads.py:25-63` |
| Lucide = ISC + Feather-MIT | fetched `lucide-icons/lucide/main/LICENSE`, 2026-09-27 |
| Tabler = MIT | fetched `tabler/tabler-icons/main/LICENSE`, 2026-09-27 |
| Material Symbols = Apache-2.0, **and it ships per-icon SVG outlines** (corrected 2026-09-27; see the row above) | fetched `google/material-design-icons/master/LICENSE` + the `.codepoints` font map + the `symbols/web/` per-icon directory listing |
| `phf` works in `no_std` | docs.rs/phf 0.14.0: `default-features = false` "to use phf in `no_std` environments", MSRV 1.85; crate `rust-version = "1.87"` |
| Text metrics must be *measured* (U-14) | `docs/plans/blue24.md:1370` |
| SDF/atlas class ruled out (U-13) | `docs/plans/blue24.md:1439-1479` |
| `no_alloc` visitor pattern is already the crate's style | `for_each_cluster` in `src/widget/metrics.rs:82` |

---

## 9. Recommended answer to each of the six questions

1. **Options:** bundle Lucide SVG paths as `&'static str` (recommended); icon
   fonts rejected (fit + BLUE24 §12.2); raster atlases rejected (snapshot and DPI
   cost); procedural kept only as the stripped-profile fallback and the
   unknown-icon placeholder.
2. **Representation:** `IconData { path: &'static str, grid: u8, stroke: u8, filled:
   bool }`; `IconName::data()` as an exhaustive `match` (no `None`), so a missing
   icon is `E0004`; `&str` lookup by `const`-sorted binary search for the JSON
   path; **no `phf` yet**; feature-gated `icons`/`icons-full` mirroring
   `check_font_data_is_opt_in.sh`.
3. **Theming:** keep `resolve_color()` exactly as it is (explicit → theme
   `text_color` → `PRIMARY`); pass the result as the path's ink. Multi-tone and
   `currentColor` are **deferred** with stated triggers.
4. **Completeness:** three guarantees — compile-time exhaustive `match`;
   generator-time upstream-digest + pairwise-distinctness refusal; runtime census
   over `tools/icon_census.txt` rendering through both backends. Snapshots stay
   flat via **one labelled sprite sheet** (dark/light pair), not 400 files.
   Malformed paths fail at generator time, then at a whole-table crate test, never
   at draw time.
5. **Sizing/alignment:** 24×24 grid with 2 px stroke (Lucide), scaled by **ink
   extent** not grid extent, aligned `CapCentred` against **measured** face
   metrics (never a literal — the U-14 lesson), delivered as a style property so
   the declarative/JSON path gets it for free.
6. **Plan:** six phases (§6), each ending in a named gate, mapped onto real files;
   the licence-attribution file is **`tools/icons/LICENSE-lucide.txt`** (verbatim
   upstream text) with a new **`NOTICE`** section carrying the exact licence names
   (**ISC** and **MIT**), the pinned upstream digest, and the "attribution must
   appear in all copies" sentence.
