#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Exercises `tools/lib_cargo_cache.sh`: a cold run compiles, a warm run replays successful output,
# and changed source/environment inputs or failures cannot reuse a stale result.
#
# # Why the cache directory is private to this test
#
# The checks below need a *cold* first step, and the real cache (`target/gate-cache`) is normally
# warm — running against it made the "must compile, not replay" assertion depend on whether some
# earlier gate had already built this exact feature set. A private directory makes the run
# deterministic and keeps the probe from evicting or polluting entries a real gate run will reuse.
set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR" || exit 1

RW_CACHE_DIR="$(mktemp -d)"
export RW_CACHE_DIR
. tools/lib_timeout.sh
. tools/lib_cargo_cache.sh
cleanup() { rm -rf "$RW_CACHE_DIR"; }
trap cleanup EXIT
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

echo "── a source change must force a real run ──"
# A *new* file, not an edit to a tracked source file. This test can run while another writer is
# active in the same tree, and rewriting someone else's source file to prove a point is a good
# way to destroy their edit. Every file under the tree is part of the digest (`_rw_source_digest`
# hashes the whole build surface, minus build output), so adding one invalidates the key exactly
# as an edit does — and a `.txt` file can never be picked up by the build.
PROBE="src/rw_cache_invalidate_probe.txt"
rm -f "$PROBE"
printf 'first\n' > "$PROBE"
start=$(date +%s)
rw_cargo_cached 600 check --no-default-features --features mini --lib >/tmp/c3.out 2>/tmp/c3.err
rc_edit=$?
echo "  rc=$rc_edit  elapsed=$(( $(date +%s) - start ))s"
if [ "$rc_edit" != "0" ] || grep -q "reused from cache" /tmp/c3.err; then
  echo "  FAIL changed-source check failed or reused a cached result ($rc_edit)"
  failures=$((failures + 1))
else
  echo "  PASS a source change forced a real run"
fi
rm -f "$PROBE"

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

# The harness's own knobs are not compiler inputs. `run_all_gates.sh` documents raising them from
# the environment, and while they took part in the key that advice invalidated every cached step:
# a warm tree recompiled from scratch and read as a hang. Guard the exclusion here so it cannot
# regress into that.
echo "── harness budget knobs must NOT change the key ──"
key_plain="$(_rw_cache_key check --no-default-features --features mini --lib)"
key_gate="$(RW_GATE_TIMEOUT=1200 _rw_cache_key check --no-default-features --features mini --lib)"
key_run="$(RW_RUN_TIMEOUT=7200 _rw_cache_key check --no-default-features --features mini --lib)"
key_nocache="$(RW_GATE_NO_CACHE=1 _rw_cache_key check --no-default-features --features mini --lib)"
if [ "$key_plain" = "$key_gate" ] && [ "$key_plain" = "$key_run" ] && [ "$key_plain" = "$key_nocache" ]; then
  echo "  PASS RW_GATE_TIMEOUT / RW_RUN_TIMEOUT / RW_GATE_NO_CACHE leave the key unchanged"
else
  echo "  FAIL a harness knob changed the key:"
  echo "    plain   $(printf '%s' "$key_plain" | tr '\n' '/')"
  echo "    gate    $(printf '%s' "$key_gate" | tr '\n' '/')"
  echo "    run     $(printf '%s' "$key_run" | tr '\n' '/')"
  echo "    nocache $(printf '%s' "$key_nocache" | tr '\n' '/')"
  failures=$((failures + 1))
fi

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

echo "── probe file removed ──"
if [ ! -e "$PROBE" ]; then
  echo "  PASS"
else
  echo "  FAIL probe file left in the tree: $PROBE"
  failures=$((failures + 1))
fi
if [ "$failures" -eq 0 ]; then
  echo "PASS cargo cache checks"
else
  echo "FAIL cargo cache checks ($failures failure(s))" >&2
fi
exit "$(( failures > 0 ))"
