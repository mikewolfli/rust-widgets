// criterion (benchmark harness) cannot compile for wasm32.
#![cfg(not(target_arch = "wasm32"))]
//! Benchmarks for the signal system (emit, connect, disconnect).

//!
//! Covers:
//! - `signal_emit_N_slots` — emit cost with 10 / 100 / 1000 slots
//! - `signal_connect` — connect a single slot
//! - `signal_disconnect` — disconnect an existing slot
//! - `signal_emit_large_payload` — emit with a large `Arc<String>` payload

use criterion::{criterion_group, criterion_main, Criterion};
use rust_widgets::signal::Signal;
use std::sync::Arc;

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
    bench_signal_emit,
    bench_signal_emit_10_slots,
    bench_signal_emit_100_slots,
    bench_signal_connect,
    bench_signal_disconnect,
    bench_signal_emit_large_payload,
);
criterion_main!(benches);
