#!/usr/bin/env bash
# The iOS build surface must type-check, as far as this host can take it (BLUE19 T-12).
#
# # What this can and cannot do, measured rather than assumed
#
# `blue19.md` recorded T-12 as "本机能力外" on the reasoning that iOS verification needs Xcode. That is
# true for **running** the app and for anything that **links**, but it is not true for type-checking:
#
#   $ cargo check --target aarch64-apple-ios --no-default-features --features "mobile,ios"
#   Finished `dev` profile … in 29.81s          ← succeeds on Linux
#
# So this gate does what is possible here, and states the boundary rather than claiming more:
#
#   * **possible**: `cargo check` of the lib for `aarch64-apple-ios` and `aarch64-apple-ios-sim`,
#     which compiles every `cfg(target_os = "ios")` module and every `objc2`/`objc2-ui-kit` path.
#     That is what catches a wrong selector name, a missing feature gate, or a type error in the
#     iOS backend — the same class the Android gate found.
#   * **not possible**: `--all-targets`, because a transitive build script (`alloca`, via `objc2`)
#     compiles C for the target and needs a cross `CC` for iOS. Verified: the failure is in
#     `alloca`'s build script, with **zero** errors in `rust_widgets`' own code.
#   * **not possible**: running the app, which needs a macOS host with a simulator.
#
# # Why `--lib` and not `--all-targets`
#
# Because `--all-targets` fails for a reason unrelated to this crate, and a gate that reports a
# dependency's host limitation as a defect would be noise that trains people to ignore it. The
# limitation is recorded here instead, so the next reader does not have to rediscover it.
#
# Usage: tools/check_ios_cross.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"

GATE_TIMEOUT="${GATE_TIMEOUT:-900}"

# Both iOS targets: the device ABI and the simulator slice. A `#[cfg(target_os = "ios")]` module is
# compiled for both, but the two differ in `target_abi`/`target_arch`, which is enough to hide a
# break in one — so both are checked.
TARGETS=(
  aarch64-apple-ios
  aarch64-apple-ios-sim
)

# The feature set the iOS build uses. `mobile` supplies the device profile; `ios` selects the
# Apple-mobile backend.
FEATURES="mobile,ios"

MISSING=()
for target in "${TARGETS[@]}"; do
  if ! rustup target list --installed 2>/dev/null | grep -qx "$target"; then
    MISSING+=("$target")
  fi
done
if [[ "${#MISSING[@]}" -gt 0 ]]; then
  echo "FAIL: these iOS targets are not installed, so this gate cannot check them:" >&2
  for target in "${MISSING[@]}"; do
    echo "  - $target" >&2
  done
  echo "" >&2
  echo "Install them with:" >&2
  echo "  rustup target add ${MISSING[*]}" >&2
  exit 1
fi

echo "=== iOS targets: ${#TARGETS[@]} target(s), lib type-check only ==="
for target in "${TARGETS[@]}"; do
  echo "--- $target"
  if ! rw_run_bounded "$GATE_TIMEOUT" cargo check --lib \
      --target "$target" --no-default-features --features "$FEATURES"; then
    echo "" >&2
    echo "FAIL: \`$target\` does not type-check with the iOS feature set." >&2
    echo "      This compiles every \`cfg(target_os = \"ios\")\` module and the objc2 paths, so a" >&2
    echo "      wrong selector, a missing gate, or a type error in the iOS backend lands here." >&2
    exit 1
  fi
done

echo ""
echo "  note: running the app and anything that links still needs a macOS host with a"
echo "        simulator; \`--all-targets\` additionally needs a cross \`CC\` for iOS because"
echo "        \`alloca\` (via \`objc2\`) compiles C for the target. Neither is a defect here."
echo ""
echo "ios cross-target checks passed."
