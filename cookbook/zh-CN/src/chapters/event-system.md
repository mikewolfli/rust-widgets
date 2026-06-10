# 事件系统 (Event System)

`rust-widgets` 事件系统提供了一套全面的分层通信管道：来自平台泵的输入事件、基于 `mpsc` 的解耦投递队列、用于调度的后台事件循环、焦点/指针/定时器管理，以及触控到鼠标的事件转换。该系统拥有 **54 种事件变体**，涵盖鼠标、键盘、触控、手势、绘图、定时器和游戏手柄输入。

---

## 核心架构

```
Platform Pump → EventLoop (bg thread) → EventQueue (mpsc) → EventHandler::handle_event()
                    ↑                            ↑
               TimerManager              EventSender (可克隆)
                                          FocusManager
                                          PointerCaptureManager
```

---

## `Event` 枚举 — 54 种变体

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

**辅助构造函数**让事件创建更加简洁：

```rust
let press = Event::mouse_press(Point::new(50, 50), 0);        // button 0 (left)
let key   = Event::key_press(65, 0);                           // key 'A', no modifiers
let tap   = Event::tap(Point::new(100, 200));
let timer_event = Event::timer(42);                            // timer ID 42
let pinch = Event::pinch(1.5);                                 // 150% zoom
```

**手势分类：** `Event::gesture_class()` 返回 `Some(GestureClass::Single)`、`Some(GestureClass::Multi)` 或 `None`（非手势事件）。

**触控检测：** `Event::is_touch()` 对触控和手势事件变体返回 `true`，对鼠标/键盘事件返回 `false`。

---

## `EventHandler` Trait

每个控件都实现了 `EventHandler` trait 以接收事件：

```rust
pub trait EventHandler {
    fn handle_event(&mut self, event: &Event) -> bool;
}
```

返回 `true` 表示事件已被消费（停止传播），返回 `false` 则将其传递给下一个处理器。

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

## EventQueue — 基于 mpsc 的投递/消费

`EventQueue` 封装了 `std::sync::mpsc` 通道，用于无界事件投递和消费：

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

## `EventSender` — 可克隆的投递句柄

`EventSender` 轻量且可克隆，专为跨线程共享设计：

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

**基于优先级的投递：**

| 方法 | 优先级 | 使用场景 |
|---|---|---|
| `sender.post(id, event)` | `EventPriority::Normal` | 默认：用户输入、重绘请求 |
| `sender.post_with_priority(id, event, priority)` | 自定义 | 显式优先级控制 |
| `sender.post_idle(id, event)` | `EventPriority::Idle` | 低优先级后台任务 |

---

## `EventPriority`

```rust
pub enum EventPriority {
    High,    // Process immediately: resize, quit, orientation change
    Normal,  // Standard input: mouse, keyboard, touch
    Idle,    // Process when idle: background updates, pre-rendering
}
```

事件循环按优先级顺序消费：先处理所有 `High` 事件，然后是 `Normal`，最后是 `Idle`。

---

## EventLoop — 后台线程泵

`EventLoop` 运行一个专用的后台线程，驱动事件系统：

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

**平台事件泵集成：**

```rust
// Set a native platform event pump callback
event_loop.set_native_pump(|| {
    // Called when the event queue is empty — poll the platform for new events
    platform.poll_events()
});
```

当内部队列被消费完毕后，会调用原生泵，使事件循环能够拉取平台事件而不会阻塞。

---

## FocusManager — 键盘焦点与 Tab 键顺序

管理键盘焦点状态和 Tab 键顺序遍历：

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

当焦点改变时，会向相应的控件投递 `Event::FocusGained` 和 `Event::FocusLost` 事件。

---

## PointerCaptureManager — 拖拽操作

管理拖拽交互期间的指针捕获：

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

这确保了平滑的拖拽操作：一旦在某个控件上开始拖拽，该控件将接收所有 `MouseMove` 事件，即使光标离开了它的边界。

---

## TimerManager — 一次性与重复定时器

管理周期性和延迟事件分发：

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

定时器管理器通过 `start_timer`/`stop_timer` 方法与 `EventLoop` 集成。

---

## 触控到鼠标的事件转换

`translator` 模块（通过 `feature = "touch"` 门控）将触控事件转换为合成鼠标事件，供仅实现了鼠标处理的控件使用：

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

| 触控事件 | 合成鼠标事件 |
|---|---|
| `TouchBegin` | `MousePress` → `MouseEnter` |
| `TouchMove` | `MouseMove` |
| `TouchEnd` | `MouseRelease` → `MouseLeave` |
| `Tap` | `MousePress` + `MouseRelease` (点击) |

---

## 异步任务调度

事件系统通过 `AsyncTask` 支持异步工作的调度：

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

## 通用队列原语

`queue` 模块提供了基础数据结构：

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

这些原语由 `EventQueue` 内部使用，也可复用于自定义事件缓冲。

---

## 常见模式

### 包含所有鼠标状态的按钮

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

### 可键盘聚焦的输入字段

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

### 带指针捕获的拖放

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

### 使用定时器的帧循环动画

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

### 平台集成：自定义事件循环

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
