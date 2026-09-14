// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Menu and toolbar widgets.
pub mod action;
#[cfg(not(alloc_frugal))]
pub mod dropdown_menu;
pub mod menu;
pub mod menu_bar;
#[cfg(not(alloc_frugal))]
pub mod menu_button;
pub mod status_bar;
pub mod tool_bar;
pub mod tool_button;
// Re-export menu and toolbar types
pub use action::Action;
#[cfg(not(alloc_frugal))]
pub use dropdown_menu::{DropdownItem, DropdownMenu};
pub use menu::Menu;
pub use menu_bar::MenuBar;
#[cfg(not(alloc_frugal))]
pub use menu_button::{MenuButton, MenuItem};
pub use status_bar::StatusBar;
pub use tool_bar::ToolBar;
pub use tool_button::ToolButton;
