//! Navigation widget types — app-level navigation and page containers.

#[cfg(not(feature = "mini"))]
pub mod adaptive_scaffold;
#[cfg(not(feature = "mini"))]
pub mod app_bar;
#[cfg(not(feature = "mini"))]
pub mod bottom_navigation_bar;
#[cfg(not(feature = "mini"))]
pub mod navigation_drawer;
#[cfg(not(feature = "mini"))]
pub mod navigation_stack;
pub mod tab_view;

// Re-exports
#[cfg(not(feature = "mini"))]
pub use adaptive_scaffold::AdaptiveScaffold;
#[cfg(not(feature = "mini"))]
pub use app_bar::AppBar;
#[cfg(not(feature = "mini"))]
pub use bottom_navigation_bar::BottomNavigationBar;
#[cfg(not(feature = "mini"))]
pub use bottom_navigation_bar::NavItem;
#[cfg(not(feature = "mini"))]
pub use navigation_drawer::NavigationDrawer;
#[cfg(not(feature = "mini"))]
pub use navigation_stack::NavigationEvent;
#[cfg(not(feature = "mini"))]
pub use navigation_stack::NavigationStack;
pub use tab_view::TabPage;
pub use tab_view::TabView;
