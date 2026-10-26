// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reusable word buffers shared by the software and WGPU CPU raster paths.
//!
//! The only raster scratch large enough to matter is the glyph winding accumulator: a 400x400
//! cell at 4x4 subsampling is 2.56 M `i32` cells. Allocating that block per uncached glyph made
//! every large-text miss pay a zeroing page-fault storm (G-004). The pool keeps the peak block
//! alive between misses and clears it on reuse, so a caller never observes stale winding counts.

use crate::compat::Vec;
use core::ops::{Deref, DerefMut};

#[cfg(not(alloc_frugal))]
thread_local! {
    static I32_POOL: core::cell::RefCell<Vec<Vec<i32>>> = const { core::cell::RefCell::new(Vec::new()) };
}

/// A reusable, zeroed `i32` scratch buffer.
///
/// `new(len)` returns a buffer of exactly `len` zeroed words, taking the smallest cached block
/// whose capacity already covers `len`. Dropping the buffer returns its allocation to the
/// thread-local pool (bounded to eight blocks), so a raster pass that visits many glyph sizes
/// does not hold every size resident.
pub(crate) struct ScratchI32(Vec<i32>);

impl ScratchI32 {
    pub(crate) fn new(len: usize) -> Self {
        #[cfg(not(alloc_frugal))]
        let mut words = I32_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            let index = pool
                .iter()
                .enumerate()
                .filter(|(_, buffer)| buffer.capacity() >= len)
                .min_by_key(|(_, buffer)| buffer.capacity())
                .map(|(index, _)| index);
            index.map(|index| pool.swap_remove(index)).unwrap_or_default()
        });
        #[cfg(alloc_frugal)]
        let mut words = Vec::new();
        words.resize(len, 0);
        words.fill(0);
        Self(words)
    }
}

impl Deref for ScratchI32 {
    type Target = [i32];
    fn deref(&self) -> &[i32] {
        &self.0
    }
}

impl DerefMut for ScratchI32 {
    fn deref_mut(&mut self) -> &mut [i32] {
        &mut self.0
    }
}

impl Drop for ScratchI32 {
    fn drop(&mut self) {
        #[cfg(not(alloc_frugal))]
        let _ = I32_POOL.try_with(|pool| {
            let mut pool = pool.borrow_mut();
            if pool.len() < 8 {
                pool.push(core::mem::take(&mut self.0));
            }
        });
    }
}

#[cfg(all(test, not(alloc_frugal)))]
mod tests {
    use super::*;

    #[test]
    fn reuses_capacity_and_clears_previous_contents() {
        I32_POOL.with(|pool| pool.borrow_mut().clear());
        let mut first = ScratchI32::new(4096);
        first.fill(7);
        let pointer = first.as_ptr();
        drop(first);
        let second = ScratchI32::new(1024);
        assert_eq!(pointer, second.as_ptr());
        assert!(second.iter().all(|&word| word == 0));
        let nested = ScratchI32::new(1024);
        assert_ne!(nested.as_ptr(), second.as_ptr());
    }
}
