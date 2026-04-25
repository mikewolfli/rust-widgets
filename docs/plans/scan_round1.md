# Scan Round 1: Core/Error/Event/Layout/Widget

## Issues Found
| # | File | Line | Severity | Issue |
|---|------|------|----------|-------|
| 1 | `src/core/types.rs` | L71 | Medium | `pub type Result<T, E = CoreError>` shadows/conflicts with `std::result::Result` globally — users of `rust_widgets` crate will have their `Result` ambiguous |
| 2 | `src/core/types.rs` | L50-52 | Low | Unused import `Debug` from `std::fmt::{Debug, Display}` — `Debug` is only used via `#[derive(Debug)]` which is hygienic and doesn't need the import |
| 3 | `src/core/alignment.rs` | L179-181 | Low | `from_components()` returns `(Self, Self)` — a tuple of two alignments, but doc says "combined alignment". No method exists to produce a single combined `Alignment` from `(HorizontalAlignment, VerticalAlignment)` |
| 4 | `src/core/coords.rs` | L1 | Low | `use super::{Point, Rect}` — `Rect` is unused in this file (only `Point::new` is used) |
| 5 | `src/core/geometry.rs` | L229-231 | Low | `Size::to_i32()` has potential signed overflow for values > i32::MAX (the `as i32` cast truncates silently) |
| 6 | `src/error/ffi.rs` | L1+ | Low | `c_try!` and `c_try_void!` macros lack `#[cfg(feature = "controls-native")]` or similar gating — unused on platforms/builds without FFI boundary |
| 7 | `src/event/loop.rs` | L37 | Medium | Event loop `start()` has a `_event` binding that's unused — processed event is silently discarded with only a comment "In a real implementation, this would dispatch to widgets" |
| 8 | `src/event/loop.rs` | L39 | Medium | The event loop thread runs `thread::sleep(Duration::from_millis(10))` — busy-waiting approach; no condvar-based wakeup, causing up to 10ms dispatch latency |
| 9 | `src/event/loop.rs` | L19-30 | Medium | `EventLoop` uses `Arc<Mutex<EventQueue>>` but `EventQueue` already uses `mpsc::channel` internally — double-wrapping creates unnecessary contention |
| 10 | `src/event/focus.rs` | L11 | Low | `_connection_scope: ConnectionScope` field is declared but never used beyond construction — suggests incomplete focus signal wiring |
| 11 | `src/event/queue.rs` | L194-195 | Medium | `BlockingQueue::pop()` has a spin-loop pattern with `condvar.wait(queue)` followed by re-check — but no early wakeup avoidance; multiple consecutive `wait()` calls possible if spurious wake occurs on closed queue |
| 12 | `src/event/queue.rs` | L363-366 | Medium | `BoundedQueue::push()` uses `while queue.len() >= self.capacity` with `condvar.wait()` but `Condvar::wait()` can have spurious wakeups — should be `while` loop (current) but also drops the lock between checks for `closed` flag, which could race |
| 13 | `src/layout/absolute.rs` | L35 | Low | `AbsolutePosition::to_rect()` takes `_parent_size: Size` parameter that is completely unused — parent-relative positioning can't be implemented without it making the Anchor system incomplete |
| 14 | `src/layout/absolute.rs` | L139 | Medium | `AbsoluteLayout::layout()` returns `child.size_hint()` for each child — `AbsoluteLayout` stores `Box<dyn Widget>` but `size_hint()` defaults to returning `self.size()` which is `Rect::default()` (0,0) since geometry is not tracked |
| 15 | `src/layout/absolute.rs` | L269-278 | Low | `Layout for AbsoluteLayout::add_widget()` and `remove_widget()` are no-ops with comments explaining alternative API — violates the `Layout` trait contract expectation |
| 16 | `src/layout/flow.rs` | L274-283 | Low | `Layout for FlowLayout::add_widget()` and `remove_widget()` are no-ops with comments explaining alternative API — violates the `Layout` trait contract expectation |
| 17 | `src/layout/form.rs` | L42-45 | Low | `FormLayout::add_row()` accepts `_label: &str` parameter that is completely unused — method name and doc imply the label should be stored/displayed |
| 18 | `src/layout/inspector.rs` | L240-242 | Medium | `LayoutInspector::enable()` sets `ENABLED` with `Ordering::Release` but `is_enabled()` uses `Ordering::Acquire` — correct pairing, but `record_geometry()` and `register_native_layout()` do NOT check `ENABLED` before the `thread_local!` access (they do check inside, but the TLV access still allocates TLS even when disabled) |
| 19 | `src/layout/inspector.rs` | L365-383 | Low | `check_orphans()` accepts `registry: &WidgetRegistry` but `WidgetRegistry` is imported from `crate::index` which is outside the `layout/` module scope — creates tight cross-module coupling |
| 20 | `src/layout/box_layout.rs` | L131 | Low | `allocate_major_lengths()` uses `saturating_mul` and `saturating_add` for stretch arithmetic — if `total_stretch` is very large, the per-item distribution rounds down due to integer division, losing 1px per item that's then recovered in the corrective loop. Algorithm correct but not documented |
| 21 | `src/layout/box_layout.rs` | L88-99 | Low | `HBoxLayout` and `VBoxLayout` are thin wrappers around `BoxLayout` with extensive delegation — large boilerplate that could be DRY'd with a macro |
| 22 | `src/widget/widget_trait.rs` | L14-16 | High | `Widget::base()` default implementation calls `std::process::abort()` on missing override — a runtime crash for any widget implementor who forgets to provide `base()`/`base_mut()` |
| 23 | `src/widget/widget_trait.rs` | L23-25 | High | `Widget::base_mut()` default implementation calls `std::process::abort()` on missing override — same issue as #22 |
| 24 | `src/widget/widget_trait.rs` | L1-10 | Low | `Widget` trait imports `Color`, `Font`, `Margin`, `Padding`, and `WidgetStyle` at the trait level — these are only used in default method signatures, adding coupling for implementors who may not need styling |
| 25 | `src/widget/base.rs` | L19 | Low | `BaseWidget` has public field `pub mouse_pressed: bool` — but the mutability controls are done through `is_mouse_pressed()`/`set_mouse_pressed()` methods, making direct field access redundant |
| 26 | `src/widget/base.rs` | L52-54 | Medium | `BaseWidget::paint()` is a no-op that accepts `&mut RenderContext` but does nothing except `let _ = context;` — subclasses should override, but the base default silently does nothing |
| 27 | `src/widget/image.rs` | L23-26 | Medium | `Image` struct has `pub width: u32` and `pub height: u32` fields AND getter methods `width()`, `height()`, `format()`, `data()` with the same names — creates ambiguity between field access and method call |
| 28 | `src/widget/image.rs` | L41-43 | Low | `Image::from_rgba()` takes `width: u32` and `height: u32` parameters that shadow the struct field names — while valid Rust, it's confusing and inconsistent with the rest of the codebase |
| 29 | `src/widget/kind.rs` | L10-93 | Info | `WidgetKind` has 70+ variants — many are aliases (e.g. `Dialog`, `Panel`, `ContextMenu`, `ToolBox`) that duplicate other variants. This creates a 1:N mapping where multiple `WidgetKind` values map to the same concrete widget type |
| 30 | `src/widget/registry.rs` | L18 | Low | `SimpleRegistry` stores `(DrawClosure, EventClosure)` as `Box<dyn FnMut...>` tuples — `DrawClosure` takes `&mut RenderContext` but `RenderContext` is from `crate::render` which may not be available in all configurations (e.g. `embedded` feature) |
| 31 | `src/widget/draw.rs` | L19-22 | Low | `Draw::request_custom_redraw()` has `where Self: Widget` bound and calls `self.request_redraw()` — but `Widget::request_redraw()` itself delegates to `BaseWidget::request_redraw()` via the `base()` method which could `abort()` if not implemented |
| 32 | `src/widget/window.rs` | L91-98 | Low | `Window::set_close_button_size()` and similar mutators call `self.base.request_redraw()` but only emit the redraw signal — there's no actual platform-level window invalidation happening |
| 33 | `src/widget/window.rs` | L146-167 | Low | `Window` implements `Widget` trait by delegating every method to `self.base.*` — ~25 delegation methods that replicate the entire `Widget` trait surface. Could use a derive macro or delegation |
| 34 | `src/widget/window.rs` | L168-280 | Low | `Window` implements `Draw::draw()` directly — but `Draw` is a separate trait from `Widget`, meaning the rendering path is split between the two traits for the same type |
| 35 | `src/core/types.rs` | L200-220 | Low | `CoreConfig` is defined but never checked or used by any initialization path in the scanned directories — appears to be a dead type in this scope |
| 36 | `src/core/types.rs` | L155-170 | Low | `PlatformCapabilities` has `screen_width`/`screen_height`/`dpi_scale` as `u32`/`f32` public fields — the `screen_rect()` method returns a `Rect` where `x` and `y` are always 0, which is correct for screen origin but misleading for windowed contexts |
| 37 | `src/core/rect_merge.rs` | L38-39 | Info | `merge_intersecting_rects()` copies the input via `rects.to_vec()` into `working` — the copy is unavoidable since we mutate, but O(n) allocation on every call could be noted for hot paths |
| 38 | `src/event/types.rs` | L109-111 | Info | `MouseEvent` and `KeyEvent` type aliases are defined as `pub type MouseEvent = (crate::core::Point, u32)` but the `MouseDown` variant already uses `(Point, u32)` — the alias is never used in the scanned codebase |
| 39 | `src/core/geometry.rs` | L462-464, L466-468 | Low | `Rect::right()` and `Rect::bottom()` return exclusive edges — but `Rect::contains_point()` also uses exclusive max comparison — the naming doesn't distinguish "bottom" (which implies inclusive in some graphics APIs) from "max_y" |
| 40 | `src/layout/uniform_grid.rs` | L102-104 | Low | `UniformGridLayout::update()` early-returns when `self.rows == 0 || self.cols == 0` but the constructor ensures `rows.max(1)` and `cols.max(1)` — this guard is dead code |

