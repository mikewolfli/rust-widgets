#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_modal_loop_frame_heartbeat.sh — the drag/resize drain stays closed
# ============================================================================
# The rule this guards:
#
#   **A window drag or a live resize must not stop the frame.** While the user holds the mouse on
#   a window's border (or title bar), the OS runs a *modal* loop of its own: AppKit pumps the run
#   loop in `NSEventTrackingRunLoopMode` and `sendEvent:` does not return; Win32 runs the
#   `WM_ENTERSIZEMOVE` … `WM_EXITSIZEMOVE` loop inside `DefWindowProc`, so the backend's own loop
#   body — the one that calls `crate::drive_frame` — is not reached until the mouse is released.
#
# Measured, not hypothetical: both macOS backends drove frames only from their own loop, so for
# the whole drag nothing drained the trigger queue. The `Resized` trigger `windowDidResize:`
# queues was never dispatched, the client area kept the layout it had when the drag began, and no
# animation advanced. The cocoa backend's heartbeat was an `NSTimer` that
# `scheduledTimerWithTimeInterval:` had put in `NSDefaultRunLoopMode` — a mode the tracking loop
# does not run — so even its timer did not fire.
#
# # What this proves (5 assertions)
#
#   1. The shared macOS heartbeat enrols its timer in `kCFRunLoopCommonModes`. That set is exactly
#      {default, modal panel, event tracking}, which is what makes the heartbeat fire *inside* the
#      drag. A heartbeat in the default mode alone is the defect.
#   2. The cocoa backend installs that heartbeat for every mode (it has no other frame driver), and
#      the objc2 backend installs it for modal loops only (its own loop is the driver, and two
#      drivers would tick twice per interval).
#   3. Both backends' start-up paths reach the installer, so the heartbeat is not a library that
#      nothing calls.
#   4. Win32's window procedure brackets the modal size/move loop with a timer: `WM_ENTERSIZEMOVE`
#      starts it, `WM_EXITSIZEMOVE` kills it, and `WM_TIMER` drives the frame. Without the pair,
#      the timer would either never exist or leak into an idle window's wake rate.
#   5. Every modal-loop frame goes through `frame_wait::drive_once`, so a nested pump (a host's
#      modal dialog opened from a trigger handler) cannot start a second frame and advance every
#      animation at double speed.
#
# # Reverse injection
#
# Deleting the `kCFRunLoopCommonModes` enrolment, either `install(...)` call, one of the two Win32
# lifecycle arms, or the `drive_once` wrapper must each be reported by name. The gate reads the
# source rather than running it because the behaviour needs AppKit or Win32 to execute, and this
# repository's CI host is neither.
#
# Usage: tools/check_modal_loop_frame_heartbeat.sh
# Exit 0 = the drag/resize heartbeat is closed on every backend that needs one.
# Exit 1 = a finding is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# shellcheck source=tools/lib_source_scan.sh
. "$ROOT_DIR/tools/lib_source_scan.sh"

HEARTBEAT="src/platform/macos_heartbeat.rs"
COCOA="src/platform/macos/canvas.rs"
OBJC2="src/platform/macos_objc2/platform_impl.rs"
WINDOWS="src/platform/windows/types.rs"
GUARD="src/platform/frame_wait.rs"

failures=0
ok() { printf '  ✅ %s\n' "$*"; }
bad() { printf '  ❌ %s\n' "$*" >&2; failures=$((failures + 1)); }

# The source with `//` comments removed — the shared helper, so this gate and its siblings
# strip comments with identical semantics (W6-36).
#
# The negative assertions below must not read prose: these files *describe* the defect they fixed
# (`scheduledTimerWithTimeInterval:` in `NSDefaultRunLoopMode`), and a gate that matched its own
# documentation would report the module as broken for explaining itself.

# Whether `$1`'s code (comments stripped) matches the extended regex `$2`.
#
# Written as "read the output, then test it" rather than `rw_strip_line_comments … | grep -q`:
# with `pipefail` set, `grep -q` exiting at its first match closes the pipe, the stripper dies
# of SIGPIPE, and the *pipeline* reports failure — so a `grep -q` here would answer "no match"
# for text that was present. That is the exact shape the reverse-injection run caught.
code_has() {
  [ -n "$(rw_strip_line_comments "$1" | grep -E "$2")" ]
}

for file in "$HEARTBEAT" "$COCOA" "$OBJC2" "$WINDOWS" "$GUARD"; do
  if [ ! -f "$file" ]; then
    bad "$file is missing; this gate's subject is gone"
  fi
