#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_icon_entry_contract.sh — the U0 entry inventory, as an assertion
# ============================================================================
#
# blue27 U0 asks for "a complete inventory of entries, public structures and profile gates, with
# every real consumer listed and the compatibility risks named". A prose table in a log answers that
# once and then rots; this gate answers it every run.
#
# It encodes the four facts that the §19.5 `.map` and the §19.6 compatibility rules make *checkable*
# from the source text. Each one was a real defect found by hand first — the gate exists so the next
# occurrence is found by a run:
#
#   A1  A control that **publishes** `icon_source` must refuse a source it cannot persist.
#       Every one of the six owners had to answer "what does `get("icon_source")` say about borrowed
#       static geometry?" and the answer is an error, never `Null` — because `Null` reads as "this
#       control has no icon" and a document round-trip would then erase the icon. The first version
#       of `Icon`'s property and the chip item channel both got this wrong; see the log's §19.13 I13.
#       The pattern is anchored so that only the *getter's own* refusal satisfies it: a `mod tests`
#       mention (`assert_eq!(control.get("icon_source"), Err(...))`) is not a `get` implementation,
#       and matching it would let A1 stay green with the production arm deleted — measured, see
#       tools/gates_reverse_injection.md.
#
#   A2  A collection with `set_item_icon_source` must **draw** what it stores.
#       A typed setter with no draw call is a dead state: it accepts a value, reports success, and
#       changes no pixel. That was measured on `Tab`/`MdiSubWindow`/`Segment` before they were wired.
#
#   A3  A collection with `set_item_icon_source` must either publish item icons **as record rows** or
#       be listed below as index-only **with the reason**. `chip.items` is the one that carries a
#       record channel today. The list is not decoration: it is the honest statement of which
#       collections can express a per-item icon in a document, and each entry names the identity the
#       collection really has, because §19.6 forbids inventing one ("只有 index 的现有集合不得为此
#       私自重写选择模型").
#
#   A4  The typed vocabulary is closed: a token that is not in `IconName` cannot be constructed, so no
#       entry can reach a paint path with an unknown name. Checked against the generated token list
#       rather than a copy of it, so adding a token does not need this file edited.
#
# What this gate deliberately does **not** do: it does not count call sites or classify them by name.
# A regex cannot tell a user-content icon from a decoration, and a gate that guessed would report
# false findings on every edit (the first draft of this file did exactly that and was discarded).

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

failures=0
note() { printf '  %s\n' "$*"; }
ok() { printf '  ✅ %s\n' "$*"; }
bad() { printf '  ❌ %s\n' "$*" >&2; failures=$((failures + 1)); }

echo "[1/5] A1 every published icon_source owner refuses a non-persistable source"

# The three property tables that declare `icon_source`, and the control source that must implement
# the refusal for each. Pairing by file is the check: a declared property with no refusal in the
# control that owns it is the defect.
a1_pairs=(
  "src/widget/capability/properties_base.in.rs:src/widget/base_widgets/button.rs"
  "src/widget/capability/properties_menu.in.rs:src/widget/menu_toolbar/action.rs"
  "src/widget/capability/properties_menu.in.rs:src/widget/menu_toolbar/tool_button.rs"
  "src/widget/capability/properties_menu.in.rs:src/widget/menu_toolbar/menu_button.rs"
  "src/widget/capability/properties_other.in.rs:src/widget/overlay_widgets/fab.rs"
  "src/widget/capability/properties_other.in.rs:src/widget/display_widgets/icon.rs"
)
for pair in "${a1_pairs[@]}"; do
  table="${pair%%:*}"
  control="${pair##*:}"
  if ! grep -q 'PropertySchema::new("icon_source"' "$table"; then
    bad "$table no longer declares icon_source (update this gate's pairing)"
    continue
  fi
  if grep -qE '^[[:space:]]*(return[[:space:]]+)?Err\(CapabilityAccessError::IconSourceNotPersistable\)' "$control"; then
    ok "$(basename "$control") refuses a borrowed source in its getter"
  else
    bad "$(basename "$control") publishes icon_source but its getter never returns IconSourceNotPersistable"
  fi
done

echo "[2/5] A2 every set_item_icon_source collection reaches a draw call"
while IFS= read -r file; do
  if grep -qE 'paint_icon\(|draw_image\(' "$file"; then
    ok "$(basename "$file") draws what it stores"
  else
    bad "$(basename "$file") has set_item_icon_source but no icon draw call (dead state)"
  fi
done < <(grep -rl 'fn set_item_icon_source' --include=*.rs src/widget | sort)

echo "[3/5] A3 every set_item_icon_source collection is either a record channel or a named exception"

# The item-level record channel: a collection that publishes its rows through `items` and carries
# `icon_source` as a row field. `chip` is the implementation; others are added as they grow one.
record_channel() { grep -q 'RecordField::optional(' "$1" && grep -q '"icon_source"' "$1"; }

