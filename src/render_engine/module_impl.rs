//! Dual-engine render abstraction for native and embedded runtime paths.

use crate::core::RuntimeProfile;
use crate::platform::get_platform;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

#[cfg(feature = "gpu-wgpu")]
pub use crate::wgpu_backend::WgpuRenderer;

const DEFAULT_EMBEDDED_TARGET_FPS: u32 = 60;
const MIN_EMBEDDED_TARGET_FPS: u32 = 1;
const MAX_EMBEDDED_TARGET_FPS: u32 = 240;

fn clamp_embedded_target_fps(fps: u32) -> u32 {
    fps.clamp(MIN_EMBEDDED_TARGET_FPS, MAX_EMBEDDED_TARGET_FPS)
}

fn frame_interval_for_fps(fps: u32) -> Duration {
    Duration::from_nanos(1_000_000_000 / fps as u64)
}

type EmbeddedTaskFn = Box<dyn FnOnce(u64) + Send + 'static>;

struct EmbeddedTask {
    id: u64,
    label: String,
    action: Option<EmbeddedTaskFn>,
}

impl EmbeddedTask {
    fn new(id: u64, label: String, action: EmbeddedTaskFn) -> Self {
        Self {
            id,
            label,
            action: Some(action),
        }
    }

    fn run(mut self, frame_index: u64) {
        let _ = self.id;
        let _ = self.label;
        if let Some(action) = self.action.take() {
            action(frame_index);
        }
    }
}

#[derive(Default)]
struct EmbeddedRuntimeState {
    initialized: bool,
    running: bool,
    target_fps: u32,
    windows: HashMap<u64, EmbeddedWindowRecord>,
    buttons: HashMap<u64, EmbeddedButtonRecord>,
    pending_tasks: VecDeque<EmbeddedTask>,
}

impl EmbeddedRuntimeState {
    fn new() -> Self {
        Self {
            initialized: false,
            running: false,
            target_fps: DEFAULT_EMBEDDED_TARGET_FPS,
            windows: HashMap::new(),
            buttons: HashMap::new(),
            pending_tasks: VecDeque::new(),
        }
    }
}

struct EmbeddedEngineShared {
    next_widget_id: AtomicU64,
    next_task_id: AtomicU64,
    frame_count: AtomicU64,
    state: Mutex<EmbeddedRuntimeState>,
    wake_signal: Condvar,
}

impl EmbeddedEngineShared {
    fn new() -> Self {
        Self {
            next_widget_id: AtomicU64::new(1),
            next_task_id: AtomicU64::new(1),
            frame_count: AtomicU64::new(0),
            state: Mutex::new(EmbeddedRuntimeState::new()),
            wake_signal: Condvar::new(),
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, EmbeddedRuntimeState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn set_target_fps(&self, fps: u32) -> u32 {
        let mut state = self.lock_state();
        state.target_fps = clamp_embedded_target_fps(fps);
        self.wake_signal.notify_all();
        state.target_fps
    }

    fn target_fps(&self) -> u32 {
        self.lock_state().target_fps
    }

    fn init(&self) {
        let mut state = self.lock_state();
        if state.initialized {
            return;
        }
        state.initialized = true;
    }

    fn run_loop(&self) {
        {
            let mut state = self.lock_state();
            if state.running {
                return;
            }
            state.running = true;
        }

        loop {
            let frame_start = Instant::now();

            let (tasks, target_fps, still_running) = {
                let mut state = self.lock_state();
                let still_running = state.running;
                let target_fps = state.target_fps;
                let tasks = state.pending_tasks.drain(..).collect::<Vec<_>>();
                (tasks, target_fps, still_running)
            };

            if !still_running {
                break;
            }

            let frame_index = self.frame_count.fetch_add(1, Ordering::SeqCst) + 1;
            for task in tasks {
                task.run(frame_index);
            }

            let frame_interval = frame_interval_for_fps(clamp_embedded_target_fps(target_fps));
            let elapsed = frame_start.elapsed();
            if elapsed < frame_interval {
                let wait_duration = frame_interval - elapsed;
                let state = self.lock_state();
                if !state.running {
                    break;
                }
                let _ = self
                    .wake_signal
                    .wait_timeout(state, wait_duration)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        }
    }

    fn quit(&self) {
        let mut state = self.lock_state();
        state.running = false;
        state.pending_tasks.clear();
        drop(state);
        self.wake_signal.notify_all();
    }

    fn alloc_widget_id(&self) -> u64 {
        self.next_widget_id.fetch_add(1, Ordering::SeqCst)
    }

    fn register_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let window_id = self.alloc_widget_id();
        let mut state = self.lock_state();
        state.windows.insert(
            window_id,
            EmbeddedWindowRecord {
                id: window_id,
                title: title.to_string(),
                x,
                y,
                width,
                height,
            },
        );
        window_id
    }

    fn register_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        let button_id = self.alloc_widget_id();
        let mut state = self.lock_state();
        state.buttons.insert(
            button_id,
            EmbeddedButtonRecord {
                id: button_id,
                parent,
                text: text.to_string(),
                x,
                y,
                width,
                height,
            },
        );
        button_id
    }

    fn submit_task<F>(&self, label: String, action: F) -> u64
    where
        F: FnOnce(u64) + Send + 'static,
    {
        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst);
        let mut state = self.lock_state();
        state
            .pending_tasks
            .push_back(EmbeddedTask::new(task_id, label, Box::new(action)));
        drop(state);
        self.wake_signal.notify_all();
        task_id
    }

