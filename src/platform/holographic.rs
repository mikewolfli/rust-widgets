//! Laser holographic keyboard interaction (BLUE8 P4-5a, experimental).
//!
//! This module provides a state machine that simulates a laser-projected
//! keyboard on a flat surface. It uses Z-axis depth information from
//! [`Event::HolographicTouch`] (gated behind the `holographic` feature)
//! to detect finger presses, releases, and key hits.
//!
//! # Architecture
//!
//! - [`HolographicKeyboardDetector`] — Manages a virtual keyboard layout
//!   and processes depth events to emit synthetic key-press events.
//! - [`KeyboardLayout`] — Defines key positions on the projection plane.
//! - [`KeyHit`] — Represents a detected key press with confidence.
//!
//! # State machine
//!
//! ```text
//!         ┌──────────┐
//!         │  Idle    │ ◄────────────┐
//!         └────┬─────┘              │
//!              │ depth < threshold  │
//!              ▼                    │
//!         ┌──────────┐   depth >   │
//!         │ Pressed  │ ──► reset ──┘
//!         └────┬─────┘
//!              │ depth > release_threshold
//!              ▼
//!         ┌──────────┐
//!         │ Released │ ──► emit KeyHit ──► Idle
//!         └──────────┘
//! ```
//!
//! # Integration
//!
//! Call `HolographicKeyboardDetector::process_depth()` for each
//! `HolographicTouch` event. When a key is detected, the detector
//! returns a `KeyHit` with the character code and confidence level.
//! The platform layer forwards this as a synthesized keyboard event.

use crate::core::{Point, Rect};

/// Confidence level of a holographic key detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitConfidence {
    /// Low confidence — may be a false positive.
    Low,
    /// Medium confidence — likely intentional.
    Medium,
    /// High confidence — very likely a deliberate key press.
    High,
}

/// A detected key press from the holographic keyboard.
#[derive(Debug, Clone)]
pub struct KeyHit {
    /// Character that was detected (e.g. 'a', '1', etc.).
    pub character: char,
    /// Normalized confidence (0.0 = none, 1.0 = certain).
    pub confidence: f32,
    /// Confidence bucket for quick filtering.
    pub level: HitConfidence,
    /// Position on the projection plane where the hit was detected.
    pub position: Point,
}

/// A single key definition on the holographic keyboard layout.
#[derive(Debug, Clone)]
pub struct HolographicKey {
    /// Character produced when this key is pressed.
    pub character: char,
    /// Bounding rectangle on the projection plane (mm or normalized coords).
    pub bounds: Rect,
    /// Optional shift/alternate character (e.g. 'A' for 'a').
    pub shift_character: Option<char>,
}

impl HolographicKey {
    /// Create a new key definition.
    pub const fn new(character: char, bounds: Rect) -> Self {
        Self {
            character,
            bounds,
            shift_character: None,
        }
    }

    /// Set an alternate character for shift mode.
    pub fn with_shift(mut self, ch: char) -> Self {
        self.shift_character = Some(ch);
        self
    }
}

/// Standard QWERTY keyboard layout for holographic projection.
///
/// Key positions are in normalized coordinates (0..1) relative to
/// the projection surface, with row heights proportional to finger reach.
#[derive(Debug)]
pub struct KeyboardLayout {
    /// All keys in the layout.
    pub keys: Vec<HolographicKey>,
    /// Width of the projection surface (in millimetres or logical units).
    pub surface_width: f32,
    /// Height of the projection surface.
    pub surface_height: f32,
    /// Whether shift is currently active.
    pub shift_active: bool,
}

impl Default for KeyboardLayout {
    fn default() -> Self {
        Self::qwerty()
    }
}

