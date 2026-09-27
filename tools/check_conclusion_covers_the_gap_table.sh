#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_conclusion_covers_the_gap_table.sh — BLUE24 §14.5 / §14.3
# ============================================================================
# The rule this guards:
#
#   **A plan's one-sentence conclusion must cover every dimension its own gap
#   table names — and must not state a count it cannot keep.**
#
# The defect this stops
# ---------------------
# `blue24.md` §13 is the plan's single sentence of definition, and it is what a
# reader uses to check "which dimensions does this plan actually cover". It had
# three defects at once, all measurable:
#
#   * "三件事…" (three things) followed by **five** bullets;
#   * "本计划只做四件…的事" (four things) followed by **five** clauses;
#   * §5 (adaptive layout) was **absent** from §13 entirely, though it is half
#     of §0B row 4 and all of §10 batch 8.
#
# Meanwhile §0B holds **8** rows and §10 holds **9** batches. Four tables in one
# file, four different counts. This is principle #18's shape one level down: not
# "the docs disagree with the code" but "**the docs disagree with themselves**".
#
# Why a gate rather than a one-time edit
# --------------------------------------
# §14's steps fixed the counts by **removing** them, so the numbers no longer
# need a human to keep them in sync. That is the half that cannot regress. What
# *can* regress is the coverage: the next section added to this plan, or the
# next row added to §0B, would silently drop out of §13 again — exactly how §5
# dropped out the first time. This asserts the structural facts:
#
#   [1] §13 carries no hard-coded "N 件事" count word (a count a human maintains);
#   [2] every non-meta row of §0B is locatable in §13's dimension set;
#   [3] §5 (`Breakpoint`) is named in §13;
#   [4] §13's anchor back to §0B is present.
#
# The meta row
# ------------
# §0B row 7 ("the plan files themselves have no single entry point") is a **meta**
# row: it is about the plan, not about an application capability. §14.2 records it
# as deliberately not merged into §13's table, and this gate encodes that: the
# row whose `本计划节` is §9 is exempt from the coverage requirement, and the
# exemption is read from the table rather than hard-coded here — so moving the
# row would move the exemption with it.
#
# Reverse injection (tools/gates_reverse_injection.md)
# ----------------------------------------------------
# Delete the "§5 `Breakpoint`" cell          -> [3] fails;
# delete the §0B anchor line                 -> [4] fails;
# add a count word back to §13               -> [1] fails;
# remove a §0B row's dimension from §13      -> [2] fails.
#
# Usage: tools/check_conclusion_covers_the_gap_table.sh
# Exit 0 = the conclusion covers the gap table and states no stale count.
# Exit 1 = a finding (each is named with the line that shows it).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PLAN="docs/plans/blue24.md"

echo "[0/4] the plan exists and has the two sections this compares"
if [ ! -f "$PLAN" ]; then
    echo "  FAIL  $PLAN is missing"
    echo "        This gate compares §0B against §13; without the file it measures nothing,"
    echo "        and a missing file must not read as 'no findings'."
    exit 1
fi
for section in '^## 0B\.' '^## 13\.'; do
    if ! grep -qE "$section" "$PLAN"; then
        echo "  FAIL  $PLAN has no section matching $section"
        echo "        The comparison this gate makes is between those two sections; if one is"
        echo "        renamed away the gate would silently have nothing to compare."
        exit 1
    fi
done
echo "  PASS  both sections are present"

echo "[1/4] §13 states no hard-coded count word"
# The check is anchored to the **start of a sentence** (or after a full stop /
# semicolon / colon), not a bare `grep '件事'`.
#
# # Why the anchor is the whole point
#
# "同一件事" ("the same thing") is ordinary Chinese and appears throughout this
# file, including in §14's own prose describing this very defect. A bare
# `grep '件事'` reports 23 hits on a correct document and would therefore be
# wired off — the failure mode BLUE22 §F.3 lesson 4 records. What is a defect is
# the *sentence-opening* count ("三件事…", "四件事…"), because that is a number a
# reader can check against the list that follows it.
COUNT_WORDS="$(python3 - "$PLAN" <<'PY'
import re, sys

path = sys.argv[1]
lines = open(path, encoding="utf-8").read().splitlines()

# §13 runs from its heading to the next `---` (the section separator).
start = None
for index, line in enumerate(lines):
    if re.match(r"^## 13\.", line):
        start = index + 1
        break
end = len(lines)
for index in range(start, len(lines)):
    if lines[index].strip() == "---":
        end = index
        break

# A sentence-opening count: at the line start, or right after one of . ； ：
pattern = re.compile(r"(?:^|[。；：]\s*)[一二三四五六七八九十]件事")
for offset in range(start, end):
    if pattern.search(lines[offset]):
        print(f"{offset + 1}: {lines[offset].strip()}")
PY
)"
if [ -n "$COUNT_WORDS" ]; then
    echo "  FAIL  §13 opens a sentence with a count word, which must be kept in sync by hand:"
    printf '%s\n' "$COUNT_WORDS" | sed 's/^/          /'
    echo "        Do not replace it with the right number — remove it. A count a human"
    echo "        maintains is what drifted three ways in this file (§14.2 step 1)."
    exit 1
fi
echo "  PASS  no sentence-opening count word in §13"

echo "[2/4] every non-meta §0B row is locatable in §13"
echo "[3/4] §5 (adaptive layout) is named in §13"
echo "[4/4] §13 anchors back to §0B"
python3 - "$PLAN" <<'PY'
import re, sys

