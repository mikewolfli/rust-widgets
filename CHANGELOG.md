# Changelog

The canonical project changelog is maintained at [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md).

This root-level file exists for tools and release automation that expect `CHANGELOG.md` at repository root.
When the two disagree, this file is the one that ships; `tools/check_changelog_sync.sh` keeps them identical.

## 2.5.2 (2026-09-21) — The Fifth Rendering Judgement: Every Control Now Has to Paint Inside Itself

Backward compatible: no public signature was removed and no existing behaviour was changed. The one
file removed is a leftover diagnostic example whose imports only resolved on a device profile.

---

### 1. Why a fifth judgement was needed

The four judgements added in 2.5.1 are all measured from a **raster**, and a raster is bounded by the
surface it was rendered into. A control that paints *outside* its own rectangle therefore produces a
perfectly normal census: the escaped pixels are clipped away, the rest are counted, and nothing looks
wrong. The SVG snapshots are the opposite — absolute coordinates and no bound at all — so the same
defect appears there as drawing that leaves the picture.

**Two backends disagreeing about where the ink is, is the defect.** `group_box` shipped a title whose
text sat at `y = -8`, more than half of it above its own frame, while every raster assertion passed.

So `P5` renders each control through the SVG backend and requires every element it emits — `rect`,
`circle`, `line`, `text`, `path` — to lie inside the control's rectangle. It found **68 controls** with
real appearance defects, and they reduce to a handful of root causes:

* **Two coordinate spaces mixed.** `group_box`/`panel` built their title rectangle in *parent* space and
  painted it as *child* space, moving the glyph box half a line down (and therefore half of every title
  outside the frame). `splash_screen` placed its logo block above a box whose height it had taken from
  the wrong reference, putting the logo at `y = -20`.
* **An estimate and a measurement that disagreed.** `TabBar` laid tabs out from `text.len() * 8`
  (bytes!) but drew with a 14 pt font, so a two-character CJK title measured four times its drawn width.
  The estimate now mirrors the renderer's own advance model, and the real metrics are measured before
  the first tab is placed.
* **A line box read as a baseline.** `TextMetrics::ascent` is *inside* the line box, not above it, but
  eight controls centred text with `y + (box - height) / 2 + ascent` — which moves the glyph down by
  almost a full line. This alone pushed the last row of the date pickers, the last line of `empty_state`
  and the status rows of `code_editor`/`terminal_view` clean out of their controls.
* **Half-widths left out of the arithmetic.** A stroke is centred on its chord, a disc extends `r` in
  every direction, and a drop shadow is offset: `line`/`divider` painted outside their box by half a
  thickness, `slider`'s handle sat half outside the track at its minimum, `sparkline`'s last-point dot
  hung over the edge, and `fab`/`popover` grew past their frame once the shadow was added.
* **Text with no width bound at all.** A glyph advances `font.size()` pixels, so a 14 pt label advances
  14 px per character and a long one simply carried on past the control. Around forty call sites did
  this. They now go through one new entry point, `RenderContext::draw_text_fitted`, which takes the
  **rectangle** the text belongs in — every call site already had that rectangle in hand, and passing
  it makes "forgot to bound the text" impossible by construction.

### 2. Two defects in the judgement itself, fixed the same round

A gate that is wrong in the strict direction wastes as much time as one that passes everything, and an
earlier round had already shipped one of each. So `P5`'s first version was audited against every
violation it reported, and **25 of the 68 findings turned out to be the check's fault**:

* it estimated text width as one em per character, where the renderer charges a full em only for wide
  scalars and **0.6 em for everything else** — reporting a 106 px title as 182 px and demanding that
  forty controls truncate text that fits perfectly well;
* it treated the default `stroke-width` as a *half*-width, so **any** element flush with `x = 0` was
  reported as an escape.

Both are corrected. The advance model is now mirrored from the renderer rather than re-invented.

### 3. `map_view` stopped being exempt from the theme

`map_view` was registered in the data-colour table as `map-content`, on the assumption that its palette
*was* the map's. It draws no map at all — the control has no tile pipeline — so what it actually painted
was four hardcoded light colours, and a map inside a dark window was a white rectangle no theme could
change. Those four are chrome (they frame geographic content rather than being it) and now resolve the
theme; the parts that really are data (the grid step, the marker colours, which encode place and
selection) are unchanged. The stale exemption was removed, and the gate reported it as stale itself.

### 4. A second pass: labels that hugged an edge, and one button that was two colours

`snapshots/svg/` is only worth committing if somebody reads it, and reading it found more:

* **`wizard_dialog`.** Its three navigation buttons were laid out **twice** — once in `draw` from
  a 72 px column and once in `handle_event` from a fixed 80 px one placed 20 px and 8 px elsewhere.
  Two of the three did not respond where they appeared. There is now one `nav_button_rects`
  function for both, so a button cannot drift from its own click target. Every label was also
  hugging the top of its 36 px button instead of centring, and the filled button read
  `background_color` — which for a `Surface`-role control is *the dialog's own fill* — so it came
  out grey; on the last step a second token (`success`) took over, making one affordance look like
  two controls. It now reads the theme's `primary`, which is what an action colour is for.