impl KeyboardLayout {
    /// Create a standard QWERTY layout.
    ///
    /// The layout uses three rows with standard key spacing, scaled
    /// to fill the given surface dimensions (default 300×120 mm).
    pub fn qwerty() -> Self {
        let surface_width = 300.0;
        let surface_height = 120.0;
        let cols = 10;
        let rows = 3;
        let key_w = surface_width / cols as f32;
        let key_h = surface_height / rows as f32;

        let row1_chars: Vec<char> = "QWERTYUIOP".chars().collect();
        let row2_chars: Vec<char> = "ASDFGHJKL".chars().collect();
        let row3_chars: Vec<char> = "ZXCVBNM".chars().collect();

        let mut keys = Vec::with_capacity(28);

        // Row 1 (top row, offset for number row)
        for (i, &ch) in row1_chars.iter().enumerate() {
            let x = i as f32 * key_w;
            let y = 0.0;
            let lower = ch.to_ascii_lowercase();
            keys.push(
                HolographicKey::new(
                    lower,
                    Rect::new(x as i32, y as i32, key_w as u32, key_h as u32),
                )
                .with_shift(ch),
            );
        }

        // Row 2 (offset by half key for ergonomic stagger)
        let stagger = key_w * 0.25;
        for (i, &ch) in row2_chars.iter().enumerate() {
            let x = stagger + i as f32 * key_w;
            let y = key_h;
            let lower = ch.to_ascii_lowercase();
            keys.push(
                HolographicKey::new(
                    lower,
                    Rect::new(x as i32, y as i32, key_w as u32, key_h as u32),
                )
                .with_shift(ch),
            );
        }

        // Row 3
        for (i, &ch) in row3_chars.iter().enumerate() {
            let x = key_w * 1.5 + i as f32 * key_w;
            let y = key_h * 2.0;
            let lower = ch.to_ascii_lowercase();
            keys.push(
                HolographicKey::new(
                    lower,
                    Rect::new(x as i32, y as i32, key_w as u32, key_h as u32),
                )
                .with_shift(ch),
            );
        }

        // Space bar (bottom centre)
        let space_y = key_h * 2.0;
        let space_x = key_w * 2.0;
        let space_w = key_w * 6.0;
        keys.push(HolographicKey::new(
            ' ',
            Rect::new(space_x as i32, space_y as i32, space_w as u32, key_h as u32),
        ));

        // Backspace key
        let bs_x = key_w * 9.0;
        keys.push(HolographicKey::new(
            '\u{0008}', // Backspace
            Rect::new(bs_x as i32, 0, key_w as u32, key_h as u32),
        ));

        Self {
            keys,
            surface_width,
            surface_height,
            shift_active: false,
        }
    }

    /// Find the key at a given position on the projection surface.
    pub fn key_at(&self, pos: Point) -> Option<&HolographicKey> {
        self.keys.iter().find(|k| k.bounds.contains_point(pos))
    }

    /// Get the character for a key, respecting shift state.
    pub fn character_for(&self, key: &HolographicKey) -> char {
        if self.shift_active {
            key.shift_character.unwrap_or(key.character)
        } else {
            key.character
        }
    }

    /// Toggle shift state.
    pub fn toggle_shift(&mut self) {
        self.shift_active = !self.shift_active;
    }
}

/// State of the holographic keyboard finger tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FingerState {
    /// No finger detected on the projection plane.
    Idle,
    /// Finger detected at depth (possible press in progress).
    Approaching,
    /// Finger is in contact with the projection plane (key pressed).
    Pressed,
    /// Finger is lifting off (about to release).
    Releasing,
}

/// Detects key presses from holographic (Z-axis depth) touch events.
///
/// Processes `HolographicTouch` events with depth information and
/// matches them against a virtual keyboard layout to produce `KeyHit`
/// events that can be forwarded as synthetic keyboard input.
#[derive(Debug)]
pub struct HolographicKeyboardDetector {
    /// Current finger tracking state.
    state: FingerState,
    /// Virtual keyboard layout.
    layout: KeyboardLayout,
    /// Depth threshold for registering a press (lower = closer to surface).
    press_threshold: f32,
    /// Depth threshold for registering a release (higher = farther).
    release_threshold: f32,
    /// Position where the finger was last detected.
    last_position: Option<Point>,
    /// Position where the initial press was detected.
    press_position: Option<Point>,
    /// The key that was pressed (if any).
    pressed_key: Option<char>,
    /// Minimum movement (mm) to cancel a press (finger drag).
    drag_cancel_distance: f32,
}

