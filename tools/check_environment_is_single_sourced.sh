#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_environment_is_single_sourced.sh — BLUE24 §4 criterion 6
# ============================================================================
# The rule this guards:
#
#   **The eight facts about the device are answered in one place, and nowhere
#    else re-derives them.**
#
# The defect this stops
# ---------------------
# Before `style::environment`, the crate answered those eight questions eight
# different ways, and the worked example is the worst of them (BLUE24 §4.1):
#
#     // src/style/theme_state.rs, `should_use_dark`
#     let now = std::time::SystemTime::now()
#         .duration_since(std::time::UNIX_EPOCH)
#         .unwrap_or_default()
#         .as_secs();
#     let hour = ((now / 3600) % 24) as u8;      // ← guessing a *preference* from a clock
#     hour >= start && hour < end
#
# Three failures in one function, and each is the shape this gate blocks:
#
#   * a **wall clock** standing in for a user preference — the answer is not a fact
#     about time, so no clock can produce it;
#   * a `cfg`-split pair of bodies (`mini` answered a hardcoded `false`, which reads
#     as "the user wants light" when the truth was "no clock here") — principle #37's
#     fabricated capability;
#   * zero testability: a test cannot set the system clock to 22:00.
#
# The rule is therefore: those questions are asked of the environment provider, and
# the only place a provider is *derived from the OS* is inside the provider.
#
# What it accepts and what it rejects
# -----------------------------------
#   * `std::time::SystemTime` / `UNIX_EPOCH` outside the environment module and the
#     clock's own home is a finding: it means something is inferring time-dependent
#     behaviour rather than being told.
#   * The env-var API (`std::env::var`) deciding a device fact is a finding for the
#     same reason.
#
# Excluded by scope:
#   * `mod tests` and below — a fixture may set a clock deliberately.
#   * `//` comment lines, so documentation that *describes* the removed defect is
#     not read as the defect. (This gate's own header quotes it; without the strip,
#     the gate would fail on the explanation of why it exists.)
#   * The clock's legitimate homes: `src/compat/` (the abstraction itself),
#     `src/platform/` (a backend may need real time), and this module.
#
# What this gate does NOT prove
# -----------------------------
#   * It cannot see a *runtime* re-derivation (a value computed from a provider
#     answer and then cached elsewhere). It reads for the two API shapes that were
#     actually used; a third shape should be added here when it appears.
#   * It does not require every one of the eight facts to have a consumer. A fact
#     with no reader is a different finding (principle #99), and the plan tracks it
#     in its own criterion, not this one.
#
# Reverse injection
# -----------------
# Putting a `SystemTime::now()` hour comparison back into `theme_state.rs` must make
# this gate name it. See the round's report for the exact output.
#
# Usage: tools/check_environment_is_single_sourced.sh
# Exit 0 = the device facts have one source.
# Exit 1 = a finding (each offender is named file:line).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ENV_MODULE="src/style/environment.rs"

echo "[1/3] the single source exists and answers all eight facts"
if [ ! -f "$ENV_MODULE" ]; then
    echo "  FAIL  $ENV_MODULE is missing; the device facts have no single source"
    exit 1
fi
if ! grep -qE '^pub trait EnvironmentProvider' "$ENV_MODULE"; then
    echo "  FAIL  EnvironmentProvider is not defined in $ENV_MODULE"
    exit 1
fi
# The eight methods the plan fixes (§4.2), by name. A missing one means a fact has
# no channel, which is the state this gate exists to end.
MISSING=""
for method in text_scale layout_scale locale color_scheme motion_preference high_contrast text_direction mirroring; do
    if ! grep -qE "fn ${method}\(" "$ENV_MODULE"; then
        MISSING="${MISSING} ${method}"
    fi
done
if [ -n "$MISSING" ]; then
    echo "  FAIL  EnvironmentProvider is missing these facts:${MISSING}"
    echo "        BLUE24 §4.2 fixes the set; a fact with no method has no reader and"
    echo "        no default, so a control has to invent it."
    exit 1
fi
if ! grep -qE '^pub fn environment\(\)' "$ENV_MODULE"; then
    echo "  FAIL  there is no \`pub fn environment()\` -- the one read point"
    exit 1
fi
echo "  PASS  eight facts, one trait, one read point"

