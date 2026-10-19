# Optional platform backends — build boundaries (BLUE28 B28.33–B28.36)

> Generated from the 2026-10-19 audit. These four capabilities are **conditional**: they are not
> part of `desktop`/`tablet`/`mobile` by default, and a target without the native library reports
> an honest absence instead of being silently compiled out.

## Audio output (`audio-output`)

| target | backend | status |
|---|---|---|
| Windows | WASAPI (via `cpal`) | feature-gated, not in any profile |
| macOS | CoreAudio (via `cpal`) | feature-gated |
| Linux/BSD | ALSA / PulseAudio (via `cpal`) | feature-gated |
| Android | AAudio / OpenSL ES (via `cpal`) | not verified in this tree |
| iOS/tvOS/visionOS | CoreAudio (via `cpal`) | not verified in this tree |
| HarmonyOS / Redox / WASM | no `cpal` backend | **unsupported**; do not enable the feature |

The profiles deliberately omit `audio-output` (`CHANGELOG` and `Cargo.toml` record the reason:
large system dependencies). A host that enables it is responsible for the platform library;
`cargo check` with the feature is the verification boundary.

## Resource watching (`AssetWatcher`, `notify`)

`notify` supports the desktop OSes and BSD through inotify/kqueue/FSEvents/ReadDirectoryChangesW.
Android, iOS, HarmonyOS and WASM do not have a filesystem-notification API with the same
semantics; `AssetWatcher` must report "not observable" there rather than start a thread that never
emits. A mobile host that needs hot reload should ship assets in the bundle and rebuild on a
configuration change.

## GPU (`gpu` / `wgpu`)

`wgpu` 30 is the dependency. The adapters are selected through the explicit degradation ladder
(`PRIMARY` → `GL` → CPU fallback) and `GpuBackendTier` reports which rung was used. The backends
themselves are:

| target | wgpu backend | verified here |
|---|---|---|
| Windows | DX12 / Vulkan / GL | cross-check only |
| macOS | Metal | cross-check only |
| Linux | Vulkan / GL | cross-check only |
| Android | Vulkan / GLES | cross-check only |
| iOS/tvOS/visionOS | Metal | cross-check only |
| WASM | WebGPU / WebGL | `src/gpu/adapter.rs` has the browser path |
| Harmony / Redox / BSD | not verified | capability must report the software fallback |

A target where `wgpu` cannot create an adapter must fall back to the software rasteriser and say
so; it must not fail the whole profile.

## Video (`video-codecs`)

`video-codecs` links the **system** `ffmpeg-next`. A target without FFmpeg headers/libraries
cannot build that feature; the supported build boundary is "the host has FFmpeg installed", and the
platform capability reports `false` on targets where it is not. Mobile and WASM do not ship
FFmpeg in this tree, so the feature is not part of their profiles.

## Print (`print`)

The desktop backends submit through the OS spooler (`lp`/`lpr`, `Start-Process -Verb Print`), and
iOS uses `UIPrintInteractionController` (BLUE28 B28.27). Android submits through
`PrintManager.print` with a `PrintDocumentAdapter` implemented by the shipped Java host class
(`rust_widgets.RustWidgets.printFile`): the adapter streams the job file's bytes to the system
print destination, and `has_print_support` is true exactly when the JNI bridge holds the Activity
context that call needs (BLUE28 B28.27).

## Verification commands

```bash
# The default profiles do not carry these features:
cargo check --no-default-features --features desktop
cargo check --no-default-features --features tablet
cargo check --no-default-features --features mobile

# A host that wants one asks for it explicitly and supplies the system library:
cargo check --no-default-features --features "desktop,audio-output"
cargo check --no-default-features --features "desktop,video-codecs"
```
