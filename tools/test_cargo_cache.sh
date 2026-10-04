#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Exercises `tools/lib_cargo_cache.sh`: a cold run compiles, a warm run replays successful output,
# and changed source/environment inputs or failures cannot reuse a stale result.
set -uo pipefail
cd /Users/mikewolfli/Desktop/workspace/rust-widgets
. tools/lib_timeout.sh
. tools/lib_cargo_cache.sh
failures=0

echo "── cold run (must compile, not replay) ──"
start=$(date +%s)
rw_cargo_cached 600 check --no-default-features --features mini --lib >/tmp/c1.out 2>/tmp/c1.err
rc_cold=$?
echo "  rc=$rc_cold  elapsed=$(( $(date +%s) - start ))s"
if [ "$rc_cold" != "0" ] || grep -q "reused from cache" /tmp/c1.err; then
  echo "  FAIL cold run did not execute successfully"
  failures=$((failures + 1))
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
  failures=$((failures + 1))
fi
if [ "$rc_warm" != "0" ]; then
  echo "  FAIL warm run returned $rc_warm"
  failures=$((failures + 1))
fi

echo "── the caller must see identical output either way ──"
if diff -q /tmp/c1.out /tmp/c2.out >/dev/null; then
  echo "  PASS stdout identical"
else
  echo "  FAIL stdout differs"
  failures=$((failures + 1))
fi
if [ "$rc_cold" = "$rc_warm" ]; then
  echo "  PASS status identical ($rc_cold)"
else
  echo "  FAIL status differs: $rc_cold vs $rc_warm"
  failures=$((failures + 1))
fi

echo "── a source edit must force a recompile ──"
cp src/widget/metrics.rs /tmp/metrics.keep
printf '\n// cache invalidate probe\n' >> src/widget/metrics.rs
start=$(date +%s)
rw_cargo_cached 600 check --no-default-features --features mini --lib >/tmp/c3.out 2>/tmp/c3.err
rc_edit=$?
echo "  rc=$rc_edit  elapsed=$(( $(date +%s) - start ))s"
if [ "$rc_edit" != "0" ] || grep -q "reused from cache" /tmp/c3.err; then
  echo "  FAIL edited-source check failed or reused a cached result ($rc_edit)"
  failures=$((failures + 1))
else
  echo "  PASS source edit forced a real run"
fi
cp /tmp/metrics.keep src/widget/metrics.rs

echo "── an exported environment change must change the key ──"
export RW_CARGO_CACHE_TEST_INPUT=first
key_before="$(_rw_cache_key check --no-default-features --features mini --lib)"
export RW_CARGO_CACHE_TEST_INPUT=second
key_after="$(_rw_cache_key check --no-default-features --features mini --lib)"
if [ "$key_before" != "$key_after" ]; then
  echo "  PASS environment change invalidated the key"
else
  echo "  FAIL environment change did not invalidate the key"
  failures=$((failures + 1))
fi
unset RW_CARGO_CACHE_TEST_INPUT

echo "── a failing step must run again instead of caching its failure ──"
rw_cargo_cached 600 check --no-default-features --features no_such_feature_xyz >/tmp/c4.out 2>/tmp/c4.err
rc_bad=$?
rw_cargo_cached 600 check --no-default-features --features no_such_feature_xyz >/tmp/c5.out 2>/tmp/c5.err
rc_bad2=$?
if [ "$rc_bad" != "0" ] && [ "$rc_bad" = "$rc_bad2" ] \
  && ! grep -q "reused from cache" /tmp/c4.err \
  && ! grep -q "reused from cache" /tmp/c5.err; then
  echo "  PASS failures are rerun and remain failures ($rc_bad)"
else
  echo "  FAIL failure was cached or not reproduced: $rc_bad / $rc_bad2"
  failures=$((failures + 1))
fi

echo "── a timed-out step must run again instead of caching timeout ──"
timeout_bin="$(mktemp -d)"
timeout_path="$timeout_bin:$PATH"
printf '#!/usr/bin/env bash\nsleep 3\n' > "$timeout_bin/cargo"
chmod +x "$timeout_bin/cargo"
PATH="$timeout_path" rw_cargo_cached 1 check >/tmp/c6.out 2>/tmp/c6.err
rc_timeout=$?
PATH="$timeout_path" rw_cargo_cached 1 check >/tmp/c7.out 2>/tmp/c7.err
rc_timeout2=$?
rm -f "$timeout_bin/cargo"
rmdir "$timeout_bin"
if [ "$rc_timeout" = "124" ] && [ "$rc_timeout2" = "124" ] \
  && ! grep -q "reused from cache" /tmp/c6.err \
  && ! grep -q "reused from cache" /tmp/c7.err; then
  echo "  PASS timeouts are rerun rather than cached"
else
  echo "  FAIL timeout was cached or not returned: $rc_timeout / $rc_timeout2"
  failures=$((failures + 1))
fi

echo "── tree restored ──"
if diff -q /tmp/metrics.keep src/widget/metrics.rs >/dev/null; then
  echo "  PASS"
else
  echo "  FAIL tree dirty"
  failures=$((failures + 1))
fi
if [ "$failures" -eq 0 ]; then
  echo "PASS cargo cache checks"
else
  echo "FAIL cargo cache checks ($failures failure(s))" >&2
fi
exit "$(( failures > 0 ))"
