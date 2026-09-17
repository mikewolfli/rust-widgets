# `tools/`

Scripts that verify or generate the repository's artifacts. Nothing here is part of
the shipped library.

## `check_*.sh` / `check_*.py` — the gates

Each `check_*.sh` is one gate with a single PASS/FAIL verdict; the paired `check_*.py`
holds the logic when the check needs real parsing. `tools/lib_python.sh` and
`tools/lib_check_locking.py` are shared helpers, not gates.

Run them all:

```bash
for s in tools/check_*.sh; do
  n=$(basename "$s" .sh)
  if timeout 400 bash "$s" >/tmp/$n.out 2>&1; then echo "PASS $n"; else echo "FAIL $n"; fi
done
```

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
| `gtk_check.py`, `gtk_property_check.py` | Inspect a live GTK widget tree (needs a display). |
| `missing_docs_report.py` | Reports public items lacking docs; a report, not a gate. |
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
