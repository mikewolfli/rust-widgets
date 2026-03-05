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

fn bench_signal_connect(c: &mut Criterion) {
    let signal = Signal::<u32>::new();
    c.bench_function("signal_connect", |b| {
        b.iter(|| {
            signal.connect(|_v: Arc<u32>| {});
        })
    });
}

criterion_group!(benches, bench_signal_emit, bench_signal_connect);
criterion_main!(benches);