* **A systematic case of the same mistake.** The renderer's text origin is the glyph's **top-left**
  and it paints *downward*, but `TextMetrics::ascent` was read as though it were space *above* the
  line box. So the idiom `centre + ascent` — and its `centre`-only cousin — pushed labels half a
  line down in **`tab_bar`, `menu`, `badge`, `snackbar`, `emoji_picker`, `dock_widget`,
  `command_palette`, `input_dialog`/`dialog`, `popup_window`, `web_engine`, `query_builder`,
  `number_picker`** and made the `heatmap` legend's reserved band one line too short. All are fixed,
  and the arithmetic is now expressed as `box.y + (box.height - line_box) / 2` so the mistake has
  nowhere to hide.
* **`heatmap`'s legend band** was `18` px while its contents (8 px ramp + 2 px gap + 10 px line)
  needed 20. The reservation is now *derived* from the parts instead of guessed, which is why the
  numbers no longer sit on the control's last row.

### 5. The cookbook had drifted two releases behind

`cookbook/` was still pinned at **2.5.0** in all three languages, and two chapters carried pins that
would not resolve at all (`version = "1.0"` and `version = "2.4"`). English also still said **179**
widget kinds where the registry has 180. All three languages are now on **2.5.2** with consistent
counts, and the web-engine chapter no longer calls `WebEngineViewEnhanced` a "full browser engine" —
it is a page model with no HTML/CSS pipeline, which is what `has_real_engine()` says. All three
mdbook builds pass.

> The cookbook had no gate watching it, which is why it drifted. The version now appears in eight
documentation files across four trees; a check that they agree would have caught this the moment
`Cargo.toml` moved.

### Measured facts

| Quantity | Value |
|---|---|
| Controls covered by the rendering census | **188** |
| Judgements asserted (`P1`–`P5`) | **5** |
| Controls with an appearance defect found by `P5` | **68** |
| Of those, findings that were the *check's* fault and were corrected | **25** |
| Root causes the 68 reduce to | **6** |
| Additional label-placement defects found by reading the snapshots | **13 controls** |
| `P5` overflow exemptions | **0** (empty by evidence, not by assumption) |
| Data-colour exemptions after removing the stale `map_view` row | **18** |
| SVG snapshots regenerated | **376** (188 × 2 appearances) |
| Cookbook version pins brought forward (2.5.0 / 1.0 / 2.4 → 2.5.2) | **48** across 3 languages |
| Library tests | **5279 passed, 0 failed** |
| Profiles built clean (`--all-targets`) | **5 / 5** (`desktop`, `tablet`, `mobile`, `mini`, `embedded`) |
| `clippy -D warnings` | **clean** |

### Upgrading

Nothing to do. `draw_text_fitted` is additive, the exemption tables only lost a row that had gone
stale, and the SVG snapshots are generated artifacts that regenerate byte-for-byte.

## 2.5.1 (2026-09-21) — A Control's Rendering Became a Verified Dimension, the WebEngine Stopped Claiming What It Could Not Do, and Three Broken Builds Were Fixed

Backward compatible: no public signature was removed and no existing behaviour changed. The one
removed item is a **private** trait (`NativeWebEngine`) that had a single implementation which never
displayed anything — see "The WebEngine became honest" below.

---

### 1. The rendering dimension — 188 controls now have their pixels checked

Every gate before this one was built on **declarations**: does the control publish a property, does
it publish an event, does it `impl Draw`. Round 58 shipped four defects the user saw with their own
eyes — a control laid out off-canvas, a window fill covering its children, a theme switch that did
nothing, `list_box` painted in the window's own colour — and **every one of them satisfied all 64
gates that existed**, because none of those gates looked at a rendered pixel.

This release adds that dimension, in five layers:

* **A rendering golden table.** All **188** controls are constructed, rendered twice (light and dark)
  into an in-memory raster, and asserted on four judgements: `P1` it painted at least one pixel
  distinguishable from its background; `P2` its dominant colour is not the surface it sits on; `P3`
  its dominant colour differs between the two appearances; `P4` each of the four semantic tokens
  (`error`/`warning`/`success`/`info`) has a real consumer and moves with the appearance. The
  traversal unit is the **canonical name**, never `WidgetKind`: 13 kinds are shared by 2–5 controls,
  so a kind sweep would have silently skipped 19 of them.
* **A data-colour exemption table.** `P3` would otherwise fail a chart whose *series* colour is
  deliberately constant. Controls whose colour **is** the data (series palettes, K-line red/green,
  the colour spectrum, meter thresholds, map tiles) are registered in
  `tools/control_color_exemptions.txt` **with a written reason**; the four controls that carry
  *semantic* colours (`banner`, `calendar`, `progress_dialog`, `message_box`) are refused entry, so
  "the theme declares four semantic tokens and nobody reads them" cannot be legalised.
* **A declaration/implementation alignment gate.** Three assertions that no existing gate could make:
  `Q1` every declared property is answered by the control that declares it (with its declared
  writability); `Q2` every `draw` body actually paints — an empty `impl Draw` is what principle #5
  forbids; `Q3` every published event list is a set and carries a payload shape.
