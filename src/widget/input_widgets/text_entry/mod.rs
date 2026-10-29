// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Text-entry family: the single-line field, its variants and the chrome they draw.
//!
//! Everything here exists because a value is **typed**, not picked: the members share the
//! single-line text model of [`lineedit::LineEdit`] (a `String`, a byte caret, an optional
//! selection anchor and an undo stack), and the rest of the family either derives a field from
//! that model or presents something a field asks the host to place beside it.
//!
//! | Member | Why it belongs to the family |
//! |---|---|
//! | [`lineedit`] | The control itself: one line of text, a caret, selection, echo modes, validation and the undo model every other member reuses. |
//! | [`masked_edit`] | The same field with an input mask attached; it drives `mask_engine`, the grammar `LineEdit::set_input_mask` also uses, and is not an independent kind because it adds no second text model. |
//! | [`search_box`] | A `LineEdit` with a search affordance; it reads the query and the clear action from the field rather than owning a model of its own. |
//! | [`search_bar`] | The wider search variant: the same query model plus find options, hosted as the bar a search box expands into. |
//! | [`auto_complete_edit`] | A `LineEdit` with completion candidates; the inserted text still lands in the field's own model. |
//! | [`editable_combo_box`] | A `LineEdit` that also chooses from a list; the editable half is the field, so the typing contract stays single-sourced. |
//! | [`suggestion_popup`] | The candidate list a text control hangs under itself (COMP-B01); it is chrome drawn for a field, not a standalone chooser. |
//! | [`validation_bubble`] | The message a field's validator anchors beside it (COMP-B02). |
//! | [`recent_values_popup`] | Previously committed values offered back to a field (COMP-B03). |
//! | [`input_companions`] | Plain value/geometry pieces the input family pairs with (chips, stepper buttons, a scroll indicator, the format toolbar); they carry no runtime dependency and exist to be drawn by these controls. |
//! | [`ime_preedit`] | The inline composition string a field draws while an input method is active; it is presentation for the field's `composition` state, and has no independent `create_*` constructor of its own. |
//!
//! Independent pickers — `combobox`, `multi_select_combo_box`, `font_combo_box`, `dropdown`,
//! `mention`, `otp_input`, `tag_input`, `spinbox`, `keyboard`, `command_link` and
//! `inplace_editor` — stay in the parent module because each owns a widget kind (and a
//! `create_*` entry point) of its own rather than deriving from the text model.
//!
//! # Reachability
//!
//! The parent module re-exports these module names, so every published path
//! (`input_widgets::lineedit::LineEdit`, `input_widgets::search_box::SearchBox`, …) resolves
//! exactly as it did before the family was grouped into this folder.

#[cfg(widgets_unstripped)]
pub mod auto_complete_edit;
#[cfg(widgets_unstripped)]
pub mod editable_combo_box;
#[cfg(widgets_unstripped)]
pub mod ime_preedit;
pub mod input_companions;
pub mod lineedit;
/// Masked text input.
///
/// Gated on `widgets_unstripped` because it is a **widget kind**: the profile's
/// [`WidgetKind`](crate::widget::WidgetKind) census decides which controls a build carries, and
/// `mini` does not carry this one. The *grammar* it is built on is not gated —
/// [`mask_engine`](super::mask_engine) is what a single-line field needs.
#[cfg(widgets_unstripped)]
pub mod masked_edit;
pub mod recent_values_popup;
#[cfg(widgets_unstripped)]
pub mod search_bar;
#[cfg(widgets_unstripped)]
pub mod search_box;
pub mod suggestion_popup;
pub mod validation_bubble;

#[cfg(widgets_unstripped)]
pub use self::auto_complete_edit::{
    AutoCompleteEdit, CompletionClosePolicy, CompletionMatchMode, CompletionSeparator,
};
#[cfg(widgets_unstripped)]
pub use self::editable_combo_box::EditableComboBox;
#[cfg(widgets_unstripped)]
pub use self::ime_preedit::{ImePreedit, PreeditSegment};
pub use self::input_companions::{
    Chip, DropSide, ScrollOrientation, Scrollbar, StepButtons, StepDirection, StepOrientation,
};
/// The format toolbar is the one companion that speaks the rich-text editor's own format value,
/// so it follows `rich_edit`'s gate rather than the module's.
#[cfg(widgets_unstripped)]
pub use self::input_companions::{FormatAction, FormatActionSet, FormatToolbar};
pub use self::lineedit::{
    EchoMode, InputValidator, LineEdit, LineEditAction, ValidationState, Validator,
};
#[cfg(widgets_unstripped)]
pub use self::masked_edit::MaskedEdit;
pub use self::recent_values_popup::RecentValuesPopup;
#[cfg(widgets_unstripped)]
pub use self::search_bar::{SearchBar, SearchOptions};
#[cfg(widgets_unstripped)]
pub use self::search_box::SearchBox;
pub use self::suggestion_popup::{MatchMode, PopupItem, SuggestionPopup};
pub use self::validation_bubble::{ValidationBubble, ValidationSeverity};
