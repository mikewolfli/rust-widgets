#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
# ============================================================================
# check_status_docs_name_real_types.sh
# ============================================================================
# A backend's `status.md` is what a host reads before choosing a build, and no
# gate read it — which is how `src/platform/macos/status.md` kept claiming
# per-kind `NSButton`/`NSTextField` controls, an `NSMenu` menu bar and
# `NSAlert` dialogs for two backends that construct none of them, and how
# `src/platform/android/status.md` kept a native-view inventory for a factory
# that BLUE15 deleted.
#
# Three checks, because the same defect has three spellings:
#
#   1. An ✅ row naming a **native type** the backend's sources never mention.
#   2. An ✅ row pairing a logical kind with an **OS widget class** (`UIButton`,
#      `android.widget.Button`, `gtk::Notebook`, …). The library paints every
#      `WidgetKind` itself on every backend, so no such mapping exists — this is
#      the framing the user corrected, and its residue was in three documents.
#   3. A **`nativeCreate*` / `nativeSetView*`** symbol that `src/bindings/java_jni.rs`
#      does not export.
#
# Also covers `src/platform/android/activity_integration.md`, which carried the same
# class of claim — a `Rust factory` column naming `create_native_view` on every row
# (deleted under BLUE15) and an entire `nativeSetView*` family that was never written.
#
# A row that names any of these **in order to deny it** ("no `android.app.AlertDialog`
# is constructed", "⛔ Deleted", "used to") is skipped: that is true history, and
# reporting it would be reporting the correction as the defect.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" tools/check_status_docs_name_real_types.py

echo "✅ platform docs: no ✅ row claims a native control or widget mapping; every JNI symbol"
echo "   a doc names is actually exported"
