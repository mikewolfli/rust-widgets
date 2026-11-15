# Widget Conventions

This chapter is the checklist a new control is measured against. Every row names the **mechanism**
a sibling control already uses, the sibling to copy, and the gate (if any) that proves it. It exists
because the same gaps keep arriving one family at a time — an undo signal added to three text
fields and not the fourth, a disabled veil painted on seven controls and not the eighth — and a
written list is the cheapest way to stop the ninth.

---

## 1. The sister-alignment checklist

| Concern | Mechanism | Copy from | Proved by |
|---|---|---|---|
| Hover feedback | `BaseWidget::paint_hover_wash(context, rect)` at the end of `draw` | `Button` | *no gate* — the sibling comparison is the check |
| Disabled appearance | `BaseWidget::paint_disabled_veil(context, rect)`; also consult `self.base.is_enabled()` before handling input | `Button` | `tools/check_disabled_state_covers_kinds.sh` |
| Focus ring | `BaseWidget::draw_focus_ring_if_needed(...)` on the focusable roster | `Button` | `tools/check_focus_ring_respects_reason.sh`, `tools/check_focus_reachable_key_arms.sh` |
| Enabled contract | Early return in `handle_event` when `!self.base.is_enabled()` | `Button` | `tools/check_enabled_is_honoured.sh` |
| Undo availability | `undo_available` / `redo_available` (`Signal1<bool>`, emitted only on change) — or a change event that also fires on undo/redo | `SearchBar`, `Canvas` | `tools/check_capability_events_are_emitted.sh`, `tools/check_event_payload_types.py` |
| Bidirectional caret | `text_utils::visual_caret_advance` / `text_utils::visual_byte_at` instead of byte arithmetic | `TagInput`, `InplaceEditor` | unit tests in `text_utils` |
| Accessibility state | `widget::a11y_submit::submit_state_changed(id)` from the setter that changes the state | `Switch`, `Rating` | `tools/check_a11y_has_a_producer.sh` |
| Published events | `event_signal_dyn` arm + a line in `tools/event_published_census.txt` | any control with a signal | `tools/check_event_signal_dyn.sh` |
| Size hint | `ControlMetrics::implicit_size` with `estimate_line_height` / `estimate_text_width` | `ZoomStrip` | `tools/check_implicit_size_uses_metrics.sh` |
| Implicit repaint | `BaseWidget::request_redraw` from every structural mutator | any | `tools/check_no_anonymous_repaint.sh` |

### Rows with no gate are the ones to argue about

Three of the concerns above are not mechanically checked — hover feedback, the *shape* of a bidi
caret calculation, and whether a control's disabled appearance is merely painted versus genuinely
respected in its input path. `check_disabled_state_covers_kinds.sh` proves the veil call exists for
every registered control, not that the control ignores input while disabled; `check_enabled_is_honoured.sh`
covers the second half for input-handling controls. When you add a control, reading the two sibling
implementations is still part of the job.

---

## 2. Undo history: what a *programmatic* write does

The user-visible half of undo is the same everywhere (a stack, `undo()`/`redo()`, a `can_undo()`
query). The half that actually differs between controls is what a **programmatic** write — a host
calling `set_value`, a binding pushing a model change — does to the history. Two policies are
documented in the tree, and a control must state which it uses in its setter's documentation:

| Policy | What happens | Documented example |
|---|---|---|
| **Record the step** | The write is an undoable step like any user edit | `DateEdit::set_date` — the setter pushes a `ValueCommand` unless it is already restoring history |
| **Discard the history** | The write is the new baseline: no entry, and the previous history no longer describes anything | `LineEdit`'s `EditOrigin::Programmatic` — "no undo entry is recorded and the history that described the previous value is discarded"; `Canvas::clear()` empties both stacks |

A third arrangement — a "modified" flag with no history at all — does not exist among the
registered controls (`grep 'fn is_modified'` finds no control without an `undo_stack`), so it is
not an option to introduce quietly.

### Availability must be announced, not polled

A host that draws an undo button needs to know when the answer changes:

- **Text fields** publish `undo_available` / `redo_available` and emit only when the value changes.
  `undo()`, `redo()` and every mutation site call one `refresh_edit_capabilities()`.
- **Canvas and the two editors** publish a document-change event that also fires from the restore
  path (`content_changed`, `text_changed`), so a host reacts to the same signal and re-reads
  `can_undo()`.

Whichever shape a control picks, the rule is the same: undo and redo must be visible through a
signal, not only through a getter.

---

## 3. Why these are conventions and not a framework

Every mechanism above lives on `BaseWidget` or in a shared helper, so a control opts in with one
call instead of re-deriving the rule. The conventions exist for the part that cannot be shared: the
*decision* about which state a control has, what a programmatic write means for it, and which
sibling it is claiming to match. Write that decision in the setter's documentation, where the next
reader will look.
