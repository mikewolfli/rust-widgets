// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! How the WGPU backend's three delivery paths compare, on the same command stream.
//!
//! # Why this exists
//!
//! The backend can answer one command stream three ways: the CPU rasteriser (the fallback), the GPU
//! pipelines plus a full readback (when the dispatcher claims the stream), and — for a host that
//! consumes the picture on the GPU — a GPU pass handed over as a texture with no readback at all.
//! Until this harness existed there was no measurement behind any claim about which is faster, so
//! "move it to the GPU" was an assertion rather than a finding.
//!
//! # How the two paths are selected, without touching internals
//!
//! `WgpuDrawCommand::SetBlendMode` is sticky stream state. A stream whose mode is never `Normal`
//! is refused by the dispatcher while `blend_modes_on_gpu` is off — so **appending one
//! `SetBlendMode`** to a workload forces it through the CPU rasteriser, while the same stream with
//! the switch on runs on the GPU. Nothing is drawn after that trailing command, so the two pictures
//! are the same pixels: the benchmark compares cost, not work.
//!
//! # What each column measures
//!
//! - `cpu` — the CPU rasteriser, returned straight to the caller: no GPU work at all.
//! - `gpu (raw)` — one GPU pass, then `copy_texture_to_buffer` plus a blocking `map_async` wait,
//!   with the pixel-path cost gate disarmed so this is the baseline §1.1's facts came from.
//! - `selected` — the path a host gets with no settings: the default quality intent plus the armed
//!   cost gate. This is the column the winner is decided on.
//! - `texture` — the same pass, submitted and handed to the caller as a texture with **no
//!   readback**. `Queue::submit` is asynchronous, so this column is the CPU-side cost of the
//!   hand-off (encode + submit).
//! - `texture+wait` — `texture` followed by [`WgpuRenderer::wait_idle`], which quantifies how much
//!   GPU completion time the asynchronous column leaves out.
//! - `cpu-fallback` — the same texture entry point with a stream the dispatcher refuses, so it takes
//!   the CPU raster plus upload branch. This is the real cost of a texture host handing over a
//!   stream the pipelines cannot cover.
//!
//! # Quality tiers
//!
//! The renderer's `RenderQuality` states what the host needs. `Antialiased` (the default) accepts
//! the edge-feathered pipelines' rims and keeps the pre-existing picture. `ByteExact` moves two
//! classes to the reference rasteriser — streams containing a circle fill, ring, arc or rounded
//! outline, and streams drawn onto a transparent target with no whole-frame opaque clear — so the
//! bytes match exactly. The tier is set with `renderer.set_render_quality(..)`; this harness runs
//! the default tier because it measures cost, not equality (the equality cases are the library
//! tests).
//!
//! # Running it
//!
//! ```text
//! cargo bench --features desktop,gpu-wgpu --bench wgpu_bench
//! ```
//!
//! Output is a plain table (µs per frame, median of the samples) instead of criterion statistics:
//! the decision this harness exists to support is a coarse one — which path wins, at which frame
//! size and workload shape — and a table is what a reviewer can read.
use rust_widgets::render::BlendMode;
use rust_widgets::wgpu_backend::{PixelRect, RenderQuality, Rgba8, WgpuDrawCommand, WgpuRenderer};
use std::hint::black_box;
use std::time::{Duration, Instant};

/// One named stream and the frame it is drawn into.
struct Workload {
    name: &'static str,
    width: u32,
    height: u32,
    commands: Vec<WgpuDrawCommand>,
}

const WARMUP: usize = 2;
const SAMPLES: usize = 9;

fn opaque(r: u8, g: u8, b: u8) -> Rgba8 {
    Rgba8 { r, g, b, a: 255 }
}

fn rect(x: i32, y: i32, width: u32, height: u32) -> PixelRect {
    PixelRect { x, y, width, height }
}

