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
| `rwui create <name> [--profile desktop\|tablet\|mobile\|embedded\|mini] [-o <parent>] [--rust-widgets-path <dir>]` | project name | a runnable project directory | 0 / 1 / 2 |

`--json` writes one diagnostic object per line (NDJSON) to stdout. `--strict` treats style gaps
and generator gaps as errors.

## Creating a project

```sh
rwui create myapp --profile desktop
cd myapp && cargo run
```

`create` writes one directory named after the project, under `-o` (default `.`):

```text
myapp/
  Cargo.toml          the profile as one explicit, `default-features = false` feature
  README.md           how to run it and where the UI lives
  .gitignore          target/
  ui/app.json         the declarative document (the design source)
  ui/app.css          the stylesheet with the SAME base name
  src/main.rs         entry stub: `app::run()` only
  src/app.rs          App + window + UI mount
```

The `ui/` pair shares a base name on purpose: the document and its stylesheet are one design
unit, and the generated `app.rs` names both (`include_str!("../ui/app.json")` next to
`"../ui/app.css"`) so a rename of one that misses the other is a compile error rather than a
silently unstyled window.

### Profiles

| Profile | Window | UI source |
|---|---|---|
| `desktop` | 960x640 | `ui/app.json` loaded at run time; `ui/app.css` registered as `app` |
| `tablet` | 820x1180 | same |
| `mobile` | 390x844 | same |
| `embedded` | 480x320 | host skeleton; `ui/` kept as the design source |
| `mini` | 320x240 | host skeleton; `ui/` kept as the design source |

`embedded` and `mini` do not compile the JSON loader or the window app module, so their
`src/app.rs` is an **integration point**: it asks the build what it is, sets the frame budget,
allocates a surface with an explicit stride, queues one frame task and reports the engine's own
counters. The `ui/` pair is still written — hand it to `rwui json2rust` if the project later
moves to a declarative profile.

Generated files are a pure function of `(name, profile, rust-widgets-path)`: running `create`
twice with the same arguments produces identical bytes, and the command refuses a directory that
already holds anything rather than overwriting a project.

`--rust-widgets-path <dir>` writes a path dependency instead of a version dependency, which is
what a checkout of this repository needs:

```sh
rwui create myapp --profile mobile --rust-widgets-path ../rust-widgets
```

The dependency always carries `default-features = false`: the library's default set adds
`desktop` on top of any other profile, which is a mixed configuration the library forbids.

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
is told so (`RWCLI-006`) rather than generated without them. `create` is additional to that plan
and is described above. The full plan is
[`docs/plans/blue31.md`](../../docs/plans/blue31.md).
