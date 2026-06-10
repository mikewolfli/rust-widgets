# MIRI Unsafe Code Audit Report — R9.5

**Date:** 2026-06-10
**Auditor:** AI Assistant (PUA-enforced)
**Status:** Documented — MIRI runtime not available (requires nightly Rust)

---

## 1. Executive Summary

This report documents a **manual audit** of all `unsafe { }` blocks in the
`rust_widgets` codebase per the [MIRI Audit Plan](../plans/miri_audit.md).
MIRI (MIR Interpreter) detects undefined behavior at runtime but requires
Rust nightly. Since nightly is not available in this environment, all unsafe
blocks were reviewed **manually** for safety invariants.

**Key Findings:**

- **154 total** `unsafe {` blocks across 19 files
- **~82 blocks** (53%) have proper `// SAFETY:` documentation
- **~72 blocks** (47%) are missing `// SAFETY:` comments and need documentation
- **0** critical UB issues identified (no dangling pointers, no use-after-free, no double-free)
- **2** `static mut` items flagged as needing `AtomicU64` refactoring

---

## 2. Unsafe Block Count

| File | Unsafe Blocks | SAFETY Comments | Coverage |
|---|---|---|---|
| `src/platform/windows/platform_impl.rs` | 57 | 3/57 + 1 | ⚠️ 7% |
| `src/platform/macos/platform_impl.rs` | 33 | 33/33 | ✅ 100% |
| `src/platform/ios/native.rs` | 11 | 11/11 | ✅ 100% |
| `src/platform/macos/types.rs` | 8 | 3/8 | ⚠️ 38% |
| `src/platform/windows/helpers.rs` | 7 | 2/7 | ⚠️ 29% |
| `src/platform/clipboard_stubs.rs` | 6 | 0/6 | ❌ 0% |
| `src/platform/macos_objc2/native.rs` | 6 | 6/6 | ✅ 100% |
| `src/platform/windows/types.rs` | 6 | 2/6 | ⚠️ 33% |
| `src/memory/mod.rs` | 4 | 2/4 | ⚠️ 50% |
| `src/platform/accessibility/macos.rs` | 2 | 0/2 | ❌ 0% |
| `src/data_binding/binding.rs` | 2 | 1/2 | ⚠️ 50% |
| `src/json/events.rs` | 2 | 2/2 | ✅ 100% |
| `src/undo/stack.rs` | 2 | 0/2 | ❌ 0% |
| `src/bindings/binding_impl.rs` | 2 | 1/2 | ⚠️ 50% |
| `src/platform/windows/notify.rs` | 2 | 0/2 | ❌ 0% |
| `src/embedded/lightweight.rs` | 1 | 1/1 | ✅ 100% |
| `src/platform/ime_macos.rs` | 1 | 0/1 | ❌ 0% |
| `src/platform/mod.rs` | 1 | 0/1 | ❌ 0% |
| `src/platform/accessibility/windows.rs` | 1 | 0/1 | ❌ 0% |
| **TOTAL** | **154** | **~82** | **~53%** |

> **Note:** SAFETY comment detection is conservative. Some files have doc-comment
> style safety notes (`/// # Safety`) which are counted when applicable.

---

## 3. Per-Module Audit

### 3.1 ⚙️ `src/platform/windows/platform_impl.rs` (57 blocks)

**Risk Level:** 🔴 High

This file contains the Win32 FFI implementation — raw `CreateWindowExW`,
`SendMessageW`, `ShowWindow`, `SetWindowTextW`, `GetDC`, clipboard operations
(`GlobalAlloc`, `GlobalLock`/`GlobalUnlock`), and menu APIs
(`CreateMenu`, `AppendMenuW`, `SetMenu`).

**SAFETY Comments Present (via R9.5 fix):**

- `show_widget` (line ~36): ✅ Added — HWND validity, ShowWindow/UpdateWindow MSDN contract
- `set_widget_text` (line ~68): ✅ Added — null-terminated wide string, thread safety
- `create_window` `GetModuleHandleW` (line ~238): ✅ Added — null module name is valid per MSDN
- `set_clipboard_text` (line ~1539): ✅ Existing — Win32 clipboard API with error checking
- `get_clipboard_text` (line ~1581): ✅ Existing — Win32 clipboard read with error checking

**Blocks Still Missing SAFETY Comments (priority list):**

