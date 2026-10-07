#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_generated_files_have_a_runnable_producer.sh
# ============================================================================
# The rule this guards:
#
#   **A file whose header says it is generated must be reproducible from a
#   generator that exists and agrees with what is committed.**
#
# # The defect this exists to catch
#
# `src/widget/icon_fallback_data.rs` carried a `GENERATED FILE — DO NOT EDIT BY HAND` header and
# then, three lines later, admitted: *"there is no generator for this table … A
# `gen_icon_fallback.py` that removes the by-hand step is a known follow-up."* That state lived for
# several rounds. Nothing went red, because **no gate asked the question** — a file can claim to be
# generated and be maintained by hand, and every existing check (census, integrity, licence) passes
# either way. This gate asks the question.
#
# # Why "the generator exists" is not the assertion
#
# A generator that exists but produces something *other* than what is committed is the same defect
# with an extra step. So each pair is checked with the generator's own `--check` (which is
# byte-comparison, not a re-derivation), and the comparison is what is asserted.
#
# # The pairs are declared as data
#
# The table below is the whole contract: one line per generated artifact. Adding a generated file
# means adding a line, which is the point — the gate is the registry, so a new "generated" header
# cannot ship without someone deciding what produces it.
#
# Reverse injection
# -----------------
# Edit any committed generated file by one byte: this fails naming that file and its generator.
# Delete a generator: this fails naming the missing script.
#
# Usage: tools/check_generated_files_have_a_runnable_producer.sh
# Exit 0 = every declared generated file matches its runnable generator.
# Exit 1 = a finding.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_timeout.sh"
# `"$PYTHON"` (a validated Python 3) replaces the bare `python3` below: on Windows `python3` is
# often the Microsoft Store alias, which *blocks* rather than runs the script. `lib_python.sh`
# sources `lib_timeout.sh` itself, but both are listed so the dependency is explicit here.
. "$ROOT_DIR/tools/lib_python.sh"

# `file|generator|check-args` — one line per generated artifact the repository ships.
#
# `gen_icon_data.py --check` still demands the inbound licence claim, because the same flag drives
# both its write and its verify path; passing it here keeps one contract rather than two.
PAIRS=(
    "src/widget/icon_data.rs|tools/gen_icon_data.py|--check --license=apache-2.0"
    "src/widget/icon_fallback_data.rs|tools/gen_icon_fallback.py|--check"
    "src/widget/display_widgets/icon_names.rs|tools/gen_icon_names.py|--check"
    # The capability event-payload table declares itself generated in its own header
    # ("The declared payload of every published capability event — **generated**") and is
    # re-derived by this script. It was absent from this list, so the gate's stated contract
    # — "a new 'generated' header cannot ship without someone deciding what produces it" —
    # did not hold for it: the file could have been hand-edited with the gate still green.
    "src/widget/capability/event_payloads.rs|tools/derive_event_payloads.py|--check"
    # The shipped glyph tables. Each `.rs` here is a thin wrapper around a committed `.ttf`, and
    # the generator writes the **`.ttf`** — so that is what this gate must verify, not the
    # wrapper. Naming the wrapper would report "does not match (wrapper header)" forever, which
    # is a false finding produced by pointing the check at the description rather than the
    # artifact (the same shape as principle #38: verify the thing that actually exists).
    #
    # The `--license` (and `--face`) flags are not decoration: every one of these generators
    # refuses to run without them, because they embed third-party outlines and the caller — not
    # the script — must record which licence is being relied on. The values here are the ones
    # each file's own header records as how it was produced.
    "src/render/text/font_assets/latin.ttf|tools/gen_font_subset.py|--check --face=latin --license=ofl-1.1"
    "src/render/text/font_assets/arabic.ttf|tools/gen_font_subset.py|--check --face=arabic --license=ofl-1.1"
    "src/render/text/font_assets/cjk.ttf|tools/gen_font_subset.py|--check --face=cjk --license=ofl-1.1"
    "src/render/text/font_assets/emoji.ttf|tools/gen_emoji_subset.py|--check --license=ofl-1.1"
    "src/render/text/font_assets/cjk_shard_latin.ttf|tools/gen_cjk_shards.py|--check --license=ofl-1.1"
    "src/render/text/font_assets/cjk_shard_han.ttf|tools/gen_cjk_shards.py|--check --license=ofl-1.1"
    "src/render/text/font_assets/cjk_shard_kana.ttf|tools/gen_cjk_shards.py|--check --license=ofl-1.1"
    "src/render/text/font_assets/cjk_shard_symbols.ttf|tools/gen_cjk_shards.py|--check --license=ofl-1.1"
    "src/render/text/font_assets/cjk_shard_fullwidth.ttf|tools/gen_cjk_shards.py|--check --license=ofl-1.1"
    # This one *is* the artifact: the CJK bitmap table is generated source, not a binary.
    "src/render/text/cjk_bitmap_data.rs|tools/gen_cjk_bitmap.py|--check --license=ofl-1.1"
)

