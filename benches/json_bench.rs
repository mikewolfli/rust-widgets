// criterion (benchmark harness) cannot compile for wasm32 and should be excluded
// from synthetic mini/embedded profiles that are intentionally outside the
// desktop UI surface.
#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "desktop",
    not(any(feature = "mini", feature = "embedded"))
))]
use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "desktop",
    not(any(feature = "mini", feature = "embedded"))
))]
use rust_widgets::json::load_layout_from_str;
#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "desktop",
    not(any(feature = "mini", feature = "embedded"))
))]
use std::hint::black_box;

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "desktop",
    not(any(feature = "mini", feature = "embedded"))
))]
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

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "desktop",
    not(any(feature = "mini", feature = "embedded"))
))]
fn bench_json_loader_parse_and_bind(c: &mut Criterion) {
    c.bench_function("json_loader_parse_bind_medium_tree", |b| {
        b.iter(|| {
            let layout = load_layout_from_str(JSON_LAYOUT)
                .expect("json bench layout must parse and instantiate successfully");
            black_box(layout.len());
        })
    });
}

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "desktop",
    not(any(feature = "mini", feature = "embedded"))
))]
criterion_group!(benches, bench_json_loader_parse_and_bind);

#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "desktop",
    not(any(feature = "mini", feature = "embedded"))
))]
criterion_main!(benches);

/// The fallback entry point whenever `criterion_main!` above is compiled out.
///
/// The crate used to be gated whole with a crate-level `#![cfg(not(target_arch =
/// "wasm32"))]`. That also removed this `main`, and since a bench *target* exists in
/// every configuration, `cargo check --all-targets --target wasm32-unknown-unknown`
/// failed with E0601 ("main function not found in crate `json_bench`"). Gating the
/// items instead of the crate is what keeps every configuration a valid crate.
#[cfg(not(all(
    not(target_arch = "wasm32"),
    feature = "desktop",
    not(any(feature = "mini", feature = "embedded"))
)))]
fn main() {
    // Benchmark is intentionally disabled for unsupported feature profiles.
}