/// A spread of workloads chosen to separate the shapes the two paths treat differently:
/// setup-dominated work (one clear), throughput work (many instances, large fills), upload work
/// (text, images) and the new blend runs.
fn workloads(width: u32, height: u32) -> Vec<Workload> {
    let mut all = Vec::new();
    let frame = rect(0, 0, width, height);

    all.push(Workload {
        name: "clear",
        width,
        height,
        commands: vec![WgpuDrawCommand::Clear { color: opaque(20, 30, 40) }],
    });

    for count in [200usize, 2000] {
        let mut commands = vec![WgpuDrawCommand::Clear { color: opaque(10, 10, 10) }];
        for index in 0..count {
            let x = (index as i32 * 37) % width.max(1) as i32;
            let y = (index as i32 * 53) % height.max(1) as i32;
            commands.push(WgpuDrawCommand::FillRect {
                rect: rect(x, y, 8, 8),
                color: opaque((index % 256) as u8, 120, 200),
                clip: None,
            });
        }
        all.push(Workload {
            name: if count == 200 { "rects-200" } else { "rects-2000" },
            width,
            height,
            commands,
        });
    }

    all.push(Workload {
        name: "fills-x4",
        width,
        height,
        commands: vec![
            WgpuDrawCommand::Clear { color: opaque(0, 0, 0) },
            WgpuDrawCommand::FillRect { rect: frame, color: opaque(200, 40, 40), clip: None },
            WgpuDrawCommand::FillRect { rect: frame, color: opaque(40, 200, 40), clip: None },
            WgpuDrawCommand::FillRect { rect: frame, color: opaque(40, 40, 200), clip: None },
            WgpuDrawCommand::FillRect { rect: frame, color: opaque(120, 120, 40), clip: None },
        ],
    });

    let mut ramp = Vec::new();
    for stop in 0..32u8 {
        ramp.extend_from_slice(&[255 - stop * 7, stop * 7, 128, 255]);
    }
    all.push(Workload {
        name: "gradient-full",
        width,
        height,
        commands: vec![
            WgpuDrawCommand::Clear { color: opaque(0, 0, 0) },
            WgpuDrawCommand::FillLinearGradient {
                rect: frame,
                gradient_data: ramp.clone().into(),
                clip: None,
            },
        ],
    });

    let mut text = vec![WgpuDrawCommand::Clear { color: opaque(250, 250, 250) }];
    for index in 0..40 {
        text.push(WgpuDrawCommand::DrawText {
            rect: rect(4, 4 + index * 18, 120, 16),
            text: "The quick brown fox".into(),
            color: opaque(20, 20, 20),
            clip: None,
        });
    }
    all.push(Workload { name: "text-40", width, height, commands: text });

    let pixels: Vec<u8> =
        (0..64 * 64).flat_map(|index| [(index % 251) as u8, 90, 160, 255]).collect();
    let mut images = vec![WgpuDrawCommand::Clear { color: opaque(30, 30, 30) }];
    for index in 0..8 {
        images.push(WgpuDrawCommand::DrawImage {
            rect: rect(8 + index * 70, 40, 64, 64),
            rgba8: pixels.clone().into(),
            image_width: 64,
            image_height: 64,
            clip: None,
        });
    }
    all.push(Workload { name: "images-8", width, height, commands: images });

    let mut shadows = vec![WgpuDrawCommand::Clear { color: opaque(245, 245, 245) }];
    for index in 0..8 {
        shadows.push(WgpuDrawCommand::BoxShadow {
            rect: rect(10 + index * 60, 30, 48, 40),
            color: Rgba8 { r: 0, g: 0, b: 0, a: 90 },
            offset_x: 2,
            offset_y: 3,
            blur_radius: 6,
            clip: None,
        });
    }
    all.push(Workload { name: "shadows-blur-8", width, height, commands: shadows });

    // The blend runs: the switchable path this harness was extended for.
    all.push(Workload {
        name: "blend-full",
        width,
        height,
        commands: vec![
            WgpuDrawCommand::Clear { color: opaque(200, 160, 120) },
            WgpuDrawCommand::SetBlendMode { mode: BlendMode::Multiply as u8 },
            WgpuDrawCommand::FillRect { rect: frame, color: opaque(90, 180, 255), clip: None },
        ],
    });
    all.push(Workload {
        name: "blend-small",
        width,
        height,
        commands: vec![
            WgpuDrawCommand::Clear { color: opaque(200, 160, 120) },
            WgpuDrawCommand::SetBlendMode { mode: BlendMode::Multiply as u8 },
            WgpuDrawCommand::FillRect {
                rect: rect(16, 16, 48, 24),
                color: opaque(90, 180, 255),
                clip: None,
            },
        ],
    });

    let mut mixed = vec![WgpuDrawCommand::Clear { color: opaque(235, 235, 238) }];
    for index in 0..50 {
        mixed.push(WgpuDrawCommand::FillRect {
            rect: rect((index * 13) % (width as i32), (index * 29) % (height as i32), 12, 12),
            color: opaque(60, 90, 120),
            clip: None,
        });
    }
    mixed.push(WgpuDrawCommand::FillLinearGradient {
        rect: rect(0, height as i32 / 2, width, height / 2),
        gradient_data: ramp.into(),
        clip: None,
    });
    for index in 0..10 {
        mixed.push(WgpuDrawCommand::DrawText {
            rect: rect(8, 8 + index * 18, 160, 16),
            text: "mixed frame".into(),
            color: opaque(30, 30, 30),
            clip: None,
        });
    }
    for index in 0..4 {
        mixed.push(WgpuDrawCommand::DrawImage {
            rect: rect(8 + index * 70, height as i32 - 80, 64, 64),
            rgba8: pixels.clone().into(),
            image_width: 64,
            image_height: 64,
            clip: None,
        });
    }
    for index in 0..4 {
        mixed.push(WgpuDrawCommand::BoxShadow {
            rect: rect(20 + index * 50, 60, 40, 30),
            color: Rgba8 { r: 0, g: 0, b: 0, a: 80 },
            offset_x: 1,
            offset_y: 2,
            blur_radius: 4,
            clip: None,
        });
    }
    all.push(Workload { name: "mixed", width, height, commands: mixed });

    all
}

