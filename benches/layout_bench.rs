use criterion::{criterion_group, criterion_main, Criterion};
use rust_widgets::core::Rect;
use rust_widgets::layout::{FlowAlignment, FlowDirection, FlowLayout, FlowLayoutConfig};
use rust_widgets::widget::base_widgets::button::Button;
use std::hint::black_box;

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
        let button = Button::new(
            format!("Item {i}"),
            Rect::new(0, 0, 96 + (i % 4) as u32 * 8, 32),
        );
        layout.add_child(Box::new(button));
    }
    layout
}

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

criterion_group!(benches, bench_flow_layout_200_items);
criterion_main!(benches);