# Collections that keep only a positional identity, with the reason. blue27 §19.6 allows this; what
# it forbids is inventing an identity to make a record channel possible.
#
# `pie_menu` left this list on 2026-10-13: its items gained a stable id (the menu assigns one, a host
# may pin one) and an item-owned typed icon, so it publishes a record channel above instead of being
# exempted here.
index_only() {
  case "$1" in
    *mdiarea.rs)
      echo "sub-window identity is an ObjectId handle, which is not a document value";;
    *tabwidget.rs|*tab_bar.rs)
      echo "tabs are addressed by position; the collection has no stable id to key a row on";;
    *toolbox.rs)
      echo "items hold private fields and no id; a row would have to invent one";;
    *bottom_navigation_bar.rs|*navigation_drawer.rs|*core.rs)
      echo "nav items are position/keyed by their own model; no item id is published";;
    *material.rs)
      echo "rail items are addressed by index and carry no id; the collection has no stable key for a row";;
    *)
      echo "";;
  esac
}

for file in $(grep -rl 'fn set_item_icon_source' --include=*.rs src/widget | sort); do
  if record_channel "$file"; then
    ok "$(basename "$file") publishes a record channel"
    continue
  fi
  reason="$(index_only "$file")"
  if [ -n "$reason" ]; then
    note "• $(basename "$file"): index-only — $reason"
  else
    bad "$(basename "$file") has no record channel and no documented reason"
  fi
done

echo "[4/5] A4 the typed token vocabulary is closed"
# Two declaration files, one vocabulary: `icon_tokens.txt` (hand-written) and `icon_catalog.txt`
# (generated from upstream's codepoint list). Both use `token | RustVariant | doc string` with `#`
# section comments, so the token is the first `|`-separated field of a non-comment line — not the
# whole line. Reading both is what makes "closed" true of the 4,323 tokens the crate ships rather
# than of the 75 a human happened to write down.
tokens_file="crates/rust-widgets-icons/tools/icon_tokens.txt"
catalog_file="crates/rust-widgets-icons/tools/icon_catalog.txt"
if [ -f "$tokens_file" ] && [ -f "$catalog_file" ]; then
  declared_list="$(mktemp)"
  arms_list="$(mktemp)"
  trap 'rm -f "$declared_list" "$arms_list"' EXIT
  for declaration_file in "$tokens_file" "$catalog_file"; do
    while IFS= read -r line; do
      case "$line" in ''|\#*) continue;; esac
      token="${line%%|*}"
      token="$(printf '%s' "$token" | tr -d '[:space:]')"
      [ -n "$token" ] || continue
      printf '%s\n' "$token" >> "$declared_list"
    done < "$declaration_file"
  done
  # One pass over the generated file instead of one `grep` per token: the vocabulary is 4,323
  # tokens, and a per-token grep made this gate minutes long for the same answer.
  grep -oE '^[[:space:]]*"[a-z0-9_]+" =>' crates/rust-widgets-icons/src/names.rs \
    | sed -E 's/^[[:space:]]*"([a-z0-9_]+)".*/\1/' | sort -u > "$arms_list"
  declared="$(wc -l < "$declared_list")"
  missing="$(sort -u "$declared_list" | comm -23 - "$arms_list")"
  if [ -n "$missing" ]; then
    while IFS= read -r token; do
      bad "token '$token' is declared but has no from_name arm"
    done <<< "$missing"
  else
    ok "every declared token parses through IconName::from_name ($declared tokens)"
  fi

  # The count is documented in prose, and prose drifts: the two READMEs and the cookbook all said 68
  # while `tools/icon_tokens.txt` held 69, and nothing noticed until the READMEs were rewritten. The
  # sibling gate for the widget-kind count states the rule — a documented number needs a check tied to
  # the single source of truth — so the same assertion is made here against the two declaration files.
  #
  # The scanner is its own file for the same reason `text_coverage_claim_scan.py` is: a heredoc holding
  # regexes inside `$( )` is a quoting trap (the first attempt broke `bash -n`), and a gate that cannot
  # be parsed cannot fail.
  . "$ROOT_DIR/tools/lib_python.sh"
  if ! doc_errors="$("$PYTHON" tools/icon_count_claim_scan.py "$declared" 2>&1)"; then
    printf '%s\n' "$doc_errors" >&2
    bad "a document states an icon count that disagrees with the declaration files"
  fi
else
  bad "$tokens_file or $catalog_file is missing; the closed-vocabulary check cannot run"
fi

echo "[5/5] A5 the committed inventory matches the source it was generated from"

# U0 asked for an *exported* inventory, and the plan's own note records why an assertion list is not
# one: it can only speak about the entries somebody thought to assert about. The artifact is
# generated from the source, so the check is a regeneration and a diff — an entry that appears or
# disappears changes the file, and the gate says so instead of passing quietly.
. "$ROOT_DIR/tools/lib_python.sh"
INVENTORY="tools/icon_entry_inventory.txt"
if [ ! -f "$INVENTORY" ]; then
  bad "$INVENTORY is missing; regenerate with tools/gen_icon_entry_inventory.py"
elif "$PYTHON" tools/gen_icon_entry_inventory.py --check --output "$INVENTORY" >/dev/null 2>&1; then
  ok "$INVENTORY matches the source ($(grep -v '^#' "$INVENTORY" | grep -c ' | ') entries)"
else
  bad "$INVENTORY is stale; regenerate with tools/gen_icon_entry_inventory.py"
fi

if [ "$failures" -ne 0 ]; then
  echo "check_icon_entry_contract: $failures finding(s)" >&2
  exit 1
fi
echo "check_icon_entry_contract: OK"
