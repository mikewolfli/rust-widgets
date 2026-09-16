// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Memory management utilities: pool allocator, arena allocator, stack allocator,
//! and memory monitoring with pressure handling.

mod pool;
pub use pool::*;

/// Arena, pool, and stack allocator implementations with a common
/// allocator interface.
pub mod allocators;
pub use allocators::*;
