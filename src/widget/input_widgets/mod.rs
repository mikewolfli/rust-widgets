// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Input widgets: text editors, combo boxes, spin boxes, etc.

#[cfg(full_widgets)]
/// Multi-level option chooser that walks a path down a tree.
pub mod cascader;
/// Editable and non-editable drop-down item selector.
pub mod combobox;
#[cfg(widgets_unstripped)]
/// Hyperlink-styled button that opens a URL or runs a command.
pub mod command_link;
/// Base type for drop-down popups shared by the combo-box family.
pub mod dropdown;
#[cfg(widgets_unstripped)]
/// Combo box whose items are font families, with a live preview.
pub mod font_combo_box;
#[cfg(widgets_unstripped)]
pub mod inplace_editor;
/// The input grammars a single-line field can enforce.
pub mod input_validator;
pub mod keyboard;
/// The single-line value editor an editable drop-down field carries (I-06).
pub(crate) mod line_editor;
pub mod listbox;
/// The input-mask grammar shared by `MaskedEdit` and `LineEdit::set_input_mask` (I-011).
///
/// Profile-neutral: a value type with no widget kind and no runtime, so `LineEdit` can keep its
/// mask support in a build that excludes the masked widget.
pub(crate) mod mask_engine;
/// `@`-triggered candidate completion inside free text.
#[cfg(full_widgets)]
pub mod mention;
#[cfg(widgets_unstripped)]
pub mod multi_select_combo_box;
#[cfg(widgets_unstripped)]
pub mod number_picker;
#[cfg(widgets_unstripped)]
pub mod otp_input;
/// Conversions and layout facts shared by the input family's companion popups (COMP-B01..B03).
pub(crate) mod popup_support;
#[cfg(widgets_unstripped)]
pub mod range_slider;
#[cfg(widgets_unstripped)]
pub mod rich_edit;
#[cfg(widgets_unstripped)]
pub mod shortcut_editor;
pub mod spinbox;
#[cfg(widgets_unstripped)]
pub mod tag_input;
/// The text-entry family: the single-line field, its variants and the chrome they draw.
pub mod text_entry;
/// Allocation-free case-fold comparisons shared by the input family (I-P05).
pub(crate) mod text_match;
pub mod textarea;
#[cfg(widgets_unstripped)]
pub mod textedit;
// The family's module paths stay reachable from this module, so `input_widgets::lineedit::X`,
// `input_widgets::search_box::Y` and every other published path resolve as before the move.
#[cfg(widgets_unstripped)]
pub use text_entry::auto_complete_edit;
#[cfg(widgets_unstripped)]
pub use text_entry::editable_combo_box;
#[cfg(widgets_unstripped)]
pub use text_entry::ime_preedit;
pub use text_entry::input_companions;
pub use text_entry::lineedit;
#[cfg(widgets_unstripped)]
pub use text_entry::masked_edit;
pub use text_entry::recent_values_popup;
#[cfg(widgets_unstripped)]
pub use text_entry::search_bar;
#[cfg(widgets_unstripped)]
pub use text_entry::search_box;
pub use text_entry::suggestion_popup;
pub use text_entry::validation_bubble;
// Re-export widget types
#[cfg(widgets_unstripped)]
pub use auto_complete_edit::AutoCompleteEdit;
#[cfg(full_widgets)]
pub use cascader::{Cascader, CascaderOption};
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
pub use input_companions::{
    Chip, DropSide, ScrollOrientation, Scrollbar, StepButtons, StepDirection, StepOrientation,
};
/// The format toolbar is the one companion that speaks the rich-text editor's own format value,
/// so it follows `rich_edit`'s gate rather than the module's.
#[cfg(widgets_unstripped)]
pub use input_companions::{FormatAction, FormatActionSet, FormatToolbar};
pub use keyboard::Keyboard;
pub use lineedit::{EchoMode, InputValidator, LineEdit};
pub use listbox::{ListBox, SelectionMode};
#[cfg(widgets_unstripped)]
pub use masked_edit::MaskedEdit;
#[cfg(full_widgets)]
pub use mention::{CompletedMention, Mention, MentionCandidate};
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
// One `SuggestionPopup`: COMP-B01's widget and COMP-C01's host were merged (principle #101), and
// `input_companions` re-exports these names so COMP-C01's path still resolves to the same type.
pub use suggestion_popup::{MatchMode, PopupItem, SuggestionPopup};
#[cfg(widgets_unstripped)]
pub use tag_input::TagInput;
pub use textarea::TextArea;
#[cfg(widgets_unstripped)]
pub use textedit::TextEdit;