done

echo "[1/5] the macOS heartbeat lives in the run-loop modes AppKit's modal loops run"
# Every assertion below reads the source with comments stripped, and names the *call* rather than a
# symbol that an import line would satisfy. Both are reverse-injection findings: a commented-out
# `add_timer` and a deleted `KillTimer` both used to leave this gate green.
if code_has "$HEARTBEAT" 'add_timer\(Some\(&timer\),.*kCFRunLoopCommonModes'; then
  ok "the timer is added to the main run loop with kCFRunLoopCommonModes"
else
  bad "$HEARTBEAT does not add its timer in kCFRunLoopCommonModes; a drag would not drain"
fi
# The two spellings that *were* the defect: a scheduled NSTimer, and the default mode. Either one
# coming back means the heartbeat stops exactly where it is needed.
if code_has "$HEARTBEAT" 'scheduledTimerWithTimeInterval|NSDefaultRunLoopMode'; then
  bad "$HEARTBEAT schedules its timer the old way (default mode only); see this gate's header"
else
  ok "the heartbeat does not fall back to a default-mode-only timer"
fi

echo "[2/5] each macOS backend asks for the scope its own loop needs"
if code_has "$COCOA" 'HeartbeatScope::EveryMode'; then
  ok "the cocoa backend installs the heartbeat for every mode (it has no other frame driver)"
else
  bad "$COCOA no longer installs the every-mode heartbeat; its frames stop during a drag"
fi
if code_has "$OBJC2" 'HeartbeatScope::ModalLoopsOnly'; then
  ok "the objc2 backend installs the heartbeat for modal loops only (no double tick)"
else
  bad "$OBJC2 no longer installs the modal-loops-only heartbeat; its frames stop during a drag"
fi

echo "[3/5] both backends' start-up paths reach the installer"
if code_has "$COCOA" 'macos_heartbeat::install' \
  && code_has "$OBJC2" 'macos_heartbeat::install'; then
  ok "both macOS backends call macos_heartbeat::install"
else
  bad "a macOS backend links the heartbeat module without installing it"
fi
if grep -q 'fn install_frame_heartbeat' "$COCOA"; then
  ok "the cocoa backend still installs its heartbeat from run()"
else
  bad "$COCOA lost install_frame_heartbeat, which run() calls"
fi

echo "[4/5] Win32 brackets its modal size/move loop with a timer"
if code_has "$WINDOWS" 'SetTimer\(hwnd, FRAME_HEARTBEAT_TIMER_ID'; then
  ok "WM_ENTERSIZEMOVE starts the frame heartbeat"
else
  bad "$WINDOWS does not start a heartbeat when Win32 takes the mouse; a drag would not drain"
fi
if code_has "$WINDOWS" 'KillTimer\(hwnd, FRAME_HEARTBEAT_TIMER_ID'; then
  ok "WM_EXITSIZEMOVE kills it, so an idle window gains no wake-up"
else
  bad "$WINDOWS leaves the drag heartbeat running after the drag"
fi
if code_has "$WINDOWS" 'WM_TIMER if wparam == FRAME_HEARTBEAT_TIMER_ID'; then
  ok "WM_TIMER for that id drives the frame"
else
  bad "$WINDOWS never drives a frame from the drag heartbeat's WM_TIMER"
fi

echo "[5/5] a nested pump cannot start a second frame"
if code_has "$HEARTBEAT" 'drive_once\(\|\|' \
  && code_has "$WINDOWS" 'drive_once\(\|\|'; then
  ok "both modal-loop heartbeats drive through frame_wait::drive_once"
else
  bad "a modal-loop heartbeat calls drive_frame directly, so a nested pump would tick twice"
fi
if grep -q 'pub(crate) fn drive_once' "$GUARD"; then
  ok "the shared guard exists in $GUARD"
else
  bad "$GUARD no longer defines drive_once"
fi
if code_has "$HEARTBEAT" 'heartbeat_may_drive\('; then
  ok "the mode rule is read through the tested predicate"
else
  bad "$HEARTBEAT decides its own mode rule inline; the predicate's tests cover nothing"
fi

if [ "$failures" -ne 0 ]; then
  echo "check_modal_loop_frame_heartbeat: $failures finding(s)" >&2
  exit 1
fi
echo "modal-loop frame heartbeat: OK (macOS common modes + Win32 size/move loop, both guarded)"