## Details

### 1. `pub type Result<T, E = CoreError>` — Shadowing `std::result::Result`
**File:** `src/core/types.rs`, line 71

A public `Result` type alias with default error `CoreError` is defined in the `core` module and re-exported at the crate root via `pub use types::CoreResult`. This `Result` type alias, while similar to `CoreResult`, potentially conflicts when users `use rust_widgets::core::*` because `Result` would shadow `std::result::Result`. The `CoreResult<T>` alias already exists for this purpose.

**Recommendation:** Either remove the `Result` type alias from the public API surface, or `pub(crate)` scope it to avoid global namespace conflicts.

### 2. Unused `Debug` import
**File:** `src/core/types.rs`, lines 1-3

`use std::fmt::{Debug, Display}` — `Display` is used for `impl Display for Version`. However, `Debug` is never used explicitly; all `#[derive(Debug)]` usages are satisfied through the derive macro's own import path.

### 3. `from_components()` — Questionable return type
**File:** `src/core/alignment.rs`, lines 179-181

```rust
pub fn from_components(horizontal: HorizontalAlignment, vertical: VerticalAlignment) -> (Self, Self) {
    (horizontal.into(), vertical.into())
}
```

Returns a tuple `(Alignment, Alignment)` — the caller gets two independent `Alignment` values rather than a single combined alignment state (like `TopLeft`, `CenterCenter`). The method name "from components" suggests a single combined alignment.

