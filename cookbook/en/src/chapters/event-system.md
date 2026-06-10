# Event System

The `rust-widgets` event system provides a comprehensive tiered communication pipeline: input events from the platform pump, an `mpsc`-based queue for decoupled posting, a background event loop for dispatch, focus/pointer/timer management, and touch-to-mouse event translation. With **54 event variants**, the system covers mouse, keyboard, touch, gesture, paint, timers, and gamepad input.

---

## Core Architecture

```
Platform Pump → EventLoop (bg thread) → EventQueue (mpsc) → EventHandler::handle_event()
                    ↑                            ↑
               TimerManager              EventSender (cloneable)
                                          FocusManager
                                          PointerCaptureManager
```

---

## The `Event` Enum — 54 Variants

```rust
pub enum Event {
    // Mouse
    MouseDown,                              MouseUp,
    MouseMove { pos: Point },               MousePress { pos: Point, button: u32 },
    MouseRelease { pos: Point, button: u32 }, MouseDoubleClick { pos: Point, button: u32 },
    MouseEnter { pos: Point },              MouseLeave { pos: Point },
    Wheel { delta: (f32, f32), modifiers: u32 },

    // Keyboard
    KeyDown, KeyUp,
    KeyPress { key: u32, modifiers: u32 },  KeyRelease { key: u32, modifiers: u32 },

    // Focus
    FocusGained, FocusLost,

    // Paint / Layout
    Paint,  Resize { size: Size },

    // Timer
    Timer { id: u32 },

    // Touch (8 variants)
    TouchBegin { pos: Point, touch_id: TouchId },
    TouchEnd   { pos: Point, touch_id: TouchId },
    TouchMove  { pos: Point, touch_id: TouchId },

    // Gestures (9 variants)
    Tap { pos: Point },                     DoubleTap { pos: Point },
    LongPress { pos: Point },               Swipe { start: Point, end: Point, velocity: f32 },
    Pinch { scale: f32 },                   Rotate { angle: f32 },
    Drag { pos: Point, touch_id: TouchId, delta: (f32, f32) },
    TwoFingerTap { pos: Point },            TwoFingerSwipe { centroid_start, centroid_end, velocity },
    Fling { pos: Point, velocity: f32, touch_id: TouchId },

    // Holographic (XR/3D)
    HolographicTouch { pos: Point, depth: f32, touch_id: TouchId },

    // Pointer (stylus with pressure/tilt)
    PointerPress  { pos: Point, button: u32, pressure: f32, tilt_x: f32, tilt_y: f32 },
    PointerMove   { pos: Point, pressure: f32, tilt_x: f32, tilt_y: f32 },
    PointerRelease { pos: Point, button: u32, pressure: f32 },

    // Gamepad (4 variants)
    GamepadPress { button: u32 },           GamepadRelease { button: u32 },
    GamepadAxis { axis: u32, value: f32 },  GamepadConnected { id: u32 },
    GamepadDisconnected { id: u32 },

    // Orientation & Lifecycle
    OrientationChanged { orientation: ScreenOrientation },
    Custom { name: String, payload: Box<dyn std::any::Any> },
    Quit,
}
```

**Helper constructors** make event creation concise:

```rust
let press = Event::mouse_press(Point::new(50, 50), 0);        // button 0 (left)
let key   = Event::key_press(65, 0);                           // key 'A', no modifiers
let tap   = Event::tap(Point::new(100, 200));
let timer_event = Event::timer(42);                            // timer ID 42
let pinch = Event::pinch(1.5);                                 // 150% zoom
```

**Gesture classification:** `Event::gesture_class()` returns `Some(GestureClass::Single)`, `Some(GestureClass::Multi)`, or `None` for non-gesture events.

**Touch detection:** `Event::is_touch()` returns `true` for all touch and gesture event variants, `false` for mouse/keyboard events.

---

## `EventHandler` Trait

