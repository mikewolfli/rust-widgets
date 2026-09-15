// criterion (benchmark harness) cannot compile for wasm32.
#![cfg(not(target_arch = "wasm32"))]
//! Benchmarks for the signal system (emit, connect, disconnect).
//!
//! Covers:
//! - `signal_emit_1..8_slots` — emit at realistic UI scale (one to eight handlers)
//! - `signal_emit_nested_1_level` / `signal_emit_with_self_disconnect` — the behaviour
//!   the take/restore design exists for, so its cost is visible
//! - `signal_emit_10/100/1000_slots` — throughput at larger fan-out
//! - `signal_connect` / `signal_disconnect` — the mutation operations
//! - `signal_emit_large_payload` — emit with a large `Arc<String>` payload
//!
//! # Why the small-slot-count group exists
//!
//! A UI signal usually has **one to three** slots — a button's `clicked` typically
//! has one handler. The original benchmarks started at 10 slots, so the regime the
//! library actually runs in was unmeasured, and any claim about emit overhead at
//! realistic scale was unsupported.
//!
//! `emit_1_slot` through `emit_8_slots` close that gap. Together with `emit_nested`
//! and `emit_with_self_disconnect` — the two behaviours the take/restore design
//! exists to support — they make it possible to tell whether a proposed change to
//! `emit` actually helps at the sizes that matter, and what it costs in the cases
//! the design was built for.
//!
//! # A measured dead end, recorded so it is not re-attempted
//!
//! The obvious optimisation — skip the `Vec` snapshot when the signal has exactly one
//! slot — was prototyped and measured against these benchmarks. It loses:
//!
//! | case | snapshot | single-slot fast path | delta |
//! |---|---:|---:|---:|
//! | 1 slot | 60.7 ns | 57.4 ns | **−3.3 ns** (−5%) |
//! | 2 slots | 81.3 ns | 99.2 ns | **+17.9 ns** (+22%) |
//! | 4 slots | 129.4 ns | 154.3 ns | **+24.9 ns** (+19%) |
//!
//! The predicted saving was ~47 ns (the `Vec` allocation plus the hash-map collect).
//! Only 3.3 ns materialised, because `HashMap::len()` followed by `keys().next()` costs
//! nearly what `collect()` did for a one-entry map — while the added branch regressed
//! every case with more than one slot. Against a 16.67 ms frame budget, 3.3 ns is
//! 0.00002% of a frame.
//!
//! **The current `emit` shape is the right one for this workload.** A future proposal
//! to restructure it should start by re-running these benchmarks rather than by
//! reasoning about which operations look expensive. What matters at realistic sizes is
//! the ~30 ns per-slot take/restore, and that cost is what buys self-disconnect and
//! re-entrancy — both of which are now pinned by tests in `signal::core_signal`.

use criterion::{criterion_group, criterion_main, Criterion};
use rust_widgets::signal::{ConnectionHandle, Priority, Signal};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Emits against `slot_count` no-op slots, for the given label.
fn bench_emit_at(c: &mut Criterion, slot_count: usize) {
    let signal = Signal::<u32>::new();
    for _ in 0..slot_count {
        signal.connect(|_v: Arc<u32>| {});
    }
    c.bench_function(&format!("signal_emit_{slot_count}_slots"), |b| b.iter(|| signal.emit(42)));
}

/// The regime a UI actually runs in: one handler per signal.
fn bench_signal_emit_small(c: &mut Criterion) {
    for n in [1usize, 2, 4, 8] {
        bench_emit_at(c, n);
    }
}