impl Default for HolographicKeyboardDetector {
    fn default() -> Self {
        Self {
            state: FingerState::Idle,
            layout: KeyboardLayout::default(),
            press_threshold: 5.0,    // 5 mm from surface
            release_threshold: 15.0, // 15 mm from surface
            last_position: None,
            press_position: None,
            pressed_key: None,
            drag_cancel_distance: 8.0, // 8 mm drag cancels press
        }
    }
}

impl HolographicKeyboardDetector {
    /// Create a new holographic keyboard detector with default QWERTY layout.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a detector with a custom keyboard layout.
    pub fn with_layout(layout: KeyboardLayout) -> Self {
        Self {
            layout,
            ..Default::default()
        }
    }

    /// Configure the press depth threshold (closer = smaller value).
    pub fn with_press_threshold(mut self, threshold: f32) -> Self {
        self.press_threshold = threshold;
        self
    }

    /// Configure the release depth threshold.
    pub fn with_release_threshold(mut self, threshold: f32) -> Self {
        self.release_threshold = threshold;
        self
    }

    /// Process a holographic touch event and return a key hit if detected.
    ///
    /// `depth` is the Z-axis distance from the sensor (lower = closer
    /// to the projection surface). `pos` is the XY position on the
    /// projection plane.
    ///
    /// Returns `Some(KeyHit)` when a key press is completed (finger
    /// touched the surface and lifted off).
    pub fn process_depth(&mut self, pos: Point, depth: f32) -> Option<KeyHit> {
        self.last_position = Some(pos);

        match self.state {
            FingerState::Idle => {
                if depth < self.press_threshold {
                    // Finger is approaching the surface.
                    self.state = FingerState::Approaching;
                    self.press_position = Some(pos);
                }
                None
            }
            FingerState::Approaching => {
                if depth < self.press_threshold * 0.5 {
                    // Finger made contact — register the initial touch.
                    self.state = FingerState::Pressed;
                    // Find which key is under the finger.
                    self.pressed_key = self
                        .layout
                        .key_at(pos)
                        .map(|k| self.layout.character_for(k));
                    None
                } else if depth > self.release_threshold {
                    // Finger moved away before contact — cancel.
                    self.reset();
                    None
                } else {
                    None
                }
            }
            FingerState::Pressed => {
                // Check for drag-cancel: finger moved too far.
                if let Some(press_pos) = self.press_position {
                    let dx = (pos.x - press_pos.x) as f32;
                    let dy = (pos.y - press_pos.y) as f32;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > self.drag_cancel_distance {
                        // Finger dragged away — cancel the press.
                        self.reset();
                        return None;
                    }
                }

                if depth > self.release_threshold {
                    // Finger lifted off — emit the key hit.
                    self.state = FingerState::Releasing;
                    // Will transition to Idle on next call or here.
                    let hit = self.pressed_key.map(|ch| {
                        let confidence = Self::compute_confidence(depth, self.release_threshold);
                        KeyHit {
                            character: ch,
                            confidence,
                            level: if confidence > 0.8 {
                                HitConfidence::High
                            } else if confidence > 0.5 {
                                HitConfidence::Medium
                            } else {
                                HitConfidence::Low
                            },
                            position: pos,
                        }
                    });
                    self.reset();
                    return hit;
                }
                None
            }
            FingerState::Releasing => {
                // Transition back to idle.
                self.reset();
                None
            }
        }
    }

    /// Process absence of a finger (no event received) to force timeout.
    ///
    /// Call this when no `HolographicTouch` event has been received for
    /// a while to reset the detector state.
    pub fn process_timeout(&mut self) {
        self.reset();
    }

    /// Reset the detector to idle state.
    pub fn reset(&mut self) {
        self.state = FingerState::Idle;
        self.last_position = None;
        self.press_position = None;
        self.pressed_key = None;
    }

    /// Returns a mutable reference to the keyboard layout.
    pub fn layout_mut(&mut self) -> &mut KeyboardLayout {
        &mut self.layout
    }

    /// Returns the current finger tracking state.
    pub(crate) fn finger_state(&self) -> FingerState {
        self.state
    }

