# Cocoa 0.24 → objc2 Migration Plan

**Status:** Assessment Phase  
**Target:** Remove `#![allow(deprecated)]` from 3 macOS files, eliminate cocoa 0.24 dependency  
**Related:** BLUE11 R1.5 / R2.3, BLUE12 R2

---

## 1. Current State Analysis

### 1.1 Dependency Overview

| Crate | Version | Type | Status |
|-------|---------|------|--------|
| `cocoa` | 0.24 | Required (always) | **Legacy fallback** — deprecated since macOS 10.14+ |
| `objc` | 0.2 | Required (always) | Old runtime; still needed for `msg_send!`, `class!`, `sel!` |
| `objc-foundation` | 0.1 | Required (always) | Old Foundation bindings for `NSString`, etc. |
| `objc2` | 0.6 | Optional (`objc2-macos`) | **Target** — modern, safe, maintained |
| `objc2-foundation` | 0.3 | Optional (`objc2-macos`) | Modern Foundation bindings |
| `objc2-app-kit` | 0.3 | Optional (`objc2-macos`) | Modern AppKit bindings |
| `objc2-core-graphics` | 0.3 | Optional (`objc2-macos`) | Modern CG bindings |

### 1.2 Files Using cocoa 0.24

**File 1: `src/platform/macos/types.rs`**  
- `#![allow(deprecated)]` at module level
- **cocoa imports:** `cocoa::appkit::{NSView, NSWindow, NSWindowStyleMask}`, `cocoa::base::{id, nil}`, `cocoa::foundation::{NSPoint, NSRect, NSSize, NSString}`
- **objc imports:** `objc::declare::ClassDecl`, `objc::runtime::{Class, Object, Sel}`, `objc::{class, msg_send, sel, sel_impl}`
- **Usage patterns:**
  - `NSRect::new(NSPoint::new(...), NSSize::new(...))` — 2 call sites
  - `NSWindowStyleMask::NSTitledWindowMask | NSClosableWindowMask | ...` — 1 call site
  - `NSString::alloc(nil).init_str(...)` — ~5 call sites
  - `NSWindow::contentView(...)` — 2 call sites (via `as_id`)
  - `NSView::addSubview_(...)` — 1 call site
  - `msg_send![...]` — ~10 call sites for dynamic ObjC messaging
  - `ClassDecl::new(...)`, `decl.add_method(...)`, `decl.register()` — runtime class creation (3 classes)

