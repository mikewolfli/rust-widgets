//! Embedded runtime state, task queue, and shared engine internals.

use crate::compat::Condvar;
use crate::compat::HashMap;
use crate::compat::Instant;
use crate::compat::Mutex;
use crate::compat::MutexGuard;
use crate::compat::OnceLock;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;

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
        Self { id, label, action: Some(action) }
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

pub(crate) struct EmbeddedEngineShared {
    next_widget_id: AtomicU64,
    next_task_id: AtomicU64,
    frame_count: AtomicU64,
    state: Mutex<EmbeddedRuntimeState>,
    #[cfg(not(feature = "mini"))]
    wake_signal: Condvar,
}

impl EmbeddedEngineShared {
    fn new() -> Self {
        Self {
            next_widget_id: AtomicU64::new(1),
            next_task_id: AtomicU64::new(1),
            frame_count: AtomicU64::new(0),
            state: Mutex::new(EmbeddedRuntimeState::new()),
            #[cfg(not(feature = "mini"))]
            wake_signal: Condvar::new(),
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, EmbeddedRuntimeState> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn set_target_fps(&self, fps: u32) -> u32 {
        let mut state = self.lock_state();
        state.target_fps = clamp_embedded_target_fps(fps);
        #[cfg(not(feature = "mini"))]
        self.wake_signal.notify_all();
        state.target_fps
    }

    fn target_fps(&self) -> u32 {
        self.lock_state().target_fps
    }

    pub(crate) fn init(&self) {
        let mut state = self.lock_state();
        if state.initialized {
            return;
        }
        state.initialized = true;
    }

    #[cfg(not(feature = "mini"))]
    pub(crate) fn run_loop(&self) {
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

    #[cfg(feature = "mini")]
    pub(crate) fn run_loop(&self) {
        // mini: no-thread embedded loop — process tasks inline, no sleep/wait
        {
            let mut state = self.lock_state();
            if state.running {
                return;
            }
            state.running = true;
        }
        loop {
            let (tasks, still_running) = {
                let mut state = self.lock_state();
                let still_running = state.running;
                let tasks = state.pending_tasks.drain(..).collect::<Vec<_>>();
                (tasks, still_running)
            };
            if !still_running {
                break;
            }
            let frame_index = self.frame_count.fetch_add(1, Ordering::SeqCst) + 1;
            for task in tasks {
                task.run(frame_index);
            }
        }
    }

    pub(crate) fn _destroy_window(&self, window_id: u64) {
        let mut state = self.lock_state();
        state.windows.remove(&window_id);
    }

    pub(crate) fn _destroy_button(&self, button_id: u64) {
        let mut state = self.lock_state();
        state.buttons.remove(&button_id);
    }

    pub(crate) fn quit(&self) {
        let mut state = self.lock_state();
        state.running = false;
        state.windows.clear();
        state.buttons.clear();
        state.pending_tasks.clear();
        drop(state);
        #[cfg(not(feature = "mini"))]
        self.wake_signal.notify_all();
    }

    fn alloc_widget_id(&self) -> u64 {
        self.next_widget_id.fetch_add(1, Ordering::SeqCst)
    }

    pub(crate) fn register_window(
        &self,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        let window_id = self.alloc_widget_id();
        let mut state = self.lock_state();
        state.windows.insert(
            window_id,
            EmbeddedWindowRecord { id: window_id, title: title.to_string(), x, y, width, height },
        );
        window_id
    }

    pub(crate) fn register_button(
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
        state.pending_tasks.push_back(EmbeddedTask::new(task_id, label, Box::new(action)));
        drop(state);
        #[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
pub(crate) fn embedded_engine_shared() -> Arc<EmbeddedEngineShared> {
    static SHARED: OnceLock<Arc<EmbeddedEngineShared>> = OnceLock::new();
    SHARED.get_or_init(|| Arc::new(EmbeddedEngineShared::new())).clone()
}

#[cfg(feature = "mini")]
pub(crate) fn embedded_engine_shared() -> Arc<EmbeddedEngineShared> {
    static SHARED: OnceLock<Arc<EmbeddedEngineShared>> = OnceLock::new();
    SHARED.get_or_init(|| Arc::new(EmbeddedEngineShared::new())).clone()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_guard() -> crate::compat::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn embedded_target_fps_clamps() {
        let _guard = test_guard();
        // Reset to default first — other tests may leave the global state
        // at a non-default value since the embedded engine is a process-wide singleton.
        set_embedded_target_fps(DEFAULT_EMBEDDED_TARGET_FPS);
        assert_eq!(set_embedded_target_fps(0), MIN_EMBEDDED_TARGET_FPS);
        assert_eq!(set_embedded_target_fps(999), MAX_EMBEDDED_TARGET_FPS);
        assert_eq!(set_embedded_target_fps(72), 72);
        assert_eq!(embedded_target_fps(), 72);
        // Clean up for subsequent tests
        set_embedded_target_fps(DEFAULT_EMBEDDED_TARGET_FPS);
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
