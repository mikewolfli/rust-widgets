# rust_widgets Demos / 示例

## Documentation links / 文档链接

- Architecture: [../docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md)
- Help (EN): [../docs/HELP.en.md](../docs/HELP.en.md)
- 帮助（简体中文）: [../docs/HELP.zh-CN.md](../docs/HELP.zh-CN.md)
- 幫助（繁體中文）: [../docs/HELP.zh-TW.md](../docs/HELP.zh-TW.md)
- Aide (FR): [../docs/HELP.fr.md](../docs/HELP.fr.md)
- Справка (RU): [../docs/HELP.ru.md](../docs/HELP.ru.md)

## English

This folder contains runnable examples for each core control and subsystem.

### Run all examples (compile check)

```bash
cargo check --examples
```

### Main entry

- `cargo run --example demo_main` - minimal complete app entry.

### Window and shell controls

- `cargo run --example demo_window` - window.
- `cargo run --example demo_dialog` - dialog.
- `cargo run --example demo_popup` - popup window.

### Basic interactive controls

- `cargo run --example demo_button` - button.
- `cargo run --example demo_checkbox` - checkbox.
- `cargo run --example demo_radiobutton` - radio button.
- `cargo run --example demo_label` - label.
- `cargo run --example demo_line_edit` - single-line input.
- `cargo run --example demo_text_edit` - multi-line input.

### Selection and data view controls

- `cargo run --example demo_combobox` - combo box.
- `cargo run --example demo_listbox` - list box.
- `cargo run --example demo_treeview` - tree view.
- `cargo run --example demo_table` - table.
- `cargo run --example demo_grid` - grid widget.

### Progress and range controls

- `cargo run --example demo_progress` - progress bar.
- `cargo run --example demo_slider` - slider.
- `cargo run --example demo_scrollbar` - scroll bar.

### Containers and navigation controls

- `cargo run --example demo_panel` - panel.
- `cargo run --example demo_groupbox` - group box.
- `cargo run --example demo_tab_widget` - tab container.
- `cargo run --example demo_stack_widget` - stack container.

### Menu, tool, and status controls

- `cargo run --example demo_menubar` - menu bar.
- `cargo run --example demo_menu` - hierarchical menu, shortcuts, trigger polling.
- `cargo run --example demo_native_events` - typed native trigger polling (menu + widget).
- Linux native GTK signal bridge: `cargo run --features gtk-native --example demo_native_events`.
- `cargo run --example demo_toolbar` - tool bar.
- `cargo run --example demo_statusbar` - status bar.

### Graphics, chart, layout, XML, i18n

- `cargo run --example demo_canvas` - custom drawing canvas shell.
- `cargo run --example demo_chart` - chart rendering model.
- `cargo run --example demo_layout` - code-based layout.
- `cargo run --example demo_xml` - XML layout loading and lookup by id.
- `cargo run --example demo_i18n` - runtime language switch.

## 简体中文

本目录提供按控件拆分的可运行示例，便于快速验证架构和接口。

### 统一检查

```bash
cargo check --examples
```

### 主入口

- `cargo run --example demo_main`：最小完整应用入口。

### 窗口与壳层控件

- `cargo run --example demo_window`：窗口。
- `cargo run --example demo_dialog`：对话框。
- `cargo run --example demo_popup`：弹出窗口。

### 基础交互控件

- `cargo run --example demo_button`：按钮。
- `cargo run --example demo_checkbox`：复选框。
- `cargo run --example demo_radiobutton`：单选框。
- `cargo run --example demo_label`：标签。
- `cargo run --example demo_line_edit`：单行输入。
- `cargo run --example demo_text_edit`：多行文本。

### 选择与数据展示控件

- `cargo run --example demo_combobox`：下拉框。
- `cargo run --example demo_listbox`：列表框。
- `cargo run --example demo_treeview`：树视图。
- `cargo run --example demo_table`：表格。
- `cargo run --example demo_grid`：网格控件。

### 进度与范围控件

- `cargo run --example demo_progress`：进度条。
- `cargo run --example demo_slider`：滑动条。
- `cargo run --example demo_scrollbar`：滚动条。

### 容器与导航控件

- `cargo run --example demo_panel`：面板。
- `cargo run --example demo_groupbox`：分组框。
- `cargo run --example demo_tab_widget`：标签页容器。
- `cargo run --example demo_stack_widget`：堆栈容器。

### 菜单、工具、状态控件

- `cargo run --example demo_menubar`：菜单栏。
- `cargo run --example demo_menu`：层级菜单、快捷键、触发轮询。
- `cargo run --example demo_native_events`：类型化原生触发轮询（菜单 + 控件）。
- `cargo run --example demo_toolbar`：工具栏。
- `cargo run --example demo_statusbar`：状态栏。

### 图形、图表、布局、XML、国际化

- `cargo run --example demo_canvas`：自定义绘制画布壳。
- `cargo run --example demo_chart`：图表绘制模型。
- `cargo run --example demo_layout`：代码布局。
- `cargo run --example demo_xml`：XML 加载与按 ID 查找。
- `cargo run --example demo_i18n`：运行时语言切换。
