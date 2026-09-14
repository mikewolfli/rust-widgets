// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

pub(crate) mod create_widgets;
pub mod custom_paint;

pub use custom_paint::CustomPaintControlBackend;

/// The property names a control may use for its user-visible label.
///
/// Kinds disagree: a `Button` exposes `text`, a `Window`/`GroupBox`/`PopupWindow`
/// exposes `title`, a `StatusBar` exposes `message`. A backend that sets or reads
/// "the label" has to know the spellings, so they are listed once here rather than
/// repeated at each call site.
pub(crate) const LABEL_PROPERTY_NAMES: &[&str] = &["text", "title", "message"];

/// The kind used for a toggle button when the stripped profiles compile that
/// variant out.
///
/// A stripped profile drops `WidgetKind::ToggleButton`, but the trait method that
/// creates one must stay callable in **every** profile (the public API surface must
/// not vary — principle #53). Substituting the closest always-present kind keeps
/// the signature stable; the creation then reports `0` because the profile has no
/// constructor registry at all, which is the honest outcome.
pub(crate) const TOGGLE_BUTTON_KIND: crate::widget::WidgetKind = {
    #[cfg(widgets_unstripped)]
    {
        crate::widget::WidgetKind::ToggleButton
    }
    #[cfg(not(widgets_unstripped))]
    {
        crate::widget::WidgetKind::Button
    }
};

/// The kind used for a menu when the stripped profiles compile that variant out.
///
/// Same reasoning as [`TOGGLE_BUTTON_KIND`]: the trait method must stay callable in
/// every profile, so the variant is substituted rather than the signature gated.
#[allow(dead_code)]
pub(crate) const MENU_KIND: crate::widget::WidgetKind = {
    #[cfg(widgets_unstripped)]
    {
        crate::widget::WidgetKind::Menu
    }
    #[cfg(not(widgets_unstripped))]
    {
        crate::widget::WidgetKind::Panel
    }
};

impl CustomPaintControlBackend {
    /// Builds a widget of `kind` and mounts it on the host surface.
    ///
    /// This is the single creation path for every `create_*` method on this backend
    /// (BLUE15 rules #55/#65): the object comes from `WidgetFactory`, which is the
    /// only component that knows every kind's constructor, and it is handed to
    /// [`crate::widget::runtime`], which owns it from then on.
    ///
    /// Before this existed, `create_*` recorded geometry and a few strings into six
    /// shadow maps and produced an id that addressed **no widget object at all** —
    /// so the library could neither paint the control nor read its real state, and
    /// every property answered from a duplicate of the widget's own fields.
    /// Returning a mounted widget id is what makes the id meaningful.
    ///
    /// Returns `0` when the kind has no constructor, which is the honest answer and
    /// keeps a caller's `assert_ne!(id, 0)` meaningful.
    ///
    /// Gated on the full widget set for the same reason `WidgetFactory` is: a
    /// stripped profile compiles neither the factory nor the registry of
    /// constructors this resolves against, so there is nothing to mount there.
    pub(crate) fn mount_widget_of_kind(
        &self,
        kind: crate::widget::WidgetKind,
        parent: crate::core::ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> crate::core::ObjectId {
        // The constructor registry is gated on the full widget set, so a build
        // without it has nothing to build from. Reporting `0` is the honest answer
        // and matches the trait's own default behaviour there; the alternative
        // would be to compile a shadow registry, which is what this refactor
        // removed.
        #[cfg(not(full_widgets))]
        {
            let _ = (kind, parent, text, x, y, width, height);
            log::warn!(
                "custom backend: widgets cannot be created in this profile (no constructor \
                 registry is compiled in); returning 0"
            );
            0
        }
        #[cfg(full_widgets)]
        {
            let rect = crate::core::Rect::new(x, y, width, height);
            let name = kind_factory_name(kind);
            if name.is_empty() {
                log::warn!(
                    "custom backend: {kind:?} has no resolvable constructor; returning 0 rather \
                     than an id that addresses nothing"
                );
                return 0;
            }
            let Some(mut widget) =
                crate::widget::WidgetFactory::new_with_defaults().create(name, rect, text)
            else {
                log::warn!(
                    "custom backend: the factory lists {name:?} but produced no widget for \
                     {kind:?}; returning 0"
                );
                return 0;
            };
            // A window is a root; anything else must name an existing parent. `0` is the
            // "no parent" id the API uses, so a non-window with parent `0` is a caller
            // error — and so is an id that addresses nothing. Rejecting both here keeps
            // the C ABI's documented contract ("an unknown parent is rejected with 0")
            // true, which it could not be while creation did not check the parent.
            if kind != crate::widget::WidgetKind::Window {
                if parent == 0 || !crate::widget::runtime::is_mounted(parent) {
                    log::warn!(
                        "custom backend: refusing to create {kind:?} under parent {parent}, which \
                         addresses no live container"
                    );
                    return 0;
                }
                widget.set_parent(Some(parent));
            }
            crate::widget::runtime::register(widget).unwrap_or(0)
        }
    }

    /// Runs `f` against the live widget registered under `widget_id`.
    ///
    /// Every accessor on this backend reads the widget's **own** state instead of a
    /// shadow copy, so a property cannot disagree with the control it describes
    /// (BLUE15 §10.3: the shadow maps were one of three duplicated truths).
    ///
    /// The alloc-frugal profile has no widget registry at all, so the answer there
    /// is always `None` rather than a compile error.
    #[cfg(not(alloc_frugal))]
    pub(crate) fn with_live_widget<R>(
        &self,
        widget_id: crate::core::ObjectId,
        f: impl FnOnce(&mut dyn crate::widget::Widget) -> R,
    ) -> Option<R> {
        #[cfg(not(alloc_frugal))]
        {
            crate::widget::runtime::with_widget_mut(widget_id, f)
        }
        #[cfg(alloc_frugal)]
        {
            let _ = (widget_id, f);
            None
        }
    }
}

/// The `WidgetFactory` name for a [`crate::widget::WidgetKind`].
///
/// Split out of the creation helper so both the mount path and any future caller
/// resolve a kind to its constructor name the same way. The mapping lives here
/// rather than on `WidgetKind` because it belongs to the factory's vocabulary, not
/// to the enum. Alias variants are resolved by the callee.
#[cfg(widgets_unstripped)]
#[allow(dead_code)]
pub(crate) fn kind_factory_name(kind: crate::widget::WidgetKind) -> &'static str {
    crate::widget::capability::factory_name_for_kind(kind)
}

/// Tests for the custom backend's creation and state accessors.
///
/// Gated on the full widget set because every one of them exercises the real
/// creation path: they assert that a `create_*` call returns an id addressing a
/// mounted widget, and that reads and writes reach that widget. The alloc-frugal
/// profile compiles neither the factory nor the widget registry, so those
/// assertions describe machinery that is deliberately absent there — an earlier
/// revision kept these tests alive by asserting against a backend-side shadow map,
/// which is the duplication this refactor removed.
#[cfg(all(test, not(embedded_surface), widgets_unstripped))]
mod tests;
