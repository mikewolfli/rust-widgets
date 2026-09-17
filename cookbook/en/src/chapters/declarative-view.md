# Declarative View Layer

> **Platform availability.** `rust_widgets::view` is compiled on `desktop`, `tablet` and
> `mobile` **by default**. Pass `no-declarative-view` to leave it out of a device build:
>
> ```bash
> cargo build --no-default-features --features desktop,no-declarative-view
> ```
>
> On `mini` and `embedded` the module **does not exist** — those profiles use `add_child`
> and the `create_*` functions. See [Platform Support](platform-support.md).

## What it does

It lets you describe a widget tree as a **function of your state**, and works out what
changed for you:

```rust
impl View for Counter {
    fn build(&self) -> Node {
        // "the screen is this, given self"
    }
}
```

```rust
engine.mount(&state, &create);                 // build the tree
state.count += 1;
let report = engine.update(&state, &create);   // one SetProperty, nothing else
```

## Why use it

Without it, every place that mutates state also has to reach the right control and know
which property to set. That knowledge spreads over every mutation site, and "what should
the screen look like when `count == 3`" is answered nowhere in particular. With it, that
question has exactly one answer: `build`.

Updates also become cheap and **local**. The engine diffs the new tree against the previous
one and touches only what differs, so a control's focus, scroll offset and internal state
survive an edit to a sibling — which is what makes this different from tearing the tree down
and rebuilding it each frame.

**The retained model is unchanged.** Controls are still long-lived objects with an
`ObjectId`, `add_child` still works, and the two styles can be mixed in one app. This layer
adds a *description* of structure; it replaces nothing. (React, Flutter and SwiftUI are all
declarative *and* retained for the same reason: the two are orthogonal.)

### When it pays off

| Use it when | Skip it when |
|---|---|
| A tree changes with state, and you have a previous tree to compare against | You build a tree once and never change it — `JsonLoader::load` already does that, and a diff over a static tree is pure overhead |
| You want one place that answers "what should the UI be?" | You set one property once — `btn.set_text("x")` is cheaper than describing a whole tree to produce one patch |
| Several controls derive from shared state | You are animating a value frame by frame — that is `PropertyAnimation`'s job |

## How to use it

### 1. Implement `View::build`

Return a `Node` tree from your state:

```rust
use rust_widgets::view::{Node, View};
use rust_widgets::widget::capability::CapabilityValue;

struct Counter {
    count: i64,
    items: Vec<String>,
}

impl View for Counter {
    fn build(&self) -> Node {
        Node::new("group_box")
            .key("root")
            .child(
                Node::new("label")
                    .key("count")
                    .prop("text", CapabilityValue::String(format!("Count: {}", self.count))),
            )
            .children_of(
                self.items.iter().map(|item| {
                    Node::new("label")
                        .key(item.clone())          // stable identity per item
                        .prop("text", CapabilityValue::String(item.clone()))
                }),
            )
    }
}
```

`Node::new(name)` takes the **factory name** — the same spelling `JsonLoader` uses
(`"group_box"`, `"label"`, `"listview"`, …). `prop` takes the same property names a control
publishes, so a typo is a refused write rather than a silent no-op.

`build` must be **pure**: no clock, no randomness. It may be called any number of times, and
a value that changes between calls would make the diff see changes that did not come from
state.

### 2. Give it a constructor

The engine does not know how to build a control; you tell it:

```rust
use rust_widgets::core::{ObjectId, Rect};
use rust_widgets::view::Node;
use rust_widgets::widget::{runtime, Widget, WidgetFactory};

let factory = WidgetFactory::new_with_defaults();
let create = move |node: &Node| -> Option<ObjectId> {
    let widget: Box<dyn Widget> = factory.create(
        &node.widget,
        Rect::new(0, 0, 200, 28),
        node.key_str().unwrap_or("anon"),
    )?;
    runtime::register(widget)
};
```

It is injected rather than hardwired, so the layer can be exercised **without a window**
(that is how its own tests run), and a host with its own construction policy — a design tool
reusing a control pool, a test that must stub — is not forced through the loader.

### 3. Mount once, update on change

```rust
use rust_widgets::view::ViewEngine;

let mut state = Counter { count: 0, items: vec!["alpha".into(), "beta".into()] };
let mut engine = ViewEngine::new();

engine.mount(&state, &create);        // builds the tree

state.count = 1;
let report = engine.update(&state, &create);

assert_eq!(report.patches.len(), 1);  // one SetProperty, nothing else
```

`update` before `mount` mounts instead, so a caller that does not want to distinguish the
first call from the rest does not have to.

### 4. Give list items a `key`

A `key` is how the diff recognises *the same control* across rebuilds. Without one, matching
falls back to position — and then inserting at the head shifts every later node's identity,
moving focus and scroll state onto the wrong controls.

```rust
// Good: a head insert leaves the others alone.
.children_of(rows.iter().map(|r| Node::new("label").key(r.id)))

// Degraded: matched by position, so a head insert renumbers everything after it.
.children_of(rows.iter().map(|r| Node::new("label")))
```

