# rust_widgets — Pure Rust GUI Library

<p align="center">
  <img src="snapshots/header.jpg" alt="rust_widgets" width="800">
</p>

A cross-platform GUI library written in pure Rust that paints **every control itself**. There is no
`NSButton`, no `gtk_button_new`, no `android.widget.Button` — a test greps the source for those and
fails if one appears. The only `CreateWindowExW` in the crate is the one that creates the window.

It renders to a live window, to a PNG, or to SVG, and it runs where a GUI toolkit cannot: `mini` and
`embedded` need no OS at all.

| | Self-drawn (this library) | Native controls |
|---|---|---|
| Appearance | Identical on every OS | Differs per toolkit *and* per toolkit version |
| Controls | **181 kinds** everywhere | Only the ones the toolkit ships |
| Dependencies | No GUI toolkit linked | GTK / AppKit / Win32 / Android SDK |
| Headless, embedded | Runs with no OS (`mini`, SVG) | Not possible |
| Tests | Byte-reproducible SVG and pixel snapshots | Needs a real display |

The backend keeps only what genuinely belongs to the operating system: window creation, the event
loop, input translation, and services such as IME, clipboard, file dialogs and DPI. If a backend
cannot even offer a surface, the library paints into an in-memory buffer and carries on. See
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Install

```toml
[dependencies]
rust_widgets = "2.8.9"
```

Choose **exactly one device profile**. They are mutually exclusive: `mini` and `embedded` compile
parts of the crate *out*, so pairing one with `desktop` is not a lowest common denominator — it is a
build that cannot work.

```toml
rust_widgets = { version = "2.8.9", features = ["desktop"] }                       # default
rust_widgets = { version = "2.8.9", default-features = false, features = ["tablet"] }
rust_widgets = { version = "2.8.9", default-features = false, features = ["mobile"] }
rust_widgets = { version = "2.8.9", default-features = false, features = ["embedded"] }
rust_widgets = { version = "2.8.9", default-features = false, features = ["mini"] }
```

> `cargo check --features embedded` is **wrong**, and quietly so: `desktop` is a default feature, so
> that command switches on two mutually exclusive profiles at once. Pass `--no-default-features`
> whenever you select anything other than `desktop`.

## Hello, control

