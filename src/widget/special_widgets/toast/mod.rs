// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Toast notifications — one message, and the stack that queues them.
//!
//! # Two controls, two files
//!
//! `ToastStack` and `Toast` are separate `WidgetKind`s serving separate needs, and they
//! used to share one file. That was not a size problem: the two have opposite
//! lifecycles, and reading them together made the difference easy to miss.
//!
//! | | `ToastStack` | `Toast` |
//! |---|---|---|
//! | Owns | many messages | one message |
//! | Layout | rows, newest at the bottom | a single bar |
//! | Selection | one toast is selected, arrows move it | none |
//! | Interactive keys | Up / Down / Enter / Delete | Escape |
//! | Signals | `toast_activated`, `toast_dismissed` | `dismissed` |
//! | Typical use | a host that queues and animates | `toast("saved")` |
//!
//! The common case is one message, and reaching it through the stack meant building a
//! stack, constructing an item, and pushing — for a control the caller then had to keep
//! alive to hear the dismissal. `Toast` exists so that case is one control.
//!
//! # Module layout
//!
//! * `item` — [`ToastLevel`] and [`ToastItem`], the data both controls speak. Shared
//!   rather than duplicated, because a stack of one control's items and a single
//!   control that invented its own severity enum would disagree about what "warning"
//!   means.
//! * `stack` — [`ToastStack`].
//! * `single` — [`Toast`].
//! * `tests` — both controls' tests, kept together so the shared item type is covered
//!   from both directions.

mod item;
mod single;
mod stack;

#[cfg(test)]
mod tests;

pub use item::{ToastItem, ToastLevel};
pub use single::Toast;
pub use stack::ToastStack;
