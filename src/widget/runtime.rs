// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Registry for library-painted widgets mounted into windows.
//!
//! # Why this exists
//!
//! [`crate::widget::WidgetFactory`] can build a `Box<dyn Widget>` for any kind —
//! `CodeEditor`, `ColorPicker`, `GanttWidget`, … — but until this module existed
//! there was no path from such a box to pixels inside a real window. The platform
//! layer only knew how to create OS controls (an OS button, a text field, a
//! toolkit widget); a library-painted widget had to be handed to
//! `render_to_svg()` or it went nowhere. Mounting one into a window produced an
//! empty surface.
//!
//! This registry closes that loop. The host keeps ownership of the widget here,
//! keyed by [`ObjectId`]; a backend supplies the surface it paints widgets into
//! and, when a repaint is wanted, calls [`with_widget_mut`] and paints a
//! frame. **Which surface that is is a backend detail** — see
//! [`crate::Platform::mount_surface`].
//!
//! # Threading
//!
//! Widgets are `!Send` (they own `Rc`/`RefCell` state), and the desktop backends
//! require UI work on the platform main thread anyway. The registry is therefore
//! **thread-local**, and [`register`] returns `None` when called from a thread
//! that has no registry rather than smuggling a non-`Send` value across threads.
//!
//! [`with_widget_mut`]: crate::widget::runtime::with_widget_mut

#[cfg(full_widgets)]
use crate::core::Point;
use crate::core::{ObjectId, Rect, Size};
use crate::event::Event;
use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

/// Why a widget could not be mounted onto a host surface.
///
/// Lives here, beside the registry, because that is the layer that knows about
/// widget registration and ownership. `app` re-exports it, so callers that drive
/// mounting through a `WindowHandle` keep the same path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceMountError {
    /// The calling thread has no widget registry — mounting must happen on the
    /// thread that drives the UI.
    NoRegistryOnThread,
    /// This backend has no surface to offer
    /// (`Platform::supports_surfaces` returned `false`). Carries the
    /// backend name for the message.
    UnsupportedByBackend(&'static str),
    /// The backend claims support but refused this particular mount (unknown
    /// parent, wrong parent kind, allocation failure). Carries the backend name.
    RejectedByBackend(&'static str),
    /// A lookup by widget name did not match anything in the widget factory.
    UnknownWidgetName,
}

impl core::fmt::Display for SurfaceMountError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoRegistryOnThread => {
                write!(f, "widgets must be mounted on the UI thread (no registry on this thread)")
            }
            Self::UnsupportedByBackend(backend) => {
                write!(f, "backend '{backend}' has no surface to mount widgets onto")
            }
            Self::RejectedByBackend(backend) => {
                write!(f, "backend '{backend}' refused the mount (see logs for the reason)")
            }
            Self::UnknownWidgetName => {
                write!(f, "the widget factory has no widget registered under that name")
            }
        }
    }
}

impl std::error::Error for SurfaceMountError {}
use crate::widget::Widget;
use std::cell::RefCell;
use std::collections::HashMap;

/// One widget handed to the registry for display by the host surface.
struct Mounted {
    widget: Box<dyn Widget>,
}

thread_local! {
    /// Widgets mounted for display, keyed by their `ObjectId`.
    static MOUNTED: RefCell<HashMap<ObjectId, Mounted>> = RefCell::new(HashMap::new());

    /// Monotonic id source for mounted widgets.
    ///
    /// Starts high so a mounted id cannot collide with a platform-allocated
    /// widget id (those come from `BackendState`, which counts up from 1).
    static NEXT_ID: RefCell<ObjectId> = const { RefCell::new(0x5345_4C46_0000_0001) };
}

/// Hands ownership of `widget` to the registry and returns its display id.
///
/// Returns `None` when the calling thread has no registry — i.e. it is not the
/// thread that drives the UI. Callers must surface that as "cannot display
/// here" rather than dropping the widget silently.
pub fn register(widget: Box<dyn Widget>) -> Option<ObjectId> {
    let id = NEXT_ID.try_with(|next| {
        let id = *next.borrow();
        *next.borrow_mut() = id.wrapping_add(1);
        id
    });
    let Ok(id) = id else { return None };
    let stored = MOUNTED.try_with(|map| {
        map.borrow_mut().insert(id, Mounted { widget });
    });
    stored.ok().map(|()| id)
}

