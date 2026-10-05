// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Embedded runtime state, task queue, and shared engine internals.

#[cfg(not(alloc_frugal))]
use crate::compat::Condvar;
use crate::compat::HashMap;
use crate::compat::Instant;
use crate::compat::Mutex;
use crate::compat::MutexGuard;
use crate::compat::OnceLock;
use crate::compat::{lock, Box, MiniToString, String, Vec};
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;

/// The target frame rate a freshly-created embedded engine starts at.
///
/// `pub(crate)` rather than private because it is the value a test must **restore**:
/// the engine is process-wide, so a test that raises the rate to make its own loop tick
/// faster has to put this back before releasing the shared test guard. Leaving the
/// constant private forced such a test to restate `60`, which is the copy that drifts.
pub(crate) const DEFAULT_EMBEDDED_TARGET_FPS: u32 = 60;
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
        // `id` and `label` are read by `EmbeddedEngineShared::stats` (which publishes the pending
        // `(id, label)` pairs), so they are no longer discarded here.
        if let Some(action) = self.action.take() {
            action(frame_index);
        }
    }
}

/// Runs one frame's task, isolating a panic so the task itself is still consumed
/// and the rest of the frame is still delivered.
///
/// # Why isolation is needed
///
/// The run loop drains a whole frame's tasks before running any of them. A task
/// that panicked unwound out of the `for` loop, so every task after it in the same
/// frame was dropped un-run, and (before the running guard) the loop itself never
/// restarted. Catching the unwind per task makes the outcome explicit: the
/// panicking task is reported and the loop moves on to the next one, so "deliver
/// every drained task" holds unconditionally rather than only when nothing panics.
///
/// `AssertUnwindSafe` is correct here because a panicking task cannot leave any
/// shared invariant the loop reads half-updated: the task owns nothing the loop
/// inspects, and the loop's own state (`running`, `pending_tasks`) is only mutated
/// under the state lock, which a task does not hold while it runs.
#[cfg(not(alloc_frugal))]
fn run_task(task: EmbeddedTask, frame_index: u64) {
    let id = task.id;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        task.run(frame_index);
    }));
    if let Err(_payload) = result {
        log::error!("[embedded] task {id} panicked while running on frame {frame_index}; the remaining tasks of this frame still run");
    }
}

/// Mini-mode task runner: no `std` to catch an unwind with, so the task runs
/// directly. The running guard still restores `running` if it unwinds (under a
/// `panic = abort` toolchain nothing after the panic runs, which is the honest
/// behaviour there).
#[cfg(alloc_frugal)]
fn run_task(task: EmbeddedTask, frame_index: u64) {
    task.run(frame_index);
}

/// Restores `running = false` when the run loop leaves, on the normal path and on
/// the unwind path alike.
///
/// Without it, a task panic unwound out of [`EmbeddedEngineShared::run_loop`] with
/// `running` still `true`, so [`embedded_engine_stats`] reported a live loop and the
/// next `run_loop` saw `running` set and returned immediately — the engine could
/// never be restarted.
struct RunningGuard<'a> {
    shared: &'a EmbeddedEngineShared,
}

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        // Take the state lock even if it is poisoned by an earlier panic: `lock`
        // already recovers the inner value, which is the only way to guarantee the
        // flag is cleared after a panic that poisoned it.
        self.shared.lock_state().running = false;
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
    #[cfg(not(alloc_frugal))]
    wake_signal: Condvar,
}

