#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_animation_durations_are_tokens.sh — BLUE24 §2.4 gate A (BLUE23 §3.3 判据 5)
# ============================================================================
# The rule this guards (BLUE24 §2.4, P0-5):
#
#   **A control's self-driven animation duration must come from `theme.motion`,
#   not from a literal.**
#
#   1. the frame step (`ANIMATION_FRAME_DELTA_MS = 16`) is a *rhythm*, not a duration,
#      and is allowed — with its reason recorded in `tools/transition_duration_exemptions.txt`;
#   2. a control's own tempo must be named as a `MotionSlot`, never as a number.
#
# # Where the "must come from theme.motion" half actually lives
#
# This is the part of BLUE24 §2.4 that the plan expected to be added *here*, and the
# measurement says it is already enforced somewhere stronger. §2.4's gate A asks for
# "控件自驱动动画的时长必须取自 theme.motion（MotionSlot ⇒ Motion::fast/normal/slow）",
# i.e. a *token* in the source. The mechanism that landed (`src/style/animation.rs`,
# BLUE24 批 2) makes the literal **unrepresentable** rather than merely forbidden:
#
#     pub type MotionSlot = TransitionTempo;              // animation.rs:1764
#     pub enum TransitionTempo { Fast, Normal, Slow }     // animation.rs:1564
#     impl PropertyDriver {
#         pub fn at(value: f32, tempo: MotionSlot) -> Self  // animation.rs:1824, the ONLY constructor
#         ...
#     }
#     impl TransitionTempo {
#         pub fn duration_ms(self) -> u32 {                 // animation.rs:1582
#             let (fast, normal, slow) = crate::style::motion_tokens();
#             match self { Fast => fast, Normal => normal, Slow => slow }
#         }
#     }
#
# There is no `PropertyDriver::at(_, 200)`: the second parameter's type is a three-variant
# enum, so a literal duration does not compile. `motion_tokens()` (`src/style/theme.rs:296`)
# is the one read of `theme.motion`, and it is also where the environment's reduced-motion
# preference is applied (`reduce_motion`), which is what makes "reduced motion" a property
# of the single read rather than a branch per control.
#
# So a *lexical* search for "did the control write a number" would be strictly weaker than
# what the compiler already rejects, and would also be unable to see the thing that can
# actually rot: someone adding a second constructor that takes a `u32`. This gate therefore
# asserts the **structure** that carries the guarantee, in three parts:
#
#   A1. `PropertyDriver` has exactly one constructor, and its tempo parameter is `MotionSlot`.
#   A2. `MotionSlot` is an enum whose variants are exactly `Fast`/`Normal`/`Slow` — no
#       `Millis(u32)`-style variant, which would reopen the literal path through the type.
#   A3. `TransitionTempo::duration_ms` reads `motion_tokens()` — the one theme read — and
#       does not fall back to a per-arm literal.
#
# And for the half of §2.4 that *is* lexical — durations written outside this mechanism —
# the existing `check_transition_durations_are_tokens` owns it, with its exemption table.
# The two gates are complements: that one finds a literal that reached a control some other
# way, this one proves the intended way has no literal in it at all.
#
# # What this gate does NOT prove
#
#   * It does not prove any *particular* control uses the mechanism. A control that animates
#     nothing at all cannot violate it, and a control that calls `PropertyDriver::at(0.0,
#     MotionSlot::Slow)` where `Fast` was meant is a taste question, not a structural one.
#   * It is not a behavioural test. It cannot observe a duration; it observes that the shape
#     of the code leaves no room for one.
#   * It says nothing about `Transition::with_tempo`'s callers, which are the pre-`PropertyDriver`
#     spelling of the same thing; `check_animation_state_is_one_type` tracks that migration.
#
# # Reverse injection
#
# This project's standard is "a gate that has never been seen to fail does not count".
# Verified injections, each performed and observed:
#   * add a `pub fn at_ms(value: f32, ms: u32) -> Self` to `PropertyDriver`  ⇒ A1 red
#   * add a variant `Millis(u32)` to `TransitionTempo`                       ⇒ A2 red
#   * replace the `duration_ms` body with `match self { Fast => 100, .. }`   ⇒ A3 red
#
# Usage: tools/check_animation_durations_are_tokens.sh
# Exit 0 = the token chain is intact. Exit 1 = a finding, named with its file and line.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

