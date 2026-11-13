#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_native_redraw_goes_through_the_runtime.sh — BLUE24 §7 criterion 4
# ============================================================================
# The rule this guards:
#
#   **A platform redraw the library did not ask for must be announced to the
#   library.** `queue_draw` / `setNeedsDisplay:` may be called (the platform owns
#   those pixels and only it can repaint them), but the call site must be paired
#   with `notify_platform_surface_repaint`, so the frame ledger learns about it.
#
# The defect this stops
# ---------------------
# A mixed native/self-painted window has two worlds. The self-painted one routes
# every redraw through `request_repaint`, which records it against the frame. The
# native one called the platform's own redraw API directly, so three things broke
# at once (BLUE24 §7.2):
#
#   1. `performance::render_dirty_regions` computed damage the frame never
#      submitted, so "what did this frame paint?" had no answer;
#   2. the still-frame guarantee (an idle window submits nothing) could no longer
#      be shown, so "smooth is not paid for with a burning core" was unprovable;
#   3. the two worlds each kept their own count, so one interaction was submitted
#      twice.
#
# The fix is **not** to remove the platform calls — the platform must repaint its
# own pixels. It is to *tell the library*, which is what this gate insists on.
#
# The rule, stated structurally
# -----------------------------
# Under `src/platform/**`, every call to a toolkit redraw API (`queue_draw`,
# `setNeedsDisplay:`) is either:
#   (a) the body of `invalidate_surface*` — the **library** asked, and already
#       counted it, so announcing again would double-count; or
#   (b) a **platform-initiated** redraw, which must reach one of the two
#       announcement helpers (`note_canvas_redraw` / `note_platform_surface_repaint`).
#
# The two allowlisted files below hold the library-side functions; a new call site
# in any *other* function is a finding.
#
# What this gate does NOT prove
# -----------------------------
#   * It is lexical. It matches the call sites, not whether the announcement runs
#     on the path that reached them. The behavioural half is the runtime tests
#     (`a_native_redraw_is_counted_by_the_frame`).
#   * It cannot see a redraw the platform requests through some *other* API (a
#     layer invalidation, an animation tick). Those are not redraw requests this
#     crate can name, and inventing a list of them would be guessing.
#
# Reverse injection
# -----------------
# Removing a `note_platform_surface_repaint` pairing from an event handler must make this
# gate name that file. See the round's report for the exact output.
#
# Usage: tools/check_native_redraw_goes_through_the_runtime.sh
# Exit 0 = every platform-initiated redraw is announced to the library.
# Exit 1 = a finding (each unannounced call site is named).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# shellcheck source=tools/lib_source_scan.sh
. "$ROOT_DIR/tools/lib_source_scan.sh"

echo "[1/3] the announcement entry point exists"
if ! grep -qE '^pub fn notify_platform_surface_repaint' src/widget/runtime/window_map.rs; then
    echo "  FAIL  src/widget/runtime/window_map.rs no longer defines \`pub fn notify_platform_surface_repaint\`"
    echo "        It is the one way a platform tells the library about its own redraws."
    exit 1
fi
echo "  PASS  notify_platform_surface_repaint is the platform's entry into the frame ledger"

echo "[2/3] every toolkit redraw call site is announced or is the library's own"
# Collect the raw call sites, with their enclosing file, then decide by file.
RAW="$(grep -rn 'queue_draw()\|setNeedsDisplay: YES' src/platform/ --include=*.rs \
    | rw_strip_line_comments || true)"

if [ -z "$RAW" ] || [ "$(printf '%s' "$RAW" | tr -d '[:space:]' | wc -c)" -eq 0 ]; then
    echo "  FAIL  no toolkit redraw call sites found under src/platform/"
    echo "        Either the backends changed their API or the search is wrong; either"
    echo "        way this gate is measuring nothing and must not be trusted."
    exit 1
fi

