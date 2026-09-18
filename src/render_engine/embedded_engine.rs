// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Embedded render engine with independent lifecycle and resource registry.

use super::embedded::embedded_engine_shared;
use super::engine_trait::RenderEngine;
#[cfg(not(alloc_frugal))]
use super::native::NativeRenderEngine;
use crate::compat::Box;
use crate::core::RuntimeProfile;

/// Embedded engine with independent lifecycle and resource registry.
#[derive(Clone)]
pub struct EmbeddedRenderEngine;

impl EmbeddedRenderEngine {
    /// Create embedded engine.
    pub const fn new() -> Self {
        Self
    }
}

crate::impl_default_via_new!(EmbeddedRenderEngine);

impl RenderEngine for EmbeddedRenderEngine {
    fn name(&self) -> &'static str {
        "embedded-render-engine"
    }

    fn profile(&self) -> RuntimeProfile {
        RuntimeProfile::Embedded
    }

    fn init(&self) {
        embedded_engine_shared().init();
    }

    fn run(&self) {
        embedded_engine_shared().run_loop();
    }

    fn quit(&self) {
        embedded_engine_shared().quit();
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        embedded_engine_shared().register_window(title, x, y, width, height)
    }

    fn create_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        embedded_engine_shared().register_button(parent, text, x, y, width, height)
    }
}

/// Build default engine for compile-time profile.
#[cfg(not(alloc_frugal))]
pub fn default_render_engine() -> Box<dyn RenderEngine> {
    if cfg!(embedded_surface) {
        Box::new(EmbeddedRenderEngine::new())
    } else {
        Box::new(NativeRenderEngine::new())
    }
}
/// Default render engine in mini mode uses the embedded engine.
#[cfg(alloc_frugal)]
pub fn default_render_engine() -> Box<dyn RenderEngine> {
    Box::new(EmbeddedRenderEngine::new())
}

#[cfg(all(test, not(alloc_frugal), not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::render_engine::embedded::{set_embedded_target_fps, submit_embedded_task};
    use core::time::Duration;
    #[cfg(not(alloc_frugal))]
    use std::sync::mpsc;
    #[cfg(not(alloc_frugal))]
    use std::thread;

    /// Serialises the embedded-engine tests against **every other module** that drives
    /// the same singleton.
    ///
    /// # Why this must not be a local `OnceLock`
    ///
    /// The embedded engine is process-wide (one paint budget, one window registry, one
    /// target FPS), so the tests here mutate state that `render_engine::embedded`'s tests
    /// and `bindings::binding_impl`'s tests read and write. A module-local mutex excludes
    /// only the tests inside this module — two locks over one resource exclude nothing,
    /// which is precisely how `embedded_target_fps_clamps` failed at random.
    ///
    /// The window was measured rather than assumed: widening that test's assertion span
    /// by 400 ms made `set_embedded_target_fps(120)` from this module land on it and the
    /// assertion report `left: 120, right: 72`. Without the widening the two collide in a
    /// window of nanoseconds, which is why it read as an intermittent flake and passed on
    /// retry.
    ///
    /// Delegating to the one shared guard is what actually excludes them, so the fix is
    /// to remove the local lock rather than to add a second one.
    fn test_guard() -> crate::compat::MutexGuard<'static, ()> {
        crate::render_engine::embedded::embedded_test_guard()
    }

    #[test]
    fn embedded_task_executes_in_run_loop() {
        let _guard = test_guard();
        let engine = EmbeddedRenderEngine::new();
        set_embedded_target_fps(120);
        let (tx, rx) = mpsc::channel();
        submit_embedded_task("unit-test-task", move |frame| {
            let _ = tx.send(frame);
        });
        let runner = engine.clone();
        let handle = thread::spawn(move || {
            runner.run();
        });
        let frame = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("embedded task should execute within timeout");
        assert!(frame >= 1);
        engine.quit();
        handle.join().expect("embedded render loop thread should join");
        // Restore the shared default before releasing the guard. This test raises the
        // target FPS to 120 purely to make the loop tick faster, and leaving it raised
        // would hand a *correct* looking value to whichever test runs next — the other
        // half of the same contamination this guard exists to prevent.
        set_embedded_target_fps(crate::render_engine::embedded::DEFAULT_EMBEDDED_TARGET_FPS);
    }

    #[test]
    fn embedded_task_queue_order_is_deterministic() {
        let _guard = test_guard();
        let engine = EmbeddedRenderEngine::new();
        set_embedded_target_fps(120);
        let (tx, rx) = mpsc::channel();
        submit_embedded_task("task-1", {
            let tx = tx.clone();
            move |_| {
                let _ = tx.send(1u32);
            }
        });
        submit_embedded_task("task-2", {
            let tx = tx.clone();
            move |_| {
                let _ = tx.send(2u32);
            }
        });
        submit_embedded_task("task-3", move |_| {
            let _ = tx.send(3u32);
        });
        let runner = engine.clone();
        let handle = thread::spawn(move || {
            runner.run();
        });
        let first = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first embedded task should execute within timeout");
        let second = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second embedded task should execute within timeout");
        let third = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("third embedded task should execute within timeout");
        assert_eq!([first, second, third], [1, 2, 3]);
        engine.quit();
        handle.join().expect("embedded render loop thread should join");
        // Restore the shared default: see the note in `embedded_task_executes_in_run_loop`.
        set_embedded_target_fps(crate::render_engine::embedded::DEFAULT_EMBEDDED_TARGET_FPS);
    }
}
