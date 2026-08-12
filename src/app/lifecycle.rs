//! Application lifecycle management — foreground/background state tracking,
//! state save/restore, and lifecycle event notification.

use crate::compat::Instant;
#[cfg(all(feature = "serde", not(any(feature = "mini", feature = "embedded"))))]
use serde::{Deserialize, Serialize};

/// Application lifecycle states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(all(feature = "serde", not(any(feature = "mini", feature = "embedded"))), derive(Serialize, Deserialize))]
pub enum AppLifecycleState {
    /// Application is starting up
    Starting,
    /// Application is running in foreground
    Foreground,
    /// Application is running in background
    Background,
    /// Application is being suspended
    Suspended,
    /// Application is terminating
    Terminating,
}

impl AppLifecycleState {
    /// Returns `true` if the application is actively processing user input
    /// in the foreground.
    pub fn is_active(&self) -> bool {
        matches!(self, AppLifecycleState::Foreground)
    }

    /// Returns `true` if the application's UI is visible to the user
    /// (either starting up or in the foreground).
    pub fn is_visible(&self) -> bool {
        matches!(self, AppLifecycleState::Foreground | AppLifecycleState::Starting)
    }
}

/// Lifecycle event that listeners can subscribe to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// Application is about to enter the foreground
    WillEnterForeground,
    /// Application has entered the foreground
    DidEnterForeground,
    /// Application is about to enter the background
    WillEnterBackground,
    /// Application has entered the background
    DidEnterBackground,
    /// Application is about to terminate
    WillTerminate,
    /// System memory warning received
    MemoryWarning,
    /// State has been restored from a previous session
    StateRestored,
}

/// Callback type for lifecycle events
pub type LifecycleCallback = Box<dyn FnMut(LifecycleEvent) + Send>;

// ── Serializable snapshot for state save/restore ─────────────

/// Intermediate serializable representation of the lifecycle state.
#[derive(Debug, Clone)]
#[cfg_attr(all(feature = "serde", not(any(feature = "mini", feature = "embedded"))), derive(Serialize, Deserialize))]
struct LifecycleSnapshot {
    state: AppLifecycleState,
    total_background_secs: f64,
}

// ── AppLifecycle ─────────────────────────────────────────────

/// Application lifecycle manager.
///
/// Tracks the current lifecycle state, elapsed foreground/background time,
/// and notifies registered listeners on state transitions.
///
/// # Example
///
/// ```rust
/// use rust_widgets::app::lifecycle::{AppLifecycle, AppLifecycleState, LifecycleEvent};
///
/// let mut lc = AppLifecycle::new();
/// assert!(lc.state().is_visible());
///
/// lc.transition(AppLifecycleState::Foreground);
/// assert!(lc.state().is_active());
/// ```
pub struct AppLifecycle {
    state: AppLifecycleState,
    started_at: Instant,
    background_entry: Option<Instant>,
    total_background_duration: std::time::Duration,
    listeners: Vec<LifecycleCallback>,
}

impl AppLifecycle {
    /// Create a new lifecycle manager in the [`Starting`](AppLifecycleState::Starting) state.
    pub fn new() -> Self {
        Self {
            state: AppLifecycleState::Starting,
            started_at: Instant::now(),
            background_entry: None,
            total_background_duration: std::time::Duration::ZERO,
            listeners: Vec::new(),
        }
    }

    /// Transition to a new state and notify listeners of any events that
    /// should fire as a result of the transition.
    ///
    /// The method emits the appropriate pair of `Will*` / `Did*` events
    /// based on the old and new state.
    pub fn transition(&mut self, new_state: AppLifecycleState) {
        let old_state = self.state;
        if old_state == new_state {
            return;
        }

        // Track time spent in background.
        if old_state == AppLifecycleState::Background {
            if let Some(entry) = self.background_entry.take() {
                self.total_background_duration += entry.elapsed();
            }
        }
        if new_state == AppLifecycleState::Background {
            self.background_entry = Some(Instant::now());
        }

        // Clear background entry when leaving suspended state.
        if new_state != AppLifecycleState::Background && new_state != AppLifecycleState::Suspended {
            self.background_entry = None;
        }

        self.state = new_state;

        // Emit lifecycle events based on the transition.
        match (old_state, new_state) {
            (AppLifecycleState::Background, AppLifecycleState::Foreground)
            | (AppLifecycleState::Suspended, AppLifecycleState::Foreground)
            | (AppLifecycleState::Starting, AppLifecycleState::Foreground) => {
                self.fire(LifecycleEvent::WillEnterForeground);
                self.fire(LifecycleEvent::DidEnterForeground);
            }
            (AppLifecycleState::Foreground, AppLifecycleState::Background) => {
                self.fire(LifecycleEvent::WillEnterBackground);
                self.fire(LifecycleEvent::DidEnterBackground);
            }
            (AppLifecycleState::Foreground, AppLifecycleState::Suspended) => {
                self.fire(LifecycleEvent::WillEnterBackground);
                self.fire(LifecycleEvent::DidEnterBackground);
            }
            (_, AppLifecycleState::Terminating) => {
                self.fire(LifecycleEvent::WillTerminate);
            }
            _ => {}
        }
    }

