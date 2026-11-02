// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TreeIndex build and lookup cost at a document size the flat benchmarks did not reach
//! (RW-59).
//!
//! The existing benches stop at roughly 200 nodes. A designer document is a whole tree — the
//! capability census already reports 274 controls — so the index that answers "which id is at
//! this path" and "what is this id's key" is exercised here at 2 000 and 10 000 nodes. Both
//! directions are measured: `key_of` is the reverse lookup that used to be a linear scan (C-2),
//! and `id_at` is the forward lookup the update path takes per patch.

#[cfg(not(target_arch = "wasm32"))]
use criterion::{criterion_group, criterion_main, Criterion};
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::core::ObjectId;
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::view::TreeIndexBuilder;
#[cfg(not(target_arch = "wasm32"))]
use std::hint::black_box;

/// Builds a flat index of `count` siblings below one root, which is the widest shape a designer
/// panel produces (a long outline) and the worst case for a per-patch path lookup.
#[cfg(not(target_arch = "wasm32"))]
fn build_index(count: usize) -> rust_widgets::view::TreeIndex<ObjectId> {
    let mut builder = TreeIndexBuilder::<ObjectId>::new().generation(1);
    for index in 0..count {
        let id = (index + 1) as ObjectId;
        let key = format!("node_{index}");
        builder = builder.node(&[0, index], id, "button", Some(key.as_str()));
    }
    builder.build()
}

#[cfg(not(target_arch = "wasm32"))]
fn bench_tree_index_build(c: &mut Criterion) {
    for count in [2_000usize, 10_000] {
        c.bench_function(&format!("tree_index_build_{count}"), |b| {
            b.iter(|| black_box(build_index(count).len()));
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn bench_tree_index_lookup(c: &mut Criterion) {
    let index = build_index(10_000);
    let mid = 5_000 as ObjectId;
    c.bench_function("tree_index_key_of_10k", |b| {
        b.iter(|| black_box(index.key_of(mid).map(str::to_string)));
    });
    c.bench_function("tree_index_id_at_10k", |b| {
        b.iter(|| black_box(index.id_at(&[0, 5_000])));
    });
}

#[cfg(not(target_arch = "wasm32"))]
criterion_group!(benches, bench_tree_index_build, bench_tree_index_lookup);
#[cfg(not(target_arch = "wasm32"))]
criterion_main!(benches);

/// A bench target needs a `main` in every configuration; `criterion_main!` supplies it only where
/// the host-only dependency compiles.
#[cfg(target_arch = "wasm32")]
fn main() {}
