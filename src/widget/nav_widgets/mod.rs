// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Navigation widget types — app-level navigation and page containers.

pub mod adaptive_scaffold;
pub mod app_bar;
pub mod bottom_navigation_bar;
pub mod navigation_drawer;
pub mod navigation_stack;
#[cfg(widgets_unstripped)]
pub mod pagination;
pub mod tab_view;

// Re-exports
pub use adaptive_scaffold::AdaptiveScaffold;
pub use app_bar::AppBar;
pub use bottom_navigation_bar::BottomNavigationBar;
pub use bottom_navigation_bar::NavItem;
pub use navigation_drawer::NavigationDrawer;
pub use navigation_stack::NavigationEvent;
pub use navigation_stack::NavigationStack;
#[cfg(widgets_unstripped)]
pub use pagination::Pagination;
pub use tab_view::TabPage;
pub use tab_view::TabView;
