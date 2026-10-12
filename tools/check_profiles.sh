#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_cargo_cache.sh"

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
rw_cargo_cached "$PROFILE_TIMEOUT" check

echo "[2/9] cargo check --examples"
rw_cargo_cached "$PROFILE_TIMEOUT" check --examples

echo "[3/9] cargo check --no-default-features --features tablet --all-targets"
rw_cargo_cached "$PROFILE_TIMEOUT" check --no-default-features --features tablet --all-targets

echo "[4/9] cargo check --no-default-features --features mobile --all-targets"
rw_cargo_cached "$PROFILE_TIMEOUT" check --no-default-features --features mobile --all-targets

echo "[5/9] cargo check --no-default-features --features mini --all-targets"
rw_cargo_cached "$PROFILE_TIMEOUT" check --no-default-features --features mini --all-targets

echo "[6/9] cargo check --no-default-features --features embedded --all-targets"
rw_cargo_cached "$PROFILE_TIMEOUT" check --no-default-features --features embedded --all-targets

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
rw_cargo_cached "$PROFILE_TIMEOUT" check \
  --no-default-features --features desktop,no-declarative-view --all-targets

echo "[6a/9b] cargo check --features tablet,designer --all-targets (the explicit opt-in)"
rw_cargo_cached "$PROFILE_TIMEOUT" check \
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
#
# **The linker alone is not enough**, which is why the second half exists. A host
# with a linker but no way to compile C for the target takes the non-skip path and
# then fails inside `minimp3-sys`'s build script:
#
#     cargo:warning=minimp3/minimp3.h:46:10: fatal error: 'stdlib.h' file not found
#     error occurred in cc-rs: command did not execute successfully ...
#
# That message names a C header in a build script, so it reads as a defect in the
# code under test when in fact nothing on this host can compile for that target.
# Measured here: `x86_64-w64-mingw32-gcc` was installed, `lib.exe`/`link.exe` were
# not, `cc-rs` still chose the host `cc` with `--target=x86_64-pc-windows-msvc`,
# and clang could not find the MSVC SDK headers. A presence test on a *different*
# compiler cannot answer "will the compiler cc-rs picks work", so this asks that
# question directly: compile one file that includes a standard header, exactly the
# way cc-rs invokes it, and let the answer decide.
#
# # What actually makes this step runnable off Windows: an `xwin` splat
#
# What a non-Windows host lacks is not a linker but the **CRT and Windows SDK
# headers/libs**, and `xwin` is the tool that fetches them:
#
#     cargo install xwin
#     xwin --accept-license splat --output ~/xwin-splat
#
# With that splat present this script arms the toolchain itself (clang-cl for the
# target, `llvm-lib` to archive, `-Lnative=` to the splat's lib dirs) and the step
# runs for real. Measured on this host:
#
#   * before: `cargo check --target x86_64-pc-windows-msvc --no-default-features
#     --features desktop` → `'stdlib.h' file not found` inside `minimp3-sys`;
#   * after: the same command finishes clean, and this step covers three profiles
#     plus the no-profile feature set.
#
# The splat lives outside the repository (`$XWIN_SPLAT`, else `~/xwin-splat`), so
# the gate reads it rather than owning it; a host without one still gets an honest
# SKIP that names the command to fix it.
require_windows_linker() {
  if command -v lib.exe >/dev/null 2>&1 || command -v link.exe >/dev/null 2>&1; then
    return 0
  fi
  if command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    return 0
  fi
  return 1
}

# The `xwin` splat directory, if this host has one.
#
# Both halves are checked because a half-populated directory would arm a toolchain
# that fails later, with the failure attributed to the code under test rather than
# to the splat.
xwin_splat_dir() {
  local splat="${XWIN_SPLAT:-$HOME/xwin-splat}"
  if [ -d "$splat/crt/include" ] && [ -d "$splat/sdk/include/um" ] \
     && [ -d "$splat/sdk/lib/ucrt/x86_64" ]; then
    printf '%s' "$splat"
  fi
}

# Point cc-rs at the splat. Returns non-zero, exporting nothing, when it cannot.
#
# LLVM ships `clang-cl` and `llvm-lib`; Homebrew and most Linux distributions keep
# them off `PATH`, so the candidate directories are probed rather than assumed.
arm_windows_toolchain_from_xwin() {
  local splat
  splat="$(xwin_splat_dir)"
  if [ -z "$splat" ]; then
    return 1
  fi
  local candidate
  for candidate in /opt/homebrew/opt/llvm/bin /usr/local/opt/llvm/bin /usr/lib/llvm/bin; do
    if [ -x "$candidate/clang-cl" ]; then
      PATH="$candidate:$PATH"
      break
    fi
  done
  if ! command -v clang-cl >/dev/null 2>&1 || ! command -v llvm-lib >/dev/null 2>&1; then
    return 1
  fi
  export PATH
  # cc-rs reads both spellings; the underscored one is the canonical Rust target env
  # name, the dashed one is what `cargo` itself exports to build scripts.
  local includes="-imsvc $splat/crt/include -imsvc $splat/sdk/include/ucrt \
    -imsvc $splat/sdk/include/shared -imsvc $splat/sdk/include/um"
  export CC_x86_64_pc_windows_msvc=clang-cl
  export CXX_x86_64_pc_windows_msvc=clang-cl
  export AR_x86_64_pc_windows_msvc=llvm-lib
  export CFLAGS_x86_64_pc_windows_msvc="$includes"
  export CXXFLAGS_x86_64_pc_windows_msvc="$includes"
  export RUSTFLAGS="-Lnative=$splat/crt/lib/x86_64 -Lnative=$splat/sdk/lib/um/x86_64 \
    -Lnative=$splat/sdk/lib/ucrt/x86_64 ${RUSTFLAGS:-}"
  return 0
}

