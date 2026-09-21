#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"

# One budget per `cargo` step. A profile check that compiles many modules can
# take minutes cold, and a `cargo` waiting on a lock never returns on its own —
# without this, a wedged step hangs the gate with no output at all.
PROFILE_TIMEOUT=1200
TEST_TIMEOUT=900

# Runs one named test and fails if the filter matched nothing.
#
# `cargo test <filter>` exits **0** when the filter matches no test at all, so a
# stale test name turns a step into a silent no-op: `set -e` sees success, the
# script prints "All profile checks passed", and zero tests ran. That is exactly
# what happened to step [7] — it kept naming
# `platform::tests::embedded_profile_combo_list_state_event_data_roundtrip` after
# commit 45ea40b renamed the test to `embedded_profile_selection_state_roundtrip`
# (the sibling `check_behavior_matrix.sh` had its `run_case` reference updated in
# the same commit; this file's plain `cargo test` line was missed), so the
# "embedded P4c regression gate" verified nothing for that whole period.
#
# `check_behavior_matrix.sh` already refuses to pass a case that ran zero tests;
# this is the same guard, for the same reason, in the gate that lacked it.
run_test_case() {
  local title="$1"
  shift
  local output_file
  output_file="$(mktemp)"
  echo "  - running: $title"
  if ! rw_run_bounded "$TEST_TIMEOUT" "$@" >"$output_file" 2>&1; then
    cat "$output_file"
    rm -f "$output_file"
    echo "❌ $title FAILED" >&2
    return 1
  fi
  if ! grep -Eq 'running [1-9][0-9]* tests?' "$output_file"; then
    echo "❌ QA case ran zero tests: $title" >&2
    echo "   (cargo test exits 0 on a filter that matches nothing — fix the filter)" >&2
    rm -f "$output_file"
    return 1
  fi
  rm -f "$output_file"
  echo "  - ✅ $title"
}

echo "[1/9] cargo check (default)"
rw_run_bounded "$PROFILE_TIMEOUT" cargo check

echo "[2/9] cargo check --examples"
rw_run_bounded "$PROFILE_TIMEOUT" cargo check --examples

echo "[3/9] cargo check --no-default-features --features tablet --all-targets"
rw_run_bounded "$PROFILE_TIMEOUT" cargo check --no-default-features --features tablet --all-targets

echo "[4/9] cargo check --no-default-features --features mobile --all-targets"
rw_run_bounded "$PROFILE_TIMEOUT" cargo check --no-default-features --features mobile --all-targets

echo "[5/9] cargo check --no-default-features --features mini --all-targets"
rw_run_bounded "$PROFILE_TIMEOUT" cargo check --no-default-features --features mini --all-targets

echo "[6/9] cargo check --no-default-features --features embedded --all-targets"
rw_run_bounded "$PROFILE_TIMEOUT" cargo check --no-default-features --features embedded --all-targets

# The designer's build surface (BLUE19 D7-b-3).
#
# `desktop` enables the `designer` feature by default, so [1/9] already covers the
# default path. What it does **not** cover is the two configurations this feature
# creates and that nothing else would build:
#
#   * `desktop,no-declarative-view` — the designer's *default* template emits
#     `crate::view` code, so the generator must still compile when the declarative
#     layer is absent from the host. (It reports rather than emitting; the point is
#     that the module itself still builds.)
#   * `tablet,designer` — the opt-in on a profile that does not enable it, which is
#     the configuration `tools/check_designer_feature_gate.sh` proves is *reachable*.
#     A reachable configuration that does not compile is worse than an unreachable one.
#
# Without these two lines the feature gate would be the only thing that ever names
# them, and it only compiles a probe that calls `GENERATED_MARKER` — not the
# generator, and not the artifact writer.
echo "[6a/9] cargo check --features desktop,no-declarative-view --all-targets (designer without the declarative layer)"
rw_run_bounded "$PROFILE_TIMEOUT" cargo check \
  --no-default-features --features desktop,no-declarative-view --all-targets

echo "[6a/9b] cargo check --features tablet,designer --all-targets (the explicit opt-in)"
rw_run_bounded "$PROFILE_TIMEOUT" cargo check \
  --no-default-features --features tablet,designer --all-targets

