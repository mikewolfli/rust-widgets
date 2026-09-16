// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Raw touch and hardware-button input for embedded targets.
//!
//! [`HardwareInputManager`] is the event source: it tracks live contacts and
//! button levels and turns a completed contact into a [`GestureType`]. It is
//! self-contained and does **not** feed the crate-level gesture engine; see
//! [`HardwareInputManager`] for how the two relate and which gestures each can
//! produce.
//!
//! [`InputFilter`] sits upstream of any handler and cleans a raw stream: it drops
//! light contacts and suppresses jitter. It is stateful across events, so one
//! instance per contact stream.

use crate::core::Point;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
/// Which physical input path an event arrived on.
///
/// A classification tag for routing and for deciding which affordances to offer;
/// it carries no data about the event itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    /// A finger or stylus contact on the panel.
    Touch,
    /// A pointing device with a persistent cursor position.
    Mouse,
    /// Text entry, as key codes rather than characters.
    Keyboard,
    /// A physical button wired to the board, addressed by id rather than key code.
    HardwareButton,
    /// A recognised multi-finger or timed motion; see [`GestureType`].
    Gesture,
}
/// Phase of one touch contact's lifetime.
///
/// Ordered, but only [`TouchEvent::Down`] and [`TouchEvent::Cancel`] are terminal
/// for a contact — see [`HardwareInputManager::process_touch`] for how each phase
/// is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchEvent {
    /// The contact began on the panel.
    Down,
    /// The contact moved while still down.
    Move,
    /// The contact ended normally, so a gesture may be recognised from it.
    Up,
    /// The contact was taken over by the system, so it must not produce a gesture.
    Cancel,
}
/// One finger's state at a point in time.
///
/// Immutable snapshot rather than a live cursor: [`HardwareInputManager`] replaces
/// the stored point for an id as moves arrive.
#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    /// Contact identifier, stable for the whole contact. A second finger gets a
    /// different id, which is what makes multi-touch separable.
    pub id: u32,
    /// Where the contact is, relative to the panel origin and in logical pixels.
    pub position: Point,
    /// Tip pressure in the normalised range `0.0..=1.0`, where `1.0` is full
    /// press. Reported as `1.0` by [`TouchPoint::new`], because a panel that does
    /// not measure pressure should read as a firm touch, not a light one.
    pub pressure: f32,
    /// Contact area in the units the digitizer reports, typically `1.0` for a
    /// whole finger. Units are device-dependent, so this is only comparable
    /// between events from the same panel. Reported as `1.0` by
    /// [`TouchPoint::new`].
    pub size: f32,
}
impl TouchPoint {
    /// Creates a contact at `(x, y)` with full pressure and unit size.
    ///
    /// `x` and `y` are logical pixels relative to the panel origin.
    pub fn new(id: u32, x: i32, y: i32) -> Self {
        Self { id, position: Point::new(x, y), pressure: 1.0, size: 1.0 }
    }
    /// Sets [`TouchPoint::pressure`], clamped to `0.0..=1.0`.
    ///
    /// Clamping is silent, so a driver reporting a raw out-of-range value gets a
    /// usable point rather than a rejection; check the stored field if the exact
    /// value matters.
    pub fn with_pressure(mut self, pressure: f32) -> Self {
        self.pressure = pressure.clamp(0.0, 1.0);
        self
    }
}
/// State change of a physical button wired to the board.
#[derive(Debug, Clone, Copy)]
pub struct HardwareButtonEvent {
    /// Which button, in the board's own numbering. Values are id-keyed into the
    /// 32 slots of [`HardwareInputManager`], so ids `32` and above are dropped.
    pub button_id: u32,
    /// `true` for press, `false` for release — not a toggle.
    pub pressed: bool,
    /// When the change was observed, as a monotonic instant.
    ///
    /// Suited to measuring an interval (a hold duration, a debounce window), not
    /// to be confused with wall-clock time: it is meaningless across processes
    /// and does not survive a reboot.
    pub timestamp: Instant,
}
/// A recognised gesture, as produced by [`HardwareInputManager`].
#[derive(Debug, Clone, Copy)]
pub struct GestureEvent {
    /// Which gesture was recognised. This is the field to match on; the remaining
    /// fields are meaningful only for some of its values.
    pub gesture_type: GestureType,
    /// The gesture's reference point, in logical pixels. For the discrete
    /// gestures produced by this module it is *where the finger lifted*, not the
    /// midpoint of the motion.
    pub center: Point,
    /// Scale change relative to the start of the gesture, where `1.0` means no
    /// change. Never anything but `1.0` from [`HardwareInputManager`], which has
    /// no pinch recogniser; the field exists so a gesture carries the same shape
    /// as the crate-level recognisers' output.
    pub scale: f32,
    /// Rotation in radians, `0.0` meaning no rotation. Never anything but `0.0`
    /// from [`HardwareInputManager`], which has no rotate recogniser.
    pub rotation: f32,
    /// Motion rate as `(dx, dy)` in logical pixels per *second* — the underlying
    /// division is by `Duration::as_secs_f32()`. `(0.0, 0.0)` for the discrete
    /// gestures, which have no direction. Only swipes populate it.
    pub velocity: (f32, f32),
}
/// The gestures [`HardwareInputManager`] can recognise.
///
/// Note what is missing: [`GestureType::DoubleTap`], [`GestureType::Pan`],
/// [`GestureType::Pinch`] and [`GestureType::Rotate`] have no producer in this
/// module, because its recogniser is a single `Down`/`Up` pair (see
/// [`HardwareInputManager::process_touch`]). They are kept because the richer
/// crate-level recogniser set names the same gestures, so code holding a
/// `GestureType` does not have to switch representation to mention one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureType {
    /// A short, nearly stationary contact.
    Tap,
    /// Two quick taps; **not currently produced** by this module.
    DoubleTap,
    /// A long, nearly stationary contact.
    LongPress,
    /// Continuous dragging motion; **not currently produced** by this module.
    Pan,
    /// Two-finger scale change; **not currently produced** by this module.
    Pinch,
    /// Two-finger twist; **not currently produced** by this module.
    Rotate,
    /// A leftward drag past the swipe threshold.
    SwipeLeft,
    /// A rightward drag past the swipe threshold.
    SwipeRight,
    /// An upward drag past the swipe threshold.
    SwipeUp,
    /// A downward drag past the swipe threshold.
    SwipeDown,
}
/// Raw-input tracker that turns a stream of touch and button events into gestures.
///
/// # What it holds between events
///
/// Recognition is stateful, and the state is the whole contact lifetime, not the
/// last event:
///
/// * `touch_start_time` / `touch_start_position` are set on the first
///   [`TouchEvent::Down`] and are **not** updated by later `Down` events, so only
///   one contact's origin is remembered even when several fingers are down.
/// * `last_touch_position` is tracked on every `Down` and `Move` so a later
///   recogniser can read the latest position, but the current recogniser derives
///   everything from `touch_start_position` instead, so nothing reads it yet.
///   It is live state rather than a snapshot, so it can be used as-is.
/// * `button_states` is a 32-slot map, not a queue: transient press/release pairs
///   between two calls to [`Self::process_button`] are not observable, only the
///   current level.
/// * Recognised gestures accumulate in a queue and wait for
///   [`Self::get_gesture`], so a caller that never drains it will hold them
///   forever.
///
/// All three origin fields are cleared once the last contact lifts, so a new
/// sequence starts clean. [`Self::clear`] resets everything at once.
///
/// # Relationship to the crate's gesture recognisers
///
/// This is a self-contained, allocation-light recogniser attached to the raw
///
/// input path. It is **not** the crate-level gesture engine, which consumes
/// already-constructed `event::Event`s and recognises eleven gestures through a
/// chain of individual recognisers. The two do not interact: this manager emits
/// [`GestureEvent`], the engine emits `Event::Tap` and friends.
///
/// # Thresholds
///
/// See [`Self::new`] for the values; they are fixed per instance and there is no
/// setter for any of them.
pub struct HardwareInputManager {
    touch_points: Vec<TouchPoint>,
    button_states: [bool; 32],
    gesture_buffer: VecDeque<GestureEvent>,
    touch_start_time: Option<Instant>,
    touch_start_position: Option<Point>,
    last_touch_position: Option<Point>,
    tap_threshold: Duration,
    long_press_threshold: Duration,
    // double_tap_threshold: Duration,
    swipe_threshold: i32,
}
impl HardwareInputManager {
    /// Creates a manager with the standard thresholds and no contacts in flight.
    ///
    /// * `tap_threshold` — `200` ms: a contact shorter than this that moved less
    ///   than the swipe threshold is a [`GestureType::Tap`].
    /// * `long_press_threshold` — `500` ms: at or beyond this, with the same
    ///   movement limit, it is a [`GestureType::LongPress`] instead.
    /// * `swipe_threshold` — `50` logical pixels, used for both axes, and compared
    ///   against the straight-line distance from the start point.
    ///
    /// The gesture queue is pre-reserved for 16 events but is unbounded, since a
    /// [`VecDeque`] grows. Note the two boundaries leave a gap: a contact between
    /// `200` ms and `500` ms that barely moved matches no gesture at all.
    pub fn new() -> Self {
        Self {
            touch_points: Vec::new(),
            button_states: [false; 32],
            gesture_buffer: VecDeque::with_capacity(16),
            touch_start_time: None,
            touch_start_position: None,
            last_touch_position: None,
            tap_threshold: Duration::from_millis(200),
            long_press_threshold: Duration::from_millis(500),
            // double_tap_threshold: Duration::from_millis(300),
            swipe_threshold: 50,
        }
    }
    /// Feeds one touch phase into the tracker.
    ///
    /// * [`TouchEvent::Down`] records the contact's origin and time (overwriting
    ///   any previous origin) and adds the point.
    /// * [`TouchEvent::Move`] replaces the stored point with the same `id`, if one
    ///   is being tracked. A move for an unknown id is dropped — it does not start
    ///   a contact.
    /// * [`TouchEvent::Up`] runs gesture recognition *first* (using the stored
    ///   origin, not `point`'s), then removes the contact. When it was the last
    ///   one, the origin and timing state is cleared.
    /// * [`TouchEvent::Cancel`] removes the contact without recognising anything,
    ///   but does **not** clear the origin and timing state. A `Cancel` that
    ///   empties `touch_points` therefore leaves a stale start position behind,
    ///   which the next `Up` from a *different* contact can be measured against.
    pub fn process_touch(&mut self, event: TouchEvent, point: TouchPoint) {
        match event {
            TouchEvent::Down => {
                self.touch_start_time = Some(Instant::now());
                self.touch_start_position = Some(point.position);
                self.last_touch_position = Some(point.position);
                self.touch_points.push(point);
            }
            TouchEvent::Move => {
                if let Some(index) = self.touch_points.iter().position(|p| p.id == point.id) {
                    self.touch_points[index] = point;
                    self.last_touch_position = Some(point.position);
                }
            }
            TouchEvent::Up => {
                self.detect_gesture(point.position);
                self.touch_points.retain(|p| p.id != point.id);
                if self.touch_points.is_empty() {
                    self.touch_start_time = None;
                    self.touch_start_position = None;
                    self.last_touch_position = None;
                }
            }
            TouchEvent::Cancel => {
                self.touch_points.retain(|p| p.id != point.id);
            }
        }
    }
    /// Classifies one completed contact and queues the result.
    ///
    /// Measures from the stored origin to `end_position`, against the stored
    /// start time — so the classification depends on when `Down` arrived, not on
    /// when the finger stopped moving.
    ///
    /// Exactly one gesture can be queued, and the branches are exclusive in this
    /// order: a short, short-distance contact is a tap; an equally short-distance
    /// contact that lasted at least the long-press threshold is a long press;
    /// anything that travelled at least the swipe threshold is a swipe in the
    /// dominant axis. A contact that is neither short nor far queues nothing.
    ///
    /// The swipe's velocity is computed over the *whole* contact, in logical
    /// pixels per millisecond (the denominator is `Duration::as_secs_f32`, i.e.
    /// seconds, so `dx / dt_seconds` is px/s and is **not** px/ms as a caller
    /// might read it). The denominator is floored at `1e-6`, which caps the
    /// reported speed instead of producing an infinity when a contact is
    /// instantaneous.
    ///
    /// Does nothing when no start position or time has been recorded, which is
    /// the case for a stray `Up` with no matching `Down`.
    fn detect_gesture(&mut self, end_position: Point) {
        if let (Some(start_time), Some(start_pos)) =
            (self.touch_start_time, self.touch_start_position)
        {
            let duration = start_time.elapsed();
            let dx = end_position.x - start_pos.x;
            let dy = end_position.y - start_pos.y;
            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            if duration < self.tap_threshold && distance < self.swipe_threshold as f32 {
                self.gesture_buffer.push_back(GestureEvent {
                    gesture_type: GestureType::Tap,
                    center: end_position,
                    scale: 1.0,
                    rotation: 0.0,
                    velocity: (0.0, 0.0),
                });
            } else if duration >= self.long_press_threshold
                && distance < self.swipe_threshold as f32
            {
                self.gesture_buffer.push_back(GestureEvent {
                    gesture_type: GestureType::LongPress,
                    center: end_position,
                    scale: 1.0,
                    rotation: 0.0,
                    velocity: (0.0, 0.0),
                });
            } else if distance >= self.swipe_threshold as f32 {
                let gesture_type = if dx.abs() > dy.abs() {
                    if dx > 0 {
                        GestureType::SwipeRight
                    } else {
                        GestureType::SwipeLeft
                    }
                } else {
                    if dy > 0 {
                        GestureType::SwipeDown
                    } else {
                        GestureType::SwipeUp
                    }
                };
                self.gesture_buffer.push_back(GestureEvent {
                    gesture_type,
                    center: end_position,
                    scale: 1.0,
                    rotation: 0.0,
                    velocity: (
                        dx as f32 / duration.as_secs_f32().max(1e-6),
                        dy as f32 / duration.as_secs_f32().max(1e-6),
                    ),
                });
            }
        }
    }
    /// Records the current level of a physical button.
    ///
    /// `button_id` indexes a fixed table of 32 slots; ids `32` and above are
    /// silently ignored rather than growing the table, which keeps this
    /// allocation-free. Idempotent — writing the same level twice is harmless.
    pub fn process_button(&mut self, button_id: u32, pressed: bool) {
        if button_id < 32 {
            self.button_states[button_id as usize] = pressed;
        }
    }
    /// Returns the last level recorded for `button_id`.
    ///
    /// A never-seen button and an out-of-range id (32 or above, the same range
    /// [`Self::process_button`] drops) both read as `false`, so those two cases
    /// are indistinguishable here.
    pub fn is_button_pressed(&self, button_id: u32) -> bool {
        if button_id < 32 {
            self.button_states[button_id as usize]
        } else {
            false
        }
    }
    /// Removes and returns the oldest queued gesture, or `None` when none is
    /// waiting.
    ///
    /// Draining is the caller's job: gestures pile up in order of recognition and
    /// are never coalesced, expired or dropped by the manager itself.
    pub fn get_gesture(&mut self) -> Option<GestureEvent> {
        self.gesture_buffer.pop_front()
    }
    /// Returns how many contacts are currently down.
    ///
    /// Counts tracked ids, so a contact that went `Down` and never moved is
    /// still counted — and a `Cancel` for an unknown id does not change it.
    pub fn touch_point_count(&self) -> usize {
        self.touch_points.len()
    }
    /// Borrows every contact currently down, in the order the contacts first
    /// arrived.
    ///
    /// The slice is live state, not a snapshot: later calls to
    /// [`Self::process_touch`] can reorder it when a contact is removed.
    pub fn get_touch_points(&self) -> &[TouchPoint] {
        &self.touch_points
    }
    /// Drops all tracked state: contacts, button levels, queued gestures, and the
    /// start time, start position and last position.
    ///
    /// Use this when input is disconnected and any partial contact should not be
    /// carried into the next session. No gesture is recognised from what was
    /// cleared, so an in-flight touch is abandoned rather than cancelled — which
    /// is also why the queued gestures are discarded rather than delivered.
    pub fn clear(&mut self) {
        self.touch_points.clear();
        self.button_states = [false; 32];
        self.gesture_buffer.clear();
        self.touch_start_time = None;
        self.touch_start_position = None;
        self.last_touch_position = None;
    }
}
crate::impl_default_via_new!(HardwareInputManager);
/// Noise filter smoothing a raw touch stream before it reaches a handler.
///
/// Two independent stages, applied in this order: a pressure cut-off that drops
/// light contacts entirely, then a dead-zone plus low-pass blend on position. It
/// is stateful — the blend needs the previously *emitted* position, so the filter
/// must see a contact's events in order, and one instance should not be shared
/// across unrelated contacts.
///
/// # The dead zone is one-sided
///
/// The dead zone is tested as `|dx| < dead_zone && |dy| < dead_zone`, so a
/// jitter that exceeds it in either axis emits a point. On the move that first
/// passes the test the emitted point is blended toward the last emitted position
/// — the caller does not get the raw input — and it is the *blended* value that
/// is stored as the new reference. A steady stream of sub-dead-zone jitter is
/// therefore suppressed only while each step stays under the threshold from the
/// last emitted position, and the reference does not drift toward the raw input
/// while events are being dropped.
pub struct InputFilter {
    min_pressure: f32,
    max_pressure: f32,
    dead_zone: i32,
    smoothing_factor: f32,
    last_position: Option<Point>,
}
impl InputFilter {
    /// Creates a filter with the standard embedded settings.
    ///
    /// * `min_pressure` — `0.1`: anything lighter is dropped.
    /// * `max_pressure` — `1.0`: the ceiling reported points are clamped to.
    /// * `dead_zone` — `5` logical pixels, per axis.
    /// * `smoothing_factor` — `0.5`: each emitted move closes half the remaining
    ///   gap to the raw point.
    ///
    /// No position is remembered, so the first point to survive the pressure
    /// check is passed through unblended.
    pub fn new() -> Self {
        Self {
            min_pressure: 0.1,
            max_pressure: 1.0,
            dead_zone: 5,
            smoothing_factor: 0.5,
            last_position: None,
        }
    }
    /// Sets the per-axis dead zone.
    ///
    /// `zone` is in logical pixels. The value is used as given: a negative zone
    /// makes the `dx.abs() < zone` test false for every move, i.e. no filtering.
    pub fn with_dead_zone(mut self, zone: i32) -> Self {
        self.dead_zone = zone;
        self
    }
    /// Offers one raw point to the filter, returning what should be passed on.
    ///
    /// Returns `None` — dropping the point — when `point.pressure` is below the
    /// minimum, or when the point sits inside the dead zone on both axes. Those
    /// two cases are not distinguished by the return value, so a caller cannot
    /// tell a dropped light touch from suppressed jitter.
    ///
    /// On a point that passes, the returned `TouchPoint` has the *blended*
    /// position, the incoming pressure clamped to `max_pressure`, and the id and
    /// size copied through unchanged. The blended position becomes the reference
    /// for the next call. A point inside the dead zone does **not** update that
    /// reference, so repeated jitter cannot slowly drag it.
    pub fn filter_touch(&mut self, point: &TouchPoint) -> Option<TouchPoint> {
        if point.pressure < self.min_pressure {
            return None;
        }
        let filtered_position = if let Some(last) = self.last_position {
            let dx = point.position.x - last.x;
            let dy = point.position.y - last.y;
            if dx.abs() < self.dead_zone && dy.abs() < self.dead_zone {
                return None;
            }
            Point::new(
                last.x + (dx as f32 * self.smoothing_factor) as i32,
                last.y + (dy as f32 * self.smoothing_factor) as i32,
            )
        } else {
            point.position
        };
        self.last_position = Some(filtered_position);
        Some(TouchPoint {
            id: point.id,
            position: filtered_position,
            pressure: point.pressure.min(self.max_pressure),
            size: point.size,
        })
    }
    /// Forgets the reference position, so the next accepted point is passed
    /// through unblended.
    ///
    /// Call this when a contact lifts and a new one begins; otherwise the new
    /// contact's first move is blended toward the old contact's last position.
    /// The thresholds are untouched.
    pub fn reset(&mut self) {
        self.last_position = None;
    }
}
crate::impl_default_via_new!(InputFilter);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_touch_point() {
        let point = TouchPoint::new(1, 100, 200).with_pressure(0.8);
        assert_eq!(point.id, 1);
        assert_eq!(point.position.x, 100);
        assert_eq!(point.position.y, 200);
        assert!((point.pressure - 0.8).abs() < 0.01);
    }
    #[test]
    fn test_hardware_input_manager() {
        let mut manager = HardwareInputManager::new();
        let point = TouchPoint::new(1, 100, 100);
        manager.process_touch(TouchEvent::Down, point);
        assert_eq!(manager.touch_point_count(), 1);
        manager.process_button(0, true);
        assert!(manager.is_button_pressed(0));
        manager.process_button(0, false);
        assert!(!manager.is_button_pressed(0));
    }
    #[test]
    fn test_input_filter() {
        let mut filter = InputFilter::new().with_dead_zone(10);
        let point1 = TouchPoint::new(1, 100, 100);
        let result1 = filter.filter_touch(&point1);
        assert!(result1.is_some());
        let point2 = TouchPoint::new(1, 105, 105);
        let result2 = filter.filter_touch(&point2);
        assert!(result2.is_none());
        let point3 = TouchPoint::new(1, 120, 120);
        let result3 = filter.filter_touch(&point3);
        assert!(result3.is_some());
    }
}
