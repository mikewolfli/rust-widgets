use criterion::{criterion_group, criterion_main, Criterion};
use rust_widgets::core::*;
use rust_widgets::render::*;

fn bench_fill_pixels(c: &mut Criterion) {
    let mut pixels = vec![0u8; 1920 * 1080 * 4];
    let color = Color::rgba(128, 64, 32, 255);
    c.bench_function("fill_pixels 1080p", |b| {
        b.iter(|| fill_pixels(&mut pixels, color))
    });
}

fn bench_blend_pixel(c: &mut Criterion) {
    let mut pixels = vec![0u8; 1920 * 1080 * 4];
    let color = Color::rgba(128, 64, 32, 128);
    c.bench_function("blend_pixel 1080p", |b| {
        b.iter(|| blend_pixel(&mut pixels, 1920, 100, 100, color, 0.5))
    });
}

criterion_group!(benches, bench_fill_pixels, bench_blend_pixel);
criterion_main!(benches);
