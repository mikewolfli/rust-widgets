#!/usr/bin/env bash
# Every published capability event must resolve to a signal its own widget emits.
#
# # Why this gate exists
#
# `properties.rs` publishes event *names* per capability, and `WidgetFactory::connect_event`
# accepts a name by comparing it against that list before subscribing on the hub. A name the
# table publishes but no signal backs is therefore accepted and never delivered: the consumer
# gets a live-looking subscription, not an error.
#
# Two defects of exactly that shape were found by hand — `button`/`toggle_button` publishing
# `pressed`/`released` for `pressed_signal`/`released_signal`, and `find_replace_dialog`
# publishing five names nothing emits. The same audit run found four more:
#
#   * `auto_complete_edit` published `changed`/`selected`; the signals are `text_changed` and
#     `suggestion_selected`.
#   * `animated_image`, `lottie_widget`, `rive_widget` published `finished`; each emits
#     `animation_finished` (and `frame_changed`, for the first).
#   * `video_player` published `finished`; it emits `playback_started`, `playback_paused`,
#     `playback_ended` and `time_updated`.
#
# No existing gate compared the three sides: the published name, the signal that carries it, and
# the `.emit()` that fires it.
#
# The check is `tools/check_capability_events_are_emitted.py`; see its docstring for why the
# emission side is attributed per struct rather than per repository (a name emitted by *another*
# control is not a producer for this one).
#
# Usage: tools/check_capability_events_are_emitted.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_capability_events_are_emitted.py; then
    echo "FAIL: a published capability event cannot be delivered"
    exit 1
fi

echo "capability event checks passed."
