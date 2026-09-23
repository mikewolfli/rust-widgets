#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Exercises `tools/lib_cargo_cache.sh`: a cold run compiles, a warm run replays the *same* output
# and the *same* status, and an edited source re-keys so the answer can never be stale.
set -uo pipefail
cd /Users/mikewolfli/Desktop/workspace/rust-widgets
. tools/lib_timeout.sh
. tools/lib_cargo_cache.sh

echo "── cold run (must compile, not replay) ──"
start=$(date +%s)
rw_cargo_cached 600 check --no-default-features --features mini --lib >/tmp/c1.out 2>/tmp/c1.err
rc_cold=$?
echo "  rc=$rc_cold  elapsed=$(( $(date +%s) - start ))s"
if grep -q "reused from cache" /tmp/c1.err; then
  echo "  >> UNEXPECTED HIT on a cold run"
else
  echo "  PASS cold run really ran"
fi

echo "── warm run (must replay, not recompile) ──"
start=$(date +%s)
rw_cargo_cached 600 check --no-default-features --features mini --lib >/tmp/c2.out 2>/tmp/c2.err
rc_warm=$?
echo "  rc=$rc_warm  elapsed=$(( $(date +%s) - start ))s"
if grep -q "reused from cache" /tmp/c2.err; then
  echo "  PASS warm run was a cache hit"
else
  echo "  FAIL warm run recompiled"
fi

echo "── the caller must see identical output either way ──"
if diff -q /tmp/c1.out /tmp/c2.out >/dev/null; then
  echo "  PASS stdout identical"
else
  echo "  FAIL stdout differs"
fi
if [ "$rc_cold" = "$rc_warm" ]; then
  echo "  PASS status identical ($rc_cold)"
else
  echo "  FAIL status differs: $rc_cold vs $rc_warm"
fi

echo "── a source edit must force a recompile ──"
cp src/widget/metrics.rs /tmp/metrics.keep
printf '\n// cache invalidate probe\n' >> src/widget/metrics.rs
start=$(date +%s)
rw_cargo_cached 600 check --no-default-features --features mini --lib >/tmp/c3.out 2>/tmp/c3.err
rc_edit=$?
echo "  rc=$rc_edit  elapsed=$(( $(date +%s) - start ))s"
if grep -q "reused from cache" /tmp/c3.err; then
  echo "  FAIL stale answer served after a source edit"
else
  echo "  PASS source edit forced a real run"
fi
cp /tmp/metrics.keep src/widget/metrics.rs

echo "── a failing step must cache its failure, not its success ──"
rw_cargo_cached 600 check --no-default-features --features no_such_feature_xyz >/tmp/c4.out 2>/tmp/c4.err
rc_bad=$?
rw_cargo_cached 600 check --no-default-features --features no_such_feature_xyz >/tmp/c5.out 2>/tmp/c5.err
rc_bad2=$?
if [ "$rc_bad" != "0" ] && [ "$rc_bad" = "$rc_bad2" ]; then
  echo "  PASS a failure stays a failure ($rc_bad)"
else
  echo "  FAIL failure not reproduced: $rc_bad / $rc_bad2"
fi

echo "── tree restored ──"
diff -q /tmp/metrics.keep src/widget/metrics.rs >/dev/null && echo "  PASS" || echo "  FAIL tree dirty"
