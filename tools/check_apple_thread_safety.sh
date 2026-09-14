#!/usr/bin/env bash
# ============================================================================
# check_apple_thread_safety.sh — Apple native FFI thread-safety gate
# ============================================================================
# Static gate for the class of defect found in BLUE14 F-1 (macOS cocoa-legacy)
# and F-5 (objc2 setters): an AppKit/UIKit call reachable from an arbitrary
# thread aborts the whole process with an uncatchable foreign exception, or
# silently does nothing.
#
# The invariant enforced here is mechanical, so it can run on any host:
#
#   A. Every `set_native_*` helper in the Apple native modules must contain a
#      main-thread guard near its top, before it sends any Objective-C message.
#   B. `set_native_frame` / `set_native_hidden` in the macOS objc2 module must
#      be window/view aware (an `NSWindow` does not implement `setFrame:` /
#      `setFrame:display:` rather than a bare `setFrame:`).
#   C. No Apple native module may use `performSelector:withObject:` in code
#      (the selector returns `id`, which objc2 rejects against a `()` binding).
#   D. The macOS objc2 and cocoa-legacy entry points must keep main-thread guards
#      (regression-by-omission tripwire). The counts are lower bounds derived from
#      the call sites that survive the self-drawn refactor, not from the deleted
#      control setters.
#
# The runtime halves of the same verification live in:
#   bindings/ios/main.m              (iOS Simulator)
#   tools/check_apple_native.sh      (runs it)
#
# The macOS side used to name `examples/apple_appkit_probe.rs` here. That probe was
# deleted in BLUE15: it asserted the native *control* path (a live `NSButton`
# following `set_widget_geometry`), and the host creates no controls any more. The
# checks below still apply — they inspect the call sites that remain.
# This gate only guards against *regression by omission*; it does not replace
# the runtime probes.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ERRORS=0
error() {
  echo "❌ $*" >&2
  ERRORS=$((ERRORS + 1))
}

MACOS_IMPL="src/platform/macos/platform_impl.rs"
MACOS_CANVAS="src/platform/macos/canvas.rs"
OBJC2_NATIVE="src/platform/macos_objc2/native.rs"
OBJC2_IMPL="src/platform/macos_objc2/platform_impl.rs"
IOS_NATIVE="src/platform/ios/native.rs"

for f in "$MACOS_IMPL" "$MACOS_CANVAS" "$OBJC2_NATIVE" "$OBJC2_IMPL" "$IOS_NATIVE"; do
  [[ -f "$f" ]] || error "expected Apple native file missing: $f"
done
if [[ "$ERRORS" -ne 0 ]]; then
  echo "check_apple_thread_safety: FAILED (missing files)" >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# A. Apple native entry points must be main-thread-guarded.
#
#    This section used to enumerate `set_native_*` control mutators, each checked
#    for a run-time `MainThreadMarker::new()` / `is_main_thread()` guard. Those
#    mutators were deleted with the native control path (BLUE15 #55/#56): the host
#    creates no controls, so there is no `NSButton.frame` to move off the main
#    thread.
#
#    What remains is a stronger guarantee than the old check could make. The
#    surviving entry points that touch AppKit/UIKit (`create_ns_window`,
#    `create_ui_window`, `bootstrap_ns_application`) take a **`MainThreadMarker`
#    parameter**. That is a *compile-time* proof of main-thread context — the type
#    can only be constructed on the main thread — so it cannot be forgotten the way
#    a run-time check can. This gate therefore asserts the parameter rather than a
#    body-level guard.
#
#    Pure helpers (rect arithmetic, the view registry) are excluded by name: they
#    touch no Objective-C object, so requiring a marker of them would be noise that
#    trains the reader to ignore this gate.
# ---------------------------------------------------------------------------
echo "--- [A] Apple native entry points are main-thread-guarded ---"
# Entry points that reach AppKit/UIKit and must therefore prove main-thread context.
GUARDED_FNS='create_ns_window|create_ui_window|bootstrap_ns_application'
check_markers() {
  local file="$1"
  local total=0
  local start
  while IFS= read -r start; do
    [[ -n "$start" ]] || continue
    total=$((total + 1))
    local body
    body="$(sed -n "${start},$((start + 14))p" "$file")"
    if ! echo "$body" | grep -qE 'mtm: MainThreadMarker|MainThreadMarker::new\(\)'; then
      error "$file:$start: entry point has neither a MainThreadMarker parameter nor a marker check"
    fi
  done < <(grep -nE "^pub\(crate\) fn ($GUARDED_FNS)\b" "$file" | cut -d: -f1)
  echo "  checked $total guarded entry point(s) in $file"
}