* **376 SVG snapshots** under `snapshots/svg/`, one per control per appearance, named by canonical
  name, committed, with a regenerate-and-compare gate. A wrong-looking control is not something an
  assertion can catch; a diffable image is.
* **A WebEngine that reports what it is**, below.

### 2. Real defects this dimension found (and fixed)

These were **not** visible from any declaration, and every one of them is now covered by an
assertion with a reverse-injection record:

| Defect | How it was found |
|---|---|
| `message_box` declared `modal` as neither readable nor writable while the control had a working getter and setter | `Q1` — the designer was hiding a property that works |
| `order_book::show_spread` answered `TypeMismatch` for a non-bool write and `ReadOnlyProperty` for a bool one | `Q1` writability — a caller was told "wrong type" about a name that can never accept a write |
| The JS engine **documented arithmetic** and `1 + 2` returned `undefined` | `Q1`'s investigation — the docs promised what the code did not do |
| The SVG exporter rendered every control with the theme never applied, so `<name>.svg` and `<name>.light.svg` were byte-identical apart from a comment | `check_svg_snapshots.sh` step [4] — the snapshots existed and proved nothing |
| One example and one test used a crate-level `#![cfg]` without `required-features`, so `cargo check --all-targets` on `mini` failed with `E0601: main function not found` | `check_profiles.sh` |
| The clipboard test raced other tests on the process-wide clipboard and failed intermittently under the parallel harness | repeated `cargo test` runs |

### 3. Three build configurations that were already broken are now fixed

`cargo check --no-default-features --features mini`, `... --features embedded`, and
`--features "windows desktop-runtime controls-native controls-custom"` **failed at the previous
tag**: the widget layer referenced `crate::theme` from ~120 files, while that module is gated on
`device_profile` and those three configurations do not set it (78 errors in the last one).

Widening the theme module was not the fix — it needs `serde` and the capability registry. The fix
is a single always-available entry point in `src/style/` (`resolved_theme_style`,
`resolved_theme_style_for`, `theme_manager`, `semantic_color`, plus the type shapes), so one path
compiles in every profile and answers "no theme" where there is no theme. `mini` has no colour
model at all, and now says so instead of failing to compile.

### 4. The WebEngine became honest

`WebEngineView` models a web page; it does not render one, and now says so.

A real engine used to be reachable on Linux behind the `webkit-engine` feature: **76 lines** of
one-line forwards to `webkit2gtk`, one platform, and the `WebView` was **never added to a GTK
container** — so no user could ever have seen a page through this library, while the feature list
and the docs said otherwise. It was removed rather than completed, because completing it means
500–1500 lines per platform plus a hard dependency on system libraries, and because JavaScript
evaluation (the other half of "web support") runs on the pure-Rust `boa` engine and never went
through that trait at all.

What replaced it:

* `Platform::supports_web_engine()` — a capability question, answered `false` on every current
  backend, instead of an `Option` whose `None` conflated "no engine exists" with "the engine could
  not be constructed".
* `WebEngineViewEnhanced::has_real_engine()` — the degradation is now **queryable** from the widget
  itself. It previously was not: the constructor's doc told a caller that had to know to "query the
  platform directly", which is impossible when the *widget* is what held the engine. That made
  "this is a simulated view" undetectable — the same defect class as an event that is published but
  never emitted.
* `tools/check_web_engine_honest.sh` fails if any of the removed names returns.

### 5. Two toolchain-independent gate defects (false failures on Windows)

