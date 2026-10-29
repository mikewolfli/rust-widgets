//! The crate-root procedural host API: lifecycle, creation and surfaces.

#[cfg(all(not(alloc_frugal), not(widgets_unstripped)))]
use crate::NO_SUCH_CONTROL;
#[cfg(not(alloc_frugal))]
use crate::{backend_for_kind, control_backend, widget};
use crate::{platform, trace_runtime_route};

// Named by `create_window`'s documentation; the definition lives in `control`.
#[cfg(not(alloc_frugal))]
pub use super::control::show_widget;

/// Initialize global platform and i18n subsystems.
///
/// One function for every profile (BLUE15 rule #58): the branch that used to be
/// a `cfg`-gated pair now asks [`platform::profile`], so adding a profile does
/// not mean adding a copy of this function.
pub fn init() {
    trace_runtime_route("init");
    platform::profile::runtime_init();
    platform::profile::init_optional_subsystems();
    // One-time menu policy probe, at startup rather than on the first menu draw.
    #[cfg(feature = "advanced-widgets")]
    crate::menu_config::init();
}
/// Run platform main event loop.
pub fn run() {
    trace_runtime_route("run");
    platform::profile::runtime_run();
}
/// Request platform event loop shutdown, from any thread.
///
/// Returns whether this call put the session on a path to ending: `true` means either a loop in
/// this process is running its frames — so it reads the request at its next one and stops,
/// whatever thread this was called from — or the backend performed its own stop from here.
/// `false` means neither: no loop has driven a frame yet and this thread could not stop one, so
/// nothing is reading the request right now. A host that receives `false` and needs the session
/// over immediately must arrange that itself; the request stays recorded, so a loop started
/// later honours it at its first frame.
///
/// # Why this works from a thread the toolkit would refuse
///
/// No toolkit accepts a stop from an arbitrary thread (`-[NSApplication stop:]` is
/// main-thread-only, `gtk_main_quit` walks a global main-loop stack only the loop's thread may
/// touch, `PostQuitMessage` is thread-affine). Recording the request is one atomic store, which
/// every thread may do, and the loop's own frame tick performs the toolkit's stop — every loop
/// this library owns calls `drive_frame` on the thread that owns it. See
/// `platform::stop` for that seam, and [`platform::Platform::quit`] for
/// the backend-level "perform it now" half.
///
/// This is the entry a host should use when it is **not** driving the loop: a smoke test that
/// has to end its own session, CI, or a scripted demo. It is also what makes such a host's exit
/// distinguishable from a killed process, because the loop returns normally.
pub fn quit() -> bool {
    trace_runtime_route("quit");
    platform::profile::runtime_quit()
}