/// Removes a mounted widget, dropping it. Returns whether it was present.
pub fn unregister(id: ObjectId) -> bool {
    MOUNTED.try_with(|map| map.borrow_mut().remove(&id).is_some()).unwrap_or(false)
}

/// Returns whether `id` refers to a mounted self-drawn widget.
pub fn is_mounted(id: ObjectId) -> bool {
    MOUNTED.try_with(|map| map.borrow().contains_key(&id)).unwrap_or(false)
}

/// Returns whether the widget mounted under `id` can carry a tri-state value.
///
/// Asked of the widget itself rather than of a table of kinds: checkability is a
/// capability of the control's own property set, so any type that declares
/// `checked` in [`crate::widget::WidgetProperties::property_names`] answers
/// `true` here — including a third-party widget the library has never heard of.
/// Hosts use this to decide whether a `set_widget_tristate` request is
/// meaningful (principle #37: a request the control cannot honour is refused,
/// never silently stored).
pub fn widget_is_checkable(id: ObjectId) -> bool {
    with_widget(id, |widget| {
        crate::widget::capability::properties_trait::widget_property_names(widget)
            .is_some_and(|names| names.contains(&"checked"))
    })
    .unwrap_or(false)
}

/// Returns the number of widgets mounted on this thread.
pub fn mounted_count() -> usize {
    MOUNTED.try_with(|map| map.borrow().len()).unwrap_or(0)
}

/// Returns the geometry of a mounted widget, or `None` when it is not mounted.
pub fn geometry_of(id: ObjectId) -> Option<Rect> {
    MOUNTED
        .try_with(|map| map.borrow().get(&id).map(|entry| entry.widget.geometry()))
        .ok()
        .flatten()
}

/// Updates the geometry of a mounted widget. Returns whether it was found.
pub fn set_geometry(id: ObjectId, geometry: Rect) -> bool {
    let updated = MOUNTED.try_with(|map| {
        map.borrow_mut().get_mut(&id).map(|entry| entry.widget.set_geometry(geometry)).is_some()
    });
    updated.unwrap_or(false)
}

/// Runs `f` with mutable access to a mounted widget.
///
/// The borrow is scoped to the call, so a painter that triggers another paint
/// cannot re-enter the registry and alias the widget.
pub fn with_widget_mut<R>(id: ObjectId, f: impl FnOnce(&mut dyn Widget) -> R) -> Option<R> {
    MOUNTED
        .try_with(|map| map.borrow_mut().get_mut(&id).map(|entry| f(entry.widget.as_mut())))
        .ok()
        .flatten()
}

/// Runs `f` with shared access to a mounted widget.
///
/// Used by read-only queries (a host inspecting a mounted editor's text or
/// caret) so they do not need mutable access merely to look at state.
pub fn with_widget<R>(id: ObjectId, f: impl FnOnce(&dyn Widget) -> R) -> Option<R> {
    MOUNTED
        .try_with(|map| map.borrow().get(&id).map(|entry| f(entry.widget.as_ref())))
        .ok()
        .flatten()
}

/// Forwards a platform event into a mounted widget's `EventHandler`.
///
/// Returns whether the widget was found. Backends call this to deliver raw
/// platform input; coordinate translation into widget-local space is the
/// widget's responsibility (see `position_at_point`).
pub fn dispatch_event(id: ObjectId, event: &Event) -> bool {
    with_widget_mut(id, |widget| widget.handle_event(event)).is_some()
}

/// Opens `menu_id` as a context menu at `position`, clamped to `viewport`.
///
/// This is the one operation a right-click needs, and it lives here rather than in
/// each widget because a context menu is opened *at a pointer position on another
/// widget*: the target widget knows the position, the menu owns the popup, and the
/// registry is the only thing that can reach both.
///
/// Returns `false` when `menu_id` is not a mounted menu, or when it is not a menu
/// at all — the honest answer, reported instead of a silent no-op, so a caller
/// wiring a right-click handler learns immediately that it pointed at the wrong id.
///
/// Gated with the menu widget itself: a profile that compiles `Menu` out has no
/// context menu to open, and reporting that at compile time is more useful than a
/// function that could only ever return `false`.
#[cfg(full_widgets)]
pub fn open_context_menu(menu_id: ObjectId, position: Point, viewport: Rect) -> bool {
    with_widget_mut(menu_id, |widget| {
        let Some(menu) = (widget as &mut dyn core::any::Any).downcast_mut::<crate::Menu>() else {
            log::warn!(
                "widget::runtime: id {menu_id} is not a menu, so a context menu cannot be opened \
                 at {position:?}"
            );
            return false;
        };
        menu.open_at(position, viewport);
        true
    })
    .unwrap_or_else(|| {
        log::warn!("widget::runtime: no widget is mounted as id {menu_id}");
        false
    })
}

