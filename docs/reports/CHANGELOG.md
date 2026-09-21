# Changelog

The canonical project changelog is maintained at [docs/reports/CHANGELOG.md](docs/reports/CHANGELOG.md).

This root-level file exists for tools and release automation that expect `CHANGELOG.md` at repository root.
When the two disagree, this file is the one that ships; `tools/check_changelog_sync.sh` keeps them identical.

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
