#!/usr/bin/env bash
# ============================================================================
# check_wildcard_exports.sh — the glob-re-export surface has an owner
# ============================================================================
# Issue HX-07 recorded 89 `pub use submodule::*;` sites across 27 files and called
# them a risk to the module aggregation surface. Counting them was the easy part;
# the real question is *which* of them is a risk.
#
# A wildcard is safe when the module it forwards is a child **of the same file**:
# the author of `foo/mod.rs` declares `pub mod bar;` right there, so the forwarded
# surface is exactly the surface they own, and adding a name to `bar.rs` is the
# intended way to extend it. A wildcard is a risk when it forwards a module owned
# **elsewhere** (`pub use crate::platform::types::*;` from `contract.rs`): the
# exporting file now silently republishes another module's names, and that module
# can grow without the exporting file's author ever seeing it.
#
# The crate root is the one place where *any* wildcard is unacceptable: `pub use
# x::*` in `lib.rs` makes the whole public API of `x` part of the crate's own
# surface, so a new `pub` item becomes a semver commitment by accident.
#
# # Why this is a gate and not a one-off cleanup
#
# The scan that produced HX-07 is a snapshot. Without this script the next
# cross-module wildcard lands unnoticed, and the "89" the issue recorded becomes
# stale the way every hand-counted number in this tree has. The gate encodes the
# rule, so the surface stays owned.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" - <<'PY'
import pathlib
import re
import sys

SRC = pathlib.Path("src")
WILDCARD = re.compile(r"^\s*pub use\s+([A-Za-z0-9_:]+)::\*;")
DECLARED_MOD = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z0-9_]+)\s*;", re.M)


def module_path(rel: pathlib.Path) -> list[str]:
    """`src/a/b.rs` -> ['a','b']; `src/a/b/mod.rs` -> ['a','b']."""
    parts = list(rel.with_suffix("").parts)
    if parts and parts[0] == "src":
        parts = parts[1:]
    if parts and parts[-1] == "mod":
        parts = parts[:-1]
    return parts


errors: list[str] = []
sites = 0
files = 0

for path in sorted(SRC.rglob("*.rs")):
    text = path.read_text(encoding="utf-8")
    matches = [
        (lineno, m.group(1))
        for lineno, line in enumerate(text.splitlines(), 1)
        for m in [WILDCARD.match(line)]
        if m
    ]
    if not matches:
        continue
    files += 1
    own = module_path(path.relative_to("."))
    declared = set(DECLARED_MOD.findall(text))

    for lineno, target in matches:
        sites += 1
        location = f"{path}:{lineno}"

        # The crate root may not forward a wildcard at all.
        if path == pathlib.Path("src/lib.rs"):
            errors.append(
                f"{location} forwards `{target}::*` from the crate root; every public "
                "item of that module would become part of this crate's API by accident"
            )
            continue

        last = target.rsplit("::", 1)[-1]
        if last in declared:
            continue

        # A fully-qualified path to the file's *own* child module is the same thing
        # written out; accept it (`platform::windows::types::*` inside `windows/mod.rs`).
        stripped = target[6:] if target.startswith("crate::") else target
        if stripped == "::".join([*own, last]):
            continue

        errors.append(
            f"{location} forwards `{target}::*`, but `{last}` is not a module declared "
            f"in this file. Declare it here or re-export the specific names "
            f"(`pub use {target}::{{A, B}};`) so this file's surface stays its own."
        )

if errors:
    for error in errors:
        print(f"❌ {error}", file=sys.stderr)
    print(f"\ncheck_wildcard_exports: FAILED ({len(errors)} unowned wildcard(s))", file=sys.stderr)
    sys.exit(1)

if sites == 0:
    print("check_wildcard_exports: FAILED (the scan found no wildcards at all)", file=sys.stderr)
    sys.exit(1)

print(
    f"check_wildcard_exports: {sites} wildcard(s) in {files} file(s), "
    "all forwarding a module the file itself declares"
)
PY

echo "✅ check_wildcard_exports: every glob re-export forwards the file's own child module"
