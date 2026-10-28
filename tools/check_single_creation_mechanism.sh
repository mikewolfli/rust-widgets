#!/usr/bin/env bash
# ============================================================================
# check_single_creation_mechanism.sh — BLUE15 #55/#56 gate
# ============================================================================
# The architectural rule this guards:
#
#   The host platform supplies a window and a drawing surface. The library paints
#   every `WidgetKind`. There is exactly ONE creation mechanism.
#
# Two ways that rule can regress, both checked here:
#
#   [1] A backend re-gains a per-kind control constructor. The `Platform` trait
#       declares them (with honest default bodies, so a real primitive could be
#       declared deliberately one day), but a *backend override* means a host is
#       building controls again. Only `create_window` and the menu data model are
#       allowed to override.
#
#   [2] The routing policy stops being single-valued. `route_preference_for_widget_kind`
#       must return `CustomRequired` for every kind; a reintroduced `match` with a
#       `NativePreferred` arm means two creation paths exist again.
#
# This is a SOURCE gate, so it runs on any host — including CI runners that have no
# macOS/iOS/Android toolchain and therefore cannot compile those backends.
#
# Exit 0 = single mechanism holds. Exit 1 = a regression was found.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ERRORS=0

# Backends allowed to override a `create_*` beyond `create_window`.
#   * `create_menu*` — not control construction: no platform has a standalone menu
#     View, so the in-process menu tree plus its injectable trigger queue is a host
#     capability. See the module docs in each backend for the full reasoning.
#   * `create_native_platform` / `create_event_loop_pump` / `create_web_engine` /
#     `create_ui_window` / `create_ns_window` / `create_toplevel` / `create_widget` /
#     `create_child_widget`
#     — host-level constructors of the backend, its loop, its engine and its state
#     rows, not per-kind controls.
#
#     `create_toplevel` is the Wayland spelling of the window constructor the other two
#     name: it builds one `xdg_toplevel` (Wayland's window role) on the session the
#     backend owns, and it is a private inherent helper of
#     `src/platform/wayland/platform_impl.rs`, not a `Platform` trait method. It was
#     simply missing from this list, so the gate reported the *window* constructor as a
#     per-kind control — a stale allowlist rather than a violation. The distinction the
#     rule is about is unchanged: no backend may construct a per-kind **control**.
ALLOWED='^(create_window|create_menu|create_menu_bar|create_native_platform|create_event_loop_pump|create_web_engine|create_ui_window|create_ns_window|create_toplevel|create_widget|create_child_widget|create_with_list_data)$'

echo "=== [1] Backends must not override per-kind control constructors ==="

# Search backend implementation files only. The trait declarations live in
# `src/platform/types/traits.rs` and are intentionally excluded: they carry honest default
# bodies so a future backend can declare a genuine primitive deliberately.
#
# `#[cfg(test)]` bodies are excluded too: a test helper named `create_*` verifies
# behaviour, it does not give the host a control. The awk pass drops every block
# after a `mod tests` line.
violations=0
while IFS= read -r hit; do
  file="${hit%%:*}"
  rest="${hit#*:}"
  line="${rest%%:*}"
  # Extract the function name from the matched line.
  fn="$(printf '%s' "$hit" | sed -n 's/.*fn \(create_[A-Za-z0-9_]*\).*/\1/p')"
  [[ -z "$fn" ]] && continue
  # `create_file_dialog` has one legitimate override shape: a *logical state handle*.
  #
  # P-204 requires the id to be non-zero exactly when the `file_dialog` capability is
  # advertised, and the library paints the chooser itself — so the backend records a state row
  # and returns it instead of constructing a native dialog. No control exists, only the id the
  # capability promised.
  #
  # This is checked on the *body* rather than by adding the name to ALLOWED: an override that
  # built a real dialog control would still fail here, on every backend including these.
  if [[ "$fn" == "create_file_dialog" ]]; then
      fn_body="$(awk -v start="$line" 'NR >= start { print; if (NR > start && $0 == "    }") exit }' "$file")"
      if printf '%s' "$fn_body" | grep -qE 'insert_widget|register_state_only_handle|state\.create_widget'; then
          echo "  ✅ $file:$line create_file_dialog records a logical state handle (P-204), not a control"
          continue
      fi
  fi
  if ! printf '%s' "$fn" | grep -Eq "$ALLOWED"; then
    echo "  ❌ $file:$line overrides $fn — the host must create no per-kind control"
    violations=$((violations + 1))
  fi
done < <(awk '
    # The trait declarations live under src/platform/types/ and are not a backend: they carry
    # honest default bodies so a future backend can override one deliberately. The glob is
    # src/platform/*/*.rs, which now matches that directory too, so it is excluded by name.
    # (No apostrophes in this block: it is inside a single-quoted awk program.)
    FILENAME ~ /^src\/platform\/types\// { next }
    # Print only the part of each backend file that precedes `mod tests`.
    FNR == 1 { in_tests = 0 }
    /^mod tests/ { in_tests = 1 }
    in_tests { next }
    /^[[:space:]]{4}fn create_[A-Za-z0-9_]+\(/ { print FILENAME ":" FNR ":" $0 }
  ' src/platform/*/*.rs 2>/dev/null || true)

if [[ "$violations" -gt 0 ]]; then
  ERRORS=$((ERRORS + violations))
else
  echo "  ✅ no backend overrides a per-kind control constructor"
fi

echo "=== [2] Routing policy must be single-valued ==="

ROUTING="src/control_backend/routing.rs"
if [[ ! -f "$ROUTING" ]]; then
  echo "  ❌ $ROUTING is missing"
  ERRORS=$((ERRORS + 1))
else
  # The function body must not reintroduce a decision between two mechanisms.
  body="$(sed -n '/pub fn route_preference_for_widget_kind/,/^}/p' "$ROUTING")"

  if printf '%s' "$body" | grep -q 'NativePreferred'; then
    echo "  ❌ route_preference_for_widget_kind still returns NativePreferred —"
    echo "     that arm is the second creation path (BLUE15 §2.5 / G-4)."
    ERRORS=$((ERRORS + 1))
  elif printf '%s' "$body" | grep -q 'ControlRoutePreference::CustomRequired'; then
    echo "  ✅ routing policy returns CustomRequired for every kind"
  else
    echo "  ❌ route_preference_for_widget_kind declares no preference"
    ERRORS=$((ERRORS + 1))
  fi

  # The enum keeps both variants on purpose (BLUE15 §七): deleting them would make
  # a deliberate future addition invisible. Only the *policy* must be single-valued.
  echo "  note: ControlRoutePreference keeps both variants by design (BLUE15 §七);"
  echo "        only the policy above is required to be single-valued."
fi

echo ""
if [[ "$ERRORS" -gt 0 ]]; then
  echo "single-creation-mechanism gate: FAILED ($ERRORS finding(s))" >&2
  exit 1
fi
echo "✅ single-creation-mechanism gate passed"
