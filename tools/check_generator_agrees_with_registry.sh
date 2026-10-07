#!/usr/bin/env bash
# A name the generator can construct must be a name the registry can construct — and vice versa.
#
# # Why this gate exists
#
# The generator's availability probe asks the **registry** (`WidgetFactory::create`, which resolves
# aliases) while its generated `create_for` used to map a name to a concrete Rust type through a
# hand-maintained table. Those two answers came from two tables that nothing checked agreed, and for
# an alias they did not: `availability("btn")` said `Local` (the registry resolves `btn` to
# `button`) while the table had no arm for it, so `assemble` skipped the arm and the generated
# `create_for` matched nothing —
#
#     let mut control = match widget {
#         _ => None,
#     };
#
# — so `ViewEngine::mount` could not create the root, the whole tree never mounted, and the window
# was blank, **while `GenerationReport::unsupported` stayed empty**. A clean report over a program
# that builds nothing is principle #18's "reported success for something that did not happen". The
# same table covered only 20 of the registry's 188 constructible controls, so most controls were
# unbuildable too.
#
# Nothing caught it: `check_generator_output_compiles.sh` compiles the output (an empty `match`
# compiles), and the mode-consistency test only asserts that a name *appears* in the source.
#
# # How it is checked
#
# `tools/generator_agreement_probe.rs` is a `[[test]]` target, because both answers under test are
# *runtime* answers: only asking the crate can show they disagree. A source grep would find both
# tables and call them consistent. The target carries `required-features = ["desktop"]` — a stripped
# profile has no registry to agree with.
#
# # Reverse injection
#
# Step 2 removes the `btn` alias from the registry, which is exactly the defect class, and requires
# the gate to FAIL naming `btn`. Without it, a probe that iterated only the registered aliases would
# pass against a registry that had simply lost entries.
#
# Usage: tools/check_generator_agrees_with_registry.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"
. "$ROOT_DIR/tools/lib_timeout.sh"

PROPERTIES="src/widget/capability/properties.rs"

# Probe output goes to `mktemp` files, not fixed `gate.1.out` / `gate.2.out` names in the working
# tree: the fixed names polluted `git status` and raced a concurrent run. The variables hold the
# paths so each is referenced by the same name throughout, and the trap removes them on exit.
OUT1="$(mktemp)"
OUT2="$(mktemp)"

run_probe() {
    rw_run_bounded 900 cargo test --no-default-features --features desktop \
        --test generator_agreement_probe -- --nocapture
}

echo "=== [1/2] every constructible name and alias is buildable by the generated program ==="
if ! run_probe > "$OUT1" 2>&1; then
    cat "$OUT1"
    rm -f "$OUT1"
    echo "FAIL: the generator and the registry disagree about what can be constructed" >&2
    exit 1
fi
grep -E "^OK |test result" "$OUT1" || true
rm -f "$OUT1"

echo ""
echo "=== [2/2] reverse injection: an unresolvable alias must fail the gate ==="
BACKUP="$(mktemp)"
cp "$PROPERTIES" "$BACKUP"
# shellcheck disable=SC2064
trap "cp '$BACKUP' '$PROPERTIES'; rm -f '$BACKUP' '$OUT1' '$OUT2'" EXIT

"$PYTHON" tools/_inject_unresolvable_alias.py "$ROOT_DIR/$PROPERTIES"

if run_probe > "$OUT2" 2>&1; then
    cat "$OUT2"
    echo "FAIL: removing an alias from the registry does not fail the gate" >&2
    exit 1
fi
if ! grep -q "btn" "$OUT2"; then
    cat "$OUT2"
    echo "FAIL: the gate failed but did not name the alias that became unresolvable" >&2
    exit 1
fi
echo "  injection detected as expected"

cp "$BACKUP" "$PROPERTIES"
if ! run_probe > /dev/null 2>&1; then
    echo "FAIL: the gate does not pass again after the injection is reverted" >&2
    exit 1
fi
rm -f "$BACKUP" "$OUT1" "$OUT2"
trap 'rm -f "$OUT1" "$OUT2"' EXIT

echo ""
echo "generator/registry agreement checks passed."
