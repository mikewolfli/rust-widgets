# group-h-elide — report (R496)

CLOSED: R496

OPEN: none.

`docs/issues/slices/group-h-elide.md` names only R497, which is a `metrics.rs` row owned by
`group-g-base` and is already recorded as closed there. This slice's work is the R496
consumer-migration gap described in `group-g-base-report.md`.

SCHEMA: none. Every change is an internal text helper moving onto a shared primitive; no
widget property was added, removed, or given a new default, so no capability row is needed.

## What was still duplicated

The tree had already moved ten consumers onto `text_utils` before this slice:

| Consumer | Entry point |
| --- | --- |
| `chart_widgets/charts.rs` | `truncate_legend_label` → `elide_end` (`:448`) |
| `container_widgets/dockwidget.rs` | `vertical_title` → `elide_end` (`:1299`) |
| `input_widgets/shortcut_editor.rs` | row name → `elide_end` (`:910`) |
| `view_widgets/image_gallery.rs` | caption → `elide_end` (`:998`) |
| `view_widgets/virtual_table.rs` | header name → `elide_end` (`:833`) |
| `view_widgets/data_grid.rs` | header name → `elide_end` (`:1180`) |
| `misc_widgets/barcode_scanner.rs` | decoded text → `elide_end` (`:566`) |
| `misc_widgets/segmented_button.rs` | `fitted_caption` → `elide_to_width` (`:403`) |
| `advanced_widgets/tab_bar.rs` | `fitted_title` → `elide_to_width` (`:935`) |
| `cupertino/segmented_control.rs` | `fitted_segment_label` → `elide_to_width` (`:518`) |

Six files still carried a private implementation:

1. `base_widgets/label.rs` — `elide_in_mode` (width dispatcher; `ElideRight` already
   delegated, `ElideLeft`/`ElideMiddle` private) and `wrap_text` (65-line pixel wrap).
2. `display_widgets/empty_state.rs` — `wrap_text` (37-line pixel wrap).
3. `container_widgets/tabwidget.rs` — `elide_to_width` (linear pixel elide).
4. `dialog/alert_dialogs.rs` — `wrap_words` (character-budget wrap).
5. `dialog/tooltip.rs` — `Tooltip::wrapped_lines` (pixel wrap).
6. `special_widgets/chart.rs` — `draw_truncated_label` (inline character budget + `..`).

