# Shared brief for audit-slice work

Read your slice file (the one named in your task) and the current source before editing.

## Rules (mandatory)

1. Work only inside the file set assigned to you. Never edit `src/widget/capability/**`,
   `Cargo.toml`, `.github/**`, or another slice's files. The main thread owns the
   capability schema; report the rows you need instead of adding them.
2. For every slice row: read the cited code first. If the gap is already closed, do not
   invent work — record the evidence. If it is open, implement it fully.
3. No placeholders: no `todo!()`, `unimplemented!()`, empty bodies. Every `unsafe` block
   needs a `// SAFETY:` comment stating the precondition.
4. Comments and docs in English only. Never mention another GUI toolkit by name in code,
   comments, or docs (use neutral wording such as "the reference toolkit" if a comparison is
   unavoidable).
5. Every public item gets a doc comment. New behaviour gets one unit test in the file's
   existing `#[cfg(test)] mod tests`.
6. Setters early-return when the value is unchanged; setters that affect pixels call
   `request_redraw()`.
7. Preserve existing behaviour by default: new visual features are opt-in (default off)
   so the 213 SVG snapshots stay byte-identical.
8. Do not run `cargo fmt --all`. You may run `rustfmt` on files you changed.
9. You may run `cargo check --lib` and targeted `cargo test --lib <filter>`; a build lock
   may make them wait. Do not run the full test suite.
10. Do not revert changes made by others; the worktree is shared.

## Capability schema requests

When a new property needs a schema row, append to your report file a line in this exact
shape (one per property):

```
SCHEMA | <canonical_widget_name> | <property> | <UInt|Int|Float|Bool|String|Color|NumberList|Records|...> | <readable true/false> | <writable true/false> | <default expression or -> for null>
```

## Report

Write `docs/issues/slices/<your-group>-report.md` containing:

- `CLOSED: R001, R002, ...` — rows fully implemented or verified as already satisfied.
- `OPEN: R0xx — <one-line reason>` — anything left, with a concrete next step.
- The `SCHEMA | ...` lines described above.
- A short note per closed row (one line each) with the file and the API/test that proves it.

Your final chat message must be a single line: `<group>: closed=N open=M tests=<targeted test results>`.
