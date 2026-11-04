// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Memory management utilities: pool allocator, arena allocator, stack allocator,
//! and memory monitoring with pressure handling.
//!
//! # Reachability
//!
//! **State:** Reserved public API, and nothing more.
//!
//! * `BufferPool` — a reusable byte-buffer pool. It has **no** production attach point: the GPU
//!   upload path this module was reserved for (`GpuStagingBufferPool`) was deleted in
//!   issue-20261104 RD-04, because `wgpu`'s own `Queue::write_buffer`/`write_texture` already stage
//!   and coalesce uploads — a second ring would have added a copy and a second upload path.
//!   `BufferPool` remains a public utility a host can use directly, and the modules below are
//!   exercised by their own tests.
//! * `ObjectPool` / `SharedPool` / `StringPool` / `VecPool` / `PoolManager` —
//!   **public API retained, no in-repo production consumer**. They are exercised
//!   by this module's own tests and documented in the cookbook, but nothing
//!   outside `memory` acquires from them.
//! * The pool and allocator wrappers do **not** back the widget registry, and
//!   `src/lib.rs:1` is not a production call. An earlier note here claimed both;
//!   neither was true, and neither is restated as if it were.
//!
//! # Fallback wiring status (G-215, restated once RD-04 closed)
//!
//! There is no GPU fallback chain left to wire: the staging pool that would have consulted a
//! `BufferPool` on overflow was removed together with its allocator. An upload over budget is a
//! `wgpu` decision, not this module's, so nothing here claims a wiring that does not exist.

mod pool;
pub use pool::*;

/// Arena, pool, and stack allocator implementations with a common
/// allocator interface.
pub mod allocators;
pub use allocators::*;