# The `.rs` wrappers around those binaries, and the files whose header quotes the old
# "generated" claim while correcting it (`properties_*.in.rs`). They are hand-maintained, and
# their headers say so; listing them keeps the reverse check honest about what it has already
# considered rather than re-deciding the same file every run.
HAND_MAINTAINED=(
    "src/render/text/font_assets/latin.rs"
    "src/render/text/font_assets/arabic.rs"
    "src/render/text/font_assets/cjk.rs"
    "src/render/text/font_assets/emoji.rs"
    "src/render/text/font_assets/cjk_shards.rs"
    "src/render/text/font_assets/cjk_shard_latin.rs"
    "src/render/text/font_assets/cjk_shard_han.rs"
    "src/render/text/font_assets/cjk_shard_kana.rs"
    "src/render/text/font_assets/cjk_shard_symbols.rs"
    "src/render/text/font_assets/cjk_shard_fullwidth.rs"
)

echo "[1/3] every declared generator exists"
missing=0
for pair in "${PAIRS[@]}"; do
    generator="${pair#*|}"
    generator="${generator%%|*}"
    if [ ! -f "$generator" ]; then
        echo "  FAIL  $generator does not exist, but a generated file names it as its producer"
        missing=1
    else
        echo "        present: $generator"
    fi
done
if [ "$missing" -ne 0 ]; then
    echo '        A file claiming GENERATED with no runnable producer is the defect this gate exists'
    echo '        for: it can only be maintained by hand, and its header is then a lie.'
    exit 1
fi
echo '  PASS  all declared generators exist'

echo "[2/3] every declared generated file is present"
for pair in "${PAIRS[@]}"; do
    file="${pair%%|*}"
    if [ ! -f "$file" ]; then
        echo "  FAIL  $file is declared generated but is missing"
        exit 1
    fi
    echo "        present: $file"
done
echo '  PASS  all declared generated files are present'

echo "[3/3] every generator agrees with what is committed"
failed=0
for pair in "${PAIRS[@]}"; do
    file="${pair%%|*}"
    rest="${pair#*|}"
    generator="${rest%%|*}"
    args="${rest#*|}"
    # `--check` is a byte comparison inside the generator, so no timeout wrapper is stacked here
    # beyond the generator's own (principle #59.3: one timeout, not two). `set -e` is suspended for
    # this call so a single failure reports every offender rather than aborting on the first.
    #
    # The args string is split into an array on whitespace and expanded as `"${args[@]}"`, so a
    # value such as `--license=ofl-1.1` reaches the generator as one argument rather than being
    # re-split and glob-expanded by the shell the way an unquoted `$args` would be.
    read -ra args_arr <<< "$args"
    if out="$("$PYTHON" "$generator" "${args_arr[@]}" 2>&1)"; then
        echo "        OK: $file  ($out)"
    else
        echo "  FAIL  $file does not match $generator"
        printf '        %s\n' "$out"
        failed=1
    fi
done
if [ "$failed" -ne 0 ]; then
    echo '        Regenerate the file with its own generator rather than editing it by hand.'
    exit 1
fi
echo '  PASS  every generated file matches its producer'

echo "[4/4] no source file calls itself generated without a declared producer"
# The list above is what makes each *declared* pair verifiable, but a list only checks what is
# on it. A file that grows a "generated" header and is not added here keeps its hand-edited
# body forever, which is the one failure the header is supposed to prevent.
#
# The pattern matches the *header* spelling of a generated-file claim, in the two forms this
# repository uses:
#
#   * `// GENERATED FILE — DO NOT EDIT BY HAND.` / `// Produced by \`tools/...\`` — the banner
#     the glyph tables and icon data carry;
#   * a module doc line reading `— **generated**` — how the capability event-payload table
#     states it.
#
# Prose elsewhere is deliberately not matched: a sentence like "the table is generated from the
# signals" describes another file, and a header that quotes the old claim while correcting it
# (`properties_*.in.rs` does exactly that) is documentation, not a claim.
#
# Held in a variable rather than inlined: the pattern itself contains a backtick (`\`tools/\``),
# and inside an unquoted `$( ... )` the shell would run it as command substitution — which
# silently made this check find **nothing** and therefore pass in every case. The first version
# of this check had exactly that bug, which is why the injection test for it initially showed
# no failure.
GENERATED_HEADER_PATTERN='^// GENERATED FILE|^// Produced by `tools/|— \*\*generated\*\*\.'
undeclared=0
# `while IFS= read -r` rather than `for candidate in $(grep …)`: command substitution word-splits its
# output, so a path containing whitespace would arrive as several candidates (and a `*` in a name
# would be glob-expanded against the filesystem). Reading line by line keeps each path intact.
while IFS= read -r candidate; do
    [[ -n "$candidate" ]] || continue
    declared=0
    for pair in "${PAIRS[@]}"; do
        if [ "${pair%%|*}" = "$candidate" ]; then declared=1; break; fi
    done
    if [ "$declared" -eq 0 ]; then
        for hand in ${HAND_MAINTAINED[@]+"${HAND_MAINTAINED[@]}"}; do
            if [ "$hand" = "$candidate" ]; then declared=1; break; fi
        done
    fi
    if [ "$declared" -eq 0 ]; then
        echo "  FAIL  $candidate carries a generated-file header but no producer is declared for it"
        undeclared=1
    fi
done < <(grep -rlE "$GENERATED_HEADER_PATTERN" --include='*.rs' src/ 2>/dev/null | sort)
if [ "$undeclared" -ne 0 ]; then
    echo '        Add it to PAIRS with its generator, or correct the header if the file is really'
    echo '        hand-maintained. A "generated" header with no producer is a claim, not a fact.'
    exit 1
fi
echo '  PASS  every self-declared generated file has a declared producer'

echo
echo "check_generated_files_have_a_runnable_producer: OK"