```rust
use rust_widgets::core::{Color, Rect};
use rust_widgets::style::WidgetStyle;
use rust_widgets::widget::base_widgets::button::Button;
use rust_widgets::widget::Widget;   // brings `set_style` into scope

fn main() {
    // A control is a value placed in a `Rect`. Rendering it to SVG is the shortest way to see
    // it; a backend only changes where the pixels go, not how the control draws.
    let mut button = Button::new("Click me".to_string(), Rect::new(0, 0, 160, 36));

    // Style is a struct of optional values, so an unset field inherits rather than overwriting
    // what the theme resolved for this control.
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

Window creation, the event loop, layout, theming, i18n and the C ABI each have their own chapter —
start at [`cookbook/en/src/README.md`](cookbook/en/src/README.md).

## Rendering

| Backend | Use | Notes |
|---|---|---|
| Software rasteriser | Default everywhere | Anti-aliased, no GPU, runs headless |
| SVG | Snapshots, printing, vector output | Byte-reproducible; this is what `snapshots/svg/` holds |
| wgpu | GPU acceleration | Explicit ladder: Vulkan/Metal/DX12/WebGPU → GL → CPU |

The renderer's text origin is the glyph box's **top-left**, not a baseline. Both backends honour that
one contract — the SVG emitter writes `dominant-baseline="text-before-edge"` — so a label measured
against one lands in the same place in the other. That is the whole reason `snapshots/svg/` is usable
as review material instead of being a second renderer that quietly disagrees.

## Sizing, layout, behaviour

A control is handed a rectangle by whatever placed it, and that rectangle is the area it **may**
occupy — not how big it should draw. Keeping those two questions apart is what stops a switch from
being painted as a 240x120 stadium:

```text
implicit_size = max(floor, content + padding)
```

The `max` is load-bearing: the **floor is a minimum tappable area**, so a button labelled with 5 px of
text is still 64x40, not 64x18. `widget::ControlMetrics` exposes that formula and the helpers built on
it; `widget::metrics::dimensions` holds every control dimension, so one fact has one definition.

Layout **asks each child how big it wants to be** rather than being told in advance. A child answers
with a floor, a preferred value and a ceiling, because "how small may I be squeezed" and "how large
may I be stretched" cannot both be expressed by one number. `fill` is separate from the size on
purpose: a slider and a button may share a preferred size while only one should stretch across a form.
`AxisHints::new` normalises `min <= pref <= max` on construction, so an unnormalised hint cannot exist.

Behaviour is three contracts a control's shape does not communicate:

- **`clicked` requires the release to land inside the control.** A drag that starts on a button and
  ends off it emits `canceled`, and only the primary button activates.
- **`pressed` is continuous.** Dragging off a held button clears it; dragging back restores it.
- **A focus ring means the user is on the keyboard, not merely that the control has focus.**
  `FocusReason::draws_focus_ring()` is `false` for a pointer press — a ring under the cursor reads as a
  stuck highlight.

## Theming, animation, disabled states

Colours resolve through one ladder — the control's explicit style, then the theme's resolved style for
that control, then a literal as the last resort — so an untouched control still follows an appearance
switch while a caller's deliberate colour always wins. `WidgetState` re-resolves that ladder on
`hover`/`pressed`/`disabled`, which is how a theme author's `"button:hover"` key reaches the paint code.

Animation is one stored-target `PropertyDriver` (a value that travels between `0.0` and `1.0`, priced
by a `MotionSlot` tempo token) surfaced through `Widget::tick` / `Widget::is_animating`, so the frame
loop discovers a control owes frames without every control registering itself somewhere.
`set_animating` is deliberately not a thing you toggle: `is_animating()` is derived, so the control and
the frame loop cannot disagree about whether anything is moving.

Disabled is **two** moves, named separately because using one where the other belongs produces the
opposite of the intended state:

| | recedes by stepping toward | measured |
|---|---|---|
| `disabled_ink_on` — text, icons | the surface's own contrast colour | 4.57–5.91:1 |
| `disabled_surface_near` — fills, panels | the page behind them | always less prominent than enabled |

Both weights come from `dimensions::DISABLED_VEIL_ALPHA` (`0.55`), the smallest value that clears the
4.5:1 body-text floor on **both** appearances. A half-transparent mid-grey has no direction: it darkens
over a light surface and lightens over a dark one, so "disabled" would read as *more* prominent on
whichever appearance was already hardest to read.

## Text coverage

**The default build draws Latin/ASCII only.** No font data ships with the crate, so glyphs come from a
fixed 8x8 bitmap face covering `U+0000`–`U+007F`.

| Input | What the default build draws |
|---|---|
| `A`, `z`, `7`, `!` | the real glyph |
| CJK, Cyrillic, Arabic, emoji, anything else | a **fallback glyph** — a hollow box |

The label still lays out and the control still renders; only the glyph is wrong. Lines are ordered for
painting by the Unicode bidirectional algorithm, so an Arabic run draws right to left — but ordering is
not shape, and script-specific shaping (Arabic joining, Indic reordering) is not applied at all.

This library therefore does **not** implement Unicode text rendering and is not described as
multilingual. Coverage beyond Latin/ASCII is a separate opt-in axis that needs font data, asked for by
name, because a `mini` build must not carry glyphs it will never draw:

| feature | data | what it adds |
|---|---|---|
| `fonts-cjk-bitmap` | 84 996 bytes, generated | 16x16 CJK bitmap face — Chinese, Japanese, Korean, no shaping needed |
| `fonts-cjk` | 361 704 bytes, OFL subset | the same script as **outlines**: antialiased, any size |
| `fonts-vector-latin` | 35 896 bytes, OFL subset | real advances and kerning |
| `fonts-complex` | 70 576 bytes, OFL subset | Arabic joining, so `بيت` is a word and not three letters |
| `fonts-emoji-color` | 1 602 492 bytes, OFL subset | colour emoji, 317 codepoints including the 26 regional indicators |

None is enabled by any profile. The vector and colour subsets are lazy-loaded from the binary's
read-only section and recorded, with upstream digests, in [`NOTICE`](NOTICE).

## Icons

`Icon` ships **69** `IconName` tokens. With `icons` (**on by default**) each token draws the real
**Material Symbols** outline; with the feature off it draws generated fallback geometry derived from
the *same* outlines, so the two paths cannot describe different shapes for one icon.

```console
cargo build                                            # the real outlines (default)
cargo build --no-default-features --features desktop  # the derived fallback shapes
```

`IconName::data()` answers `IconData`, not `Option`, so adding a token without geometry is a compile
error rather than a blank spot on the screen. The token set is declared once in `tools/icon_tokens.txt`
and `tools/gen_icon_names.py` generates the type from it: adding an icon is a two-line edit plus a
generator run, not the six-file chore it used to be.

A host can register its **own** icons at runtime; the built-in set is a closed vocabulary, and
`register_icon` is the open extension point beside it:

```rust
use rust_widgets::widget::register_icon;