A scan of every control that fits text to a box reaches roughly twenty-two sites; most were
already consolidated, and the remaining genuine copies are the six above.
(The render layer's `fit_text_to_width`, `src/render/backend/surface.rs:374`, is a seventh
implementation but it predicts cluster advances from one shaping pass rather than measuring
each candidate, and `src/render/**` is outside this slice's file set.)

## Closed — migrated

- **`label.rs` / `elide_in_mode`** — `TextElideMode::ElideMiddle` now calls
  `text_utils::elide_middle_to_width`; `ElideRight` already called `elide_to_width`;
  `None` still borrows. The private middle search (35 lines) is gone. The `ElideLeft` arm is
  kept and documented in place with the reason below. Test:
  `elide_left_and_middle_keep_the_end_they_promise` (unchanged, still pins all three endings).
- **`label.rs` / `wrap_text`** — now calls `text_utils::wrap_to_width` and slices the returned
  byte ranges. The label's own contributions are only (a) the one-line fallback for a zero
  budget and (b) a `collapse_whitespace` pre-pass, because the private loop used
  `split_whitespace` and therefore painted a whitespace *run* as one space and never painted
  stray ends; normalising before the split keeps that byte-for-byte. Tests:
  `wrapping_breaks_on_words_and_hard_splits_a_long_one`,
  `wrapping_honours_explicit_line_breaks_and_blank_paragraphs`, and the new
  `wrapping_keeps_the_labels_whitespace_policy`.
- **`empty_state.rs` / `wrap_text`** — now a thin adapter over `text_utils::wrap_to_width`
  (measure callback + range-to-`String`). The 37-line loop is gone. Test:
  `the_message_wraps_at_the_shared_word_boundaries` (new) pins the word-boundary split and the
  character-boundary split of an over-long word.
- **`tabwidget.rs` / `elide_to_width`** — the linear implementation is deleted; the call site
  measures through `RenderContext` and calls `text_utils::elide_to_width`. The one rule kept is
  this control's floor, in a named adapter `elide_tab_title`: the shared primitive reports
  "not even the ellipsis fits" as `""`, while a tab has always painted `…` there so a cut title
  is distinguishable from an empty one. Test:
  `a_tab_title_is_elided_by_the_shared_primitive_with_an_ellipsis_floor` (new).

### Why the observable output is unchanged

- `label::elide_in_mode` middle: the private search capped `high` at `count - 1`, the shared one
  at `count`. The candidate at `count` is the whole string *plus* an ellipsis, which is wider
  than the string that was just measured as too wide, so that probe can never be accepted and
  both searches return the same largest fitting split.
- `label::wrap_text`: `collapse_whitespace` reproduces `split_whitespace`'s result per paragraph,
  and `wrap_to_width` applies the same greedy rule the deleted loop did (word moves whole,
  over-long word split on character boundaries, trailing space excluded from the line).
- `tabwidget`: the adapter returns exactly what the deleted loop returned, including the
  ellipsis-floor and the `width == 0` / empty-text cases.
- `empty_state`: the rule that changed is the over-long-word case (the private loop kept such a
  word whole and let the fitted draw clip it; the shared primitive splits it). The shipped
  message has single spaces, no `\n`, and no word wider than the census width, so its line
  decomposition is identical. `snapshots/svg/empty_state.svg` and `.light.svg` are unaffected;
  neither `label` nor `tab_widget` reaches the changed code in the snapshot set because
  `sample_fill` leaves `word_wrap` off and `elide` at `none`.

## Kept by design — genuinely different semantics

Each of these is a *rule*, not a copy of the shared algorithm, and the shared primitive cannot
express it without a policy parameter (which would mean editing `text_utils.rs`, outside this
slice's file set). Each keeps its own unit test, and each definition states the rule it owns;
`alert_dialogs::wrap_words`' doc comment now names the fill rule explicitly so the next reader
does not mistake it for an unconverted copy. No behaviour changed in those three files.

- **`dialog/alert_dialogs.rs::wrap_words`** — a word that does not fit **fills the remaining
  room** on the current line before continuing on the next, while `wrap_to_width` moves the whole
  word down. Example: `"abcdefgh ij"` at 5 columns yields `["abcde", "fgh i", "j"]` here and
  `["abcde", "fgh", "ij"]` from the shared rule. Changing it would re-flow every dialog body,
  which the slice's "snapshots must not change" constraint forbids. Next step: add a
  `fill_remainder: bool` policy to `wrap_to_width` (or accept the re-flow and regenerate the
  dialog snapshots in the same commit). Test: `a_message_wraps_on_word_boundaries`.
- **`dialog/tooltip.rs::wrapped_lines`** — a single word wider than the bubble is deliberately
  kept whole on its own line and left to the fitted draw, rather than split mid-word (stated in
  the method's doc comment as the reason a URL stays readable). `wrap_to_width` hard-splits.
  Next step: the same `break_long_words: bool` policy parameter. Test:
  `wrapping_grows_the_bubble`.
- **`special_widgets/chart.rs::draw_truncated_label`** — an axis label is cut to
  `budget - 2` characters and suffixed with the **ASCII** `..`, not the `…` the shared
  primitives emit, and its budget is derived from the measured advance of `M`. Both the marker
  and the character count are painted pixels, so delegating to `elide_end` would change every
  chart axis label. Next step: give `text_utils` an `elide_to_width` variant that takes the
  marker, then migrate. The axis-label geometry is covered by the chart tests and its
  snapshots.

## Follow-up (not a slice row)

`text_utils` publishes the character-budget `elide_start` but no pixel-budget counterpart, which
is why `label::elide_in_mode`'s `ElideLeft` arm still owns a binary search (a width→character
conversion would approximate the renderer's advance). Adding `elide_start_to_width` next to
`elide_middle_to_width` would close that last arm; `text_utils.rs` is owned by the base slice.

## Validation

`cargo check --lib` clean (no errors, no warnings for this slice's files).

Targeted tests, all passing:

- `widget::base_widgets::label` — 52 passed
- `widget::display_widgets::empty_state` — 21 passed
- `widget::container_widgets::tabwidget` — 81 passed
- `widget::text_utils` — 17 passed (unchanged; the primitives were not edited)
- neighbours that consume the same primitives: `widget::base_widgets::button` 99,
  `widget::advanced_widgets::tab_bar` 40, `widget::misc_widgets::segmented_button` 15,
  `widget::cupertino::segmented_control` 15, `widget::chart_widgets` 94,
  `widget::dialog::alert_dialogs` 15, `widget::dialog::tooltip` 18 — all passed.

The SVG snapshot gate (`tools/check_svg_snapshots.sh`) was **not** run: it regenerates the
whole 449-file set from the shared worktree, where sibling slices are mid-edit, so a diff would
mix their in-progress drawing changes with this slice's. The byte-identical claim is instead by
construction, per the evidence above, and the set's committed files were clean before the change:
neither `label` nor `tab_widget` reaches the migrated code in the snapshot configuration
(`sample_fill` leaves `word_wrap` off and `elide` at `none`), and `empty_state`'s default message
has no over-long word, no `\n` and no repeated spaces, so its line decomposition is the same
under both implementations.

Files changed: `src/widget/base_widgets/label.rs`,
`src/widget/display_widgets/empty_state.rs`,
`src/widget/container_widgets/tabwidget.rs`, `src/widget/dialog/alert_dialogs.rs`
(doc comment only), and this report. No capability schema, `Cargo.toml`, `src/render/**`,
`src/widget/text_utils.rs`, or snapshot file was touched.
