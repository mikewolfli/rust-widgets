# 事件系統

`rust-widgets` 事件系統提供了一個全面的分層通訊管線：來自平台泵的輸入事件、用於解耦發布的基於 `mpsc` 的佇列、用於分派的背景事件迴圈、焦點/指針/計時器管理，以及觸控到滑鼠的事件轉譯。系統包含 **54 個事件變體**，涵蓋滑鼠、鍵盤、觸控、手勢、繪製、計時器和遊戲手把輸入。

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

## `Event` 列舉——54 個變體

```rust
pub enum Event {
    // 滑鼠
    MouseDown,                              MouseUp,
    MouseMove { pos: Point },               MousePress { pos: Point, button: u32 },
    MouseRelease { pos: Point, button: u32 }, MouseDoubleClick { pos: Point, button: u32 },
    MouseEnter { pos: Point },              MouseLeave { pos: Point },
    Wheel { delta: (f32, f32), modifiers: u32 },

    // 鍵盤
    KeyDown, KeyUp,
    KeyPress { key: u32, modifiers: u32 },  KeyRelease { key: u32, modifiers: u32 },

    // 焦點
    FocusGained, FocusLost,

    // 繪製 / 版面配置
    Paint,  Resize { size: Size },

    // 計時器
    Timer { id: u32 },

    // 觸控（8 個變體）
    TouchBegin { pos: Point, touch_id: TouchId },
    TouchEnd   { pos: Point, touch_id: TouchId },
    TouchMove  { pos: Point, touch_id: TouchId },

    // 手勢（9 個變體）
    Tap { pos: Point },                     DoubleTap { pos: Point },
    LongPress { pos: Point },               Swipe { start: Point, end: Point, velocity: f32 },
    Pinch { scale: f32 },                   Rotate { angle: f32 },
    Drag { pos: Point, touch_id: TouchId, delta: (f32, f32) },
    TwoFingerTap { pos: Point },            TwoFingerSwipe { ... },
    Fling { pos: Point, velocity: f32, touch_id: TouchId },

    // 全像投影（XR/3D）
    HolographicTouch { pos: Point, depth: f32, touch_id: TouchId },

    // 觸控筆（含壓力/傾斜）
    PointerPress  { pos: Point, button: u32, pressure: f32, tilt_x: f32, tilt_y: f32 },
    PointerMove   { pos: Point, pressure: f32, tilt_x: f32, tilt_y: f32 },
    PointerRelease { pos: Point, button: u32, pressure: f32 },

    // 遊戲手把（4 個變體）
    GamepadPress { button: u32 },           GamepadRelease { button: u32 },
    GamepadAxis { axis: u32, value: f32 },  GamepadConnected { id: u32 },
    GamepadDisconnected { id: u32 },

    // 方向 & 生命週期
    OrientationChanged { orientation: ScreenOrientation },
    Custom { name: String, payload: Box<dyn std::any::Any> },
    Quit,
}
```

---

## `EventHandler` 特徵

每個控制項實作 `EventHandler` 特徵來接收事件：

```rust
pub trait EventHandler {
    fn handle_event(&mut self, event: &Event) -> bool;
}
```

回傳 `true` 表示事件已被消費（停止傳播），`false` 則傳遞給下一個處理器。

---

## EventQueue——基於 mpsc 的發布/清空

`EventQueue` 包裝 `std::sync::mpsc` 通道，用於無界事件發布和清空：

```rust
let queue = EventQueue::new();
let sender: EventSender = queue.sender();

sender.post(widget_id, Event::Paint)?;
sender.post_with_priority(widget_id, event, EventPriority::High)?;
sender.post_idle(widget_id, Event::Paint)?;
```

---

## EventPriority

```rust
pub enum EventPriority {
    High,    // 立即處理：resize, quit, orientation change
    Normal,  // 標準輸入：mouse, keyboard, touch
    Idle,    // 空閒時處理：background updates, pre-rendering
}
```

---

## EventLoop——背景執行緒泵

`EventLoop` 執行一個專用的背景執行緒來驅動事件系統：

```rust
let mut event_loop = EventLoop::new();
event_loop.set_dispatch_fn(|target_id, event, priority| { /* ... */ });
event_loop.start();
```

---

## FocusManager——鍵盤焦點與 Tab 順序

管理鍵盤焦點狀態和 Tab 順序遍歷：

```rust
let mut focus = FocusManager::new();
focus.request_focus(button_id);
focus.focus_next();
focus.focus_previous();
```

---

## PointerCaptureManager——拖曳操作

管理拖曳互動期間的指針捕獲：

```rust
let mut capture = PointerCaptureManager::new();
capture.capture(draggable_widget);
capture.release();
```

---

## TimerManager——一次性與重複計時器

管理定期和延遲的事件分派：

```rust
let mut timers = TimerManager::new();
let id = timers.start(widget_id, Duration::from_millis(500), false);
let anim_id = timers.start(widget_id, Duration::from_millis(16), true);
```

---

## 觸控到滑鼠事件轉譯

`translator` 模組（透過 `feature = "touch"` 門控）將觸控事件轉換為合成滑鼠事件：

```rust
#[cfg(feature = "touch")]
use rust_widgets::event::translator::TouchTranslator;
let translator = TouchTranslator::new();
let events = translator.translate_touch_event(&touch_event);
```

---

## 常見模式

### 按鈕包含所有滑鼠狀態

```rust
impl EventHandler for InteractiveButton {
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseEnter { .. } => { self.state = WidgetState::Hover; true }
            Event::MouseLeave { .. } => { self.state = WidgetState::Normal; true }
            Event::MousePress { pos, button: 0 } if self.bounds.contains(*pos) => {
                self.state = WidgetState::Pressed; true
            }
            Event::MouseRelease { pos, button: 0 } => {
                if self.state == WidgetState::Pressed && self.bounds.contains(*pos) {
                    self.on_click();
                }
                true
            }
            _ => false,
        }
    }
}
```

### 動畫框架迴圈

```rust
let timer_id = event_loop.start_timer(animation_widget, Duration::from_millis(16));
impl EventHandler for AnimatedWidget {
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Timer { id } if *id == ANIM_TIMER_ID => {
                self.animation_progress += 0.016;
                self.request_repaint();
                true
            }
            _ => false,
        }
    }
}
```
