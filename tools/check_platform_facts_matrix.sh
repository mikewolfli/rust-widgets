#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_platform_facts_matrix.sh — BLUE28 B28.30 gate.
#
# The generated matrix must match the source. A new `Platform` method, a deleted override or a
# backend that stops using the shared state macro all change the generated document, so a stale
# copy means a reader is looking at a matrix the code no longer implements.

set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT" || exit 1

if python3 tools/generate_platform_facts_matrix.py --check; then
    echo "RESULT: PASS"
else
    echo "RESULT: FAIL"
    exit 1
fi
