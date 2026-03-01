#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

python3 - <<'PY'
import re
import sys
from pathlib import Path

expected = [
    "DEMO_PROFILE",
    "ABI_VERSION",
    "TARGET_FPS",
    "APPLIED_FPS",
    "TASK_ID",
    "WINDOW_ID",
    "BUTTON_ID",
    "ENGINE_INITIALIZED",
    "ENGINE_RUNNING",
    "FRAME_COUNT",
    "PENDING_TASK_COUNT",
    "WINDOW_COUNT",
    "BUTTON_COUNT",
]

checks = [
    (
        Path("examples/c_abi_embedded_engine_demo.c"),
        re.compile(r'printf\("([A-Z_]+)='),
        "C demo",
    ),
    (
        Path("examples/python/demo_embedded_engine.py"),
        re.compile(r'print\(f?"([A-Z_]+)='),
        "Python demo",
    ),
    (
        Path("examples/java/RustWidgetsEmbeddedEngineDemo.java"),
        re.compile(r'System\.out\.println\("([A-Z_]+)='),
        "Java demo",
    ),
]

failed = False
for file_path, pattern, label in checks:
    text = file_path.read_text(encoding="utf-8")
    found = [match.group(1) for match in pattern.finditer(text)]
    if found != expected:
        failed = True
        print(f"[FAIL] {label}: output schema mismatch")
        print(f"  file: {file_path}")
        print(f"  expected: {expected}")
        print(f"  found:    {found}")
    else:
        print(f"[OK] {label}: schema and order match")

if failed:
    sys.exit(1)

print("embedded demo schema checks passed")
PY
