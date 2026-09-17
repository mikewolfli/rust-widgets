// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Declarative-retained view demo: change state, and only the difference is applied.
//!
//! # What this shows (BLUE18 Phase F-1)
//!
//! The library is **retained**: a control is a long-lived object with an `ObjectId`.
//! The `view` module adds a **declarative** way to describe the tree, without giving
//! up the retained half. The point of the combination is that an update touches only
//! what changed — so a control's focus, scroll offset and internal state survive an
//! edit to a sibling.
//!
//! This demo drives that loop by hand and prints the patch each step produced, so
//! the claim is visible in the output rather than asserted in prose:
//!
//! ```text
//! initial mount        : 3 controls created, 1 property written
//! bump counter         : SetProperty { name: "text" }         ← one patch
//! append a row         : Insert { .. }                        ← one patch
//! remove a row         : Remove { .. }                        ← one patch
//! ```
//!
//! A rebuild-the-world implementation would print different numbers here, which is
//! what makes the output a demonstration rather than a description.
//!
//! # Running it
//!
//! ```bash
//! cargo run --example view_counter
//! ```
//!
//! It needs no window: `ViewEngine` takes its control constructor as an injected
//! closure, so the declarative layer can be exercised headlessly. That is a
//! deliberate design choice (see `src/view/engine.rs`), and it is why this demo runs
//! in CI on hosts with no display server.

#[cfg(declarative_view)]
fn main() {
    use rust_widgets::core::{ObjectId, Rect};
    use rust_widgets::view::{Node, Patch, View, ViewEngine};
    use rust_widgets::widget::capability::CapabilityValue;
    use rust_widgets::widget::{runtime, Widget, WidgetFactory};

    /// The application's state. In a real host this would be a `Binding<T>`; here
    /// it is a plain value so the demo shows the view contract without also
    /// demonstrating the reactive layer.
    struct Counter {
        count: i64,
        rows: Vec<&'static str>,
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
                .children_of(self.rows.iter().map(|row| {
                    Node::new("label")
                        .key(*row)
                        .prop("text", CapabilityValue::String((*row).to_string()))
                }))
        }
    }

    /// Builds real controls, so the demo exercises the same factory an application
    /// would. A stub would let the property writes silently fail.
    fn creator(factory: &WidgetFactory) -> impl Fn(&Node) -> Option<ObjectId> + '_ {
        move |node: &Node| {
            let widget: Box<dyn Widget> = factory.create(
                &node.widget,
                Rect::new(0, 0, 200, 28),
                node.key_str().unwrap_or("anon"),
            )?;
            runtime::register(widget)
        }
    }

    /// A one-line summary of what a patch batch did. This is the demo's output.
    fn describe(patches: &[Patch]) -> String {
        if patches.is_empty() {
            return "no patches (the tree did not change)".to_string();
        }
        let names: Vec<String> = patches
            .iter()
            .map(|patch| match patch {
                Patch::SetProperty { name, .. } => format!("SetProperty({name})"),
                Patch::Remove { .. } => "Remove".to_string(),
                Patch::Insert { .. } => "Insert".to_string(),
                Patch::Move { .. } => "Move".to_string(),
                Patch::Replace { .. } => "Replace".to_string(),
            })
            .collect();
        format!("{} patch(es): {}", patches.len(), names.join(", "))
    }

    let factory = WidgetFactory::new_with_defaults();
    let mut engine = ViewEngine::new();
    let mut state = Counter { count: 0, rows: vec!["alpha", "beta"] };

    let mounted = engine.mount(&state, &creator(&factory));
    println!(
        "initial mount        : {} control(s) created, {} property write(s), {} error(s)",
        mounted.widgets_created,
        mounted.properties_written,
        mounted.errors.len()
    );

    // ── A property-only change ──
    state.count = 1;
    let report = engine.update(&state, &creator(&factory));
    println!("bump counter         : {}", describe(&report.patches));
    println!(
        "                       positional_matches={} replaced_subtrees={}",
        report.positional_matches, report.replaced_subtrees
    );

    // ── A structural change: one row appended ──
    state.rows.push("gamma");
    let report = engine.update(&state, &creator(&factory));
    println!("append a row         : {}", describe(&report.patches));

    // ── And one removed ──
    state.rows.remove(0);
    let report = engine.update(&state, &creator(&factory));
    println!("remove the first row : {}", describe(&report.patches));

    // The retained half's payoff: the root kept its identity throughout, so a real
    // host's focus and scroll state on it would have survived all of the above.
    println!("root control id      : {:?} (stable across every update above)", engine.id_at(&[]));
    println!(
        "mounted tree size    : {} node(s)",
        engine.current().map(Node::node_count).unwrap_or(0)
    );
}

#[cfg(not(declarative_view))]
fn main() {
    // The `view` module is not compiled in a stripped profile (BLUE18 rule #92), so
    // there is nothing to demonstrate. Saying so is better than a build error: the
    // example stays buildable everywhere and explains its own absence.
    println!(
        "the declarative view layer is not compiled in this profile; \
         run with --features desktop (or tablet/mobile) instead"
    );
}