    fn stats(&self) -> EmbeddedEngineStats {
        let state = self.lock_state();
        EmbeddedEngineStats {
            initialized: state.initialized,
            running: state.running,
            frame_count: self.frame_count.load(Ordering::SeqCst),
            pending_task_count: state.pending_tasks.len(),
            window_count: state.windows.len(),
            button_count: state.buttons.len(),
            target_fps: state.target_fps,
        }
    }
}

/// Snapshot record of an embedded window handle and geometry.
#[derive(Clone, Debug)]
pub struct EmbeddedWindowRecord {
    /// Logical window id allocated by the platform backend.
    pub id: u64,
    /// Window title at creation time.
    pub title: String,
    /// Window origin X in logical pixels.
    pub x: i32,
    /// Window origin Y in logical pixels.
    pub y: i32,
    /// Window width in logical pixels.
    pub width: u32,
    /// Window height in logical pixels.
    pub height: u32,
}

/// Snapshot record of an embedded button handle and geometry.
#[derive(Clone, Debug)]
pub struct EmbeddedButtonRecord {
    /// Logical button id allocated by the platform backend.
    pub id: u64,
    /// Parent logical widget id.
    pub parent: u64,
    /// Button text at creation time.
    pub text: String,
    /// Button origin X in logical pixels.
    pub x: i32,
    /// Button origin Y in logical pixels.
    pub y: i32,
    /// Button width in logical pixels.
    pub width: u32,
    /// Button height in logical pixels.
    pub height: u32,
}

/// Runtime statistics for the embedded render-engine loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddedEngineStats {
    /// Whether the embedded engine has completed initialization.
    pub initialized: bool,
    /// Whether the embedded run loop is currently active.
    pub running: bool,
    /// Number of frames processed by the embedded run loop.
    pub frame_count: u64,
    /// Number of queued tasks waiting for the next frame.
    pub pending_task_count: usize,
    /// Number of registered windows tracked by the runtime.
    pub window_count: usize,
    /// Number of registered buttons tracked by the runtime.
    pub button_count: usize,
    /// Current target FPS used by the embedded scheduler.
    pub target_fps: u32,
}

fn embedded_engine_shared() -> Arc<EmbeddedEngineShared> {
    static SHARED: OnceLock<Arc<EmbeddedEngineShared>> = OnceLock::new();
    SHARED
        .get_or_init(|| Arc::new(EmbeddedEngineShared::new()))
        .clone()
}

/// Unified rendering/runtime engine abstraction.
pub trait RenderEngine: Send + Sync {
    /// Engine display name.
    fn name(&self) -> &'static str;
    /// Runtime profile category.
    fn profile(&self) -> RuntimeProfile;
    /// Initialize engine resources.
    fn init(&self);
    /// Run engine event loop.
    fn run(&self);
    /// Request event loop shutdown.
    fn quit(&self);
    /// Create a top-level window.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64;
    /// Create a button control.
    fn create_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64;
}

/// Native desktop engine backed by platform adapters.
pub struct NativeRenderEngine;

impl NativeRenderEngine {
    /// Create native engine.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NativeRenderEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderEngine for NativeRenderEngine {
    fn name(&self) -> &'static str {
        "native-render-engine"
    }

    fn profile(&self) -> RuntimeProfile {
        RuntimeProfile::Full
    }

    fn init(&self) {
        get_platform().init();
    }

    fn run(&self) {
        get_platform().run();
    }

    fn quit(&self) {
        get_platform().quit();
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        get_platform().create_window(title, x, y, width, height)
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
        get_platform().create_button(parent, text, x, y, width, height)
    }
}

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

/// Set embedded engine target FPS. Returns the applied clamped FPS value.
pub fn set_embedded_target_fps(fps: u32) -> u32 {
    embedded_engine_shared().set_target_fps(fps)
}

/// Read embedded engine target FPS.
pub fn embedded_target_fps() -> u32 {
    embedded_engine_shared().target_fps()
}

/// Submit a task to execute on the next embedded frame.
pub fn submit_embedded_task<F>(label: impl Into<String>, action: F) -> u64
where
    F: FnOnce(u64) + Send + 'static,
{
    embedded_engine_shared().submit_task(label.into(), action)
}

/// Return embedded engine runtime stats for diagnostics and test assertions.
pub fn embedded_engine_stats() -> EmbeddedEngineStats {
    embedded_engine_shared().stats()
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
    use std::sync::mpsc;
    use std::thread;

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn embedded_target_fps_clamps() {
        let _guard = test_guard();
        assert_eq!(set_embedded_target_fps(0), MIN_EMBEDDED_TARGET_FPS);
        assert_eq!(set_embedded_target_fps(999), MAX_EMBEDDED_TARGET_FPS);
        assert_eq!(set_embedded_target_fps(72), 72);
        assert_eq!(embedded_target_fps(), 72);
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

    #[test]
    fn embedded_resource_registry_tracks_window_and_button() {
        let _guard = test_guard();
        let before = embedded_engine_stats();

        let shared = embedded_engine_shared();
        let window_id = shared.register_window("stats", 1, 2, 300, 200);
        let _button_id = shared.register_button(window_id, "ok", 10, 10, 80, 24);

        let after = embedded_engine_stats();

        assert!(after.window_count > before.window_count);
        assert!(after.button_count > before.button_count);
    }
}

