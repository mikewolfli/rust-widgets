# 事件系統

`rust-widgets` 事件系統提供了一個全面的分層通訊管線：來自平台泵的輸入事件、用於解耦發布的基於 `mpsc` 的佇列、用於分派的背景事件迴圈、焦點/指針/計時器管理、文字輸入/IME 提交，以及觸控到滑鼠的事件轉譯。

---

## 核心架構

```
Platform Pump → EventLoop (bg thread) → EventQueue (mpsc) → EventHandler::handle_event()
                    ↑                            ↑
               TimerManager              EventSender (cloneable)
                                          FocusManager
                                          PointerCaptureManager
```

---

## `Event` 列舉

```rust
pub enum Event {
    // Mouse
    MouseDown((Point, u32)),                MouseUp((Point, u32)),
    MouseMove { pos: Point },               MousePress { pos: Point, button: u32 },
    MouseRelease { pos: Point, button: u32 }, MouseDoubleClick { pos: Point, button: u32 },
    MouseEnter { pos: Point },              MouseLeave { pos: Point },
    Wheel { delta: Point, modifiers: u32 },

    // Keyboard
    KeyDown((u32, u32)),                    KeyUp((u32, u32)),
    KeyPress { key: u32, modifiers: u32 },  KeyRelease { key: u32, modifiers: u32 },

    // Text input and IME
    TextInput { text: String },
    ImePreedit { text: String, cursor: usize },
    ImeCommit { text: String },

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
    Custom { name: String, payload: Vec<u8> },
    Quit,
}
```

**輔助建構子** 讓事件建立更簡潔：

```rust
let press = Event::mouse_press(50, 50, 0);                    // button 0 (left)
let key   = Event::key_press(65, 0);                           // key 'A', no modifiers
let text  = Event::text_input("hello");
let timer_event = Event::timer(42);                            // timer ID 42
```

**手勢分類：** `Event::gesture_class()` 會對非手勢事件回傳 `Some(GestureClass::Single)`、`Some(GestureClass::Multi)` 或 `None`。

**觸控偵測：** `Event::is_touch()` 對所有觸控與手勢事件變體回傳 `true`，對滑鼠/鍵盤事件回傳 `false`。

---

## `EventHandler` 特徵

每個控制項實作 `EventHandler` 特徵來接收事件：

```rust
pub trait EventHandler {
    fn handle_event(&mut self, event: &Event);
}
```

容器負責決定是否把事件繼續轉發給子控制項；handler 本身不回傳 consumed 標誌。

```rust
impl EventHandler for MyButton {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MousePress { pos, .. } if self.bounds.contains(*pos) => {
                self.pressed = true;
                self.request_repaint();
            }
            Event::MouseRelease { pos, .. } if self.pressed => {
                self.pressed = false;
                self.on_click();
            }
            _ => {}
        }
    }
}
```

---

## EventQueue——基於 mpsc 的發布/清空

`EventQueue` 包裝 `std::sync::mpsc` 通道，用於無界事件發布和清空：

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

## `EventSender`——可複製的發布控制代碼

`EventSender` 輕量且可複製，設計為可跨執行緒共享：

```rust
let sender1: EventSender = queue.sender();
let sender2 = sender1.clone();

// Send from multiple threads
std::thread::spawn(move || {
    sender1.post(network_widget, Event::Custom {
        name: "data-arrived".into(),
        payload: payload_bytes,
    }).unwrap();
});

std::thread::spawn(move || {
    sender2.post(timer_widget, Event::Timer { id: 1 }).unwrap();
});
```

**依優先順序發布：**

| 方法 | 優先順序 | 使用情境 |
|---|---|---|
| `sender.post(id, event)` | `EventPriority::Normal` | 預設：使用者輸入、重繪請求 |
| `sender.post_with_priority(id, event, priority)` | 自訂 | 明確控制優先順序 |
| `sender.post_idle(id, event)` | `EventPriority::Idle` | 低優先順序的背景工作 |

---

## `EventPriority`

```rust
pub enum EventPriority {
    High,    // Process immediately: resize, quit, orientation change
    Normal,  // Standard input: mouse, keyboard, touch
    Idle,    // Process when idle: background updates, pre-rendering
}
```

事件迴圈會依優先順序清空佇列：先處理所有 `High` 事件，接著是 `Normal`，最後是 `Idle`。

---

## EventLoop——背景執行緒泵

`EventLoop` 執行一個專用的背景執行緒來驅動事件系統：

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

**平台事件泵整合：**

```rust
// Set a native platform event pump callback
event_loop.set_native_pump(|| {
    // Called when the event queue is empty — poll the platform for new events
    platform.poll_events()
});
```

