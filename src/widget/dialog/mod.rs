// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dialog widgets.
pub mod alert_dialogs;
pub mod bottom_sheet;
pub mod color_dialog;
pub mod dialog_widget;
pub mod file_dialog;
pub mod find_replace_dialog;
pub mod font_dialog;
pub mod input_dialog;
pub mod message_box;
mod modal;
pub mod modal_bottom_sheet;
pub mod modal_host;
pub mod popover;
pub mod popup_window;
pub mod progress_dialog;
pub mod tooltip;
pub mod wizard;
// Re-export dialog types
pub use alert_dialogs::{AboutDialog, AlertDialog, ConfirmDialog};
pub use bottom_sheet::BottomSheet;
pub use color_dialog::ColorDialog;
pub use dialog_widget::Dialog;
pub use file_dialog::FileDialog;
pub use find_replace_dialog::FindReplaceDialog;
pub use font_dialog::FontDialog;
pub use input_dialog::InputDialog;
pub use message_box::MessageBox;
pub use modal_bottom_sheet::ModalBottomSheet;
pub use modal_host::ModalHost;
pub use popover::Popover;
pub use popup_window::PopupWindow;
pub use progress_dialog::ProgressDialog;
pub use tooltip::Tooltip;
pub use wizard::{NavAction, WizardDialog, WizardStep, WizardStyle};

pub(crate) use modal::{
    draw_modal_scrim, draw_modal_scrim_scaled, localized_label, move_focus, scale_about_centre,
    REVEAL_MIN_SCALE,
};