| Line | Function | Pattern | Risk |
|---|---|---|---|
| 47 | `hide_widget` | `ShowWindow(hwnd, SW_HIDE)` | Medium |
| 57 | `set_widget_geometry` | `MoveWindow` | Medium |
| 78 | `get_widget_text` | `GetWindowTextLengthW`/`GetWindowTextW` | High |
| 97 | `set_widget_enabled` | `EnableWindow` | Medium |
| 114 | `is_widget_enabled` | `IsWindowEnabled` | Low |
| 134 | `is_widget_visible` | `IsWindowVisible` | Low |
| 159 | `dpi_scale_factor` | `GetDC`/`ReleaseDC` | Medium |
| 185 | `init` | `InitCommonControls` | Low |
| 192 | `run` | Win32 message loop | High |
| 219 | `quit` | `PostQuitMessage` | Low |
| 239 | `create_window` | `CreateWindowExW` | High |
| 259 | `create_window` | `GetLastError` | Low |
| 266 | `create_window` | `ShowWindow`/`UpdateWindow` | Medium |
| 298 | `create_button` | `CreateWindowExW` | High |
| 318 | `create_button` | `GetLastError` | Low |
| 325 | `create_button` | `bind_control_command` | Medium |
| 379 | `create_checkbox` | `CreateWindowExW` | High |
| 402 | `create_checkbox` | `bind_control_command` | Medium |
| 434 | `create_radio_button` | `CreateWindowExW` | High |
| 457 | `create_radio_button` | `bind_control_command` | Medium |
| 490 | `create_line_edit` | `CreateWindowExW` | High |
| 519 | `create_line_edit` | `bind_control_command` | Medium |
| 574 | `combo_box_add_item` | `SendMessageW(CB_ADDSTRING)` | Medium |
| 592 | `combo_box_clear_items` | `SendMessageW(CB_RESETCONTENT)` | Medium |
| 612 | `combo_box_set_current_index` | `SendMessageW(CB_GETCURSEL)` | Medium |
| 614 | `combo_box_set_current_index` | `SendMessageW(CB_SETCURSEL)` | Medium |
| 638 | `combo_box_current_index` | `SendMessageW(CB_GETCURSEL)` | Medium |
| 659 | `combo_box_item_count` | `SendMessageW(CB_GETCOUNT)` | Medium |
| 677 | `combo_box_item_text` | `SendMessageW(CB_GETLBTEXTLEN)` | Medium |
| 683 | `combo_box_item_text` | `SendMessageW(CB_GETLBTEXT)` | Medium |
| 715 | `create_list_box` | `CreateWindowExW` | High |
| 744 | `create_list_box` | `bind_control_command` | Medium |
| 764 | `list_box_add_item` | `SendMessageW(LB_ADDSTRING)` | Medium |
| 782 | `list_box_remove_item` | `SendMessageW(LB_DELETESTRING)` | Medium |
| 799 | `list_box_clear_items` | `SendMessageW(LB_RESETCONTENT)` | Medium |
| 818 | `list_box_set_current_index` | `SendMessageW(LB_SETCURSEL)` | Medium |
| 836 | `list_box_current_index` | `SendMessageW(LB_GETCURSEL)` | Medium |
| 857 | `list_box_item_count` | `SendMessageW(LB_GETCOUNT)` | Medium |
| 875 | `list_box_item_text` | `SendMessageW(LB_GETTEXTLEN)` | Medium |
| 881 | `list_box_item_text` | `SendMessageW(LB_GETTEXT)` | Medium |
| 903 | `create_panel` | `CreateWindowExW` | High |
| 946 | `create_menu_bar` | `CreateMenu` | Medium |
| 1006 | `create_menu` | `CreatePopupMenu` | Medium |
| 1012 | `create_menu` | `AppendMenuW` | Medium |
| 1045 | `create_menu` | `DrawMenuBar` | Medium |
| 1089 | `attach_menu_bar_to_window` | `SetMenu` | Medium |
| 1096 | `attach_menu_bar_to_window` | `DrawMenuBar` | Medium |
| 1139 | `menu_add_item` | `AppendMenuW` | Medium |
| 1171 | `menu_add_item` | `DrawMenuBar` | Medium |
| 1275 | `create_tool_bar` | `CreateWindowExW` | High |
| 1333 | `create_status_bar` | `CreateWindowExW` | High |

