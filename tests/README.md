# `tests/` — integration tests

Most of the crate's tests live **inline** next to the code they exercise
(`#[cfg(test)] mod tests` in `src/**`): roughly 11k inline tests against a few
hundred here. An inline test can name private items, which is what a control's
own arithmetic and caches need; a file in this directory is compiled as a
separate crate and can only use the public surface, so it is the right place for
a claim about the **published contract** (a property round-trip, a rendered SVG,
a gate that reads the built artefact).

## Suffixes

The suffixes are meaningful, not decorative:

| Suffix | What the file is |
|---|---|
| `*_test.rs` | An assertion suite. Almost every file here. |
| `*_tests.rs` | The same idea, older spelling. New files use `_test`. |
| `*_probe.rs` | A **measurement** with printed evidence: it renders or counts something and asserts a property of the measurement (contrast, prominence, cost), so a future reader can re-run it and read the numbers. |
| `*_allocations.rs` | A counting allocator proves that a steady-state path allocates nothing. |
| (no suffix) | Older files kept for their stable name (`snapshot_tests.rs`, `integration_test.rs`, `property_based_tests.rs`). Do not rename a test target: CI and the gate scripts address some of them by name. |

Run one target with its file stem:

```sh
cargo test --test disabled_ink_probe
```

Several probes are gated on `full_widgets` + `desktop` because the reduced
`mini` / `embedded` profiles compile the widget registry out; the gate is
documented at the top of the file rather than applied by the caller.
