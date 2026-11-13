#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_skill_copies_agree.sh — W4-22 residual
# ============================================================================
# The repository-specific maintenance guide exists twice:
#
#   * `.dsh/skills/rust-widgets-maintainer/SKILL.md` — the copy the agent reads;
#   * `dsh-plugin/assets/rust-widgets-maintainer/SKILL.md` — the copy the plugin ships.
#
# The two were synchronised by hand once (W4-22), and nothing checked them
# afterwards, so the drift that motivates the file could silently return. This
# gate compares the two copies byte for byte and names the first differing line.
#
# Reverse injection: edit either copy and the gate must exit 1 with that file
# named; restore it and the gate must exit 0.
#
# Usage: tools/check_skill_copies_agree.sh
# Exit 0 = the two copies are byte-identical.
# Exit 1 = a finding (the drift is shown).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

AGENT_COPY=".dsh/skills/rust-widgets-maintainer/SKILL.md"
PLUGIN_COPY="dsh-plugin/assets/rust-widgets-maintainer/SKILL.md"

missing=0
for copy in "$AGENT_COPY" "$PLUGIN_COPY"; do
    if [[ ! -f "$copy" ]]; then
        echo "missing skill copy: $copy" >&2
        missing=1
    fi
done
if [[ "$missing" -ne 0 ]]; then
    exit 1
fi

if cmp -s "$AGENT_COPY" "$PLUGIN_COPY"; then
    echo "✅ skill copies agree: $AGENT_COPY == $PLUGIN_COPY"
    exit 0
fi

echo "skill copies have drifted; the two must say the same thing (W4-22):" >&2
echo "  agent:  $AGENT_COPY" >&2
echo "  plugin: $PLUGIN_COPY" >&2
echo >&2
diff -u "$AGENT_COPY" "$PLUGIN_COPY" >&2 || true
exit 1
