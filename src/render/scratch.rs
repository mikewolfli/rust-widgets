// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reusable byte buffers shared by the software and WGPU CPU raster paths.

use crate::compat::Vec;
use core::ops::{Deref, DerefMut};

#[cfg(not(alloc_frugal))]
thread_local! {
    static POOL: core::cell::RefCell<Vec<Vec<u8>>> = const { core::cell::RefCell::new(Vec::new()) };
}

pub(crate) struct ScratchBuffer(Vec<u8>);

impl ScratchBuffer {
    pub(crate) fn new(len: usize) -> Self {
        #[cfg(not(alloc_frugal))]
        let mut bytes = POOL.with(|pool| {
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
        let mut bytes = Vec::new();
        bytes.resize(len, 0);
        bytes.fill(0);
        Self(bytes)
    }
}

impl Deref for ScratchBuffer {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl DerefMut for ScratchBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl Drop for ScratchBuffer {
    fn drop(&mut self) {
        #[cfg(not(alloc_frugal))]
        let _ = POOL.try_with(|pool| {
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
        POOL.with(|pool| pool.borrow_mut().clear());
        let mut first = ScratchBuffer::new(4096);
        first.fill(255);
        let pointer = first.as_ptr();
        drop(first);
        let second = ScratchBuffer::new(1024);
        assert_eq!(pointer, second.as_ptr());
        assert!(second.iter().all(|&byte| byte == 0));
        let nested = ScratchBuffer::new(1024);
        assert_ne!(nested.as_ptr(), second.as_ptr());
    }
}