**File 2: `src/platform/macos/platform_impl.rs`**  
- `#![allow(deprecated)]` at module level
- **cocoa imports:** `cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicyRegular, NSBackingStoreBuffered, NSBezelStyle, NSButton, NSControl, NSRunningApplication, NSTextField, NSView, NSWindow}`, `cocoa::base::{id, nil, BOOL, NO, YES}`, `cocoa::foundation::{NSArray, NSAutoreleasePool, NSData, NSPoint, NSString}`
- **objc imports:** `objc::runtime::Sel`, `objc::{class, msg_send, sel, sel_impl}`
- **Usage patterns (approximately 60+ call sites across ~1400 lines):**
  - Application lifecycle: `NSApp()`, `NSApplication::sharedApplication()`, `app.activateIgnoringOtherApps(...)`, `app.setActivationPolicy(...)`, `app.run()`, `app.terminate(...)`
  - Window creation: `NSWindow::alloc(...).initWithContentRect_styleMask_backing_defer_screen(...)`, `window.makeKeyAndOrderFront(...)`, `window.setTitle(...)`, `window.contentView()`, `window.setFrame_display(...)`
  - Button/control creation: `NSButton::alloc(...).initWithFrame(...)`, `button.setButtonType(...)`, `button.setBezelStyle(...)`, `button.setTarget(...)`, `button.setAction(...)`, `control.setStringValue(...)`, `control.setEnabled(...)`
  - Text field: `NSTextField::alloc(...).initWithFrame(...)`, `field.setStringValue(...)`, `field.setPlaceholderString(...)`, `field.setBezeled(...)`, `field.setDrawsBackground(...)`, `field.setEditable(...)`, `field.setSelectable(...)`
  - Checkbox: `button.setAllowsMixedState(...)`, `button.setState(...)`
  - Slider: `slider.setMinValue(...)`, `slider.setMaxValue(...)`, `slider.setDoubleValue(...)`
  - Progress: `progress.setIndeterminate(...)`, `progress.setUsesThreadedAnimation(...)`, `progress.startAnimation(...)`, `progress.stopAnimation(...)`, `progress.setDoubleValue(...)`
  - Menu: `NSMenu::alloc(...).initWithTitle(...)`, `menu.addItem(...)`, `NSMenuItem::alloc(...).initWithTitle_action_keyEquivalent(...)`
  - Toolbar/status bar: `NSToolbar::alloc(...).initWithIdentifier(...)`, `NSStatusBar::systemStatusBar()`, `statusBar.statusItemWithLength(...)`
  - Combo box: `combo.addItemWithObjectValue(...)`, `combo.removeAllItems()`, `combo.selectItemAtIndex(...)`, `combo.indexOfSelectedItem()`, `combo.numberOfItems()`, `combo.itemObjectValueAtIndex(...)`
  - List box/tables: `NSScrollView::alloc(...).initWithFrame(...)`, `NSTableView::alloc(...).initWithFrame(...)`, `table.reloadData()`
  - Data conversion: `NSString::alloc(nil).init_str(...)`, `NSData::alloc(nil).initWithBytes_length(...)`, `NSArray::alloc(nil).initWithObjects(...)`
  - Clipboard: `NSPasteboard::generalPasteboard(...)`, `pasteboard.clearContents()`, `pasteboard.setString_forType(...)`, `pasteboard.stringForType(...)`
  - Drag and drop: `view.beginDraggingSessionWithItems_event_source(...)`
  - Miscellaneous: `NSAutoreleasePool::new()`, `pool.drain()`, `BOOL::YES`, `BOOL::NO`

**File 3: `src/platform/macos/tests.rs`**  
- `#![allow(deprecated)]` at module level (inner `mod tests`)
- Only uses types from `MacOSPlatform`, not cocoa directly
- The deprecation allow is purely inherited from importing `crate::platform::macos::*`

### 1.3 Existing objc2 Backend

A complete `objc2` backend already exists at `src/platform/macos_objc2/`:

| Module | Contents |
|--------|----------|
| `mod.rs` | Module declarations and re-exports |
| `platform_impl.rs` | Full `Platform` trait implementation using objc2 |
| `types.rs` | Types, handles, constants |
| `native.rs` | Native AppKit FFI wrappers (`#![allow(dead_code)]`) |
| `widget_creation.rs` | Widget creation factories |
| `menu_impl.rs` | Menu/status bar implementation |
| `widget_state.rs` | Widget state management |
| `clipboard_dnd.rs` | Clipboard and drag-drop |
| `dialog_creation.rs` | Dialog creation factories |
| `tests.rs` | Tests |

The bridge in `src/platform/macos/macos_bridge.rs` selects `SelectedMacOSPlatform` based on feature flags:
- With `objc2-macos` → uses `macos_objc2::MacOSObjc2Platform`
- Without → uses `macos::MacOSPlatform` (cocoa 0.24 fallback)

---

## 2. objc2 Equivalents for Each cocoa API

### 2.1 Type Mapping

