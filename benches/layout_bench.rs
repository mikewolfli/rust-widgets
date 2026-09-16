#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::core::Rect;
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::layout::{FlowAlignment, FlowDirection, FlowLayout, FlowLayoutConfig};
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::widget::base_widgets::button::Button;
#[cfg(not(target_arch = "wasm32"))]
use std::hint::black_box;

#[cfg(not(target_arch = "wasm32"))]
fn make_flow_layout(item_count: usize) -> FlowLayout {
    let config = FlowLayoutConfig {
        direction: FlowDirection::Horizontal,
        alignment: FlowAlignment::Start,
        spacing: 6,
        padding: 8,
        wrap: true,
    };
    let mut layout = FlowLayout::with_config(config);
    for i in 0..item_count {
        let button = Button::new(format!("Item {i}"), Rect::new(0, 0, 96 + (i % 4) as u32 * 8, 32));
        layout.add_child(Box::new(button));
    }
    layout
}

#[cfg(not(target_arch = "wasm32"))]
fn bench_flow_layout_200_items(c: &mut Criterion) {
    let layout = make_flow_layout(200);
    let available = Rect::new(0, 0, 1920, 1080);
    c.bench_function("layout_flow_200_items_1080p", |b| {
        b.iter(|| {
            let result = layout.layout(available);
            black_box(result.len());
        })
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
criterion_group!(benches, bench_flow_layout_200_items);
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
/// ("main function not found in crate `layout_bench`").
#[cfg(target_arch = "wasm32")]
fn main() {}