impl EmbeddedEngineShared {
    fn new() -> Self {
        Self {
            next_widget_id: AtomicU64::new(1),
            next_task_id: AtomicU64::new(1),
            frame_count: AtomicU64::new(0),
            state: Mutex::new(EmbeddedRuntimeState::new()),
            #[cfg(not(alloc_frugal))]
            wake_signal: Condvar::new(),
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, EmbeddedRuntimeState> {
        lock(&self.state)
    }

    fn set_target_fps(&self, fps: u32) -> u32 {
        let mut state = self.lock_state();
        state.target_fps = clamp_embedded_target_fps(fps);
        #[cfg(not(alloc_frugal))]
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

    #[cfg(not(alloc_frugal))]
    pub(crate) fn run_loop(&self) {
        {
            let mut state = self.lock_state();
            if state.running {
                return;
            }
            state.running = true;
        }
        // Clearing `running` is guard-owned, not just done on the normal exit path. A
        // task panic used to unwind straight past the loop, leaving `running == true`
        // forever: the next `run_loop` saw a live run and returned immediately, so the
        // engine never ticked again. The guard restores `running = false` on both the
        // normal break and the unwind path. `run_task` isolates each task's panic, so
        // in practice a task does not unwind here — the guard is the belt to that
        // suspenders, and pins the invariant for any future body that can panic.
        let _running_guard = RunningGuard { shared: self };
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
            // Every drained task is delivered, even if an earlier task in the same
            // frame panics: a panic is reported and the loop continues rather than
            // dropping the rest of the frame's tasks silently.
            for task in tasks {
                run_task(task, frame_index);
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

    #[cfg(alloc_frugal)]
    pub(crate) fn run_loop(&self) {
        // mini: no-thread embedded loop — process tasks inline, no sleeping.
        //
        // There is no second thread to wake this loop, so the desktop arm's
        // `Condvar::wait_timeout` frame pacing has nothing to wait on. Pacing is
        // still applied, by busy-waiting on the same monotonic clock and the same
        // `frame_interval_for_fps` budget: without it the loop would spin at full
        // speed and every task would run far more often than the target rate asks
        // for, which on a device is a battery and thermal problem rather than a
        // cosmetic one. The state lock is re-checked inside the wait so a
        // `stop()` from a task is still observed promptly.
        {
            let mut state = self.lock_state();
            if state.running {
                return;
            }
            state.running = true;
        }
        // See the desktop arm's note: the guard clears `running` on the unwind path as
        // well as the normal one, so a panicking task cannot leave the engine stuck
        // "running" and no-op every subsequent call.
        let _running_guard = RunningGuard { shared: self };
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
                run_task(task, frame_index);
            }
            let frame_interval = frame_interval_for_fps(clamp_embedded_target_fps(target_fps));
            while frame_start.elapsed() < frame_interval {
                if !self.lock_state().running {
                    return;
                }
                core::hint::spin_loop();
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
        // Take the queued tasks under the lock, then drop them **after** releasing it.
        // Each task owns the user closure's captures, so clearing in place while the
        // state lock is held destroyed those captures with the lock held; a capture
        // whose `Drop` re-enters the engine (`embedded_engine_stats`,
        // `submit_embedded_task`, or `quit`) then self-deadlocked on the
        // non-reentrant state mutex. `cancelled` is destroyed with no lock held.
        let cancelled = {
            let mut state = self.lock_state();
            state.running = false;
            state.windows.clear();
            state.buttons.clear();
            core::mem::take(&mut state.pending_tasks)
        };
        drop(cancelled);
        #[cfg(not(alloc_frugal))]
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
        #[cfg(not(alloc_frugal))]
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
            pending_tasks: state
                .pending_tasks
                .iter()
                .map(|task| (task.id, task.label.clone()))
                .collect(),
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
    /// `(id, label)` of every queued task, in submission order.
    ///
    /// # Why the labels are exposed
    ///
    /// `submit_embedded_task` returns a task id, and `EmbeddedTask` stored both an id and a label
    /// that nothing read — so the returned id was a handle a caller could hold but never act on, and
    /// the label was inert. Publishing the pending `(id, label)` pairs makes the id meaningful (it
    /// is what the caller got back) and the label observable in diagnostics, which is the honest way
    /// to keep the two fields rather than leaving them written-but-unread.
    pub pending_tasks: Vec<(u64, alloc::string::String)>,
    /// Number of registered windows tracked by the runtime.
    pub window_count: usize,
    /// Number of registered buttons tracked by the runtime.
    pub button_count: usize,
    /// Current target FPS used by the embedded scheduler.
    pub target_fps: u32,
}

#[cfg(not(alloc_frugal))]
pub(crate) fn embedded_engine_shared() -> Arc<EmbeddedEngineShared> {
    static SHARED: OnceLock<Arc<EmbeddedEngineShared>> = OnceLock::new();
    SHARED.get_or_init(|| Arc::new(EmbeddedEngineShared::new())).clone()
}

#[cfg(alloc_frugal)]
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

/// Returns embedded engine runtime stats for diagnostics and test assertions.
pub fn embedded_engine_stats() -> EmbeddedEngineStats {
    embedded_engine_shared().stats()
}

/// Serialises tests that mutate the process-wide embedded engine.
///
/// # Why this is public
///
/// The embedded engine is a process-wide singleton (one paint budget, one window
/// registry), so any two tests that change it must not interleave — a test that sets
/// the target FPS to 72 while another is asserting the same value fails at random.
///
/// The lock has to be **shared across modules**, and that is the whole point of this
/// function: `src/render_engine/embedded.rs` and `src/bindings/binding_impl.rs` both
/// drive the same singleton, and each used to take its own module-local `OnceLock`.
/// Two locks over one resource exclude nothing, which is how
/// `embedded_target_fps_clamps` came to fail depending on scheduling.
///
/// Callers must take it for the whole mutate-assert-restore span.
#[cfg(test)]
pub(crate) fn embedded_test_guard() -> crate::compat::MutexGuard<'static, ()> {
    use crate::compat::{lock, Mutex, OnceLock};

    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    lock(GUARD.get_or_init(|| Mutex::new(())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_guard() -> crate::compat::MutexGuard<'static, ()> {
        embedded_test_guard()
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

    /// A submitted task's id and label must be observable while it is queued.
    ///
    /// Pins the defect: `EmbeddedTask` stored an `id` and a `label` that nothing read (the old
    /// `run` even had `let _ = self.id; let _ = self.label;`), so `submit_embedded_task`'s returned
    /// id was a handle a caller could hold but never act on. `stats` now publishes the pending
    /// `(id, label)` pairs, which is what makes the returned id meaningful.
    #[test]
    fn a_queued_task_publishes_its_id_and_label() {
        let _guard = test_guard();
        let id = submit_embedded_task("publish-me", |_frame| {});
        let stats = embedded_engine_stats();
        assert!(
            stats
                .pending_tasks
                .iter()
                .any(|(task_id, label)| *task_id == id && label == "publish-me"),
            "the queued task's id and label must be visible in the stats: {:?}",
            stats.pending_tasks
        );
        // Clean up so the queued task does not leak into another test's view.
        embedded_engine_shared().quit();
    }

    /// N-S-30 extension: `quit` must not drop queued tasks' captures under the state lock.
    ///
    /// A queued task owns its closure and therefore everything the closure captured.
    /// `quit` used to `pending_tasks.clear()` while holding the state mutex, so a captured
    /// value whose `Drop` re-enters the engine (`embedded_engine_stats`, which takes the
    /// same mutex) self-deadlocked. The tasks are now taken out under the lock and dropped
    /// after it is released. Run on a worker with a bounded join so a regression fails as a
    /// timeout rather than hanging the suite.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn quit_does_not_drop_task_captures_under_the_lock() {
        use core::time::Duration;
        use std::sync::mpsc;

        /// A capture whose `Drop` re-enters the engine's state lock.
        struct ReentrantProbe;
        impl Drop for ReentrantProbe {
            fn drop(&mut self) {
                // Takes the state mutex that `quit` must not be holding here.
                let _ = embedded_engine_stats();
            }
        }

        let _guard = test_guard();
        // Queue a task that *owns* a re-entrant capture (dropped when the task is
        // dropped by `quit`), leaving it un-run.
        let probe = ReentrantProbe;
        submit_embedded_task("reentrant-quit", move |_frame| {
            let _ = &probe;
        });

        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            embedded_engine_shared().quit();
            let _ = done_tx.send(());
        });

        assert!(
            done_rx.recv_timeout(Duration::from_secs(5)).is_ok(),
            "quit must complete without dropping queued task captures under the state lock"
        );
        worker.join().expect("the quit worker must not panic");
        assert_eq!(embedded_engine_stats().pending_task_count, 0, "quit cleared the queue");
    }

    /// N-S-44: a panicking task must not stop the rest of the frame, and the loop must
    /// stay restartable afterwards.
    ///
    /// A task panic used to unwind out of the `for` loop, dropping every task after it in
    /// the same frame and (before the running guard) leaving `running == true` so the next
    /// run no-opped forever. `run_task` isolates the panic and the guard restores `running`.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_panicking_task_does_not_stop_the_frame_or_the_engine() {
        use crate::render_engine::RenderEngine;
        use core::time::Duration;
        use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
        use std::sync::mpsc;

        let _guard = test_guard();
        set_embedded_target_fps(120);

        let after_ran = Arc::new(AtomicBool::new(false));
        let after_ran_slot = Arc::clone(&after_ran);
        // The panicking task goes first; the flag task second. Both are drained into the
        // same frame, so if the panic aborted the frame the second would never run.
        submit_embedded_task("panicker", |_frame| {
            panic!("deliberate panic from an embedded task");
        });
        let (tx, rx) = mpsc::channel();
        submit_embedded_task("after", move |_frame| {
            after_ran_slot.store(true, AtomicOrdering::SeqCst);
            let _ = tx.send(());
        });

        let engine = crate::render_engine::EmbeddedRenderEngine::new();
        let runner = engine.clone();
        let handle = std::thread::spawn(move || runner.run());

        assert!(
            rx.recv_timeout(Duration::from_secs(2)).is_ok(),
            "a task after a panicking task must still run in the same frame"
        );
        assert!(after_ran.load(AtomicOrdering::SeqCst));

        // Stop and restart: the running flag must have been cleared, so a second loop can
        // run (this is what the running guard makes possible).
        engine.quit();
        handle.join().expect("the embedded loop must join after a task panic");
        set_embedded_target_fps(DEFAULT_EMBEDDED_TARGET_FPS);

        // A fresh loop must not no-op as "already running".
        set_embedded_target_fps(120);
        let (tx2, rx2) = mpsc::channel();
        submit_embedded_task("restart", move |_frame| {
            let _ = tx2.send(());
        });
        let restart_engine = engine.clone();
        let restart = std::thread::spawn(move || restart_engine.run());
        assert!(
            rx2.recv_timeout(Duration::from_secs(2)).is_ok(),
            "the engine must be restartable after a task panic"
        );
        engine.quit();
        restart.join().expect("the restarted loop must join");
        set_embedded_target_fps(DEFAULT_EMBEDDED_TARGET_FPS);
    }
}
