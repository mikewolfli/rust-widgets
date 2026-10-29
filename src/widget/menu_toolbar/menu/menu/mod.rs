// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Menu widget.
//!
//! # The popup's rows are derived, not painted ad hoc
//!
//! BLUE22 §B.8 lists `menu` / `menu_item`'s defect as "the check and arrow paddings written by
//! hand". The *columns* of a row are derived (see `Menu::indicator_box` and
//! `Menu::label_box`, which state the standard menu-item relations), and the *run* of rows is
//! one derivation in `Menu::item_bands` that the painter and every hit path share. The run used
//! to be a `let mut y` inside `draw`; a `FlexLayout` column replaced it for a while, but every row
//! has a fixed height the control owns, so the layout only rediscovered constants — on each draw,
//! which is once per frame while the popup reveals.
//!
//! # Layout of this module
//!
//! * `model` — [`MenuEntry`], the mnemonic helpers and the [`Menu`] state/API;
//! * `popup` — the popup's row layout, nested-panel paint and pointer hit testing;
//! * `events` — the `Widget`, `WidgetProperties` and `EventHandler` implementations.
//!
//! The item surface is re-exported here, so `menu_toolbar::menu::menu::Menu` and the
//! family's own `menu_toolbar::menu::Menu` resolve to the same type.

mod events;
mod model;
mod popup;

#[cfg(test)]
mod tests;

pub use crate::widget::menu_toolbar::MAX_SUBMENU_DEPTH;
pub use model::{
    menu_mnemonic, strip_menu_mnemonic, Menu, MenuEntry, PopupDirection, MENU_ITEM_SCHEMA,
};
