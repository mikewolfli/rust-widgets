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
#      `setHidden:`; it uses `setFrame:display:` / `orderOut:`).
#   C. No Apple native module may use `performSelector:withObject:` in code
#      (the selector returns `id`, which objc2 rejects against a `()` binding).
#   D. The macOS objc2 and cocoa-legacy entry points must keep a healthy number
#      of main-thread guards (regression-by-omission tripwire).
#
# The runtime halves of the same verification live in:
#   examples/apple_appkit_probe.rs   (macOS, main thread)
#   bindings/ios/main.m              (iOS Simulator)
#   tools/check_apple_native.sh      (runs both)
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
OBJC2_NATIVE="src/platform/macos_objc2/native.rs"
OBJC2_IMPL="src/platform/macos_objc2/platform_impl.rs"
IOS_NATIVE="src/platform/ios/native.rs"

for f in "$MACOS_IMPL" "$OBJC2_NATIVE" "$OBJC2_IMPL" "$IOS_NATIVE"; do
  [[ -f "$f" ]] || error "expected Apple native file missing: $f"
done
if [[ "$ERRORS" -ne 0 ]]; then
  echo "check_apple_thread_safety: FAILED (missing files)" >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# A. set_native_* helpers must declare a main-thread guard near their top.
#    We look at the source window from the signature to the next `pub(crate) fn`
#    / `pub fn` / `fn ` at column 0, which is a reliable statement boundary.
# ---------------------------------------------------------------------------
echo "--- [A] set_native_* helpers carry a main-thread guard ---"
check_setters() {
  local file="$1"
  local total=0
  # Line numbers of every top-level fn definition.
  local lines
  lines="$(grep -nE '^(pub\(crate\) )?fn ' "$file" | cut -d: -f1)"
  while IFS=$'\t' read -r start name; do
    [[ -n "$start" ]] || continue
    [[ "$name" == set_native_* ]] || continue
    total=$((total + 1))
    # Window: from the signature line to the start of the next top-level fn,
    # capped at 40 lines so a stray brace cannot swallow the whole file.
    local next
    next="$(echo "$lines" | awk -v s="$start" '$1 > s {print $1; exit}')"
    [[ -n "$next" ]] || next=$((start + 40))
    local end=$((next - 1))
    [[ $((end - start)) -gt 40 ]] && end=$((start + 40))
    local body
    body="$(sed -n "${start},${end}p" "$file")"
    if ! echo "$body" | grep -qE 'MainThreadMarker::new\(\)|is_main_thread\(\)'; then
      error "$file:$start: $name has no main-thread guard"
    fi
  done < <(paste <(echo "$lines") <(grep -oE '^(pub\(crate\) )?fn [a-zA-Z0-9_]+' "$file" | sed -E 's/.*fn //'))
  echo "  checked $total setter(s) in $file"
}

check_setters "$OBJC2_NATIVE"
check_setters "$IOS_NATIVE"

# ---------------------------------------------------------------------------
# B. macOS objc2 frame/hidden setters must branch on NSWindow.
# ---------------------------------------------------------------------------
echo "--- [B] macOS objc2 frame/hidden setters are window/view aware ---"
for fname in set_native_frame set_native_hidden; do
  start="$(grep -nE "^pub\(crate\) fn $fname\b" "$OBJC2_NATIVE" | head -1 | cut -d: -f1)"
  if [[ -z "$start" ]]; then
    error "$OBJC2_NATIVE: $fname not found"
    continue
  fi
  body="$(sed -n "${start},$((start + 45))p" "$OBJC2_NATIVE")"
  if ! echo "$body" | grep -q 'isKindOfClass'; then
    error "$OBJC2_NATIVE:$start: $fname does not branch on NSWindow (isKindOfClass)"
  fi
done
grep -q 'setFrame: rect, display:' "$OBJC2_NATIVE" \
  || error "$OBJC2_NATIVE: window path must use setFrame:display:"
grep -q 'orderOut:' "$OBJC2_NATIVE" \
  || error "$OBJC2_NATIVE: window hide path must use orderOut:"

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
guards="$(grep -c 'objc2::MainThreadMarker::new()' "$OBJC2_IMPL" || true)"
if [[ "${guards:-0}" -lt 10 ]]; then
  error "$OBJC2_IMPL: expected >=10 MainThreadMarker guards, found ${guards:-0}"
fi
echo "  MainThreadMarker guards in $OBJC2_IMPL: ${guards:-0}"

guards="$(grep -c 'super::types::is_main_thread()' "$MACOS_IMPL" || true)"
if [[ "${guards:-0}" -lt 20 ]]; then
  error "$MACOS_IMPL: expected >=20 is_main_thread() guards, found ${guards:-0}"
fi
echo "  is_main_thread() guards in $MACOS_IMPL: ${guards:-0}"

if [[ "$ERRORS" -ne 0 ]]; then
  echo "" >&2
  echo "check_apple_thread_safety: FAILED with $ERRORS issue(s)" >&2
  exit 1
fi

echo ""
echo "✅ check_apple_thread_safety: all Apple native FFI guards present"
