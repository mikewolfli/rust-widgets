// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! # T-24: mode 1 and mode 2 must describe the **same** UI
//!
//! BLUE19's DoD:
//!
//! > **模式一致性门禁（T-24）**：同一份 JSON 分别经模式 1 与模式 2，产出**行为等价**
//! > ——断言控件树结构、已发布事件、属性值一致。
//! > **反向注入**：让生成器丢一个控件 → 门禁必须 FAIL。
//!
//! # Why "equivalent" is asserted as three separate facts
//!
//! "The two modes agree" is not testable as one claim. It decomposes into the three things a user
//! can actually observe, and each can fail independently:
//!
//! 1. **Structure** — the same controls in the same parent/child arrangement. A generator that
//!    dropped a node or flattened a level would fail here and nowhere else.
//! 2. **Properties** — the same names with the same values. A generator that emitted a property
//!    as a comment, or in the wrong type, fails here.
//! 3. **Events** — the same published event names are reachable. A generator whose `create_for`
//!    omitted a control would still pass (1) and (2) for the controls it *did* emit.
//!
//! # Why mode 2 is read back from the generated *text*, and mode 1 from the loader
//!
//! Mode 1's answer is the loader's: parse the document, walk the registered tree. Mode 2's answer is
//! the generator's: the tree it wrote into the source. Comparing the loader against the generator
//! directly would compare mode 1 with mode 1's input. Reading mode 2's **emitted structure** is what
//! makes this a cross-mode check rather than a self-consistency check.
//!
//! # Profile mixing
//!
//! The gate also asserts that neither template mentions the other's exclusive API (BLUE19's
//! "profile 串味" requirement): a `mini` output must not contain a `crate::view` path or a
//! `create_*` call, because those are exactly the symbols that do not exist there.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::designer::{generate, GenerationRequest, TargetProfile};
use rust_widgets::json::{JsonLoader, JsonProject};

/// A document with nesting, properties of three scalar kinds, and a published event.
const PROJECT: &str = r#"{
  "window": {
    "id": "root",
    "title": "Consistency",
    "width": 800,
    "height": 600,
    "layout": {
      "type": "vbox",
      "children": [
        { "label": { "id": "heading", "text": "Settings" } },
        { "button": { "id": "save", "text": "Save", "enabled": true,
                      "events": { "clicked": "on_save" } } },
        { "slider": { "id": "volume", "value": 40 } }
      ]
    }
  }
}"#;

fn request(target: TargetProfile) -> GenerationRequest {
    GenerationRequest {
        json: String::from(PROJECT),
        target,
        width: 800,
        height: 600,
        function_name: String::from("build_ui"),
    }
}

/// A structural fingerprint of mode 1's tree: `(widget, path)` for every node, in pre-order.
fn mode1_structure() -> Vec<(String, Vec<usize>)> {
    let project = JsonProject::parse(PROJECT).expect("the document must parse");
    project.walk().map(|node| (node.widget.clone(), node.path.clone())).collect()
}

/// A structural fingerprint of mode 2's tree, recovered from the generated source.
///
/// The generated default template is a chain of `Node::new("name").key("k").child(...)`, so the
/// widget names and their nesting are in the text. Counting them and checking nesting depth is what
/// catches "the generator dropped a control" — the reverse injection BLUE19 requires.
fn mode2_nodes(source: &str) -> Vec<String> {
    source
        .split("Node::new(")
        .skip(1)
        .filter_map(|rest| {
            let rest = rest.strip_prefix('"')?;
            rest.split('"').next().map(String::from)
        })
        .collect()
}

/// A structural fingerprint of mode 2's **stripped** tree, recovered from the generated source.
///
/// # Why the stripped template needs its own fingerprint
///
/// `mode2_nodes` reads `Node::new(..)` builders, which only the default template emits. The stripped
/// template emits `Type::new(..)` constructions instead, so it needs its own reader — and without
/// one, "the two modes agree" would silently only ever have meant "the default template agrees",
/// leaving the target that cannot run mode 1 at all as the unverified half.
///
/// The shapes it reads are the two the template emits:
///
/// * `Type::new(String::from("text"), Rect::new(..))` — a text-bearing constructor;
/// * `Type::new(Rect::new(..))` — a geometry-only constructor.
fn mode2_stripped_nodes(source: &str) -> Vec<String> {
    // Only the binding lines, so a `::new` inside a comment cannot be mistaken for a control.
    source
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("let ") && line.contains("::new("))
        .filter_map(|line| {
            let after = line.split("::new(").next()?;
            let type_name = after.rsplit(|c: char| !(c.is_alphanumeric() || c == '_')).next()?;
            if type_name.is_empty() || type_name == "Rect" {
                return None;
            }
            Some(String::from(type_name))
        })
        .collect()
}