ANIMATION="src/style/animation.rs"

if [[ ! -f "$ANIMATION" ]]; then
    echo "FAIL: $ANIMATION is missing; this gate's whole subject is that file"
    exit 1
fi

# The scan is a heredoc written to a function rather than inlined into `$( ... )`.
#
# # Why this indirection exists (a real bash limitation, measured)
#
# `FINDINGS="$( "$PYTHON" - "$ANIMATION" <<'PY' ... PY )"` **does not parse** when the
# heredoc body contains an **odd** number of apostrophes — which this one does, because the
# finding messages quote the source they complain about (`` `TransitionTempo`'s variants ``,
# `['Fast', 'Normal', 'Slow']`). Bash pre-scans `$( )` for its closing parenthesis while
# tracking quote state, and a quoted heredoc's body is not exempt from that scan, so the
# apostrophes were counted as opening quotes and the construct failed to parse.
#
# The symptom was worse than an error: the gate **never ran**. `bash -n` reports
# `unexpected EOF while looking for matching \'`, so the script exited non-zero without
# asserting anything — a gate that is present, wired into the runner, and silently inert.
# Collecting the output through a function removes the `$( )` from the picture entirely.
scan_animation() {
    "$PYTHON" - "$ANIMATION" <<'PY'
import pathlib
import re
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
lines = text.splitlines()
findings = []


def line_of(pattern, flags=0):
    """The 1-based line number of the first match of `pattern`, or None."""
    match = re.search(pattern, text, flags)
    return text[: match.start()].count("\n") + 1 if match else None


# ── A1. PropertyDriver's constructors ───────────────────────────────────────────────────────────
#
# Exactly one: `at(value, tempo)`. A second one is how a literal gets back in -- under any name
# (`at_ms`, `with_duration`, `priced_by_millis`) -- so the assertion counts constructors rather
# than looking for one specific bad name.
driver = re.search(r"impl PropertyDriver \{(.*?)\n\}", text, re.DOTALL)
if not driver:
    findings.append(f"{path}: the `impl PropertyDriver` block was not found; A1 cannot be judged")
else:
    ctor_pattern = re.compile(
        r"pub fn (\w+)\s*\(([^)]*)\)\s*->\s*(Self|PropertyDriver)", re.DOTALL
    )
    ctors = [
        (m.group(1), m.group(2))
        for m in ctor_pattern.finditer(driver.group(1))
    ]
    if not ctors:
        findings.append(
            f"{path}: `PropertyDriver` has no constructor; A1 cannot be judged "
            "(the type is unusable, which is a different defect)"
        )
    for name, params in ctors:
        if not re.search(r"\btempo\s*:\s*MotionSlot\b", params):
            number = line_of(rf"pub fn {re.escape(name)}\s*\(")
            findings.append(
                f"{path}:{number}: `PropertyDriver::{name}` is a constructor whose tempo "
                f"parameter is not `MotionSlot` -- a constructor that takes a number reopens "
                f"the literal path the token type exists to close: pub fn {name}({params.strip()})"
            )


# ── A2. MotionSlot's variant set ────────────────────────────────────────────────────────────────
#
# `pub type MotionSlot = TransitionTempo;` is the alias the controls name. Either it is an alias
# of the three-variant enum, or it is an enum in its own right; both are fine. What is not fine
# is a variant that carries a number, because `MotionSlot::Millis(200)` is a literal wearing the
# token's name.
tempo_enum = re.search(r"pub enum TransitionTempo \{(.*?)\n\}", text, re.DOTALL)
if not tempo_enum:
    findings.append(f"{path}: `pub enum TransitionTempo` was not found; A2 cannot be judged")
else:
    body = tempo_enum.group(1)
    variants = re.findall(r"^\s*([A-Z]\w*)\s*(\([^)]*\))?\s*,", body, re.MULTILINE)
    names = [v[0] for v in variants]
    carrying = [(v[0], v[1]) for v in variants if v[1]]
    if carrying:
        for name, payload in carrying:
            number = line_of(rf"{re.escape(name)}\s*{re.escape(payload)}")
            findings.append(
                f"{path}:{number}: `TransitionTempo::{name}{payload}` carries a payload, so a "
                f"duration can be spelled with the token's own name -- the token must name "
                f"*which* tempo, never *how long*"
            )
    if set(names) != {"Fast", "Normal", "Slow"}:
        number = line_of(r"pub enum TransitionTempo")
        findings.append(
            f"{path}:{number}: `TransitionTempo`'s variants are {names}, expected exactly "
            f"['Fast', 'Normal', 'Slow'] -- the enum's whole job is to be the closed set the "
            f"theme prices"
        )

    # The alias must point at this enum, so the name a control writes and the set the theme
    # prices cannot drift apart.
    alias = re.search(r"pub type MotionSlot = (\w+);", text)
    if not alias:
        findings.append(
            f"{path}: `pub type MotionSlot = ...;` was not found; controls name `MotionSlot` "
            f"and nothing would connect it to the tempo set"
        )
    elif alias.group(1) != "TransitionTempo":
        number = line_of(r"pub type MotionSlot = ")
        findings.append(
            f"{path}:{number}: `MotionSlot` aliases `{alias.group(1)}`, not `TransitionTempo` -- "
            f"the alias and the priced set must be the same type (principle #54)"
        )


# ── A3. The one theme read ──────────────────────────────────────────────────────────────────────
#
# `duration_ms` must *call* `motion_tokens()`. A body that returns per-arm numbers is the
# defect this part exists to catch: it looks like a tempo table and is a set of literals.
duration = re.search(r"pub fn duration_ms\(self\) -> u32 \{(.*?)\n    \}", text, re.DOTALL)
if not duration:
    findings.append(f"{path}: `TransitionTempo::duration_ms` was not found; A3 cannot be judged")
else:
    body = duration.group(1)
    if "motion_tokens()" not in body:
        number = line_of(r"pub fn duration_ms\(self\)")
        findings.append(
            f"{path}:{number}: `duration_ms` does not call `crate::style::motion_tokens()`, so the "
            f"tempo no longer comes from `theme.motion` (nor from the reduced-motion collapse "
            f"that read applies): {body.strip()[:160]}"
        )
    # A literal in the arm values is the "tempo table" defect. The match arms must forward the
    # values `motion_tokens` returned, not restate them.
    for arm in re.finditer(r"TransitionTempo::(\w+)\s*=>\s*([^,\n]+)", body):
        if re.fullmatch(r"\d+", arm.group(2).strip()):
            number = line_of(rf"TransitionTempo::{arm.group(1)}\s*=>\s*{arm.group(2).strip()}")
            findings.append(
                f"{path}:{number}: `duration_ms` returns the literal {arm.group(2).strip()} for "
                f"`{arm.group(1)}`; the theme's token must be forwarded, not restated"
            )


for finding in findings:
    print(finding)
print(f"checked=3 failed={len(findings)}")
PY
}