# Cross-target backend checks.
#
# Why these exist: the five profile checks above all run against the **host**
# target. Everything behind `cfg(target_os = "windows")` or
# `cfg(target_os = "macos")` is therefore cfg'd out and never compiled here,
# which is how a broken `winapi::um::windef::RECT` path (the symbol lives in
# `winapi::shared::windef`) and an ungated `super::canvas` reference survived:
# the only job that compiled Windows was CI's `windows-cross-check`, and it
# passes a hand-written capability list with **no device profile**, so
# `full_widgets` was false there and the device-gated half of the Windows
# backend was never built either. The result was thousands of lines of Windows
# backend code that no job had ever compiled.
#
# `--target` is what makes these run; the target must be installed (rustup),
# so a missing target is reported rather than silently skipped -- a check that
# quietly does nothing is worse than no check (the same reasoning as
# `run_test_case` refusing a filter that matches no test).
#
# A pre-flight for the **toolchain**, not just the target.
#
# Installing the rustup target is necessary but not sufficient: building for
# `x86_64-pc-windows-msvc` needs an MSVC **linker** (`lib.exe`, and behind it
# `link.exe`). `cc-rs` does not fail fast when one is missing -- it walks its whole
# candidate list, and each candidate it rejects re-enters a full `cargo` dependency
# resolution. Measured on a host with no MSVC: this single gate consumed its entire
# 1800s budget, which is what made `tools/run_all_gates.sh` look like it had hung.
# Because the runner is serial, one gate eating the budget also delays every gate
# behind it, so the symptom was "the gate script never finishes" rather than "one
# step is slow".
#
# Detecting the toolchain here turns a 30-minute silent stall into an immediate,
# honest "unsupported host". That is a different claim from "the code is fine" and
# is reported as such by `run_all_gates.sh`'s SKIP classification (principle #59.4).
#
# The check is deliberately two-sided: `lib.exe` OR a mingw `gcc` satisfies
# cc-rs for this target, so requiring MSVC specifically would skip a gate that
# would genuinely have run.
have_windows_linker() {
  if command -v lib.exe >/dev/null 2>&1 || command -v link.exe >/dev/null 2>&1; then
    return 0
  fi
  if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    return 0
  fi
  return 1
}

if rustc --print target-list 2>/dev/null | grep -qx 'x86_64-pc-windows-msvc' \
   && rustup target list --installed 2>/dev/null | grep -qx 'x86_64-pc-windows-msvc'; then
  if ! have_windows_linker; then
    echo "[6b/9] SKIPPED: unsupported host -- the x86_64-pc-windows-msvc target"
    echo "   is installed but no MSVC linker is available (need lib.exe/link.exe,"
    echo "   or x86_64-w64-mingw32-gcc). Not a defect in the code under test:"
    echo "   nothing here can link a Windows binary. Install MSVC build tools to"
    echo "   run this step."
  else
    echo "[6b/9] cargo check --target x86_64-pc-windows-msvc (windows backend, mixed profiles)"
    for profile in desktop embedded mini; do
      echo "  - windows target, profile: $profile"
      rw_run_bounded "$PROFILE_TIMEOUT" cargo check \
        --target x86_64-pc-windows-msvc --no-default-features --features "$profile"
    done
    echo "  - windows target, no device profile and no touch capability"
    rw_run_bounded "$PROFILE_TIMEOUT" cargo check \
      --target x86_64-pc-windows-msvc --no-default-features \
      --features "windows desktop-runtime controls-native controls-custom"
  fi
else
  echo "[6b/9] SKIPPED: x86_64-pc-windows-msvc target not installed"
  echo "   install it with: rustup target add x86_64-pc-windows-msvc"
fi

# Android JNI build — the feature set `tools/build_android_testapp.sh` actually
# passes. It has no device profile, so `crate::theme` and `crate::json` are
# compiled out while `src/bindings/` is compiled in; any unconditional reference
# to either from the ABI is a hard error. That combination was broken (9 errors)
# and nothing checked it, so it is checked here.
echo "[6c/9] cargo check (android-jni feature set, no device profile)"
rw_run_bounded "$PROFILE_TIMEOUT" cargo check \
  --no-default-features \
  --features "android-jni jni mobile-api controls-custom controls-native serde serde_json"

