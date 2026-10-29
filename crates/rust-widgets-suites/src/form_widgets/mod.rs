// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `form_widgets` group: validated forms and form sets.
//!
//! # Why this is a group rather than two loose controls
//!
//! `Form` and `FormSet` answer two halves of one question — "collect and validate a set of values"
//! — and they compose: a `FormSet` page hosts a `Form`. Keeping them in one module is what lets a
//! host reach both through one `install` call and one documentation page, while the crate root
//! stays a list of groups (`future groups add one module and one line`).
//!
//! # Adding a group later
//!
//! A group is a module under `src/` with:
//!
//! 1. a `capability` submodule holding its property schemas, event schemas and constructors;
//! 2. an `install(factory)` function that registers every control in the group;
//! 3. a `mod.rs` that only declares and re-exports (principle #8).
//!
//! The crate root calls each group's installer from `install`. No group may register a name the
//! parent already owns; `try_register` refuses a collision rather than shadowing it.

mod capability;
/// The validated form container.
pub mod form;
/// The ordered set of forms.
pub mod formset;
mod model;

#[cfg(test)]
mod tests;

pub use capability::{
    create_form, create_form_set, default_property_value, form_capability, form_set_capability,
    install, published_event_names, published_property_names, FORM_PROPERTIES, FORM_SET_PROPERTIES,
};
pub use form::{Form, FormFieldLayout, FormLabelPosition, DEFAULT_WIDTH};
pub use formset::{FormSet, FormSetHeaderStyle};
pub use model::{FormField, FormPage};
