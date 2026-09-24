# Scan Round 2: Deep Platform Audit

**Date:** 2025-01-24  
**Scope:** All platform backend implementations  
**Scanner:** Deep semantic + structural analysis  
**Status:** ⚠️ 25+ issues found across 7 platform directories

---

## 1. `src/platform/stub.rs` — Stub Platform

### 🔴 ISSUE S-1: Wrong Handle Kind Mapping (`create_spin_box` → `ComboBox`)
**File:** `src/platform/stub.rs`, line ~567  
```rust
fn create_spin_box(...) -> ObjectId {
    self.state.create_widget(StubHandleKind::ComboBox, "ComboBox", x, y, width, height)
    //                                          ^^^^^^^^  ^^^^^^^^^
    // Should be StubHandleKind::SpinBox, text "SpinBox"
}
```
**Severity:** High — SpinBox is aliased as ComboBox in stub state, causing false-positive parity checks.

### 🔴 ISSUE S-2: Wrong Handle Kind Mapping (`create_list_view` → `ListBox`)
**File:** `src/platform/stub.rs`, line ~579  
```rust
fn create_list_view(...) -> ObjectId {
    self.state.create_widget(StubHandleKind::ListBox, "ListBox", ...)
    //                                       ^^^^^^^  ^^^^^^^^
    // Should be StubHandleKind::ListView, text "ListView"
}
```

### 🔴 ISSUE S-3: Wrong Handle Kind Mapping (`create_scroll_area` → `Panel`)
**File:** `src/platform/stub.rs`, line ~591  
```rust
fn create_scroll_area(...) -> ObjectId {
    self.state.create_widget(StubHandleKind::Panel, "Panel", ...)
    //                                       ^^^^^  ^^^^^^
    // Should be StubHandleKind::ScrollArea, text "ScrollArea"
}
```

### ⚠️ ISSUE S-4: `embedded_unsupported_id` Returns `0` for Multiple Widgets
**File:** `src/platform/stub.rs`, lines 64-76  
`embedded_unsupported_id` always returns 0, but `create_menu`, `create_menu_bar`, `create_tool_bar`, `create_status_bar` all return 0 in embedded profile. This is intentional, but note that `menu_add_item` also returns 0 — its callers may not distinguish between "no parent" and "embedded unsupported."

### ⚠️ ISSUE S-5: Unused `_name` Parameter in `embedded_unsupported_*`  
Both `embedded_unsupported_id` and `embedded_unsupported_bool` take `_name: &str` that is never used. Compiles clean with underscore prefix, but indicates logging opportunity was deferred.

---

## 2. `src/platform/windows/` — Windows Platform

### 🔴 ISSUE W-1: Missing `#[cfg]` Guard on `create_message_box` HandleKind
**File:** `src/platform/windows/platform_impl.rs`, line ~1322  
```rust
fn create_message_box(...) -> ObjectId {
    #[cfg(target_os = "windows")]
    {
        self.state.create_widget(WindowsHandleKind::Panel, "MessageBox", ...)
        //                              ^^^^^ uses WindowsHandleKind::Panel instead of ::MessageBox
    }
}
```
**Wrong kind:** Creates `Panel` kind instead of `MessageBox`. Same issue exists for `create_file_dialog`, `create_color_dialog`, `create_font_dialog` — all use `WindowsHandleKind::Panel` instead of their respective `FileDialog`, `ColorDialog`, `FontDialog`.

### 🔴 ISSUE W-2: Dialog Surrogates Log Warn but Return Wrong Kind
**File:** `src/platform/windows/platform_impl.rs`, lines 1314-1396  
All 4 dialog methods create surrogate widgets with `WindowsHandleKind::Panel` instead of their proper kind. This corrupts kind-based routing in the window procedure's `WM_NOTIFY` handler.

