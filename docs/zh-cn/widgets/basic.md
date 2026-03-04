# 基础控件

本章介绍 rust_widgets 库中的基础控件，包括按钮、复选框、标签等。

## 按钮 (Button)

按钮是最常用的控件之一，用于触发操作。

```rust
use rust_widgets::create_button;

let button = create_button(parent, "点击我", x, y, width, height);
```

## 复选框 (Checkbox)

复选框用于表示二进制状态（选中或未选中）。

```rust
use rust_widgets::create_checkbox;

let checkbox = create_checkbox(parent, "选项", x, y, width, height);
```

## 标签 (Label)

标签用于显示文本信息。

```rust
use rust_widgets::create_label;

let label = create_label(parent, "这是一个标签", x, y, width, height);
```

## 文本输入框 (LineEdit)

文本输入框用于接收用户输入的文本。

```rust
use rust_widgets::create_line_edit;

let line_edit = create_line_edit(parent, "默认文本", x, y, width, height);
```

## 单选按钮 (RadioButton)

单选按钮用于从多个选项中选择一个。

```rust
use rust_widgets::create_radio_button;

let radio_button = create_radio_button(parent, "选项 1", x, y, width, height);
```

## 滑块 (Slider)

滑块用于在一定范围内选择值。

```rust
use rust_widgets::create_slider;

let slider = create_slider(parent, x, y, width, height);
```

## 进度条 (ProgressBar)

进度条用于显示操作的进度。

```rust
use rust_widgets::create_progress_bar;

let progress_bar = create_progress_bar(parent, x, y, width, height);
```

## 组合框 (ComboBox)

组合框用于从下拉列表中选择一个选项。

```rust
use rust_widgets::{create_combo_box, combo_box_add_item};

let combo_box = create_combo_box(parent, x, y, width, height);
combo_box_add_item(combo_box, "选项 1");
combo_box_add_item(combo_box, "选项 2");
```

## 列表框 (ListBox)

列表框用于显示和选择列表中的项目。

```rust
use rust_widgets::{create_list_box, list_box_add_item};

let list_box = create_list_box(parent, x, y, width, height);
list_box_add_item(list_box, "项目 1");
list_box_add_item(list_box, "项目 2");
```

## 旋转框 (SpinBox)

旋转框用于通过点击上下按钮或直接输入来选择数值。

```rust
use rust_widgets::create_spin_box;

let spin_box = create_spin_box(parent, x, y, width, height);
```