# rust_widgets — Pure Rust GUI Library

<p align="center">
  <img src="snapshots/header.jpg" alt="rust_widgets" width="800">
</p>

A cross-platform GUI library written in pure Rust. It paints **every control itself** — there is no
`NSButton`, `gtk_button_new` or `android.widget.Button` anywhere in the crate, and the only
`CreateWindowExW` is the one that creates the window itself — and
it can render to a window, to a PNG, or to SVG. Desktop, tablet, mobile, embedded and a minimal
`mini` profile are all supported.

Why self-drawn controls are worth the effort:

| | Self-drawn (this library) | Native controls |
|---|---|---|
| Appearance | Identical on every OS | Differs per toolkit and version |
| Control count | 181 kinds everywhere | Only what the toolkit offers |
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
rust_widgets = "2.8.3"
```

Pick **exactly one device profile**. They are mutually exclusive — `mini` and `embedded` compile parts
of the crate *out*, so combining one with `desktop` is not a lowest common denominator, it is a broken
build:

```toml
rust_widgets = { version = "2.8.3", features = ["desktop"] }                       # default
rust_widgets = { version = "2.8.3", default-features = false, features = ["tablet"] }
rust_widgets = { version = "2.8.3", default-features = false, features = ["mobile"] }
rust_widgets = { version = "2.8.3", default-features = false, features = ["embedded"] }
rust_widgets = { version = "2.8.3", default-features = false, features = ["mini"] }
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

## Animation and state

Two mechanisms, both shared rather than per-control:

- **`WidgetState`** — `set_hovered` / `set_pressed` / `set_enabled` re-resolve the control's style
  through `Widget::set_state_theme_hook`, so a theme author's `"button:hover"` / `":pressed"` /
  `":disabled"` keys actually reach the painting code. The hook is object-safe, because it has to be
  callable through `dyn Widget` from the setters themselves.
- **`PropertyDriver`** — a stored-target progress value between `0.0` and `1.0`, priced by a
  `MotionSlot` tempo token (`Fast` / `Normal` / `Slow`). A control exposes it through
  `Widget::tick(delta_ms)` and `Widget::is_animating()`, which is how the frame loop discovers that a
  control owes frames without each control registering itself anywhere. A driver built at a value is
  **at rest**, not about to travel, so a freshly constructed control never animates away from its own
  state on its first frame.

`set_animating` is deliberately not a thing a caller toggles: `is_animating()` is derived from the
driver, so the control and the frame loop cannot disagree about whether anything is moving.

## Theming and disabled states

Colours resolve through one ladder — the control's explicit style, then the theme's resolved style for
that control, then a literal as the last resort — so an untouched control still follows an appearance
switch while a caller's deliberate colour always wins.

Disabled is two different moves, and the crate names them separately because applying one where the
other belongs produces the *opposite* of the intended state:

| | recedes by stepping toward | measured |
|---|---|---|
| `BaseWidget::disabled_ink_on(ink, surface)` — text and icons | the surface's own contrast colour | 4.57–5.91:1 |
| `BaseWidget::disabled_surface_near(surface, window)` — fills and panels | the page behind them | always less prominent than enabled |

The weights come from one shared constant, `dimensions::DISABLED_VEIL_ALPHA` (`0.55`) — the smallest
value that clears the 4.5:1 body-text floor on **both** the dark and the light appearance. A
half-transparent mid-grey has no direction: over a light surface it darkens and over a dark one it
lightens, so "disabled" would read as *more* prominent on whichever appearance was already hardest to
read. Stepping toward the surface is the only direction that reads as "receded" on both.

Text legibility is available to callers as well: `Color::contrast_ratio` measures the WCAG ratio, and
`Color::legible_on(surface, min_ratio)` returns the caller's colour, or the nearest step toward the
surface's contrast colour that clears `min_ratio`.

## Verifying a change

```bash
cargo fmt --all -- --check
cargo check --all-targets --no-default-features --features desktop
cargo clippy --all-targets --no-default-features --features desktop -- -D warnings
cargo test --no-default-features --features desktop -q
cargo test --no-default-features --features "desktop,icons" -q

# Remaining CI test profiles
for profile in full embedded mini tablet mobile; do
  cargo test --no-default-features --features "$profile" -q
done

# Build-only checks for every device profile
for profile in desktop tablet mobile mini embedded; do
  cargo check --no-default-features --features "$profile"
done

bash tools/run_all_gates.sh # source and contract gates, PASS/FAIL table
```

The desktop command runs the main desktop test suite; the separate `desktop,icons`
command covers the opt-in icon data. CI also tests `tablet` and `mobile`, and
tests `full`, `embedded`, and `mini` in its feature matrix. The desktop/default
configuration is tested once. CI build-checks all five device profiles. Do not use
`--all-features` as a substitute: `desktop` and `mini` are mutually exclusive,
and `mini` switches the crate to `no_std`. The gate runner checks source/API and
platform contracts; it is not a replacement for the Rust test matrix. It can
report host-limited checks as skipped. Native GTK, Apple, and Android tests are
run in platform-specific CI jobs; code coverage is collected separately by CI's
`cargo llvm-cov` job.

