---
name: rust-widgets-maintainer
description: Maintain, extend and audit the rust-widgets Rust GUI library. Use when changing controls, layouts, rendering, platform backends, the code editor, capability metadata, JSON layout loading, or running the repository's validation gates.
whenToUse: Load when working inside the rust-widgets repository on a coding, review, or validation task.
metadata:
  license: MIT
  project: rust-widgets
---

# rust-widgets maintainer

`rust-widgets` is a dependency-light Rust GUI library with around 242 controls,
several platform backends, a JSON layout loader, a designer, and a source-code
editor. Work is governed by `docs/plans/principle.md`; read it before touching
code.

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
| `src/data_binding/` | Model ↔ property bindings |
| `src/widget/capability/` | Property/event schemas, factory registration |
| `tools/check_*.sh` | Validation gates |
| `tests/`, `benches/` | Integration tests and Criterion benchmarks |
| `docs/plans/principle.md` | The governing engineering rules |

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

## Adding or extending a control

- Register the kind in `src/widget/capability/registration.rs` and the factory in
  `src/widget/capability/constructors.rs`.
- Publish properties in the relevant `properties_*.in.rs` file, then implement
  `get`/`set`/`property_names` in the control.
- Publish events by adding them to the event census and regenerating payloads:
  ```sh
  python3 tools/derive_event_payloads.py
  ```
- Structural mutators must end with a repaint (`request_redraw` or the narrower
  frame path) so damage-driven rendering shows the change.
- Add a unit test and an SVG snapshot when the gate expects one.

## The code editor

`src/widget/special_widgets/editor/syntax/` owns the built-in lexer and the
`SyntaxHighlighter` plugin seam. The two editor controls and the lexer are independent features
(`code-editor`, `markdown-editor`, `builtin-languages`); all three are in `default`, and a
profile-only build compiles none of them. `code_editor` re-exports the shared types, so
`code_editor::syntax::*` and `code_editor::LanguageId` remain valid paths.

- Language families are `LanguageId` variants; each must supply `name()`,
  keyword and type tables (byte-sorted, because lookup is a binary search), and
  comment/string predicates.
- Zig support is complete: `//` line comments, no block comments, `'a'`
  character literals, `\\` multiline string lines, and `@"…"` quoted identifiers.
- `keyword_tables_are_sorted` pins the sort invariant; add every new language to
  its list.
- A grammar-aware highlighter plugs in through
  `CodeEditor::register_language_highlighter`; do not fork the lexer.

## Validation and benchmarks

```sh
cargo test --lib                     # ~9.7k unit tests
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

## Reporting

End every task with:

- what changed (file paths),
- which commands actually ran and their result,
- Plan items closed and items still open,
- any assumption made where the request was ambiguous.
