// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! How the WGPU backend's two paths actually compare, on the same command stream.
//!
//! # Why this exists
//!
//! The backend can answer one command stream two ways: the CPU rasteriser (the fallback) or the GPU
//! pipelines (when the dispatcher claims the stream). Until this harness existed there was no
//! measurement behind any claim about which is faster, so "move it to the GPU" was an assertion
//! rather than a finding. Both paths return pixels through the same public entry points, which is
//! exactly the work a host pays for, readback included.
//!
//! # How the two paths are selected, without touching internals
//!
//! `WgpuDrawCommand::SetBlendMode` is sticky stream state. A stream whose mode is never `Normal`
//! is refused by the dispatcher while `blend_modes_on_gpu` is off — so **appending one
//! `SetBlendMode`** to a workload forces it through the CPU rasteriser, while the same stream with
//! the switch on runs on the GPU. Nothing is drawn after that trailing command, so the two pictures
//! are the same pixels: the benchmark compares cost, not work.
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
use rust_widgets::wgpu_backend::{PixelRect, Rgba8, WgpuDrawCommand, WgpuRenderer};
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

fn main() {
    let renderer = match WgpuRenderer::new() {
        Ok(renderer) => renderer,
        Err(error) => {
            println!("no GPU adapter on this host: {error}");
            return;
        }
    };
    let mut renderer = renderer;
    let sizes = [(320u32, 200u32), (1280, 720)];

    println!("wgpu backend: CPU rasteriser vs GPU pipelines (median of {SAMPLES}, µs/frame)");
    println!(
        "{:<13} {:>9} {:>9} {:>9} {:>9} {:>8}",
        "workload", "frame", "cpu", "gpu", "cpu/gpu", "winner"
    );
    for (width, height) in sizes {
        for workload in workloads(width, height) {
            renderer.set_blend_modes_on_gpu(false);
            let cpu = measure(&renderer, &workload, true);
            renderer.set_blend_modes_on_gpu(true);
            let gpu = measure(&renderer, &workload, false);
            let winner = if gpu < cpu { "gpu" } else { "cpu" };
            println!(
                "{:<13} {:>9} {:>9.1} {:>9.1} {:>9.2} {:>8}",
                workload.name,
                format!("{width}x{height}"),
                cpu.as_secs_f64() * 1e6,
                gpu.as_secs_f64() * 1e6,
                cpu.as_secs_f64() / gpu.as_secs_f64(),
                winner
            );
        }
    }

    // The texture entry point skips the readback entirely, which is the cost this harness exists to
    // expose: the pixel-returning calls copy the whole frame back and wait for it, and at 720p that
    // dominates every workload that does not already spend milliseconds per pixel.
    println!();
    println!("readback vs texture-only (GPU, µs/frame)");
    println!("{:<13} {:>9} {:>11} {:>11} {:>8}", "workload", "frame", "frame-copy", "texture", "speedup");
    renderer.set_blend_modes_on_gpu(true);
    for (width, height) in [(320u32, 200u32), (1280, 720)] {
        for workload in workloads(width, height) {
            let with_readback = measure(&renderer, &workload, false);
            for _ in 0..WARMUP {
                renderer
                    .with_draw_commands_texture(workload.width, workload.height, &workload.commands, |view| {
                        black_box(view);
                    })
                    .expect("the texture path renders");
            }
            let mut samples = Vec::with_capacity(SAMPLES);
            for _ in 0..SAMPLES {
                let started = Instant::now();
                renderer
                    .with_draw_commands_texture(workload.width, workload.height, &workload.commands, |view| {
                        black_box(view);
                    })
                    .expect("the texture path renders");
                samples.push(started.elapsed());
            }
            let texture = median(&mut samples);
            println!(
                "{:<13} {:>9} {:>11.1} {:>11.1} {:>8.2}",
                workload.name,
                format!("{width}x{height}"),
                with_readback.as_secs_f64() * 1e6,
                texture.as_secs_f64() * 1e6,
                with_readback.as_secs_f64() / texture.as_secs_f64()
            );
        }
    }

    // The borrowed entry point reads the GPU frame back into the renderer's own buffer, which is one
    // copy more than the owned path pays. Measure it rather than assume it is free.
    println!();
    println!("borrowed vs owned entry point (GPU, µs/frame)");
    println!("{:<13} {:>9} {:>9} {:>9} {:>8}", "workload", "frame", "owned", "borrowed", "delta");
    renderer.set_blend_modes_on_gpu(true);
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