### 4. Unused `Rect` import in coords.rs
**File:** `src/core/coords.rs`, line 1

`use super::{Point, Rect}` — `Rect` is imported but never used directly anywhere in the file. All functions work with individual `i32`/`f32`/`(f32, f32)` tuples. The `Rect` type is re-exported by the parent module, not consumed here.

### 5. `Size::to_i32()` — Silent truncation for large values
**File:** `src/core/geometry.rs`, lines 229-231

```rust
pub fn to_i32(&self) -> (i32, i32) {
    (self.width as i32, self.height as i32)
}
```

If `self.width > i32::MAX` (2,147,483,647), the `as i32` cast silently wraps to negative. In practice this is unlikely for pixel dimensions, but a `try_into().unwrap_or(i32::MAX)` pattern would be safer.

### 6. Missing feature-gating on FFI macros
**File:** `src/error/ffi.rs`

The `c_try!`, `c_try_void!` macros and all `CAbiSafe` impls are compiled into every build even when no C FFI is exposed (non-cdylib). They should be guarded with `#[cfg(feature = "controls-native")]` or similar.

### 7. `_event` binding unused in event loop
**File:** `src/event/loop.rs`, line 37

```rust
if let Some(_event) = queue.lock().unwrap().dequeue() {
    // Process the event
    // In a real implementation, this would dispatch to widgets
}
```

