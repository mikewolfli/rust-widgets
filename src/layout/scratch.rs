// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reusable buffers for the resize chain (issue-20261104 LY-07).
//!
//! # Why this exists
//!
//! A window resize walks the layout tree: the window layout, then every panel layout it places,
//! then every declarative container inside those. Each level collects its children's rectangles
//! into a fresh `Vec` before writing them back to the widgets, so one resize allocated once per
//! level — the same "per call, not per frame" cost [`crate::layout::grid`] already pools
//! (`GRID_FLOOR_SCRATCH` / `GRID_EMITTED_SCRATCH`).
//!
//! This is that pattern for the rectangle lists. The list is **taken** at the start of a level and
//! **returned** once its rectangles have been applied, so after the first resize the whole chain
//! reuses the same allocations.
//!
//! # Why a stack, and why it is capped
//!
//! Levels nest: applying a panel's layout re-enters the same function for a nested panel, and that
//! inner call needs its own buffer while the outer one is still in use. A stack gives each live
//! level its own buffer, and the cap keeps a pathological tree from turning "reuse" into "keep one
//! buffer per depth forever" — beyond the cap an extra level simply allocates, which is the cost
//! this module started with.
//!
//! # Why `try_with`
//!
//! A layout pass can run while this thread's TLS is being torn down (a widget dropped during
//! thread exit). `try_with` reports that instead of panicking, and a miss costs one allocation —
//! the honest fallback, not a crash.

use std::cell::RefCell;

use crate::compat::Vec;
use crate::core::{ObjectId, Rect};

/// One level's rectangle list, as the resize chain collects it.
pub(crate) type ChildGeometries = Vec<(ObjectId, Rect)>;

/// How many buffers the stack keeps.
///
/// Four covers the nesting a real interface has (window → panel → declarative container → nested
/// panel). Deeper than that and the extra levels allocate, which is the behaviour this module
/// replaced.
const SCRATCH_DEPTH: usize = 4;

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static GEOMETRY_SCRATCH: RefCell<Vec<ChildGeometries>> = const { RefCell::new(Vec::new()) };
}

/// Takes an empty rectangle list, keeping the capacity a previous pass grew.
pub(crate) fn take_geometry_scratch() -> ChildGeometries {
    let mut buffer = GEOMETRY_SCRATCH
        .try_with(|scratch| scratch.borrow_mut().pop())
        .ok()
        .flatten()
        .unwrap_or_default();
    buffer.clear();
    buffer
}

/// Returns a rectangle list to the stack, keeping its capacity.
///
/// Cleared before it is stored so a stale entry can never be read as a live layout result.
pub(crate) fn return_geometry_scratch(mut buffer: ChildGeometries) {
    let _ = GEOMETRY_SCRATCH.try_with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        if scratch.len() < SCRATCH_DEPTH {
            buffer.clear();
            scratch.push(buffer);
        }
    });
}
