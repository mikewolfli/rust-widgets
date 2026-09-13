# Mounting custom-painted widgets into native windows

Status: implemented (macOS native / Windows native / GTK-native; fallback elsewhere)

## Problem

`render_to_svg()` was the only way a `Box<dyn Widget>` could reach pixels. The
native platform layer had **no concept of a custom-painted widget**: its 41
`create_*` methods all map onto real OS controls (`NSButton`, `HWND` + `BUTTON`,
`gtk::Button`, …). Consequently every widget that paints itself through
`Draw::draw()` — `CodeEditor`, `ColorPicker`, `GanttWidget`, `TerminalView`,
`Chip`, `Snackbar`, `GridWidget`, … — was invisible inside a native window.
`WidgetFactory::create("code_editor")` returned a perfectly valid object that
was then dropped on the floor: built, never shown.

## Decision

Introduce **one** platform capability, `mount_custom_widget`, rather than 60+
per-widget native constructors:

```rust
fn mount_custom_widget(&self, parent: ObjectId, id: ObjectId, rect: Rect) -> bool;
```

The host keeps ownership of the widget in a process-wide registry keyed by
`ObjectId`; the backend only allocates a native canvas surface and, when the OS
asks it to repaint, pulls an RGBA frame out of the registry and blits it.

```
WindowHandle::mount_custom_widget(widget)
        │  registers Box<dyn Widget> under a fresh ObjectId
        ▼
Platform::mount_custom_widget(parent, id, rect)   ── default: returns false
        │
        ├─ macOS  : NSView subclass, drawRect: → CGContext
        ├─ Windows: child HWND, WM_PAINT → HDC
        └─ Linux  : gtk::DrawingArea, Draw → cairo surface
```

## Why opt-in-and-honest over per-OS platform guessing

The caller does not detect the OS. Detection would be wrong twice over:

1. A desktop build on Linux *without* `gtk-native` has no native window at all;
   the correct outcome is an explicit "cannot display this here", not a
   `#[cfg]` guess that reports support that is not present.
2. `mount_custom_widget` returns `bool`. The default trait body returns `false`
   and logs why, so backends that have not been taught yet degrade loudly
   instead of silently producing a blank window — which is exactly the bug this
   work exists to eliminate.

`#[cfg]` is still used, but only *inside* each backend, to select the native
mechanism. The caller-facing contract is uniform.

## Backend mechanics

### macOS (cocoa / objc)

* `ClassDecl::new("RustWidgetsCanvasView", class!(NSView))` registers an
  `NSView` subclass once per process (`OnceLock`).
* `drawRect:` is implemented as an `extern "C" fn(&Object, Sel, NSRect)`. It
  looks the widget up by the `ObjectId` stored on the instance, renders one
  frame with `SoftwarePaintBackend`, and blits through CoreGraphics.
* **`isFlipped` stays `NO`.** Reporting `YES` made AppKit hand `drawRect:` a
  rect of `(0, -28, 900, 648)` — the title-bar offset — and the whole canvas
  rendered above the visible region, so the window looked blank while the logs
  said the blit succeeded. Native orientation is correct because
  `blit_rgba` is orientation-preserving.
* The widget id is attached with **`objc_setAssociatedObject`**, not
  `setValue:forKey:`. KVC on a plain `NSView` subclass raises
  `NSUnknownKeyException`, and a foreign exception aborts the process
  (`Rust cannot catch foreign exceptions`).
* CoreGraphics FFI is declared in `macos/cg.rs`. The destination rect is derived
  from the **image's own pixel size** (`CGImageGetWidth/Height`), never from the
  logical view size: bitmap contexts on Retina hosts carry a backing scale, and
  a logical-size rect draws the image at 2× and crops it.
* Input: `mouseDown:` / `mouseUp:` / `mouseDragged:` / `keyDown:` are declared on
  the class and forward `MousePress` / `MouseRelease` / `MouseMove` /
  `KeyPress`+`TextInput` into the widget, then `setNeedsDisplay:`.

### Pitfalls discovered (each now covered by a test)

| Symptom | Cause | Guard |
|---|---|---|
| Windows appears, then process aborts | `setValue:forKey:` on a non-KVC view | `association_key` / `widget_id_of` use `objc_setAssociatedObject` |
| `drawRect:` rect has a negative y | `isFlipped` returned `YES` | `is_flipped` returns `NO`; rect asserted via logs |
| Mounted widget paints nothing | `Widget::as_draw_mut` defaulted to `None` while the widget did implement `Draw` | `custom_widgets_report_the_draw_bridge` |
| Blitted image enlarged and cropped | draw rect taken from logical size under a 2× backing scale | `blit_preserves_channel_order_and_orientation` |
| Red and blue swapped | bitmap `ByteOrder32Big` with `PremultipliedLast` lays out ABGR | same test, run per byte-order |

### Windows (winapi)

* A dedicated child window class `RustWidgetsCanvasClass` is registered with
  `CS_HREDRAW | CS_VREDRAW | CS_OWNDC` and a `wnd_proc` that handles
  `WM_PAINT` / `WM_ERASEBKGND` / `WM_SIZE` / mouse + key messages.
* `WM_PAINT` renders a frame into a `SoftwarePaintBackend` and pushes it with
  `StretchDIBits` using a 32-bit `BITMAPINFOHEADER` (`biHeight` negative to
  select top-down rows).
* A `WNDCLASSW` registered per-process; the canvas is a normal `WS_CHILD |
  WS_VISIBLE` window so it participates in the OS layout.

### Linux (gtk-native)

* `gtk::DrawingArea` added into the window's existing `gtk::Fixed` content
  container, so it coexists with the native-control rows.
* `connect_draw` renders a frame and blits with
  `cairo::ImageSurface::create_for_data` + `Context::set_source_surface` +
  `paint`.

## Registry (`widget::runtime`)

`src/widget/runtime.rs` owns:

* `register(Box<dyn Widget>) -> ObjectId` — takes ownership, assigns an id.
* `with_widget_mut(id, f)` — hands the widget to a painter; keeps the borrow
  scoped so a nested paint cannot re-enter.
* `geometry_of(id)` / `set_geometry(id, Rect)`.
* `dispatch_pointer(id, event)` / `dispatch_key(id, event)` — so backends can
  forward input into the widget's `EventHandler`.
* `unregister(id)`.

Widgets are `!Send` (they hold `Rc`/`RefCell`), which is correct here: all
three native backends require UI work on the platform's main thread, and the
registry is a thread-local.

## Verification

* `cargo check` / `clippy --all-targets` clean on host.
* `demo/code_editor` and `demo/control`: `mount_custom_widget` must return true on
  a desktop backend and the window must show rendered content.
* Backends without an implementation must keep returning `false`, and the
  demos must say so rather than claim success.
