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
#   with `notify_native_redraw`, so the frame ledger learns about it.
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
#       announcement helpers (`note_canvas_redraw` / `note_native_redraw`).
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
# Removing a `note_native_redraw` pairing from an event handler must make this
# gate name that file. See the round's report for the exact output.
#
# Usage: tools/check_native_redraw_goes_through_the_runtime.sh
# Exit 0 = every platform-initiated redraw is announced to the library.
# Exit 1 = a finding (each unannounced call site is named).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/3] the announcement entry point exists"
if ! grep -qE '^pub fn notify_native_redraw' src/widget/runtime.rs; then
    echo "  FAIL  src/widget/runtime.rs no longer defines \`pub fn notify_native_redraw\`"
    echo "        It is the one way a platform tells the library about its own redraws."
    exit 1
fi
echo "  PASS  notify_native_redraw is the platform's entry into the frame ledger"

echo "[2/3] every toolkit redraw call site is announced or is the library's own"
# Collect the raw call sites, with their enclosing file, then decide by file.
RAW="$(grep -rn 'queue_draw()\|setNeedsDisplay: YES' src/platform/ --include=*.rs \
    | sed -e 's://.*::' || true)"

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
    # The name of the nearest preceding `fn` declaration is the enclosing function.
    enclosing="$(awk -v upto="$ln" '
        NR <= upto && match($0, /fn[[:space:]]+[A-Za-z0-9_]+/) {
            s = substr($0, RSTART, RLENGTH); sub(/fn[[:space:]]+/, "", s); name = s
        }
        END { print name }
    ' "$file")"
    case "$enclosing" in
        *repaint_canvas|*repaint_canvas_rect|*invalidate_surface*) continue ;;
    esac
    # The enclosing function must mention an announcement somewhere in its body. Reading
    # from the function's `fn` line to the next one is enough for the flat module layout
    # these backends use, and it is the shape a review reads anyway.
    body="$(awk -v name="$enclosing" '
        index($0, "fn " name "(") { inside = 1 }
        inside { print }
        inside && /^}$/ { exit }
    ' "$file")"
    if ! printf '%s' "$body" | grep -E 'note_canvas_redraw\(|note_native_redraw\(' \
        | sed -e 's://.*::' | grep -q .; then
        unannounced="${unannounced}${file}:${ln} in ${enclosing}()
"
    fi
done <<< "$RAW"

if [ -n "$unannounced" ]; then
    echo "  FAIL  these files queue more platform redraws than they announce:"
    printf '%s' "$unannounced" | sed 's/^/          /'
    echo "        Pair each toolkit redraw with a \`note_native_redraw(id)\` /"
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
    # Extract each invalidate_surface* body and look for an announcement inside.
    for fn in invalidate_surface_impl invalidate_surface_rect_impl; do
        body="$(awk -v name="$fn" '
            $0 ~ "fn " name "\\(" { inside=1 }
            inside { print }
            inside && /^    }$/ { exit }
        ' "$file" 2>/dev/null || true)"
        if printf '%s' "$body" | grep -qE 'note_canvas_redraw\(|note_native_redraw\('; then
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
