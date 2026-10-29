// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Per-kind capability tables, split by the widget family each constructor belongs to.
//!
//! The property arrays and the published event schema are declared by the `include!`-ed
//! fragments in this file, so their macro scope reaches the child modules declared below.
//! The constructors themselves live in one module per family, mirroring the
//! `properties_<family>.in.rs` fragments.

use super::types::{Constraint, PropertySchema, PropertyValueKind, RangeSpec, WidgetCapability};
use crate::widget::WidgetKind;

// ── Published event schemas (generated from the signals) ─────────
// Included ahead of the capability constructors so `events_of!` is in scope for them. The
// generated table is the single place a name and its payload type are written down, which is
// what keeps the two from drifting (BLUE19 rule #95).
include!("../event_payloads.rs");

// ── Property schema definitions (split into per-category files) ──
include!("../properties_base.in.rs");
include!("../properties_input.in.rs");
include!("../properties_view.in.rs");
include!("../properties_container.in.rs");
include!("../properties_dialog.in.rs");
include!("../properties_menu.in.rs");
include!("../properties_advanced.in.rs");
include!("../properties_media.in.rs");
include!("../properties_other.in.rs");

// The master macro lives in its own module so this file stays a façade. It is declared
// after the includes above so the category macros are in its textual scope.
mod schema;

pub(crate) use schema::*;

// ── Widget capability functions, one module per property family ──
mod advanced;
mod base;
mod container;
mod dialog;
mod input;
mod media;
mod menu;
mod other;
mod other_companion;
mod view;

pub(crate) use advanced::*;
pub(crate) use base::*;
pub(crate) use container::*;
pub(crate) use dialog::*;
pub(crate) use input::*;
pub(crate) use media::*;
pub(crate) use menu::*;
pub(crate) use other::*;
pub(crate) use other_companion::*;
pub(crate) use view::*;