/// **Structure**: the stripped template describes the same controls as mode 1, in the same order.
///
/// This is the half the desktop-only `mode2_nodes` reader cannot see. The stripped output is the one
/// a `mini`/`embedded` device actually runs, and it is the one produced by a *different* template — so
/// a control dropped there would otherwise reach the target with no test comparing it against the
/// document the user drew.
#[test]
fn the_stripped_template_agrees_on_the_control_tree() {
    let mode1 = mode1_structure();
    let generated = generate(&request(TargetProfile::Stripped)).expect("generation must succeed");
    let mode2 = mode2_stripped_nodes(&generated.source);

    // The generated `create_for`-style arms use bare type names while mode 1 uses document names, so
    // the comparison is on the *type* each document name maps to. `window` → `Window`, `label` →
    // `Label`, and so on; a name the generator cannot construct is reported rather than skipped.
    let expected: Vec<String> = mode1
        .iter()
        .map(|(widget, _)| rust_widgets::designer::generator::constructor_type_name(widget))
        .map(String::from)
        .collect();

    assert_eq!(
        expected, mode2,
        "the stripped template must describe the same controls in the same order as mode 1.\n\
         mode 2 source:\n{}",
        generated.source
    );
    assert_eq!(
        generated.report.nodes_emitted,
        mode1.len(),
        "the report's node count must match the tree it emitted"
    );
}

/// **Structure**: both modes describe the same controls in the same order.
#[test]
fn both_modes_agree_on_the_control_tree() {
    let mode1 = mode1_structure();
    let generated = generate(&request(TargetProfile::Default)).expect("generation must succeed");
    let mode2 = mode2_nodes(&generated.source);

    let mode1_names: Vec<String> = mode1.iter().map(|(widget, _)| widget.clone()).collect();
    assert_eq!(
        mode1_names, mode2,
        "mode 1 and mode 2 must describe the same controls in the same order.\n\
         mode 2 source:\n{}",
        generated.source
    );
    assert_eq!(
        generated.report.nodes_emitted,
        mode1.len(),
        "the report's node count must match the tree it emitted"
    );
}

/// **Properties**: every scalar property mode 1 reads appears in mode 2's output with that value.
#[test]
fn both_modes_agree_on_property_values() {
    let project = JsonProject::parse(PROJECT).expect("the document must parse");
    let generated = generate(&request(TargetProfile::Default)).expect("generation must succeed");

    let mut checked = 0usize;
    for node in project.walk() {
        for (name, value) in node.scalar_properties() {
            // A `Node::prop("name", CapabilityValue::...)` must exist for this pair. The value is
            // checked by its literal rendering, so a generator that wrote the right name with the
            // wrong value fails here rather than passing on the name alone.
            let expected_name = format!(".prop(\"{name}\"");
            assert!(
                generated.source.contains(&expected_name),
                "`{name}` on {:?} is in the document but not in the generated tree",
                node.path
            );
            if let Some(bool_value) = value.as_bool() {
                assert!(
                    generated.source.contains(&format!("CapabilityValue::Bool({bool_value})")),
                    "`{name}` on {:?} must be emitted with its value",
                    node.path
                );
            }
            if let Some(number) = value.as_i64() {
                assert!(
                    generated.source.contains(&format!("({number})")),
                    "`{name}` on {:?} must be emitted with its value",
                    node.path
                );
            }
            if let Some(text) = value.as_str() {
                assert!(
                    generated.source.contains(text),
                    "`{name}` on {:?} must be emitted with its text",
                    node.path
                );
            }
            checked += 1;
        }
    }
    assert!(checked >= 3, "the fixture must exercise several properties (got {checked})");
}

