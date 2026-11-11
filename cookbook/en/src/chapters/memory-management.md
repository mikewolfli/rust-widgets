# Memory Management

rust-widgets does **not** ship a manual allocator layer. Allocation follows Rust ownership
(principle #30): controls own their state, caches are bounded collections with explicit eviction,
and nothing in the public API exposes `alloc`/`free`-style pooling.

> **Changed 2026-11-11 (issue-20261111 R-24).** Earlier builds carried a `rust_widgets::memory`
> module (`ArenaAllocator`, `StackAllocator`, `BufferPool`, `StringPool`, `VecPool`,
> `ObjectPool`, `SharedPool`, `PoolManager`, `MemoryMonitor`, `MemoryStats`). It had **zero
> consumers** in the crate, the demos, the bindings or the tests — its own module docs said
> "Reserved public API" — so it was removed rather than kept as a second allocation vocabulary.
> Code that named it should use the standard library (`Vec`, `Box`, `Arc`) or the bounded caches
> listed below. The removal is the honest form of "do not build what nothing uses".

## What actually manages memory

| Concern | Where it lives | Bound |
|---|---|---|
| Decoded image reuse | `src/image/cache.rs` | configurable byte budget, true LRU |
| Rasterised widget layers | `src/widget/runtime/cache.rs` | `MAX_CACHED_WIDGET_LAYERS` (128) + LRU-clock |
| Text shaping / metrics / coverage | `src/render/text/` | bounded tables, invalidated by face generation |
| CSS parsing | `src/style/` | four bounded levels (G-161) |
| GPU uploads | `src/wgpu_backend/renderer/resources.rs` | LRU texture and coverage caches |
| GPU staging buffers | `src/gpu/` | grow-only arena, reuse across frames |

Every one of those is a **cache** with a documented bound, not a general-purpose pool. A cache is
allowed to drop an entry at any time; that is why the library never hands out "pooled" objects and
never asks a host to return one.

## Writing allocation-light controls

- Reuse per-frame scratch buffers (`RenderContext` keeps its own); allocate per draw only when the
  geometry genuinely changes.
- Prefer `Arc<[u8]>` for pixel payloads: the recorded-image and layer paths can then share one
  allocation instead of copying.
- Keep per-widget state in the control, not in a global map; the registry already owns the
  control, so a second table is a second source of truth (principle #101).
- For embedded profiles, the `alloc_frugal` cfg removes the allocator-dependent subsystems
  entirely rather than emulating them.

## Embedded profiles

`mini`/`embedded` builds compile out the widget registry, the theme and every cache that needs an
allocator. A control there is expected to be driven by the host directly; there is no pool to
manage and no background retention. See `docs/plans/principle.md` rule #47 for the profile gates.

## Summary

- No manual memory pool API; ownership and bounded caches instead.
- The former `memory` module was removed with evidence (R-24).
- Every cache states its bound and its eviction rule next to the code that implements it.