# A call site is acceptable when the FILE also announces to the library. The
# platform's redraws all live in per-backend canvas modules, and each of those
# modules now carries the announcement helper; a backend that queues a redraw
# without ever announcing is the state this gate was written against.
unannounced=""
# Function-scoped, not file-scoped: the question is "did the *code that queued this redraw*
# tell the library?", and a file may legitimately hold both kinds (the library's own
# `repaint_canvas` and the platform's own event handlers). Counting per file failed on
# `platform_impl.rs`, where one release handler queues two redraws on one surface and one
# announcement is correct for both.
#
# The allowlist is the *library's* invalidation functions: they are reached from
# `request_repaint`, which already recorded the submission, so announcing there would count
# it twice. Everything else must announce.
# The allowlist is applied by the `case` below (the *library's* invalidation functions).
while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    file="${hit%%:*}"
    ln="${hit#*:}"; ln="${ln%%:*}"
    # The enclosing function is found structurally: brace depth plus an anchored `fn`
    # declaration. The old "nearest preceding line mentioning fn" search also matched doc
    # comments and named the wrong function when a comment explained a sibling.
    enclosing="$(rw_enclosing_function_name "$file" "$ln")"
    if [ -z "$enclosing" ]; then
        unannounced="${unannounced}${file}:${ln} is outside every function, so it has no announcement pairing
"
        continue
    fi
    case "$enclosing" in
        *repaint_canvas|*repaint_canvas_rect|*invalidate_surface*) continue ;;
    esac
    # The enclosing function must mention an announcement somewhere in its *own* body.
    # The body is sliced by brace balance (tools/lib_source_scan.sh): the previous slice ran
    # from the `fn` line to a column-0 `}`, which a method never has, so it ran to end of
    # file and let a neighbouring function's announcement vouch for this one (W6-15).
    start="$(rw_enclosing_function_line "$file" "$ln")"
    body=""
    if [ "${start:-0}" -gt 0 ]; then
        body="$(rw_slice_function_body "$file" "$start" || true)"
    fi
    # `grep -q` cannot end this pipeline: with `pipefail` set, its early exit closes the
    # pipe, the awk in `rw_strip_line_comments` dies of SIGPIPE, and the pipeline reports
    # failure for text that was present. Read the matches, then test the variable.
    announced="$(printf '%s\n' "$body" | rw_strip_line_comments \
        | grep -E 'note_canvas_redraw\(|note_platform_surface_repaint\(' || true)"
    if [ -z "$announced" ]; then
        unannounced="${unannounced}${file}:${ln} in ${enclosing}()
"
    fi
done <<< "$RAW"

if [ -n "$unannounced" ]; then
    echo "  FAIL  these files queue more platform redraws than they announce:"
    printf '%s' "$unannounced" | sed 's/^/          /'
    echo "        Pair each toolkit redraw with a \`note_platform_surface_repaint(id)\` /"
    echo "        \`note_canvas_redraw(id)\`: the platform must repaint its own pixels, but"
    echo "        the frame ledger has to learn about it (BLUE24 §7.2)."
    exit 1
fi
echo "  PASS  every backend that queues a redraw also announces it"

echo "[3/3] the library's own invalidation path does not double-count"
# `invalidate_surface*` is reached from `request_repaint`, which already records the
# submission. Announcing there too would count one submission twice, so the two
# announcement helpers must NOT be called from inside those functions.
double=""
while IFS= read -r file; do
    [ -f "$file" ] || continue
    # Extract each invalidate_surface* body and look for an announcement inside. Same
    # brace-aware slice as [2/3]: a fixed four-space `    }` window ends at the first nested
    # block that happens to close at that indent, which is not the function's boundary.
    for fn in invalidate_surface_impl invalidate_surface_rect_impl; do
        start="$(rw_function_line "$file" "$fn" || true)"
        [ -n "$start" ] || continue
        body="$(rw_slice_function_body "$file" "$start" || true)"
        [ -n "$body" ] || continue
        # Same SIGPIPE-free shape as [2/3].
        announced="$(printf '%s\n' "$body" | rw_strip_line_comments \
            | grep -E 'note_canvas_redraw\(|note_platform_surface_repaint\(' || true)"
        if [ -n "$announced" ]; then
            double="${double}${file}: ${fn} announces a redraw the library already counted
"
        fi
    done
done < <(grep -rlE 'fn invalidate_surface' src/platform/ --include=*.rs || true)

if [ -n "$double" ]; then
    echo "  FAIL  the library's own invalidation path announces again:"
    printf '%s' "$double" | sed 's/^/          /'
    echo "        That counts one submission twice. Only *platform-initiated* redraws"
    echo "        announce; the library's own are recorded by \`request_repaint\`."
    exit 1
fi
echo "  PASS  the library's invalidation path does not announce on top of its own count"

echo
echo "check_native_redraw_goes_through_the_runtime: OK"
