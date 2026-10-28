# rust_widgets — Pure Rust GUI Library

<p align="center">
  <img src="snapshots/header.jpg" alt="rust_widgets" width="800">
</p>

`rust_widgets` is a cross-platform GUI library written in pure Rust. It paints **every control
itself** — there is no `NSButton`, no `gtk_button_new`, no `android.widget.Button`. The OS backend
keeps only what only the OS can provide: a window, the event loop, input translation, IME,
clipboard, file dialogs and DPI. Everything above that is the same code on every target, so the
same widget tree looks and behaves the same everywhere.

It renders into a live window, a PNG, or SVG, and the `mini` / `embedded` profiles run with no OS
services at all.

## Highlights

- **228 widget kinds** — inputs, tables, trees, docks, dialogs, media, charts, editors and more,
  with one implementation shared by every platform.
- **Cascading menus and real session state** — dropdowns, menu buttons and menus open up to four
  nested submenu levels, and controls opt in to `SaveState` so a host can restore their user state
  with explicit, restart-stable keys.
- **Reproducible output** — SVG and pixel snapshots are committed per control and per appearance,
  so a visual change is a reviewable diff rather than a screenshot in a chat.
- **Honest capabilities** — a service the host cannot provide returns `false` / `None`; it is
  never simulated and then reported as success.
- **Extensible at runtime** — register your own icons and font faces, or drive the whole library
  from C, C++, Python, Java or Node through the stable C ABI.
- **Small profiles** — the same widgets compile for a full desktop, a phone, a browser tab, or a
  no-OS target with a software rasteriser.

## Install

```toml
[dependencies]
rust_widgets = "2.9.4"
```

Choose **exactly one** device profile. They are mutually exclusive: `mini` and `embedded` compile
parts of the crate *out*, so pairing one with `desktop` is not a lowest common denominator — it is
a build that cannot work.

```toml
rust_widgets = { version = "2.9.4", features = ["desktop"] }                       # default
rust_widgets = { version = "2.9.4", default-features = false, features = ["tablet"] }
rust_widgets = { version = "2.9.4", default-features = false, features = ["mobile"] }
rust_widgets = { version = "2.9.4", default-features = false, features = ["embedded"] }
rust_widgets = { version = "2.9.4", default-features = false, features = ["mini"] }
```

| Profile | Intended for |
|---|---|
| `desktop` | Windows, macOS, Linux/BSD — native surface, wgpu, print/PDF, charts, accessibility, bundled text |
| `tablet` | Touch-first devices — wgpu, native controls, bundled text, no desktop extras |
| `mobile` | iOS, Android, HarmonyOS — touch, wgpu, mobile platform services |
| `embedded` | Software rasteriser only, no OS services |
| `mini` | The smallest core, `no_std`-oriented |

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

## How it fits together

A control is handed a rectangle by whatever placed it, and that rectangle is the area it **may**
occupy, not how big it should draw. Its preferred size is a floor (a minimum tappable area), a
content size, and a ceiling; layout asks each child for those three answers instead of guessing.
`ControlMetrics` exposes the formula and `widget::metrics::dimensions` holds every dimension, so a
fact has one definition.

Colours resolve through one ladder: the control's explicit style, then the theme's resolved style
for that control, then a literal as the last resort. An untouched control follows an appearance
switch; a caller's deliberate colour always wins.

Behaviour is explicit rather than incidental: `clicked` requires the release to land inside the
control, `pressed` is continuous while a drag stays on it, and a focus ring is drawn only when the
user is actually on the keyboard.

## Text, fonts and icons

- The `desktop`, `tablet` and `mobile` profiles include OFL subsets for Latin, complex scripts and
  CJK; `desktop` additionally includes colour emoji. `mini` and `embedded` carry no font data and
  draw a built-in 8x8 bitmap face covering ASCII.
- Text quality is feature-driven: `fonts-vector-latin`, `fonts-complex`, `fonts-cjk`,
  `fonts-cjk-bitmap` and `fonts-emoji-color` can be enabled independently. The font payloads'
  licence record is [`crates/rust-widgets-font/NOTICE`](crates/rust-widgets-font/NOTICE); the root
  `NOTICE` records the non-font material.
- Fonts can be supplied at runtime with `render::text::register_face` (feature `runtime-fonts`).
  Installed system fonts are enumerated through CoreText on Apple targets, DirectWrite on Windows,
  fontconfig on Linux/BSD and `/system/fonts` on Android. `FontDialog` and
  `FontComboBox::load_system_fonts` use one process-wide snapshot, and a chosen family is
  registered from its backend file when one is available.
