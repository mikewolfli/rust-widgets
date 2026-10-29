// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Master macro that expands all category macros at once.
//
// The `impl_properties_*!` macros are defined by the `include!`-ed fragments in the parent
// module, whose textual macro scope reaches this child because the child is declared after
// them. The constants the expansion declares are re-exported from the parent, which is how
// the capability constructors in the sibling family modules reach them.

use super::*;

macro_rules! impl_all_properties {
    () => {
        impl_properties_base!();
        impl_properties_input!();
        impl_properties_view!();
        impl_properties_container!();
        impl_properties_dialog!();
        impl_properties_menu!();
        impl_properties_advanced!();
        impl_properties_media!();
        impl_properties_other!();
    };
}
impl_all_properties!();
