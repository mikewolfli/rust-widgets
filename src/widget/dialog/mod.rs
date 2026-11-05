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
    draw_modal_scrim, draw_modal_scrim_scaled, localized_label, localized_label_into, move_focus,
    note_plane_painted, pop_modal_layer, push_modal_layer, reset_plane_painted, scale_about_centre,
    scrim_owner_for_root, scrim_plane_color, REVEAL_MIN_SCALE,
};

// A paint backend that exists only to answer measurement questions.
//
// The dialogs below derive their button bands and row heights on the *press* path, where there is
// no render context of its own, and a press arrives on every mouse move. Building a
// `SoftwarePaintBackend` at the control's size for that did what a backend does when it is not a
// frame: it allocated **two** full-size RGBA buffers and zeroed both, per hover pixel (DL2-01).
// The derivation never draws a pixel — it only asks `measure_text` for a line height and a run
// width — so a zero-sized target answers every question it needs and touches no buffer.
//
// One instance is kept per thread rather than per call, so a steady hover costs no allocation at
// all. Same shape as the delegate `GpuPaintBackend` keeps for its own measurement-only work
// (`render/present/smooth.rs`).
#[cfg(not(alloc_frugal))]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static MEASUREMENT_BACKEND: crate::compat::RefCell<crate::render::SoftwarePaintBackend> =
        crate::compat::RefCell::new(crate::render::SoftwarePaintBackend::new(
            crate::core::Size::new(0, 0),
            1.0,
        ));
}

/// Runs `f` with a render context backed by the thread's measurement-only backend.
///
/// The context is valid for the callback only: it borrows the shared backend, which is why this
/// takes a closure instead of returning the context. Nothing painted through it reaches a pixel —
/// it is for `measure_text` and the geometry helpers built on it.
#[cfg(not(alloc_frugal))]
pub(crate) fn with_measurement_context<R>(
    f: impl FnOnce(&crate::render::RenderContext<'_>) -> R,
) -> R {
    MEASUREMENT_BACKEND.with(|backend| {
        let mut backend = backend.borrow_mut();
        let context = crate::render::RenderContext::new(&mut *backend);
        f(&context)
    })
}

/// The allocation-frugal build carries no shared render backend: each caller derives its geometry
/// with a stack-local zero-sized context instead.
#[cfg(alloc_frugal)]
pub(crate) fn with_measurement_context<R>(
    f: impl FnOnce(&crate::render::RenderContext<'_>) -> R,
) -> R {
    let mut backend = crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
    let context = crate::render::RenderContext::new(&mut backend);
    f(&context)
}
