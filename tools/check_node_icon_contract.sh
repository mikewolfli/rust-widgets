#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_node_icon_contract.sh - the icon contract as the **Node** binding sees it
# ============================================================================
# The Rust tests exercise the property contract in-process and `check_abi.sh` pins the C symbols, so
# a defect in the binding's own layer — the JavaScript encoder/decoder, the id marshalling, the
# lifetime of a string out-slot — was invisible to every gate in this repository. That is not
# hypothetical: running this probe for the first time found four real defects in `index.js`
# (a BigInt argument `ref-napi` rejects, widget ids collapsing through `Number()`, a freed out-slot
# pointer that aborted the next ffi call, and a second `ref-napi` pulled in by a dead dependency that
# core-dumped the process). See `bindings/nodejs/README.md` and `docs/log/log-20261012-1.md`.
#
# # Why this gate can be skipped, and why that is spelled out
#
# It needs two things a CI box may legitimately lack:
#
#   1. a **Node >= 16** toolchain. The binding loads koffi, which ships prebuilt binaries, so no
#      compiler and no C++ toolchain is involved — Node 16 through 24 all pass this probe (measured
#      2026-10-13, 17 checks each). Before the koffi port the requirement was Node **16 exactly**,
#      because `ffi-napi@4.0.3` could not be compiled against the const-env N-API signature of 18+;
#      that constraint is gone with the dependency;
#   2. `bindings/nodejs/node_modules` — an `npm install` step this repository does not commit.
#
# Either absence is reported as `unsupported host ...` and exits non-zero, which is the marker the
# runner (tools/run_all_gates.sh) turns into `SKIP(host-limited)` rather than a pass or a failure.
# Printing the exact remedy matters: a skip that does not say how to stop skipping is how a gate
# stays unrun forever.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_timeout.sh"

BINDING_DIR="bindings/nodejs"
PROBE="$BINDING_DIR/verify_icon_contract.js"

if [ ! -f "$PROBE" ]; then
  echo "FAIL: $PROBE is missing; the Node-side icon contract has no verification" >&2
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "unsupported host: no \`node\` on PATH; install Node 16 or newer to run this gate" >&2
  exit 2
fi

# koffi ships prebuilt binaries and is N-API based, so any Node >= 16 runs the probe. Measured
# 2026-10-13 with the same `node_modules`: 16.20.2, 18.20.8, 20.20.2, 22.23.3 and 24.15.0 each pass all
# 17 checks. The Node-16-exactly rule disappeared with the ffi-napi dependency.
node_major="$(node -p 'process.versions.node.split(".")[0]')"
if [ "$node_major" -lt 16 ]; then
  echo "unsupported host: node $(node -v) is older than the 16 the binding and koffi require" >&2
  echo "  measured working: 16.20.2 through 24.15.0. Use \`nvm use 24\` (or any version in range)." >&2
  exit 2
fi

if [ ! -d "$BINDING_DIR/node_modules/koffi" ]; then
  echo "unsupported host: $BINDING_DIR/node_modules is absent or stale; run:" >&2
  echo "  cd $BINDING_DIR && npm install" >&2
  exit 2
fi

# The library the binding loads. Built here rather than assumed, so the gate cannot pass against a
# stale cdylib that predates the icons work.
#
# # Why the extension comes from `uname` and not a literal
#
# This read `target/debug/librust_widgets.so` unconditionally, which is the **Linux** spelling. On
# macOS cargo writes `librust_widgets.dylib`, so on that host the path never existed: the gate
# rebuilt the cdylib on every run (the `-f` test was always false) and then handed
# `RUST_WIDGETS_LIB_PATH` a path that did not exist, so the binding could not load anything and the
# gate failed. It was invisible because the gate sits behind a `node_modules` prerequisite, and on a
# host without that prerequisite it reports `unsupported host` (a SKIP) rather than failing — so the
# first run *with* the prerequisite is what found it.
case "$(uname -s)" in
  Darwin) LIBRARY_EXT=dylib ;;
  *) LIBRARY_EXT=so ;;
esac
LIBRARY="target/debug/librust_widgets.$LIBRARY_EXT"
if [ ! -f "$LIBRARY" ]; then
  echo "[1/2] building the cdylib the binding loads"
  rw_run_bounded "${RW_GATE_TIMEOUT:-900}" cargo build --no-default-features --features desktop
fi
# Asserted rather than assumed: a wrong extension must fail here, naming the path, instead of
# surfacing later as "the binding failed the icon contract" with the real cause buried in a dlopen
# list.
if [ ! -f "$LIBRARY" ]; then
  echo "FAIL: expected the cdylib at $LIBRARY after \`cargo build\`; the host's library extension" >&2
  echo "      may be neither .so nor .dylib (uname -s = $(uname -s)). Fix this gate's mapping." >&2
  exit 1
fi

echo "[2/2] running the binding's icon contract probe"
if (cd "$BINDING_DIR" && RUST_WIDGETS_LIB_PATH="../../$LIBRARY" node verify_icon_contract.js); then
  echo "check_node_icon_contract: OK"
else
  echo "check_node_icon_contract: the Node binding failed the icon contract" >&2
  exit 1
fi