Keys must be unique among siblings. `Node::duplicate_sibling_keys()` reports violations, and
`tools/check_view_keys_are_unique.sh` fails the build on a literal duplicate.

The degradation is **reported, not silent** — check the report:

```rust
let report = engine.update(&state, &create);
if report.positional_matches > 0 {
    log::warn!("{} nodes matched positionally; add Node::key(..)", report.positional_matches);
}
```

| `DiffReport` field | Meaning | What to do |
|---|---|---|
| `patches` | The changes to apply | — |
| `positional_matches` | Nodes matched by **position** because they had no `key` | Non-zero means "add keys" |
| `replaced_subtrees` | Subtrees rebuilt because their type or key changed | Expected when a node genuinely becomes a different control |

## Driving it from reactive state

`Binding<T>` can be set from any thread, so `BindingListener` requires `Send`. A `ViewEngine`
and its controls are `!Send`, because the widget registry is thread-local. **A listener
therefore cannot hold the engine** — that is the contract, and it names the only sound design:

```text
  worker thread                      UI thread
  ─────────────                      ─────────
  binding.set(v)
    └─ listener fires   ──queue──▶   host.pump()
                                       └─ view.build()
                                          └─ diff → apply   (touches controls)
```

```rust
use rust_widgets::data_binding::Binding;
use rust_widgets::view::ReactiveHost;
use std::sync::Arc;

let text = Arc::new(Binding::new(String::from("first")));

// The view borrows the binding, so `build` re-reads the current value every time.
struct Greeting<'a> { text: &'a Binding<String> }
// impl View for Greeting<'_> { .. }

let mut host = ReactiveHost::new(Greeting { text: &text }, Box::new(create));
host.mount();
host.subscribe(&text);

// On any thread:
text.set(String::from("second"));

// On the UI thread, typically once per frame. Returns how many rebuilds it did;
// 0 is the common case and costs one uncontended lock.
let rebuilds = host.pump();
```

It is a queue rather than an atomic flag so the number of updates stays equal to the number
of sets. A flag would lose intermediate values, making "how many patches did that change
cause" depend on timing — which breaks both tests and reasoning.

## Putting a patch on screen

`apply` writes through each control's property contract. What happens next depends on your
host:

```rust
host.pump();
// then repaint whatever your surface policy requires
```

> **On partial repaint — you do not have to do anything.** `apply` writes through each
> control's property contract, and that write ends in the control's own
> `request_redraw`, which records the damage. So a declarative update produces the same
> damage a hand-written one would: nothing in this layer has to mark a rectangle, and
> a view does not need a special repaint policy.
>
> How much gets repainted is decided for you at **mount time**. A surface large enough,
> holding more than one control, that has actually asked to be repainted is enabled in
> `RepaintMode::Adaptive`; anything else stays in `RepaintMode::Full`. `Adaptive` is
> self-correcting — a frame whose damage covers the surface falls back to a whole paint
> for that frame and resumes regioning when the damage shrinks — so the decision cannot
> produce a wrong frame, only a bounded amount of bookkeeping. You can ask explicitly
> with `enable_damage_tracking_if_useful(window)`, or force a policy with
> `set_repaint_mode`. See [Performance & Quality](performance-quality.md) for the full
> pipeline.

## Testing without a window

Because the constructor is injected, the whole layer is testable with no display:

```rust
let mut engine = ViewEngine::new();
let ids = StubIds::new(10);          // hands out 10, 11, 12, …
engine.mount(&state, &ids.creator());

assert_eq!(engine.id_at(&[]), Some(10));   // the root
assert_eq!(engine.id_at(&[0]), Some(11));  // its first child
```

`id_at(&[0, 2])` answers "is this still the control I had focus on?" without walking the
tree — which is how you assert that an update preserved identity.

## The pieces

| Type | What it is |
|---|---|
| `Node` | A tree described as a value: widget name, optional `key`, properties, children. No ids, no live controls, no platform calls — which is what makes `diff` a pure function |
| `View` | A trait with one method, `build(&self) -> Node` |
| `ViewEngine` | Holds the previous tree, calls `diff`, and applies the result |
| `Patch` | One change: `SetProperty` / `Insert` / `Remove` / `Move` / `Replace` |
| `ReactiveHost` | Ties a `Binding` to an engine so a `set` from any thread drives `update` |

`Patch::SetProperty` lands on each control's own published property contract — the same one
the JSON loader writes through — so the declarative layer cannot invent a property a control
does not have. `apply` is the only function in the layer that mutates anything.

## What it does not do

- **No component model** — no `context`, no hooks, no component boundary. Controls are flat
  `WidgetKind`s; introducing those abstractions would build a hierarchy the library does not
  have.
- **No async or concurrent diff** — controls are `!Send`, so the diff runs on the UI thread.
- **No animation interpolation** — animation is `PropertyAnimation`'s job. This layer
  expresses *target* state; mixing in time would make the diff non-deterministic.
- **No style cascade in the diff** — `Patch::SetProperty` covers properties; stylesheets are
  the CSS layer's business.
- **Not on `mini` / `embedded`**, and not exposed over the C ABI — Rust only.
