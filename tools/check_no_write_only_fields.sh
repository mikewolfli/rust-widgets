#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Gate: no struct field in the widget/style/layout layers may be written and never read
# (principle #99). See tools/check_no_write_only_fields.py for the rules, the reviewed
# allow-list and the reverse-injection record.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Resolve Python through the shared helper rather than exec'ing `python3`.
#
# On Windows, `python3` is often the Microsoft Store execution alias, which blocks on the
# Store UI instead of running the script — the gate hangs rather than failing (see
# tools/lib_python.sh). The helper probes candidates under a wall-clock bound and picks a
# real Python 3.
# shellcheck source=tools/lib_python.sh
. "$here/lib_python.sh"
exec "$PYTHON" "$here/check_no_write_only_fields.py" "$@"