Every widget implements the `EventHandler` trait to receive events:

```rust
pub trait EventHandler {
    fn handle_event(&mut self, event: &Event) -> bool;
}
```

Return `true` if the event was consumed (stops propagation), `false` to pass it to the next handler.

```rust
impl EventHandler for MyButton {
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MousePress { pos, .. } if self.bounds.contains(*pos) => {
                self.pressed = true;
                self.request_repaint();
                true  // consumed
            }
            Event::MouseRelease { pos, .. } if self.pressed => {
                self.pressed = false;
                self.on_click();
                true
            }
            _ => false,  // not handled — propagate
        }
    }
}
```

---

## EventQueue — mpsc-Based Post/Drain

The `EventQueue` wraps `std::sync::mpsc` channels for unbounded event posting and draining:

```rust
let queue = EventQueue::new();
let sender: EventSender = queue.sender();

// Post events from any thread
sender.post(widget_id, Event::Paint)?;
sender.post(widget_id, Event::mouse_press(Point::new(10, 20), 0))?;
sender.post_with_priority(widget_id, Event::resize(Size::new(800, 600)), EventPriority::High)?;
sender.post_idle(widget_id, Event::Paint)?;

// Drain on the main thread
while let Some((target, event, priority)) = queue.dequeue() {
    dispatch_event(target, &event, priority);
}

// Blocking drain (for background threads)
while let Some((target, event, priority)) = queue.dequeue_blocking() {
    dispatch_event(target, &event, priority);
}
```

---

## `EventSender` — Cloneable Post Handle

The `EventSender` is lightweight and cloneable, designed to be shared across threads:

```rust
let sender1: EventSender = queue.sender();
let sender2 = sender1.clone();

// Send from multiple threads
std::thread::spawn(move || {
    sender1.post(network_widget, Event::Custom {
        name: "data-arrived".into(),
        payload: Box::new(payload_bytes),
    }).unwrap();
});

std::thread::spawn(move || {
    sender2.post(timer_widget, Event::Timer { id: 1 }).unwrap();
});
```

**Priority-based posting:**

| Method | Priority | Use Case |
|---|---|---|
| `sender.post(id, event)` | `EventPriority::Normal` | Default: user input, paint requests |
| `sender.post_with_priority(id, event, priority)` | Custom | Explicit priority control |
| `sender.post_idle(id, event)` | `EventPriority::Idle` | Low-priority background tasks |

---

## `EventPriority`

```rust
pub enum EventPriority {
    High,    // Process immediately: resize, quit, orientation change
    Normal,  // Standard input: mouse, keyboard, touch
    Idle,    // Process when idle: background updates, pre-rendering
}
```

The event loop drains in priority order: all `High` events first, then `Normal`, then `Idle`.

---

## EventLoop — Background Thread Pump

`EventLoop` runs a dedicated background thread that drives the event system:

```rust
let mut event_loop = EventLoop::new();

// Set the dispatch function
event_loop.set_dispatch_fn(|target_id, event, priority| {
    // Route to the appropriate widget
    if let Some(widget) = widget_registry.get_mut(target_id) {
        widget.handle_event(&event);
    }
});

// Start the event loop on a background thread
event_loop.start();

// Post events from anywhere
event_loop.post_event(widget_id, Event::Paint);

// Request animation frame callbacks
let frame_id = event_loop.request_animation_frame(|_timestamp_ms| {
    // Update animation state and request repaint
});

// Timer integration
let timer_id = event_loop.start_timer(widget_id, Duration::from_millis(16));
event_loop.stop_timer(timer_id);

// Stop timers for a specific target
event_loop.stop_timers_for_target(widget_id);

// Stop the event loop
event_loop.stop();
```

**Platform event pump integration:**

```rust
// Set a native platform event pump callback
event_loop.set_native_pump(|| {
    // Called when the event queue is empty — poll the platform for new events
    platform.poll_events()
});
```

