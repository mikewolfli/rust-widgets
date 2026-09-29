#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Gate: no struct field in the widget/style/layout layers may be written and never read
# (principle #99). See tools/check_no_write_only_fields.py for the rules, the reviewed
# allow-list and the reverse-injection record.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$here/check_no_write_only_fields.py" "$@"