/// Create a top-level window with specified title and geometry.
///
/// The window is created **hidden**. [Show it](show_widget) after its children exist (or at any
/// later time); creating it alone is not a request to map it, so a caller that never shows it
/// gets no window on screen.
///
/// # Example
/// ```
/// let window_id = rust_widgets::create_window("My App", 100, 100, 800, 600);
/// // Build the window's controls first, so the first frame includes them.
/// rust_widgets::show_widget(window_id);
/// ```
#[cfg(not(alloc_frugal))]
pub fn create_window(
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Window).create_window(title, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Create a button control as a child of specified parent.
///
/// The backend decides whether this becomes a platform button or is painted by
/// the platform's custom surface; callers get a handle either way.
pub fn create_button(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Button).create_button(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a check-box. The backend decides whether it is a platform control
/// or is painted by the platform's custom surface.
pub fn create_checkbox(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::CheckBox)
        .create_checkbox(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a single-line text editor. The backend chooses how it is hosted.
pub fn create_line_edit(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::LineEdit)
        .create_line_edit(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a read-only text label. The backend chooses how it is hosted.
pub fn create_label(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Label).create_label(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a radio button. The backend chooses how it is hosted.
pub fn create_radio_button(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::RadioButton)
        .create_radio_button(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a slider. The backend chooses how it is hosted.
pub fn create_slider(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Slider).create_slider(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a progress bar. The backend chooses how it is hosted.
pub fn create_progress_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ProgressBar)
        .create_progress_bar(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a combo box. The backend chooses how it is hosted.
pub fn create_combo_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ComboBox).create_combo_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a list box. The backend chooses how it is hosted.
pub fn create_list_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ListBox).create_list_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a panel (a container surface). The backend chooses how it is hosted.
pub fn create_panel(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    // `WidgetKind::Panel` is the closest always-available kind; on profiles where
    // a dedicated panel kind exists the routing table answers for it.
    backend_for_kind(widget::WidgetKind::Panel).create_panel(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Create a message box dialog as a child of specified parent.
///
/// Creates a message box. The backend chooses how it is hosted.
///
/// Returns `0` on a profile that does not ship the message box (`mini`,
/// `embedded`). Callers must treat `0` as "no control was created" rather than
/// using it as a parent: see the note above `NO_SUCH_CONTROL`.
pub fn create_message_box(
    parent: crate::core::ObjectId,
    title: &str,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::MessageBox)
            .create_message_box(parent, title, text, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, title, text, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Create a file dialog as a child of specified parent.
///
/// Creates a file dialog. The backend chooses how it is hosted.
///
/// Returns `0` on a profile that does not ship the file dialog.
pub fn create_file_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::FileDialog)
            .create_file_dialog(parent, "", x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Create a color dialog as a child of specified parent.
///
/// Returns `0` on a profile that does not ship the color dialog.
pub fn create_color_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::ColorDialog)
            .create_color_dialog(parent, "", x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Create a font dialog as a child of specified parent.
///
/// Returns `0` on a profile that does not ship the font dialog.
pub fn create_font_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::FontDialog)
            .create_font_dialog(parent, "", x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
#[cfg(not(alloc_frugal))]
/// Creates a spin box. The backend chooses how it is hosted.
pub fn create_spin_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::SpinBox).create_spin_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a list view. The backend chooses how it is hosted.
///
/// Returns `0` on a profile that does not ship the item view.
pub fn create_list_view(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::ListView).create_list_view(parent, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Creates a scroll area. The backend chooses how it is hosted.
#[cfg(not(alloc_frugal))]
pub fn create_scroll_area(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ScrollArea).create_scroll_area(parent, x, y, width, height)
}

/// Creates a tab widget. The backend chooses how it is hosted.
///
/// A tab widget's pages are child ids declared with
/// [`app::TabWidgetHandle::add_tab`](crate::app::TabWidgetHandle::add_tab), not the
/// generic `Layout` machinery: a page is selected, not placed in a track.
///
/// # Why the gate is `widgets_unstripped`
///
/// This function names [`widget::WidgetKind::TabWidget`], which exists only where the
/// widget set was not stripped (`widgets_unstripped`, i.e. not `mini` **and** not
/// `embedded`). Gating the constructor with `not(alloc_frugal)` was narrower than the
/// variant it references: `embedded` has `alloc_frugal = false` and
/// `widgets_unstripped = false`, so the constructor compiled and the variant did not,
/// making the profile fail with `no variant ... named TabWidget`. The constructor and
/// the variant must share the same predicate (principle #47).
#[cfg(widgets_unstripped)]
pub fn create_tab_widget(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::TabWidget).create_tab_widget(parent, x, y, width, height)
}

/// Creates a splitter. Its panes are child ids added through
/// [`app::SplitterHandle::add_pane`](crate::app::SplitterHandle::add_pane); ratios are the
/// splitter's own content model, not a generic layout.
/// # Profile gate
///
/// Shares [`create_tab_widget`]'s `widgets_unstripped` gate for the same reason: the
/// `Splitter` variant exists under that predicate and not under `not(alloc_frugal)`.
#[cfg(widgets_unstripped)]
pub fn create_splitter(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Splitter).create_splitter(parent, x, y, width, height)
}

/// Creates a grid widget — a visual row/column matrix, distinct from the generic
/// `layout::GridLayout` placement engine.
/// # Profile gate
///
/// Shares [`create_tab_widget`]'s `widgets_unstripped` gate for the same reason: the
/// `Grid` variant exists under that predicate and not under `not(alloc_frugal)`.
#[cfg(widgets_unstripped)]
pub fn create_grid_widget(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Grid).create_grid(parent, x, y, width, height)
}

/// Creates a generic dialog. Its content child is set with
/// [`app::DialogHandle::set_content_widget`](crate::app::DialogHandle::set_content_widget);
/// the dialog owns the chrome and the content owns its own layout.
/// # Profile gate
///
/// Shares [`create_tab_widget`]'s `widgets_unstripped` gate for the same reason: the
/// `Dialog` variant exists under that predicate and not under `not(alloc_frugal)`.
#[cfg(widgets_unstripped)]
pub fn create_dialog(
    parent: crate::core::ObjectId,
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Dialog).create_dialog(parent, title, x, y, width, height)
}
/// Mounts a widget onto a surface supplied by the host.
///
/// The host platform supplies a window and a drawing surface; the library paints
/// the widget through [`widget::Draw`]. Which surface hosts it — a child window,
/// a drawing area, a view — is decided inside `src/platform/` and never named
/// here; callers only need to know whether the backend can host widgets, which
/// [`supports_surfaces`] answers.
///
/// `id` must already be registered in `widget::runtime`. Prefer the
/// higher-level `app::WindowHandle::mount_surface`, which performs the
/// registration for you and reports failures as a `Result`.
///
/// Returns `false` when the backend has no surface to offer, or when it refuses
/// this particular mount. Backends that cannot host widgets log why.
#[cfg(not(alloc_frugal))]
pub fn mount_surface(
    parent: crate::core::ObjectId,
    id: crate::core::ObjectId,
    rect: crate::core::Rect,
) -> bool {
    platform::get_platform().mount_surface(parent, id, rect)
}

/// Moves and resizes a mounted surface.
#[cfg(not(alloc_frugal))]
pub fn resize_surface(id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
    platform::get_platform().resize_surface(id, rect)
}

/// Unmounts a surface from its window.
#[cfg(not(alloc_frugal))]
pub fn unmount_surface(id: crate::core::ObjectId) -> bool {
    platform::get_platform().unmount_surface(id)
}

/// Marks a mounted surface as needing a repaint.
///
/// Returns `false` when the id is not a surface mounted on the active backend.
#[cfg(not(alloc_frugal))]
pub fn invalidate_surface(id: crate::core::ObjectId) -> bool {
    platform::get_platform().invalidate_surface(id)
}

/// Asks the platform to repaint one rectangle of a mounted widget.
///
/// Returns `true` when the backend narrowed the repaint to `rect`. `false` — including
/// from a backend that never implemented this — means the caller should fall back to
/// [`invalidate_surface`], which repaints the whole widget. Backends are not required
/// to implement it, because several window toolkits offer only whole-widget
/// invalidation.
#[cfg(not(alloc_frugal))]
pub fn invalidate_surface_rect(id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
    platform::get_platform().invalidate_surface_rect(id, rect)
}

/// Asks the platform to repaint one rectangle, or reports that it cannot.
///
/// The alloc-frugal profile has no platform trait to forward to, so this reports
/// `false` and the caller invalidates the whole surface instead — which is what that
/// profile would have done anyway.
#[cfg(alloc_frugal)]
pub fn invalidate_surface_rect(_id: crate::core::ObjectId, _rect: crate::core::Rect) -> bool {
    false
}

/// Removes and returns the next widget whose surface is awaiting a repaint.
///
/// # Why a host outside the process needs this
///
/// On the three desktop backends the library drives the event loop, so an
/// invalidation reaches the toolkit's own "this window is dirty" call directly. A
/// host that owns its own loop and its own drawing API — OpenHarmony (ArkUI
/// `Canvas`/XComponent), Android (`Bitmap`), iOS (`NSView`), the objc2 preview —
/// cannot be called into that way, so the backend records the stale surfaces and the
/// host pulls them. This is that pull, and it is the function
/// [`rw_take_pending_repaint`](crate::bindings) exposes to the C ABI.
///
/// # Coalescing
///
/// A widget already awaiting a repaint is not queued twice, so a burst of
/// invalidations in one frame costs the host one entry. That is what makes a drag
/// that produces hundreds of geometry updates pay for one repaint per frame rather
/// than one per update.
///
/// `None` when nothing is pending — the honest answer rather than an empty sentinel
/// id, because `0` is a valid `ObjectId` in no backend but is equally not a widget
/// this call could have returned.
#[cfg(not(alloc_frugal))]
pub fn take_pending_repaint() -> Option<crate::core::ObjectId> {
    platform::get_platform().take_pending_repaint()
}

/// The alloc-frugal profile's [`take_pending_repaint`].
///
/// Reports `None` because this profile has no widget registry and therefore keeps no
/// repaint queue to drain — the same truthful "nothing is pending" a full build
/// answers for a window with no damage. See the full build's version for why the
/// function exists at all.
#[cfg(alloc_frugal)]
pub fn take_pending_repaint() -> Option<crate::core::ObjectId> {
    None
}

/// Returns `true` when the active backend can host library-painted widgets.
#[cfg(not(alloc_frugal))]
pub fn supports_surfaces() -> bool {
    platform::get_platform().supports_surfaces()
}

/// Delivers a pointer event that arrived on a host-owned drawing surface.
///
/// # Why this exists, and why it was missing
///
/// On the three desktop backends the library drives the event loop, so a click arrives
/// through the toolkit's own callback and is routed inside it (`windows/types.rs`'
/// window procedure, `linux/platform_impl.rs`' GTK handler). A host that owns its own
/// loop — OpenHarmony's ArkUI, Android's `Activity`, iOS's touch handling, a browser page —
/// **is never called into**, so it has the opposite obligation: it must hand its touches
/// back. Until this function existed there was no way to do that.
///
/// That was not a documentation gap. `src/platform/android/status.md`,
/// `src/platform/ios/status.md` and `docs/plans/harmony_integration.md` all told a host to
/// call `rw_dispatch_pointer_event(...)`, and `grep -rn rw_dispatch_pointer_event src/
/// include/` returned nothing: the documented integration step — without which a mounted
/// widget is visible but completely inert — had no implementation at any layer.
///
/// # What `root` is, and why a hit test rather than an id
///
/// `point` is in the **window's** coordinate space, and `root` names the widget subtree to
/// resolve it in — normally the window. The library hit-tests the point against that
/// subtree ([`crate::widget::runtime::widget_at`]), so the host does not have to know which
/// widget was under the finger. That is the whole point of keeping the routing here: a host
/// that sent the event to a widget id would need the same hit test the library already owns,
/// and the two would drift.
///
/// Returns whether a widget accepted the event. `false` means the point was outside every
/// widget in `root`, or the resolved widget refused it (disabled, or blocked by a modal) —
/// never "this host cannot route", which is a distinct case the alloc-frugal profile
/// reports through [`route_pointer_event`](crate::platform::Platform::route_pointer_event).
#[cfg(not(alloc_frugal))]
pub fn dispatch_pointer_event(
    root: crate::core::ObjectId,
    event: &crate::event::Event,
    point: crate::core::Point,
) -> bool {
    platform::get_platform().route_pointer_event(root, event, point)
}

/// The alloc-frugal profile's [`dispatch_pointer_event`].
///
/// Reports `false` because this profile compiles out the widget registry, so there is no
/// subtree to hit-test and no widget that could accept the event. That is the same honest
/// "this host cannot do it" answer [`crate::platform::Platform::route_pointer_event`]'s
/// default gives, rather than a silent no-op that looks like a delivered event.
#[cfg(alloc_frugal)]
pub fn dispatch_pointer_event(
    _root: crate::core::ObjectId,
    _event: &crate::event::Event,
    _point: crate::core::Point,
) -> bool {
    false
}

/// Delivers an event straight to `widget_id`, without hit-testing.
///
/// The counterpart to [`dispatch_pointer_event`] for the cases where the host **already
/// knows** the target: a key event for the focused control, a text-input commit, a scroll
/// from a surface the host tracks itself. Routing a pointer event this way skips the
/// library's hit test, so a host that uses it for touches must do its own — which is why
/// the pointer path above exists and is the one the integration documents name.
///
/// Returns whether the widget accepted the event: `false` when the id is not a live widget, or
/// when a modal blocks it. A **disabled** control is refused by the pointer router
/// ([`dispatch_pointer_event`]) before delivery, which is where the crate's "the router skips
/// disabled controls when delivering" rule lives; this entry point delivers to whatever live id it
/// is handed, so a host that routes input straight to a control it has disabled must check its own
/// state.
#[cfg(not(alloc_frugal))]
pub fn dispatch_event(widget_id: crate::core::ObjectId, event: &crate::event::Event) -> bool {
    crate::widget::runtime::dispatch_event(widget_id, event)
}

/// The alloc-frugal profile's [`dispatch_event`].
///
/// `false` for the same reason as [`dispatch_pointer_event`]: no registry, so no widget.
#[cfg(alloc_frugal)]
pub fn dispatch_event(_widget_id: crate::core::ObjectId, _event: &crate::event::Event) -> bool {
    false
}

/// Completes a theme switch by re-styling the controls the switch notified (G-404).
///
/// # How a switch reaches mounted controls
///
/// Every mounted control subscribes to the theme manager's `theme_changed` signal at
/// registration (see `widget::runtime::finish_registration`). A switch — `set_theme`,
/// `set_appearance`, the high-contrast override, a system-accent refresh, a loaded-and-activated
/// theme — emits that signal, and each subscription records its control in the runtime's
/// pending-restyle set. This call drains that set: each notified control is re-merged with the
/// active theme through the same `theme::apply_active_theme` a newly created control gets, so an
/// explicit per-control style still wins (see `theme::apply`).
///
/// # Why the restyle is a drain, not a synchronous notification
///
/// The signal fires while the switching caller holds the theme mutex, and the re-merge resolves
/// theme styles — which takes that same non-reentrant mutex. Restyling inside the slot would
/// deadlock, so the slot only records intent and the merge happens here, after the manager guard
/// is released, or at the next frame (`drive_frame` step 0a) for switch paths that do not call
/// this.
///
/// # Coverage
///
/// A control subscribes exactly once, at registration, so the pending set equals the set of
/// controls mounted on the switching thread — the same set the previous full walk of the
/// registry restyled, minus the blind re-merge of controls nothing notified about.
/// `tests/theme_application_test.rs` pins the parity: mount, switch, assert restyled.
///
/// Without a preceding switch the set is empty and this is a no-op: nothing has notified, so no
/// control carries a style that disagrees with the active theme. A profile without a theme
/// module has nothing to apply and does nothing.
#[cfg(not(alloc_frugal))]
pub fn reapply_active_theme() {
    #[cfg(device_profile)]
    {
        widget::runtime::drain_pending_theme_restyle();
    }
}

/// Switches to the appearance the **device** asks for, then restyles every live widget.
///
/// # Why the two steps are one call
///
/// Choosing a theme and painting the widgets that already exist are a single user-visible outcome:
/// a host that called only the first would have the right `current_theme()` and the old palette on
/// screen until something else forced a repaint. Composing them here is what keeps "the app follows
/// the system appearance" from depending on the caller remembering the sweep.
///
/// Returns whether the appearance was recognised, so a host can tell "nothing needed doing"
/// (the device asks for the appearance already active) from "no theme declares that appearance"
/// (a packaging error worth surfacing). See
/// [`ThemeManager::follow_environment_appearance`](crate::theme::ThemeManager::follow_environment_appearance)
/// for what each [`ThemeMode`](crate::style::ThemeMode) resolves to.
///
/// # When to call it
///
/// The device preference is read through `crate::style::environment`, whose snapshot is taken once
/// per frame by `drive_frame`. A host that learns of a system appearance
/// change therefore calls this between frames — which is exactly what the environment's own docs
/// describe ("a host that changed a system setting between frames sees it take effect on the next
/// one"). A profile without a theme module has nothing to switch and reports `false`.
#[cfg(not(alloc_frugal))]
pub fn apply_environment_appearance() -> bool {
    #[cfg(device_profile)]
    {
        let changed = crate::theme::global_theme_manager().follow_environment_appearance();
        if changed {
            reapply_active_theme();
        }
        changed
    }
    #[cfg(not(device_profile))]
    {
        // No theme registry in this profile, so no appearance to follow. Reporting `false` is the
        // truthful answer rather than a fabricated "changed".
        false
    }
}

/// Creates a widget of any kind — **the single, mechanism-free creation entry**.
///
/// # How this differs from the per-kind `create_*` functions
///
/// The `create_*` functions are the typo-safe spelling for the common widgets
/// (`create_button`, `create_slider`, …). This function is the general form: hand
/// it the same `WidgetKind` spelling the JSON loader and demo shell use.
///
/// # What this does now
///
/// Every kind is painted by the library (BLUE15 rule #55), so there is a single
/// path: build (or accept) the `Box<dyn Widget>` and hand it to the host surface
/// through `widget::runtime`. The `Platform` layer supplies a surface to paint on,
/// not a control to map onto, so no branch here asks which mechanism to use.
///
/// # The `widget` argument
///
/// A caller may supply a pre-built object so that a widget with constructor
/// arguments the factory cannot express (`CodeEditor` with initial text, a chart
/// with series) reaches this path without a bespoke branch. When it is `None` the
/// widget factory is asked; that factory is the single place that knows every
/// kind's constructor (rule #65).
///
/// Returns `0` — a truthful "not created" — when the kind has no constructor and
/// the caller supplied no object. Callers already handle `0` for the per-kind
/// functions, so this needs no separate error channel.
#[cfg(not(alloc_frugal))]
pub fn create_widget_of_kind(
    kind: widget::WidgetKind,
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    widget: Option<Box<dyn widget::Widget>>,
) -> crate::core::ObjectId {
    let rect = crate::core::Rect::new(x, y, width, height);

    // Prefer the caller's object; otherwise build from the factory, which is the
    // only component that knows every kind's constructor.
    //
    // The gate is `full_widgets`, not `device_profile`: the factory and
    // `kind_name` both require the registry, which exists only when a device
    // profile is present *and* the widgets were not stripped. Spelling it
    // `device_profile` made the callee narrower than its caller, so a build that
    // overlapped two axis-1 features (`--features "tablet,embedded"`) failed with
    // `cannot find function kind_name` / `cannot find WidgetFactory`. `full_widgets`
    // is exactly the conjunction the body needs, so there is no narrower gate left
    // for this call site to drift away from (principle #47).
    let widget = widget.or_else(|| {
        #[cfg(all(device_profile, full_widgets))]
        {
            // Direct kind lookup: `create(&kind_name(..))` resolved the kind to a canonical
            // name (allocating a `String`), then `create` normalised that name and looked the
            // capability up again. `create_by_kind` uses the same constructor without the
            // round trip; the name path is only the fallback for a kind the registry lists by
            // name but not by kind.
            let factory = widget::WidgetFactory::shared();
            factory.create_by_kind(kind, rect, text).or_else(|| {
                let name = widget::capability::factory_name_for_kind(kind)?;
                factory.create(name, rect, text)
            })
        }
        #[cfg(not(all(device_profile, full_widgets)))]
        {
            let _ = (text, rect);
            None
        }
    });

    let Some(widget) = widget else {
        log::warn!(
            "create_widget_of_kind: no constructor for {kind:?} and no widget object was \
             supplied; returning 0"
        );
        return 0;
    };

    // A backend that has no per-widget surface has exactly one way to host a control: the
    // control backend's widget-tree path, which is also what every `create_*` function uses.
    // Sending the object to `mount_widget_object` first would register it, fail the mount,
    // unregister it and drop it — so this branch has to be taken **before** the surface path,
    // not as a recovery after it. That asymmetry is what made `create_widget_of_kind` return
    // 0 for every kind on a native desktop backend while the typed constructors succeeded.
    if !supports_surfaces() {
        // Parity with the typed constructors: a non-window control needs a live parent, or
        // the returned id would address a widget no tree can reach. The surface path got this
        // for free because `mount_surface` refuses an unknown parent; the widget-tree path has
        // to say it explicitly.
        if kind != widget::WidgetKind::Window
            && (parent == 0 || !widget::runtime::is_mounted(parent))
        {
            log::warn!(
                "create_widget_of_kind: refusing to create {kind:?} under parent {parent}, which \
                 addresses no live container; returning 0"
            );
            return 0;
        }
        let id = control_backend::get_control_backend().mount_widget_box(widget, Some(parent), 0);
        if id == 0 {
            log::warn!(
                "create_widget_of_kind: {kind:?} could neither be hosted on a platform \
                 surface nor adopted by the widget-tree backend; returning 0"
            );
        }
        return id;
    }

    match crate::mount_widget_object(parent, widget, rect) {
        Ok(id) => id,
        Err(error) => {
            log::warn!("create_widget_of_kind: cannot host {kind:?}: {error}");
            0
        }
    }
}

/// Stub for mini mode (no platform runtime, no windows).
#[cfg(alloc_frugal)]
pub fn mount_surface(
    _parent: crate::core::ObjectId,
    _id: crate::core::ObjectId,
    _rect: crate::core::Rect,
) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn resize_surface(_id: crate::core::ObjectId, _rect: crate::core::Rect) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn unmount_surface(_id: crate::core::ObjectId) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn invalidate_surface(_id: crate::core::ObjectId) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn supports_surfaces() -> bool {
    false
}