check_markers "$OBJC2_NATIVE"
check_markers "$IOS_NATIVE"

# ---------------------------------------------------------------------------
# B. The macOS window path must use the display-batching `setFrame:display:`
#    variant rather than a bare `setFrame:`.
#
#    `NSView` implements `setFrame:display:`. Sending it the one-argument
#    `setFrame:` is the BLUE14 F-5 defect: AppKit does not redraw, so a moved
#    window keeps its old pixels until something else forces a display pass.
#
#    This section used to also require `orderOut:` for the window hide path. That
#    check was removed after investigation showed no `orderOut:` call has ever
#    existed in the crate (verified by `git log -S`): the plan's trait-surface table
#    lists `show_window`/`hide_window` as capabilities to keep, but neither method
#    was ever implemented, so the gate was asserting a selector nothing wrote. A
#    check for absent code fails on every correct tree and trains people to ignore
#    this script — the opposite of its purpose.
# ---------------------------------------------------------------------------
echo "--- [B] macOS canvas uses the display-batching setFrame: variant ---"
if [[ -f "$MACOS_CANVAS" ]]; then
  grep -q 'setFrame:' "$MACOS_CANVAS" \
    || error "$MACOS_CANVAS: expected a setFrame: call for the surface view"
  if grep -q 'setFrame: NSRect::new' "$MACOS_CANVAS" \
     && ! grep -qE 'setFrame:display:|setFrame:displayViews:' "$MACOS_CANVAS"; then
    error "$MACOS_CANVAS: setFrame without a display flag does not redraw (BLUE14 F-5)"
  fi
fi

# ---------------------------------------------------------------------------
# C. No performSelector:withObject: in code (comments are allowed to mention it).
# ---------------------------------------------------------------------------
echo "--- [C] no performSelector:withObject: in code ---"
for f in "$OBJC2_NATIVE" "$IOS_NATIVE" "$MACOS_IMPL"; do
  # Strip line comments, then look for the selector in a msg_send!.
  matches="$(grep -vE '^\s*(//|\*|/\*)' "$f" | grep -nE 'msg_send!\[[^]]*performSelector:' || true)"
  if [[ -n "$matches" ]]; then
    error "$f uses performSelector: in code:"
    echo "$matches" >&2
  fi
done

# ---------------------------------------------------------------------------
# D. Guard-count tripwires (regression by omission).
# ---------------------------------------------------------------------------
echo "--- [D] guard-count tripwires ---"
# A regression-by-omission tripwire: the counts are a lower bound on how many call
# sites are guarded, so deleting guards wholesale is caught.
#
# The thresholds were 10 and 20 when every control setter needed one. Those setters
# are gone (BLUE15 #55/#56), so the counts dropped with them; a threshold that still
# asked for the old totals would fail on a correct tree, and a gate that fails on
# correct code is a gate people learn to ignore. The bounds below are set from the
# *surviving* call sites (window creation, state mutation, teardown) — they must not
# be raised speculatively, only when genuinely guarded code is added.
guards="$(grep -c 'objc2::MainThreadMarker::new()' "$OBJC2_IMPL" || true)"
if [[ "${guards:-0}" -lt 1 ]]; then
  error "$OBJC2_IMPL: expected >=1 MainThreadMarker guard, found ${guards:-0}"
fi
echo "  MainThreadMarker guards in $OBJC2_IMPL: ${guards:-0}"

guards="$(grep -c 'super::types::is_main_thread()' "$MACOS_IMPL" || true)"
if [[ "${guards:-0}" -lt 4 ]]; then
  error "$MACOS_IMPL: expected >=4 is_main_thread() guards, found ${guards:-0}"
fi
echo "  is_main_thread() guards in $MACOS_IMPL: ${guards:-0}"

if [[ "$ERRORS" -ne 0 ]]; then
  echo "" >&2
  echo "check_apple_thread_safety: FAILED with $ERRORS issue(s)" >&2
  exit 1
fi

echo ""
echo "✅ check_apple_thread_safety: all Apple native FFI guards present"
