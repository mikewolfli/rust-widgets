# MIRI Unsafe Code Audit Plan

## Overview

This document defines the audit strategy for verifying unsafe Rust code using **MIRI**
(MIR Interpreter), Rust's experimental undefined behavior detection tool. The goal is
to identify and eliminate UB in all `unsafe` blocks across the rust_widgets codebase.

**MIRI** checks for:
- Out-of-bounds memory accesses (reads/writes beyond allocation)
- Use-after-free and double-free
- Invalid pointer arithmetic (e.g., offsetting past allocation end)
- Violations of pointer provenance / Stacked Borrows rules
- Incorrect `UnsafeCell` usage / data races
- Invalid FFI: null pointer dereference, wrong ABI, unaligned access
- Violation of `noalias` / `Box` guarantees
- Integer overflow in `unsafe` contexts (when `-Zmiri-check-number-ops` is active)

## Current Unsafe Code Count

As of BLUE11 R9.5 audit:

| Module | Unsafe Blocks | Risk Level |
|---|---|---|
| `src/platform/macos_objc2/` | ~12 | Medium |
| `src/platform/ios/` | ~10 | Medium |
| `src/platform/windows/` | ~20 | High |
| `src/platform/wayland/` | ~8 | Medium |
| `src/platform/android_jni.rs` | ~0 (JNI API is safe) | Low |
| `src/platform/wasm/` | ~2 | Low |
| `src/platform/accessibility/` | ~2 | Low |
| `src/render_engine/` (wgpu) | ~4 | Medium |
| `src/memory/` | ~3 | Medium |
| `src/core/` | ~2 | Low |
| **Total** | **~63** | **—** |

(*Counts are approximate; exact counts depend on cfg-gated code paths.*)

## Audit Strategy

### Phase 1: Platform Backends (Priority 1)

Platform backends contain the most `unsafe` code and are the highest risk for UB.
Audit in this order:

1. **Windows (`src/platform/windows/`)** — Win32 FFI with raw pointer casts,
   `unsafe extern "system"` callbacks, `SetWindowLongPtrW`, and window procedure
   (`rust_widgets_wnd_proc`). Check for:
   - Valid HWND after `CreateWindowExW` (already null-checked; verify provenance)
   - Correct pointer casts in `WM_NOTIFY` handler (`lparam as *const NMHDR`)
   - `SetWindowLongPtrW` GWLP_ID modifications (aliasing/borrow stack concerns)
   - `RegisterClassW` safety (verified via OnceLock)
   - Clipboard API: `GlobalLock` / `GlobalUnlock` pairing, `GlobalAlloc` usage

2. **macOS objc2 (`src/platform/macos_objc2/`)** — objc2 message sending via
   `initWithFrame`, `initWithContentRect_styleMask_backing_defer`. These are low-risk
   because objc2 wraps the raw objc_msgSend with safety checks, but verify:
   - `MainThreadMarker` guarantee (mtm) is respected
   - `NativePtr` Send+Sync impl does not cause data races
   - Retained object lifetimes are correct

3. **iOS UIKit (`src/platform/ios/`)** — Same pattern as macOS objc2. Verify:
   - `MainThreadMarker` usage is consistent
   - No double-free or dangling Retained pointers

4. **Wayland (`src/platform/wayland/`)** — wayland-client dispatch implementations.
   Verify:
   - `Dispatch` trait implementations pass correct state pointers
   - No out-of-bounds in event processing
   - Correct `wl_proxy` ownership transfer

5. **Android JNI (`src/platform/android_jni.rs`)** — JNI is safe in the `jni` crate.
   No unsafe blocks currently. Verify:
   - `JavaVM::attach_current_thread()` returns valid env
   - GlobalRef lifetimes do not outlive JVM

### Phase 2: Core Infrastructure (Priority 2)

- **`src/memory/`** — Custom allocators or pointer manipulations
- **`src/render_engine/`** — wgpu buffer/device creation (needs GPU, MIRI cannot
  fully verify but can check CPU-side memory safety)
- **`src/core/`** — Type erasure / downcast patterns

### Phase 3: Remaining Modules (Priority 3)

- `src/event/` — Raw event queues
- `src/gesture/` — Touch state tracking
- `src/signal/` — Slot storage pointer manipulation

## How to Run MIRI

### Prerequisites

MIRI requires Rust nightly:
```bash
rustup toolchain install nightly
rustup +nightly component add miri
```

### Run MIRI Test Suite

```bash
# Basic run (default features)
cargo +nightly miri test

# Run with all features (may fail on platform-specific code)
cargo +nightly miri test --all-features

# Run with specific profile
cargo +nightly miri test --no-default-features --features desktop

# Run with embedded profile (fewer FFI dependencies)
cargo +nightly miri test --no-default-features --features embedded
```