echo "[2/3] no wall-clock inference of an appearance or motion preference"
# The rule is about the *decision*, not about time: a timestamp, a file mtime and a
# nonce seed are all honest uses of a clock. What is forbidden is a clock producing
# one of the eight device facts.
#
# So the scan is narrower than "SystemTime" on purpose. It matches the two shapes the
# defect actually took -- an hour-of-day extraction, and a comparison of one against a
# window -- restricted to the files that decide appearance or motion. Anything a
# human reads as a clock use for a timestamp is left alone, because an over-broad
# gate is a gate nobody can satisfy and therefore one nobody runs
# (BLUE22 §F.3 lesson 4, the same failure this project has recorded before).
APPEARANCE_FILES="src/style src/theme src/widget/base_widgets src/widget/display_widgets"
CLOCK_HITS="$(grep -rnE 'SystemTime::now|UNIX_EPOCH|as_secs\(\)|/ 3600|/3600' $APPEARANCE_FILES --include=*.rs \
    | sed -e 's://.*::' \
    | grep -vE ':\s*$' \
    || true)"

# Drop hits that live inside a `mod tests` block or a comment.
CODE_CLOCK_HITS="$(printf '%s\n' "$CLOCK_HITS" | while IFS= read -r line; do
    [ -z "$line" ] && continue
    f="${line%%:*}"; rest="${line#*:}"; ln="${rest%%:*}"
    mod_line="$(awk -v target="$ln" '/mod tests/{m=NR} NR==target{print m+0}' "$f" 2>/dev/null)"
    if [ "${mod_line:-0}" -eq 0 ]; then
        printf '%s\n' "$line"
    fi
done || true)"

# A hit only counts if the same file also compares a derived hour against a window
# or otherwise reaches an appearance decision. The narrow form is: the file mentions
# an hour AND a theme/appearance concept. That is what keeps a print timestamp from
# being reported as a preference guess.
FINDINGS=""
while IFS= read -r line; do
    [ -z "$line" ] && continue
    f="${line%%:*}"
    if grep -qiE 'hour|theme_mode|should_use_dark|color_scheme|motion_preference' "$f"; then
        FINDINGS="${FINDINGS}${line}\n"
    fi
done <<< "$CODE_CLOCK_HITS"

if [ -n "$FINDINGS" ] && [ "$(printf '%b' "$FINDINGS" | tr -d '[:space:]' | wc -c)" -gt 0 ]; then
    echo "  FAIL  an appearance preference is inferred from the wall clock:"
    printf '%b' "$FINDINGS" | sed 's/^/          /'
    echo "        A user's preference is not a fact about the time of day. Ask the"
    echo "        installed EnvironmentProvider instead (BLUE24 §4 criterion 6)."
    exit 1
fi
echo "  PASS  no appearance preference is inferred from the clock"

echo "[3/3] no environment variable decides a device fact"
# Same shape, second API: reading `TZ`/`LANG`/`TEXT_SCALE`-style variables to decide
# one of the eight would put the answer in the process environment rather than in
# the provider, which no test can substitute.
ENV_HITS="$(grep -rnE 'std::env::var\(|env::var_os\(' src/style/ src/widget/ src/theme/ --include=*.rs \
    | sed -e 's://.*::' \
    | grep -vE ':\s*$' \
    || true)"
CODE_ENV_HITS="$(printf '%s\n' "$ENV_HITS" | while IFS= read -r line; do
    [ -z "$line" ] && continue
    f="${line%%:*}"; rest="${line#*:}"; ln="${rest%%:*}"
    mod_line="$(awk -v target="$ln" '/mod tests/{m=NR} NR==target{print m+0}' "$f" 2>/dev/null)"
    if [ "${mod_line:-0}" -eq 0 ]; then
        printf '%s\n' "$line"
    fi
done || true)"

if [ -n "$CODE_ENV_HITS" ] && [ "$(printf '%s' "$CODE_ENV_HITS" | tr -d '[:space:]' | wc -c)" -gt 0 ]; then
    echo "  FAIL  a device fact is decided by an environment variable:"
    printf '%s\n' "$CODE_ENV_HITS" | sed 's/^/          /'
    echo "        A host states the fact by installing a provider, so a test can"
    echo "        substitute one (BLUE24 §4.2's testability argument)."
    exit 1
fi
echo "  PASS  no environment variable decides a device fact"

echo
echo "check_environment_is_single_sourced: OK"
