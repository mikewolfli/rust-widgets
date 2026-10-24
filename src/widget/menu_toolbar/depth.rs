// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The cascade depth contract shared by the controls that paint submenus.

/// The maximum number of nested submenu panels below a control's own list.
///
/// Level 0 is the control's own list and levels 1 through this value are the child panels
/// opened from it, so a cascade can be five rows wide (the list plus four panels) but never
/// deeper. A branch that sits on the deepest level cannot open another panel; activating it
/// still announces the branch through the control's `submenu_requested` signal, but no panel
/// appears, and the deepest panel paints no disclosure arrow.
///
/// One constant governs [`DropdownMenu`](super::DropdownMenu), [`Menu`](super::Menu) and
/// [`MenuButton`](super::MenuButton), because a depth limit that each control re-declared
/// could drift and the three would disagree about how deep a cascade may go.
pub const MAX_SUBMENU_DEPTH: usize = 4;