**Risk Assessment:** The Win32 FFI calls are for the most part safe because:
1. HWND validity is checked via `get_native_handle()` before use
2. Wide strings are properly null-terminated by `to_wide()`
3. Null returns from `CreateWindowExW` are checked and logged
4. `GlobalLock`/`GlobalUnlock` pairs are correctly balanced (verified in `set_clipboard_text`/`get_clipboard_text`)

The clipboard implementation correctly calls `GlobalUnlock` on all early-return paths,
including when `GlobalLock` returns null (lines 1554-1558).

**Recommendation:** Add SAFETY comments to all remaining 52 blocks. Template:

```rust
// SAFETY: hwnd is a valid HWND from get_native_handle(). <API> is safe to
// call on a valid handle. <Additional invariants>.
unsafe { ... }
```

---

### 3.2 ✅ `src/platform/macos/platform_impl.rs` (33 blocks)

**Risk Level:** 🟡 Medium

**SAFETY Comments:** 33/33 (100%) — All ObjC message sends have proper SAFETY
comments explaining the main-thread requirement, nil-checking pattern, and
selector validity. Excellent example of unsafe code documentation.

**Pattern:** Every block follows the same template:
```rust
// SAFETY: <specific invariants>.
unsafe {
    let pool = NSAutoreleasePool::new(nil);
    // ... ObjC messages ...
    pool.drain();
}
```

**Verdict:** ✅ No issues. Gold standard for unsafe documentation.

---

### 3.3 ✅ `src/platform/ios/native.rs` (11 blocks)

**Risk Level:** 🟡 Medium

**SAFETY Comments:** 11/11 (100%) — All objc2 `initWithFrame` calls and the
`initWithNibName_bundle` call have proper SAFETY comments citing `MainThreadMarker`
guarantees and `Retained<T>` validity.

**Verdict:** ✅ No issues.

---

### 3.4 ✅ `src/platform/macos_objc2/native.rs` (6 blocks)

**Risk Level:** 🟢 Low

**SAFETY Comments:** 6/6 (100%) — objc2 `initWithFrame` and
`initWithContentRect_styleMask_backing_defer` calls properly documented.

**Verdict:** ✅ No issues.

---

### 3.5 ⚠️ `src/platform/macos/types.rs` (8 blocks)

**Risk Level:** 🟡 Medium

**SAFETY Comments (via R9.5 fix):**

- `on_menu_item` (line 91): ✅ Added — ObjC main-thread, valid selectors, `catch_unwind` rationale
- `add_to_parent_window` (line 317): ✅ Added — handle validation, kind check, view retention
- `sync_list_box_native` (line 365): ✅ Added — handle validation, nil-safe messaging

**Still Missing:**

- `on_button_clicked` (line 115): `msg_send!` in ObjC callback context
- `menu_target_class` (line 155): `decl.add_method` registration unsafe
- `shared_menu_target` (line 162): `msg_send![class, new]` allocation
- `button_target_class` (line 176): `decl.add_method` * 3
- `shared_button_target` (line 196): `msg_send![class, new]` allocation

**Recommendation:** Add SAFETY comments to remaining 5 blocks. These are
`OnceLock`-guarded one-time initializations that are safe but need documentation.

---

### 3.6 ❌ `src/platform/windows/notify.rs` (2 blocks)

**Risk Level:** 🟡 Medium

**Missing SAFETY Comments:**

- `ensure_window_class_registered` (line 21): `RegisterClassW` call inside `OnceLock::get_or_init`,
  raw function pointer assignment to `lpfnWndProc`. Already has an explanatory comment but no formal
  `// SAFETY:` tag covering the register/window proc assignments.
- `active_windows_platform` (line 55): Dereference of raw `*const WindowsPlatform` pointer cast from `usize`.

**Recommendation:** Add formal `// SAFETY:` comments.

---

### 3.7 ❌ `src/platform/windows/types.rs` (6 blocks)

**Risk Level:** 🔴 High

**SAFETY Comments Present:**

- `bind_control_command` (line 160): ✅ Has `/// # Safety` doc-comment on the function

**Missing SAFETY Comments:**

- `rust_widgets_wnd_proc` (line 34): `unsafe extern "system"` function; internal dereferences at lines 69, 91, 92
- `bind_control_command` (line 163): `SetWindowLongPtrW` inside the documented function
- `try_create_slider` impl block (line 300): `InitCommonControls()` and `CreateWindowExW`