echo "[7/9] embedded P4c regression gate"
run_test_case "embedded selection-state roundtrip" \
  cargo test --lib --no-default-features --features embedded platform::tests::embedded_profile_selection_state_roundtrip
run_test_case "embedded task queue determinism" \
  cargo test --lib --no-default-features --features embedded render_engine::embedded_engine::tests::embedded_task_queue_order_is_deterministic

echo "[8/9] declarative-layer platform gate (BLUE18 rules #92/#94)"
# Asserts `crate::view` is compiled for desktop/tablet/mobile and ABSENT for
# mini/embedded and for a build with no device profile. Run here, beside the
# profile checks it duplicates in spirit, so a `cargo check` that starts
# succeeding on a stripped profile is caught in the same place.
bash tools/check_view_platform_gate.sh

# BLUE18 rule #88: every node built by a `Node` view chain must carry a `key`,
# and keys must be unique among siblings. Run beside the view layer's other
# gate so both protections move together; it was previously only runnable by
# hand, so CI never exercised it (BLUE18 E-1).
bash tools/check_view_keys_are_unique.sh

echo "[9/9] gpu P3g parity regression gate"
run_test_case "gpu auto-compose mixed scene" \
  cargo test --lib --features gpu-wgpu render::tests::auto_compose_renders_mixed_commands_scene_with_gpu_or_cpu_backend
run_test_case "gpu auto-compose cpu fallback" \
  cargo test --lib --features gpu-wgpu render::tests::auto_compose_falls_back_to_cpu_backend_when_gpu_path_is_rejected
rw_run_bounded "$PROFILE_TIMEOUT" cargo check --features gpu-wgpu --example demo_wgpu_control_parity

# ── `mini` (no_std) × each OS backend ──
#
# `mini` is the only profile that turns on `#![no_std]` (see `build.rs`:
# `alloc_frugal`), so it is the only one that removes the standard prelude. Any
# module still naming `String`/`Vec` from the prelude — or calling
# `Mutex::lock().expect(..)`, which `spin::Mutex` does not have — compiles fine on
# `desktop`/`tablet`/`mobile` and **fails only here**.
#
# These cells were entirely uncovered, and three of them were broken at once:
# `mini,macos` (7 errors), `mini,i18n` (65 errors) and `mini,cocoa-legacy`
# (17 errors, which also re-derived a missing-`serde` bug that `macos` had already
# been patched for). Adding the combination to the gate is the root-cause fix:
# the modules could never regress silently again.
#
# The backend list is `--features <backend>` on the **host**, not a `--target`
# cross build, because the defect is in profile-gated import/lock hygiene rather
# than in backend-specific code. Cross-target coverage stays where it is above.
if [ "${RW_SKIP_MINI_MATRIX:-0}" = "1" ]; then
  echo "[9b/9] SKIPPED: mini x backend matrix (RW_SKIP_MINI_MATRIX=1)"
else
  echo "[9b/9] mini x backend compile matrix (no_std prelude hygiene)"
  # `macos`/`cocoa-legacy` only exist on an Apple host; naming them elsewhere fails
  # with "package does not have that feature enabled" style errors that are not
  # defects. Probe the host so the gate states the truth on every machine.
  MINI_BACKENDS="wasm android harmony ios windows linux-wayland"
  case "$(uname -s)" in
    Darwin) MINI_BACKENDS="$MINI_BACKENDS macos cocoa-legacy" ;;
  esac
  for backend in $MINI_BACKENDS; do
    echo "  - mini,$backend"
    rw_run_bounded "$PROFILE_TIMEOUT" cargo check \
      --no-default-features --features "mini,$backend"
  done
  # `i18n` is a capability, not a backend, but it is the largest module on the
  # `compat` bridge and `mini,i18n` was the worst of the three failures.
  echo "  - mini,i18n"
  rw_run_bounded "$PROFILE_TIMEOUT" cargo check --no-default-features --features "mini,i18n"
fi

echo "All profile checks passed."
