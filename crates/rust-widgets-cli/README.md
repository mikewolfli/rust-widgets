# rust-widgets-cli — `rwui`

The designer's command-line tool. It is a **host-side** package beside the library (like
`rust-widgets-suites`): a shipping application never links it.

`rwui` calls the library's own generator (`rust_widgets::designer`), so the sources it writes
cannot disagree with what a designer produces in-process. It does not render anything: there is
no window and no render context.

## Build and run

```sh
cargo build --manifest-path crates/rust-widgets-cli/Cargo.toml
cargo run   --manifest-path crates/rust-widgets-cli/Cargo.toml -- --help
```

## Commands

| Command | Input | Output | Exit |
|---|---|---|---|
| `rwui check <bundle-or-json>` | document (+ optional CSS) | diagnostics; writes nothing | 0 no error / 1 error / 2 could not run |
| `rwui json2rust <bundle-or-json> [-o <dir>] [--css <file>]… [--target default\|stripped] [--check]` | document | `src/generated/ui_default.rs` and/or `ui_stripped.rs`; `--check` compares instead of writing | 0 / 1 / 2 |
| `rwui fmt <document.json> [-o <file>]` | document | stable key order and indentation, same values | 0 / 1 / 2 |

`--json` writes one diagnostic object per line (NDJSON) to stdout. `--strict` treats style gaps
and generator gaps as errors.

## Bundles

A bundle is a directory:

```text
my-ui.rwui/
  ui.json          # the document (or the name given by rwui.json)
  theme.css        # stylesheet (optional; picked up by convention)
  rwui.json        # manifest (optional, JSON)
  assets/          # reserved for images/fonts; not consumed yet
```

`rwui.json` fields: `version` (currently `1`), `document`, `stylesheets`, `targets`
(`default`/`stripped`), `width`, `height`, `function_name`, `output_dir`, `strict`.

A bare `.json` document also works; its parent directory becomes the bundle root.

## Diagnostics

Findings carry stable codes so CI can select on them instead of on prose:
`RWCLI-001` JSON syntax · `RWCLI-002` document contract · `RWCLI-006` style gap ·
`RWCLI-007` generator-unsupported node · `RWCLI-008` capacity overflow ·
`RWCLI-011` committed artifact drift · `RWCLI-012` I/O failure.

## Current stage

BLUE31 stage P0 and the document half of P1 are implemented: parsing, size resolution,
generation for both targets, drift comparison and diagnostics. Static CSS resolution, `json2css`,
reverse parsing (`rust2json`) and `roundtrip` are later stages; a bundle that ships stylesheets
is told so (`RWCLI-006`) rather than generated without them. The full plan is
[`docs/plans/blue31.md`](../../docs/plans/blue31.md).