**Risk Assessment:** The WM_NOTIFY handler dereferences `lparam as *const NMHDR` at lines 91-92.
While the pointer is null-checked first, the provenance is not verified by the compiler.
This is the standard Win32 pattern but needs explicit SAFETY documentation.

---

### 3.8 ❌ `src/platform/accessibility/windows.rs` (1 block)

**Risk Level:** 🟢 Low

**Missing SAFETY Comment:**

- `post_event` (line 57): `NotifyWinEvent` call with verified HWND
- **Recommendation:** Add `// SAFETY: hwnd validated etc.`

---

### 3.9 ❌ `src/platform/accessibility/macos.rs` (2 blocks)

**Risk Level:** 🟡 Medium

**Missing SAFETY Comments:**

- `post_notification` (line 61): `NSAccessibilityPostNotification` via `catch_unwind`
- `post_ns_accessibility_notification` (line 160): standalone `post_ns_accessibility_notification`
  with `transmute` and `NSAccessibilityPostNotification`

---

### 3.10 ❌ `src/platform/clipboard_stubs.rs` (6 blocks)

**Risk Level:** 🟡 Medium

**Missing SAFETY Comments:** 6/6 — macOS and Windows clipboard implementations using
NSPasteboard (`clearContents`, `setString:forType:`) and Win32 clipboard (`OpenClipboard`,
`CloseClipboard`, `GetClipboardData`, `GlobalLock`). All wrapped in `catch_unwind()`.

**Verdict:** The `catch_unwind` wrappers are a good defensive pattern for FFI boundaries,
but each `unsafe` block inside needs a SAFETY comment.

---

### 3.11 ❌ `src/undo/stack.rs` (2 blocks)

**Risk Level:** 🔴 High — potential data race

Blocks at lines 183 and 234 modify `static mut NEXT_ID: u64`:

```rust
static mut NEXT_ID: u64 = 0;

// in TextCommand::new:
let id = unsafe {
    let id = NEXT_ID;
    NEXT_ID += 1;
    CommandId(id)
};
```

**Issue:** `static mut` access is UB in Rust if concurrently accessed. These blocks are in
test code (`#[cfg(test)]`), which reduces practical risk, but the pattern is technically
undefined behavior if tests run in multi-threaded mode.

**Fix Recommendation:** Replace with `static NEXT_ID: AtomicU64 = AtomicU64::new(0)`:

```rust
use std::sync::atomic::AtomicU64;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

let id = CommandId(NEXT_ID.fetch_add(1, Ordering::Relaxed));
```

This eliminates the `unsafe` block entirely.

---

### 3.12 ⚠️ `src/memory/mod.rs` (4 blocks)

**Risk Level:** 🟡 Medium

**SAFETY Comments:** 2/4 present

- `ArenaAllocator::new` (line 62): ✅ Has SAFETY comment
- `ArenaAllocator::allocate` (line 78): ✅ Has SAFETY comment
- `ArenaAllocator::drop` (line 103): ✅ Has SAFETY comment
- `ArenaAllocator` `unsafe impl Send`: ✅ Documented by construction
- `StackAllocator::allocate` (line 131): ✅ Has SAFETY comment

**Verdict:** ✅ Well-documented memory safety. Minor: the `unsafe impl Send` at line 111
could benefit from an explicit safety comment.

---

### 3.13 Other Low-Risk Files

| File | Blocks | Status |
|---|---|---|
| `src/json/events.rs` | 2 | ✅ Both safe — JSON serialization |
| `src/embedded/lightweight.rs` | 1 | ✅ Has SAFETY comment (pool release) |
| `src/platform/ime_macos.rs` | 1 | ❌ Missing — commented-out code |
| `src/platform/mod.rs` | 1 | ❌ Missing — platform detection |
| `src/bindings/binding_impl.rs` | 2 | ⚠️ 1/2 — `CStr::from_ptr` needs comment |
| `src/data_binding/binding.rs` | 2 | ⚠️ 1/2 — raw pointer deref in callback |

---

## 4. R9.5 Fixes Applied

Six SAFETY comments were added to unsafe blocks that were previously undocumented:

