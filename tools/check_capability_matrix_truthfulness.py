#!/usr/bin/env python3
"""check_capability_matrix_truthfulness.py — P1 capability-matrix honesty gate.

Cross-checks the generated `docs/plans/platform_capability_matrix.md` against
the per-OS `platform_impl` implementation-grade scanner:

  - A desktop cell marked ✅ (usable path) is a contradiction when the mapped
    `create_*` method is Missing or Placeholder in the corresponding OS
    `platform_impl` (i.e. the doc claims a usable path that does not exist).

StateBacked is allowed under ✅ (the matrix legend defines ✅ as "usable path,
either native or state/self-drawn"). 🔶 / ⬜ / ➖ cells are never contradicted.

Exit code is non-zero when any contradiction is found.
"""

from __future__ import annotations

import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import platform_impl_scan as scan  # noqa: E402

MATRIX_FILE = pathlib.Path("docs/plans/platform_capability_matrix.md")
USABLE = "✅"
ROW_RE = re.compile(r"^\|\s*\*\*([A-Za-z][A-Za-z0-9_]*)\*\*\s*\|(.+)$")
FN_RE = re.compile(r"\bfn\s+(create_[a-z0-9_]+)\s*\(")
DELEGATE_RE = re.compile(r"get_platform\(\)\.(create_[a-z0-9_]+)\s*\(")
NATIVE_RS = pathlib.Path("src/control_backend/native.rs")


def extract_method_bodies(source: str) -> dict:
    methods = {}
    i = 0
    while i < len(source):
        m = FN_RE.search(source, i)
        if not m:
            break
        method = m.group(1)
        open_idx = source.find("{", m.end())
        if open_idx < 0:
            i = m.end()
            continue
        brace_count = 1
        j = open_idx + 1
        while j < len(source) and brace_count > 0:
            if source[j] == "{":
                brace_count += 1
            elif source[j] == "}":
                brace_count -= 1
            j += 1
        methods[method] = source[m.start():j]
        i = j
    return methods


def native_delegates() -> dict:
    """method -> platform create_* target (native.rs fallback resolution)."""
    if not NATIVE_RS.exists():
        return {}
    text = NATIVE_RS.read_text(encoding="utf-8")
    delegates = {}
    for method, body in extract_method_bodies(text).items():
        m = DELEGATE_RE.search(body)
        if m:
            delegates[method] = m.group(1)
    return delegates

# Doc platform column -> scanner OS backend keys (a cell is satisfied when ANY
# listed backend provides a usable create path).
COLUMN_BACKENDS = {
    "Windows": ["windows"],
    "Linux/X11": ["linux"],
    "macOS": ["macos", "macos_objc2"],
    "Wayland": ["wayland"],
}


def main() -> int:
    if not MATRIX_FILE.exists():
        print(f"error: matrix file not found: {MATRIX_FILE}", file=sys.stderr)
        return 2

    canonical = scan.parse_canonical_methods()
    backends = {b.key: b for b in (scan.scan_os(k, f, canonical) for k, f in scan.OS_SOURCES.items())}
    delegates = native_delegates()

    lines = MATRIX_FILE.read_text(encoding="utf-8").splitlines()
    # Locate header row for column order.
    header_idx = None
    for idx, line in enumerate(lines):
        if line.startswith("| Widget |"):
            header_idx = idx
            break
    if header_idx is None:
        print("error: matrix header row not found", file=sys.stderr)
        return 2

    columns = [c.strip() for c in lines[header_idx].strip().strip("|").split("|")]
    columns = columns[1:]  # drop "Widget"

    contradictions = 0
    checked = 0
    for line in lines[header_idx + 1 :]:
        m = ROW_RE.match(line)
        if not m:
            continue
        kind = m.group(1)
        cells = [c.strip() for c in m.group(2).split("|")]
        if len(cells) < len(columns):
            continue
        method = scan.kind_to_method(kind)
        # Resolve the actual platform create path through native.rs fallbacks so
        # a degraded-but-usable primitive (e.g. Action -> create_button) counts
        # as usable rather than Missing.
        platform_method = delegates.get(method, method)
        for col, backend_keys in COLUMN_BACKENDS.items():
            try:
                col_idx = columns.index(col)
            except ValueError:
                continue
            if cells[col_idx] != USABLE:
                continue
            checked += 1
            grades = []
            for key in backend_keys:
                b = backends.get(key)
                grades.append(
                    b.methods.get(platform_method, scan.MISSING) if b else scan.MISSING
                )
            usable = any(g in (scan.NATIVE, scan.STATE_BACKED) for g in grades)
            if not usable:
                contradictions += 1
                print(
                    f"❌ {kind} on {col}: doc=✅ but {method} (via {platform_method}) "
                    f"grade is {'/'.join(grades)} in {backend_keys}"
                )

    print(f"capability-matrix truthfulness: checked {checked} ✅ desktop cells, "
          f"{contradictions} contradiction(s)")
    return 1 if contradictions else 0


if __name__ == "__main__":
    sys.exit(main())
