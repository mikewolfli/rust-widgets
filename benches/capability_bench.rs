// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Benchmarks for the capability/property hot path and the event queue's batch drain.
//!
//! The 2026-10-21 scan (`docs/issues/issue-20261021.md`, `BIND-19`) listed four P1 hot paths with
//! no benchmark coverage. These are the ones that can be measured without a window or a GPU: the
//! per-call capability-factory rebuild (`T1`), the event queue's per-turn drain (`CORE-04` /
//! `CORE-08`), repeated property **writes** through a typed handle, and the theme sweep over a live
//! widget set. The property-read bench exercises the shared factory through the same accessor a
//! `WindowHandle` uses.

#[cfg(not(target_arch = "wasm32"))]
use criterion::{criterion_group, criterion_main, Criterion};
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::app::CheckBoxHandle;
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::core::Rect;
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::event::{Event, EventPriority, EventQueue};
#[cfg(all(not(target_arch = "wasm32"), declarative_view))]
use rust_widgets::widget::capability::CapabilityValue;
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::widget::WidgetFactory;
#[cfg(not(target_arch = "wasm32"))]
use std::hint::black_box;

/// A 201-node declarative tree: a window of 40 rows of 5 labels.
///
/// Every node is keyed and every key is unique, so the diff has no ambiguity to resolve and
/// the benchmark measures the walking, not the matching policy.
///
/// Gated on the build-script alias, not on `desktop`: `desktop,no-declarative-view` still turns
/// the `declarative-view` *feature* on (Cargo unifies additively) while `build.rs` resolves the
/// cfg to false, so `required-features` alone cannot express this and the file must not name
/// `rust_widgets::view` in that configuration.
#[cfg(all(not(target_arch = "wasm32"), declarative_view))]
fn sample_view_tree() -> rust_widgets::view::Node {
    let rows = (0..40).map(|i| {
        rust_widgets::view::Node::new("row").key(format!("row{i}")).children_of((0..5).map(|j| {
            rust_widgets::view::Node::new("label")
                .key(format!("cell{i}_{j}"))
                .prop("text", CapabilityValue::String(format!("r{i}c{j}")))
        }))
    });
    rust_widgets::view::Node::new("window").key("root").children_of(rows)
}

/// An **idle** rebuild: the same tree diffed against itself, which is what a `View` that is
/// re-evaluated every frame with unchanged state costs (`REND-15`).
#[cfg(all(not(target_arch = "wasm32"), declarative_view))]
fn bench_view_diff_idle(c: &mut Criterion) {
    let tree = sample_view_tree();
    let lookup = |_: &[usize], _: usize| -> Option<rust_widgets::core::ObjectId> { Some(1) };
    c.bench_function("view_diff_idle_201_nodes", |b| {
        b.iter(|| black_box(rust_widgets::view::diff(&tree, &tree, &lookup)))
    });
}

/// One leaf changed in a 201-node tree: the diff must stay proportional to the **changed
/// paths** rather than to the tree, so this should be far cheaper than re-walking everything.
#[cfg(all(not(target_arch = "wasm32"), declarative_view))]
fn bench_view_diff_one_changed_leaf(c: &mut Criterion) {
    let old = sample_view_tree();
    let mut new = old.clone();
    new.children[20].children[2]
        .props
        .insert("text".to_string(), CapabilityValue::String("changed".to_string()));
    let lookup = |_: &[usize], _: usize| -> Option<rust_widgets::core::ObjectId> { Some(1) };
    c.bench_function("view_diff_one_changed_leaf_201_nodes", |b| {
        b.iter(|| black_box(rust_widgets::view::diff(&old, &new, &lookup)))
    });
}

/// Building the full registry from scratch: the cost every property access used to pay.
#[cfg(not(target_arch = "wasm32"))]
fn bench_factory_new_with_defaults(c: &mut Criterion) {
    c.bench_function("widget_factory_new_with_defaults", |b| {
        b.iter(|| black_box(WidgetFactory::new_with_defaults()))
    });
}

/// Borrowing the process-wide registry: the cost after `T1` was fixed.
#[cfg(not(target_arch = "wasm32"))]
fn bench_factory_shared(c: &mut Criterion) {
    c.bench_function("widget_factory_shared", |b| b.iter(|| black_box(WidgetFactory::shared())));
}

/// One property read through the shared factory.
#[cfg(not(target_arch = "wasm32"))]
fn bench_property_read(c: &mut Criterion) {
    let factory = WidgetFactory::shared();
    let widget = factory
        .create("slider", Rect::new(0, 0, 120, 24), "")
        .expect("slider is a registered control");
    c.bench_function("property_read_shared", |b| {
        b.iter(|| black_box(factory.read_property(widget.as_ref(), "value")))
    });
}

/// Draining a 512-event turn through the batch path (one lock, one refill).
#[cfg(not(target_arch = "wasm32"))]
fn bench_event_queue_batch_drain(c: &mut Criterion) {
    c.bench_function("event_queue_drain_into_512", |b| {
        b.iter(|| {
            let queue = EventQueue::new();
            let sender = queue.sender();
            for i in 0..512_u64 {
                sender
                    .post_with_priority(
                        i,
                        Event::MouseMove {
                            pos: rust_widgets::core::Point::new((i % 1024) as i32, 0),
                        },
                        EventPriority::Normal,
                    )
                    .expect("event post should succeed");
            }
            let mut high = Vec::new();
            let mut normal = Vec::new();
            let mut idle = Vec::new();
            let moved = queue.drain_into(&mut high, &mut normal, &mut idle, 512);
            black_box((moved, high.len(), normal.len(), idle.len()));
        })
    });
}

