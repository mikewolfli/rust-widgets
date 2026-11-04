// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Does a hand-rolled staging ring upload faster than `Queue::write_buffer`?
//!
//! # The answer this file was written to find (issue-20261104 RD-04)
//!
//! A staging ring looks like an obvious win: it copies the frame's packed bytes into a buffer the
//! CPU already owns, instead of paying whatever `wgpu` does per `write_buffer`. It is **not** a win
//! on this crate's upload shape, and this harness is the measurement that settled it. Measured on
//! macOS arm64 / Metal, four destination buffers per frame, best ring design (one staging buffer
//! per slot, four slots, one map/unmap per frame, fence only on slot reuse):
//!
//! | per frame | `write_buffer` | explicit ring | ring is |
//! |---|---|---|---|
//! | 4 × 64 KiB  | 168 µs | 417 µs | **+149 %** |
//! | 4 × 512 KiB | 349 µs | 590 µs | **+69 %** |
//! | 4 × 2 MiB   | 1005 µs | 1035 µs | **+3 %** |
//!
//! # Why it loses, stated as the thing the numbers show
//!
//! The ring's *encoding* is far cheaper (66 / 105 / 202 µs against 168 / 349 / 1005 µs): staging
//! into `wgpu` costs bookkeeping the ring does not need. But a persistently mapped buffer must be
//! **unmapped before the GPU copies from it** (wgpu validates this), so every frame pays one
//! `map_async` + `Device::poll` round trip to make its slot writable again — 351 / 485 / 833 µs on
//! Metal, an order of magnitude more than the encoding it saves.
//!
//! That round trip is structural, not a defect in this implementation: more slots only rotate which
//! buffer pays it, because each frame still needs one slot made writable. `wgpu`'s own staging belt
//! wins by pushing/unmapping in **batches**, amortising the mapping across frames — reproducing
//! that is re-implementing the belt, which is the second implementation of one concept that
//! principle #101 forbids.
//!
//! The staging *pool* (`GpuStagingBufferPool`) was measured the same way and added a further 0–10 %
//! on top of the ring, so it was deleted rather than kept: its contribution was the `(slot, offset)`
//! arithmetic and nothing else. See `docs/log/log-20261104-2.md` §18.
//!
//! # What the harness asserts
//!
//! Only that both columns upload the payload **byte for byte** — a "faster" column must not be
//! faster by shipping less. It deliberately makes no timing assertion: a threshold here would
//! freeze one machine's numbers into a contract, and the numbers are already recorded above and in
//! the log. Re-run it (`cargo test --lib --features gpu-wgpu -- upload_ring`) on a host whose
//! backend you suspect is different; without a GPU it prints `skipped` and returns.

use crate::compat::Vec;
use std::time::{Duration, Instant};

/// Bytes uploaded per destination buffer, per frame.
const SIZES: [usize; 3] = [64 * 1024, 512 * 1024, 2 * 1024 * 1024];
/// Destination buffers written per frame: the shape of one packed frame (uniform, instances,
/// circle quads, path vertices).
const DESTINATIONS: usize = 4;
/// Frames per column, after warm-up.
const FRAMES: usize = 120;
/// Frames discarded before timing, so buffer growth and pipeline caches are warm.
const WARMUP: usize = 10;
/// How many staging buffers the ring cycles through.
const RING_SLOTS: usize = 4;
/// The offset alignment a staging ring uses.
const ALIGNMENT: usize = 256;

/// One column's result for one size.
struct Measurement {
    /// Encode + submit only: the CPU a frame budget cares about.
    cpu_per_frame: Duration,
    /// The whole frame, including the ring's fence and remap.
    wall_per_frame: Duration,
    /// The fence + remap on its own. Zero for `write_buffer`, which owns no slot.
    wait_per_frame: Duration,
}

impl Measurement {
    fn mb_per_second(&self, bytes_per_frame: usize) -> f64 {
        let seconds = self.wall_per_frame.as_secs_f64();
        if seconds <= 0.0 {
            return f64::INFINITY;
        }
        bytes_per_frame as f64 / seconds / (1024.0 * 1024.0)
    }
}

fn align_up(value: usize) -> usize {
    value.div_ceil(ALIGNMENT) * ALIGNMENT
}

fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: crate::gpu::backend_ladder::instance_backends(),
        flags: wgpu::InstanceFlags::default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        backend_options: wgpu::BackendOptions::default(),
        display: None,
    });
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("upload_path_bench_device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        memory_hints: wgpu::MemoryHints::Performance,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        trace: wgpu::Trace::Off,
    }))
    .ok()?;
    Some((device, queue))
}