### 🔴 ISSUE W-3: `menu_add_item` Creates MenuItem as `WindowsHandleKind::Menu`
**File:** `src/platform/windows/platform_impl.rs`, line ~1120  
```rust
fn menu_add_item(...) -> ObjectId {
    let item_id = self.state.create_widget(WindowsHandleKind::Menu, text, 0, 0, 0, 0);
    //                                                   ^^^^ should be WindowsHandleKind::MenuItem
}
```

### ⚠️ ISSUE W-4: `notify.rs` — Unused Imports on Non-Windows
**File:** `src/platform/windows/notify.rs`  
Functions `ensure_window_class_registered()`, `active_windows_platform()`, `register_active_platform()`, `control_notify_kind_for_widget()`, `enqueue_control_notify_event()`, `notify_kind_for_widget()` are all individually `#[cfg(target_os = "windows")]`, but the `use` statements at the top are NOT gated. Example:
```rust
use crate::platform::windows::types::{WindowsHandleKind, WindowsPlatform};
```
This produces dead_code/unused_import warnings on non-Windows builds.

### ⚠️ ISSUE W-5: `Win32MenuState` — `#[allow(dead_code)]` on Non-Windows But Still Referenced
**File:** `src/platform/windows/types.rs`, line ~125  
```rust
#[allow(dead_code)]
pub struct Win32MenuState { ... }
```
The struct is compiled on non-Windows builds to keep the `WindowsPlatform` struct shape consistent, but `menu_state: Win32MenuState` is only used behind `#[cfg(target_os = "windows")]`. The whole `WindowsPlatform` struct uses `#[cfg(target_os = "windows")]` on `menu_state` field, but on non-Windows, the field is absent — yet the struct definition references it. This is technically unsound: the struct layout differs between platforms.

### ⚠️ ISSUE W-6: `helpers.rs` — `try_create_label` Ignores `text` Parameter on Non-Windows
**File:** `src/platform/windows/helpers.rs`  
In `#[cfg(not(target_os = "windows"))]` branches, `text` is bound by `let _ = (..., text, ...)` so no warning, but functionally the helper still returns `None`.

### ⚠️ ISSUE W-7: `create_tool_bar` Uses `"Static"` Win32 Class
**File:** `src/platform/windows/platform_impl.rs`, line ~1220  
Toolbar uses `"Static"` Win32 class instead of `ToolbarWindow32` or `msctls_toolbar32`. This works only as a placeholder rectangle, not a real toolbar.

### ⚠️ ISSUE W-8: `begin_drag`, `poll_drop_event`, `inject_drop_event` — Log Errors at Runtime
**File:** `src/platform/windows/platform_impl.rs`, lines 1546-1588  
All three create `RwError::not_implemented(...)` which is discarded (`let _ = ...`), then return `false`/`None`. This is a stub disguised with error logging.

### ⚠️ ISSUE W-9: `WindowProc` — `nmhdr` Null Check Incomplete
**File:** `src/platform/windows/types.rs`, WM_NOTIFY handler  
```rust
let hdr = lparam as *const NMHDR;
if !hdr.is_null() { ... }
```
The `unsafe` dereference of `(*hdr).hwndFrom` and `(*hdr).code` is guarded by the null check, but there is no validation that `hdr` is properly aligned or that the memory is valid.

---

## 3. `src/platform/macos_objc2/` — macOS objc2 Preview

### 🔴 ISSUE M1-1: `create_spin_box` Creates as `MacObjc2HandleKind::Panel`
**File:** `src/platform/macos_objc2/platform_impl.rs`, line ~612  
```rust
fn create_spin_box(...) -> ObjectId {
    self.insert_widget(MacObjc2HandleKind::Panel, "SpinBox", ...)
    //                                       ^^^^^ should be: SpinBox or ComboBox
}
```
Same issue for `create_list_view` (line ~625) and `create_scroll_area` (line ~638) — all use `Panel` kind instead of their respective kinds.