The native pump is invoked when the internal queue is drained, allowing the event loop to pull platform events without blocking.

---

## FocusManager — Keyboard Focus & Tab Order

Manages keyboard focus state and tab-order traversal:

```rust
use rust_widgets::event::FocusManager;

let mut focus = FocusManager::new();

// Request focus for a widget
focus.request_focus(button_id);

// Move focus forward
focus.focus_next();

// Move focus backward
focus.focus_previous();

// Check if a widget has focus
if focus.has_focus(button_id) {
    // Draw focus ring
}

// Get the currently focused widget
if let Some(focused_id) = focus.current_focus() {
    // Handle keyboard events for focused widget
}

// Clear focus
focus.clear_focus();
```

When focus changes, `Event::FocusGained` and `Event::FocusLost` are posted to the respective widgets.

---

## PointerCaptureManager — Drag Operations

Manages pointer capture during drag interactions:

```rust
use rust_widgets::event::PointerCaptureManager;

let mut capture = PointerCaptureManager::new();

// Start a drag operation — capture the pointer
capture.capture(draggable_widget);

// All subsequent mouse/touch events are routed to the capturing widget
if capture.is_captured() {
    let captured_widget = capture.captured_widget().unwrap();
    // Forward events to captured_widget
}

// Release the capture on mouse up
capture.release();
```

This ensures smooth drag operations: once a drag starts on a widget, that widget receives all `MouseMove` events even if the cursor leaves its bounds.

---

## TimerManager — One-Shot & Repeating Timers

Manages periodic and delayed event dispatch:

```rust
use rust_widgets::event::TimerManager;

let mut timers = TimerManager::new();

// One-shot timer: fires once after 500ms
let id = timers.start(widget_id, Duration::from_millis(500), false);

// Repeating timer: fires every 16ms (~60 FPS)
let anim_id = timers.start(widget_id, Duration::from_millis(16), true);

// Stop a specific timer
timers.stop(id);

// Cancel all timers for a widget when it's destroyed
timers.cancel_all_for(widget_id);

// Poll timers each frame
let expired = timers.poll();  // returns Vec<(ObjectId, u32)> of expired timer IDs
```

The timer manager integrates with `EventLoop` via `start_timer`/`stop_timer` methods.

---

## Touch-to-Mouse Event Translation

The `translator` module (gated behind `feature = "touch"`) converts touch events to synthetic mouse events for widgets that only implement mouse handling:

```rust
#[cfg(feature = "touch")]
use rust_widgets::event::translator::TouchTranslator;

let translator = TouchTranslator::new();

// Feed touch events; the translator emits synthetic mouse events
let events = translator.translate_touch_event(&touch_event);

for synthetic_event in events {
    widget.handle_event(&synthetic_event);
}
```

| Touch Event | Synthetic Mouse Event |
|---|---|
| `TouchBegin` | `MousePress` → `MouseEnter` |
| `TouchMove` | `MouseMove` |
| `TouchEnd` | `MouseRelease` → `MouseLeave` |
| `Tap` | `MousePress` + `MouseRelease` (click) |

---

## Async Task Scheduling

The event system supports scheduling async work via `AsyncTask`:

```rust
use rust_widgets::event::{schedule_task, drain_tasks, AsyncTask};

// Schedule a background computation
schedule_task(AsyncTask::new(widget_id, Box::new(|| {
    // Heavy work
    let result = expensive_computation();
    // Result is posted back as a Custom event
})));

// Drain completed tasks each frame
drain_tasks();  // posts Custom events for completed tasks
```

---

## Generic Queue Primitives

The `queue` module provides foundational data structures:

```rust
use rust_widgets::event::queue::{FixedSizeQueue, QueueError, DEFAULT_QUEUE_CAPACITY};

let mut queue = FixedSizeQueue::<Event>::with_capacity(DEFAULT_QUEUE_CAPACITY);

queue.push(Event::Paint).map_err(|QueueError::Full| {
    eprintln!("Event queue overflow!");
})?;

while let Some(event) = queue.pop() {
    process(event);
}
```