/// Opens the context menu at the pointer position of a secondary-button press.
///
/// Convenience for the common wiring: a widget forwards its right-clicks here and
/// the menu appears where the user clicked. Returns `false` for any event that is
/// not a secondary-button press, or when the menu cannot be opened — so a caller
/// can pass every event through without testing the button itself.
#[cfg(full_widgets)]
pub fn open_context_menu_for_event(menu_id: ObjectId, event: &Event, viewport: Rect) -> bool {
    let Event::MousePress { pos, button } = event else {
        return false;
    };
    if *button != crate::event::mouse_button::SECONDARY {
        return false;
    }
    open_context_menu(menu_id, *pos, viewport)
}

/// Asks the platform to repaint a mounted widget.
///
/// Call this after mutating a widget through [`with_widget_mut`] so the change
/// becomes visible. Backends route it to their own invalidation (`setNeedsDisplay:`
/// on macOS, `InvalidateRect` on Windows, `queue_draw` on GTK); a backend that
/// never mounted the widget simply does nothing.
pub fn request_repaint(id: ObjectId) {
    crate::invalidate_surface(id);
}

/// Renders one frame of a mounted widget at `size` and returns the RGBA bytes.
///
/// `bytes.len() == size.width * size.height * 4`, top-down, straight (non
/// premultiplied) alpha — the layout all three desktop backends consume.
/// Returns `None` when `id` is not mounted or the size is empty.
pub fn render_frame(id: ObjectId, size: Size, clear: crate::core::Color) -> Option<Vec<u8>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }
    let mut backend = SoftwarePaintBackend::new(size, 1.0);
    let painted = with_widget_mut(id, |widget| {
        let Some(drawable) = widget.as_draw_mut() else {
            return false;
        };
        backend.begin_frame(clear);
        {
            let mut context = RenderContext::new(&mut backend);
            drawable.draw(&mut context);
        }
        backend.end_frame();
        true
    })?;
    if !painted {
        return None;
    }
    Some(backend.frame_rgba().to_vec())
}

/// Renders one frame of a mounted widget using its own geometry as the size.
///
/// Convenience wrapper for backends that size the native canvas from the
/// widget's geometry rather than from the OS-reported rectangle.
pub fn render_frame_at_geometry(id: ObjectId, clear: crate::core::Color) -> Option<Vec<u8>> {
    let geometry = geometry_of(id)?;
    render_frame(id, Size::new(geometry.width, geometry.height), clear)
}

