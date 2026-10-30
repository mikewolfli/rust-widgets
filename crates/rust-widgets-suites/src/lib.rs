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
//! host installs the group into the **shared** factory, before the first control is created:
//!
//! ```ignore
//! // At startup, before anything builds a control or parses a document:
//! rust_widgets::widget::register_shared_extension(rust_widgets_suites::install)
//!     .expect("the shared table was not built yet");
//!
//! // From here on the shared table can build this package's controls by name. That is the same
//! // table `create_widget_of_kind`, the JSON loader and the designer catalogue read, so a
//! // document can drop a `form` next to a core control with no extra wiring.
//! let form = rust_widgets::widget::WidgetFactory::shared()
//!     .create("form", Rect::new(0, 0, 360, 320), "Account")
//!     .expect("the suite registered `form`");
//! ```
//!
//! `register_shared_extension` freezes the list on the first `WidgetFactory::shared()` call; a
//! registration after that returns `SharedFactoryError::AlreadyBuilt` rather than succeeding into
//! a list nothing will read. An extension that collides with an existing name leaves that control
//! out and is reported by `shared_extension_failures()`.
//!
//! A host that wants its own table instead of the shared one can build it directly — the result is
//! the same table with the same controls, just not the one the rest of the library reads:
//!
//! ```ignore
//! let factory = rust_widgets::widget::WidgetFactory::new_with_extensions(&[install])?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `install` must be usable **directly** as a shared-factory extension.
    ///
    /// The example in this crate's docs passes `install` to
    /// `rust_widgets::widget::register_shared_extension`, so the two signatures are one contract:
    /// if either changes, this line stops compiling and the documented installation stops working
    /// at the same moment.
    #[test]
    fn install_has_the_shared_extension_signature() {
        let extension: rust_widgets::widget::SharedFactoryExtension = install;
        let group: rust_widgets::widget::SharedFactoryExtension = install_form_group;

        // Both must also be accepted by the constructor that applies a list, which is the
        // non-global path a host with its own table takes.
        let result = WidgetFactory::new_with_extensions(&[extension, group]);
        assert!(
            matches!(result, Err(RegistrationError::NameTaken { .. })),
            "the second install of `form` must report the name it collided on"
        );
    }
}