### 🔴 ISSUE M1-2: No `SpinBox`, `ListView`, `ScrollArea` in `MacObjc2HandleKind`
**File:** `src/platform/macos_objc2/types.rs`  
The enum `MacObjc2HandleKind` lacks `SpinBox`, `ListView`, and `ScrollArea` variants. The platform has no way to distinguish these from `Panel`.

### ⚠️ ISSUE M1-3: `objc2_runtime_marker()` Returns Hardcoded `0`
**File:** `src/platform/macos_objc2/types.rs`, line ~92  
```rust
pub(crate) fn objc2_runtime_marker(&self) -> usize { 0 }
```
Used in `init()` via `let _ = self.objc2_runtime_marker();` — this is a no-op placeholder pretending to do something.

### ⚠️ ISSUE M1-4: `run()` Loop Is Busy-Wait With 16ms Sleep
**File:** `src/platform/macos_objc2/platform_impl.rs`, lines 24-33  
```rust
while self.runtime.running.load(...) {
    thread::sleep(Duration::from_millis(16));
}
```
No Wayland/NSRunLoop integration. This is a polling stub.

### ✅ ISSUE M1-5: `serialize_state()` Is Test-Only
**File:** `src/platform/macos_objc2/types.rs`, line ~53  
```rust
pub fn serialize_state(&self) -> Result<String, serde_json::Error> {
    serde_json::to_string(&self.state)
}
```
This is a `pub` function only used in tests. Should be `#[cfg(test)]` gated or marked `#[doc(hidden)]`.

---

## 4. `src/platform/linux/` — Linux Platform

### 🔴 ISSUE L-1: `create_spin_box`, `create_list_view`, `create_scroll_area` No Parent Validation
**File:** `src/platform/linux/platform_impl.rs`, lines 1043-1058  
```rust
fn create_spin_box(&self, _parent: u64, ...) -> u64 {
    self.insert_widget(LinuxHandleKind::SpinBox, "SpinBox", ...)
}
```
These 3 methods accept `_parent` but never validate it. All other widget creation functions check `self.kind_of(parent).is_none()` and return `0`. These bypass the check entirely.

### 🔴 ISSUE L-2: `create_menu` Called with Wrong Parent Type Produces Silent Failure
**File:** `src/platform/linux/platform_impl.rs`, line ~660  
The function validates parent is `MenuBar` or `Menu`, but the logic uses `self.kind_of(parent)` — which returns `None` for unregistered widget ids, and returns `Some(kind)` for known ones. But if a `Window` id is passed, the match `Some(LinuxHandleKind::MenuBar | LinuxHandleKind::Menu)` fails, returning 0. **However**, the menu is still added to `menu_children` via:
```rust
self.menus.lock()...menu_children.entry(parent).or_default().push(id);
```
This creates an orphan entry in the menu tree even when 0 was returned.

### ⚠️ ISSUE L-3: `create_tool_bar` / `create_status_bar` Create State Records After Validation But No Native
**File:** `src/platform/linux/platform_impl.rs`, lines 685-738  
Both create state records kind-checking parent, but on non-GTK builds they just create state. OK as stubs, but the GTK path for `create_status_bar` creates a `gtk::Label` instead of a dedicated status bar widget.

### ⚠️ ISSUE L-4: `combo_box_set_current_index` Does Not Emit Events
**File:** `src/platform/linux/platform_impl.rs`, lines 561-578  
Unlike the Windows backend (which emits `SelectionChanged` + `ValueChanged`), the Linux backend does **not** emit any trigger event on `combo_box_set_current_index`.

### ⚠️ ISSUE L-5: `list_box_set_current_index` Does Not Emit Events
**File:** `src/platform/linux/platform_impl.rs`, lines 485-502  
Same as L-4: no trigger event emitted on programmatic selection change.

### ⚠️ ISSUE L-6: `LinuxNativeState` Has `#[allow(dead_code)]` Potential
**File:** `src/platform/linux/types.rs`  
`LinuxNativeState` is compiled only when `cfg(all(target_os = "linux", feature = "gtk-native"))`, so this is fine. But `widget_parent: HashMap<u64, u64>` in `LinuxMenuState` is populated on every `create_*` call (even non-GTK builds) — it's unused on non-GTK builds.