/// A repeated **write** through the typed handle (`BIND-19`): the path a host's state-sync loop
/// takes when it pushes the same or a toggling value every frame.
///
/// The value alternates so the write is never the same-value short circuit `WDGT-15` added — this
/// measures the real property-write path, which is what the benchmark is for.
#[cfg(not(target_arch = "wasm32"))]
fn bench_checkbox_set_checked(c: &mut Criterion) {
    let widget = WidgetFactory::shared()
        .create("checkbox", Rect::new(0, 0, 120, 24), "check")
        .expect("checkbox is a registered control");
    let id = rust_widgets::widget::runtime::register(widget).expect("this thread has a registry");
    let handle = CheckBoxHandle::from_raw(id);
    let mut checked = false;
    c.bench_function("checkbox_set_checked_repeated", |b| {
        b.iter(|| {
            checked = !checked;
            handle.set_checked(checked);
            black_box(())
        })
    });
    rust_widgets::widget::runtime::unregister(id);
}

/// Re-applying the active theme to ~1000 live widgets (`BIND-19`): a global theme switch walks
/// every mounted control through `reapply_active_theme`, so its cost is the cost of one switch.
///
/// The widgets are mounted through `widget::runtime::register` rather than a window surface: this
/// benchmark measures the sweep, and a headless bench has no surface to mount onto.
#[cfg(not(target_arch = "wasm32"))]
fn bench_reapply_active_theme_1000(c: &mut Criterion) {
    const WIDGETS: usize = 1000;
    let mut ids = Vec::with_capacity(WIDGETS);
    for i in 0..WIDGETS {
        let kind = if i % 2 == 0 { "button" } else { "label" };
        let widget = WidgetFactory::shared()
            .create(kind, Rect::new(0, 0, 120, 24), "theme")
            .expect("button and label are registered controls");
        if let Some(id) = rust_widgets::widget::runtime::register(widget) {
            ids.push(id);
        }
    }
    c.bench_function("reapply_active_theme_1000_widgets", |b| {
        b.iter(|| {
            rust_widgets::reapply_active_theme();
            black_box(())
        })
    });
    for id in ids {
        rust_widgets::widget::runtime::unregister(id);
    }
}

/// A 40-child flex row and its hints, built once so the benchmark measures layout, not setup.
#[cfg(not(target_arch = "wasm32"))]
fn flex_fixture() -> (rust_widgets::layout::FlexLayout, Vec<rust_widgets::layout::ChildInfo>) {
    use rust_widgets::layout::{ChildInfo, FlexLayout, Hints, Layout};
    let mut layout = FlexLayout::new();
    let children: Vec<ChildInfo> = (0..40)
        .map(|i| {
            let id = i as u64 + 1;
            layout.add_widget(id, 0);
            ChildInfo::new(id, Hints::fixed(18, 24))
        })
        .collect();
    (layout, children)
}

/// A repeated arrangement of the **same** rectangle: the case the result cache serves
/// (measure, arrange, paint). REND-01's cache key is hashed from the layout's own fields, so a
/// hit costs the hash plus a `Vec` copy instead of the four-pass flex solver.
#[cfg(not(target_arch = "wasm32"))]
fn bench_flex_arrange_cached(c: &mut Criterion) {
    use rust_widgets::layout::Layout;
    let (layout, children) = flex_fixture();
    let rect = rust_widgets::core::Rect::new(0, 0, 800, 40);
    let mut scratch = Vec::with_capacity(children.len());
    c.bench_function("flex_arrange_40_children_cached", |b| {
        b.iter(|| {
            scratch.clear();
            layout.arrange(rect, &children, &mut |_, child| scratch.push(child));
            black_box(scratch.len())
        })
    });
}

/// The same row with a **different** rectangle every iteration, which misses the cache and runs
/// the solver. Comparing the two gives the cache's saving on the arrangement itself.
#[cfg(not(target_arch = "wasm32"))]
fn bench_flex_arrange_uncached(c: &mut Criterion) {
    use rust_widgets::layout::Layout;
    let (layout, children) = flex_fixture();
    let mut width = 800u32;
    let mut scratch = Vec::with_capacity(children.len());
    c.bench_function("flex_arrange_40_children_uncached", |b| {
        b.iter(|| {
            width = if width == 800 { 801 } else { 800 };
            scratch.clear();
            let rect = rust_widgets::core::Rect::new(0, 0, width, 40);
            layout.arrange(rect, &children, &mut |_, child| scratch.push(child));
            black_box(scratch.len())
        })
    });
}

/// The declarative-view benches, kept in one always-registered entry so the `criterion_group!`
/// list stays valid when `declarative_view` is compiled out.
#[cfg(not(target_arch = "wasm32"))]
fn bench_view_suite(c: &mut Criterion) {
    #[cfg(declarative_view)]
    {
        bench_view_diff_idle(c);
        bench_view_diff_one_changed_leaf(c);
    }
    #[cfg(not(declarative_view))]
    let _ = c;
}

#[cfg(not(target_arch = "wasm32"))]
criterion_group!(
    benches,
    bench_factory_new_with_defaults,
    bench_factory_shared,
    bench_property_read,
    bench_checkbox_set_checked,
    bench_reapply_active_theme_1000,
    bench_event_queue_batch_drain,
    bench_view_suite,
    bench_flex_arrange_cached,
    bench_flex_arrange_uncached
);
#[cfg(not(target_arch = "wasm32"))]
criterion_main!(benches);

/// A bench target must define `main` in configurations where criterion is compiled out.
///
/// `criterion` is a host-only dev-dependency, so the wasm target compiles this file with the
/// group and `criterion_main!` removed. See `event_bench.rs` for the failure this pins.
#[cfg(target_arch = "wasm32")]
fn main() {}
