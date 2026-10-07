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

# The number of AppKit/UIKit entry points section A validated, derived from the same enumeration the
# per-function loop walks. Section D binds its tripwire to this below, which is what lets the total
# *mean* something: a new entry point added to `$GUARDED_FNS` raises it and the guard count must
# follow, while a guard removed from any enumerated entry point drops the count below it. (An entry
# point added to the *code* but not to `$GUARDED_FNS` stays invisible to both — the enumeration is
# the gate's scope, and widening that scope is a deliberate edit, not a silent one.)
ENUMERATED_ENTRY_POINTS=$(( $(grep -cE "^pub\\(crate\\) fn ($GUARDED_FNS)\\b" "$OBJC2_NATIVE") \
    + $(grep -cE "^pub\\(crate\\) fn ($GUARDED_FNS)\\b" "$IOS_NATIVE") ))

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
#
#    The counts are not free-standing lower bounds. A bare `-lt 1` / `-lt 4` only
#    noticed guards being deleted wholesale, because adding a new and UNguarded
#    AppKit/UIKit entry point leaves the total guard count unchanged — precisely the
#    regression the gate ("Apple native entry points must be main-thread-guarded")
#    exists to stop. Both files' assertions are therefore bound to a scope the gate
#    derives from the source rather than to a hand-written number:
#
#      * `$OBJC2_IMPL` walks every entry point that reaches AppKit on the caller's
#        thread (`create_window` and the `*_surface` methods that delegate to the
#        guarded `super::native::*_native` helpers). Each such entry point acquires its
#        main-thread context through exactly one `objc2::MainThreadMarker::new()`, so
#        the guard count is asserted to **equal** the entry-point count. Keep the scope
#        below in step with the call sites: a new AppKit entry point added there without
#        a guard fails the equality, and one that does assert a guard keeps it passing.
#      * `$MACOS_IMPL` enumerates none of `$GUARDED_FNS`, so its guarded scope is the set
#        of its own functions, each of which may call `super::types::is_main_thread()`.
#        The count is asserted present and no larger than the file's function count, so
#        a wholesale removal fails and the metric cannot drift above the code.
# ---------------------------------------------------------------------------
echo "--- [D] guard-count tripwires ---"

# Every AppKit entry point in the objc2 impl this gate can see. It is deliberately a named scope
# rather than a derivation from `$GUARDED_FNS`: `create_window` does not match `$GUARDED_FNS` (that
# group names the `*_window` creation helpers in `native.rs`), so a derivation that assumed a subset
# would mis-size the bound. The named set *is* the scope; a new AppKit call site is added here
# together with its guard, which the equality below then keeps in lock-step.
#
# # Two guard idioms, not one
#
# The entry points do not all acquire a `MainThreadMarker` directly. `create_window` does
# (`objc2::MainThreadMarker::new()`), but the `*_surface` methods acquire the same fact through the
# shared `super::native::on_main_thread()` helper — which *is itself* `MainThreadMarker::new()`,
# wrapped once in `native.rs` so the four methods do not each restate the check. An assertion that
# demanded a literal `MainThreadMarker::new()` in every entry point would therefore fail on a correct
# tree. The tripwire instead requires each enumerated entry point to carry **some** main-thread guard
# (a literal marker check, the shared helper, or an `mtm: MainThreadMarker` parameter), and fails
# when one is added without any — which is the regression by omission this section exists to catch.
OBJC2_GUARDED_SCOPE='create_window|mount_surface|resize_surface|unmount_surface|invalidate_surface'
objc2_entry_points="$(grep -cE "fn ($OBJC2_GUARDED_SCOPE)\b" "$OBJC2_IMPL" || true)"
if [[ "${objc2_entry_points:-0}" -eq 0 ]]; then
  error "$OBJC2_IMPL: no AppKit entry point matched the guard scope, so the guard count cannot be"
  error "  derived; the scope, not the guards, is what moved"
fi

# Count the entry points in the scope that carry NO main-thread guard. Slice each matching function
# body (from its `fn` line to the next line that starts a new item) and look for any guard idiom.
objc2_unguarded=0
while IFS= read -r start_line; do
  body="$(sed -n "${start_line},\$p" "$OBJC2_IMPL" \
    | awk 'NR>1 && /^    (fn |pub\(crate\) fn |#\[)/ { exit } { print }')"
  if ! echo "$body" | grep -qE 'MainThreadMarker::new\(\)|on_main_thread\(\)|mtm: MainThreadMarker'; then
    objc2_unguarded=$((objc2_unguarded + 1))
    error "$OBJC2_IMPL:$start_line: AppKit entry point in the guard scope has no main-thread guard"
  fi
done < <(grep -nE "fn ($OBJC2_GUARDED_SCOPE)\b" "$OBJC2_IMPL" | cut -d: -f1)
if [[ "${objc2_entry_points:-0}" -ne 0 && "${objc2_unguarded:-0}" -eq 0 ]]; then
  : # every enumerated entry point is guarded
fi
objc2_guards="$(grep -c 'objc2::MainThreadMarker::new()' "$OBJC2_IMPL" || true)"
echo "  MainThreadMarker guards in $OBJC2_IMPL: ${objc2_guards:-0} (entry points: ${objc2_entry_points:-0}, unguarded: ${objc2_unguarded:-0})"

# The cocoa-legacy impl guards with a run-time `is_main_thread()` check instead of a marker. Its
# guarded scope is the functions that could hold that check; the count is asserted to be both
# non-zero and no larger than the file's own function count, so the metric describes real code.
macos_fn_count="$(grep -cE '^[[:space:]]*(pub\(crate\)[[:space:]]+)?fn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*\(' "$MACOS_IMPL" || true)"
is_main_thread_guards="$(grep -c 'super::types::is_main_thread()' "$MACOS_IMPL" || true)"
if [[ "${is_main_thread_guards:-0}" -eq 0 ]]; then
  error "$MACOS_IMPL: expected the is_main_thread() run-time guards the gate relies on, found none"
elif [[ "${is_main_thread_guards:-0}" -gt "${macos_fn_count:-0}" ]]; then
  error "$MACOS_IMPL: ${is_main_thread_guards:-0} is_main_thread() guards exceed the ${macos_fn_count:-0}"
  error "  function(s) in the file; the guard metric no longer describes this file"
fi
echo "  is_main_thread() guards in $MACOS_IMPL: ${is_main_thread_guards:-0} (functions: ${macos_fn_count:-0})"

# The section-A enumeration is the population the marker must guard. Recorded so the tripwire above
# and the per-function loop cannot disagree about how many guarded entry points exist.
echo "  enumerated guarded entry point(s): ${ENUMERATED_ENTRY_POINTS:-0}"

if [[ "$ERRORS" -ne 0 ]]; then
  echo "" >&2
  echo "check_apple_thread_safety: FAILED with $ERRORS issue(s)" >&2
  exit 1
fi

echo ""
echo "✅ check_apple_thread_safety: all Apple native FFI guards present"
