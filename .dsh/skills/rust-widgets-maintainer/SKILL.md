---
name: rust-widgets-maintainer
description: Maintain, extend and audit the rust-widgets Rust GUI library and everything beside it. Use when changing controls, layouts, rendering, platform backends, the code editor, capability metadata, JSON layout loading, the rust-widgets-cli `rwui` tool (check, json2rust, fmt, create), the suites/font/icon/mobile-services crates, the demo applications, this DSH plugin, or running the repository's validation gates.
whenToUse: Load when working inside the rust-widgets repository on a coding, review, or validation task, including tasks that touch `crates/`, `demo/`, `examples/`, or `dsh-plugin/`.
metadata:
  license: MIT
  project: rust-widgets
---

# rust-widgets maintainer

`rust-widgets` is a dependency-light Rust GUI library with around 242 controls,
several platform backends, a JSON layout loader, a designer, a source-code
editor, a designer CLI, and optional companion crates. Work is governed by
`docs/plans/principle.md`; read it before touching code.

## Non-negotiable rules

1. **Evidence, not claims.** A change is "done" only after `cargo check`,
   `cargo test` and `cargo clippy --all-targets -D warnings` have actually run.
2. **Fix the category, not the instance.** After fixing one bug, grep for the same
   pattern in sibling modules and fix or explicitly exclude it.
3. **No placeholders.** `todo!()`, `unimplemented!()`, empty bodies and log-only
   stubs are forbidden.
4. **Platform isolation.** `cfg(target_os)` and OS probing live only under
   `src/platform/`. Public APIs must not mention `native`, `self_drawn`, `HWND`,
   `NSView` or `GTK`.
5. **One concept, one implementation.** Do not add a second cache, event path or
   enum for something that already exists; reuse or consolidate.
6. **Comments are English** in every new module.
7. **Do not rename** existing public properties, methods or variables unless the
   name is genuinely conflicting.
8. **Do not run the full gate matrix per iteration.** Run focused checks while
   editing, then one full validation at the end.

## Repository map

| Path | Responsibility |
|---|---|
| `src/widget/` | Controls, capability metadata, registries |
| `src/widget/base.rs` | `BaseWidget`: geometry, signals, redraw/damage |
| `src/widget/runtime/` | Mounted-widget registry, frame driver, repaint |
| `src/widget/special_widgets/code_editor/` | Source editor: buffer, render, IME (feature `code-editor`) |
| `src/widget/special_widgets/editor/` | Shared editor layer: `syntax` vocabulary + lexer (`builtin-languages`), `caret`, `companions::FindBar` |
| `src/layout/` | Layout engines (`BoxLayout`, `Grid`, `Flex`, `Flow`, …) |
| `src/render/` | Render commands, software backend, SVG backend |
| `src/platform/` | OS backends behind the semantic `Platform` trait |
| `src/json/` | Declarative JSON layout loader |
| `src/designer/` | In-process designer: generator, artifacts, history |
| `src/data_binding/` | Model ↔ property bindings |
| `src/widget/capability/` | Property/event schemas, factory registration |
| `themes/` | Theme presets (generated; see `themes/generate.sh`) |
| `crates/` | Companion crates, each a separate package (see below) |
| `demo/` | Runnable demo applications |
| `examples/` | Cargo examples, including `paradigm_selection` and `generated_project` |
| `bindings/` | Language bindings (C ABI surface) |
| `cookbook/` | Book sources in three languages |
| `snapshots/` | SVG render baselines |
| `tools/check_*.sh` | Validation gates |
| `tests/`, `benches/` | Integration tests and Criterion benchmarks |
| `docs/plans/` | Plans, including the governing `principle.md` and `FUTURE.md` |
| `docs/issues/` | Scan reports, one file per dated pass; the newest date is the current one (no index file — list the directory and sort by name) |
| `docs/log/` | Per-day work logs with per-item status and real command output |
| `dsh-plugin/` | This DSH skill package (see below) |

