// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Declarative **adaptive** view demo: one view, three window sizes, three different trees.
//!
//! # What this shows
//!
//! A size tier chooses a **subtree**, not a coordinate. The view declares "this sidebar exists on a
//! tablet and up" and the engine supplies the tier from the viewport the host reported, so the
//! phone and desktop layouts are two different trees rather than one tree with two arithmetic
//! paths. The output makes that visible: crossing a tier boundary shows up as `Insert`/`Remove`,
//! never as a property write.
//!
//! ```text
//! 360x800  (Compact) : 1 child(ren)  phone_bar
//! 700x900  (Medium)  : 1 child(ren)  rail
//! 1400x900 (Expanded): 1 child(ren)  sidebar + main
//! ```
//!
//! # Why this example exists
//!
//! `ViewEngine::set_viewport` was added because the breakpoint mechanism was **dead wiring**: the
//! module documentation said "the framework supplies the fact" and nothing ever did, so
//! `Breakpoint::current()` was permanently `Expanded` and every narrow-layout subtree was
//! unreachable in a real program. The engine's own unit tests cover the mechanism, but until an
//! example consumed it the path had no production caller at all. This is that caller.
//!
//! # Running it
//!
//! ```bash
//! cargo run --example view_adaptive
//! ```
//!
//! Like `view_counter` it needs no window: `ViewEngine` takes its control constructor as an
//! injected closure, so the whole loop runs headlessly and in CI on hosts with no display server.

