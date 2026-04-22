use crate::core::Point;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    Touch,
    Mouse,
    Keyboard,
    HardwareButton,
    Gesture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchEvent {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    pub id: u32,
    pub position: Point,
    pub pressure: f32,
    pub size: f32,
}

impl TouchPoint {
    pub fn new(id: u32, x: i32, y: i32) -> Self {
        Self {
            id,
            position: Point::new(x, y),
            pressure: 1.0,
            size: 1.0,
        }
    }

    pub fn with_pressure(mut self, pressure: f32) -> Self {
        self.pressure = pressure.clamp(0.0, 1.0);
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HardwareButtonEvent {
    pub button_id: u32,
    pub pressed: bool,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Copy)]
pub struct GestureEvent {
    pub gesture_type: GestureType,
    pub center: Point,
    pub scale: f32,
    pub rotation: f32,
    pub velocity: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureType {
    Tap,
    DoubleTap,
    LongPress,
    Pan,
    Pinch,
    Rotate,
    SwipeLeft,
    SwipeRight,
    SwipeUp,
    SwipeDown,
}

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
                        dx as f32 / duration.as_secs_f32(),
                        dy as f32 / duration.as_secs_f32(),
                    ),
                });
            }
        }
    }

    pub fn process_button(&mut self, button_id: u32, pressed: bool) {
        if button_id < 32 {
            self.button_states[button_id as usize] = pressed;
        }
    }

    pub fn is_button_pressed(&self, button_id: u32) -> bool {
        if button_id < 32 {
            self.button_states[button_id as usize]
        } else {
            false
        }
    }

    pub fn get_gesture(&mut self) -> Option<GestureEvent> {
        self.gesture_buffer.pop_front()
    }

    pub fn touch_point_count(&self) -> usize {
        self.touch_points.len()
    }

    pub fn get_touch_points(&self) -> &[TouchPoint] {
        &self.touch_points
    }

    pub fn clear(&mut self) {
        self.touch_points.clear();
        self.button_states = [false; 32];
        self.gesture_buffer.clear();
        self.touch_start_time = None;
        self.touch_start_position = None;
        self.last_touch_position = None;
    }
}

impl Default for HardwareInputManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct InputFilter {
    min_pressure: f32,
    max_pressure: f32,
    dead_zone: i32,
    smoothing_factor: f32,
    last_position: Option<Point>,
}

impl InputFilter {
    pub fn new() -> Self {
        Self {
            min_pressure: 0.1,
            max_pressure: 1.0,
            dead_zone: 5,
            smoothing_factor: 0.5,
            last_position: None,
        }
    }

    pub fn with_dead_zone(mut self, zone: i32) -> Self {
        self.dead_zone = zone;
        self
    }

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

    pub fn reset(&mut self) {
        self.last_position = None;
    }
}

impl Default for InputFilter {
    fn default() -> Self {
        Self::new()
    }
}

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
