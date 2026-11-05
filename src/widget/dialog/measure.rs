// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A paint backend that exists only to answer measurement questions.
//!
//! The dialogs derive their button bands and row heights on the *press* path, where there is no
//! render context of its own, and a press arrives on every mouse move. Building a
//! `SoftwarePaintBackend` at the control's size for that did what a backend does when it is not a
//! frame: it allocated **two** full-size RGBA buffers and zeroed both, per hover pixel (DL2-01).
//! The derivation never draws a pixel — it only asks `measure_text` for a line height and a run
//! width — so a zero-sized target answers every question it needs and touches no buffer.
//!
//! One instance is kept per thread rather than per call, so a steady hover costs no allocation at
//! all. Same shape as the delegate `GpuPaintBackend` keeps for its own measurement-only work
//! (`render/present/smooth.rs`).
#[cfg(not(alloc_frugal))]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static MEASUREMENT_BACKEND: crate::compat::RefCell<crate::render::SoftwarePaintBackend> =
        crate::compat::RefCell::new(crate::render::SoftwarePaintBackend::new(
            crate::core::Size::new(0, 0),
            1.0,
        ));
}

/// Runs `f` with a render context backed by the thread's measurement-only backend.
///
/// The context is valid for the callback only: it borrows the shared backend, which is why this
/// takes a closure instead of returning the context. Nothing painted through it reaches a pixel —
/// it is for `measure_text` and the geometry helpers built on it.
#[cfg(not(alloc_frugal))]
pub(crate) fn with_measurement_context<R>(
    f: impl FnOnce(&crate::render::RenderContext<'_>) -> R,
) -> R {
    MEASUREMENT_BACKEND.with(|backend| {
        let mut backend = backend.borrow_mut();
        let context = crate::render::RenderContext::new(&mut *backend);
        f(&context)
    })
}

/// The allocation-frugal build carries no shared render backend: each caller derives its geometry
/// with a stack-local zero-sized context instead.
#[cfg(alloc_frugal)]
pub(crate) fn with_measurement_context<R>(
    f: impl FnOnce(&crate::render::RenderContext<'_>) -> R,
) -> R {
    let mut backend = crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
    let context = crate::render::RenderContext::new(&mut backend);
    f(&context)
}