#[cfg(declarative_view)]
fn main() {
    use rust_widgets::core::{ObjectId, Rect, Size};
    use rust_widgets::view::{Breakpoint, Node, Patch, View, ViewEngine};
    use rust_widgets::widget::capability::CapabilityValue;
    use rust_widgets::widget::{runtime, Widget, WidgetFactory};

    /// The application's state: a page and the items it lists. Independent of the viewport, which is
    /// the point — the *same* state produces a different tree at each tier.
    struct App {
        page: &'static str,
        items: Vec<&'static str>,
    }

    impl View for App {
        fn build(&self) -> Node {
            // One root, three declared regions. Which of them exist is decided by the tier the
            // engine established for this build, not by an `if` in a draw path — a branch taken while
            // *building* still sees the context `Hints` propagate upward, which a branch taken while
            // *painting* has already lost.
            Node::new("group_box")
                .key("root")
                // Narrow: a single bottom bar. This is what a phone gets.
                .breakpoint(
                    Breakpoint::Compact,
                    Node::new("label")
                        .key("phone_bar")
                        .prop("text", CapabilityValue::String(String::from("Phone bar"))),
                )
                // Tablet and up: a navigation rail instead of the bar.
                .breakpoint(
                    Breakpoint::Medium,
                    Node::new("label")
                        .key("rail")
                        .prop("text", CapabilityValue::String(String::from("Rail"))),
                )
                // Desktop: a sidebar beside the content.
                .breakpoint(
                    Breakpoint::Expanded,
                    Node::new("label")
                        .key("sidebar")
                        .prop("text", CapabilityValue::String(String::from("Sidebar"))),
                )
                // The content region exists at **every** tier, so it is declared
                // unconditionally. It is the control whose identity must survive a tier change:
                // a real host's caret and scroll offset live in it.
                .child(
                    Node::new("label")
                        .key("content")
                        .prop("text", CapabilityValue::String(self.page.to_string())),
                )
                .children_of(self.items.iter().map(|item| {
                    Node::new("label")
                        .key(*item)
                        .prop("text", CapabilityValue::String((*item).to_string()))
                }))
        }
    }

    /// Builds real controls through the same factory an application uses, so a property write that
    /// the contract refuses is a real refusal rather than a stub's silence.
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

    /// The keys of the mounted root's children, in order — the observable shape of the tree.
    fn child_keys(engine: &ViewEngine) -> Vec<String> {
        engine
            .current()
            .map(|root| {
                root.children
                    .iter()
                    .map(|child| child.key.clone().unwrap_or_else(|| String::from("<none>")))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A one-line summary of a patch batch.
    fn describe(patches: &[Patch]) -> String {
        if patches.is_empty() {
            return String::from("no patches (the tree did not change)");
        }
        let names: Vec<String> = patches
            .iter()
            .map(|patch| match patch {
                Patch::SetProperty { name, .. } => format!("SetProperty({name})"),
                Patch::Remove { .. } => String::from("Remove"),
                Patch::Insert { .. } => String::from("Insert"),
                Patch::Move { .. } => String::from("Move"),
                Patch::Replace { .. } => String::from("Replace"),
            })
            .collect();
        format!("{} patch(es): {}", patches.len(), names.join(", "))
    }

    /// The id of the keyed `content` control, or `None` when it is not mounted.
    ///
    /// Looked up **by key**, not by index: the tier decides how many regions precede it, so `id_at(&[0])`
    /// would name a different control at each viewport. A key is a statement about identity, and
    /// this is the lookup that honours it.
    fn content_id(engine: &ViewEngine) -> Option<ObjectId> {
        engine.layout().child_by_key(engine.layout().root(), "content")
    }

    let factory = WidgetFactory::new_with_defaults();
    let mut engine = ViewEngine::new();
    let state = App { page: "Dashboard", items: vec!["alpha", "beta"] };

    println!("=== one view, three viewports ===");
    println!(
        "  Breakpoint::Compact  = width <= {}",
        rust_widgets::widget::metrics::dimensions::BREAKPOINT_COMPACT_MAX
    );
    println!(
        "  Breakpoint::Medium   = width <= {}",
        rust_widgets::widget::metrics::dimensions::BREAKPOINT_MEDIUM_MAX
    );
    println!();

    // ── Phone ──
    engine.set_viewport(Size::new(360, 800));
    let mounted = engine.mount(&state, &creator(&factory));
    println!(
        "360x800  (Compact) : descriptor={:?} {} control(s) created, children={:?}",
        engine.viewport().map(Breakpoint::of),
        mounted.widgets_created,
        child_keys(&engine)
    );
    let content_on_phone = content_id(&engine);

    // ── Tablet: a tier change is structural ──
    engine.set_viewport(Size::new(700, 900));
    let report = engine.update(&state, &creator(&factory));
    println!(
        "700x900  (Medium)  : descriptor={:?} {}",
        engine.viewport().map(Breakpoint::of),
        describe(&report.patches)
    );
    println!("                     children={:?}", child_keys(&engine));

    // ── Desktop: again structural, never a property write ──
    engine.set_viewport(Size::new(1400, 900));
    let report = engine.update(&state, &creator(&factory));
    println!(
        "1400x900 (Expanded): descriptor={:?} {}",
        engine.viewport().map(Breakpoint::of),
        describe(&report.patches)
    );
    println!("                     children={:?}", child_keys(&engine));

    // ── The retained half's payoff ──
    //
    // The content region is declared at every tier, so its id is unchanged across all three
    // viewports: a real host's caret in it would have survived the resize. The regions that differ
    // between tiers, by contrast, are genuinely different controls.
    println!();
    println!("=== identity across the three tiers ===");
    println!("  content id at 360  : {:?}", content_on_phone);
    println!("  content id at 1400 : {:?}", content_id(&engine));
    println!(
        "  same control       : {} (a scroll offset or caret in it survived the resize)",
        content_on_phone == content_id(&engine)
    );
    println!(
        "  mounted tree size  : {} node(s)",
        engine.current().map(Node::node_count).unwrap_or(0)
    );

    // ── A state change at a fixed tier is *not* structural ──
    //
    // The complement of the above: within one tier a text edit is one property write, which is what
    // makes the structural patches above evidence of a tier change rather than of any update at all.
    let mut state = state;
    state.page = "Reports";
    let report = engine.update(&state, &creator(&factory));
    println!();
    println!("=== a state change at the same viewport ===");
    println!("  rename the page    : {}", describe(&report.patches));
}

#[cfg(not(declarative_view))]
fn main() {
    // The `view` module is not compiled in a stripped profile, so there is nothing to demonstrate.
    // Saying so keeps the example buildable everywhere and explains its own absence.
    println!(
        "the declarative view layer is not compiled in this profile; \
         run with --features desktop (or tablet/mobile) instead"
    );
}
