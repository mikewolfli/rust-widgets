#!/usr/bin/env bash
# A control that wires its events dynamically must resolve every name its capability publishes.
#
# # Why this gate exists
#
# `Widget::event_signal_dyn` is how a name written in a designer's project becomes a live
# subscription, and `EventSignalBinder::forward_all` wires a control by walking its capability's
# published events and asking the control to resolve each one. The two lists must therefore agree:
#
#   * a published name the control cannot resolve is a panel entry that does nothing — the wire is
#     silently dropped (`forward_all` counts it as unwired, which is honest but still a gap);
#   * a name the control resolves that the capability does not publish is dead code that looks like
#     support: `connect_event` refuses the name, so nothing can ever reach that arm.
#
# The second direction is the one that found a real defect the first time this ran: `check_box` and
# `slider` had a `clicked` arm while their capabilities publish no `clicked`, so the arm was
# unreachable. Both the arm and the reason it must not exist are now recorded in the control.
#
# # Scope
#
# Resolution is opt-in, so the check is scoped to the controls declared converted in
# `tools/check_event_signal_dyn.py`. That list is the commitment: adding a control means every
# published name it has must resolve, and removing one is a visible regression.
#
# # Reverse injection
#
# Step 2 runs the same check with one arm pretended missing and requires a failure. Step 1 alone
# would be satisfied by a comparison that never compares.
#
# Usage: tools/check_event_signal_dyn.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_event_signal_dyn.py; then
    echo "FAIL: a converted control does not resolve the events its capability publishes"
    exit 1
fi

# A non-zero exit is the expected outcome here, and it is the assertion rather than an error.
if "$PYTHON" tools/check_event_signal_dyn.py --inject=button.state_changed >/dev/null 2>&1; then
    echo "FAIL: pretending an arm is missing did not make the check fail, so it is not checking"
    exit 1
fi

echo "event wiring resolution checks passed."