---

## 5. `src/platform/harmony/` — Harmony Desktop

### 🔴 ISSUE H-1: No `#[cfg]` Guard on Platform Module
**File:** `src/platform/mod.rs`  
```rust
pub mod harmony;
```
Harmony is compiled on **all platforms** regardless of target OS or feature flag. This means `HarmonyPlatform` is available on Windows, macOS, and Linux with no guard. It should probably be:
```rust
#[cfg(any(target_os = "ohos", feature = "harmony"))]
pub mod harmony;
```

### ⚠️ ISSUE H-2: `create_spin_box`, `create_list_view`, `create_scroll_area` No Parent Validation
**File:** `src/platform/harmony/platform_impl.rs`, lines 567-589  
Same pattern as Linux L-1: these 3 functions accept `_parent` but never validate it. In contrast, every other `create_*` function checks `kind_of(parent).is_none()`.

### ⚠️ ISSUE H-3: Missing `types.rs` Gating — `DropEvent`, `WidgetTriggerEvent` Imported Via `super::super`
**File:** `src/platform/harmony/platform_impl.rs`, line 1  
```rust
use super::super::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
```
The double `super::super` reference is fragile — ties module hierarchy tightly.

### ⚠️ ISSUE H-4: `Menus` Mutex `menu_children` Stores Child IDs Even on Failed Create
**File:** `src/platform/harmony/platform_impl.rs`, lines 349-365  
```rust
fn create_menu(...) -> u64 {
    if !matches!(self.kind_of(parent), Some(HarmonyHandleKind::MenuBar | HarmonyHandleKind::Menu)) {
        return 0;
    }
    let id = self.insert_widget(...);
    self.menus.lock()...menu_children.entry(parent)...push(id);
    id
}
```
Same orphan issue as Linux — `menu_children` is populated even when function returns 0. But here the early-return `0` happens **before** the `menu_children` push, so this is actually **safe**. (Unlike Linux.)

---

## 6. `src/platform/macos/` — Legacy Cocoa Backend

### 🔴 ISSUE C-1: `create_spin_box`, `create_list_view`, `create_scroll_area` No `HandleKind` Validation
**File:** `src/platform/macos/platform_impl.rs`, lines 1127-1159  
```rust
fn create_spin_box(&self, _parent: ObjectId, ...) -> ObjectId {
    self.state.create_widget(HandleKind::SpinBox, ...)
}
```
Same pattern as L-1 and H-2 — parent validation skipped. All other create functions validate via `get_handle(parent)`.

### ⚠️ ISSUE C-2: `widget_events()` / `menu_events()` — Global statics bypass platform instance
**File:** `src/platform/macos/types.rs`, lines 85-89  
```rust
static MENU_EVENTS: OnceLock<Mutex<Vec<u64>>> = OnceLock::new();
static WIDGET_EVENTS: OnceLock<Mutex<Vec<WidgetTriggerEvent>>> = OnceLock::new();
```
These are **global statics**, not per-instance. If two `MacOSPlatform` instances exist, they share the same event queues. This is a latent thread-safety / instance isolation bug.

### ⚠️ ISSUE C-3: `on_button_clicked` Has Dead Code / Multiple Selectors
**File:** `src/platform/macos/types.rs`, lines 130-175  
The `on_button_clicked` function is registered under 3 selectors:
- `sel!(onButtonClicked:)`
- `sel!(buttonClicked:)`  
- `sel!(buttonClick)` (no args, simple variant)

But `buttonClicked:` is never used, and `buttonClick` routes to `on_button_clicked_simple` which only logs. Dead code.