The dequeued event is silently discarded. The comment acknowledges this is incomplete. This is a stub that should dispatch events to registered widget handlers.

### 8. Busy-waiting sleep-based event loop
**File:** `src/event/loop.rs`, lines 43-44

```rust
thread::sleep(Duration::from_millis(10));
```

The event loop polls every 10ms regardless of whether events are available, introducing up to 10ms latency. A condvar-based blocking dequeue (like `EventQueue::dequeue_blocking()`) would eliminate latency and CPU waste.

### 9. Double-wrapped mutex + channel
**File:** `src/event/loop.rs`, lines 19-21

```rust
queue: Arc<Mutex<EventQueue>>,
```

`EventQueue` internally uses `mpsc::channel()` which is already thread-safe. Wrapping it in `Arc<Mutex<...>>` adds unnecessary contention. The `Arc` is needed for sharing, but the `Mutex` wraps a channel that is already `Send + Sync`.

### 10. Unused `_connection_scope` field
**File:** `src/event/focus.rs`, line 11

```rust
_connection_scope: ConnectionScope,
```

Created in `new()`, stored but never queried or used. Suggests focus tracking connections were planned but not yet wired.

### 11/12. Condvar patterns in queue implementations
**File:** `src/event/queue.rs`

`BlockingQueue::pop()` closes by `condvar.notify_all()` but the receiving thread re-checks `closed` only after acquiring the mutex and potentially re-entering `wait()`. In `BoundedQueue`, the same pattern applies. While functionally correct, it's worth noting the `unwrap_or_else(|e| e.into_inner())` poison recovery pattern throughout these queue types — a poisoned mutex silently continues, potentially with corrupted data.

### 13. Unused `_parent_size` parameter
**File:** `src/layout/absolute.rs`, line 35

```rust
pub fn to_rect(&self, _parent_size: Size, child_size: Size) -> Rect {
```

The `_parent_size` parameter suggests Anchor was intended to support parent-relative positioning (e.g., anchoring to right edge), but it's never used. This makes some Anchor modes (e.g., `TopRight`) potentially behave incorrectly because the right edge calculation uses `self.x` directly rather than `parent_size.width - self.x`.

### 14. `size_hint()` returns `Rect::default()` for stored widgets
**File:** `src/layout/absolute.rs`, line 139

`AbsoluteLayout` calls `child.size_hint()` to get the child's natural size, but the default `Widget::size_hint()` returns `self.size()` which for uninitialized widgets is `Rect::default() = (0,0)`.

### 15/16. No-op Layout trait implementations
**Files:** `src/layout/absolute.rs` (lines 269-278), `src/layout/flow.rs` (lines 274-283)

