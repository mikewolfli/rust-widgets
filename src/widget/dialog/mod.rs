// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dialog widgets.
#[cfg(not(alloc_frugal))]
pub mod bottom_sheet;
pub mod color_dialog;
pub mod dialog_widget;
pub mod file_dialog;
#[cfg(not(alloc_frugal))]
pub mod find_replace_dialog;
pub mod font_dialog;
pub mod input_dialog;
pub mod message_box;
#[cfg(not(alloc_frugal))]
pub mod modal_bottom_sheet;
#[cfg(not(alloc_frugal))]
pub mod popover;
pub mod popup_window;
pub mod progress_dialog;
#[cfg(not(alloc_frugal))]
pub mod tooltip;
#[cfg(not(alloc_frugal))]
pub mod wizard;
// Re-export dialog types
#[cfg(not(alloc_frugal))]
pub use bottom_sheet::BottomSheet;
pub use color_dialog::ColorDialog;
pub use dialog_widget::Dialog;
pub use file_dialog::FileDialog;
#[cfg(not(alloc_frugal))]
pub use find_replace_dialog::FindReplaceDialog;
pub use font_dialog::FontDialog;
pub use input_dialog::InputDialog;
pub use message_box::MessageBox;
#[cfg(not(alloc_frugal))]
pub use modal_bottom_sheet::ModalBottomSheet;
#[cfg(not(alloc_frugal))]
pub use popover::Popover;
pub use popup_window::PopupWindow;
pub use progress_dialog::ProgressDialog;
#[cfg(not(alloc_frugal))]
pub use tooltip::Tooltip;
#[cfg(not(alloc_frugal))]
pub use wizard::{WizardDialog, WizardStep};