# Can this host compile C for the target?
#
# `CC_x86_64_pc_windows_msvc` overrides the compiler, so a host that sets it is
# taken at its word and probed with its own flags. Otherwise the splat is tried and,
# failing that, the host `cc` with the target flag — one translation unit that
# includes `<stdlib.h>`, which is the include that exposed the false failure.
have_c_for_windows_target() {
  local probe_dir probe compiler
  probe_dir="$(mktemp -d)"
  probe="$probe_dir/probe.c"
  printf '#include <stdlib.h>\nint main(void) { return 0; }\n' > "$probe"

  local status=1
  if [ -n "${CC_x86_64_pc_windows_msvc:-}" ]; then
    # `clang-cl` takes `/Fo`, `cc` takes `-o`; try the one that fits the compiler.
    if command -v "$CC_x86_64_pc_windows_msvc" >/dev/null 2>&1 \
       && "$CC_x86_64_pc_windows_msvc" ${CFLAGS_x86_64_pc_windows_msvc:-} /c \
          "/Fo$probe_dir/probe.obj" "$probe" >/dev/null 2>&1; then
      status=0
    fi
  elif arm_windows_toolchain_from_xwin; then
    if clang-cl ${CFLAGS_x86_64_pc_windows_msvc} /c "/Fo$probe_dir/probe.obj" "$probe" \
       >/dev/null 2>&1; then
      status=0
    fi
  else
    compiler="${CC_x86_64_pc_windows_msvc:-cc}"
    if "$compiler" --target=x86_64-pc-windows-msvc -c "$probe" -o "$probe_dir/probe.o" \
       >/dev/null 2>&1; then
      status=0
    fi
  fi
  rm -rf "$probe_dir"
  return $status
}

if rustc --print target-list 2>/dev/null | grep -qx 'x86_64-pc-windows-msvc' \
   && rustup target list --installed 2>/dev/null | grep -qx 'x86_64-pc-windows-msvc'; then
  if ! have_c_for_windows_target || ! require_windows_linker; then
    echo "[6b/9] SKIPPED: unsupported host -- the x86_64-pc-windows-msvc target"
    echo "   is installed but this host cannot compile C for it, so the Windows C"
    echo "   dependency in the tree (minimp3-sys) cannot build. Not a defect in the"
    echo "   code under test. To run this step off Windows, fetch the CRT and SDK:"
    echo "       cargo install xwin"
    echo "       xwin --accept-license splat --output ~/xwin-splat"
    echo "   (or set CC_x86_64_pc_windows_msvc to a compiler that finds the headers,"
    echo "   plus lib.exe/link.exe or a mingw gcc)."
  else
    echo "[6b/9] cargo check --target x86_64-pc-windows-msvc (windows backend, mixed profiles)"
    for profile in desktop embedded mini; do
      echo "  - windows target, profile: $profile"
      rw_cargo_cached "$PROFILE_TIMEOUT" check \
        --target x86_64-pc-windows-msvc --no-default-features --features "$profile"
    done
    echo "  - windows target, no device profile and no touch capability"
    rw_cargo_cached "$PROFILE_TIMEOUT" check \
      --target x86_64-pc-windows-msvc --no-default-features \
      --features "windows desktop-runtime controls-native controls-custom"
  fi
else
  echo "[6b/9] SKIPPED: x86_64-pc-windows-msvc target not installed"
  echo "   install it with: rustup target add x86_64-pc-windows-msvc"
fi

# Android JNI build **without a device profile** — deliberately *not* the set
# `tools/build_android_testapp.sh` passes.
#
# This step asks a narrow, historically-broken question: does the C ABI compile with no device
# profile at all? `mobile-api` is an empty marker, so naming it instead of the `mobile` profile
# produces exactly that configuration — `crate::theme` and `crate::json` compiled out while
# `src/bindings/` is compiled in, where any unconditional reference to either is a hard error.
# That combination was broken (9 errors) and nothing checked it, which is why this step exists.
#
# So the two feature sets differ **on purpose**: `check_android_cross.sh` covers the shipping
# configuration, this covers the profile-less one that must still build. Do not "align" them:
# this step is the only coverage of the profile-less ABI, and the shipping configuration is
# covered elsewhere. (The comment above used to claim this was the shipping set, which is what
# made the two easy to conflate — and conflating them in the other direction is what left the
# Android widget path dead; see `build_android_testapp.sh`.)
echo "[6c/9] cargo check (android-jni feature set, no device profile)"
rw_cargo_cached "$PROFILE_TIMEOUT" check \
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
rw_cargo_cached "$PROFILE_TIMEOUT" check --features gpu-wgpu --example demo_wgpu_control_parity

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
    rw_cargo_cached "$PROFILE_TIMEOUT" check \
      --no-default-features --features "mini,$backend"
  done
  # `i18n` is a capability, not a backend, but it is the largest module on the
  # `compat` bridge and `mini,i18n` was the worst of the three failures.
  echo "  - mini,i18n"
  rw_cargo_cached "$PROFILE_TIMEOUT" check --no-default-features --features "mini,i18n"
fi

echo "All profile checks passed."
