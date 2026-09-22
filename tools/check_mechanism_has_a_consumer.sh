#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# An abstraction must have at least one production consumer (BLUE21 E1 / P3-1g).
#
# # Why this gate exists
#
# The most expensive defect class in this crate is not a wrong value — it is a *correct* value that
# nothing calls. The mechanism is written, documented, unit-tested, and no control reaches it. From
# the source it reads as implemented; on screen it does nothing. One audit found seven such islands
# at once: `TouchTargetSize` (32/44/48/40, wired to nothing), `contains_point_with_touch_expansion`
# (called by no control), `resolve_style_for_state` (12 widget states nothing resolves),
# `LayoutContext::font_scale` and `min_touch_size` (each with one occurrence in the whole crate —
# its own declaration), and `AnimationDriver` / `AnimationGroup`, which leave all 1582 lines of
# `src/style/animation.rs` unreachable from `src/widget/`.
#
# Every other gate passes these: they compile, they are documented, and their own tests exercise
# them directly. Nothing asked "does production code call this?", which is the question.
#
# # What it asserts
#
# For each mechanism in the checker's table, at least one `src/` reference outside its own
# declaration file and outside the files that are part of the same mechanism. A reference inside a
# `#[cfg(test)]` block does not count: a test that calls a symbol proves it works, not that anything
# uses it, and counting those is exactly how a dead abstraction stays invisible.
#
# # Why the seven islands are acknowledged rather than failed
#
# They are in the tree this gate lands on, and connecting them is a feature plan of its own
# (BLUE21 P0-2 / P0-3 / P0-4 / P2-7), not a gate change. A gate that is green only after that work
# would be red today, and a permanently-red gate is one nobody reads — which is how the islands
# survived. So the checker carries an `ACKNOWLEDGED` table naming the plan item that will connect
# each one. That table is the ratchet: a **new** unconnected mechanism fails immediately, and an
# acknowledged one that later gains a consumer is reported as stale, so the list can only shrink.
#
# # Reverse injection
#
# Step 2 pretends a listed mechanism has acquired a consumer and requires the answer to change, so
# a search that matched nothing cannot pass.
#
# Usage: tools/check_mechanism_has_a_consumer.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_mechanism_has_a_consumer.py; then
    echo "FAIL: an abstraction is built and nothing calls it"
    exit 1
fi

# A non-zero exit is the expected outcome here, and it is the assertion rather than an error.
# Injecting a consumer for an acknowledged mechanism must make the checker call the
# acknowledgement stale, which is a failure — so an unchanged run means the search is not reading
# the code it reports on.
if "$PYTHON" tools/check_mechanism_has_a_consumer.py --inject=TouchTargetSize >/dev/null 2>&1; then
    echo "FAIL: injecting a consumer did not change the result, so the check is not comparing"
    exit 1
fi

echo "mechanism-consumer checks passed."
