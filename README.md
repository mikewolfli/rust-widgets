# rust_widgets — Pure Rust GUI Library

<p align="center">
  <img src="snapshots/header.jpg" alt="rust_widgets" width="800">
</p>

A cross-platform GUI library written in pure Rust. It paints **every control itself** — there is no
`CreateWindowExW`, `NSButton`, `gtk_button_new` or `android.widget.Button` anywhere in the crate — and
it can render to a window, to a PNG, or to SVG. Desktop, tablet, mobile, embedded and a minimal
`mini` profile are all supported.

Why self-drawn controls are worth the effort:

| | Self-drawn (this library) | Native controls |
|---|---|---|
| Appearance | Identical on every OS | Differs per toolkit and version |
| Control count | 180 kinds everywhere | Only what the toolkit offers |
| Dependencies | No GUI toolkit linked | GTK / AppKit / Win32 / Android SDK |
| Headless / embedded | Runs with no OS at all (`mini`, SVG) | Impossible |
| Tests | Pixel and SVG snapshots | Needs a real display |

A backend still owns the parts that genuinely belong to the operating system — window creation and the
event loop, input translation, and platform services (IME, clipboard, file dialogs, DPI). If a backend
cannot even supply a surface (a bare framebuffer, for instance), the library paints into an in-memory
buffer instead. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Install

```toml
[dependencies]
rust_widgets = "2.7.0"
```

Pick **exactly one device profile**. They are mutually exclusive — `mini` and `embedded` compile parts
of the crate *out*, so combining one with `desktop` is not a lowest common denominator, it is a broken
build:

```toml
rust_widgets = { version = "2.7.0", features = ["desktop"] }                       # default
rust_widgets = { version = "2.7.0", default-features = false, features = ["tablet"] }
rust_widgets = { version = "2.7.0", default-features = false, features = ["mobile"] }
rust_widgets = { version = "2.7.0", default-features = false, features = ["embedded"] }
rust_widgets = { version = "2.7.0", default-features = false, features = ["mini"] }
```

> `cargo check --features embedded` is **wrong**: `desktop` is a default feature, so that command
> enables two mutually exclusive profiles at once. Always pass `--no-default-features` when selecting
> any profile other than `desktop`.

## Hello, control

```rust
use rust_widgets::core::{Color, Rect};
use rust_widgets::style::WidgetStyle;
use rust_widgets::widget::base_widgets::button::Button;
use rust_widgets::widget::Widget;   // brings `set_style` into scope

fn main() {
    // A control is a value placed in a `Rect`. Rendering it to SVG is the shortest path
    // to seeing it; a backend only changes where the pixels go, not how the control draws.
    let mut button = Button::new("Click me".to_string(), Rect::new(0, 0, 160, 36));

    // Style is a plain struct of optional values, so an unset field inherits instead of
    // overwriting what the theme resolved for this control.
    let style = WidgetStyle {
        background_color: Some(Color::rgb(33, 150, 243)),
        text_color: Some(Color::WHITE),
        border_radius: Some(6),
        ..WidgetStyle::default()
    };
    button.set_style(style);

    println!("{}", rust_widgets::widget::svg::render_to_svg(&mut button));
}
```

Window creation, the event loop, layout, theming, i18n and the C ABI are each covered in their own
chapter — start at [`cookbook/en/src/README.md`](cookbook/en/src/README.md).

## Rendering

| Backend | Use | Notes |
|---|---|---|
| Software rasteriser | Default everywhere | Anti-aliased, no GPU required, runs headless |
| SVG | Snapshots, printing, vector output | Byte-reproducible; this is what `snapshots/svg/` holds |
| wgpu | GPU acceleration | Explicit fallback ladder: Vulkan/Metal/DX12/WebGPU → GL → CPU |

The renderer's text origin is the glyph box's **top-left**, not a baseline. Both backends honour that
contract identically — the SVG emitter writes `dominant-baseline="text-before-edge"` — so a label
measured against one backend lands in the same place in the other. That property is what makes
`snapshots/svg/` a usable review artifact rather than a second, divergent renderer.

## Sizing a control

A control is given a rectangle by whatever placed it, and that rectangle is the area it **may**
occupy — not how big it should draw. Those are two questions, and answering the second one is what
stops a switch from being painted as a 240x120 stadium:

```rust
implicit_size = max(floor, content + padding)
```

The `max` is the load-bearing part: the **floor is a minimum tappable area**, so a button labelled
with 5 px of text is still `64x40` rather than `64x18`. `rust_widgets::widget::ControlMetrics`
exposes that formula and the geometry helpers built on it, and
`rust_widgets::widget::metrics::dimensions` holds every control dimension (track sizes, thumb
radii, field heights, paddings) so one fact has one definition:

