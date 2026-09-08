//! Container widgets: tab widgets, scroll areas, MDI areas, etc.
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod carousel;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod collapsible_pane;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod dockwidget;
pub mod groupbox;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod masonry_layout;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod mdiarea;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod pager_page_view;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod safe_area;
pub mod scrollarea;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod splitter;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod stackedwidget;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod stepper;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod tabwidget;
pub mod tile_view;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub mod toolbox;
// Re-export container widgets
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use carousel::Carousel;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use collapsible_pane::CollapsiblePane;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use dockwidget::DockWidget;
pub use groupbox::GroupBox;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use masonry_layout::{MasonryItem, MasonryLayout};
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use mdiarea::MdiArea;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use pager_page_view::PagerPageView;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use safe_area::{SafeArea, SafeAreaInsets};
pub use scrollarea::ScrollArea;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use splitter::Splitter;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use stackedwidget::StackedWidget;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use stepper::Stepper;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use tabwidget::TabWidget;
#[cfg(not(any(feature = "mini", feature = "embedded")))]
pub use toolbox::ToolBox;