fn median(samples: &mut [Duration]) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

/// One stream measured through all three delivery paths.
///
/// Every table below is printed from these rows, so the three paths are compared on one
/// measurement each instead of re-running the same stream for every view.
struct Comparison {
    name: &'static str,
    width: u32,
    height: u32,
    cpu: Duration,
    gpu: Duration,
    selected: Duration,
    /// The path the automatic policy picks for this stream, independent of timing noise.
    accelerated: bool,
    texture: Duration,
    texture_wait: Duration,
    texture_fallback: Duration,
}

/// Runs `workload` `SAMPLES` times and reports the median.
///
/// `force_cpu` appends a `SetBlendMode` the dispatcher refuses, which is the only way to select the
/// CPU rasteriser through the public API; nothing is drawn after it, so the picture is unchanged.
fn measure(renderer: &WgpuRenderer, workload: &Workload, force_cpu: bool) -> Duration {
    let mut commands = workload.commands.clone();
    if force_cpu {
        commands.push(WgpuDrawCommand::SetBlendMode { mode: BlendMode::Multiply as u8 });
    }
    for _ in 0..WARMUP {
        let pixels = renderer
            .render_draw_commands_rgba8(workload.width, workload.height, &commands)
            .expect("the frame renders");
        black_box(&pixels);
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let pixels = renderer
            .render_draw_commands_rgba8(workload.width, workload.height, &commands)
            .expect("the frame renders");
        samples.push(started.elapsed());
        black_box(&pixels);
    }
    median(&mut samples)
}

/// Waits for every submission already queued, so a texture sample starts from an idle GPU.
///
/// The public API exposes no "wait for device" call, but a pixel-returning render submits a wait
/// for its own completion, and queue submissions complete in order — so that blocking readback also
/// drains everything submitted before it. The drain is deliberately outside the timed region: it
/// exists only to keep one sample from timing the previous ones' backlog (without it the same
/// one-command stream measures slower at 320x200 than at 1280x720, which is a queue artefact, not a
/// property of the frame).
fn drain(renderer: &WgpuRenderer, width: u32, height: u32) {
    let clear = [WgpuDrawCommand::Clear { color: opaque(0, 0, 0) }];
    renderer.render_draw_commands_rgba8(width, height, &clear).expect("the drain frame renders");
}

/// Runs `workload` through the texture entry point `SAMPLES` times and reports the median.
///
/// Nothing is read back, so the sample is the hand-off cost (encode + submit) against an idle
/// queue. The submission is asynchronous; see the module documentation for why that boundary is
/// stated rather than papered over.
fn measure_texture(renderer: &WgpuRenderer, workload: &Workload) -> Duration {
    let draw = || {
        renderer
            .with_draw_commands_texture(
                workload.width,
                workload.height,
                &workload.commands,
                |view| {
                    black_box(view);
                },
            )
            .expect("the texture path renders")
    };
    for _ in 0..WARMUP {
        draw();
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        drain(renderer, workload.width, workload.height);
        let started = Instant::now();
        draw();
        samples.push(started.elapsed());
    }
    median(&mut samples)
}

/// `measure_texture` followed by [`WgpuRenderer::wait_idle`], so the sample includes the GPU
/// completion the asynchronous column leaves out.
///
/// The drain is still outside the timed region: it exists to keep one sample from timing the
/// previous ones' backlog, which `wait_idle` would otherwise absorb into this frame's number.
fn measure_texture_wait(renderer: &WgpuRenderer, workload: &Workload) -> Duration {
    let draw = || {
        renderer
            .with_draw_commands_texture(
                workload.width,
                workload.height,
                &workload.commands,
                |view| {
                    black_box(view);
                },
            )
            .expect("the texture path renders");
        renderer.wait_idle().expect("the device reaches idle");
    };
    for _ in 0..WARMUP {
        draw();
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        drain(renderer, workload.width, workload.height);
        let started = Instant::now();
        draw();
        samples.push(started.elapsed());
    }
    median(&mut samples)
}