/// A slot that emits its own signal re-enters `emit`.
///
/// The re-entrant pass must skip the slot still on the stack (that is what makes
/// recursion terminate), so this measures the cost of the recursion guard: a
/// nested snapshot, sort, and per-handle lookup that finds nothing to call.
fn bench_signal_emit_nested(c: &mut Criterion) {
    let signal = Signal::<u32>::new();
    let depth = Arc::new(AtomicU64::new(0));

    // Observer, so the nested pass has something it is allowed to call.
    signal.connect_with_priority(|_v: Arc<u32>| {}, Priority::Low);

    let signal_for_reentry = signal.clone();
    let depth_inner = Arc::clone(&depth);
    signal.connect_with_priority(
        move |_v: Arc<u32>| {
            // Recurse exactly one level; the guard stops it going deeper.
            if depth_inner.fetch_add(1, Ordering::Relaxed) == 0 {
                signal_for_reentry.emit(0);
            }
            depth_inner.fetch_sub(1, Ordering::Relaxed);
        },
        Priority::High,
    );

    c.bench_function("signal_emit_nested_1_level", |b| b.iter(|| signal.emit(42)));
}

/// A slot that disconnects itself — the case the take/restore dance exists for.
///
/// Compared against `signal_emit_1_slot`, the delta is the price of supporting
/// self-disconnect. That is the number to weigh against any proposal to drop the
/// per-slot re-check, because dropping it would silently change this behaviour.
fn bench_signal_emit_with_self_disconnect(c: &mut Criterion) {
    c.bench_function("signal_emit_with_self_disconnect", |b| {
        b.iter(|| {
            // Rebuild each iteration: the slot removes itself, so it cannot be reused.
            let signal = Signal::<u32>::new();
            let shared = Arc::new(AtomicU64::new(0));
            let signal_for_slot = signal.clone();
            let shared_inner = Arc::clone(&shared);
            let handle = signal.connect(move |_v: Arc<u32>| {
                let own = ConnectionHandle(shared_inner.load(Ordering::Relaxed));
                signal_for_slot.disconnect(own);
            });
            shared.store(handle.0, Ordering::Relaxed);
            signal.emit(42);
        })
    });
}

fn bench_signal_emit(c: &mut Criterion) {
    let signal = Signal::<u32>::new();
    for _ in 0..1000 {
        signal.connect(|_v: Arc<u32>| {});
    }
    c.bench_function("signal_emit_1000_slots", |b| {
        b.iter(|| {
            signal.emit(42);
        })
    });
}

fn bench_signal_emit_10_slots(c: &mut Criterion) {
    let signal = Signal::<u32>::new();
    for _ in 0..10 {
        signal.connect(|_v: Arc<u32>| {});
    }
    c.bench_function("signal_emit_10_slots", |b| {
        b.iter(|| {
            signal.emit(42);
        })
    });
}

fn bench_signal_emit_100_slots(c: &mut Criterion) {
    let signal = Signal::<u32>::new();
    for _ in 0..100 {
        signal.connect(|_v: Arc<u32>| {});
    }
    c.bench_function("signal_emit_100_slots", |b| {
        b.iter(|| {
            signal.emit(42);
        })
    });
}

fn bench_signal_connect(c: &mut Criterion) {
    let signal = Signal::<u32>::new();
    c.bench_function("signal_connect", |b| {
        b.iter(|| {
            signal.connect(|_v: Arc<u32>| {});
        })
    });
}

fn bench_signal_disconnect(c: &mut Criterion) {
    let signal = Signal::<u32>::new();
    let handles: Vec<_> = (0..1000).map(|_| signal.connect(|_v: Arc<u32>| {})).collect();
    c.bench_function("signal_disconnect", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let idx = i % handles.len();
            let _ = signal.disconnect(handles[idx]);
            i += 1;
        })
    });
}

fn bench_signal_emit_large_payload(c: &mut Criterion) {
    let signal = Signal::<String>::new();
    let large = "x".repeat(4096);
    for _ in 0..100 {
        signal.connect(|_v: Arc<String>| {});
    }
    c.bench_function("signal_emit_large_payload_100_slots", |b| {
        b.iter(|| {
            signal.emit(large.clone());
        })
    });
}

criterion_group!(
    benches,
    bench_signal_emit_small,
    bench_signal_emit_nested,
    bench_signal_emit_with_self_disconnect,
    bench_signal_emit,
    bench_signal_emit_10_slots,
    bench_signal_emit_100_slots,
    bench_signal_connect,
    bench_signal_disconnect,
    bench_signal_emit_large_payload,
);
criterion_main!(benches);