path = sys.argv[1]
text = open(path, encoding="utf-8").read()
lines = text.splitlines()


def section(heading, stop):
    """The lines of `heading`, up to the first line matching `stop`."""
    out, inside = [], False
    for line in lines:
        if re.match(heading, line):
            inside = True
            continue
        if inside and re.match(stop, line):
            break
        if inside:
            out.append(line)
    return out


# ── §0B: the gap rows ────────────────────────────────────────────────────────
# A row is `| <n> | <gap> | <now> | <cost> | <section> |`. The section column is
# what decides whether it is a capability (must appear in §13) or meta.
gap_rows = []
for line in section(r"^## 0B\.", r"^## "):
    match = re.match(r"^\|\s*(\d+)\s*\|(.*)\|\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|\s*$", line)
    if not match:
        continue
    gap_rows.append({
        "n": int(match.group(1)),
        "gap": match.group(2),
        "sections": match.group(5).strip(),
    })

if not gap_rows:
    print("  FAIL  §0B has no parsable gap rows")
    print("        A gate that parses zero rows would report 'no findings' on an empty table.")
    sys.exit(1)

# ── §13: the dimension table ─────────────────────────────────────────────────
conclusion = "\n".join(section(r"^## 13\.", r"^---"))

# The dimension table's rows are `| **name** | ... | ... |`.
dims = re.findall(r"^\|\s*\*\*([^*]+)\*\*", conclusion, re.MULTILINE)
if not dims:
    print("  FAIL  §13 has no `| **dimension** |` table rows")
    print("        §14.2 step 2 replaced the prose clause with this table; a missing table")
    print("        means the coverage claim cannot be checked at all.")
    sys.exit(1)

problems = []

# ── [2] every non-meta row is locatable ──────────────────────────────────────
# The map is read from §14.2 step 4's own table, which is the plan's stated
# merge relation. Reading it rather than re-deriving it keeps this gate and the
# plan describing the same fact: if the merge relation changes, the table in the
# plan changes with it and the gate follows.
merge_rows = {}
for line in section(r"^#### 步骤 4", r"^####|^### "):
    match = re.match(r"^\|\s*([^|]+?)\s*\|(.+)\|\s*$", line)
    if not match:
        continue
    # The left cell is a list of §0B row numbers, possibly with labels:
    # `1 帧循环 / 2 属性动画`. Every number in it is a row mapping to the right cell.
    numbers = re.findall(r"\d+", match.group(1))
    if not numbers:
        continue
    for number in numbers:
        merge_rows[int(number)] = match.group(2).strip()

for row in gap_rows:
    # Meta rows are the ones whose section column is a plan-machinery section.
    is_meta = row["sections"].replace(" ", "") in {"§9", "§9.1", "§9.2"}
    if is_meta:
        continue
    if row["n"] not in merge_rows:
        problems.append(
            f"§0B row {row['n']} ({row['gap'].strip()[:40]}…) is not in §14.2 step 4's merge table"
        )
        continue
    destination = merge_rows[row["n"]]
    # "合并/并入 X" names a dimension; "不并入" is an explicit meta statement.
    if "不并入" in destination:
        continue
    # The named dimension must be one of §13's rows. Matched in **either**
    # direction: §13 labels a row "它会跟着系统设置变" while the merge table writes
    # "会跟着系统设置变", and both name the same row. Requiring exact equality would
    # make a harmless rewording of one side a finding.
    def names_a_dimension(cell: str) -> bool:
        return any(named in cell or cell in named for named in dims)

    if not names_a_dimension(destination):
        problems.append(
            f"§0B row {row['n']} merges into {destination!r}, "
            f"which names none of §13's dimensions {dims}"
        )

# Every dimension §13 claims must exist as a row — this is the other direction,
# and it is what makes the table's count its own evidence.
if len(dims) < 5:
    problems.append(
        f"§13 has only {len(dims)} dimension row(s) ({dims}); §14.2 step 2 fixes five"
    )

# ── [3] §5 is named ──────────────────────────────────────────────────────────
# The specific coverage loss this whole section exists for. Checked as its own
# finding, with its own message, because "§5 dropped out of §13" is the exact
# regression §14.1 measured — a generic "a row is missing" would not say so.
if "Breakpoint" not in conclusion:
    problems.append(
        "§5 is not named in §13 (no `Breakpoint`): it is half of §0B row 4 and all of"
        " §10 batch 8, and its absence is the defect §14.1 measured"
    )

# ── [4] the anchor back to §0B ───────────────────────────────────────────────
if "§0B" not in conclusion:
    problems.append(
        "§13 does not name §0B: §0B is the defect view and §13 the capability view of"
        " one dimension set, and without the cross-reference a reader cannot check that"
    )

if problems:
    print("  FAIL  the conclusion and the gap table disagree:")
    for problem in problems:
        print(f"          {problem}")
    sys.exit(1)

capability_rows = [r for r in gap_rows if r["sections"].replace(" ", "") not in {"§9", "§9.1", "§9.2"}]
print(f"  PASS  {len(capability_rows)} capability row(s) map into {len(dims)} dimension(s): {', '.join(dims)}")
print("  PASS  §5 (Breakpoint) is named in §13")
print("  PASS  §13 anchors back to §0B")
PY

echo
echo "check_conclusion_covers_the_gap_table: OK"