    /// Return the current lifecycle state.
    pub fn state(&self) -> AppLifecycleState {
        self.state
    }

    /// How long the application has been running (wall-clock time since
    /// construction or since the state was last restored).
    pub fn uptime(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }

    /// Total accumulated time the application has spent in the background.
    pub fn total_background_duration(&self) -> std::time::Duration {
        let active = self.background_entry.map_or(std::time::Duration::ZERO, |entry| {
            if self.state == AppLifecycleState::Background
                || self.state == AppLifecycleState::Suspended
            {
                entry.elapsed()
            } else {
                std::time::Duration::ZERO
            }
        });
        self.total_background_duration + active
    }

    /// Register a lifecycle event listener.
    ///
    /// The callback will be invoked for every lifecycle event the manager emits.
    pub fn add_listener(&mut self, callback: LifecycleCallback) {
        self.listeners.push(callback);
    }

    /// Notify all registered listeners of a lifecycle event.
    fn fire(&mut self, event: LifecycleEvent) {
        for cb in &mut self.listeners {
            cb(event);
        }
    }

    /// Serialize current lifecycle state to a JSON string for persistence.
    ///
    /// The serialized data can later be passed to [`deserialize_state`](Self::deserialize_state)
    /// to restore the background-duration accounting. Listeners are **not**
    /// serialized because they are code (not data).
    #[cfg(all(feature = "serde_json", feature = "serde", not(any(feature = "mini", feature = "embedded"))))]
    pub fn serialize_state(&self) -> Result<String, String> {
        let snapshot = LifecycleSnapshot {
            state: self.state,
            total_background_secs: self.total_background_duration().as_secs_f64(),
        };
        serde_json::to_string(&snapshot)
            .map_err(|e| format!("failed to serialize lifecycle state: {e}"))
    }

    /// Restore lifecycle state from previously serialized JSON data.
    ///
    /// The returned [`AppLifecycle`] starts in the state recorded in the
    /// snapshot, and its `started_at` clock is reset to the current time
    /// (so [`uptime`](Self::uptime) measures the time since restoration).
    #[cfg(all(feature = "serde_json", feature = "serde", not(any(feature = "mini", feature = "embedded"))))]
    pub fn deserialize_state(data: &str) -> Result<Self, String> {
        let snapshot: LifecycleSnapshot = serde_json::from_str(data)
            .map_err(|e| format!("failed to deserialize lifecycle state: {e}"))?;
        let mut lc = Self {
            state: snapshot.state,
            started_at: Instant::now(),
            background_entry: None,
            total_background_duration: std::time::Duration::from_secs_f64(
                snapshot.total_background_secs,
            ),
            listeners: Vec::new(),
        };
        // If the snapshot was in a background/suspended state, start tracking
        // the new background interval from now.
        if snapshot.state == AppLifecycleState::Background
            || snapshot.state == AppLifecycleState::Suspended
        {
            lc.background_entry = Some(Instant::now());
        }
        lc.fire(LifecycleEvent::StateRestored);
        Ok(lc)
    }

    /// Emit a synthetic [`LifecycleEvent::MemoryWarning`] to all listeners.
    /// This is typically called by the platform when the system is low on memory.
    pub fn emit_memory_warning(&mut self) {
        self.fire(LifecycleEvent::MemoryWarning);
    }
}

