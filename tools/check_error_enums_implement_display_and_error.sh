#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_error_enums_implement_display_and_error.sh — R-31 error contract
#
# The rule: a public `*Error` enum keeps its domain variants but must implement
# `Display` and `core::error::Error` (side crates: `std::error::Error`), so `?`,
# `Box<dyn Error>` and error chaining work the same for every public failure.
# Enums are NOT merged: the domains differ and merging would erase the type a
# caller matches on (principles #51/#54).
#
# The Python scanner documents its scope and self-tests its own patterns every
# run (principle #106).
#
# Usage: tools/check_error_enums_implement_display_and_error.sh
# Exit 0 = every public error enum has both impls.
# Exit 1 = a finding (file:line and the missing half).

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

STATUS=0
OUTPUT="$("$PYTHON" tools/check_error_enums_implement_display_and_error.py)" || STATUS=$?
printf '%s\n' "$OUTPUT"

if [[ "$STATUS" -ne 0 ]]; then
    exit "$STATUS"
fi

echo "check_error_enums_implement_display_and_error: OK"
