use criterion::{criterion_group, criterion_main, Criterion};
use rust_widgets::json::load_layout_from_str;
use std::hint::black_box;

const JSON_LAYOUT: &str = r#"
{
  "window": {
    "id": "bench_window",
    "title": "Bench",
    "width": 1280,
    "height": 720,
    "layout": {
      "type": "vbox",
      "spacing": 6,
      "children": [
        {"label": {"id": "title", "text": "Benchmark"}},
        {"lineedit": {"id": "name", "placeholder": "name"}},
        {"button": {"id": "save", "text": "Save"}},
        {"checkbox": {"id": "remember", "text": "Remember me"}},
        {"slider": {"id": "progress", "min": 0, "max": 100, "value": 40}},
        {"listview": {"id": "items"}},
        {"frame": {"id": "footer"}}
      ]
    }
  }
}
"#;

fn bench_json_loader_parse_and_bind(c: &mut Criterion) {
    c.bench_function("json_loader_parse_bind_medium_tree", |b| {
        b.iter(|| {
            let layout = load_layout_from_str(JSON_LAYOUT)
                .expect("json bench layout must parse and instantiate successfully");
            black_box(layout.len());
        })
    });
}

criterion_group!(benches, bench_json_loader_parse_and_bind);
criterion_main!(benches);