## Device profiles

Five device profiles are mutually exclusive; never enable two at once. `default`
is `desktop` plus the three editor features.

| Feature | Adds | Sized? |
|---|---|---|
| `desktop` | windowing, `wgpu`, touch, i18n, charts, native+custom controls, image, fonts, icons | no |
| `tablet` | `wgpu`, touch, i18n, serde, native+custom controls, image, bidi, fonts, icons | no |
| `mobile` | `tablet`'s set plus `mobile-api` | no |
| `embedded` | `software`, `controls-custom`, `text-bidi` | yes |
| `mini` | `embedded`'s set plus `heapless`/`hashbrown`/`spin` | yes |

`serde_json` and the JSON loader exist only under `full_widgets` (the
desktop/tablet/mobile profiles); `embedded` and `mini` have neither, and
`crate::app` is not compiled there. `default-features = false` is required
whenever a single profile is selected, because the library's default set would
add `desktop` on top of it.

## Companion crates

Every crate under `crates/` is a nested package: the root package's `include`
list excludes `crates/**/*`, so a published library tarball never carries them.

| Crate | Provides | Notes |
|---|---|---|
| `rust-widgets-cli` | the `rwui` binary: `check`, `json2rust`, `fmt`, `create` | Host-side; never linked by a shipping app. Calls `rust_widgets::designer`, so generated sources cannot disagree with the in-process designer. |
| `rust-widgets-suites` | seven optional control groups behind features (`form`, `webview`, …) | Each group owns its names, schemas and constructors. |
| `rust-widgets-font` | embedded font payloads for the `fonts-*` features | Split out to stay under crates.io's 10 MiB package ceiling. |
| `rust-widgets-icons` | `IconName` plus Material Symbols outline geometry | `data` feature carries the path table; a build without it draws flattened polygons. |
| `mobile-services` (`rust-widgets-mobile-services`) | keep-awake, haptics, permissions, notifications, share, deep links | Application-shell services, not widgets; a desktop build never links it. |

All of them carry the same version as the root package. A change that crosses a
crate boundary must keep `Cargo.toml` versions identical and the workspace build
green.

## The `rwui` CLI

Build and run from the repository root:

```sh
cargo run --manifest-path crates/rust-widgets-cli/Cargo.toml -- --help
```

| Command | Purpose |
|---|---|
| `rwui check <bundle-or-json>` | Validate a document; writes nothing. `--json` prints NDJSON, `--strict` promotes style/generator gaps to errors. |
| `rwui json2rust <bundle-or-json> [-o <dir>] [--css <file>]… [--target default\|stripped] [--check]` | Generate Rust; `--check` compares against the committed files instead of writing. |
| `rwui fmt <document.json> [-o <file>]` | Stable key order and indentation, values preserved. |
| `rwui create <name> [--profile desktop\|tablet\|mobile\|embedded\|mini] [-o <parent>] [--rust-widgets-path <dir>]` | Scaffold a runnable project. |

`rwui create` writes exactly seven files: `Cargo.toml`, `README.md`,
`.gitignore`, `ui/app.json`, `ui/app.css` (same base name as the document),
`src/main.rs` (entry stub calling `app::run()`) and `src/app.rs`. The document
and stylesheet are one design unit. `desktop`/`tablet`/`mobile` load the pair at
run time; `embedded`/`mini` get a host skeleton (surface, stride, frame budget,
queued frame task) and keep `ui/` as the design source. Generated bytes are a
pure function of the arguments, and the command refuses a non-empty target.

Changing the CLI means changing `src/cli.rs` (parsing and `USAGE`),
`src/app.rs` (dispatch, exit codes, reporting) and the relevant module; `main.rs`
stays an entry stub.

## Standard workflow

1. Read `docs/plans/principle.md` and the current issue file under `docs/issues/`.
2. Locate the code with `grep`/`rg`; read the whole function before editing.
3. Make the smallest complete change; update tests, capability metadata and
   snapshots when the change is part of the requested scope.
