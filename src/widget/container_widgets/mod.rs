//! Container widgets: tab widgets, scroll areas, MDI areas, etc.
pub mod dockwidget;
pub mod groupbox;
pub mod mdiarea;
pub mod scrollarea;
pub mod splitter;
pub mod stackedwidget;
pub mod tabwidget;
pub mod toolbox;
// Re-export container widgets
pub use dockwidget::DockWidget;
pub use groupbox::GroupBox;
pub use mdiarea::MdiArea;
pub use scrollarea::ScrollArea;
pub use splitter::Splitter;
pub use stackedwidget::StackedWidget;
pub use tabwidget::TabWidget;
pub use toolbox::ToolBox;