### Run MIRI on Specific Modules

```bash
# Platform-agnostic modules
cargo +nightly miri test --test platform_tests
cargo +nightly miri test core
cargo +nightly miri test memory

# Run with integer overflow detection
cargo +nightly miri test -Zmiri-check-number-ops

# Track allocation and provenance
cargo +nightly miri test -Zmiri-tag-raw-pointers
```

### Interpret MIRI Output

MIRI reports UB with a stack trace and explanation:
```
error: Undefined Behavior: trying to read from a dangling pointer
  --> src/platform/windows/helpers.rs:XX:YY
   |
   = help: ...
```

Each UB report should be:
1. **Triaged** — Determine if it is a real UB or a false positive (MIRI is strict)
2. **Documented** — Record the file, line, and UB type in the audit log
3. **Fixed** — Refactor the `unsafe` block or add safety invariants
4. **Verified** — Re-run MIRI to confirm the issue is resolved

## Known Patterns to Look For

### Pattern 1: Raw Pointer Dereference in FFI Callbacks

```rust
// RISK: lparam as *const NMHDR — provenance not tracked by compiler
let hdr = lparam as *const NMHDR;
if !hdr.is_null() {
    let hwnd_from = unsafe { (*hdr).hwndFrom };  // UB if hdr is dangling
}
```
**Fix:** Validate pointer validity before dereference. Use `NonNull` where possible.

### Pattern 2: Win32 FFI String Conversion

```rust
// RISK: GetWindowTextW may not null-terminate if buffer is too small
GetWindowTextW(hwnd, buffer.as_mut_ptr(), len + 1);
```
**Fix:** Always check return value and handle truncation.

### Pattern 3: GlobalLock / GlobalUnlock Mismatch

```rust
// RISK: Missing GlobalUnlock on early return path
let h_mem = GlobalAlloc(GHND, byte_size);
let p_dest = GlobalLock(h_mem) as *mut u16;
if p_dest.is_null() {
    CloseClipboard();
    return false;  // GlobalUnlock NOT called — memory leak
}
```
**Fix:** Ensure unlock happens on ALL return paths. Use RAII wrappers.

### Pattern 4: UnsafeCell and Mutex Interior Mutability

```rust
// RISK: MutexGuard holding reference across unsafe boundary
static NATIVE_VIEWS: LazyLock<Mutex<HashMap<u64, NativePtr>>> = ...;
```
**Fix:** Verify Send+Sync impls on wrapper types are correct. No data races.

### Pattern 5: Pointer Arithmetic with Non-Contiguous Memory

```rust
// RISK: offset from slice::from_raw_parts must be within bounds
let slice = std::slice::from_raw_parts(p_src, len);
```
**Fix:** Validate `len` before constructing the slice.

## Risk Assessment

| Risk Category | Count | Severity | Mitigation |
|---|---|---|---|
| Raw pointer FFI (Win32) | ~15 blocks | **High** | Null checks, RAII wrappers, SAFETY comments |
| objc2 message sends | ~22 blocks | **Low** | objc2 crate provides safety guarantees |
| JNI safe wrappers | 0 blocks | **None** | jni crate encapsulates FFI |
| Pointer provenance | ~5 blocks | **Medium** | Use NonNull, validate bounds |
| Send/Sync impls | ~3 blocks | **Medium** | Verify correct on target platforms |
| UnsafeCell / atomics | ~8 blocks | **Low** | AtomicOrdering audit, lock verification |
| Custom allocators | ~0 blocks | **None** | Not used — rely on std allocator |

### Priority Fix Targets

1. **Windows clipboard** (`set_clipboard_text`, `get_clipboard_text`) — GlobalLock/
   GlobalUnlock missing on early return paths.
2. **Windows WM_NOTIFY handler** — Pointer cast from `lparam` without provenance check.
3. **macOS objc2 / iOS objc2** — Verify all `init*` unsafe blocks have SAFETY comments.
4. **Wayland Dispatch impls** — Verify event queue dispatch safety.

## Audit Log Template

For each unsafe block audited, record:

```
## [module::path](file:line)
- **Unsafe block description:** ...
- **Safety invariants:** ...
- **Verified invariants:** ...
- **MIRI result:** ✅ (no UB) / ❌ (UB found — see issue #N)
- **Fix applied:** ...
- **Auditor:** ...
- **Date:** ...
```

## References

- [MIRI Book](https://github.com/rust-lang/miri)
- [Unsafe Code Guidelines](https://rust-lang.github.io/unsafe-code-guidelines/)
- [Rustonomicon](https://doc.rust-lang.org/nomicon/)
- [BLUE11 R9.5 MIRI audit requirement](docs/plans/blue11.md)