當內部佇列清空時，就會呼叫原生泵，讓事件迴圈不必阻塞即可拉取平台事件。

---

## FocusManager——鍵盤焦點與 Tab 順序

管理鍵盤焦點狀態和 Tab 順序遍歷：

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

當焦點改變時，`Event::FocusGained` 與 `Event::FocusLost` 會被發布到對應的控制項。

---

## PointerCaptureManager——拖曳操作

管理拖曳互動期間的指針捕獲：

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

這可確保拖曳操作順暢：一旦在某個控制項上開始拖曳，即使游標離開該控制項的邊界，它仍會收到所有 `MouseMove` 事件。

---

## TimerManager——一次性與重複計時器

管理定期和延遲的事件分派：

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

計時器管理器透過 `start_timer`/`stop_timer` 方法與 `EventLoop` 整合。

---

## 觸控到滑鼠事件轉譯

`translator` 模組（透過 `feature = "touch"` 門控）將觸控事件轉換為合成滑鼠事件，供只實作滑鼠處理的控制項使用：

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

| 觸控事件 | 合成滑鼠事件 |
|---|---|
| `TouchBegin` | `MousePress` → `MouseEnter` |
| `TouchMove` | `MouseMove` |
| `TouchEnd` | `MouseRelease` → `MouseLeave` |
| `Tap` | `MousePress` + `MouseRelease`（點擊） |

---

## 非同步工作排程

事件系統透過 `AsyncTask` 支援排程非同步工作：

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

## 通用佇列基礎元件

`queue` 模組提供基礎的資料結構：

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

這些基礎元件由 `EventQueue` 內部使用，也可以重複用於自訂的事件緩衝。

---

## 常見模式

### 按鈕包含所有滑鼠狀態

```rust
impl EventHandler for InteractiveButton {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MouseEnter { .. } => {
                self.state = WidgetState::Hover;
                self.request_repaint();
            }
            Event::MouseLeave { .. } => {
                self.state = WidgetState::Normal;
                self.request_repaint();
            }
            Event::MousePress { pos, button: 0 } if self.bounds.contains(*pos) => {
                self.state = WidgetState::Pressed;
                self.request_repaint();
            }
            Event::MouseRelease { pos, button: 0 } => {
                if self.state == WidgetState::Pressed && self.bounds.contains(*pos) {
                    self.state = WidgetState::Hover;
                    self.on_click();  // fire the click action
                }
            }
            _ => {}
        }
    }
}
```

### 可鍵盤聚焦的輸入欄位

```rust
impl EventHandler for TextField {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::FocusGained => {
                self.focused = true;
                self.show_cursor = true;
            }
            Event::FocusLost => {
                self.focused = false;
                self.show_cursor = false;
            }
            Event::KeyPress { key, modifiers } if self.focused => {
                if *key == 8 {  // Backspace
                    self.text.pop();
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } if self.focused => {
                self.text.push_str(text);
            }
            Event::MousePress { pos, .. } => {
                // Request focus when clicked
                focus_manager.request_focus(self.id);
                // Move cursor to click position
                self.set_cursor_from_point(*pos);
            }
            _ => {}
        }
    }
}
```

### 使用指針捕獲的拖放

```rust
impl EventHandler for DraggableItem {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MousePress { pos, .. } if self.bounds.contains(*pos) => {
                self.dragging = true;
                self.drag_offset = Point::new(pos.x - self.rect.x, pos.y - self.rect.y);
                capture_manager.capture(self.id);
            }
            Event::MouseMove { pos } if self.dragging => {
                self.rect.x = pos.x - self.drag_offset.x;
                self.rect.y = pos.y - self.drag_offset.y;
                self.request_repaint();
            }
            Event::MouseRelease { .. } if self.dragging => {
                self.dragging = false;
                capture_manager.release();
                // Check for drop target
                self.check_drop();
            }
            _ => {}
        }
    }
}
```

### 使用計時器的框架迴圈動畫

```rust
// Start a repeating 16ms timer (~60 FPS)
let timer_id = event_loop.start_timer(animation_widget, Duration::from_millis(16));

// In the widget's event handler:
impl EventHandler for AnimatedWidget {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::Timer { id } if *id == ANIM_TIMER_ID => {
                self.animation_progress += 0.016;  // advance by ~1 frame
                if self.animation_progress >= 1.0 {
                    self.animation_progress = 0.0;
                }
                self.request_repaint();
            }
            _ => {}
        }
    }
}
```

### 平台整合：自訂事件迴圈

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
