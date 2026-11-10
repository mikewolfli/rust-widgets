// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Special widgets: canvas, grid, freeform shape, editors, maps and the misc controls.
//!
//! The data-visualisation controls (charts, heatmaps, radar, gantt, kanban and the finance
//! set) live in [`crate::widget::chart_widgets`], which is where the chart drawing engine
//! already is — one classification, not two.
pub mod breadcrumb;
pub mod canvas;
pub mod chip;
/// The code editor control. Compiled only with the `code-editor` feature; its
/// capability entries, constructor and events carry the same gate (BLUE31 §1.5).
#[cfg(feature = "code-editor")]
pub mod code_editor;
pub mod color_picker;
pub mod command_palette;
/// The shared lower layer of the two editor controls: syntax vocabulary, caret
/// painting, and the companion find bar. Not a third public editor control
/// (BLUE31 §1.4).
pub mod editor;
pub mod freeform_shape;
pub mod grid;
/// The Markdown editor control. Independent of `code-editor`: it uses only the
/// shared `editor` layer (BLUE31 §1.8).
#[cfg(feature = "markdown-editor")]
pub mod markdown_editor;
pub mod notification_center;
pub mod segmented_control;
pub mod signature_pad;
pub mod snackbar;
pub mod split_button;
pub mod timeline_widget;
pub mod toast;
// Re-export special widgets
pub use breadcrumb::{Breadcrumb, BreadcrumbSegment};
pub use canvas::Canvas;
pub use chip::{Chip, ChipItem};
#[cfg(feature = "code-editor")]
pub use code_editor::{CodeEditor, DiagnosticMarker, MarkerSeverity, MultiCursor};
pub use color_picker::ColorPicker;
pub use command_palette::{CommandEntry, CommandPalette};
/// The find bar belongs to the editor family, not to the Markdown control alone,
/// so it is re-exported from the shared layer whenever an editor is compiled
/// (BLUE31 §1.9.4).
#[cfg(any(feature = "code-editor", feature = "markdown-editor"))]
pub use editor::companions::FindBar;
pub use freeform_shape::{FreeformShapeWidget, ShapeCanvas};
pub use grid::GridWidget;
#[cfg(feature = "markdown-editor")]
pub use markdown_editor::{
    MarkdownBlockRender, MarkdownBlockRenderer, MarkdownBlockSpec, MarkdownEditor,
    MarkdownOutlinePanel, MarkdownTextChange,
};
pub use notification_center::{NotificationCenter, NotificationItem, NotificationLevel};
pub use segmented_control::{SegmentItem, SegmentedControl};
pub use signature_pad::{SignaturePad, SignatureStroke};
pub use snackbar::Snackbar;
pub use split_button::{SplitAction, SplitButton};
pub use timeline_widget::{TimelineItem, TimelineWidget};
pub use toast::{Toast, ToastItem, ToastLevel, ToastStack};