The SVGs under [`snapshots/svg/`](snapshots/svg/) are one file per control per appearance (dark and
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

A snapshot is not always the right evidence. It shows a control in its **resting** state, so a defect
on a hover, disabled or animating path is invisible to it. Those paths are pinned by pixel-level
probes instead (`tests/disabled_text_contrast.rs`, `tests/disabled_surface_probe.rs`,
`tests/m3_animation_probe.rs`), and each asserts the quantity that was promised — a contrast ratio, a
painted height, a segment count — rather than a proxy such as a byte length or a "is it animating"
flag.

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
| `fonts-cjk` | 361 704 bytes, OFL subset | the same script as **outlines**, so it is antialiased and scales to any px size |
| `fonts-vector-latin` | 35 896 bytes, OFL subset | real advances and kerning, from a face named by `Font::family` |
| `fonts-complex` | 70 576 bytes, OFL subset | Arabic joining, so `بيت` shapes to the word rather than three isolated letters |
| `fonts-emoji-color` | 1 602 492 bytes, OFL subset | colour emoji — 317 codepoints including the 26 regional indicators |
| `icons` | 10 016 bytes, Apache-2.0 | one **Material Symbols** SVG outline per `IconName` token, 68 of them (**on by default**) |

`fonts-cjk-bitmap` and `fonts-cjk` are two answers to one script, and the difference is size
against quality: an outline subset at the bitmap's own coverage would weigh 581 KB, roughly 7x, so
the bitmap is what a `mini`/`embedded` build uses and the vector face is for a desktop host that
wants Chinese antialiased at any size. Enabling both makes the bitmap win for a character it
covers — it is the cheaper face, and the stack keeps the cheap source in front.

None of these is enabled by any profile, and `--all-features` is the only way to get all of them
at once. The vector and colour subsets are lazy-loaded from the binary's read-only section and are
recorded, with their upstream digests, in [`NOTICE`](NOTICE).

The boundary is asserted from both sides (`render::pipeline::pixel_ops::text_coverage_tests` and
`render::text::glyph_source`'s tests) — a non-Latin character must resolve to the fallback glyph on
a default build, and enabling the CJK data must move that boundary by exactly the face it adds — so
the documentation cannot silently overstate what is drawn.

### Icons

`Icon` ships **68** `IconName` tokens. With the `icons` feature (which is **on by default**) each
token draws the **real outline** from the Material Symbols set vendored at a pinned revision; with
the feature off it draws generated fallback geometry derived from the *same* outlines:

```console
cargo build                                            # the real outlines (default)
cargo build --no-default-features --features desktop  # the derived fallback shapes
```

Nothing else changes with it on: `IconName::as_str` / `from_name` are the same tokens and the
colour resolves through the same ladder. The fallback is **not** hand-drawn — it is a coarse
flattening of the same `tools/material_symbols/<token>.svg` files the data comes from, so the two
paths cannot describe different shapes for one icon. `IconName::data()` answers `IconData` (not
`Option`), so adding a token without geometry is a compile error rather than a blank icon.

A host can add its **own** icons at runtime — the built-in set is a fixed vocabulary, and
`register_icon` is the open extension point beside it:

```rust
use rust_widgets::widget::register_icon;

// SVG path data on the 960-unit design grid, negative y upward (the Material Symbols convention).
assert!(register_icon("disclosure", &["M480-200 240-440l480 480-240-240Z"]));
icon.set_icon("disclosure");
```

`register_icon_on_grid` takes a grid argument for a source on another scale (24 units, as Lucide
and Tabler use). `clear_registered_icons`, `registered_icon_count` and `is_registered_icon` round
out the surface, and a registered icon draws through the same code a bundled one does.

The icon data is Apache-2.0 (Google LLC); the licence copy, the attribution and the verification
gate are in [`NOTICE`](NOTICE), `tools/material_symbols/LICENSE` and `tools/check_icon_licences.sh`.
The token set is declared once in `tools/icon_tokens.txt`; `tools/gen_icon_names.py` generates the
`IconName` type from it, so adding an icon is a two-line edit plus a generator run.

The feature is deliberately **not** in any device profile: `mini` and `embedded` are *sized*, so a
payload the caller did not ask for is wrong there. A profile build that wants icons asks for them
(`--features mini,icons`).

## Language bindings

The `C ABI` lives in `src/bindings/` and exposes every control through **145 `rw_*` functions** with a
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
[![version](https://img.shields.io/badge/version-2.8.3-blue)]()
[![tests](https://img.shields.io/badge/tests-6300%2B-brightgreen)]()
[![license](https://img.shields.io/badge/license-MIT-blue)]()
[![controls](https://img.shields.io/badge/controls-180-blue)]()

<p align="center">
  <a href="README.zh-CN.md">
    <img src="https://img.shields.io/badge/%E4%B8%AD%E6%96%87-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-blue" alt="简体中文">
  </a>
</p>
