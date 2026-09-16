#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::core::*;
#[cfg(not(target_arch = "wasm32"))]
use rust_widgets::render::*;

#[cfg(not(target_arch = "wasm32"))]
fn bench_fill_pixels(c: &mut Criterion) {
    let mut pixels = vec![0u8; 1920 * 1080 * 4];
    let color = Color::rgba(128, 64, 32, 255);
    c.bench_function("fill_pixels 1080p", |b| b.iter(|| fill_pixels(&mut pixels, color)));
}

#[cfg(not(target_arch = "wasm32"))]
fn bench_blend_pixel(c: &mut Criterion) {
    let mut pixels = vec![0u8; 1920 * 1080 * 4];
    let color = Color::rgba(128, 64, 32, 128);
    c.bench_function("blend_pixel 1080p", |b| {
        b.iter(|| blend_pixel(&mut pixels, 1920, 100, 100, color, 0.5))
    });
}

/// Measures a full self-drawn frame.
///
/// This is one of the two paths `docs/plans/TODO.md` names as performance-critical
/// and that previously had no baseline. `render_frame` is what every visible control
/// goes through on each repaint, and the R-3 frame-buffer reuse (BLUE15 §10.4) exists
/// to keep its per-frame allocation flat — measuring it is what makes a regression in
/// that property observable rather than a matter of opinion.
///
/// 400x300 rather than 1080p: the cost under test is per-widget setup plus the
/// frame-buffer exercise, not raw pixel throughput (`fill_pixels` covers that), and
/// the smaller surface keeps the benchmark usable in a routine run.
///
/// Registration happens inside the loop because `render_frame` only accepts a
/// registered id; that setup is a small, fixed part of each iteration and is the same
/// order of work a real mount does.
///
/// Gated on the same condition as `widget::runtime` itself: the alloc-frugal profile
/// compiles the runtime out, so a build that enables every feature *and* `mini` (the
/// `--all-features` shape) has no path here to measure.
#[cfg(not(feature = "mini"))]
#[cfg(not(target_arch = "wasm32"))]
fn bench_render_frame(c: &mut Criterion) {
    use rust_widgets::widget::{runtime, Button, Widget};

    let geometry = Rect::new(0, 0, 400, 300);
    let clear = Color::rgba(0, 0, 0, 0);

    c.bench_function("render_frame 400x300", |b| {
        b.iter(|| {
            let widget: Box<dyn Widget> = Box::new(Button::new("ok".into(), geometry));
            let Some(id) = runtime::register(widget) else {
                panic!("the runtime must accept a widget");
            };
            let frame =
                runtime::render_frame(id, Size::new(geometry.width, geometry.height), clear);
            debug_assert!(frame.is_some(), "a Button must paint a frame");
            runtime::unregister(id);
        })
    });
}

/// Measures pointer hit-testing through a small widget tree.
///
/// The other path named in the TODO. The tree is built outside the timed loop so the
/// measurement is the traversal, not the construction. Gated like `bench_render_frame`.
#[cfg(not(feature = "mini"))]
#[cfg(not(target_arch = "wasm32"))]
fn bench_dispatch_pointer_event(c: &mut Criterion) {
    use rust_widgets::core::Point;
    use rust_widgets::event::Event;
    use rust_widgets::widget::base_widgets::label::Label;
    use rust_widgets::widget::container_widgets::groupbox::GroupBox;
    use rust_widgets::widget::runtime;
    // `add_child` lives on the `Widget` trait, so the trait must be in scope for the
    // method to resolve.
    use rust_widgets::widget::Widget;

    // Built the way `runtime`'s own tests do: children are registered, added to the
    // container by id, and the container is registered last.
    let mut parent = GroupBox::new(Rect::new(0, 0, 400, 300));
    let mut child_ids = Vec::new();
    for index in 0..8 {
        // Overlapping children stepping by 10px, so the probe point below falls
        // inside several of them and the walk cannot early-out at the root.
        let offsets: i32 = index * 10;
        let child = Label::new(format!("c{index}"), Rect::new(offsets, offsets, 60, 40));
        let Some(id) = runtime::register(Box::new(child)) else {
            panic!("the runtime must accept a child widget");
        };
        parent.add_child(id);
        child_ids.push(id);
    }
    let Some(root_id) = runtime::register(Box::new(parent)) else {
        panic!("the runtime must accept the root widget");
    };

    // Inside the last child and several earlier ones, so the traversal is exercised.
    let probe_point = Point::new(75, 45);
    let probe = Event::MouseMove { pos: probe_point };

    c.bench_function("dispatch_pointer_event 9-widget tree", |b| {
        b.iter(|| {
            let delivered = runtime::dispatch_pointer_event(root_id, &probe, probe_point);
            debug_assert!(delivered, "the probe point must reach a widget");
        })
    });

    for id in child_ids {
        runtime::unregister(id);
    }
    runtime::unregister(root_id);
}

// The two runtime benchmarks are compiled only where `widget::runtime` exists: the
// alloc-frugal profile (`mini`) compiles the widget runtime out, and a benchmark cannot
// measure a path that is not there. Grouping them separately keeps the pixel-level
// benchmarks running in every profile, including `mini`.
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
criterion_group!(pixel_benches, bench_fill_pixels, bench_blend_pixel);

#[cfg(not(feature = "mini"))]
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
criterion_group!(runtime_benches, bench_render_frame, bench_dispatch_pointer_event);

#[cfg(not(feature = "mini"))]
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
criterion_main!(pixel_benches, runtime_benches);
#[cfg(feature = "mini")]
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
criterion_main!(pixel_benches);

/// The fallback entry point for configurations where `criterion_main!` above is
/// compiled out.
///
/// `criterion` is a host-only dev-dependency (it cannot build for wasm32) — but the
/// bench *target* exists in every configuration, and a bench target must have a
/// `main`. Gating the whole file with a crate-level `#![cfg]` removed the
/// `criterion_main!`-generated `main` along with everything else, which made
/// `cargo check --all-targets --target wasm32-unknown-unknown` fail with E0601
/// ("main function not found in crate `render_bench`"). This module body is empty on
/// purpose: a benchmark that cannot run has nothing to do.
#[cfg(target_arch = "wasm32")]
fn main() {}
