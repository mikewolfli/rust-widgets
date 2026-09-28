#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_cookbook.sh — the three mdBook cookbooks build, and their API names exist
# ============================================================================
# The rule this guards (docs/plans/TODO.md, "Add/complete API documentation"):
#
#   **The cookbook is the API documentation, so it must not describe an API the crate
#   does not have.**
#
# # Why this is a gate
#
# The cookbook was verified against `src/` for the first time in an earlier round and
# **13 declared APIs did not exist**, identically in all three language editions — a
# reader following the reference would have written code that could not compile, three
# times over. Nothing in the build could tell: a Markdown file with a wrong function
# name is a valid Markdown file. So the check has to compare the two.
#
# The second half is that the books must actually **build**. They once all failed
# `mdbook build` outright (mdBook 0.5 removed the `multilingual` key from `book.toml`)
# and each carried an unclosed-HTML warning from a heading like `### Signal<T> API`.
# A reference that does not render is not a reference, and a warning is how the next
# broken heading announces itself — so warnings are failures here.
#
# # What this asserts
#
#   [1] every `` `name` `` backticked identifier the cookbooks present as an API exists
#       somewhere in `src/` (as a definition, not merely as a mention)
#   [2] all three books build with `mdbook build` and **zero** warnings
#
# # What this does NOT prove
#
#   * It does not compile the cookbook's Rust code blocks. `mdbook test` would, but it
#     needs every snippet to be a complete program, and the chapters are deliberately
#     fragments. The C snippets are compiled separately by
#     `tools/check_c_code_examples.sh`.
#   * It does not judge whether an existing name is *documented well*, only that it
#     exists. Prose quality is a review question.
#
# # Reverse injection
#
# Renaming a real API in a cookbook chapter to something the crate does not define must
# make step [1] report it. See `tools/gates_reverse_injection.md`.
#
# Usage: tools/check_cookbook.sh
# Exit 0 = every declared API exists and all three books build clean.
# Exit 1 = a finding is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_python.sh"

# Per-invocation bound for the book builds (principle #58). Initialised with the same
# default every peer gate uses: without it, `set -u` makes the first `mdbook build`
# abort on the unset variable, so step [2] failed for a shell reason on every run and
# the gate could not pass regardless of whether the books were correct.
GATE_TIMEOUT="${GATE_TIMEOUT:-900}"

BOOKS="cookbook/en cookbook/zh-CN cookbook/zh-TW"

echo "[1/2] every cookbook-declared API name exists in src/"

# The scan is in Python because it needs to read the same identifier out of prose, a
# fenced code block and a table cell, then look it up across every `.rs` file. The
# shell wrapper owns the pass/fail judgement so the exit code and the printed summary
# cannot disagree (the same split `tools/font_license_scan.py` uses).
if ! OUT="$("$PYTHON" tools/cookbook_api_scan.py)"; then
    echo "  FAIL  the cookbook API scan could not run"
    exit 1
fi
echo "$OUT"
case "$OUT" in
    *"failed=0"*) ;;
    *)
        echo ""
        echo "  A cookbook chapter names an API the crate does not define. Either the"
        echo "  snippet is wrong (fix it) or the name was renamed in src/ and the three"
        echo "  editions were not updated (fix all three — a partial update is how the"
        echo "  reference drifts)."
        exit 1
        ;;
esac

echo "[2/2] all three books build with no warnings"

for book in $BOOKS; do
    if [ ! -f "$book/book.toml" ]; then
        echo "  FAIL  $book/book.toml is missing"
        exit 1
    fi
    # mdbook writes warnings to stderr; a non-empty stderr is a finding even on exit 0,
    # because that is exactly how the unclosed-HTML heading stayed hidden.
    log="$(mktemp)"
    if ! (cd "$book" && rw_run_bounded "$GATE_TIMEOUT" mdbook build) > "$log" 2>&1; then
        echo "  FAIL  $book did not build:"
        sed 's/^/        /' "$log" | tail -20
        rm -f "$log"
        exit 1
    fi
    if grep -qiE "warning|error" "$log"; then
        echo "  FAIL  $book built with a warning or error:"
        grep -iE "warning|error" "$log" | sed 's/^/        /' | head -20
        rm -f "$log"
        exit 1
    fi
    rm -f "$log"
    echo "        $book: built clean"
done

# The rendered HTML is a build artifact and is deliberately not committed (see
# `.gitignore`); leaving it behind would make the next `git status` noisy.
for book in $BOOKS; do
    rm -rf "$book/book"
done

echo ""
echo "check_cookbook: OK"
