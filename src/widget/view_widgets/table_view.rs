//! Table view widget — deprecated type alias for `TableWidget`.
///
/// `TableView` was a thin wrapper around `TableWidget` that added no new behavior.
/// All usages should be replaced with `TableWidget` directly.
/// This file will be removed in a future version.
/// Deprecated — use `TableWidget` directly.
pub type TableView = crate::widget::view_widgets::table_widget::TableWidget;