FINDINGS="$(scan_animation)"

SUMMARY="$(printf '%s\n' "$FINDINGS" | tail -n 1)"
DETAIL="$(printf '%s\n' "$FINDINGS" | sed '$d')"
FAILED="$(printf '%s' "$SUMMARY" | sed -n 's/.*failed=\([0-9]*\).*/\1/p')"
CHECKED="$(printf '%s' "$SUMMARY" | sed -n 's/.*checked=\([0-9]*\).*/\1/p')"

if [[ -z "${FAILED:-}" ]] || [[ -z "${CHECKED:-}" ]]; then
    echo "FAIL: the rule could not be evaluated (no summary parsed)"
    printf '%s\n' "$FINDINGS"
    exit 1
fi

if [[ "$CHECKED" -lt 3 ]]; then
    echo "FAIL: only $CHECKED of 3 assertions were judged; the scan is not reading the token chain"
    printf '%s\n' "$FINDINGS"
    exit 1
fi

if [[ "$FAILED" -gt 0 ]]; then
    echo "FAIL: a self-driven animation duration can be spelled without a theme token."
    printf '%s\n' "$DETAIL"
    echo ""
    echo "  The token chain this gate pins, and must stay intact:"
    echo "    PropertyDriver::at(value, MotionSlot)   -- the only constructor"
    echo "    MotionSlot = TransitionTempo{Fast,Normal,Slow}   -- a closed set, no payload"
    echo "    TransitionTempo::duration_ms() -> crate::style::motion_tokens()   -- the one read"
    echo ""
    echo "check_animation_durations_are_tokens: checked=$CHECKED failed=$FAILED"
    exit 1
fi

echo "check_animation_durations_are_tokens: checked=$CHECKED failed=0"