Both `AbsoluteLayout` and `FlowLayout` implement `Layout::add_widget()` and `Layout::remove_widget()` as no-ops, with comments directing users to use alternative APIs. This violates the `Layout` trait contract and will silently ignore widgets added via the trait interface.

### 17. Unused label parameter
**File:** `src/layout/form.rs`, lines 42-45

```rust
pub fn add_row(&mut self, _label: &str, widget_id: ObjectId) -> usize {
```

The label is passed but never stored or used. A form layout that discards labels defeats its purpose.

### 18. TLS access even when disabled
**File:** `src/layout/inspector.rs`, lines 262-265

```rust
pub fn record_geometry(widget_id: ObjectId, rect: Rect) {
    if !Self::is_enabled() { return; }
    GEOMETRY_SNAPSHOT.with(|snap| { ...
```

The function checks `is_enabled()` first, so the TLS access is avoided. However, the compiler cannot inline this across crate boundaries, so the atomic load + branch on every layout callback is a fixed overhead when enabled.

### 19. Cross-module coupling
**File:** `src/layout/inspector.rs`, line 5

```rust
use crate::index::{WidgetKind, WidgetRegistry};
```

Layout inspector depends on `crate::index::WidgetRegistry` which lives in a completely separate module. This creates a tight coupling where changes to the registry/data model directly affect layout diagnostics.

### 20. Stretch distribution with corrective loop
**File:** `src/layout/box_layout.rs`, lines 131-166

The `allocate_major_lengths` method distributes space proportionally by stretch but uses integer arithmetic, losing fractional pixels. A corrective loop redistributes remainder. This is a valid approach but not documented as such, making the loop look like a bug at first glance.

### 21. Boilerplate in HBoxLayout/VBoxLayout
**File:** `src/layout/box_layout.rs`, lines 88-173

`HBoxLayout` and `VBoxLayout` each require ~42 lines of boilerplate delegation code to wrap `BoxLayout`. This could be reduced with a `box_layout_delegate!` macro.

### 22/23. `std::process::abort()` in trait defaults
**File:** `src/widget/widget_trait.rs`, lines 14-25

```rust
fn base(&self) -> &BaseWidget {
    log::error!("[rust_widgets] Widget::base() not implemented — aborting");
    std::process::abort();
}
```

If a widget implementor forgets to override `base()`/`base_mut()`, any call to the ~25 default methods on `Widget` will crash the entire process. This is extremely unforgiving. A `panic!()` or returned error would be safer.

### 24. Unnecessary trait import coupling
**File:** `src/widget/widget_trait.rs`, lines 1-9

The `Widget` trait imports `Color`, `Font`, `Margin`, `Padding`, `WidgetStyle`, `ConnectionScope`, `GenericSignal`, `Signal1` — all of which are only used in default method bodies. Implementors must have all these types available even if they don't use them.

### 25. Public field + getter/setter duality
**File:** `src/widget/base.rs`, line 19

```rust
pub mouse_pressed: bool,
```

`mouse_pressed` is a public field but also has `is_mouse_pressed()` and `set_mouse_pressed()` methods. Any external code could mutate the field directly, bypassing any future invariants in the setter.

### 26. No-op `paint()` method
**File:** `src/widget/base.rs`, lines 52-54

```rust
pub fn paint(&mut self, context: &mut RenderContext) {
    let _ = context;
}
```

Default paint does nothing. Subclasses are expected to override. The underscore bind suppresses the unused warning, but there's no documentation saying "override this."

### 27/28. Field-method name collision
**File:** `src/widget/image.rs`, lines 23-26, 41-43

```rust
pub struct Image {
    pub data: Vec<u8>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}
impl Image {
    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
}
```

Both public fields AND getter methods exist with the same name. While valid Rust (field access vs method call), it's confusing: `image.width` returns `u32` while `image.width()` also returns `u32`. The methods should be removed or the fields made private.

