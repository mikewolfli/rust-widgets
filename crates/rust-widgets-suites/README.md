# rust-widgets-suites

Optional widget **suites** for [`rust_widgets`](../..): groups of controls that solve one job
together, kept out of the core crate's package so a host installs only what it uses.

The first group is `form_widgets`:

| Control | Canonical name | What it does |
|---|---|---|
| `Form` | `form` (aliases `form_panel`, `form_layout`) | Lays out labelled fields in rows/columns, aggregates validity, submits and resets. |
| `FormSet` | `form_set` (aliases `formset`, `wizard_form`, `form_pages`) | Pages several forms with one active page, completed flags, navigation and aggregate validity. |

## Install

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::capability::WidgetFactory;

let mut factory = WidgetFactory::new_with_defaults();
rust_widgets_suites::install(&mut factory)?;          // every group
// or: rust_widgets_suites::install_form_group(&mut factory)?;

let mut form = factory.create("form", Rect::new(0, 0, 360, 320), "Account").unwrap();
let mut set = factory.create("form_set", Rect::new(0, 0, 480, 360), "").unwrap();
```

`install` refuses a duplicate name (`RegistrationError::NameTaken`) instead of shadowing a control
the parent or a previous install already owns. It also teaches the factory each kind's declared
defaults, so `WidgetFactory::default_property_value("form", …)` answers from the same table the
control's constructor uses (`default_property_value` in this crate).

## What "complete" means here

Each control ships the full five-part surface, not just a widget struct:

- **properties** — a typed `PropertySchema` per name, with readable/writable flags, plus a matching
  `WidgetProperties::{get,set,property_names}` implementation. Derived facts (`valid`,
  `error_count`, `dirty`, `page_count`, …) are readable and refuse writes;
- **methods** — a Rust API for the model (`add_field`, `field_layouts`, `next_page`, `goto_page`,
  `validate_field`, `focus_first_invalid`, …), including every layout rectangle so a host can
  align its own companions;
- **events** — an `EventSchema` per name with its payload kind, an `event_signal_dyn` arm per name,
  and tests that fire the representative ones;
- **wiring** — the events resolve to real signals (`connect_event` cannot return `Ok` for an inert
  name), the command channel drives the published actions, and an optional `SimpleRegistry`
  forwards pointer events into the field controls;
- **accessibility** — `accessible_name()` reports a form's title (a set's active page title), and
  `semantic_state()` reports `Error` while a field carries a message, so the themed border and the
  role assistive technology reads follow the same state;
- **capability** — a `WidgetCapability` row (kind, canonical name, aliases, schemas, commands) that
  `try_register` installs into the factory.

## Layout

`Form`:

```text
┌ form rectangle ────────────────────────────────┐
│ title band (optional)                          │
│ ┌ content ───────────────────────────────────┐ │
│ │ label │ control            │ label │ ctrl  │ │   ← columns = 2
│ │       │ error/help line    │       │       │ │
│ │ label │ control            │                 │
│ └────────────────────────────────────────────┘ │
│                        [ Reset ] [ Submit ]    │
└────────────────────────────────────────────────┘
```

`field_layouts()` returns the exact `label` / `control` / `error` boxes. A field's control box is
written into the child widget's geometry, so the tree painter draws it exactly there.

`FormSet`:

```text
┌ form-set rectangle ───────────────────────────┐
│ [ Account ] [ Shipping ] [ Payment ]          │  ← header: tabs or numbered steps
│ ┌ the active page's Form ───────────────────┐ │
│ │                                           │ │
│ └───────────────────────────────────────────┘ │
│                    [ Back ] [ Next ] [ Submit]│
└───────────────────────────────────────────────┘
```

`header_rect()`, `header_item_rect(i)`, `content_rect()` and the three navigation rectangles are
public, and the active form is moved into `content_rect()` while the other forms are hidden. A
page whose form reports `valid = false` is marked in the header, so the blocking step is visible
before the submit. `goto_page(key)` navigates by page key and `move_page(key, index)` reorders the
pages while the active page keeps its identity.

## Overflow

The form does not scroll. `size_hint()` reports the height the fields ask for; a host that gives
the control less than that paints the overflow outside its rectangle (`overflows()` answers the
question). Give the form its `size_hint` height, or wrap it in a `ScrollArea`, when the field count
is unbounded.

## Enter submits through the field

The keyboard router delivers a key to the focused control, not to its ancestors, and a form is not
focusable — its fields are. A standalone form therefore submits on Enter only when the host wires
the focused field's own `return_pressed` signal to `Form::submit`; a `FormSet` (or any
registry-forwarding container) forwards the key to its current form for you.

## Values live in the child controls

A form does not store a second copy of any value. `value_of(key)` reads the child's own
`text` / `value` / `current_text` property, `set_value_of(key, value)` writes it back, and a
required check is "the property is empty". The write path probes the same spellings (`text`, then
`current_text`, then `value` as `Int` / `UInt` / `Float`), so a bound editable combo or numeric
field is writable and not just readable. A user's keystrokes reach the child directly, so the form
re-reads the fields on the next event it forwards and publishes `field_changed` plus the
`dirty`/`valid` edges; a host that writes a child behind the form's back calls `refresh()`.

A field may also carry a host rule: `set_field_validator(key, |value| …)` runs after the required
check inside `validate` / `validate_field`, and `clear_field_validator` removes it. That is what
keeps `Form`, the child control and the host from disagreeing about the same fact.

## Adding a group later

1. add `src/<group>/` with `mod.rs` (declarations + re-exports only), `capability.rs` (schemas,
   constructors, `install`) and one file per control;
2. add one line to `install` in `src/lib.rs` that calls the group's installer;
3. add the group's name to this table.

A group owns its names, schemas and constructors, so adding one never edits another.

## Profiles

The suite targets a device profile (`desktop` / `tablet` / `mobile`), like the parent's full widget
layer. It is not part of `mini` / `embedded`: those builds have no widget registry to mount a form's
fields into.

## Licence

MIT — see the parent repository's `LICENSE`.