4. Run focused checks while iterating:
   ```sh
   cargo check --lib
   cargo test --lib <filter>
   ```
5. At the end run the full validation once:
   ```sh
   cargo check --all-targets
   cargo test
   cargo clippy --all-targets -- -D warnings
   ```
6. Run only the gates whose inputs you changed:
   ```sh
   bash tools/check_source_structure.sh
   bash tools/check_setter_census.sh
   bash tools/check_capability_events_are_emitted.sh
   bash tools/check_declaration_implementation_alignment.sh
   ```
7. Record the result in `docs/log/log-<date>-<n>.md` with per-item status and
   actual command output.

Companion crates are validated separately, because the root `cargo test` does
not build them:

```sh
(cd crates/rust-widgets-cli && cargo test && cargo clippy --all-targets)
(cd crates/rust-widgets-suites && cargo test)
```

## Adding or extending a control

- Add the enum variant to `src/widget/kind.rs` (`WidgetKind`), then register the kind in
  `src/widget/capability/registration.rs` and the factory in
  `src/widget/capability/constructors.rs`.
- Publish properties in the relevant `properties_*.in.rs` file, then implement
  `get`/`set`/`property_names` in the control.
- A `pub fn set_*` that is not published as a command must be named in
  `src/widget/capability/programmatic_setters.rs` with a reason; the setter
  census fails otherwise.
- Publish events by adding them to the event census and regenerating payloads:
  ```sh
  python3 tools/derive_event_payloads.py
  ```
- Structural mutators must end with a repaint (`request_redraw` or the narrower
  frame path) so damage-driven rendering shows the change.
- Add a unit test and an SVG snapshot when the gate expects one.

## The code editor

`src/widget/special_widgets/editor/syntax/` owns the built-in lexer and the
`SyntaxHighlighter` plugin seam. The two editor controls and the lexer are
independent features (`code-editor`, `markdown-editor`, `builtin-languages`);
all three are in `default`, and a profile-only build compiles none of them.
`code_editor` re-exports the shared types, so `code_editor::syntax::*` and
`code_editor::LanguageId` remain valid paths.

- Language families are `LanguageId` variants; each must supply `name()`,
  keyword and type tables (byte-sorted, because lookup is a binary search), and
  comment/string predicates.
- Zig support is complete: `//` line comments, no block comments, `'a'`
  character literals, `\\` multiline string lines, and `@"…"` quoted identifiers.
- `keyword_tables_are_sorted` pins the sort invariant; add every new language to
  its list.
- A grammar-aware highlighter plugs in through
  `CodeEditor::register_language_highlighter`; do not fork the lexer.

## Adding a theme token

1. **Declare it.** A colour role is a field on `Colors` in
   `src/theme/types.rs` (with a serde default so an old preset still parses); a
   per-role style override is a field on `ThemeStyleToken` in the same file.
2. **Give it a value in every preset.** The built-ins are Rust functions in
   `src/theme/manager.rs` (`Theme::default()` for light, `Theme::dark()`, and the
   `macos-*` presets). A role left at its default is the appearance-blind case:
   it will not move with the theme.
3. **Regenerate the fixtures.** `bash themes/generate.sh` rewrites
   `themes/*.json` from those presets; `bash tools/check_theme_fixtures.sh`
   fails when a fixture and the library disagree.
4. **Read it somewhere.** A role with no production consumer fails
   `bash tools/check_declared_tokens_have_consumers.sh` — that gate exists
   because seven layering roles were once declared, written into every preset,
   documented, and painted by nobody.

## Other declared capabilities

The skill manifest also advertises four narrower workflows. Each is the short
version of a task you would otherwise have to re-derive from the gates:

- **platform-backend-work** — a backend is a `pub mod <family>` in
  `src/platform/mod.rs`, compiled under its own `cfg`, and every family supplies
  `platform_impl.rs`. After adding or changing one run
  `bash tools/check_platform_impl_matrix.sh`, `bash tools/check_profiles.sh` and
  `bash tools/check_target_backend_selection.sh`;
  `tools/check_platform_docs_truthfulness.sh` enforces that `docs/platform/*`
  still names real symbols rather than dead ones.
- **json-layout-loading** — the declarative path is `src/json/loader.rs`
  (construction and property writes) plus `src/json/element.rs` (registered
  named elements). It reaches controls only through the capability tables, so a
  new property must be published in `properties_*.in.rs` before a document can
  set it. `bash tools/check_json_event_route.sh` and
  `bash tools/check_declarative_path_repaints.sh` cover the two failure modes
  (an event that routes nowhere, a load that does not repaint).
- **capability-metadata** — the machine-readable surface is **generated**, not
  hand-written: `python3 tools/derive_event_payloads.py` writes
  `src/widget/capability/event_payloads.rs`, and
  `python3 tools/generate_c_header.py` writes the C ABI header. The whole
  capability document — every property, event and command per control — is
  `docs/capability_schema.json`, written by
  `cargo run --example export_capability_schema` and checked byte-for-byte by
  `bash tools/check_capability_schema_is_current.sh`, so read that file instead
  of the source tables. Re-run the generators after touching a signal or a
  property schema; each paired `check_*` gate re-derives the same table and
  fails on disagreement.
- **audit-widget-completeness** — `src/widget/census.rs` renders every kind, and
  `control.md` plus `snapshots/svg/` are its committed artefacts.
  `python3 tools/generate_control_index.py` rewrites the index and
  `cargo run --example export_control_svgs` the pictures; then run
  `bash tools/check_control_rendering.sh`,
  `bash tools/check_control_has_tests.sh` and
  `bash tools/check_svg_snapshots.sh`.

## Validation and benchmarks

```sh
cargo test --lib                     # ~11k unit tests
cargo test --test path_allocations   # allocation regression
cargo test --test designer_autosave
cargo clippy --all-targets -- -D warnings
cargo bench --bench render_bench -- --quick
```

Non-default profiles must be checked the way CI does:

```sh
cargo check --no-default-features --features mini --lib
cargo check --no-default-features --features embedded --lib
cargo check --no-default-features --features tablet --lib
cargo check --no-default-features --features "desktop,gpu-wgpu" --lib
```

`bash tools/check_profiles.sh` runs that matrix in one pass.

## This DSH plugin

`dsh-plugin/` is a Cordis plugin that registers the `ctx.skills` provider for
this skill. The loadable body's source of truth is
`.dsh/skills/rust-widgets-maintainer/SKILL.md`; `dsh-plugin/assets/` is a copy
plus the `index.json` the runtime reads without a YAML parser.

```sh
cd dsh-plugin
npm run sync-skill    # copy .dsh/skills → assets and rebuild index.json
npm run check-skill   # fail if the bundled copy drifted
npm run check-index   # fail if market-index.json drifted from the package
npm test              # node:test against the provider contract
```

- Edit the `.dsh/skills/` copy first, then run `npm run sync-skill`. Never edit
  `dsh-plugin/assets/` by hand.
- `package.json#version` must equal the root `Cargo.toml` version. The plugin
  ships with the library, so the two can never diverge.
- `market-index.json` is generated by `npm run market-index -- --git`, which
  refuses an uncommitted package and pins the newest commit that changed
  `dsh-plugin/`. Regenerate it as the last step before publishing a release.
- `dsh-plugin/lib/index.js` is the whole provider: it reads
  `assets/index.json` once and loads `assets/<name>/SKILL.md` on demand,
  stripping the frontmatter because the catalog already carries those fields.

## Reporting

End every task with:

- what changed (file paths),
- which commands actually ran and their result,
- Plan items closed and items still open,
- any assumption made where the request was ambiguous.