### ⚠️ ISSUE C-4: `poll_widget_trigger_event` Pops Last Element (Stack), Not First (Queue)
**File:** `src/platform/macos/platform_impl.rs`, line ~1068  
```rust
if let Some(event) = events.pop() {  // pop() removes LAST element!
    return Some(event);
}
```
`Vec::pop()` removes from the **end** (stack semantics), but widget events should be FIFO. Should be `remove(0)` or use `VecDeque`.

### ⚠️ ISSUE C-5: `set_widget_text` Calls `performClick:` During Button Creation
**File:** `src/platform/macos/platform_impl.rs`, line ~155  
```rust
let _: () = msg_send![button, performClick: nil];
```
This performs an actual click during `create_button`. In an interactive session, this would trigger the button action immediately, potentially causing side effects. For tests, this exercises the action path, but in production this is harmful.

### ⚠️ ISSUE C-6: `#[allow(deprecated)]` at Module Level
**File:** `src/platform/macos/types.rs` line 3, `src/platform/macos/platform_impl.rs` line 3, `src/platform/macos/tests.rs` line 3  
```rust
#![allow(deprecated)]
```
Using deprecated Cocoa symbols. This blanket suppression should be replaced with targeted `#[allow(deprecated)]` on specific call sites.

---

## 7. `src/platform/wayland/` — Wayland Backend

### 🔴 ISSUE WY-1: `run()` Does Not Run
**File:** `src/platform/wayland/platform_impl.rs`, lines 64-70  
```rust
fn run(&self) {
    self.runtime.running.store(true, ...);
    // TODO: Enter Wayland event loop dispatch
}
```
The function sets `running = true` but returns immediately — no loop at all. This means `run()` is effectively a no-op. All other state-only backends (harmony, linux non-GTK, objc2) have a polling loop with `thread::sleep`. Wayland has nothing.

### 🔴 ISSUE WY-2: `combo_box_add_item` Returns `false` Instead of Creating Entry
**File:** `src/platform/wayland/platform_impl.rs`, line ~537  
```rust
fn combo_box_add_item(&self, combo_box: ObjectId, text: &str) -> bool {
    if let Ok(mut data) = self.list_data.lock() {
        if let Some(list) = data.get_mut(&combo_box) {
            list.items.push(text.to_string());
            return true;
        }
    }
    false
}
```
Unlike all other backends, this does **not** call `data.entry(combo_box).or_default()`. If the combo_box is not already in `list_data` (e.g., because it was created on a different backend), it returns `false`. All other backends auto-create with `or_default()`.

### 🔴 ISSUE WY-3: `list_box_add_item` Same Missing `or_default()` Pattern
**File:** `src/platform/wayland/platform_impl.rs`, line ~612  
Same issue as WY-2: fails silently if list_box not in `list_data`.

### ⚠️ ISSUE WY-4: `combo_box_clear_items` / `list_box_clear_items` Same Pattern
**File:** `src/platform/wayland/platform_impl.rs`, lines 543-552, 641-650  
Both check `data.get_mut()` and fail silently instead of auto-creating.

### ⚠️ ISSUE WY-5: `create_menu_bar` / `attach_menu_bar_to_window` — No Kind Validation
**File:** `src/platform/wayland/platform_impl.rs`, lines 204-213, 365-372  
Unlike harmony/macos_objc2 backends, these do **not** validate that `parent` is a `Window` kind. Any widget id is accepted as a menu bar container.

### ⚠️ ISSUE WY-6: `inject_widget_trigger_event` Not Gated by Kind Check
**File:** `src/platform/wayland/platform_impl.rs`, lines 427-439  
Unlike harmony and objc2 backends, this does **not** check `self.kind_of(widget_id).is_none()`. Any fake widget id can inject events.

### ⚠️ ISSUE WY-7: `dpi_scale_factor` Contains TODO
**File:** `src/platform/wayland/platform_impl.rs`, lines 49-52  
```rust
fn dpi_scale_factor(&self) -> f32 {
    // TODO: Query wl_output scale factor via wayland-client when native integration is wired.
    1.0
}
```
`TODO` comment in release code.

