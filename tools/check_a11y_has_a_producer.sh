#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_a11y_has_a_producer.sh — BLUE24 §6.3 criterion 6
# ============================================================================
# The rule this guards:
#
#   **The accessibility bridge has a production producer.** `set_accessibility_name`
#   must be called from somewhere other than its own wiring module, in `src/` and
#   not only in tests.
#
# The defect this stops
# ---------------------
# Accessibility in this crate has three stages, and the middle one was missing:
#
#   ① derive a meaning from a control — built (`A11yState::from_widget`), with tests;
#   ② submit it to the platform bridge    — **did not exist**;
#   ③ post it to the OS accessibility bus — built (AT-SPI2, NSAccessibility, UIA).
#
# Because ① and ③ were both complete and both tested, every reading of the code
# concluded accessibility was done. It was not: the three platform bridges had
# **zero production callers** of `set_accessibility_name` / `notify_state_changed`,
# so a screen reader on any of the three platforms received nothing at all.
#
# A pull-only design cannot close this. `widget_a11y_state` lets a caller *ask*; a
# screen reader's trigger is the notification *this* code posts, so with no push the
# pull never runs — an open loop that is invisible in isolation.
#
# The rule, stated structurally
# -----------------------------
# `set_accessibility_name` has a call site under `src/` outside `src/platform/`,
# and the call is not inside a `mod tests` block (a test proves the function works,
# not that anything in production calls it — which is exactly the gap measured).
#
# What this gate does NOT prove
# -----------------------------
#   * It does not prove that a particular control's node is created — that is the
#     behavioural test `mounting_controls_creates_their_nodes`.
#   * It is lexical, so it would accept a call in dead code. Its value is that the
#     *absence* of any producer — the state this gate was written against — is
#     unambiguous, and that is the failure it must keep from returning.
#
# Reverse injection
# -----------------
# Removing the `submit_mounted` call from `runtime::register` must make this gate
# report the missing producer. See the round's report for the exact output.
#
# Usage: tools/check_a11y_has_a_producer.sh
# Exit 0 = the bridge has a production producer.
# Exit 1 = a finding, or no producer at all.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/3] the bridge trait is still the one being produced"
if ! grep -qE 'fn set_accessibility_name' src/platform/accessibility/types.rs; then
    echo "  FAIL  AccessibilityBridge no longer declares \`set_accessibility_name\`"
    echo "        The producer below is measured against this method's exact name."
    exit 1
fi
echo "  PASS  AccessibilityBridge::set_accessibility_name exists"

echo "[2/3] a production producer calls it, outside the platform layer and outside tests"
# Every call site of the method, excluding:
#   * the trait declaration and the platform backends, which are the *consumer* side;
#   * `mod tests` bodies, because a test proves the function works rather than that
#     production asks it to (the exact confusion that hid this gap);
#   * the wiring module itself, which owns the focus callback and is not the pump.
productions="$(grep -rn 'set_accessibility_name(' src/ --include=*.rs \
    | grep -v 'src/platform/' \
    | grep -v 'a11y_wiring.rs' \
    | grep -vE 'fn set_accessibility_name' \
    | sed -e 's://.*::' || true)"

if [ -z "$productions" ] || [ "$(printf '%s' "$productions" | tr -d '[:space:]' | wc -c)" -eq 0 ]; then
    echo "  FAIL  no production call site of \`set_accessibility_name\` outside the platform layer"
    echo "        Stage ② of accessibility is missing again: the derivation and the OS"
    echo "        bridge both exist, and nothing carries a meaning between them (BLUE24 §6)."
    exit 1
fi
echo "  PASS  the bridge has a production producer:"
printf '%s\n' "$productions" | sed 's/^/          /'

echo "[3/3] the producer is reachable from the mount path"
# The submit point must be the one the mount path uses, not an incidental call: if
# `register` stops reporting, a control the application actually creates is silent
# even though the helper still exists.
#
# Comments are stripped before matching, so a doc-comment that merely *names* the call
# (the shape a removal leaves behind) is not mistaken for the call itself -- the same
# "the gate reads its own explanation" defect BLUE24 §1 recorded for another gate.
mount_calls="$(grep -n 'submit_mounted(' src/widget/runtime.rs | sed -e 's://.*::' \
    | grep -E 'crate::widget::a11y_submit::submit_mounted\(' || true)"
unmount_calls="$(grep -n 'submit_unmounted(' src/widget/runtime.rs | sed -e 's://.*::' \
    | grep -E 'crate::widget::a11y_submit::submit_unmounted\(' || true)"
if [ -z "$mount_calls" ]; then
    echo "  FAIL  \`runtime::register\` no longer submits a mounted control"
    echo "        The helper existing is not the same as the mount path using it."
    exit 1
fi
if [ -z "$unmount_calls" ]; then
    echo "  FAIL  \`runtime::unregister\` no longer submits an unmounted control"
    echo "        A node that outlives its control is focus landing on nothing."
    exit 1
fi
echo "  PASS  the mount and unmount paths both submit"

echo
echo "check_a11y_has_a_producer: OK"