| helper | the question it answers |
|---|---|
| `implicit_size` / `content_box` | how big should I be? / where may my content go? |
| `center_in` / `centered_disc` / `centered_square` | centre fixed chrome, clamped never expanded |
| `centered_band` / `full_width_band` | full width, my height, vertically centred |
| `top_band` / `bottom_band` / `band_inset` | pin a bar to an edge and let content follow |
| `focus_ring_rect` / `focus_ring_color` | the keyboard ring, inset so it never overlaps a neighbour |

## Laying out a control

A layout **asks each child how big it wants to be** and acts on the answer, rather than being told in
advance. Each control states its wish as a floor, a preferred value and a ceiling, because "how small
may I squeeze you" and "how large may I stretch you" are questions a single size cannot answer:

```rust
use rust_widgets::layout::{AxisHints, Hints, LayoutParams, ChildInfo, Layout};

// 120 px wide minimum, would like 200, never past 400; free to stretch horizontally.
let hints = Hints { width: AxisHints::new(120, 200, 400), height: AxisHints::fixed(32) };
let children = [ChildInfo::new(widget_id, hints).with_params(LayoutParams::filled())];

let mut out = Vec::new();
layout.arrange(rect, &children, &mut |id, child_rect| out.push((id, child_rect)));
```

`fill` is separate from the size on purpose: a slider and a button can share a preferred size while
only one of them should be stretched across a form. `AxisHints::new` normalises `min <= pref <= max`
on construction, so an unnormalised hint cannot be represented. Every existing layout keeps working
unchanged — `Layout::arrange` defaults to forwarding to `Layout::update`.

An example is committed at [`examples/readme_check.rs`](examples/readme_check.rs), so these snippets
are compiled on every build rather than drifting from the API.

## Control behaviour

Three contracts that a control's shape alone does not communicate:

- **`clicked` requires the release to land inside the control.** A drag that starts on a button and
  ends off it emits `canceled` instead, and only the primary button activates.
- **`pressed` is a continuous quantity.** Dragging off a held button clears it and dragging back
  restores it, so the control reflects where the pointer actually is.
- **A focus ring is drawn when the user is on the keyboard, not merely when the control has focus.**
  `Event::FocusGained` carries a `FocusReason`, and `FocusReason::draws_focus_ring()` is `false` for a
  pointer press — a ring under the cursor reads as a stuck highlight.

## Verifying a change

```bash
cargo test --no-default-features --features desktop            # full suite
cargo clippy --no-default-features --features desktop --all-targets -- -D warnings
cargo run  --no-default-features --features desktop --example export_control_svgs
bash tools/run_all_gates.sh                                    # every gate, PASS/FAIL table
```

The 376 SVGs under [`snapshots/svg/`](snapshots/svg/) are one file per control per appearance (dark and
light). They are **committed and regenerated**, and `tools/check_svg_snapshots.sh` fails byte-for-byte
if a control's drawing changed without them being updated. So a wrong-looking control shows up as a
diff in review, and a control whose two files are identical is visibly theme-blind.

[`control.md`](control.md) is the same set laid out for reading: every control's dark and light
snapshot, grouped into 17 families by the module that implements it. It is generated by
`tools/generate_control_index.py` from the registry and the source tree, so it is a *view* of the
snapshots rather than a second copy of them — a control added to the registry appears in the gallery
without anyone editing it. The same gate that regenerates the snapshots also regenerates this page
and refuses a stale copy, so the two cannot drift.

`tools/run_all_gates.sh` runs everything in `tools/check_*.sh` and prints a per-gate table with
timings. Every gate is expected to be able to fail; the ones that matter most were verified by
reverse injection — deliberately reintroducing the defect and confirming the gate goes red.

## What "180 controls" covers

The widget library spans text and inputs (button, toggle, entry fields, masked/OTP/date/time editors,
search, tag input, keyboard, rich text, markdown, code editor, terminal), selection and display (lists,
tables and virtualised grids with frozen columns, trees, chips, badges, ratings, progress, skeleton
loaders), containers and chrome (tabs, splitters, dock panels, MDI, toolbars, menus, status bar, ribbon,
toolbox), dialogs and overlays (message box, file/font/colour pickers, wizard, popover, tooltip,
banner, toast, snackbar, bottom sheet), navigation, media, and a chart family that Material has no
equivalent for: candlestick, volume, depth, indicator, radar, heatmap, gauge, sparkline.

Profiles that strip the widget set down (`mini`, `embedded`) keep the reduced core; the exact set per
profile is generated and gated, and listed in
[`docs/plans/platform_capability_matrix.md`](docs/plans/platform_capability_matrix.md).

## Platform notes

