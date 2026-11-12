#!/usr/bin/env bash
# ============================================================================
# check_widget_kind_count.sh — documentation/code count consistency gate
# ============================================================================
# Hand-written widget counts in the docs drifted before: `codemap.md` claimed
# "166 variants" while `WidgetKind` actually had 167. That is exactly the
# "documentation deception" failure mode the project rules forbid (a doc that
# mirrors a code fact must be mechanically derived, or checked).
#
# This gate extracts the real count from `src/widget/kind.rs` — the single
# source of truth — and fails when a document states a different number.
#
# # Why the file list is explicit
#
# An earlier revision of this gate accepted the files to scan as its arguments
# and was invoked with none, so it scanned nothing and always passed. A gate
# that passes vacuously is worse than no gate: it advertises coverage that does
# not exist. The list below is therefore hard-coded, and the script fails if it
# resolves to zero files.
#
# # Why the pattern set is broad (BLUE/#2 iceberg)
#
# The first version only matched `"167 widget kinds"` and `"167 种控件"`. That
# left several real spellings unchecked, and they drifted: `"175 kinds"`,
# `"175 种"`, `"175 WidgetKind variants"` and the profile table's `"175 (full)"`
# all stayed stale while the enum grew to 179. A gate whose pattern is narrower
# than the prose it guards turns a missed edit into a silent lie. The matching
# below accepts every form the docs actually use, in all three languages, and
# tolerates line wrapping (the number and its noun may be on different lines).
#
# # What counts as a "total" claim (and what does not)
#
# Only 3-digit numbers are treated as widget-kind totals. A subset claim such as
# `"28 are available under all profiles, and 151 are unlocked with non-mini"`
# uses 2- and 3-digit numbers but never in one of the guarded forms, so it is not
# mistaken for the total.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

KIND_FILE="src/widget/kind.rs"
if [[ ! -f "$KIND_FILE" ]]; then
  echo "missing $KIND_FILE" >&2
  exit 2
fi

ACTUAL="$("$PYTHON" -c '
import re, sys
src = open("src/widget/kind.rs", encoding="utf-8").read()
m = re.search(r"pub enum WidgetKind \{(.*?)\n\}", src, re.S)
if not m:
    print("could not locate the WidgetKind enum", file=sys.stderr)
    sys.exit(2)
body = re.sub(r"//[^\n]*", "", m.group(1))
body = re.sub(r"/\*.*?\*/", "", body, flags=re.S)
variants = re.findall(r"^\s*([A-Z][A-Za-z0-9_]*)\s*(?:\([^)]*\))?\s*,", body, re.M)
print(len(variants))
')"

echo "WidgetKind variants (parsed from $KIND_FILE): $ACTUAL"

# Scans the documents below for widget-kind total claims and reports any that
# disagree with ACTUAL. Exit code 1 when at least one mismatched.
"$PYTHON" - "$ACTUAL" <<'PY'
import re
import sys
from pathlib import Path

actual = sys.argv[1]

# Every document that states the widget-kind total. Kept explicit rather than
# globbed so a renamed chapter cannot silently drop out of coverage; the
# cookbook chapters are listed for all three languages.
FILES = [
    "README.md",
    "README.zh-CN.md",
    "docs/plans/codemap.md",
    "docs/plans/platform_capability_matrix.md",
    # The architecture overview states the enum total and is the first file a new contributor
    # reads; a stale number there is the most expensive kind of drift.
    "docs/ARCHITECTURE.md",
    # The generated gallery states the enum count too. It is not hand-edited, but the
    # *generator* can regress to a literal — which it had, at 180 while the enum was 181.
    # Listing it here means the generated page is checked like any other document.
    "control.md",
    # The crate description states the kind total ("231 widgets"). The gate could not see it
    # because none of its spellings covered "widgets" (T3-52).
    "Cargo.toml",
    # The repository maintenance skill is the first file an agent reads; a stale total there is
    # repeated into every generated answer.
    ".dsh/skills/rust-widgets-maintainer/SKILL.md",
    # A pointer page, kept only so existing links resolve. It carries no counts; listing
    # it makes a future hand-written "N kinds" claim inside it fail this gate.
    "docs/WIDGET_GALLERY.md",
]
for lang in ("en", "zh-CN", "zh-TW"):
    FILES += [
        f"cookbook/{lang}/src/README.md",
        f"cookbook/{lang}/src/chapters/architecture.md",
        f"cookbook/{lang}/src/chapters/widget-system.md",
        f"cookbook/{lang}/src/chapters/getting-started.md",
        f"cookbook/{lang}/src/chapters/platform-support.md",
        f"cookbook/{lang}/src/chapters/api-reference.md",
    ]

