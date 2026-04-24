//! Embedded render engine with independent lifecycle and resource registry.

use super::embedded::embedded_engine_shared;
use super::engine_trait::RenderEngine;
use super::native::NativeRenderEngine;
use crate::core::RuntimeProfile;

/// Embedded engine with independent lifecycle and resource registry.
#[derive(Clone)]
pub struct EmbeddedRenderEngine;

impl EmbeddedRenderEngine {
    /// Create embedded engine.
    pub fn new() -> Self {
        Self
    }
}

impl Default for EmbeddedRenderEngine {
    fn default() -> Self {
        Self::new()
    }
}

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
pub fn default_render_engine() -> Box<dyn RenderEngine> {
    if cfg!(feature = "embedded") {
        Box::new(EmbeddedRenderEngine::new())
    } else {
        Box::new(NativeRenderEngine::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_engine::embedded::{set_embedded_target_fps, submit_embedded_task};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
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
        handle
            .join()
            .expect("embedded render loop thread should join");
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
        handle
            .join()
            .expect("embedded render loop thread should join");
    }

    use std::sync::{Mutex, OnceLock};
}