### 29. WidgetKind variant bloat
**File:** `src/widget/kind.rs`, lines 10-93

`WidgetKind` has 70+ variants with many aliases (e.g., `Panel ≡ GroupBox`, `ContextMenu ≡ Menu`, `DoubleSpinBox ≡ SpinBox`). This creates ambiguity in match statements — code matching on `WidgetKind::Panel` and `WidgetKind::GroupBox` would need to handle both for the same concrete type.

### 30. RenderContext dependency
**File:** `src/widget/registry.rs`, line 18

`SimpleRegistry::DrawClosure` is `Box<dyn FnMut(&mut RenderContext)>` — but `RenderContext` lives in `crate::render` which may not compile in embedded configurations, causing a hard dependency.

### 31. Potential abort chain
**File:** `src/widget/draw.rs`, lines 19-22

If a widget implements `Draw` but uses the default `request_custom_redraw()` (which calls `self.request_redraw()` → `self.base().request_redraw()`), and the widget hasn't overridden `base()`, the `abort()` in the `Widget` default triggers.

### 32. No platform-level invalidation
**File:** `src/widget/window.rs`, lines 91-98

Setter methods like `set_close_button_size()` call `self.base.request_redraw()` which emits a signal. But in a platform-backed window, this signal needs to be connected to the platform rendering loop — which isn't done here.

### 33. Trait delegation boilerplate
**File:** `src/widget/window.rs`, lines 146-167

`Window` manually delegates ~25 methods from `Widget` to `self.base.*`. Each method is a single-line delegation. This is a ~400-line pattern that could be automated with a delegation macro or a `delegate!` crate.

### 34. Split rendering interface
**File:** `src/widget/window.rs`, lines 168-280

`Window` implements `Widget` (the base contract) and `Draw` (the rendering contract) separately. This means rendering logic lives in a different trait than widget behavior, making it harder to find all render-related code for a widget type.

### 35. Dead configuration type
**File:** `src/core/types.rs`, lines 200-220

`CoreConfig` with `desktop()`, `embedded()`, `mobile()` constructors is defined and tested but never consumed by any initialization code in the scanned directories. Either unused or consumed outside the scan scope.

### 36. PlatformCapabilities screen ownership
**File:** `src/core/types.rs`, lines 155-170

`PlatformCapabilities` represents screen dimensions, but the `screen_rect()` method returns a `Rect` with `(0,0)` origin. The `x`/`y` of 0 implicitly assumes the primary screen at origin, which breaks for multi-monitor setups.

### 37. Rect merge allocation
**File:** `src/core/rect_merge.rs`, lines 38-39

`merge_intersecting_rects` does `rects.to_vec()` to get a mutable copy, plus `consumed` boolean vec. Each call allocates O(2n) extra memory beyond the result.

### 38. Unused type aliases
**File:** `src/event/types.rs`, lines 109-111

```rust
pub type MouseEvent = (crate::core::Point, u32);
pub type KeyEvent = (u32, u32);
```

These type aliases are defined but never used within the scanned directories. They exist for legacy/backward-compatibility but have no consumers.

### 39. Edge naming vs semantics
**File:** `src/core/geometry.rs`, lines 462-468

`Rect::right()` returns `x + width` (exclusive), and `Rect::bottom()` returns `y + height` (exclusive). The names "right" and "bottom" are ambiguous — some APIs use inclusive semantics (last pixel) while this uses exclusive (first pixel past the edge). `max_x()` / `max_y()` would be clearer.

### 40. Dead guard in UniformGridLayout
**File:** `src/layout/uniform_grid.rs`, line 102

```rust
if self.rows == 0 || self.cols == 0 { return; }
```

The `new()` constructor already caps `rows` and `cols` to `max(1)`, so this guard can never trigger. It's dead code — either remove the guard or change the constructor to allow zero and handle it here.