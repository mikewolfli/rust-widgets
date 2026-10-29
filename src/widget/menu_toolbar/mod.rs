// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Menu and toolbar widgets.
pub mod action;
/// The cascade depth shared by the submenu-painting controls.
pub mod depth;
/// Effect-policy boundary shared by the menu family.
pub(crate) mod effects;
/// The menu composite family: `Menu`, `MenuBar`, `DropdownMenu` and the shared reveal.
pub mod menu;
pub mod menu_button;
pub mod status_bar;
pub mod tool_bar;
pub mod tool_button;
// Re-export the moved menu modules so their module paths are unchanged.
pub use menu::{dropdown_menu, menu_bar, popup_reveal};
// Re-export menu and toolbar types
pub use action::Action;
pub use depth::MAX_SUBMENU_DEPTH;
pub use menu::{
    DropdownItem, DropdownMenu, Menu, MenuBar, NativeMenuBridge, NativeMenuPosition,
    PopupDirection, PopupReveal, RevealDirection,
};
pub use menu_button::{MenuButton, MenuItem};
pub use status_bar::StatusBar;
pub use tool_bar::ToolBar;
pub use tool_button::ToolButton;
