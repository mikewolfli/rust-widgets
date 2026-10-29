// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Optional widget suites for `rust_widgets`.
//!
//! # What this package is
//!
//! The parent crate is deliberately one layer: the widget primitives, the layout engines, the
//! runtime and the platform backends. Controls that are useful but not universal live in their own
//! package, the way `rust-widgets-webview` carries the web family. This package is the same shape
//! for **suites** — groups of controls that solve one job together.
//!
//! The first group is [`form_widgets`]: [`Form`](form_widgets::Form) collects labelled fields and
//! validates them, and [`FormSet`](form_widgets::FormSet) pages several forms with one active page
//! and aggregate validity.
//!
//! # Installing
//!
//! The dependency runs one way — this package depends on `rust_widgets`, never the reverse — so a
//! host registers the group into its own factory:
//!
//! ```ignore
//! let mut factory = rust_widgets::widget::capability::WidgetFactory::new_with_defaults();
//! rust_widgets_suites::install(&mut factory)?;
//! let form = factory.create("form", Rect::new(0, 0, 360, 320), "Account");
//! let set = factory.create("form_set", Rect::new(0, 0, 480, 360), "");
//! ```
//!
//! `install` returns the first `RegistrationError` instead of shadowing a name the parent (or an
//! earlier install) already owns.
//!
//! # Adding a suite later
//!
//! Each group is a module with its own `capability` submodule and an `install` function; the crate
//! root's [`install`] calls them in turn. A group owns its names, schemas and constructors, so
//! adding one never edits another.
//!
//! # Profiles
//!
//! The suite targets a device profile (`desktop`/`tablet`/`mobile`), the same set the parent's full
//! widget layer targets. It is not part of the `mini`/`embedded` stripped builds: those have no
//! widget registry to mount a form's fields into.

pub mod form_widgets;

use rust_widgets::widget::capability::types::RegistrationError;
use rust_widgets::widget::capability::WidgetFactory;

pub use form_widgets::{
    create_form, create_form_set, default_property_value, form_capability, form_set_capability,
    published_event_names, published_property_names, Form, FormField, FormFieldLayout,
    FormLabelPosition, FormPage, FormSet, FormSetHeaderStyle,
};

/// Installs the form group into `factory`.
///
/// The per-group entry point, so a host that wants one group can install exactly that one.
pub fn install_form_group(factory: &mut WidgetFactory) -> Result<(), RegistrationError> {
    form_widgets::install(factory)
}

/// Installs every suite group into `factory`.
///
/// Future groups add one line here; the order is the registration order, and `try_register` makes a
/// duplicate name an error rather than a silent last-writer-wins.
pub fn install(factory: &mut WidgetFactory) -> Result<(), RegistrationError> {
    install_form_group(factory)
}
