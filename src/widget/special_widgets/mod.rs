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
pub mod code_editor;
pub mod color_picker;
pub mod command_palette;
pub mod diff_viewer;
pub mod freeform_shape;
pub mod grid;
pub mod map_attribution;
pub mod map_overview;
pub mod map_view;
pub mod markdown_editor;
pub mod media_player;
pub mod notification_center;
pub mod segmented_control;
pub mod signature_pad;
pub mod snackbar;
pub mod split_button;
pub mod terminal;
pub mod timeline_widget;
pub mod toast;
// Re-export special widgets
pub use breadcrumb::{Breadcrumb, BreadcrumbSegment};
pub use canvas::Canvas;
pub use chip::{Chip, ChipItem};
pub use code_editor::{CodeEditor, DiagnosticMarker, MarkerSeverity, MultiCursor};
pub use color_picker::ColorPicker;
pub use command_palette::{CommandEntry, CommandPalette};
pub use diff_viewer::{DiffFileList, DiffKind, DiffLine, DiffViewer};
pub use freeform_shape::{FreeformShapeWidget, ShapeCanvas};
pub use grid::GridWidget;
pub use map_attribution::{AttributionEntry, Corner, MapAttributionOverlay};
pub use map_overview::{CenterMarkerStyle, MapOverviewPanel};
pub use map_view::{map_marker_detail, map_view_companions};
pub use map_view::{
    Compass, MapMarker, MapView, ScaleBar, ScaleBarStyle, ScaleUnits, TileKey, TileLoadError,
    TileLoader, TileRequest, TileViewport, ZoomButton, ZoomControl,
};
pub use markdown_editor::{FindBar, MarkdownEditor, MarkdownOutlinePanel, MarkdownTextChange};
pub use media_player::{media_scrubber, media_transport, subtitle_bar, MediaPlayer};
pub use notification_center::{NotificationCenter, NotificationItem, NotificationLevel};
pub use segmented_control::{SegmentItem, SegmentedControl};
pub use signature_pad::{SignaturePad, SignatureStroke};
pub use snackbar::Snackbar;
pub use split_button::{SplitAction, SplitButton};
pub use terminal::terminal_find_bar::{TerminalFindBar, REGEX_SUPPORTED};
pub use terminal::terminal_view::TerminalView;
pub use terminal::{terminal_find_bar, terminal_view};
pub use timeline_widget::{TimelineItem, TimelineWidget};
pub use toast::{Toast, ToastItem, ToastLevel, ToastStack};
