// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Input widgets: text editors, combo boxes, spin boxes, etc.

#[cfg(widgets_unstripped)]
pub mod auto_complete_edit;
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
pub mod editable_combo_box;
#[cfg(widgets_unstripped)]
/// Combo box whose items are font families, with a live preview.
pub mod font_combo_box;
#[cfg(widgets_unstripped)]
pub mod ime_preedit;
#[cfg(widgets_unstripped)]
pub mod inplace_editor;
/// Companion pieces the input family pairs with (COMP-C01..C05).
///
/// Not gated on `widgets_unstripped`: these are plain value/geometry types (chips, stepper
/// buttons, a scroll indicator, a format toolbar) with no `widget::runtime` dependency, so the
/// `mini` profile can carry them. They were gated only because the modules were introduced under
/// that blanket, and the gate made `spinbox`/`textarea` — which are *not* gated — fail to build
/// in `mini`.
pub mod input_companions;
/// The input grammars a single-line field can enforce.
pub mod input_validator;
pub mod keyboard;
/// The single-line value editor an editable drop-down field carries (I-06).
pub(crate) mod line_editor;
pub mod lineedit;
pub mod listbox;
/// The input-mask grammar shared by `MaskedEdit` and `LineEdit::set_input_mask` (I-011).
///
/// Profile-neutral: a value type with no widget kind and no runtime, so `LineEdit` can keep its
/// mask support in a build that excludes the masked widget.
pub(crate) mod mask_engine;
/// Masked text input.
///
/// Gated on `widgets_unstripped` because it is a **widget kind**: the profile's
/// [`WidgetKind`](crate::widget::WidgetKind) census decides which controls a build carries, and
/// `mini` does not carry this one. The *grammar* it is built on is not gated — see
/// [`mask_engine`], which is what a single-line field needs.
#[cfg(widgets_unstripped)]
pub mod masked_edit;
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
/// Recently committed values, newest first (COMP-B03).
pub mod recent_values_popup;
#[cfg(widgets_unstripped)]
pub mod rich_edit;
#[cfg(widgets_unstripped)]
pub mod search_bar;
#[cfg(widgets_unstripped)]
pub mod search_box;
#[cfg(widgets_unstripped)]
pub mod shortcut_editor;
pub mod spinbox;
/// The candidate list an input field hangs under itself (COMP-B01).
pub mod suggestion_popup;
#[cfg(widgets_unstripped)]
pub mod tag_input;
/// Allocation-free case-fold comparisons shared by the input family (I-P05).
pub(crate) mod text_match;
pub mod textarea;
#[cfg(widgets_unstripped)]
pub mod textedit;
/// A validation message anchored beside the field it accuses (COMP-B02).
pub mod validation_bubble;
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
