# Embedded / Resource-Constrained Support

> **This chapter was rewritten.** It previously documented a
> `rust_widgets::embedded` module with `EmbeddedConfig`, `ResourceManager`,
> `WidgetPool`, `DpiScaler`, `LightweightStyle`, `InputFilter`, `TouchPoint`,
> `init_embedded` and `init_desktop`. **No such module exists** — `grep -n "pub mod" src/lib.rs | grep -i embed`
> returns nothing, and every `use rust_widgets::embedded::…` in the old chapter
> failed to resolve. Roughly 660 lines of examples could not compile. The real
> surface is described below.

rust-widgets supports resource-constrained targets through two **device
profiles** that are selected at build time:

| Profile | Feature | What it changes |
|---|---|---|
| `mini` | `--features mini` | The allocation-frugal profile. `mini` switches the crate to `no_std`; the widget set is reduced and no platform runtime is linked. |
| `embedded` | `--features embedded` | An embedded drawing surface with a reduced widget set. |

The two profiles are **mutually exclusive** with `desktop`, `tablet` and
`mobile`, so a build names exactly one:

```bash
cargo check --no-default-features --features mini
cargo check --no-default-features --features embedded
```

> Do **not** verify with `--all-features`: that turns `desktop` and `mini` on at
> once, and because `mini` is `no_std` the combination cannot compile. It is not
> a useful check.

## What a reduced profile actually removes

The gate names are declared once in `build.rs` and referenced by intent, so a
module never re-derives the condition inline:

| Alias | Condition | Meaning |
|---|---|---|
| `device_profile` | `desktop \| tablet \| mobile` | is this a device build? |
| `full_widgets` | `device_profile && !(mini \| embedded)` | full widget set + device transport |
| `widgets_unstripped` | `!(mini \| embedded)` | widget set not reduced |
| `alloc_frugal` | `mini` | the allocation budget applies |
| `embedded_surface` | `embedded` | embedded drawing surface |

The most consequential difference is behavioural, not structural: a stripped
profile reports **honestly** that it cannot carry a custom-painted widget
surface, rather than mounting one and handing back a blank window.

```rust
// `supports_surfaces()` is the live name. On `mini`/`embedded` it returns
// `false`, so the host can decline to mount a surface instead of showing an
// empty one. This is a runtime fact — caller code contains no `cfg(target_os)`
// and no profile branch.
if !rust_widgets::supports_surfaces() {
    eprintln!("this profile cannot host a drawing surface");
}
```

## Asking what the current build supports

Rather than branching on the profile, ask for the capability you need:

```rust
use rust_widgets::platform::{backend_name, capabilities};

// Which backend is compiled in, as a runtime answer.
println!("backend: {}", backend_name());

// Which capabilities the backend reports.
let caps = capabilities();
println!("ime: {}, accessibility: {}, dpi_scaling: {}",
         caps.ime, caps.accessibility, caps.dpi_scaling);
```

This is the pattern the whole library follows: platform and profile differences
are **runtime facts** queried through the platform trait, never compile-time
branches in calling code (see `docs/ARCHITECTURE.md`).

## Repainting with the embedded surface

`embedded_surface` provides the drawing surface the host blits. The geometry
carries an explicit `stride`, kept separate from `width * 4` because a GPU
backbuffer is often padded to an alignment — a row copy that assumed tight
packing would shear the image on exactly those hosts:

```rust
use rust_widgets::platform::{FrameBuffer, SurfaceGeometry};

// A host fills these in from its display driver.
let geometry = SurfaceGeometry::tight(320, 240);
let mut buffer = FrameBuffer::new();
assert!(buffer.resize(geometry));

// `frame_mut()` hands out exactly the current region; anything beyond the
// current geometry is never exposed.
if let Some(frame) = buffer.frame_mut() {
    println!("{} bytes to blit", frame.len());
}
```

## Memory behaviour under `alloc_frugal`

`mini` sets `alloc_frugal`, which selects the frugal allocator paths. Their
behaviour is deliberately transparent rather than faked:

```rust
// Under `alloc_frugal`, arena accounting is a no-op and reports 0 bytes, and its
// documentation says so explicitly: "Do not use this as a memory-usage
// measurement." A value that looked plausible would be the kind of fabricated
// report the project forbids.
```

If you need real allocation figures on a constrained target, take them from the
target's own allocator, not from the frugal stubs.

## Summary

- Build with `--no-default-features --features mini` or `--features embedded`.
- Ask `supports_custom_widgets()` before mounting a surface.
- Ask `capabilities()` for feature facts rather than branching on the profile.
- Expect honestly-reported no-ops under `alloc_frugal`, not plausible-looking
  numbers.
