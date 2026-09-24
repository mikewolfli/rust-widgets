// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Style system primitives.
//!
//! The style pipeline is layered so that a widget's appearance can be resolved
//! without any single source having to know about all the others:
//!
//! 1. [`primitives`](crate::style::primitives) — the value types (colour, padding,
//!    margin, font) that the rest of the system is built from. No resolution logic.
//! 2. [`theme`](crate::style::theme) and [`theme_state`](crate::style::theme_state)
//!    — the interaction-state model: which states a control can be in, and the
//!    per-state appearance and light/dark preference that select a theme.
//! 3. [`selector`](crate::style::selector), [`css`](crate::style::css), and
//!    [`stylesheet`](crate::style::stylesheet) — declarative matching. A selector
//!    decides *whether* a rule applies to a widget, and CSS text is parsed into
//!    those rules.
//! 4. [`gradient`](crate::style::gradient) and [`animation`](crate::style::animation)
//!    / [`animation_group`](crate::style::animation_group) — appearance values that
//!    vary over position and time rather than being constants.
//!
//! # Resolution order
//!
//! A widget's appearance is resolved by *merging* successive layers, each of which
//! only fills in what the layer before it left unset. The order is:
//!
//! 1. **Theme** — `crate::theme::resolved_theme_style`, the base palette, fonts and
//!    metrics. This is the only layer that can supply a font.
//! 2. **Registered stylesheets** — `StyleSheetManager::apply_to`, app-wide CSS in
//!    priority order.
//! 3. **Inline declarations** — a node's own CSS text (`Widget::apply_css`) and then
//!    its explicit style keys.
//!
//! Each step is one call to `WidgetStyle::merge`, which cannot overwrite a value
//! an earlier (more specific) layer already set. `crate::json` performs exactly this
//! sequence; see `json::loader::apply_declared_styles`.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/theme/mod.rs:1` (the theme resolves into `WidgetStyle`); 27 files reference it.
/// Time-varying style values: a styled property that is evaluated at a point in
/// time rather than being a constant.
pub mod animation;
/// Groups of animations advanced and driven as a unit, for coordinating several
/// timelines under one play/pause/seek call.
pub mod animation_group;
pub mod css;
/// Poll-based CSS file watcher that reloads a stylesheet into the global
/// [`stylesheet::StyleSheetManager`] when the file changes.
///
/// Gated on `not(alloc_frugal)` for the same reason the glob re-export below is:
/// the watcher polls the filesystem on a timer and is not part of the `mini`
/// (allocation-frugal) surface. Without this gate the module itself was still
/// compiled under `mini` while everything in it was `cfg`-ed out, so the
/// `#[cfg(test)]` re-export of `stylesheet_test_guard` resolved to an empty
/// module and `--all-targets` reported it as an unused import.
#[cfg(not(alloc_frugal))]
pub mod css_watcher;
/// The facts about **this device, right now** — text scale, density, locale, appearance,
/// motion preference, contrast, direction, mirroring — behind one interface.
///
/// Compiled in every profile, including `mini`/`embedded`: it depends only on `core` and the
/// style layer's own value types, never on `crate::theme` (which a build with no device profile
/// does not have). That is what lets a control ask about the environment without knowing which
/// profile it was built into (BLUE24 §4.2).
pub mod environment;
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

pub use animation::*;
pub use animation_group::*;
pub use css::*;
#[cfg(not(alloc_frugal))]
pub use css_watcher::*;
pub use environment::*;
pub use gradient::*;
pub use primitives::*;
pub use selector::*;
/// Serialises tests that touch the process-wide stylesheet manager.
///
/// The watcher's tests are the other caller, so under `mini` (where the watcher
/// is not compiled) this export has no consumer.
#[cfg(all(test, not(alloc_frugal)))]
pub(crate) use stylesheet::stylesheet_test_guard;
pub use stylesheet::*;
pub use theme::*;
pub use theme_state::*;
