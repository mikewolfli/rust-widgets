// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The custom-paint backend's mounting entry points.
//!
//! Kept out of `mod.rs` (principle #8). The type's `state` field is `pub(crate)`, so an
//! inherent impl may live in a sibling module of the module that defines the type.

use super::CustomPaintControlBackend;
// The constructor-name resolver exists only where a constructor registry is compiled.
#[cfg(full_widgets)]
use super::property_names::kind_factory_name;

impl CustomPaintControlBackend {
    /// Builds a widget of `kind` and mounts it on the host surface.
    ///
    /// This is the single creation path for every `create_*` method on this backend
    /// (BLUE15 rules #55/#65): the object comes from `WidgetFactory`, which is the
    /// only component that knows every kind's constructor, and it is handed to
    /// `crate::widget::runtime`, which owns it from then on.
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
            let Some(name) = kind_factory_name(kind) else {
                log::warn!(
                    "custom backend: {kind:?} has no resolvable constructor; returning 0 rather \
                     than an id that addresses nothing"
                );
                return 0;
            };
            self.mount_named_widget(name, parent, text, x, y, width, height)
        }
    }

    /// Mounts the control registered under `name`, which the factory resolves.
    ///
    /// # Why this takes a name rather than a kind
    ///
    /// Several controls share one `WidgetKind` (`chart`, `timeline_widget` and
    /// `gantt_widget` all report `WidgetKind::Chart`). Resolving a kind back to a
    /// name can only ever answer with one of them, so a caller that asked for
    /// `timeline_widget` would silently receive `chart`. Taking the name keeps the
    /// caller's choice intact; `mount_widget_of_kind` is the kind-addressed spelling
    /// for the typed `create_*` methods, where the kind *is* the whole request.
    ///
    /// Returns `0` when the name is unknown, when the parent is not a live
    /// container, or when the factory produced no widget.
    #[cfg(full_widgets)]
    pub(crate) fn mount_named_widget(
        &self,
        name: &str,
        parent: crate::core::ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> crate::core::ObjectId {
        let rect = crate::core::Rect::new(x, y, width, height);
        let factory = crate::widget::WidgetFactory::shared();
        let Some(capability) = factory.capability(name) else {
            log::warn!(
                "custom backend: no control is registered under the name {name:?}; returning 0"
            );
            return 0;
        };
        let kind = capability.kind;
        let Some(mut widget) = crate::widget::WidgetFactory::shared().create(name, rect, text)
        else {
            log::warn!(
                "custom backend: the factory lists {name:?} but produced no widget; returning 0"
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
                    "custom backend: refusing to create {name:?} under parent {parent}, which \
                     addresses no live container"
                );
                return 0;
            }
            widget.set_parent(Some(parent));
        }
        // Apply the active theme before registering, matching the crate-root
        // funnel (`mount_widget_object`). Both must do it: a control created
        // through the C ABI and one mounted onto a window take different code
        // paths, so a theme applied on only one of them would make appearance
        // depend on how the control was created.
        //
        // The call is unconditional and the body is gated (see
        // `crate::apply_active_theme`), so this block carries no `cfg` of its
        // own to drift out of step with the theme module's gate.
        crate::apply_active_theme(&mut widget);
        self.adopt_widget_box(widget, Some(parent), 0)
    }

    /// Mounts an already-built widget box: assigns and registers its id, records both
    /// directions of the parent/child relation, and gives a window its host surface.
    ///
    /// # Why this is one function
    ///
    /// Both entry points — a caller naming a kind ([`mount_named_widget`](Self::mount_named_widget))
    /// and a caller handing over a box it built itself
    /// ([`mount_widget_box`](ControlBackend::mount_widget_box), which is what the JSON layout
    /// loader uses) — must end up with the same mounted control. Splitting the bookkeeping in two
    /// is how the loader's path came to register nothing while this path registered everything.
    ///
    /// `declared_id` is the id the caller wants the widget known by, or `0` to have one
    /// assigned. It is *reserved* rather than merely written down: the runtime must hand out
    /// that exact id, or a document's `"id"` would address a control the runtime never created.
    ///
    /// Returns `0` when there is no widget runtime on this thread, or when the reservation
    /// loses a race with another producer.
    ///
    /// # Why every field of the widget is read rather than passed in
    ///
    /// The name-based path used to be handed the title, geometry and kind it had just built from,
    /// because it built the control itself. This one is handed only the box, so those facts are
    /// read back **from the control** — which is the more honest source anyway: what the host
    /// should be told about a widget is what the widget says about itself.
    /// # Why this is absent on `alloc_frugal`
    ///
    /// The alloc-frugal profile has no widget runtime at all (`widget::runtime` is compiled out
    /// there), so there is no registry to mount into and no id to reserve. `mount_widget_box` on
    /// the trait already answers `0` for such a backend, which is the honest answer.
    #[cfg(not(alloc_frugal))]
    pub(crate) fn adopt_widget_box(
        &self,
        widget: crate::compat::Box<dyn crate::widget::Widget>,
        parent: Option<crate::core::ObjectId>,
        declared_id: crate::core::ObjectId,
    ) -> crate::core::ObjectId {
        let id = if declared_id == 0 {
            crate::widget::runtime::register(widget).unwrap_or(0)
        } else {
            match crate::widget::runtime::register_with_id(widget, declared_id) {
                Ok(id) => id,
                Err(existing) => {
                    // The id is taken. Handing out a second id would leave the document's
                    // `"id"` pointing at somebody else's control, so the request is refused
                    // and the duplicate is named.
                    log::warn!(
                        "custom backend: id {existing} is already mounted; a widget asking to be \
                         known by it was refused rather than given a different id"
                    );
                    return 0;
                }
            }
        };
        if id == 0 {
            return 0;
        }

        // Record the parent/child relation in **both** directions.
        //
        // `BaseWidget::set_parent` documents that it "does not update the old or new
        // parent's child list, so the two directions must be kept in sync by the
        // caller" — and no caller did. The child knew its parent while the parent
        // listed no children.
        //
        // That was invisible while every visible control had a native widget of its own.
        // It stopped being invisible when the Linux backend, which creates no native
        // controls, needed a window-level painter: that paints the window's child list,
        // and an empty list is why a window rendered as bare background.
        //
        // Both writes happen here because this is the one funnel every mounted control
        // passes through, so the two lists cannot be updated on one path and forgotten on
        // another. A window is excluded: it is a root, and it has no parent id to record
        // (its own parent is `None`, set by the factory).
        if let Some(parent_id) = parent {
            crate::widget::runtime::with_widget_mut(parent_id, |host| {
                host.add_child(id);
            });
        }
        // The window's host object is created **before** anything asks for a repaint.
        //
        // # Why the order is load-bearing
        //
        // `request_repaint_subtree` below walks up to the top-level window and asks the
        // platform to repaint it, and that request can only be honoured through the
        // registry-id → platform-id association `attach_window_host_if_needed` records.
        // Running it afterwards meant the first request of a window's life — the one the
        // comment below describes, "a control just added" — landed on a window the
        // backend could not resolve, so the frame stayed as it was drawn when the window
        // was still empty: a blank client area.
        //
        // It has to be **after** `add_child` as well: the host window's own creation can
        // paint it (Win32 shows and updates a new window immediately), and that early
        // paint must already be able to see the child it is about to include.
        #[cfg(full_widgets)]
        self.attach_window_host_if_needed(id);

        // A new control must become visible, and only the **window** can make that
        // happen: it paints the whole child list, and the control just added has no
        // surface of its own to invalidate. Without this the window kept the frame it
        // drew when it was still empty — a blank client area that no later call
        // repaired, because every other repaint request named the child.
        //
        // A window is its own case and has nothing to do here: it has no parent to
        // repaint it, and `add_child` above did not run for it.
        if parent.is_some() {
            crate::widget::runtime::request_repaint_subtree(id);
        }

        // The host window and the theme-driven title are `full_widgets` concerns: a stripped
        // profile has no `crate::platform::get_platform` to ask and no `String` to name it with,
        // and it mounts no controls for a host to draw. The registration above is ungated because
        // those profiles *do* own a widget runtime (they are not `alloc_frugal`), so an id handed
        // out there is a real id.

        id
    }

    /// Gives a **window** control its host object on the platform, and a non-window nothing.
    ///
    /// # Why a window needs this and a control does not
    ///
    /// A window is not only a painted widget: it needs a host object for the platform to draw
    /// into, and that is what `mount_surface` resolves a parent through. Creating it here — on the
    /// one mounting path — is what links the widget id the caller holds to the id the platform
    /// knows, instead of leaving two unreachable id spaces.
    ///
    /// A backend without host windows (state-only, e.g. no display) returns `0` and no association
    /// is recorded, so mounting onto that window is still refused honestly rather than appearing to
    /// succeed.
    ///
    /// Every fact the host needs is read **from the control**: its kind decides whether this runs
    /// at all, its geometry is where the host window opens, and its accessible name is its title
    /// (`accessible_name` resolves the kind's own label property, which for a window *is* the
    /// title — read that way rather than through a downcast, because `Widget` publishes no title
    /// accessor and one kind is not a reason to widen the trait).
    #[cfg(full_widgets)]
    fn attach_window_host_if_needed(&self, id: crate::core::ObjectId) {
        let is_window = crate::widget::runtime::with_widget(id, |live| {
            live.kind() == crate::widget::WidgetKind::Window
        })
        .unwrap_or(false);
        if !is_window {
            return;
        }
        let (x, y, width, height, title) = crate::widget::runtime::with_widget(id, |live| {
            let rect = live.geometry();
            (rect.x, rect.y, rect.width, rect.height, live.accessible_name())
        })
        .unwrap_or((0, 0, 0, 0, crate::compat::String::new()));
        let host = crate::platform::get_platform().create_window(&title, x, y, width, height);
        if host != 0 {
            crate::widget::runtime::set_host_window(id, host);
            // The association exists, so the window can now resolve a repaint — and it must be
            // asked, because the host may already have painted.
            //
            // # Why attaching is not enough on its own
            //
            // `create_window` on Win32 shows and updates its new window immediately, so the
            // host's own `WM_PAINT` runs **before** the caller has built a single child. That
            // first paint cannot include children that do not exist yet — correctly, they are
            // not there — and it leaves the window holding a frame of an empty client area.
            //
            // Attaching the association fixes the *lookup*; it does not by itself make anyone
            // paint again. The repaint request for a control is issued by the caller
            // (`adopt_widget_box` asks for the child's subtree, which walks up to this window),
            // so a window whose first child is added later is covered — but a window created
            // and left without children, or shown before its children were adopted, kept the
            // empty frame until some unrelated expose arrived. That is the "blank until you
            // nudge it" report this closes.
            //
            // The request comes **after** `set_host_window` for the reason the caller's
            // comment gives: a request the backend cannot resolve is a request that does
            // nothing.
            crate::widget::runtime::request_repaint_subtree(id);
        } else {
            log::debug!(
                "custom backend: backend '{}' built no host window for {id}; controls cannot be \
                 mounted onto it",
                crate::platform::backend_name()
            );
        }
    }

    /// Runs `f` against the live widget registered under `widget_id`.
    ///
    /// Every accessor on this backend reads the widget's **own** state instead of a
    /// shadow copy, so a property cannot disagree with the control it describes
    /// (BLUE15 §10.3: the shadow maps were one of three duplicated truths).
    ///
    /// The gate is `not(alloc_frugal)` because the alloc-frugal profile has no widget
    /// registry to read. A nested `#[cfg(alloc_frugal)]` arm returning `None` used to
    /// sit in the body as well; `alloc_frugal` cannot be true inside an item that only
    /// exists when it is false, so that arm was unreachable in every configuration and
    /// has been removed. Callers already carry their own `alloc_frugal` arm (see
    /// `create_widgets_helpers.in.rs`), so the honest `None` for a registry-free build
    /// is still reachable where it matters.
    #[cfg(not(alloc_frugal))]
    pub(crate) fn with_live_widget<R>(
        &self,
        widget_id: crate::core::ObjectId,
        f: impl FnOnce(&mut dyn crate::widget::Widget) -> R,
    ) -> Option<R> {
        crate::widget::runtime::with_widget_mut(widget_id, f)
    }
}
