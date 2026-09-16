#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::event::{Event, EventPriority, EventQueue};
#[cfg(not(target_arch = "wasm32"))]
use std::hint::black_box;

#[cfg(not(target_arch = "wasm32"))]
fn bench_event_queue_post_and_dequeue_10k(c: &mut Criterion) {
    c.bench_function("event_queue_post_dequeue_10k", |b| {
        b.iter(|| {
            let queue = EventQueue::new();
            let sender = queue.sender();
            for i in 0..10_000_u64 {
                let event = Event::MouseMove {
                    pos: rust_widgets::core::Point::new((i % 1024) as i32, (i % 768) as i32),
                };
                sender
                    .post_with_priority(i, event, EventPriority::Normal)
                    .expect("event post should succeed");
            }

            let mut drained = 0_u64;
            while let Some((id, event, priority)) = queue.dequeue() {
                black_box((id, event, priority));
                drained += 1;
            }
            black_box(drained);
        })
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
criterion_group!(benches, bench_event_queue_post_and_dequeue_10k);
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
criterion_main!(benches);

/// Bench targets always need a `main`, whatever their body compiles to.
///
/// `criterion` is a host-only dev-dependency (it cannot build for wasm32), and the
/// widget runtime is absent from the alloc-frugal profile — but the bench *target*
/// exists in every configuration. Gating the whole file with a crate-level `#![cfg]`
/// removes the `criterion_main!`-generated `main` too, which made
/// `cargo check --all-targets --target wasm32-unknown-unknown` fail with E0601
/// ("main function not found in crate ..."). Gating the items and supplying this
/// entry point keeps every configuration a valid crate.
/// The fallback entry point for configurations where `criterion_main!` above is
/// compiled out.
///
/// `criterion` is a host-only dev-dependency (it cannot build for wasm32) — but the
/// bench *target* exists in every configuration, and a bench target must have a
/// `main`. Gating the whole file with a crate-level `#![cfg]` removed the
/// `criterion_main!`-generated `main` along with everything else, which made
/// `cargo check --all-targets --target wasm32-unknown-unknown` fail with E0601
/// ("main function not found in crate `event_bench`").
#[cfg(target_arch = "wasm32")]
fn main() {}