Four gates reported failures that were about the *host*, not the code: TOML manifests written with a
native Windows path (`\` starts an escape, so the file was unparsable), and `cargo package --list`
output compared verbatim against forward-slash paths. Also, `check_android_cross.sh` /
`check_ios_cross.sh` reported a missing toolchain as `FAIL` rather than `SKIP`, so an unavailable
target was counted as a defect in the code under test. All four now report accurately.

### Measured facts

- `cargo test --no-default-features --features desktop` → **5543 passed / 0 failed** (45 suites).
- `cargo clippy --no-default-features --features desktop --all-targets -- -D warnings` → **clean**.
- `cargo check --no-default-features --features <desktop|tablet|mobile|mini|embedded> --all-targets`
  → **0 errors, 0 warnings** on all five (three of which did not build at the previous tag).
- Rendering census: **checked=188, skipped=0, failed=0**.
- Declaration alignment: **checked=188, skipped=199 (each with a reason), failed=0**.
- SVG snapshots: **376 files** (188 controls × 2 appearances), regeneration byte-identical.
- Semantic tokens: all four have consumers; none is an empty declaration.
- WebEngine: zero residue from the removed wrapper; `supports_web_engine()` is `false` everywhere.

See [`docs/log/log-20260921-3.md`](docs/log/log-20260921-3.md) for the per-change evidence, the
reverse-injection records, and the layer-by-layer counts.

## 2.5.0 (2026-09-21) — Events Became a Typed Contract, a Project Document Becomes Rust Source, and the Designer Is Gated and Committed

Backward compatible. **No public signature was removed and no existing behaviour changed.** This
release is one coherent piece of work in three parts, all of it additive:

1. **The event side became a typed contract.** `WidgetCapability.events` changed from a name array to
   a schema carrying each event's payload kind, derived from the control's real signal declaration;
   the designer's manifest round-trips through JSON byte-identically; the JSON event path and the
   capability event table were merged into one route.
2. **A project document can now become Rust source.** `rust_widgets::designer::generate` emits a
   compilable Rust function for hardware targets that cannot run the runtime JSON loader at all.
3. **The generator is gated and its artifacts are committed and verified.** The `designer` feature is
   on for `desktop` (the profile that hosts a designer) and off elsewhere; the generated sources are
   checked in, with a regenerate-and-compare gate that makes committing them safe.

See [`docs/log/log-20260921-1.md`](docs/log/log-20260921-1.md) and
[`docs/log/log-20260921-2.md`](docs/log/log-20260921-2.md) for per-change evidence.

### Measured facts

- `cargo test --no-default-features --features desktop` → **5479 passed / 0 failed**.
- `cargo clippy --no-default-features --features desktop --all-targets -- -D warnings` → clean.
- `cargo check --no-default-features --features <desktop|tablet|mobile|mini|embedded> --all-targets`
  → **0 errors, 0 warnings** on all five. `--features desktop,no-declarative-view` and
  `--features tablet,designer` are clean as well.
- `bash tools/check_designer_feature_gate.sh` → passes; `desktop` resolves `rust_widgets::designer`,
  the other four profiles do not, and `tablet,designer` does.
- `bash tools/check_generated_sources.sh` → passes: the committed artifacts carry the generated
  marker, regeneration reproduces them byte for byte, they compile under `-D warnings`, and both
  reverse injections go red as required.
- `bash tools/check_declared_targets_ship.sh` → passes; `tools/designer_generate.rs` was added to
  the `include` list, so the target the new example declares now ships.
- `bash tools/run_all_gates.sh` → **PASS=42 FAIL=1 TIMEOUT=0 SKIP=1**. The FAIL is
  `check_profiles.sh`, which needs MSVC's `lib.exe` on an `x86_64-pc-windows-msvc` target this Linux
  host does not have — a host-tooling gap reproduced by stashing every change, so it is unrelated to
  this round. The SKIP is `check_apple_native.sh`, which needs macOS.

### The generator became a capability with a boundary, not code that is always there

The generator exists (see the section below). This release also decides **where it is allowed to
exist**, and answers that with a feature: `designer` is a **development-time** capability — a
code generator plus an artifact writer that writes Rust source into the tree — and `desktop` enables it by default, because `desktop`
is the profile a designer **host** runs on.

`tablet`, `mobile`, `mini` and `embedded` leave it off, and the reason is not tidiness. They are the
**targets** of a generation, not its hosts: a device that receives `ui_stripped.rs` never runs the
program that wrote it. Linking a code generator and `std::fs::write` into a shipping application is
exactly the weight mode 2 exists to remove, so the default is the narrow one and a caller who wants
the tool elsewhere asks for it by name — `--features tablet,designer`.

### The gate is an alias in `build.rs`, not a bare `feature = "designer"`

The condition is a **conjunction**: a real device profile, not a stripped widget set, *and* the
caller having opted in. A conjunction hand-written at more than a couple of call sites drifts, which
is what rule #47 forbids, so it is written once as the `designer_tooling` alias that `build.rs`
emits, and `src/lib.rs` reads `#[cfg(designer_tooling)]` rather than the feature. The alias is also
registered with `cargo:rustc-check-cfg`, so a typo in the `cfg` is a build error instead of a
condition that is silently false.

Folding it into the existing `full_widgets` alias would have compiled, and would have been wrong:
`full_widgets` answers "does this build have the widget tree?", which every `tablet` and `mobile`
application needs, while `designer_tooling` answers "is this build **also** a design tool?", which
none of them do.

The boundary is checked by compiling rather than by grepping. A grep for `"designer"` in
`Cargo.toml` proves nothing about what the compiler sees, so `tools/check_designer_feature_gate.sh`
compiles a probe crate whose only job is to *name* `rust_widgets::designer`. It fails in both
directions that matter: the tool leaking into a delivery profile (someone copies `"designer"` into
`tablet`, and every tablet binary silently ships a generator) and disappearing from `desktop` (the
designer can no longer be built by its own host profile). A third step keeps the gate a *default*
and not a prohibition, by requiring the explicit `tablet,designer` opt-in to resolve.

### Generated sources are committed, and that is a trade with a price

`blue19.md` §5.1.6 left this open. It is now decided: **generated sources are committed**, for
reviewability. A generated file in the tree is a diff — a reviewer sees that a control was added,
moved or had a property changed, in the same pull request as the project document that caused it. A
file generated at build time is invisible until it breaks the build.

Committing has a cost, and it is stated rather than glossed: **the tree can hold a stale file.** A
designer edits `project.json`, commits the document, and forgets to regenerate. The tree now claims
to describe a UI it does not, and nothing about the committed `.rs` file looks wrong — it is valid
Rust that compiles. That is why the decision is only safe **with** a regenerate-and-compare gate:
`regenerate → `cmp` → fail on drift`, the same shape `tools/check_abi.sh` already uses for the C
header, for the same reason. Both halves are load-bearing — committing without the gate is how a
stale artifact ships, and the gate without committing has nothing to compare against.

`tools/check_generated_sources.sh` asserts four things: that every committed artifact carries the
generated marker, that regenerating reproduces the committed bytes exactly, that the committed
artifacts compile under `-D warnings`, and that both of those can fail. The last is not decoration:
a `cmp` against a file the tool just wrote passes trivially, so the gate edits the project document
and requires different bytes to come out. It also restores the tree and re-verifies it in sync
before exiting, because a gate that corrupts the tree on its way out is worse than one that fails.

### A write is refused if the text lacks the generated marker

Every file the generator writes starts with `GENERATED_MARKER`. Two things depend on it, and the
second explains why the *writer* enforces it rather than only the gate reading it: a file without
the marker is classified by the drift gate as **not generated** and skipped, so a marker-less write
would silently disable the drift check for that file while every gate still reported green.
`write_one` therefore returns an error instead of writing, and the refusal leaves nothing behind.

`--check` in `tools/designer_generate.rs` applies the same distinction on the reading side: a file
that exists without the marker is reported as "not a generated file" rather than as a diff against a
hand-written module that happens to share the path.

### The designer calls the generator through an API and a CLI

`designer::artifact::regenerate_into` returns a per-file outcome — `created`, `updated` or
`unchanged` — so a designer's status area can distinguish "saved" from "no change" instead of
re-announcing a write that did not happen. `ArtifactOutcome::wrote()` is that distinction as a
predicate, and the gate's success criterion is the same fact: it treats a run that changed nothing
as the expected outcome.

`tools/designer_generate.rs` is the same generation from a shell, because a build script, a reviewer
checking a colleague's committed artifact, and the gate itself all need it and none of them should
re-implement the argument handling. One line per file, `created|updated|unchanged <path>`, then a
summary, so a script and a status area read the same output. `--check` detects drift without
writing, which is what lets it run on a read-only checkout; its exit statuses are distinct — `1` for
stale artifacts, which is a **finding**, and `2` for a broken tool, which is not.

### Two files, not one, because the two templates emit mutually un-compilable code

The committed artifacts are `examples/generated_project/src/generated/ui_default.rs` and
`ui_stripped.rs`. One file was never an option: the default template names `crate::view`, which a
`mini` build does not compile, and the stripped template names nothing from it, so a single file
would fail to build on every target. One file per template keeps the choice in `Cargo.toml` — which
target compiles which file — rather than in generated `cfg` attributes the generator cannot reason
about.

The names are keyed on the **profile**, not the template, because the profile is what a reader
builds: `ui_default.rs` is compiled by `--features desktop`, `ui_stripped.rs` by `--features mini`.
`desktop`, `tablet` and `mobile` share one file because they emit **identical** code, so a reader
looking for `ui_tablet.rs` is looking for something that should not exist.

### The committed artifacts are verified twice, and the second check found a defect

The sync check answers "is the file in the tree the file the generator would produce?".
`tests/generated_artifacts_are_lint_clean_test.rs` answers the other question — "is what is
committed any good?" — by compiling the **committed** files, not freshly generated text, under
`RUSTFLAGS="-D warnings"`.

That check found a real defect on its first run, which is why it is a gate and not a nicety. The
generator's `mut` placement was a guess, and it was wrong in **both directions at once**:
`warning: variable does not need to be mutable` on every child whose setters ran inside their own
block (the outer binding is read once, by `add_child`), and `error: cannot borrow root as mutable` on
the root, which does need it. Neither showed up in `tools/check_generator_output_compiles.sh`,
because that gate's fixture happened to have a child with no setters at the root level. A generated
file that warns under the host's own lints fails a downstream `-D warnings` build for a reason that
has nothing to do with the project document.

### Why the reverse injection for that step is a `mut` and not something else

The gate re-introduces exactly the defect the lint step was written for: it restores the
unconditional `mut` in the generator, regenerates, and requires the lint step to **fail**. An
injection that was caught by a different assertion in the same file (a compile error, say) would
prove the file runs, not that the step can see the class of defect it exists for — the same
distinction `tools/check_mode_consistency.sh` makes when it omits a child and requires a red gate.
The generator source is restored and the artifacts regenerated afterwards, so the tree is left
exactly as it was found; a gate that leaves drift behind makes every later gate fail for a reason
it did not cause.



#### A project document can now become Rust source, and "it compiles" is a gate

##### Measured facts

- `cargo test --no-default-features --features desktop` → **5462 passed / 0 failed**.
- `cargo clippy --no-default-features --features desktop --all-targets -- -D warnings` → clean.
- `cargo check --no-default-features --features <desktop|tablet|mobile|mini|embedded> --all-targets`
  → **0 errors, 0 warnings** on all five.
- `bash tools/check_generator_output_compiles.sh` → passes; a generated program is compiled for real
  against `desktop`, `tablet`, `mobile`, `mini` and `embedded`. The gate takes **~33s**, down from
  265s once the probe crates were made to share the workspace target directory and the injection step
  stopped re-running all four cases.
- `bash tools/check_mode_consistency.sh` → passes (6 tests), with reverse injection.
- `bash tools/check_generator_reuses_wire_rules.sh` → passes, with reverse injection.
- `bash tools/run_all_gates.sh` → **PASS=40 FAIL=1 TIMEOUT=0 SKIP=1**. The single FAIL is
  `check_profiles.sh`, which needs MSVC's `lib.exe` on an `x86_64-pc-windows-msvc` target this Linux
  host does not have — a host-tooling gap reproduced by stashing every change, so it is unrelated to
  this round. The SKIP is `check_apple_native.sh`, which needs macOS.

### A design document can now become Rust source, not only be interpreted

Mode 1 already existed: `crate::json` reads a project document and builds the UI at run time, so
editing the document costs no recompilation. That is the right shape for the design loop and the
wrong shape for shipping. `rust_widgets::designer::generate` adds the other direction — it parses the
same document (through `JsonProject::parse`, the *same* parsed-project type mode 1 reads, so the two
modes cannot disagree about what a document means) and returns Rust source plus a `GenerationReport`.

Both modes exist because they answer different questions, and for two profiles mode 2 is not an
alternative but the **only possible output**: `crate::json` and `crate::view` are both compiled out
of `mini` and `embedded`, and the `alloc_frugal` budget admits neither. A device running `mini`
cannot run the generator either — it is the **target** of one — and that is stated plainly in the
module rather than glossed: a designer runs on a desktop host, and `mini`/`embedded` receive the
generated file.

### Two templates, not one template with flags

The generator emits one of two shapes, keyed by `TargetProfile`:

| Target | Emitted shape |
|---|---|
| `desktop` / `tablet` / `mobile` | a `Node` tree plus a `ViewEngine::mount` call |
| `mini` / `embedded` | imperative construction plus `add_child`, coordinates solved at generation time |

The split is not stylistic. Sharing one template would mean every line carrying a conditional, and
`create_button` and its family are gated behind `cfg(not(alloc_frugal))` — a mistake that way leaks
`create_button` into a `mini` build, compiles fine on the desktop host, and fails only on the target.
`TargetProfile::Default` covers three profiles rather than three values because the difference
between them is device capability discovered at run time, not a difference in the API surface —
collapsing them is what keeps a designer from maintaining three copies of one template.

### A property of the output that a text assertion could not check: it compiles

BLUE19's definition of done for this task does not accept "should work": the stripped template's
output must compile for real under `--no-default-features --features mini` and `--features embedded`.
`tools/check_generator_output_compiles.sh` writes the generated text into a throwaway crate, depends
on this library with the target's feature set, and runs a real `cargo check` — for the stripped output
under `mini` and `embedded`, and for the default output under `desktop`, `tablet` and `mobile`.

This is the only check that can find the class of defect the requirement exists for, because every
member of it **compiles fine on the desktop host**. Four were found this way during the work, one per
run, and all four are now recorded in the generator and the gate:

- a reference to `crate::view` from a stripped build, where the module does not exist;
- a call into `widget::runtime` (which is `cfg(not(alloc_frugal))`) — the stripped template first
  reached for `runtime::register` to obtain a control id, when a stripped target has no registry and
  the id the control already owns is the only one to hand a parent;
- `Button::new("Go", ..)`, where the constructor takes `String` and the stripped profile has `alloc`
  but not the standard prelude, so a bare `&str` does not coerce;
- `Slider::new(text, geometry)`, where the constructor takes geometry only, producing "unexpected
  argument".

This is why the gate is a compile rather than a `grep`: a test asserting that the output contains
`add_child` would pass against code that never builds.

### Mode consistency is a gate with reverse injection

"The two modes agree" is not one testable claim, so it is asserted as three facts a user can observe,
each able to fail on its own: **structure** (the same controls in the same parent/child arrangement),
**properties** (the same names with the same values) and **declared handlers** (the same published
events reachable). `tools/check_mode_consistency.sh` runs `tests/mode_consistency_test.rs`, which
reads mode 1's tree from the loader and mode 2's from the **emitted text** — comparing the generator
against the loader directly would compare mode 1 with its own input, so mode 2 is read back from the
source it wrote.

A compile check alone cannot catch this: a generator that emitted a **smaller, still-correct** tree
would compile perfectly while losing a control the user drew. That is why the gate's second step is
reverse injection: it makes the generator omit the last child of every node and **requires the gate to
go red**, then restores the source and requires it green again. Injection is what makes the claim
falsifiable rather than decorative.

### The generator reuses the runtime's wire rules, and that has a gate too

The generator does not restate the type-compatibility rules a wire must satisfy; it consults
`WIRE_RULES`, the same table the runtime uses, exported through `is_wire_key` and
`shared_wire_rule_count`. The failure mode this guards against is **not a missing call** — it is a
*second table* that happens to agree today and drifts the first time a `PropertyValueKind` variant is
added, at which point the designer accepts a wire the generated program rejects, and nothing in the
generator's own tests shows it. `tools/check_generator_reuses_wire_rules.sh` checks that the
generator names the table, then injects a local verdict and requires the gate to fail — so a
generator that named the table and ignored it (decorative reuse) would not pass.

### Capacity and layout are resolved at generation time

Layout is solved before any control exists: the generator runs the real `crate::layout` engine at
generation time and emits the resulting coordinates as literals, so the generated program carries no
second layout engine that could drift from the runtime's. Capacity is checked the same way, and the
bound is **per target** because it is a storage fact, not a policy: `mini`'s `BaseWidget::children` is
a fixed-capacity `MiniVec` (`MINI_CHILD_CAPACITY = 64`) and exceeding it **silently drops** the extra
children. A generated program that did so would look complete and be missing controls, so a container
over capacity is **reported** in `GenerationReport::capacity_overflow`, not emitted. A heap-allocating
target has a sanity bound (`DEFAULT_CHILD_CAPACITY = 4096`) rather than a storage limit, and the test
asserts the report is empty there — reporting those would be noise.

The same "reported, not silently dropped" contract covers everything the generator cannot express:
`GenerationReport::unsupported` names each node it refused with a reason. A generator that quietly
omitted a control would produce a program that looks right and is not.


#### Events became a typed contract, and the designer manifest round-trips

##### Measured facts

- `cargo check --no-default-features --features <profile>` → **0 errors, 0 warnings** on all five
  of `desktop`, `tablet`, `mobile`, `mini`, `embedded`.
- `python3 tools/check_event_payload_types.py` → **187 controls covered, 326 published pairs**,
  every declared payload matching the Rust type of its signal.
- `bash tools/check_designer_manifest_roundtrip.sh` → **4 passed / 0 failed**.
- `bash tools/check_event_signal_dyn.sh` → passes (3 converted controls resolve every name their
  capability publishes).
- `bash tools/check_json_event_route.sh` → passes: 8 compatibility keys declared, 326 published
  events in the table, reverse injection detected.
- `bash tools/check_enabled_is_honoured_containers.sh` → passes (6 container files with ungated
  emitting mutators, 29 accepted with no emitting mutator or a written reason).

### Events became a typed contract instead of a list of strings

`WidgetCapability.events` was `&'static [&'static str]` — the library stated *that* a control
emits `value_changed` but not *what arrives with it*. A designer reading that list can draw a
wire it cannot label, and a manifest that describes a payload as a scalar when the signal
carries a tuple is asserting something the control never does.

The field is now `&'static [EventSchema]`, with the payload expressed as two orthogonal fields
rather than one wider enum:

```rust
pub struct EventSchema {
    pub name: &'static str,
    pub payload: Option<PropertyValueKind>,   // what the value is; None = no payload
    pub shape: Option<EventPayloadShape>,     // how the value is arranged; None = no payload
}
```

Splitting the two is what lets the table be honest about the cases that motivated the change.
`PaneLayoutChanged` carries `Vec<f32>`, `TabMoved` carries `(usize, usize)`, and
`RichEdit::selection_changed` carries `Option<(usize, usize)>`. Folding those into
`PropertyValueKind` leaves only two options, and both are wrong: combinatorial variants
(`Tuple2UInt`, `ListFloat`, …), or a claim that discards a component — calling a pair of
integers `UInt` loses the second half, calling it `String` loses the fact that both halves are
numbers. So `shape` carries the arity and `payload` carries the element type, and the two
never contradict each other. Across the **326** published pairs the measured distribution is
`Scalar=192`, `-=91` (no payload), `Tuple2=16`, `OptionalScalar=10`, `Mixed=8`, `ListScalar=5`,
`Tuple4=2`, `Tuple3=1`, `OptionalTuple2=1`; `payload` is `String=101`, `UInt=80`, `Bool=27`,
`Int=13`, `Float=9`, `Color=4`, `Rect=1`.

The same reasoning applies to the values that were *not* invented. Domain types (`Font`,
`DateRange`, `BarcodeResult`, `Shortcut`, and the rest) are all carried as token strings with
`payload = String`, because a designer that does not understand a domain object can still
display it and forward it, whereas inventing a JSON encoding for each one would be
manufacturing semantics the library does not have. `Color` and `Rect` are the exception: they
already have `CapabilityValue` variants and stay on that existing pipeline.

### The payload type is derived, not written down, and a gate re-derives it independently

Three hundred and twenty-six hand-written payloads would be 326 opportunities to guess wrong,
and a wrong declaration is worse than a missing one: the designer draws a connection that
cannot be made. `tools/derive_event_payloads.py` therefore reads each payload off the
**signal declaration itself** — struct name, then `pub <name>: SignalN<T>` or
`pub fn <name>_signal()`, then `T` recursed through `Option`/`Vec`/tuples — and exits with an
error if any step cannot resolve, rather than skipping. The published *names* are kept separate
in `tools/event_published_census.txt` so the deriver can never take its own previous output as
the source of names; that confusion is what let an empty table survive rounds, because "the
derivation failed" and "this control publishes nothing" look identical.

The gate `tools/check_event_payload_types.sh` is a **second independent reader**, not a
comparison against the generator — comparing the table to the generator's output would be
tautological, since any generator bug would appear on both sides. It parses the `EventSchema`
rows actually declared in `src/widget/capability/event_payloads.rs`, re-derives each pair from
the signals, and reports control, event and both answers on a mismatch, along with coverage in
both directions. Reverse injection proves the gate can fail:
`python3 tools/check_event_payload_types.py --inject=slider.value_changed` reports that the row
claims `payload=Bool/shape=Scalar` while the signal `Signal1<i32>` is `payload=Int/shape=Scalar`
and exits 1 — and the gate itself fails if an injection does **not** produce a failure, so a
comparison that only ever prints `ok` cannot pass as a check.

### The designer manifest round-trips byte-identically

`capability_manifest_json(factory, control)` exports one control's capability description and
`DesignerManifest::from_json` reads it back through an **independent parser**. Serialisation is
hand-written rather than `serde`-derived, and the reason is a measured feature fact: `serde` is
in the `desktop`, `tablet` and `mobile` feature lists but **not** in `mini` or `embedded`, so a
`derive(Serialize)` on the capability layer would make that layer's data shape depend on the
profile — and a second, `cfg`-gated description is exactly the duplication the project sets out
to avoid. The handwritten encoder gives a stable field order and a diffable document.

`tests/designer_manifest_roundtrip_test.rs` exports a control, loads it with the independent
parser, exports again, and asserts the two strings are equal — for **every one of the 187
controls, not a sample** — plus sentinels (`slider.value_changed` carries `"payload": "int"`
and `slider_pressed` carries `"payload": null`) so that "two empty strings are equal" cannot
pass. A byte-identical assertion is what makes the load half real: it is what rejected an
ingenious-looking `Color` default written as `[1,2,3,4]` when the capability table actually
stores `"#DCDCDCFF"`, and it caught a trailing comma the writer emitted before `}`.

### One call wires every published event, and "is it wired?" is queryable

`EventSignalBinder::forward_all(widget)` wires **every** event the control publishes in a single
call, instead of one `connect_event` per name. Alone that is convenience; what makes it safe is
`event_is_wired(widget, event_name)`, which answers a question `connect_event` cannot. Wiring
18 of a control's 20 events is a silent failure: every call returns success, nothing is
reported, and the two events that were missed simply never arrive.
`tests/event_wiring_test.rs` covers the lifecycle directly — one call wires a unit event and a
payload-carrying event, every published event of a converted control resolves, an unwired event
reports unwired rather than succeeding, an unknown name reports false, a detached binder
reports nothing wired, and a control with no events reports zero.

`bash tools/check_event_signal_dyn.sh` covers the converted controls independently, resolving
every published name of `button` (4), `check_box` (2) and `slider` (4) through the dynamic
signal path.

### The JSON event route was merged: one path had no gate at all

`src/json/` held a second, independent event path of **eight hard-coded `on_*` keys**
(`on_click`, `on_change`, `on_close`, `on_double_click`, `on_focus`, `on_blur`,
`on_selection_changed`, `on_value_changed`) matched by hand in the loader. The two sets did not
intersect in the way that matters: `on_click` is not a published name — `clicked` is — and
adding a published event to a control never made it declarable in JSON. Nothing covered the
path, so it could drift, and it had.

A node can now declare handlers against the **published name**, resolved against the capability
table:

```json
{ "button": { "text": "Go", "events": { "clicked": "on_go" } } }
```

The `on_*` keys are **kept**, and the reason is that they are not a second spelling of the same
thing: `on_close` means the trigger intent `Closed`, `on_selection_changed` means
`SelectionChanged`, and neither is a name the published table carries; `on_double_click`,
`on_focus` and `on_blur` exist because a pointer-driven control routes them through a value
callback the published signal cannot address. Each key now carries an explicit trigger marker
in one table (`MARKER_KEYS`) rather than being extracted by two positional functions, and
`tools/check_json_event_route.sh` asserts the boundary mechanically in both directions: every
`on_*` key the loader reads must carry a stated marker, and every `events:` name must be one the
capability table publishes. The eight keys and 326 published events are reported by the gate on
every run, with a reverse injection proving it fails when the single key source is broken.

### Mirror fields are individually classified, and containers honour `enabled`

Every `WindowState` field is now either a documented fallback or has a **named test proving it
is read** (`src/app/handle.rs`). "Written but never read" is a mirror that drifts into a false
fact, and the classification is parsed against the struct by the test itself, so adding a 14th
field fails until someone states which category it is in and names the test that backs the
claim. The 13 fields split into platform-first fallbacks (six backends return `None` from
`window_icon` and `window_min_size`; the flag mirrors use `mirrored_flag`), values read by
`center_on_screen`, and `close_callback`, which is authoritative because `close()` is its only
reader.

The handler gate covered the entry point — a control that consumes input must consult
`is_enabled()` — and had a structural blind spot at the exit: the container that owns a
*programmatic* mutator. `StackedWidget` was allowlisted as "passive: its handler only
delegates to the base", and the handler did delegate; but `set_current_index` emitted
`current_changed` while the control was disabled, so a subscriber reloaded a page the user
could not reach. `handle_event` was never on the path, so the existing gate could not see it.
`tools/check_enabled_is_honoured_containers.sh` covers the exit point the way the handler gate
covers the entry point: every programmatic signal this library emits from an allowlisted
container file must be gated by `enabled`, be absent, or carry a written reason.
