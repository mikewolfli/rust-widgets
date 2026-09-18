#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_capability_feature_gates.sh — principle #41 / #53
# ============================================================================
# The rule this guards:
#
#   A **capability** feature (i18n, print, touch, chart, …) may not be gated on a
#   **profile** feature (`desktop` / `tablet` / `mobile`).
#
# Why this is a real defect class and not a style preference
# ---------------------------------------------------------
# `Cargo.toml` defines `desktop`, `tablet` and `mobile` as device profiles, and
# `i18n`, `print`, `touch`, `chart` … as capabilities. The profiles are not
# nested: `tablet` and `mobile` enable `i18n` **without** `desktop`.
#
# So `#[cfg(feature = "desktop")]` around an i18n call compiles the *untranslated*
# branch into tablet/mobile builds that have a working, initialised catalogue.
# Nothing fails, no test goes red, and the only symptom is user-visible text
# (a tooltip, a button label) that never gets translated. This shipped twice:
#
#   * `src/widget/base.rs::set_translated_tooltip`
#   * `src/widget/dialog/message_box.rs::StandardButton::translated_label`
#
# Both were two function bodies under complementary `desktop` gates — the exact
# "two bodies for two feature sets" shape principle #41 forbids — and both took
# the untranslated path on tablet/mobile.
#
# The criterion is a source check, and it is honest about being one
# ---------------------------------------------------------------
# A build test cannot catch this: the broken profiles *compile*. What is wrong is
# the pairing of a capability with a profile in a `cfg`, which is a property of
# the source text. So this script reads the text and reports the pairing.
#
# Exit 0 = no mis-gated capability. Exit 1 = a finding.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# ---------------------------------------------------------------------------
# Capability features that are NOT profile features. Every profile may enable
# any of these, so a gate naming both a capability and a profile is suspect.
#
# Read from `Cargo.toml` rather than hard-coded so the list cannot go stale: a
# feature qualifies when it is not one of the three profile names and is not an
# internal alias.
# ---------------------------------------------------------------------------
CAPABILITIES="$(sed -n '/^\[features\]/,/^\[/p' Cargo.toml \
  | grep -E '^[a-z0-9_-]+ *=' \
  | sed 's/ *=.*//' \
  | grep -vE '^(default|desktop|tablet|mobile|mini|embedded|full|os-auto|portable|no-declarative-view|macos|macos-legacy|ios|windows|linux-gtk|linux-wayland|linux-a11y|android|jni|wasm|harmony|gpu|gpu-wgpu|$)' \
  | sort)"
PROFILES="desktop|tablet|mobile"

ERRORS=0

echo "=== Capability features may not be gated on a device profile ==="
echo

# ---------------------------------------------------------------------------
# The criterion: a `cfg` containing BOTH a profile name and a capability name.
#
# `all(feature = "desktop", feature = "i18n")` is reported too, and that is
# deliberate: naming a capability beside a profile means the author was choosing
# between the profile's answer and the capability's, which is the confusion this
# gate exists to surface. If a capability genuinely belongs next to a profile the
# right fix is to say why in the source, not to add a third spelling.
# ---------------------------------------------------------------------------
findings=0
while IFS= read -r line; do
  [ -z "$line" ] && continue
  findings=$((findings + 1))
  echo "  ❌ $line"
done < <(
  grep -rnE '#\[cfg\((all\()?[^]]*feature = "' src/ --include='*.rs' 2>/dev/null \
    | while IFS= read -r hit; do
        # Only lines that name BOTH a profile and a capability in the same cfg.
        text="${hit#*:}"
        if printf '%s' "$hit" | grep -qE "feature = \"($PROFILES)\""; then
          for cap in $CAPABILITIES; do
            if printf '%s' "$hit" | grep -qE "feature = \"$cap\""; then
              printf '%s\n' "$hit"
              break
            fi
          done
        fi
      done
)

if [ "$findings" -eq 0 ]; then
  echo "  ✅ no cfg pairs a device profile with a capability feature"
else
  echo
  echo "  A cfg naming both a profile and a capability means the author was"
  echo "  choosing between two questions that have different answers. Gate on the"
  echo "  capability alone (it is the narrower, honest question), and let the"
  echo "  capability's own fallback handle the builds that lack it."
  ERRORS=$((ERRORS + 1))
fi

echo
 echo "=== The i18n call sites must not be gated on a profile ==="
echo
# A direct check on the two sites this defect actually shipped at, so a future
# rewrite that reintroduces a `desktop` gate is caught even if it is spelled in a
# way the generic scan above does not match (e.g. a `cfg` on the next line).
#
# The needle is the *capability-correct* entry point, not one particular spelling:
# `crate::translate_key` is the feature-independent function, while `tr!` is the
# feature-independent macro. Either is fine; naming `crate::i18n::` directly is not,
# because that module only exists when the capability is on.
for probe in \
  "src/widget/base.rs:translate_key" \
  "src/widget/dialog/message_box.rs:tr!"
do
  file="${probe%%:*}"
  needle="${probe#*:}"
  if ! grep -qF "$needle" "$file"; then
    echo "  ❌ $file no longer calls $needle — the translation path was removed"
    ERRORS=$((ERRORS + 1))
    continue
  fi
  # The line that carries the call must not itself sit under a profile gate.
  line_no="$(grep -nF "$needle" "$file" | head -1 | cut -d: -f1)"
  nearby="$(sed -n "$((line_no > 3 ? line_no - 3 : 1)),${line_no}p" "$file")"
  if printf '%s' "$nearby" | grep -qE "cfg\(.*feature = \"($PROFILES)\""; then
    echo "  ❌ $file:$line_no is gated on a device profile"
    ERRORS=$((ERRORS + 1))
  else
    echo "  ✅ $file:$line_no resolves through i18n unconditionally"
  fi
done

echo
if [ "$ERRORS" -gt 0 ]; then
  echo "capability feature gates: FAILED ($ERRORS finding(s))" >&2
  echo "" >&2
  echo "To confirm this gate can fail (principle #89): add" >&2
  echo '  #[cfg(feature = "desktop")]' >&2
  echo '  pub fn probe() { let _ = crate::i18n::translate("x"); }' >&2
  echo "to src/widget/base.rs and re-run — the first section must report ❌." >&2
  exit 1
fi

echo "✅ capability feature gates passed: no capability is gated on a device"
echo "   profile, and the i18n call sites resolve unconditionally."