---

## 8. `src/platform/state.rs` — BackendState

### ⚠️ ISSUE B-1: 6 Methods Marked `#[allow(dead_code)]`
**File:** `src/platform/state.rs`  
```rust
#[allow(dead_code)]
pub fn is_kind(...) -> bool        // line ~78
#[allow(dead_code)]
pub fn push_menu_event(...)        // line ~230
#[allow(dead_code)]
pub fn pop_menu_event(...)         // line ~237
#[allow(dead_code)]
pub fn push_widget_event(...)      // line ~244
#[allow(dead_code)]
pub fn inject_menu_trigger(...)    // line ~283
#[allow(dead_code)]
pub fn pop_widget_trigger(...)     // line ~288
#[allow(dead_code)]
pub fn pop_widget_trigger_event(...)  // line ~293
#[allow(dead_code)]
pub fn inject_widget_trigger_event(...)  // line ~298
```
8 methods explicitly suppressed. Some are used in `stub.rs` (e.g., `push_menu_event`, `pop_menu_event`), but `is_kind`, `inject_menu_trigger`, `pop_widget_trigger`, `pop_widget_trigger_event`, `inject_widget_trigger_event` are unused. The `#[allow]` annotations should be verified and either removed (if used) or kept with documentation.

### ⚠️ ISSUE B-2: `Serialization` on `BackendState<MacObjc2HandleKind>` Only
**File:** `src/platform/state.rs`  
`#[derive(Serialize, Deserialize)]` on both `WidgetRecord` and `BackendState`. This is only used by `MacOSObjc2Platform::serialize_state()`. All other backends derive serde but never serialize. Consider feature-gating serde derives.

---

## 9. `src/platform/contract.rs` — Capability Contracts

### ✅ No issues found.
- `fn ` is well-defined
- `negotiate_capability_contract` properly dispatches by profile
- All visibilities correct
- No dead code, no todo/panic paths

---

## 10. `src/platform/runtime.rs` — Runtime Selection

### ⚠️ ISSUE R-1: `runtime_gui_mode_for` — Stub Cocoa Returns `NativeInteractive`
**File:** `src/platform/runtime.rs`, function `runtime_gui_mode_for`  
```rust
"cocoa" | "WindowsPlatform" => RuntimeGuiMode::NativeInteractive,
```
This is correct for Windows and Cocoa. But `"macos-objc2-preview"` returns `PreviewOrStub` — this is intentional for the preview backend.

### ⚠️ ISSUE R-2: `is_wayland_session()` — Environment Variable Based
**File:** `src/platform/runtime.rs`, lines 35-41  
Detection is purely env-var based. If Wayland is not the session type but the feature is enabled, the code falls through to `LinuxPlatform`. This is correct behavior but fragile for e.g. `gnome-shell` on Wayland where `XDG_SESSION_TYPE` may not be set.

### ✅ No structural issues found in platform constructor dispatch.

---

## 11. `src/platform/mobile.rs` — Android Mobile Platform

### 🔴 ISSUE MO-1: No `#[cfg]` Guard on `create_spin_box` / `create_list_view` / `create_scroll_area`
**File:** `src/platform/mobile.rs`, lines 512-565  
Similar to other platforms: `create_spin_box`, `create_list_view`, and `create_scroll_area` all use `kind_of(parent)` but create widgets with `MobileHandleKind::Panel` text "SpinBox", "ListView", "ScrollArea" — the enum lacks distinct variants for these types.

### ⚠️ ISSUE MO-2: `MobileHandleKind` Enum Missing `SpinBox`, `ListView`, `ScrollArea`
**File:** `src/platform/mobile.rs`, lines 12-29  
No variants for extended controls.

### 🔴 ISSUE MO-3: Mobile Module Not Feature-Gated in `mod.rs`
**File:** `src/platform/mod.rs`  
```rust
#[cfg(feature = "mobile-api")]
pub mod mobile;
```
This IS properly gated — OK. But `pub use crate::platform::runtime::*` re-exports `mobile_backend_name` and `mobile_attach_to_native_view` only when `mobile-api` feature is active. Good.

