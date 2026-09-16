// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Style system primitives.
//!
//! The style pipeline is layered so that a widget's appearance can be resolved
//! without any single source having to know about all the others:
//!
//! 1. [`primitives`] — the value types (colour, padding, margin, font) that the
//!    rest of the system is built from. No resolution logic.
//! 2. [`theme`] and [`theme_state`] — the base appearance: global theme
//!    defaults, per-class overrides, and per-widget state variants.
//! 3. [`selector`], [`css`], and [`stylesheet`] — declarative matching. A
//!    selector decides *whether* a rule applies to a widget, and CSS text is
//!    parsed into those rules.
//! 4. [`gradient`] and [`animation`] / [`animation_group`] — appearance values
//!    that vary over position and time rather than being constants.
//!
//! Inheritance runs theme → per-class overrides → per-widget state, each level
//! falling through to the next when unset; see the chain note below.
/// Time-varying style values: a styled property that is evaluated at a point in
/// time rather than being a constant.
pub mod animation;
/// Groups of animations advanced and driven as a unit, for coordinating several
/// timelines under one play/pause/seek call.
/// Groups of animations advanced and driven as a unit, for coordinating several
/// timelines under one play/pause/seek call.
pub mod animation_group;
pub mod css;
/// Position-varying colour ramps, used where a constant colour would be a
/// special case of a gradient.
pub mod gradient;
/// The base style value types — colour, padding, margin, border, font — that the
/// rest of the style system composes and resolves. Contains no matching or
/// inheritance logic, which lives in the layers above.
pub mod primitives;
pub mod selector;
pub mod stylesheet;
pub mod theme;
/// Per-widget interaction state used to select a style variant, so a control can
/// look different when hovered, pressed, focused, or disabled.
pub mod theme_state;

// ── Style Inheritance Chain (BLUE11 R6.6) ──
//
// Widget style resolution follows this inheritance chain:
//
// 1. Global Theme defaults (ThemeManager → Theme)
// 2. ThemeOverrides per widget class (e.g., "Button", "Label")
// 3. Widget instance state (StatefulTheme → WidgetState)
// 4. Inline style overrides (future)
//
// The ThemeManager resolves: Theme → ThemeOverrides → WidgetState
// Each step falls through to the next level if unset.
pub use animation::*;
pub use animation_group::*;
pub use css::*;
pub use gradient::*;
pub use primitives::*;
pub use selector::*;
pub use stylesheet::*;
pub use theme::*;
pub use theme_state::*;
