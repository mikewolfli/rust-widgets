// The inner module keeps the control's own name: the folder is the family and the file is
// the control, so the historical `…::menu::menu` path stays valid. `module_inception` would
// prefer a different inner name; renaming here would move a published module path for a lint.
#![allow(clippy::module_inception)]
// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The menu composite family.
//!
//! This module holds the controls that make up a menu as one family:
//!
//! * [`menu`] — the `Menu` popup (also spelled `ContextMenu` through the alias in
//!   `widget/mod.rs`) and its `MenuEntry` rows;
//! * [`menu_bar`] — the bar that hosts and cascades those menus;
//! * [`dropdown_menu`] — the drop-down selector companion;
//! * [`popup_reveal`] — the shared open/close reveal all of them animate with.
//!
//! `menu_toolbar/mod.rs` re-exports both the modules and their item surface, so
//! `menu_toolbar::menu`, `menu_toolbar::menu_bar`, `menu_toolbar::dropdown_menu`
//! and `menu_toolbar::popup_reveal` keep resolving to the same paths as before
//! the family was consolidated here.
pub mod dropdown_menu;
pub mod menu;
pub mod menu_bar;
pub mod popup_reveal;

// Re-export the item surface that `menu_toolbar/mod.rs` exposed directly.
pub use dropdown_menu::{DropdownItem, DropdownMenu};
pub use menu::{
    menu_mnemonic, strip_menu_mnemonic, Menu, MenuEntry, PopupDirection, MAX_SUBMENU_DEPTH,
    MENU_ITEM_SCHEMA,
};
pub use menu_bar::{MenuBar, NativeMenuBridge, NativeMenuPosition};
pub use popup_reveal::{PopupReveal, RevealDirection};