    /// Compute a confidence score from the release depth.
    fn compute_confidence(depth: f32, release_threshold: f32) -> f32 {
        if depth <= 0.0 {
            return 0.0;
        }
        let ratio = depth / release_threshold;
        // Higher depth = higher confidence (finger clearly lifted).
        (ratio.min(2.0) / 2.0).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_has_expected_key_count() {
        let layout = KeyboardLayout::qwerty();
        // 10 + 9 + 7 + space + backspace = 28
        assert_eq!(layout.keys.len(), 28);
    }

    #[test]
    fn key_at_returns_correct_character() {
        let layout = KeyboardLayout::qwerty();
        // First key in row 1 is 'q' at approximately (0, 0)
        let key = layout.key_at(Point::new(5, 5)).unwrap();
        assert_eq!(key.character, 'q');
    }

    #[test]
    fn detector_starts_idle() {
        let detector = HolographicKeyboardDetector::new();
        assert_eq!(detector.state, FingerState::Idle);
    }

    #[test]
    fn approaching_depth_transitions_to_approaching() {
        let mut detector = HolographicKeyboardDetector::new();
        let result = detector.process_depth(Point::new(5, 5), 4.0);
        assert_eq!(detector.state, FingerState::Approaching);
        assert!(result.is_none());
    }

    #[test]
    fn press_and_release_produces_key_hit() {
        let mut detector = HolographicKeyboardDetector::new();
        // Approach
        let _ = detector.process_depth(Point::new(5, 5), 4.0);
        // Touch (depth < press_threshold * 0.5 = 2.5)
        let _ = detector.process_depth(Point::new(5, 5), 2.0);
        assert_eq!(detector.state, FingerState::Pressed);
        assert_eq!(detector.pressed_key, Some('q'));

        // Release (depth > release_threshold = 15.0)
        let hit = detector.process_depth(Point::new(5, 5), 20.0);
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.character, 'q');
        assert_eq!(detector.state, FingerState::Idle);
    }

    #[test]
    fn drag_during_press_cancels_hit() {
        let mut detector = HolographicKeyboardDetector::new();
        // Approach
        let _ = detector.process_depth(Point::new(5, 5), 4.0);
        // Press
        let _ = detector.process_depth(Point::new(5, 5), 2.0);
        assert_eq!(detector.pressed_key, Some('q'));

        // Drag far away (10 px > drag_cancel_distance = 8)
        let result = detector.process_depth(Point::new(50, 50), 2.0);
        assert!(result.is_none());
        assert_eq!(detector.state, FingerState::Idle);
    }

    #[test]
    fn timeout_resets_detector() {
        let mut detector = HolographicKeyboardDetector::new();
        // Approach first
        let _ = detector.process_depth(Point::new(5, 5), 4.0);
        assert_eq!(detector.state, FingerState::Approaching);
        // Then press (depth < press_threshold * 0.5 = 2.5)
        let _ = detector.process_depth(Point::new(5, 5), 1.0);
        assert_eq!(detector.state, FingerState::Pressed);
        detector.process_timeout();
        assert_eq!(detector.state, FingerState::Idle);
    }

    #[test]
    fn shift_toggle_works() {
        let mut layout = KeyboardLayout::qwerty();
        assert!(!layout.shift_active);
        layout.toggle_shift();
        assert!(layout.shift_active);
        // 'q' with shift should be 'Q'
        let q_char = layout.character_for(layout.key_at(Point::new(5, 5)).unwrap());
        assert_eq!(q_char, 'Q');
        layout.toggle_shift();
        assert!(!layout.shift_active);
        let q_char = layout.character_for(layout.key_at(Point::new(5, 5)).unwrap());
        assert_eq!(q_char, 'q');
    }

    #[test]
    fn confidence_scales_with_depth() {
        let c1 = HolographicKeyboardDetector::compute_confidence(15.0, 15.0);
        let c2 = HolographicKeyboardDetector::compute_confidence(30.0, 15.0);
        // c1 should be ~0.5, c2 should be 1.0 (clamped)
        assert!((c1 - 0.5).abs() < 0.01);
        assert!((c2 - 1.0).abs() < 0.01);
    }
}
