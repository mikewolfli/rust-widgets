// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Memory management utilities: reusable byte-buffer and object pools.
//!
//! # Reachability
//!
//! **State:** Reserved public API, and nothing more.
//!
//! * `BufferPool` — a reusable byte-buffer pool. It has **no** production attach point: the GPU
//!   upload path this module was reserved for (`GpuStagingBufferPool`) was deleted in
//!   issue-20261104 RD-04, because `wgpu`'s own `Queue::write_buffer`/`write_texture` already stage
//!   and coalesce uploads — a second ring would have added a copy and a second upload path.
//!   `BufferPool` remains a public utility a host can use directly, and the pool modules below are
//!   exercised by their own tests.
//! * `ObjectPool` / `SharedPool` / `StringPool` / `VecPool` / `PoolManager` —
//!   **public API retained, no in-repo production consumer**. They are exercised
//!   by this module's own tests and documented in the cookbook, but nothing
//!   outside `memory` acquires from them.
//! * The pool wrappers do **not** back the widget registry, and `src/lib.rs:1` is not a
//!   production call. An earlier note here claimed both; neither was true, and neither is
//!   restated as if it were.
//!
//! # Fallback wiring status (G-215, restated once RD-04 closed)
//!
//! There is no GPU fallback chain left to wire: the staging pool that would have consulted a
//! `BufferPool` on overflow was removed together with its allocator. An upload over budget is a
//! `wgpu` decision, not this module's, so nothing here claims a wiring that does not exist.
//!
//! # Removed siblings
//!
//! RD2-03: `allocators.rs` (arena/stack allocators, `MemoryStats`, `MemoryMonitor`, and the
//! pressure-handler family) had no word-boundary consumer anywhere in `src/`, `tests/`,
//! `tools/`, `examples/`, `demo/`, `crates/` or `bindings/`; only its own tests named it. It was
//! the second dead module in this directory after `pool.rs`'s RD-04 ruling and is deleted rather
//! than kept as a glob re-export.

mod pool;
pub use pool::*;
