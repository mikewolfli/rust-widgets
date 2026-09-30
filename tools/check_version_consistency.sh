#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_version_consistency.sh — every published version string must match Cargo.toml
# ============================================================================
# # Why this gate exists
#
# The crate's version appears in `Cargo.toml` and, repeated, in the two READMEs, all
# three cookbook locales, and the two changelog copies. Nothing checked that the
# copies agreed with the file the compiler actually reads — so a release bump that
# missed a locale shipped a README telling users to depend on a version that was
# never published. That is a documentation defect a user hits on the first command
# they type, and it is exactly the kind of thing a copy-paste release process
# produces.
#
# The version is *derived* at runtime (`src/core/types.rs` parses
# `env!("CARGO_PKG_VERSION")`), so the crate itself cannot disagree with itself.
# What can drift is every hand-written mention, and those are what this checks.
#
# # What it asserts
#
#   [1] `Cargo.toml` declares a parseable `major.minor.patch`
#   [2] every published doc names that version at least once
#   [3] the changelogs' newest entry names that version
#   [4] the lockfiles pin that version
#
# # Why [2] does not reject *other* versions
#
# An obvious strengthening is "this document must name no other version", and it is
# wrong. Three legitimate cases were measured in these very files:
#
#   * `getting-started.md` shows `version = "0.1.0"` inside the *reader's own*
#     `Cargo.toml` — a tutorial's version, not this crate's;
#   * `api-reference.md` says "Since 2.0.0 every control publishes its own property
#     contract" — history, and the only way to describe when a behaviour changed;
#   * `language-bindings.md` pins npm deps (`"ffi-napi": "^4.0.3"`), which are not
#     this crate at all.
#
# A check that rejects those is a check that trains its reader to ignore it, and the
# first real drift would be lost in the noise. So this gate asserts the *positive*
# fact — the current version is named where it must be — and leaves the judgement
# about other numbers to review. It is deliberately weaker than it looks like it
# could be, and that is the honest scope: a mechanical check cannot tell a tutorial's
# version from a stale pointer, and pretending otherwise is worse than saying so.
#
# Exit 0 = consistent. Exit 1 = a published document or lockfile disagrees.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ERRORS=0
fail() { echo "  FAIL $*" >&2; ERRORS=$((ERRORS + 1)); }
ok() { echo "  OK   $*"; }

# ---------------------------------------------------------------------------
# [1] The authoritative version
# ---------------------------------------------------------------------------
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    fail "Cargo.toml does not declare a major.minor.patch version (got '${VERSION:-<none>}')"
    exit 1
fi
echo "declared version: $VERSION"

# ---------------------------------------------------------------------------
# [2]/[3] The documents a reader of *this* version reads
# ---------------------------------------------------------------------------
PUBLISHED_DOCS=(
    "README.md"
    "README.zh-CN.md"
    "cookbook/en/src/README.md"
    "cookbook/en/src/chapters/getting-started.md"
    "cookbook/en/src/chapters/api-reference.md"
    "cookbook/en/src/chapters/embedded.md"
    "cookbook/en/src/chapters/advanced-topics.md"
    "cookbook/en/src/chapters/language-bindings.md"
    "cookbook/zh-CN/src/README.md"
    "cookbook/zh-CN/src/chapters/getting-started.md"
    "cookbook/zh-CN/src/chapters/api-reference.md"
    "cookbook/zh-CN/src/chapters/embedded.md"
    "cookbook/zh-CN/src/chapters/advanced-topics.md"
    "cookbook/zh-CN/src/chapters/language-bindings.md"
    "cookbook/zh-TW/src/README.md"
    "cookbook/zh-TW/src/chapters/getting-started.md"
    "cookbook/zh-TW/src/chapters/api-reference.md"
    "cookbook/zh-TW/src/chapters/embedded.md"
    "cookbook/zh-TW/src/chapters/advanced-topics.md"
    "cookbook/zh-TW/src/chapters/language-bindings.md"
)

for doc in "${PUBLISHED_DOCS[@]}"; do
    if [[ ! -f "$doc" ]]; then
        fail "$doc is missing"
        continue
    fi
    if ! grep -q "$VERSION" "$doc"; then
        fail "$doc names no version, expected $VERSION"
        continue
    fi
    ok "$doc"
done

# ---------------------------------------------------------------------------
# [3] The newest changelog entry
# ---------------------------------------------------------------------------
for cl in "CHANGELOG.md" "docs/reports/CHANGELOG.md"; do
    if [[ ! -f "$cl" ]]; then
        fail "$cl is missing"
        continue
    fi
    newest="$(sed -n 's/^## \([0-9][0-9.]*\).*/\1/p' "$cl" | head -1)"
    if [[ "$newest" != "$VERSION" ]]; then
        fail "$cl's newest entry is $newest, expected $VERSION"
    else
        ok "$cl newest entry is $VERSION"
    fi
done

# ---------------------------------------------------------------------------
# [4] The lockfiles must agree (they carry the version cargo resolved)
# ---------------------------------------------------------------------------
for lock in "Cargo.lock" "demo/control/Cargo.lock" "demo/finance/Cargo.lock" "demo/code_editor/Cargo.lock"; do
    [[ -f "$lock" ]] || continue
    got="$(grep -A 1 '^name = "rust_widgets"$' "$lock" | sed -n 's/^version = "\(.*\)"$/\1/p' | head -1)"
    if [[ "$got" != "$VERSION" ]]; then
        fail "$lock pins rust_widgets $got, expected $VERSION (run 'cargo update -p rust_widgets --precise $VERSION')"
    else
        ok "$lock"
    fi
done

# ---------------------------------------------------------------------------
echo
if [[ "$ERRORS" -ne 0 ]]; then
    echo "version consistency: $ERRORS problem(s)." >&2
    exit 1
fi
echo "version consistency: every published document and lockfile agrees with Cargo.toml ($VERSION)."
