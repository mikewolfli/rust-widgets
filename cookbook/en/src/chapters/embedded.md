# Embedded / Resource-Constrained Support

This chapter is a working guide to building for a constrained target: how to select
the profile, what the runtime gives you, how to drive a frame loop without an OS,
and how to get pixels from the library onto your own panel.

Every example below also exists as a runnable file —
[`examples/embedded_host.rs`](../../examples/embedded_host.rs) — so you can run
them instead of taking them on faith:

```bash
cargo run --no-default-features --features embedded --example embedded_host
cargo run --no-default-features --features mini     --example embedded_host
```

> **This chapter was rewritten.** It previously documented a
> `rust_widgets::embedded` module (`EmbeddedConfig`, `ResourceManager`,
> `WidgetPool`, `DpiScaler`, `LightweightStyle`, `InputFilter`, `TouchPoint`,
> `init_embedded`, `init_desktop`) that does not exist in the crate — roughly 660
> lines of examples that could not compile. Everything here was verified against
> the real API. See also
> [`docs/MIGRATION_GUIDE.md`](../../docs/MIGRATION_GUIDE.md).

---

## 1. Choosing a profile

Two profiles target constrained hardware. They are **mutually exclusive** with
`desktop`, `tablet` and `mobile`, so a build names exactly one.

| Profile | Feature | What it gives you | What it costs |
|---|---|---|---|
| `mini` | `--features mini` | Software raster, ~15 core widgets, `heapless`/`bumpalo`/`spin` allocators, no OS runtime at all. | The crate is `no_std`; the platform runtime is not linked. |
| `embedded` | `--features embedded` | Software raster, no GPU, no touch, reduced widget set. | No GPU backend, no touch. |

```toml
# Cargo.toml of your own binary
[dependencies]
rust_widgets = { version = "2.6.0", default-features = false, features = ["embedded"] }
```

A release profile that suits a device — size over speed, and `panic = "abort"`
because there is no unwinder:

```toml
[profile.release-embedded]
inherits = "release"
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

> **Do not verify with `--all-features`.** It enables `desktop` and `mini`
> together, and `mini`'s `no_std` removes the `alloc` prelude most of the crate
> resolves `String`/`Vec` through, so the combination cannot compile. It is not a
> stronger check; it is not a check at all.

## 2. Asking the build what it is

The first thing a host should do is **ask**, not assume. Every profile fact has a
runtime accessor under `platform::profile`, so the same caller code runs
unchanged across targets:

```rust
use rust_widgets::platform::profile;

println!("profile: {}", profile::profile_name());          // "mini" / "embedded" / "desktop"
println!("os runtime: {}", profile::has_os_runtime());      // false on mini
println!("full widgets: {}", profile::full_widget_set());   // false on mini/embedded
println!("alloc frugal: {}", profile::is_alloc_frugal());   // true on mini
println!("surface usable: {}", rust_widgets::supports_surfaces());
```

Verified output on each profile:

| Query | `mini` | `embedded` | `desktop` |
|---|---|---|---|
| `profile_name()` | `mini` | `embedded` | `desktop` |
| `has_os_runtime()` | `false` | `false` | `true` |
| `full_widget_set()` | `false` | `false` | `true` |
| `is_alloc_frugal()` | **`true`** | `false` | `false` |
| `supports_surfaces()` | `false` | `false` | `true` |

The gate names behind these are declared once in `build.rs`, and modules reference
them by *intent* rather than re-deriving a conjunction:

| Alias | Condition | Meaning |
|---|---|---|
| `device_profile` | `desktop \| tablet \| mobile` | is this a device build? |
| `full_widgets` | `device_profile && !(mini \| embedded)` | full widget set + device transport |
| `widgets_unstripped` | `!(mini \| embedded)` | widget set not reduced |
| `alloc_frugal` | `mini` | the allocation budget applies |
| `embedded_surface` | `embedded` | embedded drawing surface |

### When `cfg` is still the right tool

The library's rule is that **caller code branches on runtime queries for
behaviour**. But there is one case where a compile-time gate is correct: when a
symbol's *existence* differs, not its behaviour. The platform runtime
(`backend_name`, `capabilities`, `get_platform`, `quit`) is not linked into
`mini`, so:

```rust
use rust_widgets::platform::profile;

// Always available — these are const fns over profile facts.
println!("{}", profile::profile_name());

// Only present when a platform runtime is linked.
#[cfg(not(alloc_frugal))]
println!("backend: {}", rust_widgets::platform::backend_name());
```

That is a capability-existence gate, not a behaviour branch. The distinction is
worth keeping straight: `#[cfg]` decides *whether code compiles*, a query decides
*what code does*.

## 3. A frame loop with no OS

A constrained target usually has no windowing system to hand you a frame callback,
so the library provides a **frame-paced task queue**. You submit work; it runs once,
on the next frame, and receives that frame's index.

```rust
use rust_widgets::render_engine;

// The target rate is clamped to 1..=240, and the *applied* value is returned —
// so a caller that asks for 999 learns what it actually got.
let applied = render_engine::set_embedded_target_fps(60);
assert!((1..=240).contains(&applied));
println!("target fps: {}", render_engine::embedded_target_fps());

// Queue work onto the next frame.
let task_id = render_engine::submit_embedded_task("tick", |frame_index| {
    // Runs once, on frame `frame_index`.
    let _ = frame_index;
});
println!("queued task {task_id}");
```

### How pacing works, and why it is not a detail

The two profiles pace differently, and the difference matters on battery:

- **Non-`mini`** (`embedded` on a host with threads): the loop parks on a condvar
  until the next frame deadline, so the CPU sleeps between frames.
