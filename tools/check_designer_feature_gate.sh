#!/usr/bin/env bash
# The designer tooling must be present on `desktop` and **absent** everywhere else by default
# (BLUE19 D7-b-3).
#
# # Why this gate exists
#
# `designer` is a **development-time** capability: a code generator plus an artifact writer that
# writes Rust source into the tree. `desktop` enables it, because `desktop` is the profile a designer
# **host** runs on. Every other profile leaves it off, because those are the **targets** of a
# generation (BLUE19 §5.3.4) — and linking a generator and `std::fs::write` into a shipping
# application is exactly the weight mode 2 exists to remove.
#
# The failure this guards against is quiet in both directions:
#
#   * **Leaking in.** Someone adds `"designer"` to `tablet` (perhaps by copying `desktop`), and every
#     tablet application now ships a code generator. Nothing breaks; the binary just grows and the
#     delivery artifact contains a development tool.
#   * **Disappearing from desktop.** The feature is dropped from the default set, and the designer —
#     the whole consumer of this module — silently cannot be built by its own host profile.
#
# A source grep for `"designer"` in `Cargo.toml` would prove neither: the question is what the
# **compiler** sees, so the check compiles a probe that *names* the module. The probe resolving, or
# failing to, **is** the answer — the same technique `tools/check_view_platform_gate.sh` uses for
# `crate::view`, for the same reason.
#
# # What it asserts
#
#   [1] `desktop` resolves `rust_widgets::designer`
#   [2] every other device profile does **not**
#   [3] an explicit `--features <profile>,designer` does resolve it, on a profile that is off by
#       default — so the boundary is a default, not a hard prohibition
#
# Usage: tools/check_designer_feature_gate.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_cargo_cache.sh"

GATE_TIMEOUT="${GATE_TIMEOUT:-900}"

# Profiles other than `desktop`, which must not have the designer by default.
STRIPPED_FROM_DESIGNER=(tablet mobile mini embedded)

# A throwaway crate whose only job is to *name* the gated module. `use` is enough: the question is
# whether the path resolves, not whether the API works (the API has its own tests).
#
# `mktemp -d` yields a **host-native** path, which on Windows contains backslashes; a
# `--manifest-path` argument is a plain filesystem path rather than TOML, so cargo accepts it
# either way, but the derived `-dependencies` path below is TOML and must not carry them.
PROBE_DIR="$(mktemp -d)"
trap 'rm -rf "$PROBE_DIR"' EXIT
mkdir -p "$PROBE_DIR/src"
printf 'pub fn probe() -> &%sstr { rust_widgets::designer::GENERATED_MARKER }\n' "'static " \
  > "$PROBE_DIR/src/lib.rs"

# `CARGO_TARGET_DIR` is shared with the workspace so the library's rlib is reused across the five
# probes instead of being rebuilt per profile. The build-dir lock the outer `cargo` holds is released
# before this gate runs, so the reuse is safe; the whole gate is bounded above either way.
probe() {
  local features="$1"
  # The probe manifest is TOML, where `\` starts an escape, so a native Windows path
  # (`D:\Workspace\...`) written verbatim makes it unparsable and every probe fails — a
  # false failure about the host rather than a finding about the feature gate.
  #
  # The conversion is `cygpath -m` ("mixed": a Windows path with forward slashes, exactly
  # what TOML needs) where it exists. It does **not** fall back to a plain `\` → `/`
  # substitution, because under git-bash `$PWD` is the MSYS form `/d/Workspace/...` and cargo
  # reads that as a *drive-relative* path, resolving it to `C:\d\Workspace\...` — an error
  # that looks like a missing crate rather than a bad path. Where `cygpath` is absent the path
  # is already POSIX, so it is used as-is.
  local manifest_path
  if command -v cygpath >/dev/null 2>&1; then
    manifest_path="$(cygpath -m "$ROOT_DIR")"
  else
    manifest_path="$ROOT_DIR"
  fi
  cat > "$PROBE_DIR/Cargo.toml" <<EOF
[package]
name = "rw_designer_gate_probe"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
rust_widgets = { path = "$manifest_path", default-features = false, features = [$features] }
EOF
  CARGO_TARGET_DIR="$ROOT_DIR/target" \
    rw_cargo_cached "$GATE_TIMEOUT" check --quiet --manifest-path "$PROBE_DIR/Cargo.toml" \
    > /dev/null 2>&1
}

echo "=== [1/3] \`desktop\` must resolve the designer module ==="
if ! probe '"desktop"'; then
  echo "FAIL: \`--features desktop\` does not resolve \`rust_widgets::designer\`." >&2
  echo "      The designer host profile is the one profile that must have it. If the" >&2
  echo "      \`designer\` feature was removed from \`desktop\` in Cargo.toml, put it back." >&2
  exit 1
fi
echo "  resolved"

echo ""
echo "=== [2/3] every other profile must NOT resolve it ==="
LEAKED=0
for profile in "${STRIPPED_FROM_DESIGNER[@]}"; do
  if probe "\"$profile\""; then
    echo "  LEAK: \`$profile\` resolves \`rust_widgets::designer\`" >&2
    LEAKED=1
  else
    echo "  absent on $profile (correct)"
  fi
done
if [[ "$LEAKED" -ne 0 ]]; then
  echo "" >&2
  echo "FAIL: the designer tooling leaks into a delivery profile." >&2
  echo "      A code generator and \`std::fs::write\` have no place in a shipping" >&2
  echo "      application; the feature belongs on \`desktop\` only." >&2
  exit 1
fi

echo ""
echo "=== [3/3] an explicit opt-in must work on a profile that is off by default ==="
# `tablet,designer` — the boundary is a *default*, not a prohibition. A caller who wants the
# generator on a tablet build must be able to ask, or the feature is a hard split and not a gate.
if ! probe '"tablet", "designer"'; then
  echo "FAIL: \`--features tablet,designer\` does not resolve \`rust_widgets::designer\`." >&2
  echo "      The gate is meant to be a default, so the explicit opt-in must work. Check" >&2
  echo "      the \`designer_tooling\` alias in build.rs — it ANDs the feature with a device" >&2
  echo "      profile, so a missing \`serde_json\` or a stripped profile would block it." >&2
  exit 1
fi
echo "  \`tablet,designer\` resolves (the opt-in is real)"

echo ""
echo "designer feature gate checks passed."
