// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Memory management utilities: pool allocator, arena allocator, stack allocator,
//! and memory monitoring with pressure handling.
//!
//! # Reachability
//!
//! **State:** Reserved public API, plus one GPU fallback chain that is wired for
//! tests only. The consumer state is per type, not uniform:
//!
//! * `BufferPool` — **intended** as the GPU staging-buffer fallback, but the chain
//!   has no production fill point today. `GpuStagingBufferPool` holds an
//!   `Option<crate::memory::BufferPool>` as its `fallback_pool` field
//!   (`src/gpu/buffer_pool.rs`) and consults it on overflow-path allocations, but
//!   the field can only be populated by `GpuStagingBufferPool::with_fallback_pool`
//!   and `integration::create_fallback_pool`, and both of those are called from
//!   tests only (verified by grep). No production constructor installs a pool, so
//!   the overflow path currently finds `None`.
//! * `ObjectPool` / `SharedPool` / `StringPool` / `VecPool` / `PoolManager` —
//!   **public API retained, no in-repo production consumer**. They are exercised
//!   by this module's own tests and documented in the cookbook, but nothing
//!   outside `memory` acquires from them.
//! * The pool and allocator wrappers do **not** back the widget registry, and
//!   `src/lib.rs:1` is not a production call. An earlier note here claimed both;
//!   neither was true, and neither is restated as if it were.
//!
//! # Fallback wiring status (G-215)
//!
//! The `BufferPool` fallback chain is kept as the intended integration point, not
//! described as a live dependency: **the chain is test-wired only, and its
//! production attach point waits on the host upload-ring implementation**
//! (G-211/G-212). That GPU path should consume this chain when it is wired rather
//! than introducing a second buffer source; until then, an over-budget upload has
//! no fallback buffer and fails the allocation.

mod pool;
pub use pool::*;

/// Arena, pool, and stack allocator implementations with a common
/// allocator interface.
pub mod allocators;
pub use allocators::*;