impl Default for AppLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use core::time::Duration;

    // ── 1. State transitions ──────────────────────────────────

    #[test]
    fn test_state_transitions() {
        let mut lc = AppLifecycle::new();
        assert_eq!(lc.state(), AppLifecycleState::Starting);
        assert!(lc.state().is_visible());
        assert!(!lc.state().is_active());

        lc.transition(AppLifecycleState::Foreground);
        assert_eq!(lc.state(), AppLifecycleState::Foreground);
        assert!(lc.state().is_active());
        assert!(lc.state().is_visible());

        lc.transition(AppLifecycleState::Background);
        assert_eq!(lc.state(), AppLifecycleState::Background);
        assert!(!lc.state().is_active());
        assert!(!lc.state().is_visible());

        lc.transition(AppLifecycleState::Suspended);
        assert_eq!(lc.state(), AppLifecycleState::Suspended);
        assert!(!lc.state().is_active());
        assert!(!lc.state().is_visible());

        lc.transition(AppLifecycleState::Terminating);
        assert_eq!(lc.state(), AppLifecycleState::Terminating);
    }

    // ── 2. Transition to same state is a no-op ────────────────

    #[test]
    fn test_same_state_transition_noop() {
        let mut lc = AppLifecycle::new();
        lc.transition(AppLifecycleState::Foreground);
        let call_count = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&call_count);
        lc.add_listener(Box::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        }));
        // Transition to foreground again — should be a no-op.
        lc.transition(AppLifecycleState::Foreground);
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
    }

    // ── 3. Listener notification ──────────────────────────────

    #[test]
    fn test_listener_notification() {
        let mut lc = AppLifecycle::new();
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let ev = Arc::clone(&events);
        lc.add_listener(Box::new(move |e| {
            ev.lock().unwrap().push(e);
        }));

        lc.transition(AppLifecycleState::Foreground);
        lc.transition(AppLifecycleState::Background);
        lc.transition(AppLifecycleState::Terminating);

        let recorded = events.lock().unwrap();
        assert_eq!(recorded.len(), 5);
        assert_eq!(recorded[0], LifecycleEvent::WillEnterForeground);
        assert_eq!(recorded[1], LifecycleEvent::DidEnterForeground);
        assert_eq!(recorded[2], LifecycleEvent::WillEnterBackground);
        assert_eq!(recorded[3], LifecycleEvent::DidEnterBackground);
        assert_eq!(recorded[4], LifecycleEvent::WillTerminate);
    }

    // ── 4. Serialization roundtrip ────────────────────────────

    #[test]
    fn test_serialization_roundtrip() {
        let mut lc = AppLifecycle::new();
        lc.transition(AppLifecycleState::Foreground);

        // Add some background time.
        lc.transition(AppLifecycleState::Background);
        lc.transition(AppLifecycleState::Foreground);

        let json = lc.serialize_state().expect("serialize_state should succeed");
        let restored =
            AppLifecycle::deserialize_state(&json).expect("deserialize_state should succeed");

        // State should match.
        assert_eq!(restored.state(), AppLifecycleState::Foreground);

        // Total background should be greater than zero.
        assert!(restored.total_background_duration() > Duration::ZERO);
    }

    // ── 5. Uptime tracking ────────────────────────────────────

    #[test]
    fn test_uptime_increases() {
        let lc = AppLifecycle::new();
        let u1 = lc.uptime();
        std::thread::sleep(Duration::from_millis(10));
        let u2 = lc.uptime();
        assert!(u2 >= u1 + Duration::from_millis(5));
    }

    // ── 6. Background duration accumulation ───────────────────

    #[test]
    fn test_background_duration_accumulates() {
        let mut lc = AppLifecycle::new();
        lc.transition(AppLifecycleState::Foreground);
        assert_eq!(lc.total_background_duration(), Duration::ZERO);

        lc.transition(AppLifecycleState::Background);
        std::thread::sleep(Duration::from_millis(10));
        lc.transition(AppLifecycleState::Foreground);

        let bg = lc.total_background_duration();
        assert!(bg >= Duration::from_millis(5));
        // Transitioning again should accumulate further.
        lc.transition(AppLifecycleState::Background);
        std::thread::sleep(Duration::from_millis(5));
        lc.transition(AppLifecycleState::Foreground);
        let bg2 = lc.total_background_duration();
        assert!(bg2 > bg);
    }

    // ── 7. Memory warning notification ────────────────────────

    #[test]
    fn test_memory_warning() {
        let mut lc = AppLifecycle::new();
        let warned = Arc::new(AtomicUsize::new(0));
        let w = Arc::clone(&warned);
        lc.add_listener(Box::new(move |e| {
            if e == LifecycleEvent::MemoryWarning {
                w.fetch_add(1, Ordering::SeqCst);
            }
        }));
        lc.emit_memory_warning();
        assert_eq!(warned.load(Ordering::SeqCst), 1);
    }

    // ── 8. Deserialize preserves state across roundtrips ────────

    #[test]
    fn test_deserialize_preserves_state() {
        let mut lc = AppLifecycle::new();
        lc.transition(AppLifecycleState::Foreground);
        let json = lc.serialize_state().unwrap();

        let d = AppLifecycle::deserialize_state(&json).unwrap();
        assert_eq!(d.state(), AppLifecycleState::Foreground);
        assert!(d.state().is_active());
        assert!(d.state().is_visible());

        // Also test that deserialize resets the clock.
        // uptime at deserialized instance should be near zero.
        assert!(d.uptime() < Duration::from_millis(100));
    }
}
