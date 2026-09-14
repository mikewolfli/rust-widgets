// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

pub(crate) mod create_widgets;
pub mod custom_paint;

pub use custom_paint::CustomPaintControlBackend;

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
    pub(crate) fn mount_widget_of_kind(
        &self,
        kind: crate::widget::WidgetKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> crate::core::ObjectId {
        let rect = crate::core::Rect::new(x, y, width, height);
        let Some(name) = kind_factory_name(kind) else {
            log::warn!(
                "custom backend: {kind:?} has no registered constructor; returning 0 rather than \
                 an id that addresses nothing"
            );
            return 0;
        };
        let Some(widget) =
            crate::widget::WidgetFactory::new_with_defaults().create(name, rect, text)
        else {
            log::warn!(
                "custom backend: the factory lists {name:?} but produced no widget for {kind:?}; \
                 returning 0"
            );
            return 0;
        };
        crate::widget::runtime::register(widget).unwrap_or(0)
    }

    /// Runs `f` against the live widget registered under `widget_id`.
    ///
    /// Every accessor on this backend reads the widget's **own** state instead of a
    /// shadow copy, so a property cannot disagree with the control it describes
    /// (BLUE15 §10.3: the shadow maps were one of three duplicated truths).
    pub(crate) fn with_live_widget<R>(
        &self,
        widget_id: crate::core::ObjectId,
        f: impl FnOnce(&mut dyn crate::widget::Widget) -> R,
    ) -> Option<R> {
        crate::widget::runtime::with_widget_mut(widget_id, f)
    }
}

/// The `WidgetFactory` name for a [`crate::widget::WidgetKind`], when registered.
///
/// Split out of the creation helper so both the mount path and any future caller
/// resolve a kind to its constructor name the same way. The mapping lives here
/// rather than on `WidgetKind` because it belongs to the factory's vocabulary, not
/// to the enum.
pub(crate) fn kind_factory_name(kind: crate::widget::WidgetKind) -> Option<&'static str> {
    crate::widget::capability::factory_name_for_kind(kind)
}

#[cfg(all(test, not(embedded_surface)))]
mod tests;
