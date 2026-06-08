# BLUE10 R5.7 — WGPU Upgrade Evaluation

> Evaluated: 2026-06-08
> Current version: wgpu 0.16.3
> Target version: wgpu 0.22.x

## Scope of Impact

The wgpu API is used in exactly 3 files:
- `src/wgpu_backend/renderer.rs` — offscreen CPU→GPU→CPU readback
- `src/gpu/adapter.rs` — adapter enumeration and selection
- `src/quality/gpu.rs` — AdapterInfo to GpuType conversion

No swapchains, shaders, render pipelines, or bind groups are used — the renderer is purely offscreen texture upload + readback.

## Breaking Changes (0.16 → 0.22)

| API (0.16) | API (0.22) | Files Affected |
|------------|-----------|----------------|
| `Instance::default()` | `Instance::new(&InstanceDescriptor { backends, .. })` | renderer.rs, adapter.rs |
| `Limits::downlevel_defaults()` | `Limits::default()` | renderer.rs |
| `ImageCopyBuffer` | `TexelCopyBufferInfo` | renderer.rs |
| `ImageDataLayout` | `TexelCopyBufferLayout` | renderer.rs |
| `Surface` (no lifetime) | `Surface<'static>` | adapter.rs |
| `AdapterInfo::vendor` (u32) | `AdapterInfo::vendor` (u32, unchanged) | adapter.rs, gpu.rs |

## Migration Strategy

1. Update `Instance::default()` → `Instance::new(InstanceDescriptor { backends: Backends::all(), .. })` (3 call sites)
2. Update `ImageCopyBuffer` → `TexelCopyBufferInfo` + `ImageDataLayout` → `TexelCopyBufferLayout`
3. Update `Limits::downlevel_defaults()` → `Limits::default()`
4. Add lifetime parameter to `Surface` usage
5. Verify `device.poll(Maintain::Wait)` still works
6. Run `cargo check` and fix remaining compile errors

## Risk Assessment

| Factor | Rating | Notes |
|--------|--------|-------|
| Compile breakage | ~15 errors | All mechanical, no logic changes |
| Runtime behavior | Low risk | No pipelines/shader compilation involved |
| Test coverage | 0 tests for wgpu path | GPU path is behind `gpu-wgpu` feature flag |
| Effort | 2-3 hours | Straightforward API migration |

## Recommendation

Proceed with upgrade. The API surface is small and well-contained. The offscreen texture readback pattern is stable across wgpu versions.
