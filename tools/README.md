# `tools/`

Scripts that verify or generate the repository's artifacts. Nothing here is part of
the shipped library.

## `check_*.sh` / `check_*.py` — the gates

Each `check_*.sh` is one gate with a single PASS/FAIL verdict; the paired `check_*.py`
holds the logic when the check needs real parsing. `tools/lib_python.sh` and
`tools/lib_check_locking.py` are shared helpers, not gates.

Run them all:

```bash
bash tools/run_all_gates.sh                # every gate, one PASS/FAIL/NOT-RUN line each
bash tools/run_all_gates.sh --summary      # add the per-gate output
bash tools/run_all_gates.sh --summary --filter check_abi.sh   # one gate
```

`run_all_gates.sh` is the **single** maintenance entry point. It bounds each gate with
`tools/lib_timeout.sh`'s `rw_run_bounded` and stops when the whole-run budget is spent,
reporting the gates it did not reach as `NOT-RUN`.

**Do not wrap the runner in an outer `timeout`** and do not hand-roll a
`for s in tools/check_*.sh; do timeout 400 bash "$s"; done` loop. Both were recommended
here once; both are wrong. The tools' own `rw_run_bounded` already sets an internal bound,
so an outer `timeout` stacks a second timer on top and, as measured in round 52, makes
**every** gate report failure (PASS:0/FAIL:30) while each script run alone exits 0. The
hand-rolled loop also has no `rw_run_bounded` on a host without GNU `timeout`, where it
simply reports `timeout: command not found`. (D08-C-01.)

### What the runner's exit status means

The runner exits non-zero for **any** of:

* one or more gates `FAIL`;
* one or more gates `TIMEOUT` (an inconclusive result must not read as success);
* one or more gates `NOT-RUN` because the whole-run budget was exhausted (that is
  verification which did not happen);
* a `--filter` that matched **no** gate (an empty selection verified nothing).

`exit 0` therefore means "every selected gate ran and passed", not merely "nothing
failed". `SKIP` (a gate that reports `unsupported host`) is the one non-pass that does not
fail the run, because the gate itself ran and answered honestly. (D08-G-01.)

`check_apple_native.sh` reports "unsupported host" and fails on any non-macOS host by
design: it verifies AppKit/UIKit entry points, which cannot be checked elsewhere. Every
other gate must pass on Linux.

**Adding a gate.** A gate that can only report PASS is worse than no gate, because it
looks like a defence. Prove a new gate fails before trusting it: break the thing it
checks, watch it fail, restore. `tools/check_control_has_tests.py` and
`tools/check_event_producers.py` each documented two false-passing versions in their
docstrings for exactly this reason.

## `generate_*.py` — generators for published artifacts

`include/rw_generated.h`, `include/rw_errors.h`, the capability/route matrices and the
feature-completeness matrix are **generated**, never hand-edited. `tools/check_abi.sh`
regenerates them and `cmp`s the result against the published copies, so a hand edit to a
generated file fails the gate rather than shipping.

Regenerate after changing an exported signature:

```bash
python3 tools/generate_c_header.py
bash tools/check_abi.sh
```

The C ABI function count appears in `README.md` and `README.zh-CN.md`; `check_abi.sh`
compares all three and names the file to update when they disagree.

## Other scripts

| Script | Purpose |
|---|---|
| `add_spdx_headers.py` | Adds the SPDX header to source files that lack one. |
| `build_android_testapp.sh`, `run_android_testapp.sh` | Builds and runs the Android test app. |
| `build_ios_testapp.sh`, `run_ios_testapp.sh` | Builds and runs the iOS Simulator app. |
| `run_wayland_compositor_tests.sh` | Runs the Wayland tests against a real compositor. |
| `smoke_demos.sh` | Smoke-runs every demo. |
| `gtk_source_snapshot.py` | Writes the Linux/GTK canvas source, with crate-internal imports and platform-dependent bodies replaced, into a scratch directory for **offline reading**. This is a *source snapshot generator, not a compile check* — the real GTK compile check is the `linux-gtk` CI job (`cargo check --features desktop,gtk-native` on a host with GTK 3). Renamed from `gtk_check.py`, whose name and README line overstated it (D08-G-04). |
| `missing_docs_report.py` | Reports public items lacking docs; a report, not a gate. |
| `audit_text_contrast.py` | Reads the committed `snapshots/svg/` files and reports the WCAG contrast ratio of every `<text>` against the element painted under it. **Deliberately not a gate**: a disabled label and a watermark are *supposed* to be faint, so a low ratio is not by itself a defect. It is the evidence generator that says which of 188 controls deserve a look. |
| `audit_appearance.py`, `audit_theme_tokens.py`, `audit_kind_sharing.py`, `audit_control_gaps.py`, `audit_platform_create_coverage.py`, `audit_text_y.py` | Audits, not gates — they quantify a class of defect so the fixes can be prioritised. `audit_platform_create_coverage.sh` (the gate) is what asserts `UNRESOLVED (0)`. |
| `platform_impl_scan.py` | Feeds the platform implementation matrix. |
| `rename_prefix.py` | Renames the crate prefix across bindings. |
| `verify_window_pixels.py` | Reads back pixels from a live window. |
| `test_check_locking.py` | Unit tests for `lib_check_locking.py`. |
| `widget_gallery.rs` | Generates the SVG widget gallery (`cargo run --example widget_gallery`). |
| `win32_accel_probe/` | Host-side test harness for the Windows accelerator parser, whose logic is platform-independent but whose crate is Windows-gated. |
| `feature_completeness_allowlist.toml` | Data for `check_feature_completeness_matrix.sh`. |

## What is deliberately not here

The one-off migration scripts that split large modules (`split_*.py`) and the batch source
rewrites (`fix_*.sh`, `fix_*.py`) were deleted once their targets no longer existed. They
were not reusable tooling: each carried hardcoded paths and `sed -i`/`write` calls that
would silently rewrite current source if re-run, and several had already been superseded
by a `_v2`/`_v3` successor. Keeping them was a hazard rather than a reference — the split
history lives in `docs/` and `docs/log/`, which is where it belongs.
