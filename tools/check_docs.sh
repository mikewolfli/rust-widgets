#!/usr/bin/env bash
# Documentation gate.
#
# Three checks, each for a failure mode that is otherwise invisible:
#
#   [1] missing_docs   Every public item is documented, in every profile. The crate
#                      sets `#![deny(missing_docs)]`, so this is already a compile
#                      error — the step exists to *prove* it holds for the profiles
#                      CI builds separately (embedded/mini gate different modules).
#
#   [2] broken links   No intra-doc link may dangle or point at a private item.
#                      This is the check that caught docs linking to `crate::chart`
#                      (a module that does not exist), to `widget::runtime` (compiled
#                      out of `mini`), and to `pub(crate)` shader constants. A broken
#                      link is not cosmetic: rustdoc renders it as literal text, so
#                      the reader loses the reference and sees no error.
#
#   [3] portability    The same doc set must build under `embedded` and `mini`, not
#                      just `desktop`. A link that resolves only when every module is
#                      compiled is a latent failure for the constrained profiles.
#
# Usage: tools/check_docs.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PROFILES="desktop embedded mini"
fail=0

echo "[1/3] missing_docs: every profile must document every public item"
for profile in $PROFILES; do
    count=$(cargo check --no-default-features --features "$profile" 2>&1 \
        | grep -c "missing documentation" || true)
    if [ "$count" -ne 0 ]; then
        echo "  FAIL $profile: $count undocumented public item(s)"
        cargo check --no-default-features --features "$profile" --message-format=short 2>&1 \
            | grep "missing documentation" | head -10
        fail=1
    else
        echo "  $profile: 0"
    fi
done

echo
echo "[2/3] intra-doc links: no dangling links, no links to private items"
for profile in $PROFILES; do
    out=$(RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --no-default-features \
        --features "$profile" 2>&1 || true)
    if echo "$out" | grep -qE "^error"; then
        echo "  FAIL $profile:"
        echo "$out" | grep -E "^error|-->" | head -10
        fail=1
    else
        echo "  $profile: 0"
    fi
done

echo
echo "[3/3] all-features doc build (the widest link surface)"
out=$(RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features 2>&1 || true)
if echo "$out" | grep -qE "^error"; then
    echo "  FAIL all-features:"
    echo "$out" | grep -E "^error|-->" | head -10
    fail=1
else
    echo "  all-features: 0"
fi

echo
if [ "$fail" -ne 0 ]; then
    echo "Documentation checks FAILED."
    echo "  A broken link usually means the target is feature-gated or private:"
    echo "  write it as a plain code span (e.g. \`widget::runtime\`) instead of a link,"
    echo "  or point at the re-export that is public in every profile."
    exit 1
fi
echo "Documentation checks passed."
