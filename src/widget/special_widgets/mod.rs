// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Special widgets: canvas, chart, grid, freeform shape, etc.
pub mod breadcrumb;
pub mod canvas;
pub mod chart;
pub mod chip;
pub mod code_editor;
pub mod color_picker;
pub mod command_palette;
pub mod diff_viewer;
pub mod finance;
pub mod freeform_shape;
pub mod gantt_widget;
pub mod grid;
pub mod heatmap;
pub mod kanban_board;
pub mod map_view;
pub mod markdown_editor;
pub mod media_player;
pub mod notification_center;
pub mod radar_chart;
pub mod segmented_control;
pub mod signature_pad;
pub mod snackbar;
pub mod split_button;
pub mod terminal_view;
pub mod timeline_widget;
pub mod toast;
// Re-export special widgets
pub use breadcrumb::{Breadcrumb, BreadcrumbSegment};
pub use canvas::Canvas;
pub use chart::ChartWidget;
pub use chip::{Chip, ChipItem};
pub use code_editor::{CodeEditor, DiagnosticMarker, MarkerSeverity, MultiCursor};
pub use color_picker::ColorPicker;
pub use command_palette::{CommandEntry, CommandPalette};
pub use diff_viewer::{DiffKind, DiffLine, DiffViewer};
pub use freeform_shape::FreeformShapeWidget;
pub use gantt_widget::{GanttTask, GanttWidget};
pub use grid::GridWidget;
pub use heatmap::{Heatmap, HeatmapCell};
pub use kanban_board::{CardPosition, KanbanBoard, KanbanCard, KanbanColumn};
pub use map_view::{MapMarker, MapView};
pub use markdown_editor::MarkdownEditor;
pub use media_player::MediaPlayer;
pub use notification_center::{NotificationCenter, NotificationItem, NotificationLevel};
pub use radar_chart::RadarChart;
pub use segmented_control::{SegmentItem, SegmentedControl};
pub use signature_pad::{SignaturePad, SignatureStroke};
pub use snackbar::Snackbar;
pub use split_button::{SplitAction, SplitButton};
pub use terminal_view::TerminalView;
pub use timeline_widget::{TimelineItem, TimelineWidget};
pub use toast::{Toast, ToastItem, ToastLevel, ToastStack};
