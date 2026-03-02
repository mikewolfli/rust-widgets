# rust_widgets Architecture

## Related docs

- Demo catalog: [../demos/README.md](../demos/README.md)
- Help (English): [HELP.en.md](HELP.en.md)
- 帮助（简体中文）: [HELP.zh-CN.md](HELP.zh-CN.md)
- 幫助（繁體中文）: [HELP.zh-TW.md](HELP.zh-TW.md)
- Aide (Français): [HELP.fr.md](HELP.fr.md)
- Справка (Русский): [HELP.ru.md](HELP.ru.md)

## 1. Layered design

- `core`: geometry, color, font, object id, profile and platform family enums.
- `object`: base object identity and lifecycle metadata.
- `event`: thread-safe queue and dispatch loop.
- `signal`: signal-slot abstraction with typed payloads, `once` slots, and scoped auto-disconnect.
- `widget`: control model set and shared widget trait.
- `layout`: `BoxLayout`, `GridLayout`, `FormLayout`, `StackLayout`.
- `xml`: XML/JSON layout loading with id lookup.
- `i18n`: runtime language switch, plural forms, `tr!` macro.
- `theme` + `style`: runtime theme switching and style tokens.
- `platform`: unified backend interface for desktop/embedded/mobile families.
- `print`/`pdf`/`chart`: enabled by `default + full` profile with feature gates.
- `bindings`: stable C ABI and reserved foreign language entry points.

## 2. Runtime profiles

- `default + full`: desktop-oriented complete stack, including print/pdf/chart.
- `embedded`: minimal runtime for embedded targets (window/basic control/layout path).
- `mobile-api`: reserved unified extension points for Android/iOS/Harmony mobile integration.

## 3. Platform abstraction

Desktop backends covered by architecture:
- Windows (`win32`)
- macOS (`cocoa`)
- Linux (`gtk`)
- Harmony desktop (`harmony-desktop`)

`MobilePlatformExtension` provides reserved expansion points:
- Android
- iOS
- Harmony mobile

## 4. Event and signal flow

1. Platform backend receives native event.
2. Covered widget interactions normalize into typed widget-trigger routes.
3. `NativeSignalBridge` maps typed trigger routes to signal emissions.
4. `event::EventLoop` remains for system/non-covered scheduling (`timer`, `idle`, `custom`, modal dispatch).

Boundary note:
- Covered interaction routes (`clicked`, `value-changed`, `selection-changed`, `closed`) are signal-first.
- `EventLoop` is retained as a compatibility/system scheduler, not the primary covered-interaction trigger source.
5. Connected slots run in user code.

## 5. C ABI strategy

Stable C ABI exports include:
- runtime control (`rust_widgets_init/run/quit`)
- widget lifecycle (`create_window`, `create_button`, property setters/getters)
- menu actions (`rust_widgets_attach_menu_bar_to_window`, `rust_widgets_menu_add_item`, `rust_widgets_poll_menu_triggered`)
- widget trigger polling (`rust_widgets_poll_widget_triggered`, compatibility path)
- typed widget trigger polling (`rust_widgets_poll_widget_trigger_event`, kind: 0=none, 1=clicked, 2=value-changed, 3=selection-changed, 4=closed)
- memory-safe string free (`rust_widgets_free_string`)
- version and reserved language bridges:
  - `rust_widgets_bindings_api_version`
  - `rust_widgets_python_reserved`
  - `rust_widgets_cpp_reserved`
  - `rust_widgets_java_reserved`

## 6. Project structure (implemented)

```text
Cargo.toml
src/
  lib.rs
  core/
  object/
  event/
  signal/
  widget/
  layout/
  xml/
  i18n/
  platform/
    windows.rs
    macos.rs
    linux.rs
    harmony.rs
  theme/
  style/
  print/
  pdf/
  chart/
  bindings/
demos/
  demo_main.rs
  demo_window.rs
  demo_button.rs
  demo_dialog.rs
  demo_popup.rs
  demo_checkbox.rs
  demo_radiobutton.rs
  demo_label.rs
  demo_line_edit.rs
  demo_text_edit.rs
  demo_combobox.rs
  demo_listbox.rs
  demo_treeview.rs
  demo_progress.rs
  demo_slider.rs
  demo_scrollbar.rs
  demo_panel.rs
  demo_groupbox.rs
  demo_tab_widget.rs
  demo_stack_widget.rs
  demo_menubar.rs
  demo_menu.rs
  demo_toolbar.rs
  demo_statusbar.rs
  demo_canvas.rs
  demo_table.rs
  demo_grid.rs
  demo_chart.rs
  demo_layout.rs
  demo_xml.rs
  demo_i18n.rs
  assets/
docs/
  ARCHITECTURE.md
  HELP.zh-CN.md
  HELP.zh-TW.md
  HELP.en.md
  HELP.fr.md
  HELP.ru.md
```

## 7. Demo execution

Use Cargo examples:

```bash
cargo run --example demo_main
cargo run --example demo_window
cargo run --example demo_button
cargo run --example demo_dialog
cargo run --example demo_popup
cargo run --example demo_checkbox
cargo run --example demo_radiobutton
cargo run --example demo_label
cargo run --example demo_line_edit
cargo run --example demo_text_edit
cargo run --example demo_combobox
cargo run --example demo_listbox
cargo run --example demo_treeview
cargo run --example demo_progress
cargo run --example demo_slider
cargo run --example demo_scrollbar
cargo run --example demo_panel
cargo run --example demo_groupbox
cargo run --example demo_tab_widget
cargo run --example demo_stack_widget
cargo run --example demo_menubar
cargo run --example demo_menu
cargo run --example demo_toolbar
cargo run --example demo_statusbar
cargo run --example demo_canvas
cargo run --example demo_table
cargo run --example demo_grid
cargo run --example demo_chart
cargo run --example demo_layout
cargo run --example demo_xml
cargo run --example demo_i18n
```

## 8. Geometry & style primitive contract (v10)

- Geometry is standardized around `Point`, `Size`, and `Rect` conversions:
  - `Rect::from_position_size(position, size)` for construction
  - `rect.position()` / `rect.size()` / `rect.decompose()` for extraction
- Color parsing/serialization is centralized in `core::Color`:
  - accepted input forms: `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA`
  - canonical output forms: uppercase `#RRGGBB` and `#RRGGBBAA`
- Font descriptors now include normalized `weight` (`100..=900`, nearest-100 step).
  - Compatibility constructor `Font::new(family, size, bold, italic)` is preserved.
  - New explicit constructor `Font::with_weight(...)` is preferred for deterministic typography.
- Spacing contracts now distinguish content and outer spacing:
  - `Padding` for content insets
  - `Margin` for outer spacing
  - both support `all`, `symmetric`, and `normalized` helpers.

### Migration guidance

- Existing `Rect`-based call sites remain valid; adopt point/size helpers incrementally.
- Existing bold/italic call sites remain valid; migrate to explicit `weight` only where needed.
- Existing uniform spacing remains valid via `Padding::all`/`Margin::all`; migrate to per-side values when required.