| cocoa 0.24 | objc2 0.6 Equivalent | Notes |
|---|---|---|
| `cocoa::base::id` | `objc2::ffi::NSObject` or `ObjcId` | objc2 uses typed `ObjcId<NSObject>` pattern |
| `cocoa::base::nil` | `objc2::ffi::Nil` or `None` | objc2 uses `Option<ObjcId>` |
| `cocoa::base::BOOL` | `objc2::ffi::BOOL` | Direct equivalent |
| `cocoa::base::YES/NO` | `objc2::ffi::YES/NO` | Direct equivalent |
| `cocoa::foundation::NSRect` | `objc2_foundation::NSRect` | Direct equivalent |
| `cocoa::foundation::NSPoint` | `objc2_foundation::NSPoint` | Direct equivalent |
| `cocoa::foundation::NSSize` | `objc2_foundation::NSSize` | Direct equivalent |
| `cocoa::foundation::NSString` | `objc2_foundation::NSString` | Different API (`ns_string!` macro) |
| `cocoa::foundation::NSArray` | `objc2_foundation::NSArray` | Similar API |
| `cocoa::foundation::NSData` | `objc2_foundation::NSData` | Similar API |
| `cocoa::foundation::NSAutoreleasePool` | `objc2_foundation::NSAutoreleasePool` | Direct equivalent |
| `cocoa::appkit::NSView` | `objc2_app_kit::NSView` | Direct equivalent |
| `cocoa::appkit::NSWindow` | `objc2_app_kit::NSWindow` | Direct equivalent |
| `cocoa::appkit::NSButton` | `objc2_app_kit::NSButton` | Direct equivalent |
| `cocoa::appkit::NSTextField` | `objc2_app_kit::NSTextField` | Direct equivalent |
| `cocoa::appkit::NSControl` | `objc2_app_kit::NSControl` | Direct equivalent |
| `cocoa::appkit::NSComboBox` | `objc2_app_kit::NSComboBox` | Direct equivalent |
| `cocoa::appkit::NSTableView` | `objc2_app_kit::NSTableView` | Direct equivalent |
| `cocoa::appkit::NSSlider` | `objc2_app_kit::NSSlider` | Direct equivalent |
| `cocoa::appkit::NSProgressIndicator` | `objc2_app_kit::NSProgressIndicator` | Direct equivalent |
| `cocoa::appkit::NSMenu` | `objc2_app_kit::NSMenu` | Direct equivalent |
| `cocoa::appkit::NSMenuItem` | `objc2_app_kit::NSMenuItem` | Direct equivalent |
| `cocoa::appkit::NSToolbar` | `objc2_app_kit::NSToolbar` | Direct equivalent |
| `cocoa::appkit::NSStatusBar` | `objc2_app_kit::NSStatusBar` | Direct equivalent |
| `cocoa::appkit::NSPasteboard` | `objc2_app_kit::NSPasteboard` | Direct equivalent |
| `cocoa::appkit::NSScrollView` | `objc2_app_kit::NSScrollView` | Direct equivalent |

### 2.2 Cocoa Method → objc2 Translation

cocoa 0.24 used Rust methods on types (e.g., `button.setTitle("...")`), while objc2 uses a different pattern:

```rust
// cocoa 0.24 (method-style)
let btn: id = msg_send![NSButton::class(), alloc];
let btn: id = msg_send![btn, initWithFrame: rect];
btn.setTitle_(ns_string);

// objc2 0.6 (typed, safer)
let btn = NSButton::alloc();
let btn = unsafe { btn.initWithFrame(rect) };
btn.setTitle(&ns_string);
```

Key API differences:
- **Allocation:** `[SomeClass alloc]` → `SomeClass::alloc()`
- **Initialization:** `msg_send![obj, initWith...]` → `unsafe { obj.initWithFrame(rect) }`
- **Property access:** Method calls on cocoa types → Direct property access via `objc2` (`.setTitle()` vs `.setTitle()`)
- **Selector registration:** `sel!(...)` → `sel!(...)` (same)
- **Dynamic class creation:** `ClassDecl::new(...)` → `ClassBuilder::new(...)` (very similar API)
- **Autorelease pool:** `NSAutoreleasePool::new()` / `pool.drain()` → `NSAutoreleasePool::new()` / `drop(pool)`
- **String creation:** `NSString::alloc(nil).init_str("...")` → `NSString::from_str("...")` or `ns_string!("...")`

### 2.3 Migration Complexity by Category