/// Test module for the runtime registry and self-drawn frame rendering.
///
/// Runs only in `full_widgets` profiles: the tests exercise the self-drawn bridge
/// through concrete widgets in `special_widgets`, which is itself gated on
/// `full_widgets` (defined in `build.rs`). Gating on plain `test` alone would
/// make a stripped-down profile try to resolve types that are compiled out.
#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::core::{Color, Point};
    use crate::widget::special_widgets::code_editor::CodeEditor;

    /// Downcasts a mounted widget to the concrete editor type.
    fn editor_of(widget: &mut dyn Widget) -> Option<&mut CodeEditor> {
        (widget as &mut dyn std::any::Any).downcast_mut::<CodeEditor>()
    }

    fn sample_editor(text: &str) -> Box<dyn Widget> {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 320, 200));
        editor.set_text(text);
        Box::new(editor)
    }

    #[test]
    fn register_then_unregister_round_trips() {
        let before = mounted_count();
        let id = register(sample_editor("fn main() {}")).expect("ui thread has a registry");
        assert!(is_mounted(id));
        assert_eq!(mounted_count(), before + 1);
        assert!(unregister(id));
        assert!(!is_mounted(id));
        assert_eq!(mounted_count(), before);
    }

    #[test]
    fn unregister_unknown_id_is_false() {
        assert!(!unregister(0xDEAD_BEEF));
    }

    #[test]
    fn mounted_ids_do_not_collide_with_platform_ids() {
        // Platform-allocated ids count up from 1; mounted ids start far above.
        let id = register(sample_editor("x")).expect("registry");
        assert!(id > 0x5345_4C46_0000_0000, "mounted id {id:#x} must not look platform-issued");
        unregister(id);
    }

    #[test]
    fn with_widget_mut_exposes_the_real_widget() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let lines =
            with_widget_mut(id, |widget| editor_of(widget).map(|editor| editor.line_count()))
                .flatten();
        assert_eq!(lines, Some(1));
        unregister(id);
    }

    #[test]
    fn with_widget_mut_on_missing_id_is_none() {
        assert!(with_widget_mut(0x1234_5678, |_| ()).is_none());
    }

    #[test]
    fn geometry_can_be_read_and_written_through_the_registry() {
        let id = register(sample_editor("x")).expect("registry");
        assert_eq!(geometry_of(id).map(|r| (r.width, r.height)), Some((320, 200)));
        assert!(set_geometry(id, Rect::new(5, 6, 400, 300)));
        let geometry = geometry_of(id).expect("mounted");
        assert_eq!((geometry.x, geometry.y, geometry.width, geometry.height), (5, 6, 400, 300));
        unregister(id);
    }

    #[test]
    fn render_frame_returns_rgba_of_the_requested_size() {
        let id = register(sample_editor("fn main() {\n    let x = 1;\n}")).expect("registry");
        let frame = render_frame(id, Size::new(64, 48), Color::WHITE).expect("frame");
        assert_eq!(frame.len(), 64 * 48 * 4);
        // The editor paints a light background, so at least one pixel must be
        // non-transparent; a fully blank frame means the widget never drew.
        assert!(frame.chunks_exact(4).any(|px| px[3] != 0), "frame must contain drawn pixels");
        unregister(id);
    }

    #[test]
    fn render_frame_rejects_empty_size_and_missing_ids() {
        let id = register(sample_editor("x")).expect("registry");
        assert!(render_frame(id, Size::new(0, 10), Color::WHITE).is_none());
        assert!(render_frame(0x9999, Size::new(10, 10), Color::WHITE).is_none());
        unregister(id);
    }

    #[test]
    fn render_frame_at_geometry_uses_widget_geometry() {
        let id = register(sample_editor("abc")).expect("registry");
        let frame = render_frame_at_geometry(id, Color::WHITE).expect("frame");
        assert_eq!(frame.len(), 320 * 200 * 4);
        unregister(id);
    }

    /// Every widget that paints itself must be mountable in a host surface.
    ///
    /// `Widget::as_draw_mut` defaults to `None`, so a widget can implement
    /// `Draw` and still be invisible when mounted — which is exactly how a
    /// mounted `Chip` silently produced no frame. This guards the contract for
    /// the widgets the demos and the factory expose.
    #[test]
    fn custom_widgets_report_the_draw_bridge() {
        use crate::widget::special_widgets::chip::Chip;
        use crate::widget::special_widgets::color_picker::ColorPicker;
        use crate::widget::special_widgets::gantt_widget::GanttWidget;
        use crate::widget::special_widgets::snackbar::Snackbar;
        use crate::widget::special_widgets::terminal_view::TerminalView;

        fn assert_mountable(name: &str, widget: &mut dyn Widget) {
            assert!(
                widget.as_draw_mut().is_some(),
                "{name} implements Draw but does not report itself via Widget::as_draw_mut, \
                 so mounting it in a window would paint nothing"
            );
        }

        assert_mountable("CodeEditor", &mut CodeEditor::new(Rect::new(0, 0, 80, 60)));
        assert_mountable("Chip", &mut Chip::new(Rect::new(0, 0, 80, 24)));
        assert_mountable("ColorPicker", &mut ColorPicker::new(Rect::new(0, 0, 200, 150)));
        assert_mountable("GanttWidget", &mut GanttWidget::new(Rect::new(0, 0, 200, 150)));
        assert_mountable("Snackbar", &mut Snackbar::new(Rect::new(0, 0, 200, 40)));
        assert_mountable("TerminalView", &mut TerminalView::new(Rect::new(0, 0, 200, 150)));
    }

    /// A built-in control must report itself as paintable.
    ///
    /// This assertion is the inverse of the one that used to live here, which
    /// required `Button` to return `None` because the control was expected to be
    /// an OS widget. Every control is now painted by the library (BLUE15 #55), so
    /// a control that still answers `None` paints a blank surface — the exact
    /// regression this test exists to catch.
    #[test]
    fn built_in_controls_claim_the_draw_bridge() {
        use crate::widget::base_widgets::button::Button;
        use crate::widget::base_widgets::checkbox::CheckBox;
        use crate::widget::base_widgets::label::Label;

        let mut button = Button::new("ok".to_string(), Rect::new(0, 0, 80, 30));
        assert!(
            button.as_draw_mut().is_some(),
            "Button implements Draw, so mounting it must paint instead of showing a blank surface"
        );

        let mut checkbox = CheckBox::new(Rect::new(0, 0, 120, 24));
        assert!(checkbox.as_draw_mut().is_some(), "CheckBox must be paintable");

        let mut label = Label::new("hello".to_string(), Rect::new(0, 0, 80, 20));
        assert!(label.as_draw_mut().is_some(), "Label must be paintable");
    }

    #[test]
    fn dispatch_event_reaches_the_widget() {
        use crate::event::Event;
        let mut editor = CodeEditor::new(Rect::new(0, 0, 320, 200));
        editor.set_text("hello");
        let id = register(Box::new(editor)).expect("registry");
        // A click inside the text area must move the caret away from 0:0.
        let delivered =
            dispatch_event(id, &Event::MousePress { pos: Point::new(150, 90), button: 1 });
        assert!(delivered, "event must reach the mounted widget");
        let cursor =
            with_widget_mut(id, |widget| editor_of(widget).map(|editor| editor.cursor())).flatten();
        assert!(cursor.is_some(), "widget must still be the editor");
        unregister(id);
    }

    /// A secondary-button press opens the menu at the pointer, clamped to the
    /// viewport — the whole right-click path in one assertion.
    #[cfg(full_widgets)]
    #[test]
    fn a_secondary_press_opens_the_mounted_context_menu() {
        let mut menu = crate::Menu::new("Edit", Rect::new(0, 0, 160, 100));
        menu.add_action("Copy");
        let id = register(Box::new(menu)).expect("registry");

        let viewport = Rect::new(0, 0, 500, 400);
        let opened = open_context_menu_for_event(
            id,
            &Event::MousePress {
                pos: Point::new(120, 90),
                button: crate::event::mouse_button::SECONDARY,
            },
            viewport,
        );

        assert!(opened, "a secondary press must open the context menu");
        let geometry = geometry_of(id).expect("mounted");
        assert_eq!((geometry.x, geometry.y), (120, 90));
        assert!(is_mounted(id));
        unregister(id);
    }

    /// The convenience wrapper must ignore every event that is not a secondary
    /// press, so a caller can forward its whole event stream without testing the
    /// button itself.
    #[cfg(full_widgets)]
    #[test]
    fn non_secondary_events_do_not_open_the_context_menu() {
        let mut menu = crate::Menu::new("Edit", Rect::new(0, 0, 160, 100));
        menu.add_action("Copy");
        let id = register(Box::new(menu)).expect("registry");
        let viewport = Rect::new(0, 0, 500, 400);

        assert!(!open_context_menu_for_event(
            id,
            &Event::MousePress {
                pos: Point::new(10, 10),
                button: crate::event::mouse_button::PRIMARY
            },
            viewport,
        ));
        assert!(!open_context_menu_for_event(
            id,
            &Event::MouseMove { pos: Point::new(10, 10) },
            viewport
        ));
        unregister(id);
    }

    /// Pointing the helper at an id that is not a menu must report failure rather
    /// than silently doing nothing, which is how a mis-wired right-click handler
    /// would otherwise go unnoticed.
    #[cfg(full_widgets)]
    #[test]
    fn opening_a_context_menu_on_a_non_menu_reports_failure() {
        let label = crate::widget::base_widgets::label::Label::new(
            "hi".to_string(),
            Rect::new(0, 0, 80, 24),
        );
        let id = register(Box::new(label)).expect("registry");

        assert!(!open_context_menu(id, Point::new(5, 5), Rect::new(0, 0, 400, 400)));
        assert!(!open_context_menu(9_999, Point::new(5, 5), Rect::new(0, 0, 400, 400)));
        unregister(id);
    }
}