- The compiled `fonts-cjk` face is a **subset**: ASCII, CJK punctuation, kana, fullwidth forms and
  the first 1000 Han codepoints. A host whose UI text goes past that subset calls
  `render::text::register_system_cjk_font()` (feature `runtime-fonts`) once at startup: it tries
  the common Chinese families in order, registers the file backing the first one installed, and
  returns its name. `None` means no CJK face was found, and the missing glyphs will be tofu —
  the choice is the host's, because which family answers is a typography decision.
- `CodeEditor` lays text out from the **measured** advances of the face that paints it, not from
  a fixed cell width: the compiled Latin face is proportional (Open Sans), so `column × cell` was
  never where a glyph landed. Row height is `font_size × line_height_ratio` (default `1.5`, Zed's
  `Standard` is `1.3` and `Comfortable` `1.618`); `line_advance(0.0)` means "derive it". Its
  pointer is an I-beam over the buffer and an arrow over the gutter, minimap and scrollbar, and a
  click on the minimap scrolls the viewport. An IME composition is drawn at the caret until it is
  committed; tab widths are measured, tabs carry a close affordance and a hover wash; a press on a
  line number selects that line; `Ctrl`+click adds a caret; soft-wrapped continuation rows keep the
  line's indentation; and `relative_line_numbers` switches the gutter to caret-relative numbering.
- `Icon` ships **69** `IconName` tokens. The three capable profiles draw the real **Material
  Symbols** outlines by default; `mini` / `embedded` draw geometry derived from the same outlines,
  so the two paths cannot disagree. Hosts can add icons with `register_icon` or
  `register_icon_on_grid`.

## Rendering backends

| Backend | Use | Notes |
|---|---|---|
| Software rasteriser | Always available | Anti-aliased, no GPU, runs headless |
| SVG | Snapshots, printing, vector output | Byte-reproducible; this is what `snapshots/svg/` holds |
| wgpu | GPU acceleration | Vulkan / Metal / DX12 / WebGPU → GL → CPU ladder |

The text origin is the glyph box's **top-left**, not a baseline, and both raster and SVG honour
that one contract. That is why a committed SVG is usable as review material instead of being a
second renderer that quietly disagrees.

## Platforms

| Target | Backend | Notes |
|---|---|---|
| Windows | Win32 | Real surface and event loop; DirectWrite font enumeration; printing |
| macOS | AppKit (`cocoa` / `objc2`) | CoreText fonts; accessibility bridge; native file dialogs |
| Linux / BSD | GTK (default) or Wayland native | fontconfig; portal appearance settings on Wayland |
| iOS / Mac Catalyst | UIKit | CoreText fonts; scene lifecycle and safe areas |
| Android | JNI | `/system/fonts`, `PrintManager`, lifecycle forwarding |
| HarmonyOS | Native C + ArkTS bridge | Window, vibrator, permissions and notification services |
| Web (wasm32) | WebGL / WebGPU surface | Visibility, DPR and safe areas |
| Redox | Orbital | Software surface |
| `embedded` / `mini` | None | In-memory buffer; SVG output stays available |

A backend that genuinely cannot provide a service reports it through `Platform::capabilities` and
the matching method, rather than returning a plausible-looking lie.

## C ABI

The C ABI in `src/bindings/` exposes every control through **152 `rw_*` functions** with a
capability-based property and event model. C, C++, Python, Java (JNI) and Node bindings are
exercised in CI, and the generated header is checked for drift by `tools/check_abi.sh`. Start with
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
| [`control.md`](control.md) | The widget gallery, generated from the registry |

## Verifying a change

```bash
cargo fmt --all -- --check
cargo check --all-targets --no-default-features --features desktop
cargo clippy --all-targets --no-default-features --features desktop -- -D warnings
cargo test  --no-default-features --features desktop -q

for profile in embedded mini tablet mobile; do
  cargo test --no-default-features --features "$profile" -q
done

bash tools/run_all_gates.sh
```

Do not reach for `--all-features`: `desktop` and `mini` are mutually exclusive and `mini` switches
the crate to `no_std`, so it is not a substitute for the matrix. Host-limited checks report as
skipped; native GTK, Apple and Android tests run in platform-specific CI jobs.

## Requirements

Rust **1.88+**. Drawing controls needs no GUI toolkit. On Linux the `desktop` profile enables GTK
for host integration, and a Wayland-native build is available with the `wayland-native` feature.
Image codecs are pure Rust, so cross-compiling to Android, iOS or wasm needs no `pkg-config`
sysroot.

## License

MIT — see [LICENSE](LICENSE). Third-party font, icon and codec data is recorded with its licence in
[`NOTICE`](NOTICE) and [`crates/rust-widgets-font/NOTICE`](crates/rust-widgets-font/NOTICE).

## Links

- [简体中文 README](README.zh-CN.md)
- [Cookbook](cookbook/)
- [Issues](https://github.com/mikewolfli/rust-widgets/issues)
