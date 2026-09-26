#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_semantic_state_is_not_in_the_interaction_chain.sh — BLUE24 §3.3 criterion 4
# ============================================================================
# The rule this guards:
#
#   **`widget_state()` never returns a semantic variant.** The meaning channel
#   (`SemanticState`, read through `Widget::semantic_state`) is orthogonal to the
#   interaction channel, and the two are resolved on separate paths: the fill
#   answers to `widget_state`, the border to `semantic_state`.
#
# The defect this stops
# ---------------------
# `WidgetState` is a *single value* — a lookup key into the theme's colour set and a
# key for its `<kind>:<state>` transition table (BLUE23 §2.3 kept it single-valued on
# purpose, because a set makes state transitions explode). `Error`/`Warning`/`Success`
# were squeezed onto that same chain, so a refusal and an interaction had to compete
# for one value. The interaction wins whenever the pointer is present, which is
# exactly when the user is looking for the refusal: a hovered invalid input box
# silently lost its error colour.
#
# The fix splits the channels (`SemanticState`), and this gate is its **executor**:
# it proves the semantic variants cannot creep back into `widget_state`, which is
# what keeps "two channels" from becoming "two sources of truth".
#
# The rule, stated structurally
# -----------------------------
# Across every `widget_state` implementation in the crate — the trait's default in
# `src/widget/widget_trait.rs` and each control's override — the function body never
# returns `WidgetState::Error`, `WidgetState::Warning` or `WidgetState::Success`.
# The three variants stay in the enum (frozen, `pub` shape unchanged), so this is a
# structural assertion about the *return path*, not about the enum's contents.
#
# What this gate does NOT prove
# -----------------------------
#   * It does not prove a control reports its meaning at all — only that it does not
#     report it on the wrong channel. A control that should have a meaning and has
#     none is caught by its own behavioural test.
#   * It is lexical: it reads a brace-delimited function body. A `widget_state` that
#     delegates to a helper returning a semantic variant would slip through, which is
#     why the delegation is not an existing shape in the crate.
#
# Reverse injection
# -----------------
# Re-introducing `return WidgetState::Error;` into any `widget_state` must make this
# gate name the file:line. See the round's report for the exact output.
#
# Usage: tools/check_semantic_state_is_not_in_the_interaction_chain.sh
# Exit 0 = the interaction chain carries only interaction states.
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TRAIT="src/widget/widget_trait.rs"
SEMANTIC_ENUM="src/style/theme_state.rs"

echo "[1/3] the meaning channel exists beside the interaction one"
if ! grep -qE '^pub enum SemanticState' "$SEMANTIC_ENUM"; then
    echo "  FAIL  src/style/theme_state.rs no longer defines \`pub enum SemanticState\`"
    echo "        It is the channel that carries Error/Warning/Success independently."
    exit 1
fi
if ! grep -qE 'fn semantic_state\(&self\)' "$TRAIT"; then
    echo "  FAIL  the Widget trait no longer declares \`fn semantic_state\`"
    echo "        Without it the meaning has no way off the control but the interaction chain."
    exit 1
fi
echo "  PASS  SemanticState and Widget::semantic_state are both present"

echo "[2/3] every widget_state implementation is free of the semantic variants"
# Extract each `widget_state` body by brace balance, from the signature to its matching
# close brace, then look for a semantic variant in *live* code (comments stripped).
# The three variants are named with their enum qualifier so that a passing mention in
# prose or a doc-link is not a finding.
found=""
while IFS= read -r start; do
    file="${start%%:*}"
    line="${start#*:}"
    line="${line%%:*}"
    # Slice from the signature line to the file end, then stop at brace balance.
    body="$(awk -v from="$line" 'NR>=from{print} NR>=from && /^    }$/ {exit}' "$file" \
        | sed -e 's://.*::')"
    hits="$(printf '%s\n' "$body" \
        | grep -nE 'WidgetState::(Error|Warning|Success)' || true)"
    if [ -n "$hits" ]; then
        while IFS= read -r hit; do
            [ -z "$hit" ] && continue
            offset="${hit%%:*}"
            found="${found}${file}:$((line + offset - 1)):${hit#*:}
"
        done <<< "$hits"
    fi
done < <(grep -rnE 'fn widget_state\(&self\)' src/ --include=*.rs || true)

if [ -n "$found" ] && [ "$(printf '%s' "$found" | tr -d '[:space:]' | wc -c)" -gt 0 ]; then
    echo "  FAIL  a widget_state implementation returns a semantic variant:"
    printf '%s' "$found" | sed 's/^/          /'
    echo "        Error/Warning/Success belong to \`semantic_state\`; returning them here"
    echo "        puts the meaning back on the single-valued interaction chain, where it"
    echo "        loses to hover and the refusal disappears under the pointer (BLUE24 §3)."
    exit 1
fi
echo "  PASS  no widget_state returns Error, Warning or Success"

echo "[3/3] the audit population is non-empty"
# Guards against a vacuous pass: if the extractor matches nothing, the step above
# "passes" by finding nothing to look at. The trait default alone is a population of
# one, and the crate has many overrides.
checked="$(grep -rcE 'fn widget_state\(&self\)' src/ --include=*.rs | grep -v ':0$' | wc -l | tr -d ' ')"
if [ "$checked" -lt 2 ]; then
    echo "  FAIL  only $checked file(s) declare a widget_state; expected the trait plus overrides"
    echo "        A collapsing population means this gate is measuring nothing."
    exit 1
fi
echo "  PASS  $checked files declare a widget_state"

echo
echo "check_semantic_state_is_not_in_the_interaction_chain: OK"