| Category | Call Sites | Complexity | Key Challenge |
|----------|-----------|------------|---------------|
| Window creation | ~15 | Medium | Style mask bitflags differ slightly |
| Button/control | ~20 | Low | Straightforward type mapping |
| Menu system | ~15 | Medium | Action/target wiring differs |
| Text field | ~10 | Low | Similar API surface |
| Combo/list | ~10 | Low | Similar API surface |
| Progress/slider | ~6 | Low | Simple property setters |
| Clipboard/drag-drop | ~8 | Medium | Delegate pattern differences |
| Dynamic class creation | ~3 | **High** | Lifetime management differs |
| Toolbar/status bar | ~5 | Medium | Delegate pattern |
| `msg_send!` calls | ~50+ | Medium | Need manual verification per call |

---

## 3. Migration Steps

### Phase 1: Audit & Preparation (est. 2-3 sessions)

1. **Audit all cocoa call sites** in the 3 files (`types.rs`, `platform_impl.rs`, `tests.rs`)
   - Create a detailed call-site inventory
   - Mark which are covered by `macos_objc2::native.rs` already
2. **Identify shared utility functions** that exist only in the cocoa backend
   - `parse_shortcut()`, `menu_target_class()`, `button_target_class()` — these use objc 0.2 directly
   - `make_rect()`, `window_style()`, `get_handle()`, `register_handle()`, `add_to_parent_window()` — these use objc or cocoa
3. **Review test coverage** for the objc2 backend
4. **Assess breaking changes** in the `MacOSPlatform` struct fields

### Phase 2: Incremental Migration (est. 4-5 sessions)

**Recommended approach: Gradual, per-category migration**

| Step | Category | Files | Dependencies |
|------|----------|-------|-------------|
| 1 | `types.rs` foundation types | `types.rs` | None (types used by `platform_impl`) |
| 2 | Window creation | `platform_impl.rs` | Step 1 |
| 3 | Basic controls (button, checkbox, radio, label, progress, slider) | `platform_impl.rs` | Step 1 |
| 4 | Text controls (line edit, text field) | `platform_impl.rs` | Step 1 |
| 5 | Menu system | `platform_impl.rs` | Step 1 |
| 6 | Combo box / list box / table | `platform_impl.rs` | Step 1 |
| 7 | Clipboard, drag-drop | `platform_impl.rs` | Step 1 |
| 8 | Dialog creation (file, color, font) | `platform_impl.rs` | Step 1 |
| 9 | Dynamic class creation (menu/button targets) | `types.rs` | `objc2::declare` module |
| 10 | Remove cocoa/objc 0.2 deps + `#[allow(deprecated)]` | All + `Cargo.toml` | All steps above |

### Phase 3: Validation (est. 2-3 sessions)

1. **Build testing matrix:**
   - `cargo check --features objc2-macos` (new backend)
   - `cargo check` (no features = cocoa fallback — should still compile during transition)
   - `cargo test --features objc2-macos` (objc2 tests pass)
   - `cargo test` (cocoa tests still pass)
2. **Manual testing** on macOS 14+ (Sonoma/Sequoia) to verify:
   - Window creation, resizing, closing
   - All control types
   - Menu interaction
   - Clipboard/drag-drop
   - Dialog creation
3. **CI integration** — add macOS CI job with `objc2-macos` feature

---

## 4. Risk Assessment

### 4.1 Risk Matrix

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Runtime crashes from incorrect objc2 API mapping | High | High | Comprehensive test suite; gradual per-category migration with tests after each step |
| Breaking changes in objc2 0.6→0.7 | Medium | Medium | Pin to known working version; review objc2 changelog before migration |
| NSAccessibility incompatibility | Medium | High | The a11y bridge uses `NSAccessibility` protocols — verify objc2 support |
| Dynamic class creation differs | Medium | Low | `objc2::declare::ClassBuilder` API is well-documented |
| Memory management differences (ARC vs manual) | Low | Medium | objc2 uses ARC by default; cocoa 0.24 uses manual retain/release |
| Feature flag complexity | Low | Low | Keep both backends during migration; switch default later |
| Lost fixes in cocoa backend during abstraction gap | Medium | Medium | Map every cocoa call to objc2 equivalent before starting migration |

### 4.2 Key Concerns

