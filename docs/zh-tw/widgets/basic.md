# 基礎控件

本章介紹 rust_widgets 庫中的基礎控件，包括按鈕、核取方塊、標籤等。

## 按鈕 (Button)

按鈕是最常用的控件之一，用於觸發操作。

```rust
use rust_widgets::create_button;

let button = create_button(parent, "點擊我", x, y, width, height);
```

## 核取方塊 (Checkbox)

核取方塊用於表示二進制狀態（選中或未選中）。

```rust
use rust_widgets::create_checkbox;

let checkbox = create_checkbox(parent, "選項", x, y, width, height);
```

## 標籤 (Label)

標籤用於顯示文本資訊。

```rust
use rust_widgets::create_label;

let label = create_label(parent, "這是一個標籤", x, y, width, height);
```

## 文字輸入框 (LineEdit)

文字輸入框用於接收使用者輸入的文字。

```rust
use rust_widgets::create_line_edit;

let line_edit = create_line_edit(parent, "預設文字", x, y, width, height);
```

## 單選按鈕 (RadioButton)

單選按鈕用於從多個選項中選擇一個。

```rust
use rust_widgets::create_radio_button;

let radio_button = create_radio_button(parent, "選項 1", x, y, width, height);
```

## 滑塊 (Slider)

滑塊用於在一定範圍內選擇值。

```rust
use rust_widgets::create_slider;

let slider = create_slider(parent, x, y, width, height);
```

## 進度條 (ProgressBar)

進度條用於顯示操作的進度。

```rust
use rust_widgets::create_progress_bar;

let progress_bar = create_progress_bar(parent, x, y, width, height);
```

## 組合框 (ComboBox)

組合框用於從下拉列表中選擇一個選項。

```rust
use rust_widgets::{create_combo_box, combo_box_add_item};

let combo_box = create_combo_box(parent, x, y, width, height);
combo_box_add_item(combo_box, "選項 1");
combo_box_add_item(combo_box, "選項 2");
```

## 列表框 (ListBox)

列表框用於顯示和選擇列表中的項目。

```rust
use rust_widgets::{create_list_box, list_box_add_item};

let list_box = create_list_box(parent, x, y, width, height);
list_box_add_item(list_box, "項目 1");
list_box_add_item(list_box, "項目 2");
```

## 旋轉框 (SpinBox)

旋轉框用於通過點擊上下按鈕或直接輸入來選擇數值。

```rust
use rust_widgets::create_spin_box;

let spin_box = create_spin_box(parent, x, y, width, height);
```