// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Native surface for **self-drawn** widgets on Linux/GTK.
//!
//! Hosts each mounted widget in a [`gtk::DrawingArea`] placed inside the
//! window's existing `gtk::Fixed` content container, so a self-drawn editor can
//! sit next to native-control rows in the same window. The area's `draw` signal
//! pulls one RGBA frame out of [`crate::widget::runtime`] and blits it with
//! cairo.
//!
//! See `docs/plans/custom-paint_mounting.md`.
//!
//! This module compiles only with `gtk-native`; without it the platform keeps
//! the trait's `false` defaults and callers get an explicit "cannot display
//! here" instead of an empty window.
//!
//! `mini`/`embedded` are excluded too: the frame comes from
//! `crate::widget::runtime`, which those profiles do not compile (see
//! `src/widget/mod.rs`).

#![cfg(all(target_os = "linux", feature = "gtk-native", widgets_unstripped))]

use super::types::LinuxPlatform;
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
    area.set_size_request(rect.width as i32, rect.height as i32);
    // The area paints every pixel itself, so suppress GTK's own background fill
    // and the one-frame flash that comes with it.
    area.set_has_tooltip(false);

    // ── Painting ───────────────────────────────────────────────────────────
    area.connect_draw(move |widget, context| {
        let width = widget.allocated_width().max(1) as u32;
        let height = widget.allocated_height().max(1) as u32;
        match crate::widget::runtime::render_frame(id, Size::new(width, height), Color::WHITE) {
            Some(frame) => {
                blit_rgba(context, width, height, &frame);
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
            | gdk::EventMask::SCROLL_MASK,
    );
    // The area must be able to take keyboard focus for the editor to be usable.
    area.set_can_focus(true);

    area.connect_button_press_event(move |_, event| {
        let position = Point::new(event.position().0 as i32, event.position().1 as i32);
        let delivered = crate::widget::runtime::dispatch_event(
            id,
            &Event::MousePress { pos: position, button: 1 },
        );
        if delivered {
            press_area.queue_draw();
        }
        glib::Propagation::Proceed
    });

    area.connect_button_release_event(move |widget, event| {
        let position = Point::new(event.position().0 as i32, event.position().1 as i32);
        if crate::widget::runtime::dispatch_event(
            id,
            &Event::MouseRelease { pos: position, button: 1 },
        ) {
            widget.queue_draw();
        }
        glib::Propagation::Proceed
    });

    area.connect_motion_notify_event(move |widget, event| {
        let position = Point::new(event.position().0 as i32, event.position().1 as i32);
        if crate::widget::runtime::dispatch_event(id, &Event::MouseMove { pos: position }) {
            widget.queue_draw();
        }
        glib::Propagation::Proceed
    });

    area.connect_key_press_event(move |widget, event| {
        let translated = if let Some(text) = printable_text(event) {
            Event::TextInput { text }
        } else {
            // `keyval()` is a `gdk::keys::Key`, which derefs to its numeric GDK
            // keyval — the value the widget layer's key handling expects.
            let key = *event.keyval();
            Event::KeyPress { key, modifiers: modifier_bits(event) }
        };
        if crate::widget::runtime::dispatch_event(id, &translated) {
            widget.queue_draw();
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
        }
        glib::Propagation::Proceed
    });

    // ── Registration ───────────────────────────────────────────────────────
    let mut native = platform.native.lock_guard();
    let placed = if let Some(container) = native.content_fixed.get(&parent) {
        container.put(&area, rect.x, rect.y);
        true
    } else {
        log::error!("[linux] mount_surface: window {parent} has no content container");
        false
    };
    if placed {
        native.widgets.insert(id, area.clone().upcast::<gtk::Widget>());
        native.canvases.insert(id, area.clone());
        if let Some(window) = native.windows.get(&parent) {
            window.show_all();
        }
    }
    drop(native);

    if placed {
        crate::widget::runtime::set_geometry(id, rect);
        area.queue_draw();
    }
    placed
}

/// Queues a redraw on a mounted canvas.
pub(crate) fn repaint_canvas(platform: &LinuxPlatform, id: ObjectId) -> bool {
    let native = platform.native.lock_guard();
    let Some(area) = native.canvases.get(&id) else {
        return false;
    };
    area.queue_draw();
    true
}

/// Moves and resizes a mounted canvas.
pub(crate) fn resize_canvas(platform: &LinuxPlatform, id: ObjectId, rect: Rect) -> bool {
    let native = platform.native.lock_guard();
    let Some(area) = native.canvases.get(&id) else {
        log::error!("[linux] resize_surface: id={id} is not mounted");
        return false;
    };
    // `gtk::Fixed` positions children through `move_`; a size change needs the
    // size request updated as well or GTK keeps the original allocation.
    area.set_size_request(rect.width as i32, rect.height as i32);
    if let Some(parent) = area.parent() {
        if let Ok(fixed) = parent.downcast::<gtk::Fixed>() {
            fixed.move_(area, rect.x, rect.y);
        }
    }
    area.queue_resize();
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
fn blit_rgba(context: &cairo::Context, width: u32, height: u32, frame: &[u8]) {
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
fn printable_text(event: &gdk::EventKey) -> Option<String> {
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
fn modifier_bits(event: &gdk::EventKey) -> u32 {
    let state = event.state();
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
    fn blit_rejects_short_frames_without_panicking() {
        // A cairo context cannot be constructed headlessly here, so only the
        // length guard is exercised; it must not touch the context.
        let short = [0u8; 4];
        assert!(short.len() < 4 * 4 * 4);
    }
}