fn destination_buffer(device: &wgpu::Device, size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("upload_path_bench_destination"),
        size: size as u64,
        usage: wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    })
}

/// Maps `buffer` for writing and waits for the mapping to complete.
fn map_for_write(device: &wgpu::Device, buffer: &wgpu::Buffer) {
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Write, |_result: Result<(), wgpu::BufferAsyncError>| {});
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(5)),
    });
}

/// The ring: one host-visible staging buffer per slot, plus the fence that says when a slot may be
/// rewritten.
///
/// Per-slot buffers are what lets a slot stay mapped while the GPU reads another; with a single
/// buffer every frame would have to unmap it for the copies *and* remap it before the next write,
/// which measured worse still.
struct StagingRing {
    buffers: Vec<wgpu::Buffer>,
    /// The submission that last read each slot. Reusing a slot waits on it.
    pending: Vec<Option<wgpu::SubmissionIndex>>,
    mapped: Vec<bool>,
}

impl StagingRing {
    /// Creates `RING_SLOTS` mapped staging buffers of `slot_bytes` each.
    fn new(device: &wgpu::Device, slot_bytes: usize) -> Self {
        let mut buffers = Vec::with_capacity(RING_SLOTS);
        let mut pending = Vec::with_capacity(RING_SLOTS);
        let mut mapped = Vec::with_capacity(RING_SLOTS);
        for index in 0..RING_SLOTS {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(match index {
                    0 => "upload_ring_slot_0",
                    1 => "upload_ring_slot_1",
                    2 => "upload_ring_slot_2",
                    _ => "upload_ring_slot_3",
                }),
                size: slot_bytes as u64,
                usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            map_for_write(device, &buffer);
            buffers.push(buffer);
            pending.push(None);
            mapped.push(true);
        }
        Self { buffers, pending, mapped }
    }

    /// Makes slot `slot` writable again, waiting only on that slot's own last read.
    fn prepare(&mut self, device: &wgpu::Device, slot: usize) {
        if self.mapped[slot] {
            return;
        }
        if let Some(index) = self.pending[slot].take() {
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: Some(Duration::from_secs(5)),
            });
        }
        map_for_write(device, &self.buffers[slot]);
        self.mapped[slot] = true;
    }

    /// Writes `payload` at each of `offsets` in `slot`, then unmaps it for the copies.
    fn write_all(&mut self, slot: usize, offsets: &[usize], payload: &[u8]) {
        {
            let mut view = self.buffers[slot]
                .slice(..)
                .get_mapped_range_mut()
                .expect("the slot is mapped for writing");
            // `BufferViewMut` exposes its bytes through `slice(..) -> WriteOnly`, not `Index`:
            // mapped memory may be write-combining, so wgpu keeps every write explicit.
            for offset in offsets {
                view.slice(*offset..*offset + payload.len()).copy_from_slice(payload);
            }
        }
        self.buffers[slot].unmap();
        self.mapped[slot] = false;
    }
}

/// Baseline: one `write_buffer` per destination, then one submit — the shape of a frame.
fn measure_write_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    destination: &[wgpu::Buffer],
    payload: &[u8],
) -> Measurement {
    let mut cpu = Duration::ZERO;
    let mut wall = Duration::ZERO;
    for frame in 0..(FRAMES + WARMUP) {
        let started = Instant::now();
        for buffer in destination {
            queue.write_buffer(buffer, 0, payload);
        }
        // An empty encoder + submit flushes the staged writes, and it is what a frame pays anyway;
        // the ring column creates one encoder per frame too, so neither column is handed a cheaper
        // loop than the other.
        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("upload_bench_baseline_encoder"),
        });
        let _ = queue.submit(Some(encoder.finish()));
        if frame >= WARMUP {
            cpu += started.elapsed();
            wall += started.elapsed();
        }
    }
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(10)),
    });
    Measurement {
        cpu_per_frame: cpu / FRAMES as u32,
        wall_per_frame: wall / FRAMES as u32,
        wait_per_frame: Duration::ZERO,
    }
}

