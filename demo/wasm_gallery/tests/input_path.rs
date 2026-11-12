//! Input-path probe for the gallery: no browser, no window server.
//!
//! "I clicked Next and nothing happened" has exactly two causes — the click never
//! reaches `on_click`, or it arrives and the page rebuild does not. Running the same
//! prologue the browser runs (build a window, dispatch a press/release pair, advance
//! one frame) turns that into an assertion instead of a screenshot guess.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rust_widgets::app::{App, AppConfig, WidgetHandle};
#[cfg(test)]
use rust_widgets::core::{Color, Size};
use rust_widgets::core::{Point, Rect};
use rust_widgets::event::{mouse_button, Event};

fn app() -> App {
    let mut app = App::with_config(
        AppConfig::default().with_app_name("gallery probe").with_organization("rust_widgets"),
    );
    app.init();
    app
}

/// A press and release on the same point, then one frame — what the wasm backend's
/// canvas listeners do for a tap.
fn click(window: rust_widgets::core::ObjectId, x: i32, y: i32) {
    let point = Point::new(x, y);
    rust_widgets::dispatch_pointer_event(
        window,
        &Event::mouse_press(x, y, mouse_button::PRIMARY),
        point,
    );
    rust_widgets::dispatch_pointer_event(
        window,
        &Event::mouse_release(x, y, mouse_button::PRIMARY, 0),
        point,
    );
    rust_widgets::drive_frame(16);
}

/// The generic click channel: a button created by `new_button` must report a click.
#[test]
fn a_window_button_reports_its_click() {
    let app = app();
    let window = app.new_window("probe", 0, 0, 400, 300);
    let button = window.new_button("Probe", 20, 20, 120, 30);

    let clicks = Arc::new(AtomicUsize::new(0));
    {
        let clicks = Arc::clone(&clicks);
        button.on_click(move || {
            clicks.fetch_add(1, Ordering::SeqCst);
        });
    }

    click(window.raw_id(), 80, 35);
    assert_eq!(clicks.load(Ordering::SeqCst), 1, "the button must report one click");
}

/// A **card** — a control mounted the way the gallery mounts one — must be live too.
///
/// # The defect this pins
///
/// The gallery does not use `new_button`; every card goes through
/// `mount_widget_by_name`, which on a record-backed host adopts the widget into the
/// window tree. "Everything on the page is dead — I cannot move anything" is exactly
/// the case where the click reaches the window but not the adopted child, or where the
/// child is hit-tested against the wrong space. The probe clicks a check box in the
/// middle of its own rectangle and asserts the control's **own** state changed, so it
/// fails if input stops at the window.
#[test]
fn a_mounted_card_receives_the_click_and_changes_its_own_state() {
    use rust_widgets::widget::runtime::with_widget;
    use rust_widgets::widget::CheckBox;

    let app = app();
    let window = app.new_window("probe", 0, 0, 400, 300);
    let card = window
        .mount_widget_by_name("check_box", Rect::new(20, 40, 200, 32), "Card")
        .expect("a record-backed host adopts the card");

    let changes = Arc::new(AtomicUsize::new(0));
    {
        let changes = Arc::clone(&changes);
        card.on_click(move || {
            changes.fetch_add(1, Ordering::SeqCst);
        });
    }

    let checked_before = with_widget(card.raw_id(), |widget| {
        (widget as &dyn std::any::Any).downcast_ref::<CheckBox>().is_some_and(CheckBox::is_checked)
    })
    .unwrap_or(false);

    // The check box draws its tick in a square at the leading edge, so the click lands
    // there rather than on the caption: a control that only reacts to its label would
    // still pass a centre-of-caption click by accident.
    click(window.raw_id(), 32, 56);

    let checked_after = with_widget(card.raw_id(), |widget| {
        (widget as &dyn std::any::Any).downcast_ref::<CheckBox>().is_some_and(CheckBox::is_checked)
    })
    .unwrap_or(false);

    assert!(!checked_before, "the sample check box starts unchecked");
    assert!(checked_after, "the click must reach the card and toggle its own state");

    // A button is the family that publishes `clicked`: the host channel is asserted on it
    // because a check box reports its change through `state_changed`/`toggled` instead. Both
    // are mounted the same way, so this also proves the click channel is wired for a card.
    let button = window
        .mount_widget_by_name("button", Rect::new(20, 120, 200, 32), "Card")
        .expect("a record-backed host adopts the card");
    {
        let changes = Arc::clone(&changes);
        button.on_click(move || {
            changes.fetch_add(1, Ordering::SeqCst);
        });
    }
    click(window.raw_id(), 60, 136);
    assert_eq!(changes.load(Ordering::SeqCst), 1, "and the host must see the button click");
}

/// A card mounted away from the frame origin must paint **inside its own rectangle**.
///
/// # The defect this pins
///
/// A `Canvas` sample writes its commands in the control's own space. Written as fixed
/// literals they landed at `(12, 16)` of the window, which the control's own viewport
/// test then discarded: the card drawn on the page was an empty box while the same
/// control in a snapshot (at the frame origin) looked right. The probe mounts the card
/// the way the gallery does and asks for the pixel where the sample's first fill must
/// be, so the two paths cannot drift apart again.
#[test]
fn a_mounted_card_paints_its_sample_content_inside_its_rect() {
    let app = app();
    let window = app.new_window("probe", 0, 0, 300, 600);
    let rect = Rect::new(14, 428, 240, 120);

    // The gallery's own order: build, fill, mount.
    let mut widget = rust_widgets::WidgetFactory::shared()
        .create("canvas", rect, "Canvas")
        .expect("the factory builds a canvas");
    rust_widgets::sample_fill::apply("canvas", widget.as_mut());
    window.mount_surface(widget, rect).expect("a record-backed host adopts the card");

    let frame = rust_widgets::widget::runtime::render_window_frame(
        window.raw_id(),
        Size::new(300, 600),
        Color::WHITE,
    )
    .expect("frame");
    let pixel = |x: usize, y: usize| {
        let index = (y * 300 + x) * 4;
        (frame[index], frame[index + 1], frame[index + 2])
    };
    assert_eq!(
        pixel(40, 460),
        (33, 150, 243),
        "the sample's first fill must land inside the card, not at the window origin"
    );
}
