#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# lib_cargo_cache.sh — run a `cargo` verification step once per build identity
# ============================================================================
# # The problem this solves
#
# `tools/run_all_gates.sh` walks every `tools/check_*.sh`, and several of them need the *same*
# compilation. Measured statically across the tree: `cargo check --features mini` is run by four
# gates, `--features embedded` by five, `--features desktop` by fourteen. `cargo`'s own target-dir
# cache cannot help, because a different feature set is a **different compilation unit** — so a
# full sweep recompiles the same configuration over and over. That is the "every run takes
# forever" report, and the fix is to make the second caller reuse the first caller's answer.
#
# # ⚠️ The one thing this must never do
#
# A cache that returns a stale answer makes a gate pass for code it never looked at, and unlike an
# ordinary false green it is **invisible**: the output says the step ran and succeeded. So the key
# is built from everything that can change a compilation's verdict:
#
#   * the exact `cargo` argv (feature set, targets, profile flags);
#   * `Cargo.lock` (dependency graph);
#   * `Cargo.toml` (feature definitions — a `features` edit changes what a flag means);
#   * the toolchain version (`rustc -Vv`);
#   * a digest of the source tree (`src/`, `examples/`, `tests/`, `benches/`) **without** the build
#     directory.
#
# Keying on the source digest is what makes this safe rather than merely fast: touching a source
# file changes the key, so the step reruns. The cache is therefore not "skip the work", it is
# "don't repeat work that provably has the same inputs".
#
# # What it is NOT
#
# It is not a way to make a gate check less. A cached step stores both the exit status **and** the
# captured output, so the caller still sees exactly the output it would have seen, and still fails
# for the same reason. `--no-cache` (or `RW_GATE_NO_CACHE=1`) bypasses it, which is how you debug a
# step you do not trust.
#
# # `rw_cargo_cached` is for **pure** steps only — a generator must not use it
#
# The cache stores an exit status and the two output streams. Replaying them is indistinguishable
# from re-running the command **except for side effects**, which a replay does not perform. That is
# fine for every step this file was written for (`check`, `test`, `clippy` — they write only inside
# `target/`), and it is a real defect for a step that writes into the working tree.
#
# Measured: `tools/check_generated_sources.sh` calls `designer_generate` through this cache, and
# `designer_generate` **writes** `examples/generated_project/src/generated/*.rs`. Its
# `REGENERATE_IN_PLACE` does `rm -rf $dir` and then runs the generator. On a cache hit the generator
# never runs, so the directory is left **deleted** and the next step reports "commits no generated
# source" — a false failure caused entirely by the cache, and one that only appears once the cache
# is warm. `RW_GATE_NO_CACHE=1` hid it, which is why it survived until the cache was populated.
#
# The rule: a step whose purpose is to *produce* files must not be cached. Run it directly (or with
# `RW_GATE_NO_CACHE=1` in scope) so the side effect actually happens.
#
# # Usage
#
#   . "$ROOT_DIR/tools/lib_cargo_cache.sh"
#   rw_cargo_cached check --no-default-features --features mini --all-targets
#
# Exit status is the cargo invocation's own, or 124 when the bound fired.

# Where the cache lives. Inside `target/` so `cargo clean` removes it too, and so it is already
# excluded from the source digest below.
#
# # Why the directory is created eagerly
#
# `target/` may not exist yet on a fresh checkout, and a cache that assumed it did would fail on
# the first run rather than helping. Creating both levels up front is also what makes the entry
# writes below unconditional — a partially-created cache is worse than no cache, because the hit
# test would find one file of an entry and report a verdict that was never computed.
RW_CACHE_DIR="${RW_CACHE_DIR:-target/gate-cache}"
mkdir -p "$RW_CACHE_DIR" 2>/dev/null || true

# Set RW_GATE_NO_CACHE=1 to bypass (debugging a step whose result you distrust).
RW_GATE_NO_CACHE="${RW_GATE_NO_CACHE:-0}"

# The digest of everything that can change a compilation's verdict.
#
# `git hash-object` is not used: the tree is often dirty while gates run, and a cache that only
# worked on a clean checkout would be useless exactly when it is most wanted. A content digest
# over the tracked source directories is both correct and cheap.
#
# `_rw_source_digest` prints one hex string. It is deliberately tolerant of a missing directory
# (a stripped checkout may have no `benches/`), because a cache key that errors is worse than a
# cache key that is merely conservative.
_rw_source_digest() {
  {
    # `find | sort` so the digest does not depend on directory iteration order.
    find src examples tests benches -type f \
      \( -name '*.rs' -o -name '*.toml' \) 2>/dev/null | LC_ALL=C sort
    # The manifests and the lockfile: a dependency or feature change re-keys the cache.
    printf '%s\n' Cargo.toml Cargo.lock rust-toolchain.toml 2>/dev/null
  } | while IFS= read -r file; do
    [ -f "$file" ] && printf '%s\0' "$file" && cat "$file"
  done | _rw_digest_stream
}

