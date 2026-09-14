// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Navigation widget types — app-level navigation and page containers.

#[cfg(not(alloc_frugal))]
pub mod adaptive_scaffold;
#[cfg(not(alloc_frugal))]
pub mod app_bar;
#[cfg(not(alloc_frugal))]
pub mod bottom_navigation_bar;
#[cfg(not(alloc_frugal))]
pub mod navigation_drawer;
#[cfg(not(alloc_frugal))]
pub mod navigation_stack;
pub mod tab_view;

// Re-exports
#[cfg(not(alloc_frugal))]
pub use adaptive_scaffold::AdaptiveScaffold;
#[cfg(not(alloc_frugal))]
pub use app_bar::AppBar;
#[cfg(not(alloc_frugal))]
pub use bottom_navigation_bar::BottomNavigationBar;
#[cfg(not(alloc_frugal))]
pub use bottom_navigation_bar::NavItem;
#[cfg(not(alloc_frugal))]
pub use navigation_drawer::NavigationDrawer;
#[cfg(not(alloc_frugal))]
pub use navigation_stack::NavigationEvent;
#[cfg(not(alloc_frugal))]
pub use navigation_stack::NavigationStack;
pub use tab_view::TabPage;
pub use tab_view::TabView;