/// Runs `workload` through the texture entry point with a stream the dispatcher refuses, so the
/// measurement is the CPU raster plus the upload a texture host pays for an uncovered stream.
///
/// The trailing `SetBlendMode` is the same forced-refusal trick [`measure`] uses; nothing is drawn
/// after it, so the picture is unchanged.
fn measure_texture_fallback(renderer: &WgpuRenderer, workload: &Workload) -> Duration {
    let mut commands = workload.commands.clone();
    commands.push(WgpuDrawCommand::SetBlendMode { mode: BlendMode::Multiply as u8 });
    let draw = || {
        renderer
            .with_draw_commands_texture(workload.width, workload.height, &commands, |view| {
                black_box(view);
            })
            .expect("the CPU-fallback texture path renders")
    };
    for _ in 0..WARMUP {
        draw();
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        drain(renderer, workload.width, workload.height);
        let started = Instant::now();
        draw();
        samples.push(started.elapsed());
    }
    median(&mut samples)
}

fn main() {
    let mut renderer = match WgpuRenderer::new() {
        Ok(renderer) => renderer,
        Err(error) => {
            println!("no GPU adapter on this host: {error}");
            return;
        }
    };
    let sizes = [(320u32, 200u32), (1280, 720)];

    println!("wgpu backend: CPU rasteriser vs GPU pipelines (median of {SAMPLES}, µs/frame)");
    println!("quality: RenderQuality::Antialiased by default; ByteExact keeps edge-feathered and");
    println!("         clear-less transparent streams on the reference rasteriser.");
    println!(
        "cost gate: pixel paths only; set_cost_gate_enabled(false) restores the raw GPU column."
    );
    println!("the numbers are per machine; the adapter is not exposed by the public API");

    let mut comparisons = Vec::new();
    for (width, height) in sizes {
        for workload in workloads(width, height) {
            // CPU: the trailing non-Normal mode makes the dispatcher refuse the stream.
            renderer.set_blend_modes_on_gpu(false);
            renderer.set_cost_gate_enabled(false);
            renderer.set_render_quality(RenderQuality::Antialiased);
            let cpu = measure(&renderer, &workload, true);

            // Raw GPU: the gate disarmed, so this is the baseline §1.1 was measured against.
            renderer.set_blend_modes_on_gpu(true);
            let gpu = measure(&renderer, &workload, false);

            // The path a host gets with no settings: the default intent plus the armed gate.
            renderer.set_cost_gate_enabled(true);
            let selected = measure(&renderer, &workload, false);
            let accelerated = renderer.selects_accelerated_pixels(
                workload.width,
                workload.height,
                &workload.commands,
            );

            // Texture delivery: claimed on the GPU, then the same stream refused so the CPU
            // raster + upload branch is measured too.
            let texture = measure_texture(&renderer, &workload);
            let texture_wait = measure_texture_wait(&renderer, &workload);
            renderer.set_blend_modes_on_gpu(false);
            let texture_fallback = measure_texture_fallback(&renderer, &workload);
            assert!(
                texture_fallback > Duration::ZERO,
                "the CPU-fallback texture branch must be measured, not zero"
            );

            comparisons.push(Comparison {
                name: workload.name,
                width,
                height,
                cpu,
                gpu,
                selected,
                accelerated,
                texture,
                texture_wait,
                texture_fallback,
            });
        }
    }

    println!();
    println!("pixel-returning paths (readback included in `gpu (raw)` and `selected`)");
    println!(
        "{:<13} {:>9} {:>9} {:>9} {:>9} {:>8}",
        "workload", "frame", "cpu", "gpu (raw)", "selected", "winner"
    );
    for row in &comparisons {
        // The decision, not the timings: when the gate keeps a stream on the reference rasteriser,
        // both timing columns measure that rasteriser and differ only by noise, so a timing-based
        // "winner" would report phantom GPU wins. The timings are printed next to it instead.
        println!(
            "{:<13} {:>9} {:>9.1} {:>9.1} {:>9.1} {:>8}",
            row.name,
            format!("{}x{}", row.width, row.height),
            row.cpu.as_secs_f64() * 1e6,
            row.gpu.as_secs_f64() * 1e6,
            row.selected.as_secs_f64() * 1e6,
            if row.accelerated { "gpu" } else { "cpu" }
        );
    }

    // The texture entry point skips the readback entirely, which is the cost this harness exists to
    // expose. `texture+wait` adds `wait_idle`, so the difference between the two texture columns is
    // the GPU completion the asynchronous hand-off leaves out. `cpu-fallback` is the same entry
    // point on a stream the dispatcher refuses, i.e. what a texture host pays for uncovered work.
    println!();
    println!("texture delivery (no readback; `+wait` adds wait_idle)");
    println!(
        "{:<13} {:>9} {:>11} {:>12} {:>11} {:>12}",
        "workload", "frame", "texture", "texture+wait", "readback", "cpu-fallback"
    );
    for row in &comparisons {
        println!(
            "{:<13} {:>9} {:>11.1} {:>12.1} {:>11.1} {:>12.1}",
            row.name,
            format!("{}x{}", row.width, row.height),
            row.texture.as_secs_f64() * 1e6,
            row.texture_wait.as_secs_f64() * 1e6,
            (row.gpu.saturating_sub(row.texture)).as_secs_f64() * 1e6,
            row.texture_fallback.as_secs_f64() * 1e6
        );
    }

    // The aggregate is the part a host can act on: one number per frame size for each delivery
    // shape, and how often the automatic selection beats the CPU rasteriser at all.
    println!();
    println!("summary (sum of the medians above, µs/frame)");
    println!(
        "{:<13} {:>10} {:>10} {:>10} {:>9} {:>9} {:>9}",
        "frame", "cpu", "selected", "texture", "selected wins", "cpu/tex", "sel/tex"
    );
    for (width, height) in sizes {
        let rows: Vec<&Comparison> =
            comparisons.iter().filter(|row| row.width == width && row.height == height).collect();
        let sum = |pick: fn(&Comparison) -> Duration| -> f64 {
            rows.iter().map(|row| pick(row).as_secs_f64()).sum::<f64>() * 1e6
        };
        let cpu = sum(|row| row.cpu);
        let selected = sum(|row| row.selected);
        let texture = sum(|row| row.texture);
        let selected_wins = rows.iter().filter(|row| row.accelerated).count();
        println!(
            "{:<13} {:>10.1} {:>10.1} {:>10.1} {:>6}/{:<2} {:>9.2} {:>9.2}",
            format!("{width}x{height}"),
            cpu,
            selected,
            texture,
            selected_wins,
            rows.len(),
            cpu / texture,
            selected / texture
        );
    }

    // The borrowed entry point reads the GPU frame back into the renderer's own buffer, which is one
    // copy more than the owned path pays. Measure it rather than assume it is free. The gate is
    // disarmed so both columns measure the GPU path, which is the ownership cost this table is for.
    println!();
    println!("borrowed vs owned entry point (GPU, µs/frame)");
    println!("{:<13} {:>9} {:>9} {:>9} {:>8}", "workload", "frame", "owned", "borrowed", "delta");
    renderer.set_blend_modes_on_gpu(true);
    renderer.set_cost_gate_enabled(false);
    for (width, height) in [(320u32, 200u32), (1280, 720)] {
        for workload in workloads(width, height).into_iter().filter(|w| {
            matches!(w.name, "clear" | "rects-2000" | "text-40" | "blend-full" | "mixed")
        }) {
            let owned = measure(&renderer, &workload, false);
            for _ in 0..WARMUP {
                let frame = renderer
                    .render_draw_commands_cached_rgba8(
                        workload.width,
                        workload.height,
                        &workload.commands,
                    )
                    .expect("the frame renders");
                black_box(&*frame);
            }
            let mut borrowed_samples = Vec::with_capacity(SAMPLES);
            for _ in 0..SAMPLES {
                let started = Instant::now();
                let frame = renderer
                    .render_draw_commands_cached_rgba8(
                        workload.width,
                        workload.height,
                        &workload.commands,
                    )
                    .expect("the frame renders");
                borrowed_samples.push(started.elapsed());
                black_box(&*frame);
            }
            let borrowed = median(&mut borrowed_samples);
            println!(
                "{:<13} {:>9} {:>9.1} {:>9.1} {:>8.2}",
                workload.name,
                format!("{width}x{height}"),
                owned.as_secs_f64() * 1e6,
                borrowed.as_secs_f64() * 1e6,
                owned.as_secs_f64() / borrowed.as_secs_f64()
            );
        }
    }
}