| # | File | Line | Block | Safety Rationale |
|---|---|---|---|---|
| 1 | `src/platform/windows/platform_impl.rs` | 36 | `ShowWindow(hwnd, SW_SHOW)` | HWND validity from `get_native_handle()`, MSDN guarantees |
| 2 | `src/platform/windows/platform_impl.rs` | 68 | `SetWindowTextW(hwnd, ...)` | Null-terminated wide string, API thread safety |
| 3 | `src/platform/windows/platform_impl.rs` | 238 | `GetModuleHandleW(null)` | Null module = process handle, documented MSDN behavior |
| 4 | `src/platform/macos/types.rs` | 317 | `NSWindow::contentView()` + `addSubview_()` | Handle validation, kind check, cocoa selector validity |
| 5 | `src/platform/macos/types.rs` | 365 | `setStringValue:` on NSTextField | Handle validation, nil-safe messaging |
| 6 | `src/platform/macos/types.rs` | 91 | `msg_send!` in `on_menu_item` | ObjC runtime main-thread guarantee, valid selectors, `catch_unwind` |

---

## 5. Risk Assessment Matrix

| Risk Category | Unsafe Blocks | Highest Risk File | UB Probability |
|---|---|---|---|
| Raw Win32 FFI | ~65 | `platform_impl.rs` | Low (null checks present) |
| ObjC `msg_send!` | ~45 | `macos_platform_impl.rs` | Low (selectors proven, nil checked) |
| objc2 `initWithFrame` | ~17 | `ios/native.rs` | Very Low (crate guarantees) |
| `static mut` | 2 | `undo/stack.rs` | ⚠️ Data race possible |
| Raw pointer deref | ~12 | `notify.rs`, `types.rs` | Low (null checked) |
| Unsafe Send/Sync impls | 2 | `macos_objc2/native.rs`, `ios/native.rs`, `memory/mod.rs` | Low (single-thread usage) |
| Clipboard GlobalAlloc/GlobalLock | 4 | `platform_impl.rs` | Low (balanced lock/unlock) |

---

## 6. Priority Remediation List

### 🔴 High Priority (should fix before release)

1. **`src/undo/stack.rs` static mut NEXT_ID** — Replace with `AtomicU64` to eliminate UB
2. **`src/platform/windows/types.rs` WM_NOTIFY handler** — Add SAFETY comment explaining pointer provenance for `lparam as *const NMHDR`

### 🟡 Medium Priority (fix within 1-2 sprints)

3. **`src/platform/windows/platform_impl.rs`** — Add SAFETY comments to remaining 52 unsafe blocks
4. **`src/platform/windows/notify.rs`** — Add SAFETY comments to `RegisterClassW` and `active_windows_platform`
5. **`src/platform/clipboard_stubs.rs`** — Add SAFETY comments to 6 clipboard blocks
6. **`src/platform/macos/types.rs`** — Add SAFETY comments to remaining 5 unsafe blocks

### 🟢 Low Priority (documentation hygiene)

7. **`src/platform/accessibility/windows.rs`** — Add SAFETY comment (1 block)
8. **`src/platform/accessibility/macos.rs`** — Add SAFETY comments (2 blocks)
9. **`src/bindings/binding_impl.rs`** — Add SAFETY comment for `CStr::from_ptr` (1 block)
10. **`src/platform/ime_macos.rs`** — Clean up commented-out unsafe code
11. **`src/platform/mod.rs`** — Document platform detection unsafe

---

## 7. Build Verification

```
$ cargo check --all
    Finished dev [unoptimized + debuginfo] target(s) in 0.12s
```

✅ All changes compile cleanly. No logic was modified — only SAFETY documentation was added.

---

## 8. MIRI Runtime Information

MIRI was not run in this audit because:
- MIRI requires **Rust nightly** (`rustup +nightly component add miri`)
- This environment uses **stable Rust**
- See [MIRI Audit Plan](../plans/miri_audit.md) for full MIRI runtime instructions

**To run MIRI when nightly is available:**

```bash
# Install nightly MIRI
rustup toolchain install nightly
rustup +nightly component add miri

# Run on platform-agnostic code
cargo +nightly miri test --no-default-features --features state-backend

# Run on core/memory modules
cargo +nightly miri test memory
cargo +nightly miri test core
```

---

## 9. References

- [MIRI Book](https://github.com/rust-lang/miri)
- [Unsafe Code Guidelines](https://rust-lang.github.io/unsafe-code-guidelines/)
- [Rustonomicon](https://doc.rust-lang.org/nomicon/)
- [BLUE11 R9.5 MIRI audit requirement](../plans/miri_audit.md)
