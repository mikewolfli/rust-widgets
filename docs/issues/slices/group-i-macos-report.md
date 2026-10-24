# group-i-macos — report

Scope: `src/widget/macos/**` (new), `src/widget/mod.rs` (module declaration), the additive
capability rows for the five new controls, and `tools/event_published_census.txt` (+1 event row
per control).

CLOSED: R446

OPEN: —

## What was added

A desktop component set beside `src/widget/cupertino/**`. Five controls, each a full `Widget` +
`Draw` + `EventHandler` + `WidgetProperties` implementation, each registered in the factory with
its own capability and events, each drawn with `ControlMetricsProfile::Macos`:

| canonical name | type | `WidgetKind` | file | events |
|---|---|---|---|---|
| `mac_segmented_control` | `MacSegmentedControl` | `SegmentedControl` | `src/widget/macos/segmented_control.rs` | `value_changed` (UInt) |
| `mac_stepper` | `MacStepper` | `Stepper` | `src/widget/macos/stepper.rs` | `value_changed` (Int) |
| `mac_toolbar` | `MacToolbar` | `ToolBar` | `src/widget/macos/toolbar.rs` | `item_activated` (UInt) |
| `mac_source_list` | `MacSourceList` | `ListView` | `src/widget/macos/source_list.rs` | `selection_changed` (String) |
| `mac_window_chrome` | `MacWindowChrome` | `Window` | `src/widget/macos/window_chrome.rs` | `close_requested`, `minimize_requested`, `zoom_requested` (unit) |

## Row notes

- **R446** — `src/widget/macos/mod.rs` plus the five modules above. The audit row's five missing
  desktop controls now exist: a segmented picker (24 pt unified track, selection thumb, arrow /
  Home / End selection), a stepper (19x28 two-zone control with clamp and optional wrap), a
  toolbar (equal-width slots, hover/pressed states, optional slot selection), a source list
  (sections + rows, single-row selection by flat index, keyboard navigation) and window chrome
  (28 pt title bar, three traffic-light controls with per-control request signals, and a
  published draggable band).
  - Evidence, metrics: `ControlMetricsProfile::Macos` is read by every control
    (`segmented_control.rs` track radius, `stepper.rs` corner radius, `toolbar.rs` slot height,
    `source_list.rs` selection radius, `window_chrome.rs` traffic-light diameter).
    Keyboard focus uses the existing `FocusRing` via `macos::draw_focus_ring`.
    Text is measured through `RenderContext::measure_text` + `text_utils::elide_to_width`.
  - Evidence, registration: `capability/constructors.rs` (`create_mac_*`),
    `capability/registration.rs` (five `register` calls), `capability/properties.rs` (five
    `*_capability()` functions), `capability/properties_other.in.rs` (five property tables),
    `capability/access.rs` (schema defaults for the new names), and
    `capability.rs::widget_matches_capability` (one tie-break row per new canonical name, because
    each control shares a `WidgetKind` with the control it refines).
  - Evidence, events: `tools/event_published_census.txt` rows and the regenerated
    `capability/event_payloads.rs` (`CONTROL_STARTS` / `EVENT_SCHEMAS`); `event_signal_dyn` arms in
    each control; `tools/check_event_signal_dyn.py` `CONVERTED` / `STRUCT_IMPL` rows so that gate
    verifies the arms rather than reporting the controls as unconverted.
  - Evidence, generated artefacts: `tools/declaration_alignment_census.txt` regenerated
    (`controls 213 -> 218`, `skipped 217 -> 222`, plus the five standard `geometry` placeholder
    rows). No other line changed.
  - Tests: `cargo test --lib widget::macos` -> 50 passed. Includes per-control property
    round-trip, event emission, hit-testing, draw-order, in-bounds and out-of-view clipping
    assertions, plus one census test asserting all five controls paint, are visible against their
    surface and change with the light/dark appearance.

## SCHEMA

New property names (the tables also re-declare already-published names such as `selected_index`,
`value`, `minimum`, `maximum`, `step`, `item_count`, `icon_size`, `orientation`, `row_count`,
`row_height`, `title`, and the four shared base rows, with their existing types):

```
SCHEMA | mac_stepper | wraps | Bool | true | true | false
SCHEMA | mac_toolbar | selectable | Bool | true | true | false
SCHEMA | mac_toolbar | selected_index | UInt | true | true | -> (null means no slot selected)
SCHEMA | mac_toolbar | orientation | String | true | false | "horizontal"
SCHEMA | mac_source_list | section_count | UInt | true | false | 0
SCHEMA | mac_source_list | selected_row | UInt | true | true | -> (null means no row selected)
SCHEMA | mac_window_chrome | chrome_height | UInt | true | false | 28
SCHEMA | mac_window_chrome | buttons_visible | Bool | true | true | true
SCHEMA | mac_window_chrome | draggable | Bool | true | true | true
SCHEMA | mac_segmented_control | segment_count | UInt | true | false | 0
```

Rename request for the main thread: none. The five capability rows, their property tables and the
per-kind defaults in `capability/access.rs` were added here (additive only); no existing row was
changed.

## Notes for the main thread

- Each new control shares a `WidgetKind` with the control it refines (`SegmentedControl`,
  `Stepper`, `ToolBar`, `ListView`, `Window`) rather than adding five `WidgetKind` variants, so
  `kind.rs` is untouched. That required one additive tie-break arm per new canonical name in
  `WidgetFactory::widget_matches_capability`; the arms are keyed per name (not `a | b`), because a
  shared arm let the first-registered sibling claim the new control's schema.
- No existing widget, snapshot, theme file or metric was modified; new widgets have no existing
  snapshot, and every painting path in this module is used only by these five controls.