# Every spelling the docs use for "the total number of widget kinds". Each
# pattern's first group is the number. `\s` matches newlines, so a wrapped claim
# ("All 179\n  widget kinds") is still caught. Only 3-digit numbers are matched.
#
# `(?<![\d,])` is what keeps a *four*-digit claim from being read as a three-digit one.
# The cookbook's `IconName<br/>(4,323 variants)` table row matched `323 variants`
# through the `Variants` spelling and reported a widget-kind mismatch that did not
# exist — a count check that fires on somebody else's number is a false finding, and
# a false finding is as expensive as a missed one because it teaches the reader to
# skim the output. The guard rejects a number preceded by a digit or by the thousands
# separator, so `4,323` and `4323` are both left alone while a real `323 variants`
# claim is still checked.
PATTERNS = [
    # `IGNORECASE` on this one because the prefix is written both ways in the tree — `181 kinds` and
    # `180 Widget Kinds` — and the lowercase-only form silently skipped the cookbook heading.
    re.compile(r"(?<![\d,])(\d{3})\s+(?:built-in\s+)?(?:widget\s+)?[Kk]inds\b", re.IGNORECASE),
    re.compile(r"(?<![\d,])(\d{3})\s+(?:WidgetKind\s+)?[Vv]ariants\b"),
    re.compile(r"(?<![\d,])(\d{3})\s*[種种]"),
    re.compile(r"(?<![\d,])(\d{3})\s*(?:個|个)?\s*[變变][體体]"),
    re.compile(r"(?<![\d,])(\d{3})\s*[（(]\s*(?:full|完整)\s*[)）]"),
    # `controls` / `控件` spellings, and the badge URL, were NOT matched by the set above, so
    # `## What "180 controls" covers`, `「180 个控件」覆盖什么` and the `controls-180-blue` badge all
    # sat at a stale 180 while 22 other documents said 181. The gate was passing — it simply was not
    # looking at the spelling these files actually use, which is the failure mode its own header
    # comment describes ("a gate whose pattern is narrower than the vocabulary drifts"). These rows
    # are the vocabulary in use; add a new spelling here rather than assuming the set is complete.
    re.compile(r"(?<![\d,])(\d{3})\s+controls?\b"),            # 181 controls
    re.compile(r"(?<![\d,])(\d{3})\s+widgets?\b(?!\s+types?)"),  # 231 widgets (not "240 widget types")
    re.compile(r"(?<![\d,])(\d{3})\s*(?:個|个)\s*控件"),          # 181 个控件 / 181 個控件
    re.compile(r"controls-(\d{3})-"),                          # badges/controls-181-blue
    re.compile(r"(?<![\d,])(\d{3})\s+in a device profile\b"),  # "(181 in a device profile)"
]

scanned = 0
errors = 0
for fname in FILES:
    path = Path(fname)
    if not path.is_file():
        print(f"❌ {fname} is listed in the gate but does not exist", file=sys.stderr)
        errors += 1
        continue
    scanned += 1
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    for pattern in PATTERNS:
        for match in pattern.finditer(text):
            num = match.group(1)
            if num == actual:
                continue
            lineno = text.count("\n", 0, match.start()) + 1
            snippet = lines[lineno - 1].strip() if lineno <= len(lines) else ""
            print(
                f"❌ {fname}:{lineno} states '{num}' but the enum has {actual}: {snippet}",
                file=sys.stderr,
            )
            errors += 1

if scanned == 0:
    print("check_widget_kind_count: FAILED (no documents were scanned)", file=sys.stderr)
    sys.exit(1)

if errors:
    print(f"\ncheck_widget_kind_count: FAILED ({errors} mismatch(es))", file=sys.stderr)
    sys.exit(1)

print(f"check_widget_kind_count: {scanned} document(s) state the correct count ({actual})")
PY

echo "✅ check_widget_kind_count: documented variant counts match the enum"
