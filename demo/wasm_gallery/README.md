# WASM control gallery

A browser demo that shows **every control kind the widget factory can build**, one
card per kind, paged 16 at a time.

```sh
# from the repository root
rustup target add wasm32-unknown-unknown      # once
wasm-pack build demo/wasm_gallery --target web --out-dir pkg
python3 -m http.server -d demo/wasm_gallery 8000
# open http://localhost:8000/
```

`wasm-pack` writes `demo/wasm_gallery/pkg/`, which `index.html` imports. A static
server is required: `file://` cannot fetch the wasm module.

If `wasm-pack` cannot download `binaryen` (the `wasm-opt` size step), add
`--no-opt`: the build itself is unaffected, only the bundle is larger.

## What it demonstrates

- **Every registered kind.** The card list is `WidgetFactory::widget_names()`, so a
  control the factory can construct cannot be missing; the paging label reports how
  many kinds there are.
- **Composites stay composite.** `tab_widget` shows its tabs, `menu_bar` its menus,
  `tree_view` its rows, `splitter` its panes. Children are never listed as separate
  cards — each card is the control that owns them. Content comes from the library's
  own `sample_fill`, the same filler the rendering census and the SVG exporter use.
- **The browser paint path.** The wasm backend owns the canvas and forwards pointer
  and keyboard input, but the host pulls the pixels: this demo runs one
  `requestAnimationFrame` loop that calls `drive_frame` and blits
  `render_window_frame` into the canvas's 2D context, only on frames that owe
  pixels.

## Files

| File | Contents |
|---|---|
| `src/app.rs` | The gallery, the paging controls and the host frame loop |
| `src/lib.rs` | `#[wasm_bindgen(start)]` for the browser; re-exports `app` |
| `src/main.rs` | Native entry stub (`main` only calls `app::run()`) |
| `index.html` | The `<canvas id="wgpu-canvas">` the backend binds to, and the module bootstrap |

## Features

The crate depends on the library with the browser set the cookbook names:

```toml
rust_widgets = { path = "../..", default-features = false, features = ["mobile", "wasm"] }
```

`mobile` is the device profile — it brings `touch`, the vector text faces and the
full widget set — and `wasm` selects the browser backend. `wasm32-unknown-unknown`
is the only target this demo ships to; the host (`cargo check`) build exists so the
crate stays inside `tools/check_demo_crates.sh`.

## Limitations

- One canvas, one window: the wasm backend owns a single `<canvas>` for the life of
  the page, so the demo is single-window by construction.
- Native controls do not exist in a browser (there is no OS widget toolkit); every
  card is drawn by the library's own renderer.
- A backend that keeps its own widget table (WASM, Android, iOS, Harmony) cannot host
  a library-owned widget on a platform surface. Those cards are adopted into the
  window tree instead, which is what the status line's `mounted` count reports.
- The bundle is not size-optimised for shipping: it links the full `mobile` profile
  so the gallery can show every kind. A release build is ~18 MB before `wasm-opt`.