- **Windows / macOS / Linux** — full profile, real surface and event loop.
- **Linux/Wayland** — composition support has host tests under `tools/run_wayland_compositor_tests.sh`.
- **iOS / Android** — full profile; the JNI test APK is built with `tools/build_android_testapp.sh`.
- **Web (wasm32)** — WebGL/WebGPU surface; `cfg(target_arch = "wasm32")` describes the sandbox, not an
  OS, and the OS-specific knowledge stays inside `src/platform/`.
- **Embedded / mini** — software rasteriser only, no OS services.

Anything that cannot genuinely be done yet reports `false` from a runtime capability query rather than
pretending. `supports_custom_widgets()`, `supports_web_engine()` and `has_real_engine()` are honest
answers, not compile-time stubs.

## Text coverage

**The default build draws Latin/ASCII only.** The crate ships no font data, so glyphs come from a
fixed 8x8 bitmap face covering `U+0000`–`U+007F`.

| Input | What the default build draws |
|---|---|
| `A`, `z`, `7`, `!` | the real glyph |
| CJK, Cyrillic, Arabic, emoji, any other script | a **fallback glyph** (a hollow box, "tofu") |

The label still lays out and the control still renders — only the glyph is wrong. A line is also
ordered for painting by the **Unicode bidirectional algorithm**, so an Arabic or Hebrew run draws
right to left. Ordering is not glyph *shape*: script-specific shaping (Arabic joining, Indic
reordering) is not applied at all.

This library therefore does **not** implement Unicode text rendering and is not described as
multilingual. Coverage beyond Latin/ASCII is a separate, opt-in axis that needs font data — and,
for complex scripts, a shaping step. It is asked for by name, because a `mini` or `embedded`
profile must not carry glyphs it never draws:

```console
cargo build --features "desktop,fonts-cjk-bitmap"
```

`fonts-cjk-bitmap` adds a generated 16x16 CJK face (Han, kana, CJK punctuation and fullwidth
forms, ~85 KB read on demand). Latin rendering stays byte-identical when it is on: a face is
appended to the fallback stack and can only answer for characters the base face has no glyph for.

| feature | data | what it adds |
|---|---|---|
| `fonts-cjk-bitmap` | 84 996 bytes, generated | a 16x16 CJK bitmap face — Chinese, Japanese, Korean text, no shaping needed |
| `fonts-vector-latin` | 35 896 bytes, OFL subset | real advances and kerning, from a face named by `Font::family` |
| `fonts-complex` | 70 576 bytes, OFL subset | Arabic joining, so `بيت` shapes to the word rather than three isolated letters |

None of the three is enabled by any profile, and `--all-features` is the only way to get all of them
at once. The two vector subsets are lazy-loaded from the binary's read-only section and are recorded,
with their upstream digests, in [`NOTICE`](NOTICE).

The boundary is asserted from both sides (`render::pipeline::pixel_ops::text_coverage_tests` and
`render::text::glyph_source`'s tests) — a non-Latin character must resolve to the fallback glyph on
a default build, and enabling the CJK data must move that boundary by exactly the face it adds — so
the documentation cannot silently overstate what is drawn.

## Language bindings

The `C ABI` lives in `src/bindings/` and exposes every control through **130 `rw_*` functions** with a
capability-based property and event model. C, C++, Python and Java (JNI) bindings are exercised in CI;
the generated header is checked for drift by `tools/check_abi.sh`.

See [`cookbook/en/src/chapters/language-bindings.md`](cookbook/en/src/chapters/language-bindings.md).

## Documentation

| Document | Contents |
|---|---|
| [`cookbook/`](cookbook/) | The handbook — English, 简体中文, 繁體中文 |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Module layout and layering rules |
| [`docs/MIGRATION_GUIDE.md`](docs/MIGRATION_GUIDE.md) | Migrating from 1.x (native controls were removed in 2.0) |
| [`CHANGELOG.md`](CHANGELOG.md) | Release notes with the evidence for each fix |
| [`docs/reports/`](docs/reports/) | Audit and quality reports |
| [`docs/log/`](docs/log/) | Per-round engineering logs |

## Requirements

Rust **1.87+**. No system GUI libraries are needed for the default build; Linux additionally uses
Wayland/X11 for the surface, and the image codecs are pure Rust so cross-compiling to Android, iOS or
wasm needs no `pkg-config` sysroot.

## License

MIT — see [LICENSE](LICENSE).

## Support

- Issues: [GitHub Issues](https://github.com/mikewolfli/rust-widgets/issues)

[![build](https://img.shields.io/badge/build-passing-brightgreen)]()
[![version](https://img.shields.io/badge/version-2.7.0-blue)]()
[![tests](https://img.shields.io/badge/tests-5600%2B-brightgreen)]()
[![license](https://img.shields.io/badge/license-MIT-blue)]()

<p align="center">
  <a href="README.zh-CN.md">
    <img src="https://img.shields.io/badge/%E4%B8%AD%E6%96%87-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-blue" alt="简体中文">
  </a>
</p>
