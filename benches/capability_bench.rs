// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Benchmarks for the capability/property hot path and the event queue's batch drain.
//!
//! The 2026-10-21 scan (`docs/issues/issue-20261021.md`, `BIND-19`) listed four P1 hot paths with
//! no benchmark coverage. These are the two that can be measured without a window or a GPU: the
//! per-call capability-factory rebuild (`T1`) and the event queue's per-turn drain (`CORE-04` /
//! `CORE-08`). The property-read bench exercises the shared factory through the same accessor a
//! `WindowHandle` uses.

#[cfg(not(target_arch = "wasm32"))]
use criterion::{criterion_group, criterion_main, Criterion};
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::core::Rect;
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::event::{Event, EventPriority, EventQueue};
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::widget::capability::CapabilityValue;
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::widget::WidgetFactory;
#[cfg(not(target_arch = "wasm32"))]
use std::hint::black_box;

/// A 201-node declarative tree: a window of 40 rows of 5 labels.
///
/// Every node is keyed and every key is unique, so the diff has no ambiguity to resolve and
/// the benchmark measures the walking, not the matching policy.
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
fn bench_view_diff_idle(c: &mut Criterion) {
    let tree = sample_view_tree();
    let lookup = |_: &[usize], _: usize| -> Option<rust_widgets::core::ObjectId> { Some(1) };
    c.bench_function("view_diff_idle_201_nodes", |b| {
        b.iter(|| black_box(rust_widgets::view::diff(&tree, &tree, &lookup)))
    });
}

/// One leaf changed in a 201-node tree: the diff must stay proportional to the **changed
/// paths** rather than to the tree, so this should be far cheaper than re-walking everything.
#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
criterion_group!(
    benches,
    bench_factory_new_with_defaults,
    bench_factory_shared,
    bench_property_read,
    bench_event_queue_batch_drain,
    bench_view_diff_idle,
    bench_view_diff_one_changed_leaf
);
#[cfg(not(target_arch = "wasm32"))]
criterion_main!(benches);

/// A bench target must define `main` in configurations where criterion is compiled out.
///
/// `criterion` is a host-only dev-dependency, so the wasm target compiles this file with the
/// group and `criterion_main!` removed. See `event_bench.rs` for the failure this pins.
#[cfg(target_arch = "wasm32")]
fn main() {}
