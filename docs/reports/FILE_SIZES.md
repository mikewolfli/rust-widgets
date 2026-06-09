# File Size Audit Report

> Generated for R9.1: Large file audit.

| File | Lines | Status |
|------|-------|--------|
| `src/platform/macos/platform_impl.rs` | 1,384 | ✅ OK (< 1,500) |
| `src/platform/linux/platform_impl.rs` | 938 | ✅ OK (< 1,500) |
| `src/platform/windows/platform_impl.rs` | 1,657 | ⚠️ **EXCEEDS 1,500 lines** |
| `src/control_backend/trait_def.rs` | 1,730 | ⚠️ **EXCEEDS 1,500 lines** |

## Files Requiring Attention

### `src/platform/windows/platform_impl.rs` (1,657 lines)

Over the 1,500-line threshold by 157 lines. Consider:

- Extracting Win32 helper functions into `src/platform/windows/helpers.rs` (already exists — move more helpers there)
- Splitting the `Platform` trait implementation into sub-modules per concern (windowing, clipboard, accessibility, IME, etc.)
- Extracting large method bodies into standalone functions

### `src/control_backend/trait_def.rs` (1,730 lines)

Over the 1,500-line threshold by 230 lines. Consider:

- Splitting the `ControlBackend` trait definition into multiple focused traits (e.g., `WidgetCreation`, `LifecycleManager`, `PropertyAccessor`)
- Moving default implementations of related methods into separate helper modules
- Extracting documentation-heavy sections into a separate `trait_docs.rs` doc module