# A content digest, with the algorithm chosen by what the host actually has.
#
# `shasum -a 256` and `sha256sum` are not both present on every host (`lib_timeout.sh` records the
# same class of problem for GNU `timeout`), and a missing hasher that silently degraded to
# "constant" would make every key identical — the one failure mode this file exists to prevent. So
# a missing hasher is a hard error, not a fallback to something weaker.
_rw_digest_stream() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum | awk '{print $1}'
  else
    echo "lib_cargo_cache: no sha256 tool (shasum / sha256sum) on this host; refusing to run" >&2
    return 3
  fi
}

# The full cache key for a cargo argv: toolchain + argv + source digest.
#
# # Why this prints with `printf` rather than relying on the last command's status
#
# A function used as `key="$(f)"` inherits whatever status its last command produced, and callers
# here test it with `if ! key="$(...)"`. The last command is a pipe (`… | _rw_digest_stream`), and
# under `set -o pipefail` a pipe's status is the rightmost **non-zero** one — so a hasher that
# exits 0 still yields a non-zero function status whenever `find` reports a missing directory or
# `while read` hits EOF oddly. That made every call look like a failure and silently sent the
# caller down its uncached path, which is why the first version never cached anything. Ending on a
# `printf` makes the status depend only on the printf.
_rw_cache_key() {
  local toolchain
  # `rustc -Vv` rather than `cargo -V`: the compiler is what produces the artifacts.
  toolchain="$(rustc -Vv 2>/dev/null | tr '\n' ' ')"
  local argv_digest
  argv_digest="$(printf '%s\n%s\n' "$toolchain" "$*" | _rw_digest_stream)"
  local source_digest
  source_digest="$(_rw_source_digest)"
  printf '%s\n%s\n' "$argv_digest" "$source_digest"
}

# rw_cargo_cached <seconds> <cargo args…>
#
# Runs cargo under a wall-clock bound, reusing a previous run's status and output when the key
# matches. Prints the captured output to stdout (and, for a cached failure, the cached stderr to
# stderr) so the caller sees what it would have seen.
rw_cargo_cached() {
  local budget="${1:-900}"
  shift || true
  if [ "$#" -eq 0 ]; then
    echo "rw_cargo_cached: no cargo arguments given" >&2
    return 2
  fi

  if [ "$RW_GATE_NO_CACHE" = "1" ]; then
    rw_run_bounded "$budget" cargo "$@"
    return $?
  fi

  local key
  if ! key="$(_rw_cache_key "$@")"; then
    echo "rw_cargo_cached: could not compute a cache key; running uncached" >&2
    rw_run_bounded "$budget" cargo "$@"
    return $?
  fi
  # A key is two lines (argv digest + source digest); join so it is one filename component.
  local slug
  slug="$(printf '%s' "$key" | tr '\n' '-')"

  local entry="$RW_CACHE_DIR/$slug"
  if [ -f "$entry.status" ]; then
    # A hit: replay both streams and return the stored status. Replaying rather than staying
    # silent is what keeps a cached run auditable — a reviewer reading the gate log sees the same
    # lines a cold run prints.
    [ -f "$entry.out" ] && cat "$entry.out"
    [ -f "$entry.err" ] && cat "$entry.err" >&2
    local cached_status
    cached_status="$(cat "$entry.status")"
    echo "  (cargo result reused from cache: cargo $*)" >&2
    return "$cached_status"
  fi

  mkdir -p "$RW_CACHE_DIR"
  # Write to temp files and rename, so a step killed by its own bound cannot leave a
  # half-written entry that a later run would read as a verdict.
  #
  # Two separate `mktemp` calls rather than one plus a suffix: asking `mktemp` for
  # `name.XXXXXX.out` creates only `name.XXXXXX.out`'s directory entry semantics once, and the
  # `mv` below then moved a file the redirection had never created — the first version failed with
  # `cat: …/.pending.XXXXXX.out: No such file or directory` on the **second** cargo step of the
  # gate, which is exactly the kind of bug a rewrite must be run, not reviewed, to find.
  local tmp_out tmp_err
  tmp_out="$(mktemp "$RW_CACHE_DIR/.pending-out.XXXXXX")"
  tmp_err="$(mktemp "$RW_CACHE_DIR/.pending-err.XXXXXX")"

  local status=0
  rw_run_bounded "$budget" cargo "$@" >"$tmp_out" 2>"$tmp_err" || status=$?

  cat "$tmp_out"
  cat "$tmp_err" >&2

  mv "$tmp_out" "$entry.out"
  mv "$tmp_err" "$entry.err"
  printf '%s\n' "$status" > "$entry.status"
  return "$status"
}