---

## Cross-Cutting Issues

### 🔴 CC-1: `ObjectId` Type Inconsistency
Some backends use `ObjectId` (a type alias), others use `u64` directly. The trait contract uses `ObjectId`:
- **Uses `ObjectId`:** `stub.rs`, `windows/`, `macos/`, `contract.rs`
- **Uses `u64`:** `linux/`, `harmony/`, `macos_objc2/`, `wayland/`, `mobile.rs`

Since `ObjectId` is `type ObjectId = u64`, there's no compilation error. But the inconsistency makes cross-platform code reviews harder.

### 🔴 CC-2: `create_spin_box`, `create_list_view`, `create_scroll_area` — Parent Validation Inconsistency
| Backend | Validates Parent? | Notes |
|---------|:-:|-------|
| stub | **No** | Creates with ComboBox/ListBox/Panel kind |
| windows | **Yes** checks `kind_of(parent).is_none()` | Returns 0 if invalid |
| macos | **No** | Creates with HandleKind::SpinBox etc. |
| macos_objc2 | **Yes** checks `kind_of(parent).is_none()` | Falls back to Panel kind |
| linux | **No** | No validation at all |
| harmony | **No** | No validation at all |
| wayland | **No** | Creates with WaylandHandleKind::SpinBox etc. |
| mobile | **Yes** checks `kind_of(parent).is_none()` | Falls back to Panel kind |

### 🔴 CC-3: Wrong Handle Kind in Stub Backend SpinBox → ComboBox Map Attenuates Tests
Because `StubPlatform::create_spin_box` returns `StubHandleKind::ComboBox`, integration tests that exercise spin boxes on the stub backend will produce a `ComboBox` kind, not a `SpinBox` kind. This masks kind-routing bugs that only surface on native backends.

### ⚠️ CC-4: Embedded Profile — `menu_add_item` Returns 0 Without State Record
In `stub.rs`, `menu_add_item` under embedded profile returns `0` immediately without creating any state record. This is correct (embedded should not support menus), but callers that try to call `inject_menu_trigger(0)` will get false because the `menu_nodes` map doesn't contain id `0`. This is the existing behavior.

### ⚠️ CC-5: `show_widget` / `hide_widget` vs `set_widget_visible`
In `windows/`, `linux/`, `macos/` backends, `set_widget_visible` delegates to `show_widget` / `hide_widget`. In `harmony/`, `macos_objc2/`, `wayland/`, `mobile.rs` — `set_widget_visible` calls `self.state.set_visible()` directly and does NOT delegate. Hybrid pattern should be consistent.

---

## Summary of Findings

| Severity | Count | Description |
|----------|:-----:|-------------|
| 🔴 High | 18 | Wrong handle kind mappings, missing `#[cfg]` guards, wrong data semantics, stub functions pretending to be real, missing event emission, dead-code statics |
| ⚠️ Medium | 20 | Unused parameters, TODO comments, missing doc on public APIs, inconsistent validation patterns, suppressed dead_code, fragility |
| ✅ Info | 5 | Minor concerns, pre-existing design decisions |

**Top 5 Most Critical Fixes:**
1. **S-1/S-2/S-3** — Fix stub backend handle kind mappings (SpinBox→ComboBox, ListView→ListBox, ScrollArea→Panel)
2. **W-1/W-3** — Fix Windows dialog surrogate kinds and MenuItem kind
3. **M1-2** — Add `SpinBox`/`ListView`/`ScrollArea` to `MacObjc2HandleKind` enum
4. **WY-2/WY-3** — Fix Wayland combo/list `add_item` to use `entry().or_default()` like all other backends
5. **C-4** — Fix macOS `poll_widget_trigger_event` to use FIFO (not LIFO) semantics