// SVG path data on the 960-unit design grid, negative y upward (the Material Symbols convention).
assert!(register_icon("disclosure", &["M480-200 240-440l480 480-240-240Z"]));
icon.set_icon("disclosure");
```

`register_icon_on_grid` takes a grid argument for a source on another scale (24 units, as Lucide and
Tabler use). The icon data is Apache-2.0 (Google LLC); attribution and the licence gate are in
[`NOTICE`](NOTICE) and `tools/check_icon_licences.sh`.

The feature is deliberately **not** in any device profile: `mini` and `embedded` are *sized*, and a
payload the caller did not ask for is wrong there. Ask for it — `--features mini,icons` — if you want it.

## Verifying a change

```bash
cargo fmt --all -- --check
cargo check --all-targets --no-default-features --features desktop
cargo clippy --all-targets --no-default-features --features desktop -- -D warnings
cargo test  --no-default-features --features desktop -q
cargo test  --no-default-features --features "desktop,icons" -q   # the opt-in icon data

for profile in full embedded mini tablet mobile; do                # the rest of the matrix
  cargo test --no-default-features --features "$profile" -q
done
for profile in desktop tablet mobile mini embedded; do             # build-only, every profile
  cargo check --no-default-features --features "$profile"
done

bash tools/run_all_gates.sh   # 111 gates: source, contract and platform checks, with a PASS/FAIL table
```

Do not reach for `--all-features` instead: `desktop` and `mini` are mutually exclusive and `mini`
switches the crate to `no_std`, so it is not a substitute for the matrix. The gate runner can report
host-limited checks as skipped; native GTK, Apple and Android tests run in platform-specific CI jobs.

The SVGs under [`snapshots/svg/`](snapshots/svg/) are one file per control per appearance. They are
**committed and regenerated**, and `tools/check_svg_snapshots.sh` fails byte-for-byte if a control's
drawing changed without them being updated — so a wrong-looking control appears as a reviewable diff,
and a control whose two files are identical is visibly theme-blind. [`control.md`](control.md) is the
same set laid out for reading, generated from the registry so a new control shows up without anyone
editing the gallery.

A snapshot is not always the right evidence: it shows the **resting** state, so a defect on a hover,
disabled or animating path is invisible to it. Those are pinned by pixel probes
(`tests/disabled_text_contrast.rs`, `tests/disabled_surface_probe.rs`, `tests/m3_animation_probe.rs`)
that assert the promised quantity — a contrast ratio, a painted height, a segment count — rather than a
proxy like a byte length.

Every gate is expected to be *able to fail*; the ones that matter were verified by reverse injection,
reintroducing the defect and confirming the gate turns red. A gate that cannot fail is a decoration,
and this repository does not collect those.

## What the 181 controls cover

Text and inputs (button, toggle, entry fields, masked/OTP/date/time editors, search, tag input,
keyboard, rich text, markdown, code editor, terminal); selection and display (lists, tables,
virtualised grids with frozen columns, trees, chips, badges, ratings, progress, skeleton loaders);
containers and chrome (tabs, splitters, dock panels, MDI, toolbars, menus, status bar, ribbon,
toolbox); dialogs and overlays (message box, file/font/colour pickers, wizard, popover, tooltip,
banner, toast, snackbar, bottom sheet); navigation; media; and a chart family Material has no
equivalent for — candlestick, volume, depth, indicator, radar, heatmap, gauge, sparkline.

`mini` and `embedded` keep a reduced core. The exact set per profile is generated and gated, and listed
in [`docs/plans/platform_capability_matrix.md`](docs/plans/platform_capability_matrix.md).

## Platform notes

- **Windows · macOS · Linux** — full profile, real surface and event loop.
- **Linux/Wayland** — composition tests live in `tools/run_wayland_compositor_tests.sh`.
- **iOS · Android** — full profile; the JNI test APK builds with `tools/build_android_testapp.sh`.
- **Web (wasm32)** — WebGL/WebGPU surface. `cfg(target_arch = "wasm32")` describes a sandbox, not an
  OS, and all OS knowledge stays inside `src/platform/`.
- **Embedded · mini** — software rasteriser only, no OS services.

Anything that genuinely cannot be done yet says so: `supports_custom_widgets()`,
`supports_web_engine()` and `has_real_engine()` are runtime answers, not compile-time stubs pretending
to be features.

## Language bindings

The C ABI in `src/bindings/` exposes every control through **152 `rw_*` functions** with a
capability-based property and event model. C, C++, Python, Java (JNI) and Node bindings are exercised
in CI, and the generated header is checked for drift by `tools/check_abi.sh`. Start with
[`cookbook/en/src/chapters/language-bindings.md`](cookbook/en/src/chapters/language-bindings.md).

## Documentation

| Document | Contents |
|---|---|
| [`cookbook/`](cookbook/) | The handbook — English, 简体中文, 繁體中文 |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Module layout and layering rules |
| [`docs/MIGRATION_GUIDE.md`](docs/MIGRATION_GUIDE.md) | Migrating from 1.x (native controls were removed in 2.0) |
| [`CHANGELOG.md`](CHANGELOG.md) | Release notes, with the evidence for each fix |
| [`docs/plans/`](docs/plans/) | Design plans and the engineering principles behind them |
| [`docs/log/`](docs/log/) | Per-round engineering logs |
| [`docs/reports/`](docs/reports/) | Audit and quality reports |

## Requirements

Rust **1.88+**. The default build needs no system GUI libraries; on Linux the surface uses
Wayland/X11. Image codecs are pure Rust, so cross-compiling to Android, iOS or wasm needs no
`pkg-config` sysroot.

## License

MIT — see [LICENSE](LICENSE).

## Support

Issues: [GitHub Issues](https://github.com/mikewolfli/rust-widgets/issues)

[![build](https://img.shields.io/badge/build-passing-brightgreen)]()
[![version](https://img.shields.io/badge/version-2.8.9-blue)]()
[![tests](https://img.shields.io/badge/tests-7600%2B-brightgreen)]()
[![license](https://img.shields.io/badge/license-MIT-blue)]()
[![controls](https://img.shields.io/badge/controls-181-blue)]()

<p align="center">
  <a href="README.zh-CN.md">
    <img src="https://img.shields.io/badge/%E4%B8%AD%E6%96%87-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-blue" alt="简体中文">
  </a>
</p>