- **`mini`**: there is no second thread to wake the loop, so it **busy-waits on the
  same monotonic clock against the same frame budget**. Without that budget the loop
  would spin at full speed and run every task far more often than the target rate
  asks, which on a device is a thermal and battery problem rather than a cosmetic one.

Either way, the loop re-checks its running flag from inside the wait, so a
`quit()` issued from a task is observed promptly instead of after the next
deadline.

## 4. Reading runtime state

`embedded_engine_stats()` is the diagnostics surface — the numbers a watchdog or a
startup smoke test asserts on:

```rust
use rust_widgets::render_engine;

let stats = render_engine::embedded_engine_stats();
println!("initialised: {}", stats.initialized);
println!("running:     {}", stats.running);
println!("frames:      {}", stats.frame_count);
println!("queued:      {}", stats.pending_task_count);
println!("windows:     {}", stats.window_count);
println!("buttons:     {}", stats.button_count);
println!("target fps:  {}", stats.target_fps);
```

`EmbeddedEngineStats` is `Clone + Debug + PartialEq + Eq`, so it can be compared
directly in a test rather than field-by-field.

Because the engine is a **process-wide singleton** (one paint budget, one registry),
two tests that mutate it must not interleave. Tests inside the crate serialise
through a shared guard for that reason; a host normally only reads.

## 5. Getting pixels onto your panel

This is the part that is genuinely different from a desktop build: there is no
window and no compositor, so **you** own the buffer.

### Allocating a surface

```rust
use rust_widgets::platform::{FrameBuffer, SurfaceGeometry};

// Note the explicit `stride` — see below for why it is not just `width * 4`.
let geometry = SurfaceGeometry { width: 320, height: 240, stride: 320 * 4 + 16 };
let mut frame = FrameBuffer::new();

if !frame.resize(geometry) {
    // A stride narrower than one packed row cannot describe a frame. The call
    // refuses rather than producing a garbled one.
    return;
}
println!("{} bytes", frame.frame().len());
```

`SurfaceGeometry::tight(w, h)` builds a tightly packed geometry when your hardware
has no padding.

### Why `stride` is separate from `width * 4`

Real panels are routinely padded to an alignment, and a GPU backbuffer often is
too. Code that assumed row `n` starts at `n * width * 4` would **shear the image**
on exactly those hosts. Keeping `stride` explicit means the correct thing is
expressible and the wrong thing has to be written deliberately.

### Reshaping a foreign buffer

A driver whose layout differs (padding, or a bottom-up origin) should not force
every widget to learn that layout. `copy_rows` does the one conversion in one
place:

```rust
use rust_widgets::platform::portable::copy_rows;
use rust_widgets::platform::SurfaceGeometry;

let geometry = SurfaceGeometry { width: 2, height: 2, stride: 12 }; // 4 bytes of pad
let src = [1u8; 24];
let mut dst = [0u8; 24];

assert!(copy_rows(&src, &mut dst, geometry));
// Padding bytes are left untouched rather than being written with stale data.
assert_eq!(&dst[8..12], &[0u8; 4]);
```

It is bounds-checked in **both** directions and refuses rather than clipping: a
region that does not fit is an error, because a silently clipped frame is a
rendering bug that hides itself. A zero-sized surface is treated as a valid no-op
request, not a failure.

### The host, with no OS behind it

For `mini` — and for a target with no backend module — the portable host is the
`Platform` implementation:

```rust
use rust_widgets::platform::portable;

let host = portable::instance();     // a StubPlatform: in-memory state, no OS
assert_eq!(host.backend_name(), portable::BACKEND_NAME); // "portable"
assert_eq!(portable::FAMILY, rust_widgets::core::PlatformFamily::Embedded);
```

It reports `PlatformFamily::Embedded` deliberately: reporting `Desktop` would make
adaptive code pick defaults a host with no window manager cannot honour.

## 6. Memory behaviour under `alloc_frugal`

`mini` sets `alloc_frugal`, which selects the frugal allocator paths. Their
behaviour is deliberately **transparent rather than plausible**:

- Arena accounting is a no-op that reports `0` bytes, and its documentation says so
  explicitly — *"Do not use this as a memory-usage measurement."*
- The `Condvar` stub cannot park a caller, and both notify methods do nothing.

A fabricated number would be the kind of report the project forbids, so the stubs
answer honestly instead. **If you need real allocation figures on a constrained
target, take them from your target's own allocator**, not from these stubs.

## 7. What a reduced profile actually removes

Beyond the memory policy, a stripped profile loses:

- **The declarative view layer.** `crate::view` / `crate::json` / `crate::app` are
  gated on `declarative_view` (`device_profile && !stripped && !no-declarative-view`),
  so on `mini`/`embedded` the imperative `add_child` API is what you have. The
  rationale is the allocation budget, plus the fact that no caller re-evaluates a
  view per frame on these targets.
- **The custom-painted widget surface**, and it says so: `supports_surfaces()`
  returns `false` rather than mounting a surface and handing back a blank window.
- **The platform runtime** (as covered in §2).

Menus and shortcuts are deliberately *not* affected — their code carries no `mini`
gate — so a `mini` build is best described as "no custom-painted widget surface,
but fully working menus".

## 8. Checklist

- [ ] Pick **one** profile; verify with `--no-default-features --features <profile>`.
- [ ] `cargo check --no-default-features --features mini --all-targets` for warnings too.
- [ ] Ask `profile::*` and `supports_surfaces()` before mounting anything.
- [ ] Set your target FPS and read back the **applied** value.
- [ ] Provide a surface with an accurate `stride`.
- [ ] Use `copy_rows` rather than hand-rolling a row copy for a padded buffer.
- [ ] Expect honest no-ops under `alloc_frugal`; take real memory numbers from your
      own allocator.
- [ ] Assert on `embedded_engine_stats()` in a startup smoke test.
