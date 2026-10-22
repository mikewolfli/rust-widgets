// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The capability registry the JSON path resolves names through, kept out of `mod.rs`
//! (principle #8).

/// The capability registry used to resolve a JSON widget name to a constructor
/// and to look up a property's declared value kind.
///
/// Borrows the process-wide default factory: the registry is immutable once built, and this
/// function is called once per declared property and once per unknown control during a load, so
/// rebuilding ~190 registrations on each call was measurable on a large document.
///
/// Gated on the full widget set: a stripped profile compiles neither the factory
/// nor the constructors it resolves against, so the JSON path there falls back to
/// the loader's own construction table.
#[cfg(full_widgets)]
pub(crate) fn schema_factory() -> &'static crate::widget::WidgetFactory {
    crate::widget::WidgetFactory::shared()
}

/// A registry that answers "not registered" in a build without the full widget
/// set, so `properties::declared_schema` can keep a single code path.
#[cfg(not(full_widgets))]
#[derive(Debug, Default)]
pub(crate) struct EmptyFactory;

#[cfg(not(full_widgets))]
impl EmptyFactory {
    /// No capability is registered in this profile.
    pub(crate) fn capability_for_kind_instance(
        &self,
        _widget: &dyn crate::widget::Widget,
    ) -> Option<&crate::widget::capability::WidgetCapability> {
        None
    }

    /// No constructor is registered in this profile.
    pub(crate) fn create(
        &self,
        _name: &str,
        _geometry: crate::core::Rect,
        _text: &str,
    ) -> Option<Box<dyn crate::widget::Widget>> {
        None
    }
}

#[cfg(not(full_widgets))]
pub(crate) fn schema_factory() -> EmptyFactory {
    EmptyFactory
}
