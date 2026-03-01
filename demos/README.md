# rust_widgets Demos

## Documentation Links

- Architecture: [../docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md)
- Help (EN): [../docs/HELP.en.md](../docs/HELP.en.md)
- Help (zh-CN): [../docs/HELP.zh-CN.md](../docs/HELP.zh-CN.md)
- Help (zh-TW): [../docs/HELP.zh-TW.md](../docs/HELP.zh-TW.md)
- Aide (FR): [../docs/HELP.fr.md](../docs/HELP.fr.md)
- Справка (RU): [../docs/HELP.ru.md](../docs/HELP.ru.md)

This folder contains runnable examples for each core control and subsystem.

### Run all examples (compile check)

```bash
cargo check --examples
```

### Main Entry

- `cargo run --example demo_main` - minimal complete app entry.

### Window and Shell Controls

- `cargo run --example demo_window` - window.
- `cargo run --example demo_dialog` - dialog.
- `cargo run --example demo_popup` - popup window.

### Basic Interactive Controls

- `cargo run --example demo_button` - button.
- `cargo run --example demo_checkbox` - checkbox.
- `cargo run --example demo_radiobutton` - radio button.
- `cargo run --example demo_label` - label.
- `cargo run --example demo_line_edit` - single-line input.
- `cargo run --example demo_text_edit` - multi-line input.

### Selection and Data View Controls

- `cargo run --example demo_combobox` - combo box.
- `cargo run --example demo_listbox` - list box.
- `cargo run --example demo_treeview` - tree view.
- `cargo run --example demo_table` - table.
- `cargo run --example demo_grid` - grid widget.

### Progress and Range Controls

- `cargo run --example demo_progress` - progress bar.
- `cargo run --example demo_slider` - slider.
- `cargo run --example demo_scrollbar` - scroll bar.

### Containers and Navigation Controls

- `cargo run --example demo_panel` - panel.
- `cargo run --example demo_groupbox` - group box.
- `cargo run --example demo_tab_widget` - tab container.
- `cargo run --example demo_stack_widget` - stack container.

### Menu, Tool, and Status Controls

- `cargo run --example demo_menubar` - menu bar.
- `cargo run --example demo_menu` - hierarchical menu, shortcuts, trigger polling.
- `cargo run --example demo_native_events` - typed native trigger polling (menu + widget).
- Linux GTK native signal bridge: `cargo run --features gtk-native --example demo_native_events`.
- `cargo run --example demo_toolbar` - tool bar.
- `cargo run --example demo_statusbar` - status bar.

### Graphics, Chart, Layout, XML, i18n

- `cargo run --example demo_canvas` - custom drawing canvas shell.
- `cargo run --example demo_render_quality` - configurable AA sample quality comparison.
- `cargo run --example demo_chart` - chart rendering model.
- `cargo run --example demo_layout` - code-based layout.
- `cargo run --example demo_xml` - XML layout loading and lookup by ID.
- `cargo run --example demo_i18n` - runtime language switch.