1. **Dynamic class creation** (`ClassDecl::new` → `ClassBuilder::new`): The `on_menu_item`, `on_button_clicked`, and `on_button_clicked_simple` callbacks are registered at runtime. objc2's `ClassBuilder` has a different API for adding methods and registering classes. This is the highest-risk area.

2. **`msg_send!` macros**: The cocoa 0.24 code still uses raw `msg_send!` even for types that have objc2 bindings. During migration, some of these can be replaced with typed method calls, but some truly require `msg_send!` (especially custom selectors for target/action). A staged approach would keep `msg_send!` where needed and replace where possible.

3. **`#[allow(deprecated)]`**: This exists because cocoa 0.24 types like `NSWindowStyleMask` and `NSBackingStoreBuffered` are deprecated in macOS SDKs. The `#![allow(deprecated)]` on the 3 files silences these warnings. Once migrated, this is removed automatically.

4. **`#![allow(dead_code)]` in `macos_objc2/native.rs`**: The existing objc2 native wrappers have `#![allow(dead_code)]` because they're called via conditional compilation. This is acceptable since the functions ARE wired from `platform_impl.rs` for production use.

---

## 5. Recommended Approach

### Decision: **Gradual migration (incremental)**

**Rationale:**

| Factor | Gradual | One-shot | Verdict |
|--------|---------|----------|---------|
| Risk containment | ✅ Low risk per change | ❌ High risk | **Gradual** |
| Testability | ✅ Test after each step | ❌ Only test at end | **Gradual** |
| Development time | ⚠️ Longer total | ✅ Shorter | One-shot |
| Review complexity | ✅ Small diffs | ❌ Huge diff | **Gradual** |
| Parallel development | ✅ Both backends coexist | ❌ Single backend | **Gradual** |
| CI stability | ✅ Has fallback | ❌ All-or-nothing | **Gradual** |

### Gradual Migration Strategy

1. **Keep the existing `macos_bridge.rs`** feature-gate mechanism:
   - `objc2-macos` feature → new objc2 backend (already works)
   - No feature → cocoa 0.24 fallback (status quo)

2. **Migrate the cocoa backend file-by-file** while keeping it compiling:
   - First: `types.rs` — port to objc2 types; update `CocoaHandle` / `MacOSPlatform` struct
   - Then: `platform_impl.rs` — port each method category one at a time
   - Finally: `tests.rs` — update test imports

3. **After migration is complete**:
   - Remove `cocoa = "0.24"` from `Cargo.toml`
   - Remove `objc = "0.2"` and `objc-foundation = "0.1"`
   - Remove `#![allow(deprecated)]` from all 3 files
   - Update `macos_bridge.rs` to always use objc2
   - Remove `macos_objc2/` duplication (fold into main `macos/` module)

### Single Session Concrete Plan

**Session 1:** `types.rs` — Core types and `MacOSPlatform` struct  
**Session 2:** `types.rs` — Dynamic class creation (menu/button targets)  
**Session 3:** `platform_impl.rs` — Window + application lifecycle methods  
**Session 4:** `platform_impl.rs` — Controls (button, checkbox, radio, label, line edit, slider, progress)  
**Session 5:** `platform_impl.rs` — Menu, toolbar, status bar  
**Session 6:** `platform_impl.rs` — Combo box, list box, table  
**Session 7:** `platform_impl.rs` — Clipboard, drag-drop, dialogs  
**Session 8:** Cleanup — Remove old dependencies, fold `macos_objc2/`, update CI

### Timeline Estimate

- **Total:** 8 sessions × ~2 hours = ~16 hours of focused work
- **Calendar estimate:** 2-3 weeks (part-time)
- **Blocking prerequisites:** None (objc2 backend already exists as parallel implementation)

---

## 6. Appendix: Currently Blocked / Deferred

- `NSAccessibility` protocol helpers in `macos_bridge.rs` — need verification of `objc2_app_kit` support
- `NSTextInputContext` IME bridge — uses raw `msg_send!` currently; verify objc2 provides typed API
- `IBus`-bridge on macOS is not applicable (only Linux)