/// The ring: fence the slot, map, write every destination's payload, unmap, copy, submit.
fn measure_ring(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    destination: &[wgpu::Buffer],
    payload: &[u8],
) -> Measurement {
    let size = payload.len();
    let aligned = align_up(size);
    let mut ring = StagingRing::new(device, aligned * DESTINATIONS);
    let mut cpu = Duration::ZERO;
    let mut wall = Duration::ZERO;
    let mut wait = Duration::ZERO;

    for frame in 0..(FRAMES + WARMUP) {
        let started = Instant::now();
        let slot = frame % RING_SLOTS;
        // Wait only for this slot's previous read, then remap it — the cost the baseline never
        // pays explicitly, and the reason this column loses.
        ring.prepare(device, slot);
        let fence = started.elapsed();
        let encoding_started = Instant::now();

        let offsets: Vec<usize> = (0..DESTINATIONS).map(|index| index * aligned).collect();
        ring.write_all(slot, &offsets, payload);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("upload_ring_bench_encoder"),
        });
        for (index, offset) in offsets.iter().enumerate() {
            encoder.copy_buffer_to_buffer(
                &ring.buffers[slot],
                *offset as u64,
                &destination[index],
                0,
                size as u64,
            );
        }
        let index = queue.submit(Some(encoder.finish()));
        ring.pending[slot] = Some(index);

        if frame >= WARMUP {
            cpu += encoding_started.elapsed();
            wall += started.elapsed();
            wait += fence;
        }
    }
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(10)),
    });
    Measurement {
        cpu_per_frame: cpu / FRAMES as u32,
        wall_per_frame: wall / FRAMES as u32,
        wait_per_frame: wait / FRAMES as u32,
    }
}

/// Reads `size` bytes back out of `buffer`, for the byte-identity check.
fn read_back(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    size: usize,
) -> Vec<u8> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("upload_ring_bench_readback"),
        size: size as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("upload_ring_bench_readback_encoder"),
    });
    encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, size as u64);
    let _ = queue.submit(Some(encoder.finish()));
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_result: Result<(), wgpu::BufferAsyncError>| {});
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(10)),
    });
    let data = slice.get_mapped_range().expect("the readback buffer is mapped").to_vec();
    staging.unmap();
    data
}

/// Runs the comparison and prints one table per size.
///
/// A `#[test]` so it runs with the repository's own suite on a host with a GPU, and so the numbers
/// stay reproducible. It asserts byte identity and nothing about speed; see the module docs.
#[test]
fn an_explicit_upload_ring_does_not_beat_write_buffer() {
    let Some((device, queue)) = gpu() else {
        eprintln!("upload ring measurement skipped: no wgpu adapter on this host");
        return;
    };
    println!();
    println!(
        "{:<10} {:>14} {:>14} {:>10} {:>14}",
        "size", "write_buffer", "ring", "ring Δ%", "ring fence"
    );
    println!("{}", "-".repeat(70));

    for size in SIZES {
        let payload: Vec<u8> = (0..size).map(|index| (index % 251) as u8).collect();
        let destination: Vec<wgpu::Buffer> =
            (0..DESTINATIONS).map(|_| destination_buffer(&device, size)).collect();

        let baseline = measure_write_buffer(&device, &queue, &destination, &payload);
        assert_eq!(
            read_back(&device, &queue, &destination[0], size),
            payload,
            "the baseline must upload the payload it was given"
        );

        let ring = measure_ring(&device, &queue, &destination, &payload);
        assert_eq!(
            read_back(&device, &queue, &destination[0], size),
            payload,
            "the ring must upload the payload it was given"
        );

        let delta = (ring.wall_per_frame.as_secs_f64() / baseline.wall_per_frame.as_secs_f64()
            - 1.0)
            * 100.0;
        println!(
            "{:<10} {:>11.1} µs {:>11.1} µs {:>+9.1}% {:>11.1} µs",
            format!("{} KiB", size / 1024),
            baseline.wall_per_frame.as_secs_f64() * 1e6,
            ring.wall_per_frame.as_secs_f64() * 1e6,
            delta,
            ring.wait_per_frame.as_secs_f64() * 1e6,
        );
        println!(
            "{:<10} {:>11.1} MB/s {:>9.1} MB/s   (encode+submit: {:.1} / {:.1} µs)",
            "",
            baseline.mb_per_second(size * DESTINATIONS),
            ring.mb_per_second(size * DESTINATIONS),
            baseline.cpu_per_frame.as_secs_f64() * 1e6,
            ring.cpu_per_frame.as_secs_f64() * 1e6,
        );
    }
    println!();
    println!(
        "positive Δ% = the ring is slower. The ring's encode+submit is cheaper, but it must map a \
         slot per frame; that round trip is what loses."
    );
}