These primitives are used internally by `EventQueue` and can be reused for custom event buffering.

---

## Common Patterns

### Button with All Mouse States

```rust
impl EventHandler for InteractiveButton {
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseEnter { .. } => {
                self.state = WidgetState::Hover;
                self.request_repaint();
                true
            }
            Event::MouseLeave { .. } => {
                self.state = WidgetState::Normal;
                self.request_repaint();
                true
            }
            Event::MousePress { pos, button: 0 } if self.bounds.contains(*pos) => {
                self.state = WidgetState::Pressed;
                self.request_repaint();
                true
            }
            Event::MouseRelease { pos, button: 0 } => {
                if self.state == WidgetState::Pressed && self.bounds.contains(*pos) {
                    self.state = WidgetState::Hover;
                    self.on_click();  // fire the click action
                }
                true
            }
            _ => false,
        }
    }
}
```

### Keyboard-Focusable Input Field

```rust
impl EventHandler for TextField {
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::FocusGained => {
                self.focused = true;
                self.show_cursor = true;
                true
            }
            Event::FocusLost => {
                self.focused = false;
                self.show_cursor = false;
                true
            }
            Event::KeyPress { key, modifiers } if self.focused => {
                if *key == 8 {  // Backspace
                    self.text.pop();
                } else if let Some(c) = char::from_u32(*key) {
                    self.text.push(c);
                }
                true
            }
            Event::MousePress { pos, .. } => {
                // Request focus when clicked
                focus_manager.request_focus(self.id);
                // Move cursor to click position
                self.set_cursor_from_point(*pos);
                true
            }
            _ => false,
        }
    }
}
```

### Drag-and-Drop with Pointer Capture

```rust
impl EventHandler for DraggableItem {
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MousePress { pos, .. } if self.bounds.contains(*pos) => {
                self.dragging = true;
                self.drag_offset = Point::new(pos.x - self.rect.x, pos.y - self.rect.y);
                capture_manager.capture(self.id);
                true
            }
            Event::MouseMove { pos } if self.dragging => {
                self.rect.x = pos.x - self.drag_offset.x;
                self.rect.y = pos.y - self.drag_offset.y;
                self.request_repaint();
                true
            }
            Event::MouseRelease { .. } if self.dragging => {
                self.dragging = false;
                capture_manager.release();
                // Check for drop target
                self.check_drop();
                true
            }
            _ => false,
        }
    }
}
```

### Frame-Loop Animation with Timer

```rust
// Start a repeating 16ms timer (~60 FPS)
let timer_id = event_loop.start_timer(animation_widget, Duration::from_millis(16));

// In the widget's event handler:
impl EventHandler for AnimatedWidget {
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Timer { id } if *id == ANIM_TIMER_ID => {
                self.animation_progress += 0.016;  // advance by ~1 frame
                if self.animation_progress >= 1.0 {
                    self.animation_progress = 0.0;
                }
                self.request_repaint();
                true
            }
            _ => false,
        }
    }
}
```

### Platform Integration: Custom Event Loop

```rust
fn main() {
    let mut event_loop = EventLoop::new();

    event_loop.set_dispatch_fn(|target_id, event, priority| {
        app.handle_event(target_id, &event, priority);
    });

    // Integrate native platform events
    event_loop.set_native_pump(|| {
        while let Some(platform_event) = native_window.poll_event() {
            let rust_event = convert_platform_event(platform_event);
            event_loop.post_event(platform_event.target, rust_event);
        }
    });

    event_loop.start();

    // Main thread can still post events
    event_loop.post_event(root_widget, Event::Paint);

    // Run until quit
    loop {
        std::thread::sleep(Duration::from_millis(1));
        if app.should_quit() {
            break;
        }
    }

    event_loop.stop();
}
```
