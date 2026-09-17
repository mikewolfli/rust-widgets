// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Input widgets: text editors, combo boxes, spin boxes, etc.

#[cfg(widgets_unstripped)]
pub mod auto_complete_edit;
/// Editable and non-editable drop-down item selector.
pub mod combobox;
#[cfg(widgets_unstripped)]
/// Hyperlink-styled button that opens a URL or runs a command.
pub mod command_link;
/// Base type for drop-down popups shared by the combo-box family.
pub mod dropdown;
#[cfg(widgets_unstripped)]
pub mod editable_combo_box;
#[cfg(widgets_unstripped)]
/// Combo box whose items are font families, with a live preview.
pub mod font_combo_box;
#[cfg(widgets_unstripped)]
pub mod ime_preedit;
#[cfg(widgets_unstripped)]
pub mod inplace_editor;
pub mod keyboard;
pub mod lineedit;
pub mod listbox;
#[cfg(widgets_unstripped)]
pub mod masked_edit;
#[cfg(widgets_unstripped)]
pub mod multi_select_combo_box;
#[cfg(widgets_unstripped)]
pub mod number_picker;
#[cfg(widgets_unstripped)]
pub mod otp_input;
#[cfg(widgets_unstripped)]
pub mod range_slider;
#[cfg(widgets_unstripped)]
pub mod rich_edit;
#[cfg(widgets_unstripped)]
pub mod search_bar;
#[cfg(widgets_unstripped)]
pub mod search_box;
#[cfg(widgets_unstripped)]
pub mod shortcut_editor;
pub mod spinbox;
#[cfg(widgets_unstripped)]
pub mod tag_input;
pub mod textarea;
#[cfg(widgets_unstripped)]
pub mod textedit;
// Re-export widget types
#[cfg(widgets_unstripped)]
pub use auto_complete_edit::AutoCompleteEdit;
pub use combobox::ComboBox;
#[cfg(widgets_unstripped)]
pub use command_link::CommandLink;
pub use dropdown::Dropdown;
#[cfg(widgets_unstripped)]
pub use editable_combo_box::EditableComboBox;
#[cfg(widgets_unstripped)]
pub use font_combo_box::FontComboBox;
#[cfg(widgets_unstripped)]
pub use ime_preedit::ImePreedit;
#[cfg(widgets_unstripped)]
pub use inplace_editor::InplaceEditor;
pub use keyboard::Keyboard;
pub use lineedit::{EchoMode, LineEdit};
pub use listbox::{ListBox, SelectionMode};
#[cfg(widgets_unstripped)]
pub use masked_edit::MaskedEdit;
#[cfg(widgets_unstripped)]
pub use multi_select_combo_box::{MultiSelectComboBox, MultiSelectItem};
#[cfg(widgets_unstripped)]
pub use number_picker::NumberPicker;
#[cfg(widgets_unstripped)]
pub use otp_input::OtpInput;
#[cfg(widgets_unstripped)]
pub use range_slider::{RangeSlider, RangeSliderOrientation};
#[cfg(widgets_unstripped)]
pub use rich_edit::RichEdit;
#[cfg(widgets_unstripped)]
pub use search_bar::SearchBar;
#[cfg(widgets_unstripped)]
pub use search_box::SearchBox;
#[cfg(widgets_unstripped)]
pub use shortcut_editor::{ShortcutEditor, ShortcutEntry};
pub use spinbox::SpinBox;
#[cfg(widgets_unstripped)]
pub use tag_input::TagInput;
pub use textarea::TextArea;
#[cfg(widgets_unstripped)]
pub use textedit::TextEdit;
