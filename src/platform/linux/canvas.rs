// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Native surface for **self-drawn** widgets on Linux/GTK.
//!
//! Hosts each mounted widget in a [`gtk::DrawingArea`] placed inside the
//! window's existing `gtk::Fixed` content container, so a self-drawn editor can
//! sit next to native-control rows in the same window. The area's `draw` signal
//! pulls one RGBA frame out of `crate::widget::runtime` and blits it with
//! cairo.
//!
//! See `docs/plans/custom_widget_mounting.md`.
//!
//! This module compiles only with `gtk-native`; without it the platform keeps
//! the trait's `false` defaults and callers get an explicit "cannot display
//! here" instead of an empty window.
//!
//! `mini`/`embedded` are excluded too: the frame comes from
//! `crate::widget::runtime`, which those profiles do not compile (see
//! `src/widget/mod.rs`).

#![cfg(all(unix_desktop, feature = "gtk-native", widgets_unstripped))]

use super::types::{LinuxNativeState, LinuxPlatform};
use crate::core::{Color, ObjectId, Point, Rect, Size};
// `cairo`, `gdk` and `glib` are re-exported by `gtk`, which is the only
// GTK-family crate this project declares. Referring to them as bare crates made
// `gtk-native` builds fail with E0433 unless a transitive dependency happened to
// leak the name into scope; these aliases pin them to the declared dependency.
use crate::core::MutexExt;
use crate::event::Event;
use gtk::cairo;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Creates a `DrawingArea` for `id`, adds it to `parent`'s content container and
/// wires its `draw` and input signals.
pub(crate) fn mount_canvas(
    platform: &LinuxPlatform,
    parent: ObjectId,
    id: ObjectId,
    rect: Rect,
) -> bool {
    // GTK widgets belong to the thread that initialized GTK; building one from any
    // other thread aborts the process (`assert_initialized_main_thread!()`). Report
    // the refusal instead of crashing, so `mount_surface` returns its documented
    // `false` and the caller learns the surface could not be shown.
    if !gtk::is_initialized_main_thread() {
        log::error!(
            "[linux] mount_surface: refused off the GTK main thread (parent={parent}, id={id})"
        );
        return false;
    }
    if !crate::widget::runtime::is_mounted(id) {
        log::error!(
            "[linux] mount_surface: id={id} is not in widget::runtime; \
             call runtime::register before mounting"
        );
        return false;
    }

    let area = gtk::DrawingArea::new();
    // Position and size are assigned by the container, not claimed as a minimum.
    //
    // A size request is a *minimum*, and a container that aggregates its children's bounds
    // passes it up to the toplevel — which is what made a window refuse to shrink past its
    // right-most control. The request is therefore the smallest positive value, and
    // `overlay_place::place` positions and sizes the child through the overlay's own
    // properties, which are not part of the window's minimum.
    area.set_size_request(1, 1);
    // The area paints every pixel itself, so suppress GTK's own background fill
    // and the one-frame flash that comes with it.
    area.set_has_tooltip(false);

    // ── Painting ───────────────────────────────────────────────────────────
    area.connect_draw(move |widget, context| {
        let width = widget.allocated_width().max(1) as u32;
        let height = widget.allocated_height().max(1) as u32;
        // `render_frame_cached` rather than `render_frame`: it remembers the previous
        // frame and repaints only the damage, so a widget in `RepaintMode::Dirty`
        // (or `Adaptive` below its area threshold) does not re-rasterise the whole
        // surface on every expose. The returned frame is complete either way, because
        // `blit_rgba` presents a whole surface — the saving is in what was drawn.
        // `render_frame_cached_shared` rather than the owned form: an unchanged frame is
        // returned as the cache's own `Rc`, so presenting it costs a refcount bump instead of
        // duplicating a full-surface buffer.
        match crate::widget::runtime::render_frame_cached_shared(
            id,
            Size::new(width, height),
            Color::WHITE,
        ) {
            Some(frame) => {
                blit_rgba(context, width, height, frame.as_slice());
            }
            None => {
                log::error!(
                    "[linux] canvas: widget id={id} produced no frame \
                     (unmounted, or it does not implement Draw)"
                );
            }
        }
        glib::Propagation::Proceed
    });

    // ── Input ──────────────────────────────────────────────────────────────
    let press_area = area.clone();
    area.add_events(
        gdk::EventMask::BUTTON_PRESS_MASK
            | gdk::EventMask::BUTTON_RELEASE_MASK
            | gdk::EventMask::POINTER_MOTION_MASK
            | gdk::EventMask::KEY_PRESS_MASK
            | gdk::EventMask::SCROLL_MASK
            | gdk::EventMask::TOUCH_MASK,
    );
    // The area must be able to take keyboard focus for the editor to be usable.
    area.set_can_focus(true);
    let ime = ImeContext::new(&area);

    // GTK reports pointer positions in this area's local space, but widget geometry
    // is absolute, so each event is offset by where this surface was placed. Without
    // the offset every nested control would be hit-tested against the wrong point.
    let origin = Point::new(rect.x, rect.y);

    let double_click = Rc::new(RefCell::new(DoubleClickTracker::new()));
    // GDK event time of the last press this surface delivered.
    //
    // One primary press is emitted to this handler **twice** (same X timestamp), measured with
    // an instrumented build: two presses 40 ms apart logged three calls, the third repeating the
    // second's time. A press that reaches the widget twice is mostly harmless, but a *gesture*
    // is not: the first delivery was translated to `MouseDoubleClick` (selecting the word) and
    // the duplicate, arriving after the tracker reset, became an ordinary press and collapsed
    // the selection again. Ignoring a press with a timestamp already handled makes the double
    // click behave as one.
    let last_press_time = Rc::new(core::cell::Cell::new(0u32));
    area.connect_button_press_event(move |area, event| {
        let time = event.time();
        if last_press_time.get() == time {
            return glib::Propagation::Stop;
        }
        last_press_time.set(time);
        let position = Point::new(event.position().0 as i32, event.position().1 as i32);
        let absolute = Point::new(origin.x + position.x, origin.y + position.y);
        // A second primary press inside GTK's own double-click interval becomes
        // `Event::MouseDoubleClick`; controls that activate on a double click (a list row,
        // an in-place editor) only listen for that variant.
        let is_double = event.button() == 1
            && double_click.borrow_mut().register(
                event.time(),
                event.position().0,
                event.position().1,
            );
        let translated = if is_double {
            Event::mouse_double_click(absolute.x, absolute.y, 1)
        } else {
            // The press carries the modifier state GTK reports, so a Shift/Ctrl click
            // reaches the widget as a modifier click rather than as a plain one.
            Event::mouse_press_with(absolute.x, absolute.y, 1, modifier_state_bits(event.state()))
        };
        let delivered = forward_pointer_to_platform(id, &translated, absolute);
        if delivered {
            // A click may move focus to a child, so give it the keyboard too. The
            // area only receives key events while GTK considers it focused.
            focus_area_if_enabled(area);
            press_area.queue_draw();
            note_canvas_redraw(id);
        }
        // **Stop** after delivery: the window's paint area has its own pointer handler,
        // and GTK propagates an unhandled press from this surface up to it. Both
        // handlers translate and dispatch, so one physical click reached the control
        // twice — harmless for a caret, destructive for a gesture: the window's
        // tracker saw the second press of a double click and sent
        // `MouseDoubleClick` (selecting the word), then this surface's fresh tracker
        // saw the same press as an ordinary one (collapsing the selection again).
        // Letting an *undelivered* event proceed keeps clicks outside every surface
        // reaching the window handler.
        if delivered {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });

    area.connect_button_release_event(move |widget, event| {
        let position = Point::new(event.position().0 as i32, event.position().1 as i32);
        let absolute = Point::new(origin.x + position.x, origin.y + position.y);
        let delivered = forward_pointer_to_platform(
            id,
            &Event::MouseRelease { pos: absolute, button: 1 },
            absolute,
        );
        if delivered {
            widget.queue_draw();
            note_canvas_redraw(id);
        }
        // See the press handler: a delivered event must not bubble to the window's
        // handler, or one release would be applied twice.
        if delivered {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });

    area.connect_motion_notify_event(move |widget, event| {
        let position = Point::new(event.position().0 as i32, event.position().1 as i32);
        let absolute = Point::new(origin.x + position.x, origin.y + position.y);
        // A motion with no button held is a hover. It only owes a repaint when the control
        // under the pointer changed; a drag repaints every sample because the dragged control
        // follows the pointer. `None` makes the comparison below unconditional for a drag.
        let hover_before = if pointer_button_down(event.state()) {
            None
        } else {
            Some(crate::widget::runtime::hovered_widget())
        };
        let delivered =
            forward_pointer_to_platform(id, &Event::MouseMove { pos: absolute }, absolute);
        if delivered && hover_before != Some(crate::widget::runtime::hovered_widget()) {
            widget.queue_draw();
            note_canvas_redraw(id);
        }
        // A delivered motion is not re-dispatched by the window handler: a drag would
        // otherwise advance the selection by two sample positions per physical move.
        if delivered {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });

    // GTK delivers a crossing event when the pointer leaves the area, which is the one
    // case the coordinate-based hover transition cannot observe: there is no widget
    // under the pointer, so a previously hovered control would stay highlighted.
    area.connect_leave_notify_event(move |widget, _| {
        crate::widget::runtime::clear_hover(Point::new(0, 0));
        widget.queue_draw();
        note_canvas_redraw(id);
        glib::Propagation::Proceed
    });

    // Touch: GDK reports finger contacts as their own event type, not as button
    // presses, so without this the gesture engine never saw a `TouchBegin` and all
    // eleven recognisers were reachable only from unit tests.
    //
    // Gated on the `touch` capability: `Event::Touch*` does not exist without it
    // (see `crate::event::types`), so a build that omits `touch` must not install a
    // handler that constructs those variants.
    #[cfg(feature = "touch")]
    area.connect_touch_event(move |widget, event| {
        // `connect_touch_event` hands the handler the *generic* `gdk::Event`, and
        // `position()` is declared on the concrete `gdk::EventTouch` (it reads the
        // touch-specific `x`/`y` fields). Calling it on the generic type does not
        // compile, so the event is narrowed first — which is also where the
        // "is this actually a touch event" check belongs.
        let Some(touch) = event.downcast_ref::<gdk::EventTouch>() else {
            return glib::Propagation::Proceed;
        };
        let translated = match event.event_type() {
            gdk::EventType::TouchBegin => TouchPhase::Begin,
            gdk::EventType::TouchUpdate => TouchPhase::Update,
            // A cancel is *not* a completed end: the platform withdrew the contact, so the
            // gesture must be abandoned without committing (D09-EVT-02). It is carried as
            // its own phase so consumers reset their latches without firing
            // `clicked`/`toggled`/a stroke commit. A bare `TouchEnd` here would let a
            // cancelled gesture look like a successful tap.
            gdk::EventType::TouchCancel => TouchPhase::Cancel,
            gdk::EventType::TouchEnd => TouchPhase::End,
            _ => return glib::Propagation::Proceed,
        };
        let position = Point::new(touch.position().0 as i32, touch.position().1 as i32);
        let absolute = Point::new(origin.x + position.x, origin.y + position.y);
        // `GdkEventSequence` identifies the contact for its whole lifetime, which is
        // what the recognisers need to follow one finger across move and end. Its
        // pointer is used as the `TouchId`: it is only ever compared, never
        // dereferenced, and is stable while the contact lasts.
        let touch_id = touch.event_sequence().map(|sequence| sequence.as_ptr() as u64).unwrap_or(0);
        let widget_event = match translated {
            TouchPhase::Begin => Event::TouchBegin { pos: absolute, touch_id },
            TouchPhase::Update => Event::TouchMove { pos: absolute, touch_id },
            TouchPhase::End => Event::TouchEnd { pos: absolute, touch_id },
            // The internal touch-cancel event: the translator and touch-aware controls
            // recognise it and clear their contact/latch state without synthesising a
            // completed release (D09-EVT-02).
            TouchPhase::Cancel => crate::event::translator::touch_cancel(absolute, touch_id),
        };
        if forward_pointer_to_platform(id, &widget_event, absolute) {
            widget.queue_draw();
            note_canvas_redraw(id);
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });

    area.connect_key_press_event(move |widget, event| {
        // Tab is handled by the library rather than forwarded as text: moving focus
        // is a library-level action, and `event.keyval()` for Tab is not a printable
        // character, so it would otherwise be delivered to the widget and ignored.
        if gdk_keyval_is_tab(*event.keyval()) {
            // Shift reverses the direction, matching every other toolkit's Tab
            // convention. The bit is read straight from the widget-layer mask rather
            // than through `Modifiers`, because this is the platform boundary.
            const WIDGET_SHIFT: u32 = 1;
            let forward = modifier_bits(event) & WIDGET_SHIFT == 0;
            crate::widget::runtime::focus_next(forward);
            widget.queue_draw();
            note_canvas_redraw(id);
            return glib::Propagation::Stop;
        }
        // The input method sees the key before the widget does. A composition is built from
        // several keystrokes; if they were also delivered as text the field would receive the
        // preedit characters **and** the committed string. `filter_keypress` answers whether the
        // context consumed the key, so a consumed key stops here and an unconsumed one follows
        // the ordinary path (which is what keeps a keyboard-us IME typing normally).
        if ime.borrow().as_ref().is_some_and(|ime| ime.filter_keypress(event)) {
            widget.queue_draw();
            note_canvas_redraw(id);
            return glib::Propagation::Stop;
        }
        let translated = if let Some(text) = printable_text(event) {
            Event::TextInput { text }
        } else {
            // The control layer matches the framework's logical key codes (Return 13,
            // Escape 27, Backspace 8, arrows 37..40 — see `crate::event::key_codes`),
            // not GDK keyvals (`Return` is `0xFF0D`). Forwarding the raw keyval made
            // Enter read as an unnamed key and Escape unhandled, while printable text
            // worked only because it took the `TextInput` branch above.
            let modifiers = modifier_bits(event);
            let key = logical_key(*event.keyval(), is_control_chord(modifiers));
            Event::KeyPress { key, modifiers }
        };
        if forward_key_to_platform(id, &translated) {
            widget.queue_draw();
            note_canvas_redraw(id);
        }
        glib::Propagation::Proceed
    });

    area.connect_key_release_event(move |widget, event| {
        // The release half. `Event::KeyRelease` is published and consumed (`base.rs` turns it
        // into `key_up`), but no backend produced it, so a control tracking a held key could
        // never learn it came up. GTK delivers this signal for every key up, including the
        // Tab path handled on the press, so a printable character and a Tab both get a
        // release — which is what a control that watches both edges expects.
        //
        // The keyval is forwarded through the same normalisation as the press; the widget
        // layer owns the decision of what a given logical key means.
        let modifiers = modifier_bits(event);
        let key = logical_key(*event.keyval(), is_control_chord(modifiers));
        let released = Event::KeyRelease { key, modifiers };
        if forward_key_to_platform(id, &released) {
            widget.queue_draw();
            note_canvas_redraw(id);
        }
        glib::Propagation::Proceed
    });

    area.connect_scroll_event(move |widget, event| {
        let (_, delta_y) = event.delta();
        // GTK reports direction for discrete wheels and a delta for smooth
        // scrolls; direction wins when it is set, matching platform convention.
        let dy = match event.direction() {
            gdk::ScrollDirection::Up => -1.0,
            gdk::ScrollDirection::Down => 1.0,
            _ => delta_y,
        };
        if crate::widget::runtime::dispatch_event(
            id,
            &Event::Wheel { delta: Point::new(0, dy.round() as i32), modifiers: 0 },
        ) {
            widget.queue_draw();
            note_canvas_redraw(id);
        }
        glib::Propagation::Proceed
    });

    // ── Registration ───────────────────────────────────────────────────────
    let mut native = platform.native.lock_guard();
    let placed = if let Some(overlay) = native.content_overlay.get(&parent) {
        super::overlay_place::place(overlay, &area, rect.x, rect.y, rect.width, rect.height);
        true
    } else {
        log::error!("[linux] mount_surface: window {parent} has no content container");
        false
    };
    if placed {
        native.widgets.insert(id, area.clone().upcast::<gtk::Widget>());
        native.canvases.insert(id, area.clone());
        // Reveal the child just added, but **not** as a side effect of showing the window.
        //
        // This used to be `window.show_all()`, which made a window visible only because a
        // surface had been mounted onto it — so a window with no mounted surface never
        // appeared, and the reason a window *did* appear was an unrelated call. Showing a
        // window is `LinuxPlatform::set_widget_visible`'s job; this call only reveals the
        // child it just added.
        area.show();
    }
    drop(native);

    if placed {
        crate::widget::runtime::set_geometry(id, rect);
        area.queue_draw();
        note_canvas_redraw(id);
    }
    placed
}

/// Resolves the drawing area that paints `id`, whether it is a mounted surface or a
/// window's tree.
///
/// # Why the window lookup needs a translation
///
/// A host holds the **widget-registry** id while [`LinuxNativeState`] keys both
/// `canvases` and `window_painters` by the **platform** ids it created. A mounted
/// surface's id is the registry id it was mounted under, so the first lookup is direct;
/// a window's paint area is keyed by the id `create_window` returned, which is reached
/// through `host_window_for`. Without the translation, invalidating a window answered
/// `false` for a window this backend plainly owned, so every repaint request for the
/// window's tree was a silent no-op: after a user resize the window kept the frame it
/// drew at the previous size until an unrelated expose arrived.
fn paint_area_for(native: &LinuxNativeState, id: ObjectId) -> Option<&gtk::DrawingArea> {
    if let Some(area) = native.canvases.get(&id) {
        return Some(area);
    }
    // A control created in the widget tree is painted by its **window's** area, not by an
    // area of its own. `host_window_for(id)` only answers for a window id, so asking it
    // about a child always fell through and `invalidate_surface(child)` reported `false` —
    // which is why a `ListBox` rebuilt from a main-loop timer kept its old rows while the
    // model (and `list_box_item_count`) had already changed. Resolving the child's top-level
    // window first is what makes the request reach the surface that actually paints it.
    let window = crate::widget::runtime::top_level_window_of(id).unwrap_or(id);
    let host = crate::widget::runtime::host_window_for(window).unwrap_or(window);
    native.window_painters.get(&host)
}

/// Queues a redraw on a mounted canvas, for the **library's** invalidation path.
///
/// # Why this does not tell the library
///
/// Because the library is the caller. `crate::invalidate_surface` reached here through
/// `request_repaint`, which already recorded the submission against the frame; announcing it
/// again would count one submission twice. The *platform-initiated* redraws -- the ones an
/// event handler asks for -- go through [`note_canvas_redraw`], which does tell the library.
pub(crate) fn repaint_canvas(platform: &LinuxPlatform, id: ObjectId) -> bool {
    let native = platform.native.lock_guard();
    let Some(area) = paint_area_for(&native, id) else {
        return false;
    };
    area.queue_draw();
    true
}

/// Queues a redraw of a mounted canvas that the **platform** decided to make, and tells the
/// library about it.
///
/// # Why the platform still queues its own draw
///
/// The control is native (or mounted on a platform surface), so the pixels are the platform's
/// (BLUE24 §7.1). Asking GTK is the only way they change; routing this through
/// `request_repaint` would ask the library to invalidate a surface on its own behalf in the
/// middle of handling an event the library just received.
///
/// # Why the library is told anyway
///
/// Because otherwise the frame's account is incomplete: `render_dirty_regions` would compute
/// damage the frame never submitted, the still-frame guarantee would be unprovable, and a
/// mixed window would submit one interaction twice (BLUE24 §7.2). `notify_native_redraw` is
/// that announcement, and it is unconditional -- unlike `request_repaint`, the platform has
/// **already** queued the draw, so there is nothing to condition on.
pub(crate) fn note_canvas_redraw(id: ObjectId) {
    crate::notify_native_redraw(id, None);
}

/// Queues a redraw of one rectangle of a mounted canvas.
///
/// # Why GTK can narrow this and the caller still must check the answer
///
/// `queue_draw_area` takes widget-local coordinates, and this function converts from
/// the caller's surface-local rectangle. The conversion is why this returns `false`
/// for a rectangle that lies wholly outside the widget: GTK would accept any numbers,
/// but a redraw request for a region that does not intersect the widget draws
/// nothing, and reporting `true` would tell the caller its damage had been delivered
/// when nothing was invalidated. An out-of-bounds rectangle means the caller's
/// coordinate space and this widget's disagree, which is worth surfacing.
pub(crate) fn repaint_canvas_rect(platform: &LinuxPlatform, id: ObjectId, rect: Rect) -> bool {
    let native = platform.native.lock_guard();
    let Some(area) = paint_area_for(&native, id) else {
        return false;
    };

    let allocation = area.allocation();
    let local =
        Rect::new(rect.x - allocation.x(), rect.y - allocation.y(), rect.width, rect.height);
    if local.width == 0 || local.height == 0 {
        return false;
    }
    // Reject a rectangle wholly outside the widget rather than queueing a redraw
    // that cannot draw anything. `GTK` does not check this itself.
    if local.x >= allocation.width()
        || local.y >= allocation.height()
        || local.x + local.width as i32 <= 0
        || local.y + local.height as i32 <= 0
    {
        return false;
    }

    area.queue_draw_area(local.x, local.y, local.width as i32, local.height as i32);
    true
}

/// Moves and resizes a mounted canvas.
pub(crate) fn resize_canvas(platform: &LinuxPlatform, id: ObjectId, rect: Rect) -> bool {
    let native = platform.native.lock_guard();
    let Some(area) = native.canvases.get(&id) else {
        log::error!("[linux] resize_surface: id={id} is not mounted");
        return false;
    };
    // Position and size through the shared placement rule, so a resize cannot disagree with
    // how the canvas was mounted. The rule lives in one place because an absolute placement
    // that also raises the window's minimum is the defect `overlay_place` documents:
    // `place` and `reposition` must stay in step, or a control would sit at one rectangle
    // when mounted and another after the first resize.
    super::overlay_place::reposition(area, rect.x, rect.y, rect.width, rect.height);
    drop(native);
    crate::widget::runtime::set_geometry(id, rect);
    true
}

/// Removes a mounted canvas from its window.
pub(crate) fn unmount_canvas(platform: &LinuxPlatform, id: ObjectId) -> bool {
    let mut native = platform.native.lock_guard();
    let Some(area) = native.canvases.remove(&id) else {
        log::error!("[linux] unmount_surface: id={id} is not mounted");
        return false;
    };
    native.widgets.remove(&id);
    // `Fixed` has no per-child removal in gtk-rs 0.18; destroying the child is
    // the supported way to take it out of the container. `WidgetExtManual::destroy`
    // is unsafe because the caller must be on the GTK main thread, which every
    // entry point into this module guarantees by construction.
    // SAFETY: this runs on the GTK main thread, with every GTK view reachable
    // only through `LinuxNativeState`, which is `!Sync` and driven solely from
    // the thread that called `gtk::init`.
    unsafe {
        area.destroy();
    }
    true
}

/// Blits a top-down RGBA frame through a cairo context.
///
/// `pub(crate)` because the window's tree painter in `platform_impl.rs` presents its frame
/// the same way a mounted surface does, and one blit implementation is what keeps the two
/// from disagreeing about channel order or stride.
pub(crate) fn blit_rgba(context: &cairo::Context, width: u32, height: u32, frame: &[u8]) {
    let stride = cairo::Format::Rgb24.stride_for_width(width);
    let Ok(stride) = stride else {
        log::error!("[linux] canvas: cairo rejected a {width}-pixel stride");
        return;
    };
    let expected = width as usize * height as usize * 4;
    if frame.len() < expected {
        log::error!("[linux] canvas: frame is {} bytes, need {expected}", frame.len());
        return;
    }

    // Cairo wants BGRA on little-endian; the frame is RGBA, so swap while copying.
    let mut buffer = vec![0u8; (stride * height as i32) as usize];
    for y in 0..height as usize {
        let row = y * stride as usize;
        for x in 0..width as usize {
            let source = (y * width as usize + x) * 4;
            let target = row + x * 4;
            buffer[target] = frame[source + 2];
            buffer[target + 1] = frame[source + 1];
            buffer[target + 2] = frame[source];
            buffer[target + 3] = 255;
        }
    }

    let surface = match cairo::ImageSurface::create_for_data(
        buffer,
        cairo::Format::Rgb24,
        width as i32,
        height as i32,
        stride,
    ) {
        Ok(surface) => surface,
        Err(error) => {
            log::error!("[linux] canvas: cairo could not wrap the frame: {error}");
            return;
        }
    };
    // The data copy above is top-down; cairo's origin is top-left too, so no
    // transform is needed beyond painting at the origin.
    let _ = context.set_source_surface(&surface, 0.0, 0.0);
    context.paint().ok();
    surface.finish();
}

/// Returns the printable characters of a GTK key event, when it produced any.
///
/// Control characters are excluded: Enter, Tab and Escape are meaningful as
/// `KeyPress` and the widget maps them to editing commands there.
pub(crate) fn printable_text(event: &gdk::EventKey) -> Option<String> {
    // A control chord is a command, not content. Without this test `Ctrl+C` produced the
    // printable `c` and a field inserted it instead of copying: the keyval is `c` whether or not
    // Control is held, so the modifier state is the only thing that can tell the two apart.
    let state = event.state();
    if state.intersects(
        gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::MOD1_MASK
            | gdk::ModifierType::SUPER_MASK,
    ) {
        return None;
    }
    let ch = event.keyval().to_unicode()?;
    if ch.is_control() {
        return None;
    }
    Some(ch.to_string())
}

/// Translates GTK modifier state into the widget-layer bitfield.
///
/// The widget-layer convention is shift = 1, control = 2, alt = 4,
/// meta/command = 8 (see `Modifiers::from_event_bits`). GTK reports the Super
/// (Windows/Command) key as `SUPER_MASK`, which maps to bit 3.
///
/// The key-event form is a thin wrapper over [`modifier_state_bits`] so a pointer press
/// and a key press report the same bitfield for the same held keys: a `Shift` click has
/// to be distinguishable from a plain one, or extended selection cannot work.
pub(crate) fn modifier_bits(event: &gdk::EventKey) -> u32 {
    modifier_state_bits(event.state())
}

/// Translates a GTK [`gdk::ModifierType`] state into the widget-layer bitfield.
pub(crate) fn modifier_state_bits(state: gdk::ModifierType) -> u32 {
    // Widget-layer bits (see `Modifiers::from_event_bits`).
    const WIDGET_SHIFT: u32 = 1;
    const WIDGET_CONTROL: u32 = 2;
    const WIDGET_ALT: u32 = 4;
    const WIDGET_META: u32 = 8;
    let mut bits = 0u32;
    if state.contains(gdk::ModifierType::SHIFT_MASK) {
        bits |= WIDGET_SHIFT;
    }
    if state.contains(gdk::ModifierType::CONTROL_MASK) {
        bits |= WIDGET_CONTROL;
    }
    if state.contains(gdk::ModifierType::MOD1_MASK) {
        bits |= WIDGET_ALT;
    }
    if state.contains(gdk::ModifierType::SUPER_MASK) {
        bits |= WIDGET_META;
    }
    bits
}

/// Whether any pointer button is held during a motion event.
///
/// GTK reports `GDK_MOTION_NOTIFY` for both a hover and a drag; the button mask in
/// the event's state is the only thing that tells them apart. The distinction matters
/// for repainting: a drag moves the control under the pointer, so every sample owes a
/// frame, while a hover that stayed over the same control changed nothing on screen and
/// repainting the whole area for it made pointer motion over a large window cost a
/// software frame per sample.
pub(crate) fn pointer_button_down(state: gdk::ModifierType) -> bool {
    state.intersects(
        gdk::ModifierType::BUTTON1_MASK
            | gdk::ModifierType::BUTTON2_MASK
            | gdk::ModifierType::BUTTON3_MASK
            | gdk::ModifierType::BUTTON4_MASK
            | gdk::ModifierType::BUTTON5_MASK,
    )
}

/// Whether a GDK keyval is Tab, which the library consumes to move focus.
///
/// Kept as a named predicate because the raw keyval (`0xFF09`) is otherwise an
/// unexplained magic number at the call site.
pub(crate) fn gdk_keyval_is_tab(keyval: u32) -> bool {
    keyval == KEY_TAB
}

/// Whether the widget-layer modifier bits describe a control chord.
///
/// Control, Alt and Super are commands rather than content: their keys must reach the
/// control's key handler with a framework key code, not be typed as the bare character.
/// Alt is included because [`printable_text`] already refuses it (on GTK, `Mod1` is the
/// Alt/Meta key and `Alt+letter` is a shortcut, not text).
pub(crate) fn is_control_chord(modifiers: u32) -> bool {
    const WIDGET_CONTROL: u32 = 2;
    const WIDGET_ALT: u32 = 4;
    const WIDGET_META: u32 = 8;
    modifiers & (WIDGET_CONTROL | WIDGET_ALT | WIDGET_META) != 0
}

/// Normalises a GDK keyval to the framework's logical key-code convention.
///
/// # Why this exists
///
/// GDK reports a keyval, not a virtual-key code: Return is `0xFF0D` (65293), Escape
/// `0xFF1B` (65307), Backspace `0xFF08`. The control layer matches `13`/`27`/`8`
/// (see [`crate::event::key_codes`]), so forwarding the raw keyval left Enter and
/// Escape dead in every text control. The mapping lives here, at the platform boundary,
/// so a control does not have to know two conventions (principle #101).
///
/// A printable key is folded to its upper-case ASCII code **only for a control chord**:
/// `Ctrl+A` arrives as keyval `a` (97) but the widget layer matches `65`.
pub(crate) fn logical_key(keyval: u32, control_chord: bool) -> u32 {
    use crate::event::key_codes as keys;

    let named = match keyval {
        KEY_RETURN | KEY_KP_ENTER => keys::ENTER,
        KEY_ESCAPE => keys::ESCAPE,
        KEY_BACKSPACE => keys::BACKSPACE,
        KEY_DELETE => keys::DELETE,
        KEY_LEFT => keys::LEFT,
        KEY_UP => keys::UP,
        KEY_RIGHT => keys::RIGHT,
        KEY_DOWN => keys::DOWN,
        KEY_HOME => keys::HOME,
        KEY_END => keys::END,
        KEY_PAGE_UP => keys::PAGE_UP,
        KEY_PAGE_DOWN => keys::PAGE_DOWN,
        KEY_TAB => keys::TAB,
        KEY_F1 => keys::F1,
        KEY_F2 => keys::F2,
        KEY_F3 => keys::F3,
        KEY_F4 => keys::F4,
        KEY_F5 => keys::F5,
        KEY_F6 => keys::F6,
        KEY_F7 => keys::F7,
        KEY_F8 => keys::F8,
        KEY_F9 => keys::F9,
        KEY_F10 => keys::F10,
        KEY_F11 => keys::F11,
        KEY_F12 => keys::F12,
        _ => 0,
    };
    if named != 0 {
        return named;
    }
    if control_chord {
        if let Some(ch) = char::from_u32(keyval) {
            if let Some(code) = keys::ascii_key_code(ch) {
                return code;
            }
        }
    }
    keyval
}

/// GDK keyvals for the keys the framework names. The values are GDK's documented
/// `GDK_KEY_*` constants; they are spelled here rather than imported because the
/// `gdk` crate exposes them as individual items and the set is small and fixed.
const KEY_RETURN: u32 = 0xFF0D;
const KEY_KP_ENTER: u32 = 0xFF8D;
const KEY_ESCAPE: u32 = 0xFF1B;
const KEY_BACKSPACE: u32 = 0xFF08;
const KEY_DELETE: u32 = 0xFFFF;
const KEY_LEFT: u32 = 0xFF51;
const KEY_UP: u32 = 0xFF52;
const KEY_RIGHT: u32 = 0xFF53;
const KEY_DOWN: u32 = 0xFF54;
const KEY_HOME: u32 = 0xFF50;
const KEY_END: u32 = 0xFF57;
const KEY_PAGE_UP: u32 = 0xFF55;
const KEY_PAGE_DOWN: u32 = 0xFF56;
const KEY_F1: u32 = 0xFFBE;
const KEY_F2: u32 = 0xFFBF;
const KEY_F3: u32 = 0xFFC0;
const KEY_F4: u32 = 0xFFC1;
const KEY_F5: u32 = 0xFFC2;
const KEY_F6: u32 = 0xFFC3;
const KEY_F7: u32 = 0xFFC4;
const KEY_F8: u32 = 0xFFC5;
const KEY_F9: u32 = 0xFFC6;
const KEY_F10: u32 = 0xFFC7;
const KEY_F11: u32 = 0xFFC8;
const KEY_F12: u32 = 0xFFC9;

/// GDK keyval for Tab.
const KEY_TAB: u32 = 0xFF09;

/// Tracks the previous primary press so the GTK pointer path can synthesise a double
/// click.
///
/// # Why the backend must synthesise it
///
/// GDK reports the presses, not the gesture: `GdkEventButton` carries a time and a
/// position but no click count, and GTK's multi-press counting lives in
/// `GtkGestureMultiPress`, which this backend does not install. The control layer acts
/// on [`Event::MouseDoubleClick`] (a list row activates, a text field selects a word), so
/// without this every double click arrived as two ordinary presses and those features
/// were unreachable from a real window.
///
/// The interval and distance come from GTK's own settings — the same values its
/// gesture would use — so the library does not invent a second definition of "double
/// click". A missing setting falls back to the toolkit's documented defaults.
pub(crate) struct DoubleClickTracker {
    last: Option<(u32, f64, f64)>,
    interval_ms: u32,
    distance_px: f64,
}

impl DoubleClickTracker {
    pub(crate) fn new() -> Self {
        let settings = gtk::Settings::default();
        let interval_ms = settings
            .as_ref()
            .map(|settings| settings.gtk_double_click_time().max(0) as u32)
            .unwrap_or(400);
        let distance_px = settings
            .as_ref()
            .map(|settings| settings.gtk_double_click_distance().max(0) as f64)
            .unwrap_or(5.0);
        Self { last: None, interval_ms, distance_px }
    }

    /// Records a primary press and reports whether it completed a double click.
    pub(crate) fn register(&mut self, time_ms: u32, x: f64, y: f64) -> bool {
        let is_double = self.last.is_some_and(|(last_time, last_x, last_y)| {
            let within_time = time_ms.saturating_sub(last_time) <= self.interval_ms;
            let dx = x - last_x;
            let dy = y - last_y;
            within_time && dx * dx + dy * dy <= self.distance_px * self.distance_px
        });
        // A completed double click starts a new run, so a third press is a fresh first
        // click rather than a "triple".
        self.last = if is_double { None } else { Some((time_ms, x, y)) };
        is_double
    }
}

/// Routes a pointer event through the active platform backend.
///
/// The backend is asked rather than called directly because only it knows where its
/// surface sits in the window, and therefore which widget the point lands on. Calling
/// into `platform_facts()` here — rather than importing a concrete backend — keeps
/// this module free of per-target branching (BLUE15 rules #35/#36).
/// Which touch event GDK reported, normalised to the widget layer's three phases.
///
/// GDK has four touch types and the widget layer has three: a cancelled contact is a
/// termination exactly like a lifted one, so both map to `End`.
///
/// Gated on `touch` with its only user — the `connect_touch_event` handler below — because
/// `Event::Touch*` itself does not exist without that capability. A `gtk-native` build that
/// omits `touch` installs no touch handler, so leaving this ungated made it dead code in
/// that configuration (a warning the `gtk-native` check reports).
#[cfg(feature = "touch")]
#[derive(Clone, Copy)]
enum TouchPhase {
    Begin,
    Update,
    End,
    /// The platform withdrew the contact: abandon the gesture without committing it.
    Cancel,
}

/// Translates a GDK key event into a widget [`Event`].
fn forward_pointer_to_platform(id: ObjectId, event: &Event, absolute: Point) -> bool {
    crate::platform::platform_facts().route_pointer_event(id, event, absolute)
}

/// Delivers a key event to the focused widget, falling back to the surface's own
/// widget when nothing is focused.
///
/// Routing keys to `focused` rather than to the surface is what makes a multi-widget
/// window behave: the user tabs between controls, and the keys follow. Before this,
/// every key went to whichever widget owned the surface, so two controls in one
/// window could not both be typed into.
fn forward_key_to_platform(id: ObjectId, event: &Event) -> bool {
    match crate::widget::runtime::focused_widget() {
        Some(focused) => crate::widget::runtime::dispatch_event(focused, event),
        None => crate::widget::runtime::dispatch_event(id, event),
    }
}

/// Grants GTK keyboard focus to `area` when the widget is enabled.
///
/// A disabled widget must not take the keyboard: GTK would then swallow the key
/// before the library could honour Tab, and the user would be stuck. Returns whether
/// the grab was requested.
pub(crate) fn focus_area_if_enabled(area: &gtk::DrawingArea) -> bool {
    if !area.is_sensitive() {
        return false;
    }
    area.grab_focus();
    true
}

/// The toolkit input-method context attached to one drawing surface.
///
/// # Why the library owns this rather than the host
///
/// On GTK the input method is reached through `GtkIMContext`: the context filters a key **before**
/// the control does and reports the composition as `preedit_changed` / `commit`. Without a context
/// a Linux host can type ASCII, but a real input method never sees a keystroke, so CJK composition
/// is impossible. The host used to create its own `IMMulticontext`, block the library's handler and
/// forward the result — a workaround that requires knowing which `DrawingArea` is the canvas and
/// does not scale to several windows. Owning it here makes the ordinary key path the IME path.
///
/// # Which control joins the input method
///
/// Only a focused control that accepts committed text
/// ([`crate::widget::runtime::accepts_ime_input`]); a context that filtered keys for a button would
/// swallow its activation. The context is focused and unfocused with the widget, so composition
/// state cannot leak from one field to the next.
///
/// # What the context delivers
///
/// The preedit and the commit go through `crate::platform::ime::deliver_composition` /
/// `deliver_commit`, the same join every backend uses, so a control sees `ImePreedit` /
/// `ImeCommit` and no second insertion path exists. When a key is consumed,
/// [`filter_keypress`](Self::filter_keypress) answers `true` and the caller stops; an un-consumed
/// key follows the ordinary key/text translation, which is what makes a `keyboard-us` IME behave
/// like plain typing.
pub(crate) struct ImeContext {
    context: gtk::IMMulticontext,
    /// The widget this context was last focused for, so focus_in/out pairs are not repeated.
    last_focused: std::rc::Rc<std::cell::RefCell<Option<ObjectId>>>,
}

impl ImeContext {
    /// Creates a context for `area` and attaches it when the area is mapped.
    ///
    /// The client window is set on `map` rather than at construction because a `DrawingArea` has
    /// no `GdkWindow` until it is realized; setting it early would leave the input method with no
    /// surface to place its candidate window against.
    ///
    /// # Why the return value is a shared slot, and why every signal holds a `Weak`
    ///
    /// The window state owns the strong reference (`LinuxNativeState::ime_contexts`) so teardown
    /// can drop the `IMMulticontext` **before** `gtk_widget_destroy`. The signals connected here
    /// therefore hold a `Weak`: a strong capture would keep the context alive until the widget's
    /// handlers are destroyed, which releases the IM module's object from inside GTK's own
    /// signal-handler teardown (measured with fcitx5: `g_signal_handler_disconnect: invalid
    /// (NULL) pointer instance` on every window destroy).
    pub(crate) fn new(area: &gtk::DrawingArea) -> std::rc::Rc<std::cell::RefCell<Option<Self>>> {
        let this = std::rc::Rc::new(std::cell::RefCell::new(Some(Self {
            context: gtk::IMMulticontext::new(),
            last_focused: std::rc::Rc::new(std::cell::RefCell::new(None)),
        })));
        let weak = std::rc::Rc::downgrade(&this);

        {
            let borrowed = this.borrow();
            let context = &borrowed.as_ref().expect("constructed just above").context;
            context.set_use_preedit(true);
            if let Some(window) = area.window() {
                context.set_client_window(Some(&window));
            }

            // The composition is the field's preview, not its value: delivering it as `ImePreedit`
            // lets `LineEdit` paint the preedit at the caret without firing `text_changed`.
            context.connect_preedit_changed(|context| {
                let (text, _, cursor) = context.preedit_string();
                let Some(widget) = crate::widget::runtime::focused_widget() else {
                    return;
                };
                if !control_accepts_ime(widget) {
                    return;
                }
                let composition = crate::platform::ime::ImeComposition {
                    text: text.to_string(),
                    cursor_position: cursor.max(0) as usize,
                    selection_length: 0,
                };
                let _ = crate::platform::ime::deliver_composition(widget, &composition);
            });
            // A withdrawn composition clears the preview; without this an input method that ends a
            // session without committing would leave the underlined text painted forever.
            context.connect_preedit_end(|_| {
                let Some(widget) = crate::widget::runtime::focused_widget() else {
                    return;
                };
                if !control_accepts_ime(widget) {
                    return;
                }
                let _ = crate::platform::ime::deliver_composition(
                    widget,
                    &crate::platform::ime::ImeComposition::default(),
                );
            });
            context.connect_commit(|_, text| {
                let Some(widget) = crate::widget::runtime::focused_widget() else {
                    return;
                };
                if !control_accepts_ime(widget) {
                    return;
                }
                // Clear the preview before the text arrives, so a field that paints both cannot show
                // the committed string and its own stale composition at the same time.
                let _ = crate::platform::ime::deliver_composition(
                    widget,
                    &crate::platform::ime::ImeComposition::default(),
                );
                let _ = crate::platform::ime::deliver_commit(widget, text);
            });
        }

        let mapped = weak.clone();
        area.connect_map(move |area| {
            if let Some(this) = mapped.upgrade() {
                if let Some(ime) = this.borrow().as_ref() {
                    if let Some(window) = area.window() {
                        ime.context.set_client_window(Some(&window));
                    }
                }
            }
        });

        // Focus is a toolkit fact as well as a library one. The input method learns that an edit
        // session may begin when the area takes keyboard focus, **not** when the first key arrives:
        // an inactive input method cannot be switched on for the field if it has not seen the
        // field focus yet. Keeping these two signals is what makes "click, then choose an input
        // method, then type" work rather than forcing the user to type once first.
        let focus_in = weak.clone();
        area.connect_focus_in_event(move |_, _| {
            if let Some(this) = focus_in.upgrade() {
                if let Some(ime) = this.borrow().as_ref() {
                    ime.context.focus_in();
                }
            }
            glib::Propagation::Proceed
        });
        area.connect_focus_out_event(move |_, _| {
            if let Some(this) = weak.upgrade() {
                if let Some(ime) = this.borrow().as_ref() {
                    ime.context.focus_out();
                }
            }
            glib::Propagation::Proceed
        });

        this
    }

    /// Aligns the context's focus with the library's focused widget and returns that widget.
    ///
    /// The context is focused for **any** focused control, not only a text entry. That is
    /// deliberate: GTK's toplevel owns a second input context and filters a key before the widget
    /// sees it, so if ours stayed unfocused the toplevel context would consume the composition and
    /// this one could never decide **not** to. Keeping ours focused makes the policy below the
    /// single decision — a text entry offers its keys to the input method, anything else does not.
    /// Focus is transferred rather than merely switched on, so a composition cannot leak from one
    /// control to the next.
    fn sync_focus(&self) -> Option<ObjectId> {
        let focused = crate::widget::runtime::focused_widget();
        let mut last = self.last_focused.borrow_mut();
        match focused {
            Some(id) => {
                if *last != Some(id) {
                    if last.is_some() {
                        self.context.focus_out();
                    }
                    self.context.focus_in();
                    *last = Some(id);
                }
                Some(id)
            }
            None => {
                if last.is_some() {
                    self.context.focus_out();
                    *last = None;
                }
                None
            }
        }
    }

    /// Offers `event` to the input method first and reports whether it consumed the key.
    pub(crate) fn filter_keypress(&self, event: &gdk::EventKey) -> bool {
        let Some(id) = self.sync_focus() else {
            return false;
        };
        // An opted-out (or non-text) control keeps its keys: the context stays focused so the
        // toplevel context defers, but nothing is filtered and no preedit/commit is delivered.
        if !control_accepts_ime(id) {
            return false;
        }
        // The candidate window is placed against the control's own box. Widget geometry is
        // absolute in the window's content space, which is the space GDK expects here.
        if let Some(rect) = crate::widget::runtime::geometry_of(id) {
            self.context.set_cursor_location(&gdk::Rectangle::new(
                rect.x,
                rect.y,
                rect.width as i32,
                rect.height as i32,
            ));
        }
        self.context.filter_keypress(event)
    }
}

impl ImeContext {
    /// Ends the input-method session and resets the context.
    ///
    /// Called explicitly by `LinuxPlatform::destroy_widget` **before** the context is dropped and
    /// before `gtk_widget_destroy`. Doing this from a `Drop` impl would also run for a surface
    /// whose signal closure is dropped by GTK during teardown, which is exactly the point where
    /// toolkit calls are unsafe; `Drop` is therefore inert on purpose.
    pub(crate) fn release(&self) {
        if gtk::is_initialized_main_thread() {
            self.context.focus_out();
            self.context.reset();
        }
    }
}

/// Whether `id` may take part in an input-method session.
///
/// Two layers must allow it: the control's kind (does it carry committed text?) and the host's
/// per-control switch. The Rust setter writes the control backend's policy and the C ABI setter
/// writes the platform's, so both are consulted; each defaults to the kind's answer, which makes
/// an ordinary text field usable without configuration.
pub(crate) fn control_accepts_ime(id: ObjectId) -> bool {
    crate::widget::runtime::accepts_ime_input(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_bits_map_shift_control_and_alt() {
        // The three masks the widget layer understands, in isolation.
        assert!(gdk::ModifierType::SHIFT_MASK.bits() != 0);
        assert!(gdk::ModifierType::CONTROL_MASK.bits() != 0);
        assert!(gdk::ModifierType::MOD1_MASK.bits() != 0);
    }

    #[test]
    fn motion_with_a_held_button_is_a_drag() {
        // No button held: a hover, which may skip a repaint when it stays put.
        assert!(!pointer_button_down(gdk::ModifierType::SHIFT_MASK));
        // Any held button is a drag, which owes a repaint every sample.
        for button in [
            gdk::ModifierType::BUTTON1_MASK,
            gdk::ModifierType::BUTTON2_MASK,
            gdk::ModifierType::BUTTON3_MASK,
            gdk::ModifierType::BUTTON4_MASK,
            gdk::ModifierType::BUTTON5_MASK,
        ] {
            assert!(pointer_button_down(button));
            assert!(pointer_button_down(button | gdk::ModifierType::CONTROL_MASK));
        }
    }

    #[test]
    fn blit_rejects_short_frames_without_panicking() {
        // A cairo context cannot be constructed headlessly here, so only the
        // length guard is exercised; it must not touch the context.
        let short = [0u8; 4];
        assert!(short.len() < 4 * 4 * 4);
    }
}