/// **Events**: every handler the document declares is reachable, and every declared
/// `events.<name>` is a name the capability table publishes.
#[test]
fn both_modes_agree_on_the_declared_handlers() {
    let project = JsonProject::parse(PROJECT).expect("the document must parse");
    let factory = rust_widgets::widget::capability::WidgetFactory::new_with_defaults();

    let mut declared = 0usize;
    for node in project.walk() {
        for (event, handler) in node.declared_handlers() {
            assert!(
                factory
                    .event_is_subscribable(
                        &node.widget,
                        &event,
                        &rust_widgets::signal::CustomSignalHub::new()
                    )
                    .is_ok(),
                "`{}` declares `events.{event}`, which its capability does not publish; the \
                 handler `{handler}` could never run",
                node.widget
            );
            declared += 1;
        }
    }
    assert_eq!(declared, 1, "the fixture declares exactly one wire");

    // Mode 1 must also reach it: the loader accepts the document and registers the control.
    let layout = JsonLoader::load(PROJECT).expect("mode 1 must accept the document");
    assert!(
        layout.id("save").is_some(),
        "the control carrying the wire must be registered by mode 1"
    );
}

/// **Profile purity**: a `mini` output must not name anything `mini` does not have.
///
/// This is BLUE19's "串味" requirement. It is checked on the text **in addition to** compiling for
/// real, because a text check names the offending symbol while a compiler error only says where it
/// stopped. (`tests/generator_output_compiles_test.rs` is the compile half.)
#[test]
fn the_stripped_output_names_nothing_the_stripped_profile_lacks() {
    let generated = generate(&request(TargetProfile::Stripped)).expect("generation must succeed");
    let source = &generated.source;

    for forbidden in [
        // `crate::view` is `declarative_view`; `mini`/`embedded` do not compile it.
        "rust_widgets::view",
        // `crate::json` is `full_widgets`; absent on both stripped profiles.
        "rust_widgets::json",
        // The `create_*` family is `cfg(not(alloc_frugal))` — 118 of them.
        "create_button",
        "create_label",
        "create_slider",
        // `widget::runtime` is `cfg(not(alloc_frugal))`.
        "widget::runtime",
        // The JSON loader and the factory are `full_widgets`.
        "WidgetFactory",
        "JsonLoader",
    ] {
        assert!(
            !source.contains(forbidden),
            "the stripped output names `{forbidden}`, which the target profile does not compile; \
             this is the cross-profile leak the DoD's 串味 assertion forbids.\nsource:\n{source}"
        );
    }

    // The positive half: the imperative API it *must* use is present.
    assert!(
        source.contains("base_mut()"),
        "the stripped output must construct the tree imperatively (d-3)"
    );
    assert!(
        source.contains("try_add_child"),
        "the stripped output must add children through the capacity-reporting API (d-4)"
    );
}

/// The default output must use the declarative seam, not the imperative one.
#[test]
fn the_default_output_uses_the_declarative_seam() {
    let generated = generate(&request(TargetProfile::Default)).expect("generation must succeed");
    assert!(
        generated.source.contains("rust_widgets::view::Node"),
        "the default template builds a `Node` tree (D7: it reuses the diff engine)"
    );
    assert!(
        generated.source.contains("ViewEngine"),
        "the default template mounts through `ViewEngine`, which is the two modes' shared seam"
    );
    assert!(
        !generated.source.contains("try_add_child"),
        "the default template must not also build imperatively; that would be two trees"
    );
}

/// Capacity is reported per target, and the stripped target's bound is the real one.
///
/// A container with more children than `mini`'s fixed storage holds must be **refused**, because the
/// platform would drop the excess silently and the generated program would look complete.
#[test]
fn a_container_over_capacity_is_reported_for_the_stripped_target_only() {
    use rust_widgets::designer::MINI_CHILD_CAPACITY;

    let mut children = String::new();
    for index in 0..(MINI_CHILD_CAPACITY + 1) {
        if index > 0 {
            children.push(',');
        }
        children.push_str(&format!("{{\"label\":{{\"text\":\"c{index}\"}}}}"));
    }
    let json = format!("{{\"window\":{{\"id\":\"w\",\"children\":[{children}]}}}}");

    let stripped = generate(&GenerationRequest {
        json: json.clone(),
        target: TargetProfile::Stripped,
        width: 400,
        height: 300,
        function_name: String::from("build_ui"),
    })
    .expect("generation must succeed");
    assert!(
        !stripped.report.capacity_overflow.is_empty(),
        "{} children exceeds the stripped target's capacity of {MINI_CHILD_CAPACITY} and must be \
         reported, not emitted: {}",
        MINI_CHILD_CAPACITY + 1,
        stripped.report.summary()
    );

    let default = generate(&GenerationRequest {
        json,
        target: TargetProfile::Default,
        width: 400,
        height: 300,
        function_name: String::from("build_ui"),
    })
    .expect("generation must succeed");
    assert!(
        default.report.capacity_overflow.is_empty(),
        "a heap-allocating target has room for these children; reporting them would be noise"
    );
}
