// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Memory management utilities: pool allocator, arena allocator, stack allocator,
//! and memory monitoring with pressure handling.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/gpu/buffer_pool.rs:236`; the `BufferPool`
//! in this module is the GPU staging-buffer **fallback** chain.
//!
//! The consumer state is per type, not uniform:
//!
//! * `BufferPool` — **in production use**. `GpuStagingBufferPool` holds an
//!   `Option<crate::memory::BufferPool>` as its `fallback_pool` field
//!   (`src/gpu/buffer_pool.rs`), populated by `GpuStagingBufferPool::with_fallback_pool`
//!   and `integration::create_fallback_pool`, and consulted on overflow-path
//!   allocations. This is the fallback chain that keeps an over-budget upload
//!   from failing outright when the ring buffer has no free slot.
//! * `ObjectPool` / `SharedPool` / `StringPool` / `VecPool` / `PoolManager` —
//!   **public API retained, no in-repo production consumer**. They are exercised
//!   by this module's own tests and documented in the cookbook, but nothing
//!   outside `memory` acquires from them.
//! * The pool and allocator wrappers do **not** back the widget registry, and
//!   `src/lib.rs:1` is not a production call. An earlier note here claimed both;
//!   neither was true, and neither is restated as if it were.
//!
//! The module is kept because `BufferPool` is load-bearing for the GPU path and
//! because the rest is a shipped, documented public surface.

mod pool;
pub use pool::*;

/// Arena, pool, and stack allocator implementations with a common
/// allocator interface.
pub mod allocators;
pub use allocators::*;
