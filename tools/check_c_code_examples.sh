#!/usr/bin/env bash
# C code example gate for the cookbook.
#
# The cookbook is where a C caller learns the ABI, and until now nothing compiled
# the C in it: a snippet could name a function that does not exist, or contradict
# the published header, and the prose would stay confidently wrong (principle
# #18). `check_cookbook.sh` closes that hole for *Rust* declarations only, and
# explicitly does not compile prose code blocks; `check_abi.sh` verifies the
# header against the Rust exports but never reads the cookbook.
#
# This gate compiles every ```c block as its own translation unit with
# `cc -fsyntax-only`, so a snippet is checked against the real C grammar rather
# than by reading it.
#
#   [1] EXTRACT   pull every ```c fenced block out of every cookbook language.
#   [2] COMPILE   wrap each in a translation unit and syntax-check it.
#   [3] AGREEMENT every `rw_*` name the blocks mention must exist in
#                 `include/rw_generated.h`. Syntax checking alone would accept a
#                 block that declares a function the library does not have, or
#                 declares one with a *different* signature, so this direction is
#                 what makes the snippets contract-checked rather than merely
#                 well-formed.
#
# Blocks that are deliberately illustrative are skipped by an explicit
# `<!-- c-example: skip -->` marker on or before the fence — an inline `...` is no
# longer enough, because ellipses also appear in blocks meant to compile. Each
# skip is printed, so the exemption list is visible rather than silent.
#
# Usage: tools/check_c_code_examples.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

fail=0

echo "[1/3] extract + compile every \`\`\`c block in the cookbook"
if ! "$PYTHON" tools/check_c_code_examples.py --compile; then
    echo "FAIL: a cookbook C block does not compile"
    fail=1
else
    echo "OK   every compilable C block is syntactically valid"
fi

echo "[2/3] every name a C block mentions must exist in the published header"
if ! "$PYTHON" tools/check_c_code_examples.py --agree --header include/rw_generated.h; then
    echo "FAIL: a cookbook C block names something the header does not declare"
    fail=1
else
    echo "OK   every \`rw_*\` name in the cookbook exists in include/rw_generated.h"
fi

echo "[3/3] the published header itself must compile as C"
if ! "$PYTHON" tools/check_c_code_examples.py --header-check; then
    echo "FAIL: include/rw_generated.h does not compile standalone"
    fail=1
else
    echo "OK   include/rw_generated.h compiles standalone"
fi

if [ "$fail" -ne 0 ]; then
    echo "FAIL: C code example checks failed"
    exit 1
fi
echo "C code example checks passed."
