use super::*;
use crate::core::{Color, Point, Rect, Size};
use crate::event::Event;
#[cfg(all(feature = "image", not(alloc_frugal)))]
use crate::widget::Image;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ── Helper ─────────────────────────────────────────────────────────
fn make_button() -> Button {
    Button::new("Click".into(), Rect::new(10, 20, 120, 36))
}

#[cfg(all(feature = "image", not(alloc_frugal)))]
fn make_image() -> Image {
    Image::try_from_rgba(vec![0u8; 8 * 8 * 4], 8, 8).expect("the buffer matches its declared size")
}

fn rect() -> Rect {
    Rect::new(10, 20, 120, 36)
}

// ── Icon tint (feedback-20261030 F-4, extended to Button) ──────────

/// The tint is a property of its own and round-trips through the contract.
#[test]
fn an_icon_tint_round_trips_through_the_property_contract() {
    let mut button = make_button();
    assert_eq!(button.icon_tint(), None);
    assert_eq!(button.get("icon_tint").expect("readable"), CapabilityValue::Null);

    let red = Color::rgb(220, 40, 40);
    button.set("icon_tint", CapabilityValue::Color(red)).expect("writable");
    assert_eq!(button.icon_tint(), Some(red));
    assert_eq!(button.get("icon_tint").expect("readable"), CapabilityValue::Color(red));

    button.set("icon_tint", CapabilityValue::Null).expect("clearable");
    assert_eq!(button.icon_tint(), None);
    assert!(
        button.set("icon_tint", CapabilityValue::Bool(true)).is_err(),
        "a non-colour value must be refused rather than silently coerced"
    );
}

/// Counts the pure-red pixels a paint produces, which is where a red tint must show up.
#[test]
fn a_tinted_button_icon_is_painted_in_the_tint_colour() {
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    fn red_pixels(button: &mut Button) -> usize {
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 60), 1.0);
        backend.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut backend);
            button.draw(&mut context);
        }
        backend.end_frame();
        backend
            .frame_rgba()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[0] > 200 && pixel[1] < 60 && pixel[2] < 60)
            .count()
    }

    let mut button = Button::new(String::new(), Rect::new(0, 0, 120, 40));
    button.set_icon_source(Some(IconSource::builtin(crate::widget::IconName::Info)));
    assert_eq!(red_pixels(&mut button), 0, "the default ink is not red");

    button.set_icon_tint(Some(Color::rgb(255, 0, 0)));
    assert!(
        red_pixels(&mut button) > 0,
        "the icon must be painted in the tint colour the control was given"
    );
}

/// A legacy image whose own size differs from the icon slot must be resampled, not skipped.
///
/// This is the defect the tint work exposed: `draw_image` requires a buffer at least as large
/// as its rectangle, so a 1x1 icon drawn into a 16x16 slot painted **nothing** (and a larger
/// one painted its top-left corner). The assertion counts the icon's own colour inside the
/// slot, which was zero before the resample existed.
#[cfg(all(feature = "image", not(alloc_frugal)))]
#[test]
fn a_legacy_button_icon_that_differs_from_the_slot_is_resampled() {
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    let mut button = Button::new(String::new(), Rect::new(0, 0, 120, 40));
    let slot = Rect::new(0, 0, 16, 16);
    let black =
        crate::image::Image::from_raw(crate::image::ImageData::Rgba8(vec![0, 0, 0, 255]), 1, 1);
    button.set_icon(black);
    button.set_icon_size(16);

    let mut backend = SoftwarePaintBackend::new(Size::new(120, 40), 1.0);
    backend.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(&mut backend);
        button.draw(&mut context);
    }
    backend.end_frame();

    let icon_box = button.icon_rect().unwrap_or(slot);
    let rgba = backend.frame_rgba();
    let mut dark_in_slot = 0;
    for y in icon_box.y..icon_box.y + icon_box.height as i32 {
        for x in icon_box.x..icon_box.x + icon_box.width as i32 {
            let index = (y as usize * 120 + x as usize) * 4;
            if rgba[index] < 60 && rgba[index + 1] < 60 && rgba[index + 2] < 60 {
                dark_in_slot += 1;
            }
        }
    }
    assert!(
        dark_in_slot > 0,
        "a 1x1 icon must be resampled into the {icon_box:?} slot instead of painting nothing"
    );
}

/// A tinted image icon is recoloured, not left in its original colours.
#[cfg(all(feature = "image", not(alloc_frugal)))]
#[test]
fn a_tinted_button_image_icon_is_recoloured() {
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    fn green_pixels(button: &mut Button) -> usize {
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 60), 1.0);
        backend.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut backend);
            button.draw(&mut context);
        }
        backend.end_frame();
        backend
            .frame_rgba()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[1] > 200 && pixel[0] < 60 && pixel[2] < 60)
            .count()
    }

    let mut button = Button::new(String::new(), Rect::new(0, 0, 120, 40));
    let white = crate::image::Image::from_raw(
        crate::image::ImageData::Rgba8(vec![255, 255, 255, 255]),
        1,
        1,
    );
    button.set_icon(white);
    assert_eq!(green_pixels(&mut button), 0, "a white icon has no green pixels");

    button.set_icon_tint(Some(Color::rgb(0, 255, 0)));
    assert!(green_pixels(&mut button) > 0, "the image must be recoloured by the tint");

    // The legacy path draws the tinted copy into the same box, so swapping the icon back to a
    // vector source must drop the composed image rather than keep drawing it.
    button.set_icon_source(None);
    assert_eq!(green_pixels(&mut button), 0);
}

// ── Property contract (BLUE15 C-1) ─────────────────────────────────

/// `Button`'s own properties must round-trip through the new contract.
///
/// This is the reference test for every other control's migration: it proves
/// the read path, the write path and the published name list describe the same
/// properties, which the centralised `match kind()` dispatch could not
/// guarantee (a name could be writable and unreadable, or listed and absent).
#[test]
fn widget_properties_round_trip() {
    let mut button = make_button();

    assert_eq!(button.get("text"), Ok(CapabilityValue::String("Click".into())));
    button.set("text", CapabilityValue::String("Go".into())).expect("text is writable");
    assert_eq!(button.get("text"), Ok(CapabilityValue::String("Go".into())));

    assert_eq!(button.get("pressed"), Ok(CapabilityValue::Bool(false)));
    button.set("pressed", CapabilityValue::Bool(true)).expect("pressed is writable");
    assert_eq!(button.get("pressed"), Ok(CapabilityValue::Bool(true)));

    button.set("default", CapabilityValue::Bool(true)).expect("default is writable");
    assert_eq!(button.get("default"), Ok(CapabilityValue::Bool(true)));

    let icon = crate::widget::icon_source::IconSpec::Builtin(crate::widget::IconName::Info);
    button
        .set("icon_source", CapabilityValue::IconSpec(icon.clone()))
        .expect("built-in icon descriptors are applicable without a resolver");
    assert_eq!(button.get("icon_source"), Ok(CapabilityValue::IconSpec(icon)));
    button.set("icon_source", CapabilityValue::Null).expect("null clears the icon");
    assert_eq!(button.get("icon_source"), Ok(CapabilityValue::Null));
    assert_eq!(
        button.set(
            "icon_source",
            CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                id: "project/icons/save".into(),
                color_mode: crate::widget::icon_source::IconColorMode::Monochrome,
            }),
        ),
        Err(CapabilityAccessError::ResourceResolutionRequired)
    );
    pub(crate) static DATA: crate::widget::display_widgets::icon::IconData =
        crate::widget::display_widgets::icon::IconData { name: "runtime", grid: 24, paths: &[] };
    button.set_icon_source(Some(IconSource::static_geometry(&DATA)));
    assert_eq!(button.get("icon_source"), Err(CapabilityAccessError::IconSourceNotPersistable));
}

/// The shared properties must work through `Button` too, not only through the
/// base helpers directly — that forwarding is what a migration can silently
/// break.
#[test]
fn shared_properties_are_forwarded_to_the_base_contract() {
    let mut button = make_button();

    assert_eq!(button.get("enabled"), Ok(CapabilityValue::Bool(true)));
    button.set("enabled", CapabilityValue::Bool(false)).expect("enabled is writable");
    assert!(!button.is_enabled(), "the base state must actually change");

    assert_eq!(button.get("geometry"), Ok(CapabilityValue::String("10,20,120,36".into())));
}

/// Every name `Button` publishes must be readable, and its own names must all
/// be present — a `property_names` that omits a property makes it invisible to
/// schema consumers even though `get` would answer.
#[test]
fn published_names_match_the_contract() {
    let button = make_button();
    let names = button.property_names();

    for required in ["text", "pressed", "default"] {
        assert!(names.contains(&required), "property_names must publish {required:?}");
        assert!(button.get(required).is_ok(), "published {required:?} must be readable");
    }
}

// ── 1. Button creation ─────────────────────────────────────────────
#[cfg(feature = "image")]
#[test]
fn button_creation_text_geometry_defaults_icon() {
    let b = make_button();
    assert_eq!(b.text(), "Click");
    assert_eq!(b.geometry(), rect());
    assert_eq!(b.state(), ButtonState::Normal);
    assert!(!b.is_pressed());
    assert!(!b.is_default());
    assert!(b.icon().is_none());
}

// ── 2. State transitions ───────────────────────────────────────────
#[test]
fn state_transition_normal_pressed_released() {
    let mut b = make_button();
    assert_eq!(b.state(), ButtonState::Normal);

    b.press();
    assert!(b.is_pressed());
    assert_eq!(b.state(), ButtonState::Pressed);

    b.release();
    assert!(!b.is_pressed());
    assert_eq!(b.state(), ButtonState::Normal);
}

#[test]
fn state_transition_idempotent_press_release_noop() {
    let mut b = make_button();
    // Already Normal → release is a no-op
    b.release();
    assert_eq!(b.state(), ButtonState::Normal);
    assert!(!b.is_pressed());

    // Press once
    b.press();
    // Second press is a no-op
    b.press();
    assert!(b.is_pressed());
    assert_eq!(b.state(), ButtonState::Pressed);

    // Release once
    b.release();
    // Second release is a no-op
    b.release();
    assert!(!b.is_pressed());
    assert_eq!(b.state(), ButtonState::Normal);
}

// ── 3. Signal emission ─────────────────────────────────────────────
#[test]
fn signal_press_emits_pressed_and_state_changed() {
    let mut b = make_button();
    let pressed_fired = Arc::new(AtomicBool::new(false));
    let changed_fired = Arc::new(AtomicBool::new(false));
    b.pressed_signal.connect({
        let flag = Arc::clone(&pressed_fired);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });
    b.state_changed.connect({
        let flag = Arc::clone(&changed_fired);
        move |_| {
            flag.store(true, Ordering::SeqCst);
        }
    });

    b.press();

    assert!(pressed_fired.load(Ordering::SeqCst));
    assert!(changed_fired.load(Ordering::SeqCst));
}

#[test]
fn signal_release_emits_released_and_state_changed() {
    let mut b = make_button();
    let released_fired = Arc::new(AtomicBool::new(false));
    let changed_fired = Arc::new(AtomicBool::new(false));
    b.released_signal.connect({
        let flag = Arc::clone(&released_fired);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });
    b.state_changed.connect({
        let flag = Arc::clone(&changed_fired);
        move |_| {
            flag.store(true, Ordering::SeqCst);
        }
    });

    b.press(); // Press first
    released_fired.store(false, Ordering::SeqCst);
    changed_fired.store(false, Ordering::SeqCst);

    b.release(); // Then release

    assert!(released_fired.load(Ordering::SeqCst));
    assert!(changed_fired.load(Ordering::SeqCst));
}

#[test]
fn set_enabled_state_requests_redraw() {
    let mut b = make_button();
    let fired = Arc::new(AtomicBool::new(false));
    b.base.redraw_requested.connect({
        let flag = Arc::clone(&fired);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });

    b.set_enabled_state(false);

    assert!(fired.load(Ordering::SeqCst));
}

#[test]
fn signal_no_emission_on_noop_transition() {
    let mut b = make_button();
    let fired = Arc::new(AtomicBool::new(false));
    b.pressed_signal.connect({
        let flag = Arc::clone(&fired);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });
    b.released_signal.connect({
        let flag = Arc::clone(&fired);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });
    b.state_changed.connect({
        let flag = Arc::clone(&fired);
        move |_| {
            flag.store(true, Ordering::SeqCst);
        }
    });

    // No-op: Normal → Normal
    b.release();
    assert!(!fired.load(Ordering::SeqCst));

    // Press → Pressed
    b.press();
    fired.store(false, Ordering::SeqCst);

    // No-op: Pressed → Pressed
    b.press();
    assert!(!fired.load(Ordering::SeqCst));
}

// ── 4. Disabled state ──────────────────────────────────────────────
#[test]
fn disabled_prevents_transitions() {
    let mut b = make_button();
    b.set_enabled_state(false);
    assert_eq!(b.state(), ButtonState::Disabled);

    // Press is ignored when disabled
    b.press();
    assert_eq!(b.state(), ButtonState::Disabled);
    assert!(!b.is_pressed());

    // Release is ignored when disabled
    b.release();
    assert_eq!(b.state(), ButtonState::Disabled);
}

#[test]
fn re_enable_restores_normal_state() {
    let mut b = make_button();
    b.set_enabled_state(false);
    assert_eq!(b.state(), ButtonState::Disabled);

    b.set_enabled_state(true);
    assert_eq!(b.state(), ButtonState::Normal);
    assert!(!b.is_pressed());
}

// ── 5. Text update and icon ────────────────────────────────────────
#[test]
fn set_text_updates_text() {
    let mut b = make_button();
    assert_eq!(b.text(), "Click");

    b.set_text("OK");
    assert_eq!(b.text(), "OK");
}

#[test]
fn set_empty_text() {
    let mut b = make_button();
    b.set_text("");
    assert_eq!(b.text(), "");
    assert!(b.text().is_empty());
}

#[test]
fn typed_icon_source_owns_layout_and_clears_cleanly() {
    let mut button = make_button();
    assert!(button.icon_rect().is_none());
    let source = IconSource::builtin(crate::widget::IconName::Info);
    button.set_icon_source(Some(source));
    assert_eq!(button.icon_source(), Some(source));
    assert!(button.icon_rect().is_some());
    assert!(
        button.implicit_size().width > Button::new("Click".into(), rect()).implicit_size().width,
        "typed geometry reserves the same content slot as a legacy image"
    );
    #[cfg(feature = "image")]
    assert!(button.icon().is_none(), "typed content is not exposed as a legacy image");

    button.set_icon_source(None);
    assert_eq!(button.icon_source(), None);
    assert!(button.icon_rect().is_none(), "clearing releases the icon layout slot");
}

#[cfg(all(feature = "image", not(alloc_frugal)))]
#[test]
fn shared_icon_pixels_replace_other_sources_and_keep_the_shared_allocation() {
    let handle = crate::widget::SharedIconPixels::from_rgba8(vec![255; 4 * 4 * 4], 4, 4)
        .expect("valid RGBA8 data");
    let clone = handle.clone();
    assert!(handle.shares_allocation_with(&clone));

    let mut button = make_button();
    button.set_shared_icon_pixels(Some(handle.clone()));
    assert!(button.icon().is_none());
    assert!(button.shared_icon_pixels().unwrap().shares_allocation_with(&handle));
    assert!(button.icon_rect().is_some());
    let svg = crate::widget::svg::render_widget_to_svg(&mut button, rect());
    assert!(svg.contains("<image "), "shared in-memory pixels must reach the paint backend");
    let icon_box = button.icon_rect();
    button.set_enabled_state(false);
    assert_eq!(button.icon_rect(), icon_box, "disabled state must not shift the icon slot");

    button.set_icon_source(Some(IconSource::builtin(crate::widget::IconName::Info)));
    assert!(button.shared_icon_pixels().is_none());
    assert!(button.icon_rect().is_some());
    button.set_shared_icon_pixels(None);
    assert!(button.icon_source().is_none());
    assert!(button.icon_rect().is_none());

    button.set_shared_icon_pixels(Some(handle));
    button.set_icon(make_image());
    assert!(button.shared_icon_pixels().is_none());
    assert!(button.icon().is_some(), "the legacy image setter becomes the active source");
}

#[cfg(all(feature = "image", not(alloc_frugal)))]
#[test]
fn shared_pixels_scale_to_many_consumers_and_release_with_the_last_control() {
    let allocation = Arc::new(vec![128; 4]);
    let weak = Arc::downgrade(&allocation);
    let handle = crate::widget::SharedIconPixels::from_shared_rgba8(Arc::clone(&allocation), 1, 1)
        .expect("valid one-pixel RGBA image");
    let mut consumers = Vec::with_capacity(100);
    for _ in 0..100 {
        let mut button = make_button();
        button.set_shared_icon_pixels(Some(handle.clone()));
        assert!(button
            .shared_icon_pixels()
            .expect("shared source")
            .shares_allocation_with(&handle));
        consumers.push(button);
    }

    drop(allocation);
    drop(handle);
    assert!(weak.upgrade().is_some(), "live controls must retain the shared allocation");
    drop(consumers);
    assert!(weak.upgrade().is_none(), "dropping the final control must release the pixels");
}

#[cfg(all(feature = "image", not(alloc_frugal)))]
#[test]
fn set_icon_and_default_icon() {
    let mut b = make_button();
    assert!(b.icon().is_none());

    b.set_icon_source(Some(IconSource::builtin(crate::widget::IconName::Info)));
    let img = make_image();
    b.set_icon(img);
    assert!(b.icon().is_some());
    assert_eq!(b.icon_source(), None, "legacy image setter replaces typed content");
}

// ── 6. Default property ────────────────────────────────────────────
#[test]
fn default_property() {
    let mut b = make_button();
    assert!(!b.is_default());

    b.set_default(true);
    assert!(b.is_default());

    b.set_default(false);
    assert!(!b.is_default());
}

// ── 7. Event handling ──────────────────────────────────────────────
#[test]
fn focus_gained_sets_focused_flag() {
    let mut btn = make_button();
    assert!(!btn.is_focused(), "Button should not be focused by default");
    btn.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
    assert!(btn.is_focused(), "Button should be focused after FocusGained");
}

#[test]
fn focus_lost_clears_focused_flag() {
    let mut btn = make_button();
    btn.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
    assert!(btn.is_focused());
    btn.handle_event(&Event::FocusLost);
    assert!(!btn.is_focused(), "Button should not be focused after FocusLost");
}

#[test]
fn key_press_enter_triggers_click() {
    let mut btn = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    btn.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });
    btn.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
    assert!(!clicked.load(Ordering::SeqCst), "an unfocused button ignores Enter");

    btn.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
    btn.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
    assert!(clicked.load(Ordering::SeqCst), "Enter key should trigger click");
}

#[test]
fn key_press_space_triggers_click() {
    let mut btn = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    btn.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });
    btn.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
    assert!(!clicked.load(Ordering::SeqCst), "an unfocused button ignores Space");

    btn.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
    btn.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
    assert!(clicked.load(Ordering::SeqCst), "Space key should trigger click");
}

#[test]
fn key_press_other_key_does_not_trigger_click() {
    let mut btn = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    btn.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });
    btn.handle_event(&Event::KeyPress { key: 65, modifiers: 0 }); // 'A'
    assert!(!clicked.load(Ordering::SeqCst), "Non-Enter/Space key should NOT trigger click");
}

#[test]
fn mouse_enter_sets_hovered() {
    let mut btn = Button::new("OK".to_string(), Rect::new(0, 0, 80, 30));
    assert!(!btn.is_hovered());
    btn.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
    assert!(btn.is_hovered());
}

#[test]
fn mouse_leave_clears_hovered() {
    let mut btn = Button::new("OK".to_string(), Rect::new(0, 0, 80, 30));
    btn.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
    assert!(btn.is_hovered());
    btn.handle_event(&Event::MouseLeave { pos: Point::new(1, 1) });
    assert!(!btn.is_hovered());
}

#[test]
fn event_mouse_down_presses_button() {
    let mut b = make_button();
    // `mouse_button::PRIMARY`, not a bare `1`: the activation guard reads the named
    // code, and a test that spelled its own number would not notice the guard moving.
    let event = Event::MousePress {
        pos: crate::core::Point::new(15, 25),
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    };
    b.handle_event(&event);
    assert!(b.is_pressed());
    assert_eq!(b.state(), ButtonState::Pressed);
}

#[test]
fn event_mouse_up_releases_and_clicks() {
    let mut b = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    b.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });

    // Must be pressed first
    b.press();
    assert!(b.is_pressed());

    let event = Event::MouseRelease {
        pos: crate::core::Point::new(15, 25),
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    };
    b.handle_event(&event);
    assert!(!b.is_pressed());
    assert_eq!(b.state(), ButtonState::Normal);
    assert!(clicked.load(Ordering::SeqCst));
}

/// D09-POINTER-01: a stylus press/release activates the button exactly like a mouse.
///
/// Before the fix the button only armed on `MousePress`, so a `PointerPress` armed
/// nothing and the `PointerRelease` arm (guarded on the grab) was unreachable. The
/// assertions cover the whole lifecycle: press arms, the release inside clicks, and the
/// released latch is cleared.
#[test]
fn event_pointer_press_and_release_activate_the_button() {
    let mut b = make_button();
    let clicks = Arc::new(core::sync::atomic::AtomicUsize::new(0));
    b.base.clicked.connect({
        let counter = Arc::clone(&clicks);
        move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    let canceled = Arc::new(AtomicBool::new(false));
    b.canceled.connect({
        let flag = Arc::clone(&canceled);
        move || flag.store(true, Ordering::SeqCst)
    });

    b.handle_event(&Event::pointer_press(
        Point::new(15, 25),
        crate::event::mouse_button::PRIMARY,
        0.8,
        0.0,
        0.0,
    ));
    assert!(b.is_pressed(), "a stylus press must arm the button like a mouse press");
    assert_eq!(b.state(), ButtonState::Pressed);

    b.handle_event(&Event::pointer_release(
        Point::new(15, 25),
        crate::event::mouse_button::PRIMARY,
        0.7,
    ));
    assert!(!b.is_pressed(), "the stylus release must clear the pressed latch");
    assert_eq!(b.state(), ButtonState::Normal);
    assert_eq!(clicks.load(Ordering::SeqCst), 1, "a stylus tap inside must click once");
    assert!(!canceled.load(Ordering::SeqCst), "a release inside must not cancel");
}

/// D09-POINTER-01: dragging a stylus off the button and releasing cancels, mirroring the
/// mouse contract — a pen drag-off must not activate.
#[test]
fn a_pointer_release_outside_the_button_cancels() {
    let mut b = make_button();
    let clicks = Arc::new(core::sync::atomic::AtomicUsize::new(0));
    b.base.clicked.connect({
        let counter = Arc::clone(&clicks);
        move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    let canceled = Arc::new(AtomicBool::new(false));
    b.canceled.connect({
        let flag = Arc::clone(&canceled);
        move || flag.store(true, Ordering::SeqCst)
    });

    b.handle_event(&Event::pointer_press(
        Point::new(20, 30),
        crate::event::mouse_button::PRIMARY,
        0.9,
        0.0,
        0.0,
    ));
    // The pen moves off the control before lifting: `pressed` clears, the grab survives.
    b.handle_event(&Event::pointer_move(Point::new(400, 400), 0.9, 0.0, 0.0));
    assert!(!b.is_pressed(), "off the control the painted pressed state clears");

    b.handle_event(&Event::pointer_release(
        Point::new(400, 400),
        crate::event::mouse_button::PRIMARY,
        0.0,
    ));
    assert_eq!(clicks.load(Ordering::SeqCst), 0, "a release outside must not click");
    assert!(canceled.load(Ordering::SeqCst), "a release outside must cancel");
}

// ── The activation contract (P0-6 / P0-7) ────────────────────────────

#[test]
fn a_release_outside_the_button_cancels_instead_of_clicking() {
    // The defect this guards: every release used to activate, so dragging off a
    // button and letting go still fired it.
    let mut b = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    let canceled = Arc::new(AtomicBool::new(false));
    b.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || flag.store(true, Ordering::SeqCst)
    });
    b.canceled.connect({
        let flag = Arc::clone(&canceled);
        move || flag.store(true, Ordering::SeqCst)
    });

    b.handle_event(&Event::MousePress {
        pos: crate::core::Point::new(20, 30),
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    });
    assert!(b.is_pressed());

    b.handle_event(&Event::MouseRelease {
        pos: crate::core::Point::new(900, 900),
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    });
    assert!(!b.is_pressed(), "the press ends either way");
    assert!(canceled.load(Ordering::SeqCst), "a release outside must cancel");
    assert!(!clicked.load(Ordering::SeqCst), "a release outside must not click");
}

#[test]
fn a_release_inside_the_button_clicks_and_does_not_cancel() {
    let mut b = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    let canceled = Arc::new(AtomicBool::new(false));
    b.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || flag.store(true, Ordering::SeqCst)
    });
    b.canceled.connect({
        let flag = Arc::clone(&canceled);
        move || flag.store(true, Ordering::SeqCst)
    });

    b.handle_event(&Event::MousePress {
        pos: crate::core::Point::new(20, 30),
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    });
    b.handle_event(&Event::MouseRelease {
        pos: crate::core::Point::new(30, 40),
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    });
    assert!(clicked.load(Ordering::SeqCst));
    assert!(!canceled.load(Ordering::SeqCst), "a release inside must not cancel");
}

#[test]
fn pressed_is_continuous_as_the_pointer_moves_out_and_back() {
    // `pressed` is not an edge: dragging off clears it, dragging back restores it.
    let mut b = make_button();
    b.handle_event(&Event::MousePress {
        pos: crate::core::Point::new(20, 30),
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    });
    assert!(b.is_pressed());

    b.handle_event(&Event::MouseMove { pos: crate::core::Point::new(900, 900) });
    assert!(!b.is_pressed(), "dragging off must clear pressed");

    b.handle_event(&Event::MouseMove { pos: crate::core::Point::new(25, 35) });
    assert!(b.is_pressed(), "dragging back must restore pressed");
}

#[test]
fn a_secondary_press_does_not_activate() {
    // The secondary button opens a context menu; it must not also fire the action.
    let mut b = make_button();
    b.handle_event(&Event::MousePress {
        pos: crate::core::Point::new(20, 30),
        button: crate::event::mouse_button::SECONDARY,
        modifiers: 0,
    });
    assert!(!b.is_pressed());
}

#[test]
fn losing_focus_mid_press_abandons_the_gesture() {
    let mut b = make_button();
    let canceled = Arc::new(AtomicBool::new(false));
    b.canceled.connect({
        let flag = Arc::clone(&canceled);
        move || flag.store(true, Ordering::SeqCst)
    });

    b.handle_event(&Event::MousePress {
        pos: crate::core::Point::new(20, 30),
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    });
    b.handle_event(&Event::FocusLost);
    assert!(!b.is_pressed());
    assert!(canceled.load(Ordering::SeqCst));
}

// ── Visual focus (P0-5) ────────────────────────────────────────────

#[test]
fn a_pointer_click_focuses_without_drawing_a_focus_ring() {
    let mut b = make_button();
    b.handle_event(&Event::FocusGained { reason: FocusReason::Pointer });
    assert!(b.is_focused(), "the control is the keyboard target");
    assert!(!b.visual_focus(), "but the user is on the pointer, so no ring is drawn");
}

#[test]
fn tab_and_shortcut_focus_draw_the_focus_ring() {
    for reason in [FocusReason::Tab, FocusReason::BackTab, FocusReason::Shortcut] {
        let mut b = make_button();
        b.handle_event(&Event::FocusGained { reason });
        assert!(b.is_focused());
        assert!(b.visual_focus(), "{reason:?} must draw the ring");
    }
}

/// The Material metrics profile keeps the inset stroke exactly as it was.
///
/// Focus-ring styling is now selected by the theme's metrics profile, so this is the
/// regression guard that the default profile did not acquire the macOS halo. The inner
/// ring is a 2 px stroke and no box-blur filter is emitted.
#[cfg(device_profile)]
#[test]
fn a_material_focused_button_keeps_the_inset_ring() {
    let _theme_guard = crate::style::theme_test_guard();
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    let mut b = make_button();
    b.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
    let svg = crate::widget::svg::render_to_svg(&mut b);
    assert!(
        svg.contains(&format!("stroke-width=\"{FOCUS_RING_WIDTH}\"")),
        "the Material profile must keep the inset ring stroke: {svg}"
    );
    assert!(
        !svg.contains("shadow_blur_"),
        "the Material profile must not acquire the macOS halo: {svg}"
    );
}

/// A theme on the macOS metrics profile draws the focus ring outside the control.
///
/// The macOS focus indicator is a blurred accent glow around the control, not an inset
/// stroke. The halo goes through the renderer's existing box-blur command, so the marker
/// to assert is the emitted blur filter (and the absence of the 2 px inset stroke).
#[cfg(device_profile)]
#[test]
fn a_macos_focused_button_draws_an_outer_halo() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut theme = crate::theme::Theme::default();
    theme.name = "p2-1-macos".to_string();
    theme.metrics_profile = ControlMetricsProfile::Macos;
    {
        let mut manager = crate::theme::global_theme_manager();
        manager.register_theme(theme);
        assert!(manager.set_theme("p2-1-macos"));
    }

    let mut b = make_button();
    b.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
    let svg = crate::widget::svg::render_to_svg(&mut b);
    assert!(
        svg.contains(&format!("shadow_blur_{FOCUS_HALO_BLUR_RADIUS}")),
        "the macOS profile must draw the halo through the box-blur primitive: {svg}"
    );
    assert!(
        !svg.lines().any(|line| line.contains("<rect") && line.contains("stroke-width=\"2\"")),
        "the halo replaces the inset stroke rather than joining it: {svg}"
    );

    assert!(crate::theme::global_theme_manager().set_theme("default"));
}

/// The Material metrics profile is the existing constant table, applied to the control.
///
/// This is the regression guard for the default appearance: the button's hint is the old
/// `64x40` floor and the painted face keeps the old 4 px radius.
#[cfg(device_profile)]
#[test]
fn the_material_profile_keeps_the_button_constants() {
    let _theme_guard = crate::style::theme_test_guard();
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    let mut b = Button::new("Go".into(), Rect::new(0, 0, 200, 40));
    crate::theme::apply_active_theme(&mut b);
    let hint = b.implicit_size();
    let svg = crate::widget::svg::render_to_svg(&mut b);

    assert_eq!(hint.width, dimensions::BUTTON_MIN.width, "the old width floor: {hint:?}");
    assert_eq!(hint.height, dimensions::BUTTON_MIN.height, "the old height floor: {hint:?}");
    assert!(
        svg.contains("rx=\"4\""),
        "Material keeps the old 4 px radius rather than acquiring a profile shape: {svg}"
    );
}

/// A Macos-profile theme draws the shorter, rounder macOS push button.
///
/// The floor is `64x28`, and the corner radius follows the profile's shape token rather
/// than the theme's generic border radius. The height assertion is the load-bearing one:
/// the profile's floor must also own the vertical air, or Material's 8 px padding would
/// push the content-plus-padding term past 28 and the profile would be invisible.
#[cfg(device_profile)]
#[test]
fn a_macos_profile_theme_shortens_and_rounds_the_button() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut theme = crate::theme::Theme::default();
    theme.name = "p2-6-button-macos".to_string();
    theme.metrics_profile = ControlMetricsProfile::Macos;
    {
        let mut manager = crate::theme::global_theme_manager();
        manager.register_theme(theme);
        assert!(manager.set_theme("p2-6-button-macos"));
    }

    let mut b = make_button();
    crate::theme::apply_active_theme(&mut b);
    let hint = b.implicit_size();
    let svg = crate::widget::svg::render_to_svg(&mut b);
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    assert_eq!(hint.height, 28, "macOS's push button is 28 px tall: {hint:?}");
    assert_eq!(
        hint.width,
        dimensions::BUTTON_MIN.width,
        "the profile keeps Material's width floor: {hint:?}"
    );
    assert!(svg.contains("rx=\"6\""), "the profile's 6 px radius reaches the painted face: {svg}");
}

/// An explicit Material radius survives a profile that would otherwise round it further.
///
/// This is the R-23 regression: the draw path treated the Material baseline value as the
/// "not set by the caller" sentinel, so asking for Material's 4 px on a macOS profile was
/// silently rewritten to the profile's 6 px. Authorship is now recorded by the setter, not
/// inferred from the value.
#[cfg(device_profile)]
#[test]
fn an_explicit_material_radius_is_not_rewritten_by_the_profile() {
    let _theme_guard = crate::style::theme_test_guard();
    let baseline = profile_metrics(ControlMetricsProfile::Material).button_radius;
    assert_ne!(baseline, 6, "the fixture needs the baseline and the macOS radius to differ");

    let mut theme = crate::theme::Theme::default();
    theme.name = "r23-button-explicit-radius".to_string();
    theme.metrics_profile = ControlMetricsProfile::Macos;
    {
        let mut manager = crate::theme::global_theme_manager();
        manager.register_theme(theme);
        assert!(manager.set_theme("r23-button-explicit-radius"));
    }

    let mut b = make_button();
    crate::theme::apply_active_theme(&mut b);
    b.set_border_radius(baseline);
    let svg = crate::widget::svg::render_to_svg(&mut b);
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    assert!(
        svg.contains(&format!("rx=\"{baseline}\"")),
        "the caller's explicit radius must reach the painted face: {svg}"
    );
    assert!(
        !svg.contains("rx=\"6\""),
        "the profile's radius must not replace the explicit one: {svg}"
    );
}

/// The Cupertino profile keeps the Material button row unchanged.
#[cfg(device_profile)]
#[test]
fn the_cupertino_profile_keeps_the_button_constants() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut theme = crate::theme::Theme::default();
    theme.name = "p2-6-button-cupertino".to_string();
    theme.metrics_profile = ControlMetricsProfile::Cupertino;
    {
        let mut manager = crate::theme::global_theme_manager();
        manager.register_theme(theme);
        assert!(manager.set_theme("p2-6-button-cupertino"));
    }

    let mut b = Button::new("Go".into(), Rect::new(0, 0, 200, 40));
    crate::theme::apply_active_theme(&mut b);
    let hint = b.implicit_size();
    let svg = crate::widget::svg::render_to_svg(&mut b);
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    assert_eq!(hint.width, dimensions::BUTTON_MIN.width, "Cupertino is the Material row");
    assert_eq!(hint.height, dimensions::BUTTON_MIN.height);
    assert!(svg.contains("rx=\"4\""), "and the Material radius: {svg}");
}

/// A **default** button is drawn differently, and a normal one is untouched.
///
/// # The defect this pins
///
/// `default_button` was stored, published in the schema and toggled by the constructor while
/// `draw` never read it, so `set_default(true)` redrew the same square. On desktop the default
/// button is the one Enter activates, and every convention says so on screen -- without it the
/// user cannot tell which button the key will press. The sibling `focus_ring` tests could not
/// cover this: a default button need not be focused, and a focused one need not be default.
///
/// Both halves are asserted, and the second is the load-bearing one: a document that merely
/// *differs* is satisfied by any extra ink, so the emphasis is pinned to a **heavier stroke on
/// the ring's own inset** -- the same rectangle the focus ring uses, which is what keeps the two
/// from overlapping when a default button is also focused.
#[test]
fn a_default_button_is_drawn_with_an_emphasis_ring() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut plain = make_button();
    let plain_svg = crate::widget::svg::render_to_svg(&mut plain);

    let mut default = make_button();
    assert!(!default.is_default(), "a button is not the default unless it says so");
    default.set_default(true);
    assert!(default.is_default(), "and it reports what was set");
    let default_svg = crate::widget::svg::render_to_svg(&mut default);

    assert_ne!(
        plain_svg, default_svg,
        "a default button must not paint the same as an ordinary one"
    );
    assert!(
        default_svg.contains(&format!("stroke-width=\"{DEFAULT_EMPHASIS_WIDTH}\"")),
        "the emphasis is a stroke of its own width, not a shifted border: {default_svg}"
    );
    assert!(
        !plain_svg.contains(&format!("stroke-width=\"{DEFAULT_EMPHASIS_WIDTH}\"")),
        "an ordinary button has no such stroke: {plain_svg}"
    );
}

/// A declared `background_gradient` must actually be painted.
///
/// Pins the defect: the field was settable, mergeable and CSS-expressible, but no painter read
/// it, so a button with a gradient drew a solid fill. This asserts the rendered SVG grows a
/// gradient definition that the solid button does not have.
#[test]
fn a_declared_background_gradient_is_painted() {
    use crate::style::Gradient;
    let _theme_guard = crate::style::theme_test_guard();

    let mut plain = make_button();
    let plain_svg = crate::widget::svg::render_to_svg(&mut plain);

    let mut gradient_button = make_button();
    let gradient = Gradient::linear(Point::new(0, 0), Point::new(0, 40))
        .add_stop(0.0, Color::rgb(255, 0, 0))
        .add_stop(1.0, Color::rgb(0, 0, 255));
    gradient_button.set_style(gradient_button.style().clone().with_gradient(gradient));
    let gradient_svg = crate::widget::svg::render_to_svg(&mut gradient_button);

    assert_ne!(
        plain_svg, gradient_svg,
        "a gradient must change what is painted, not be silently dropped"
    );
    assert!(
        gradient_svg.contains("linearGradient"),
        "the gradient must reach the SVG as a paint server: {gradient_svg}"
    );
    assert!(
        !plain_svg.contains("linearGradient"),
        "a button with no gradient must not paint one: {plain_svg}"
    );
}

#[test]
fn a_keyboard_activation_keeps_the_ring() {
    let mut b = make_button();
    let clicks = Arc::new(core::sync::atomic::AtomicUsize::new(0));
    b.base.clicked.connect({
        let counter = Arc::clone(&clicks);
        move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    b.handle_event(&Event::FocusGained { reason: FocusReason::Shortcut });
    b.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
    assert_eq!(clicks.load(Ordering::SeqCst), 1);
    assert!(b.visual_focus());
}

#[test]
fn a_disabled_button_never_arms_and_never_clicks() {
    let mut b = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    b.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || flag.store(true, Ordering::SeqCst)
    });
    b.set_enabled_state(false);
    b.handle_event(&Event::MousePress {
        pos: crate::core::Point::new(20, 30),
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    });
    assert!(!b.is_pressed());
    b.handle_event(&Event::MouseRelease {
        pos: crate::core::Point::new(20, 30),
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    });
    assert!(!clicked.load(Ordering::SeqCst));
}

#[cfg(feature = "touch")]
#[test]
fn event_touch_begin_presses() {
    let mut b = make_button();
    let event = Event::TouchBegin { pos: Point::new(15, 25), touch_id: 0 };
    b.handle_event(&event);
    assert!(b.is_pressed());
    assert_eq!(b.state(), ButtonState::Pressed);
}

#[cfg(feature = "touch")]
#[test]
fn event_touch_end_releases_and_clicks() {
    let mut b = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    b.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });

    b.press();
    let event = Event::TouchEnd { pos: Point::new(15, 25), touch_id: 0 };
    b.handle_event(&event);
    assert!(!b.is_pressed());
    assert_eq!(b.state(), ButtonState::Normal);
    assert!(clicked.load(Ordering::SeqCst));
}

#[cfg(feature = "touch")]
#[test]
fn event_tap_triggers_click() {
    let mut b = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    b.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });

    let event = Event::Tap { pos: Point::new(15, 25) };
    b.handle_event(&event);
    assert!(clicked.load(Ordering::SeqCst));
    // Tap does not change pressed state
    assert!(!b.is_pressed());
}

/// D09-EVT-02: a touch cancel resets the latch without firing `clicked`.
///
/// A platform `TouchCancel` is normalised into a `TouchEnd`, which the button treated as a
/// completed gesture and clicked. The internal cancel event must instead behave like an
/// abandoned gesture: the pressed latch is cleared and `canceled` (not `clicked`) fires.
#[cfg(feature = "touch")]
#[test]
fn event_touch_cancel_resets_without_clicking() {
    let mut b = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    let canceled = Arc::new(AtomicBool::new(false));
    b.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });
    b.canceled.connect({
        let flag = Arc::clone(&canceled);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });

    b.press();
    assert!(b.is_pressed());
    let cancel = crate::event::translator::touch_cancel(Point::new(15, 25), 0);
    b.handle_event(&cancel);

    assert!(!b.is_pressed(), "a cancelled gesture must clear the latch");
    assert!(!b.is_grabbed(), "a cancelled gesture must drop the grab");
    assert!(!clicked.load(Ordering::SeqCst), "a cancel must not click");
    assert!(canceled.load(Ordering::SeqCst), "a cancel reports the abandoned gesture");
}

/// D09-EVT-02: a cancel when nothing is pressed is a harmless no-op (no phantom click).
#[cfg(feature = "touch")]
#[test]
fn event_touch_cancel_without_a_press_is_a_noop() {
    let mut b = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    b.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });
    let cancel = crate::event::translator::touch_cancel(Point::new(15, 25), 0);
    b.handle_event(&cancel);
    assert!(!b.is_pressed());
    assert!(!clicked.load(Ordering::SeqCst));
}

#[test]
fn event_disabled_ignores_mouse_down() {
    let mut b = make_button();
    b.set_enabled_state(false);
    let event = Event::MouseDown((Point::new(15, 25), 0));
    b.handle_event(&event);
    assert!(!b.is_pressed());
    assert_eq!(b.state(), ButtonState::Disabled);
}

#[test]
fn event_disabled_ignores_mouse_up() {
    let mut b = make_button();
    b.set_enabled_state(false);
    // Even if somehow pressed, disabled handle_event should not process MouseUp
    let event = Event::MouseUp((Point::new(15, 25), 0));
    b.handle_event(&event);
    assert!(!b.is_pressed());
}

#[cfg(feature = "touch")]
#[test]
fn event_disabled_ignores_tap() {
    let mut b = make_button();
    b.set_enabled_state(false);
    let clicked = Arc::new(AtomicBool::new(false));
    b.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || {
            flag.store(true, Ordering::SeqCst);
        }
    });
    let event = Event::Tap { pos: Point::new(15, 25) };
    b.handle_event(&event);
    assert!(!clicked.load(Ordering::SeqCst));
}

#[cfg(feature = "touch")]
#[test]
fn event_disabled_ignores_touch_begin() {
    let mut b = make_button();
    b.set_enabled_state(false);
    let event = Event::TouchBegin { pos: Point::new(15, 25), touch_id: 0 };
    b.handle_event(&event);
    assert!(!b.is_pressed());
}

#[cfg(feature = "touch")]
#[test]
fn event_disabled_ignores_touch_end() {
    let mut b = make_button();
    b.set_enabled_state(false);
    let event = Event::TouchEnd { pos: Point::new(15, 25), touch_id: 0 };
    b.handle_event(&event);
    assert!(!b.is_pressed());
}

// ── 8. Widget trait delegation ─────────────────────────────────────
#[test]
fn widget_trait_id_and_kind() {
    let b = make_button();
    assert!(b.id() != 0);
    assert_eq!(b.kind(), WidgetKind::Button);
}

#[test]
fn widget_trait_geometry() {
    let mut b = make_button();
    assert_eq!(b.geometry(), rect());
    let new_rect = Rect::new(0, 0, 200, 50);
    b.set_geometry(new_rect);
    assert_eq!(b.geometry(), new_rect);
}

#[test]
fn widget_trait_visibility() {
    let mut b = make_button();
    assert!(b.is_visible());
    b.hide();
    assert!(!b.is_visible());
    b.show();
    assert!(b.is_visible());
}

#[test]
fn widget_trait_enabled() {
    let mut b = make_button();
    assert!(b.is_enabled());
    b.set_enabled(false);
    assert!(!b.is_enabled());
    assert_eq!(b.state(), ButtonState::Disabled);
    b.set_enabled(true);
    assert!(b.is_enabled());
    assert_eq!(b.state(), ButtonState::Normal);
}

#[test]
fn widget_trait_parent_and_children() {
    let mut b = make_button();
    assert!(b.parent().is_none());
    assert!(b.children().is_empty());

    let child: crate::core::ObjectId = 42;
    let parent: crate::core::ObjectId = 99;

    b.set_parent(Some(parent));
    assert_eq!(b.parent(), Some(parent));

    b.add_child(child);
    assert_eq!(b.children(), &[child]);

    b.remove_child(child);
    assert!(b.children().is_empty());
}

#[test]
fn widget_trait_min_max_size() {
    let mut b = make_button();
    assert!(b.min_size().is_none());
    assert!(b.max_size().is_none());

    b.set_min_size(Some(Size::new(80, 24)));
    assert_eq!(b.min_size(), Some(Size::new(80, 24)));

    b.set_max_size(Some(Size::new(400, 200)));
    assert_eq!(b.max_size(), Some(Size::new(400, 200)));
}

#[test]
fn widget_trait_tooltip() {
    let mut b = make_button();
    assert_eq!(b.tooltip(), "");
    b.set_tooltip("Save".into());
    assert_eq!(b.tooltip(), "Save");
}

#[test]
fn widget_trait_style() {
    let mut b = make_button();
    let default_style = b.style().clone();
    // Verify we can set a modified style via builder pattern.
    let new_style = default_style.clone().with_background(Color::rgb(255, 0, 0));
    b.set_style(new_style.clone());
    assert_eq!(b.style().background_color, new_style.background_color);
}

#[test]
fn widget_trait_signals() {
    let b = make_button();
    // Verify signals are accessible via Widget trait
    let _ = b.hover_signal();
    let _ = b.mouse_down_signal();
    let _ = b.mouse_up_signal();
    let _ = b.key_down_signal();
    let _ = b.key_up_signal();
    let _ = b.focus_gained_signal();
    let _ = b.focus_lost_signal();
    let _ = b.redraw_requested_signal();
    let _ = b.layout_requested_signal();
}

#[test]
fn widget_trait_connection_scope() {
    let b = make_button();
    let _scope = b.connection_scope();
}

// ── 11. Visibility toggling ────────────────────────────────────────
#[test]
fn test_button_visibility_toggle() {
    let mut b = make_button();
    assert!(b.is_visible());

    // Single hide
    b.hide();
    assert!(!b.is_visible());

    // Double hide is idempotent
    b.hide();
    assert!(!b.is_visible());

    // Show restores
    b.show();
    assert!(b.is_visible());

    // Double show is idempotent
    b.show();
    assert!(b.is_visible());

    // Multiple toggle cycles
    for _ in 0..3 {
        b.hide();
        assert!(!b.is_visible());
        b.show();
        assert!(b.is_visible());
    }
}

// ── 12. Animated interaction (BLUE21 P0-4) ─────────────────────────

/// The transition advances toward the pressed state and then **stops**.
///
/// The boolean return is the load-bearing part: a `tick` that kept reporting `true` after the
/// value had settled would keep the host scheduling frames forever. The engine existed and
/// nothing called it, so this is the first control to prove the contract end to end.
#[test]
fn the_interaction_transition_settles_and_reports_so() {
    let mut b = make_button();
    b.set_hovered(true);

    // A hover moves the fill, so at least one tick must report more work to do.
    assert!(b.tick(30), "a hover must start a transition, so the first tick reports another frame");

    // Ticking well past the theme's 200 ms settles it exactly, and then reports completion.
    let mut frames = 1;
    while b.tick(1000) {
        frames += 1;
        assert!(frames < 100, "the transition must terminate rather than tick forever");
    }
    assert_eq!(b.interaction_progress.value(), 0.5, "a hover settles exactly on its target");

    // Settled means settled: **each** further tick reports no work. Asserted in a loop rather
    // than once, because a single `!tick()` would also pass for an implementation that
    // alternated between reporting work and not — the defect this pins is "keeps asking for
    // frames after the value has stopped changing", and only repetition can see it.
    for frame in 0..5 {
        assert!(
            !b.tick(16),
            "a settled transition must report that no frame is needed (tick {frame})"
        );
        assert_eq!(b.interaction_progress.value(), 0.5, "and must not drift while settled");
    }
}

/// A state change that arrives mid-flight re-aims the same progress rather than restarting.
#[test]
fn a_press_during_a_hover_re_aims_rather_than_restarting() {
    let mut b = make_button();
    b.set_hovered(true);
    b.tick(50);
    let partway = b.interaction_progress.value();
    assert!(partway > 0.0 && partway < 0.5, "the hover is in flight: {partway}");

    b.set_pressed(true);
    assert!(b.tick(1), "the press must continue the movement, not settle on the first frame");
    assert!(
        b.interaction_progress.value() > partway,
        "a press must move the progress onward from where the hover left it, not back to zero"
    );
}

/// A disabled control rests, whatever its press flags say.
#[test]
fn a_disabled_button_rests_regardless_of_its_flags() {
    let mut b = make_button();
    b.set_enabled(false);
    assert_eq!(b.interaction_target_progress(), 0.0);
    assert!(!b.tick(1000), "a disabled button has nothing to animate toward, so no frame is owed");
}

/// The transition is visible: a fully-progressed hover paints a different fill.
#[test]
fn the_progress_changes_what_is_painted() {
    // Holds the crate-wide theme guard: this test renders, and a concurrent
    // test that switches the appearance would otherwise change a later frame.
    let _theme_guard = crate::style::theme_test_guard();
    let rect = Rect::new(0, 0, 120, 32);
    let mut resting = Button::new("Go".to_string(), rect);
    let rest_svg = crate::widget::svg::render_to_svg(&mut resting);

    let mut hovered = Button::new("Go".to_string(), rect);
    hovered.set_hovered(true);
    while hovered.tick(1000) {}
    let hover_svg = crate::widget::svg::render_to_svg(&mut hovered);

    assert_ne!(rest_svg, hover_svg, "a completed hover transition must change the rendered fill");
}

/// A push button's label is centred in its own box, not hung off its left padding.
///
/// A button's `contentItem` is centred and Material M3 centres the `TextButton`
/// child, so a label flush to the left edge is wrong however wide the button is — and at
/// the census rectangle the button is 240 px wide while a 14 px "Sample" is 50 px, which
/// is what made `snapshots/svg/button.svg` read as a left-aligned caption.
///
/// The assertion is on the **ink**, not on an element's attribute: text leaves the backend
/// as the `font8x8` rectangles the rasteriser fills (see
/// `SvgPaintBackend::execute_command`), so the document holds a picture of the label rather
/// than a `<text>` element to read. That is the stronger check — the old form asserted the
/// `x` the backend had written down, so a label drawn a pixel off its own reported origin
/// would still have passed.
#[test]
fn the_label_is_centred_in_the_button() {
    let rect = Rect::new(0, 0, 240, 40);
    let mut b = Button::new("Sample".to_string(), rect);
    let svg = crate::widget::svg::render_to_svg(&mut b);
    let (x, _, right, _) = crate::widget::svg::text_ink_box(&svg)
        .unwrap_or_else(|| panic!("the button must paint its label: {svg}"));
    assert!(right > x, "the label laid down ink: {x}..{right}");
    let font = Font::default();
    let mut backend = crate::render::SvgPaintBackend::new(Size::new(240, 40));
    let width = RenderContext::new(&mut backend).measure_text("Sample", &font).width as i32;
    let content = ControlMetrics::content_box(
        rect,
        EdgeOffsets {
            left: dimensions::BUTTON_PADDING_H,
            right: dimensions::BUTTON_PADDING_H,
            top: 0,
            bottom: 0,
        },
    );
    // The centred origin is an upper bound on where the ink can begin: a glyph's first
    // bitmap column is set somewhere inside the 8-column raster, so the ink starts at the
    // origin or to its right and never before it.
    let centred = content.x + (content.width as i32 - width) / 2;
    assert!(
        x >= centred && x < centred + width / 4,
        "the label must sit in the middle of the button's padded box: ink at {x}, centre {centred}"
    );
    assert!(
        x > rect.x + dimensions::BUTTON_PADDING_H as i32,
        "not flush to the padding: ink at {x}"
    );
}

/// A narrow button still honours the minimum padding rather than touching its border.
#[test]
fn a_label_wider_than_the_padding_keeps_the_padding() {
    let rect = Rect::new(0, 0, 90, 40);
    let mut b = Button::new("A very wide label".to_string(), rect);
    let svg = crate::widget::svg::render_to_svg(&mut b);
    let (x, _, right, _) = crate::widget::svg::text_ink_box(&svg)
        .unwrap_or_else(|| panic!("the button must paint its fitted label: {svg}"));
    assert!(right > x, "the label laid down ink: {x}..{right}");
    assert!(
        x >= rect.x + dimensions::BUTTON_PADDING_H as i32 - 1,
        "a fitted label must not start left of the button's own padding, got {x}"
    );
}

#[test]
fn test_button_zero_geometry() {
    // Zero width/height — should not panic
    let _b = Button::new("Zero".into(), Rect::new(0, 0, 0, 0));

    // Negative position — should not panic
    let _b = Button::new("Neg".into(), Rect::new(-10, -20, 100, 30));

    // Zero geometry with negative position — should not panic
    let _b = Button::new("All".into(), Rect::new(-5, -5, 0, 0));
}

#[test]
fn test_button_signal_not_emitted_on_same_value() {
    let mut b = make_button();
    let changed_count = Arc::new(AtomicBool::new(false));
    let c = Arc::clone(&changed_count);
    b.base.changed.connect(move || {
        c.store(true, Ordering::SeqCst);
    });

    // set_text does NOT emit base.changed — it only calls request_redraw()
    b.set_text("Click");
    assert!(
        !changed_count.load(Ordering::SeqCst),
        "set_text with same value should not emit changed"
    );

    // Even setting different text does not emit base.changed
    b.set_text("Different");
    assert!(
        !changed_count.load(Ordering::SeqCst),
        "set_text with different value should not emit changed"
    );
}

// ── §B.7 template: `button_with_icon` ─────────────────────────────

/// A button with an icon is `icon + gap + label` on one line, and its own hint grows when
/// either part grows.
///
/// This is §B.9's mechanical test in its smallest form (§B.7 chose this template first for
/// exactly that reason): the room a control claims must be derived from its children's own
/// sizes, so a part that gets wider pushes the rest along rather than overlapping it.
#[cfg(device_profile)]
#[test]
fn the_implicit_size_is_the_icon_and_the_label_on_one_line() {
    // The assertions are the Material floor, so a concurrent test that activates the
    // Macos profile must not change what this control reports.
    let _theme_guard = crate::style::theme_test_guard();
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    let empty = Button::new(String::new(), Rect::new(0, 0, 200, 40));
    let short = Button::new("Go".into(), Rect::new(0, 0, 200, 40));
    let long = Button::new("Go somewhere much further".into(), Rect::new(0, 0, 200, 40));

    assert!(
        long.implicit_size().width > short.implicit_size().width,
        "a longer label must claim more room"
    );
    // The floor is the load-bearing term: a two-character label must still be pressable.
    assert_eq!(
        short.implicit_size().width,
        dimensions::BUTTON_MIN.width,
        "a short label is below the floor, so the floor is the answer"
    );
    assert_eq!(short.implicit_size().height, dimensions::BUTTON_MIN.height);
    // A label-less button is not a special case: it is content plus padding, floored.
    assert!(empty.implicit_size().width >= dimensions::BUTTON_MIN.width);
}

/// An icon reserves a leading slot, and the label follows it.
///
/// The two boxes are read from one derivation each (`icon_rect` / `label_rect`), and they tile
/// with exactly the shared gap between them. Before this, the icon's advance was a term added
/// to a hand-rolled label estimate and the two were never compared, so a wider icon could not
/// push the label — the drift §B.9 names.
#[test]
fn the_icon_is_leading_and_the_label_follows_it() {
    let plain = Button::new("Save".into(), Rect::new(0, 0, 200, 40));
    // Without an icon the label slot is the whole content box, and there is no icon box.
    assert!(plain.icon_rect().is_none(), "a button with no icon has no icon box");
    let box_ = ControlMetrics::content_box(
        plain.geometry(),
        EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
    );
    assert_eq!(plain.label_rect(), box_, "the label takes the whole content box alone");

    // The exact arithmetic, stated in the open: the content width is the measured label plus
    // both paddings, and the result is *floored* — so below the floor the floor is the answer
    // and above it the arithmetic is. `"Save"` is four clusters and measures below the floor,
    // which is exactly the case the floor exists for.
    let measured =
        estimate_text_width("Save", &Font::default(), 1.0) + 2 * dimensions::BUTTON_PADDING_H;
    assert!(measured < dimensions::BUTTON_MIN.width, "this label must be below the floor");
    assert_eq!(
        plain.implicit_size().width,
        dimensions::BUTTON_MIN.width,
        "below the floor the floor is the answer, not the raw arithmetic"
    );

    // Above the floor the same arithmetic is what is returned, which is the other half of
    // `implicit_size`'s `max`.
    let long = Button::new("A considerably longer label".into(), Rect::new(0, 0, 200, 40));
    let long_measured =
        estimate_text_width(long.text(), &Font::default(), 1.0) + 2 * dimensions::BUTTON_PADDING_H;
    assert!(long_measured > dimensions::BUTTON_MIN.width);
    assert_eq!(long.implicit_size().width, long_measured);
}

/// B-001: the icon slot is configurable, and the default is still leading.
///
/// The four slots tile the content box without overlap: a stacked icon owns a row and the
/// label owns the other, and a trailing icon owns the far edge. The hint is rebuilt for the
/// chosen slot, so a stacked button is taller than an inline one.
#[test]
fn the_icon_position_selects_the_slot_and_defaults_to_leading() {
    let mut button = Button::new("Save".into(), rect());
    button.set_icon_source(Some(IconSource::builtin(crate::widget::IconName::Info)));
    assert_eq!(button.icon_position(), IconPosition::Leading, "leading is the default");

    let content = ControlMetrics::content_box(
        button.geometry(),
        EdgeOffsets::symmetric(dimensions::BUTTON_PADDING_V, dimensions::BUTTON_PADDING_H),
    );
    let leading_icon = button.icon_rect().expect("the icon has a box");
    let leading_label = button.label_rect();
    assert_eq!(leading_icon.x, content.x, "a leading icon starts at the content edge");
    assert!(leading_label.x > leading_icon.x, "the label follows the leading icon");
    let leading_hint = button.implicit_size();

    // Asking for leading explicitly must reproduce the default picture exactly.
    button.set_icon_position(IconPosition::Leading);
    assert_eq!(button.icon_rect(), Some(leading_icon));
    assert_eq!(button.label_rect(), leading_label);
    assert_eq!(button.implicit_size(), leading_hint);

    // Trailing: the label keeps the leading edge and the icon takes the far one.
    button.set_icon_position(IconPosition::Trailing);
    assert_eq!(button.icon_position(), IconPosition::Trailing);
    let trailing_icon = button.icon_rect().expect("the icon has a box");
    let trailing_label = button.label_rect();
    assert_eq!(trailing_label.x, content.x, "the label keeps the leading edge");
    assert_eq!(
        trailing_icon.x + trailing_icon.width as i32,
        content.x + content.width as i32,
        "the trailing icon ends at the content edge"
    );
    assert!(trailing_label.width < content.width, "the icon's slot is removed from the label");

    // Top and bottom: the icon owns its own row, centred horizontally.
    button.set_icon_position(IconPosition::Top);
    let top_icon = button.icon_rect().expect("the icon has a box");
    let top_label = button.label_rect();
    assert_eq!(top_icon.y, content.y);
    assert_eq!(
        top_label.y,
        (top_icon.y + top_icon.height as i32 + dimensions::BUTTON_ICON_SPACING as i32)
            .min(content.y + content.height as i32),
        "the label follows the icon's row by the shared gap, or starts at the content \
             bottom when the given box is too short for both rows"
    );
    assert_eq!(
        top_icon.x + top_icon.width as i32 / 2,
        content.x + content.width as i32 / 2,
        "a stacked icon is horizontally centred"
    );
    let stacked_hint = button.implicit_size();
    assert!(
        stacked_hint.height > leading_hint.height,
        "a stacked icon needs its own row: {stacked_hint:?} vs {leading_hint:?}"
    );

    button.set_icon_position(IconPosition::Bottom);
    let bottom_icon = button.icon_rect().expect("the icon has a box");
    let bottom_label = button.label_rect();
    assert_eq!(
        bottom_icon.y + bottom_icon.height as i32,
        content.y + content.height as i32,
        "a bottom icon ends at the content edge"
    );
    assert_eq!(bottom_label.y, content.y, "the label moves above the bottom icon");
    assert!(bottom_label.y + bottom_label.height as i32 <= bottom_icon.y);

    // Without an icon every slot is the label-only picture, so the position is inert.
    button.set_icon_source(None);
    let plain = button.label_rect();
    assert_eq!(plain, content);
    assert!(button.icon_rect().is_none());
}

/// The size hint and the drawn box agree about the button's height.
///
/// `draw` derives its painted height from `size_hint`, so this is the property that keeps a
/// wide button from being drawn as a wide pill: the reported height is what it paints.
#[test]
fn the_hint_height_is_the_height_painted() {
    // `implicit_size` reads the active metrics profile, which is theme state; hold the
    // crate-wide guard so a neighbouring appearance switch cannot change the answer.
    let _guard = crate::style::theme_test_guard();
    let b = make_button();
    let hint = b.implicit_size();
    // A census cell is far taller than a button; the drawn box must be the hint, not the cell.
    let tall = Button::new("OK".into(), Rect::new(0, 0, 240, 120));
    assert_eq!(tall.implicit_size().height, hint.height);
    assert!(tall.implicit_size().height < 120, "the hint must not follow the cell height");
}

/// BLUE24 §10A.6 criterion 2: the two styles differ in **geometry**, not only in colour.
///
/// # Why "geometry" and not "a colour changed"
///
/// The whole claim of the surface channel is that a flat theme and a dimensional one are two
/// corners of one parameter space — so the observable difference must be a *shape*: a bevelled
/// face draws two light/shade lines along its boundary, and a flat one draws none. Asserting on
/// a fill colour would pass for any theme change, including one that never reached the bevel.
///
/// The bevel is drawn with `<line>` elements, which is what the assertion counts — the same
/// marker `check_control_feature_visible_in_own_snapshot` uses for `group_box`.
///
/// # Why this is gated on `device_profile`
///
/// It drives the real theme manager, which exists only where a device profile does. On
/// `mini`/`embedded` there is no theme to resolve, so there is nothing for the channel to
/// carry — the subject of the test does not exist there, which is what a gate has to say.
#[cfg(device_profile)]
#[test]
fn flat_and_dimensional_styles_differ_in_geometry() {
    use crate::theme::{global_theme_manager, Theme};

    let _theme_guard = crate::style::theme_test_guard();

    // The global manager is what `apply_active_theme` reads, so the test drives the same
    // path production does: register both presets, draw under one, then the other.
    {
        let mut manager = global_theme_manager();
        manager.register_theme(Theme::default());
        manager.register_theme(Theme::dimensional());
    }

    let draw_under = |name: &str| {
        assert!(global_theme_manager().set_theme(name), "{name} is registered");
        let mut b = make_button();
        crate::theme::apply_theme_to_widget(&mut b);
        crate::widget::svg::render_to_svg(&mut b)
    };

    let flat = draw_under("default");
    let dimensional = draw_under("dimensional");

    let bevel_lines = |svg: &str| svg.matches("<line").count();
    assert_eq!(
        bevel_lines(&flat),
        0,
        "the flat style must draw no bevel at all, or the default snapshots would change"
    );
    assert!(
        bevel_lines(&dimensional) > 0,
        "and the dimensional style must draw the bevel's light/shade pair, or the channel \
             never reached the control"
    );

    // Restore the light default so a later test in this binary starts where it expects.
    assert!(global_theme_manager().set_theme("default"));
}

// ── D09-STYLE-02: the hint measures with the font the paint uses ──

/// A caller-authored `style.font` must move the hint together with the painted label.
///
/// # The defect this pins
///
/// `implicit_size()` measured with `Font::default()` while `draw()` painted with
/// `style.font`, so a custom face or a scaled theme font left the hint at the default size:
/// the drawn label was `draw_text_fitted` into a box that was too small and got squeezed or
/// truncated. Both paths now read `metrics::effective_font`.
#[test]
fn a_custom_font_grows_the_button_hint() {
    let mut baseline = Button::new("A wide label".into(), Rect::new(0, 0, 240, 40));
    let base_hint = baseline.implicit_size();

    let mut big = Button::new("A wide label".into(), Rect::new(0, 0, 240, 40));
    big.set_style(crate::style::WidgetStyle::default().with_font(Font::simple("Test", 30.0)));
    let big_hint = big.implicit_size();

    assert!(
        big_hint.width > base_hint.width,
        "a larger font must widen the hint: {} vs {}",
        big_hint.width,
        base_hint.width
    );
    assert!(
        big_hint.height > base_hint.height,
        "a larger font must raise the hint: {} vs {}",
        big_hint.height,
        base_hint.height
    );
    // The draw path reads the same font, so its ink must be wider too.
    let base_ink = ink_width(&mut baseline);
    let big_ink = ink_width(&mut big);
    assert!(big_ink > base_ink, "the painted label must widen too: {big_ink} vs {base_ink}");
}

/// A text scale above 1.0, resolved through the theme, must widen the hint.
#[cfg(device_profile)]
#[test]
fn a_theme_text_scale_above_one_grows_the_button_hint() {
    use crate::style::environment::{
        install_environment, uninstall_environment, EnvironmentProvider,
    };
    use crate::style::MotionPreference;
    use crate::theme::{global_theme_manager, AppearanceMode, Theme};

    struct Scaled(f32);
    impl EnvironmentProvider for Scaled {
        fn text_scale(&self) -> f32 {
            self.0
        }
        fn motion_preference(&self) -> MotionPreference {
            MotionPreference::NoPreference
        }
    }

    let _guard = crate::style::theme_test_guard();
    let previous = install_environment(Box::new(Scaled(1.0)));
    {
        let mut manager = global_theme_manager();
        manager.register_theme(Theme::default());
        manager.set_appearance(AppearanceMode::Light);
    }
    let mut at_one = Button::new("Scaled label".into(), Rect::new(0, 0, 240, 40));
    crate::theme::apply_active_theme(&mut at_one);
    let hint_one = at_one.implicit_size();

    install_environment(Box::new(Scaled(1.5)));
    let mut scaled = Button::new("Scaled label".into(), Rect::new(0, 0, 240, 40));
    crate::theme::apply_active_theme(&mut scaled);
    let hint_large = scaled.implicit_size();

    let _ = uninstall_environment();
    if let Some(previous) = previous {
        install_environment(previous);
    }

    assert!(
        hint_large.width > hint_one.width,
        "a text scale above 1.0 must widen the button hint: {} vs {}",
        hint_large.width,
        hint_one.width
    );
}

/// The ink width of the label a button paints, read back from an SVG render.
fn ink_width(button: &mut Button) -> i32 {
    let svg = crate::widget::svg::render_to_svg(button);
    crate::widget::svg::text_ink_box(&svg).map(|(x, _, right, _)| right - x).unwrap_or(0)
}

/// A hover frame submits damage bounded by the painted box and the margin.
#[cfg(not(alloc_frugal))]
#[test]
fn a_hover_frame_submits_the_painted_box_with_the_damage_margin() {
    use crate::widget::runtime;

    let id = runtime::register(crate::compat::Box::new(Button::new(
        "Go".to_string(),
        Rect::new(4, 8, 80, 32),
    )))
    .expect("registry");
    assert!(runtime::set_repaint_mode(id, runtime::RepaintMode::Dirty));

    assert!(runtime::dispatch_event(id, &Event::mouse_enter(1, 1)));

    let expected = runtime::painted_box_with_damage_margin(id).expect("painted box");
    assert_eq!(
        runtime::dirty_rects(id),
        vec![expected],
        "a hover must damage the painted box plus the margin"
    );
    runtime::unregister(id);
}

/// `Full` keeps the whole-control path and records no damage rectangle.
#[cfg(not(alloc_frugal))]
#[test]
fn a_hover_frame_in_full_mode_keeps_the_whole_control_repaint() {
    use crate::platform::RecordingInvalidations;
    use crate::widget::runtime;

    let id = runtime::register(crate::compat::Box::new(Button::new(
        "Go".to_string(),
        Rect::new(4, 8, 80, 32),
    )))
    .expect("registry");
    assert_eq!(runtime::repaint_mode(id), runtime::RepaintMode::Full);

    // A recorder that accepts narrowed repaints proves the control did not offer one:
    // the `Full` arm falls back to the whole-control invalidation.
    let log: &'static RecordingInvalidations = Box::leak(Box::new(RecordingInvalidations::new()));
    let dispatched = crate::platform::with_recorded_invalidations(log, || {
        runtime::dispatch_event(id, &Event::mouse_enter(1, 1))
    });
    assert!(dispatched);
    assert_eq!(log.calls(), vec![(id, None)], "Full mode keeps the whole-control repaint");
    assert!(
        runtime::dirty_rects(id).is_empty(),
        "Full mode records no damage, so the control repaints whole"
    );
    runtime::unregister(id);
}

// ── Icon size, shortcut, flat, auto-default (R001, R002, R006) ─────

#[test]
fn a_larger_icon_size_grows_the_hint_and_the_icon_box() {
    let mut button = make_button();
    button.set_icon_source(Some(IconSource::Builtin(crate::widget::IconName::Info)));
    let small = button.implicit_size();
    let icon = button.icon_rect().expect("an icon is present");
    assert_eq!(icon.width, dimensions::BUTTON_ICON_SIZE);

    button.set_icon_size(20);
    assert_eq!(button.icon_size(), 20);
    assert!(button.implicit_size().width > small.width, "the reserved advance follows");
    assert_eq!(button.icon_rect().expect("icon").width, 20);
}

#[test]
fn the_shortcut_property_round_trips_and_parses() {
    let mut button = make_button();
    assert_eq!(button.get("shortcut"), Ok(CapabilityValue::Null));
    button
        .set("shortcut", CapabilityValue::String("Primary+S".into()))
        .expect("a parseable shortcut is writable");
    assert_eq!(
        button.get("shortcut"),
        Ok(CapabilityValue::String("Primary+S".into())),
        "the canonical spelling round-trips"
    );
    assert_eq!(
        button.set("shortcut", CapabilityValue::String("Primary+S+T".into())),
        Err(CapabilityAccessError::TypeMismatch),
        "a malformed chord is refused rather than half-parsed"
    );
    button.set("shortcut", CapabilityValue::Null).expect("null clears it");
    assert!(button.shortcut().is_none());
}

#[test]
fn the_declared_shortcut_activates_the_button() {
    use crate::shortcut::{Key, Shortcut};
    let mut button = make_button();
    button.set_shortcut(Some(Shortcut::primary(Key::S)));
    let clicked = Arc::new(AtomicBool::new(false));
    let flag = clicked.clone();
    button.clicked_signal().connect(move || flag.store(true, Ordering::SeqCst));

    let modifiers = 0b1000;
    button.handle_event(&Event::KeyPress { key: 83, modifiers });
    assert!(clicked.load(Ordering::SeqCst), "the declared chord activates the button");

    // A different chord is a different binding.
    clicked.store(false, Ordering::SeqCst);
    button.handle_event(&Event::KeyPress { key: 83, modifiers: 0 });
    assert!(!clicked.load(Ordering::SeqCst), "the bare key is not the declared chord");
}

/// WC6-05: the icon slot's position is readable and writable through the property layer.
///
/// The three sibling icon rows (`icon_source`/`icon_size`/`icon_tint`) were already published,
/// so the one property that says *where* the icon goes was the only unreachable member of the
/// family — and it is a geometry input the painter and the hit test both read.
#[test]
fn icon_position_is_published_with_the_enum_tokens() {
    let mut button = make_button();
    assert_eq!(
        button.get("icon_position"),
        Ok(CapabilityValue::String("leading".into())),
        "the default is the enum's default"
    );

    for (token, expected) in [
        ("trailing", IconPosition::Trailing),
        ("top", IconPosition::Top),
        ("bottom", IconPosition::Bottom),
        ("leading", IconPosition::Leading),
    ] {
        button.set("icon_position", CapabilityValue::String(token.into())).expect("writable");
        assert_eq!(button.icon_position(), expected);
        assert_eq!(button.get("icon_position"), Ok(CapabilityValue::String(token.into())));
    }

    assert_eq!(
        button.set("icon_position", CapabilityValue::String("sideways".into())),
        Err(CapabilityAccessError::TypeMismatch),
        "an unknown token is refused rather than silently ignored"
    );
}

#[test]
fn flat_and_auto_flags_are_published_and_writable() {
    let mut button = make_button();
    for name in ["flat", "auto_default", "auto_exclusive", "drag_enabled", "accepts_drops"] {
        assert_eq!(button.get(name), Ok(CapabilityValue::Bool(false)), "{name} defaults off");
        button.set(name, CapabilityValue::Bool(true)).expect("writable");
        assert_eq!(button.get(name), Ok(CapabilityValue::Bool(true)));
    }
    assert!(button.is_flat());
}

#[test]
fn auto_default_follows_focus() {
    let mut button = make_button();
    button.set_auto_default(true);
    button.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
    assert!(button.is_default(), "focus makes an auto-default button the default");
    button.handle_event(&Event::FocusLost);
    assert!(!button.is_default(), "losing focus gives the emphasis back");
}

#[test]
fn auto_exclusive_buttons_move_the_default_emphasis_inside_one_parent() {
    let mut first = make_button();
    let mut second = Button::new("Other".into(), Rect::new(10, 60, 120, 36));
    first.set_parent(Some(42));
    second.set_parent(Some(42));
    first.set_auto_exclusive(true);
    second.set_auto_exclusive(true);

    first.click();
    assert!(first.is_default(), "the clicked member carries the emphasis");
    second.click();
    assert!(!first.is_default(), "the group has one emphasised member");
    assert!(second.is_default());
}

#[test]
fn the_elide_property_round_trips() {
    let mut button = make_button();
    assert_eq!(button.get("elide"), Ok(CapabilityValue::String("right".into())));
    button.set("elide", CapabilityValue::String("middle".into())).expect("writable");
    assert_eq!(button.elide_mode(), TextElideMode::ElideMiddle);
    assert_eq!(
        button.set("elide", CapabilityValue::String("sideways".into())),
        Err(CapabilityAccessError::TypeMismatch)
    );
}

// ── Menu attachment (R001, R003) ───────────────────────────────────

#[cfg(full_widgets)]
#[test]
fn an_attached_menu_opens_and_reports_the_activated_row() {
    use crate::widget::menu_toolbar::menu_button::MenuItem;
    use std::sync::Mutex;

    let mut button = Button::new("File".into(), Rect::new(10, 20, 120, 36));
    button.set_menu(vec![MenuItem::new(1, "Open"), MenuItem::new(2, "Save")]);
    assert_eq!(button.menu_items().len(), 2);

    let triggered = Arc::new(Mutex::new(0u64));
    let sink = triggered.clone();
    button.item_triggered.connect(move |id| {
        *sink.lock().unwrap() = *id;
    });

    button.open_menu();
    assert!(button.is_menu_open());
    let panel = button.menu_rect();
    let first_row = crate::widget::menu_toolbar::menu_button::menu_item_rect(panel, 0);
    button.handle_event(&Event::MousePress {
        pos: Point::new(first_row.x + 4, first_row.y + 4),
        button: 1,
        modifiers: 0,
    });
    assert_eq!(*triggered.lock().unwrap(), 1);
    assert!(!button.is_menu_open(), "activating a row closes the menu");
}

#[cfg(full_widgets)]
#[test]
fn the_menu_owns_the_pointer_while_it_is_open() {
    use crate::widget::menu_toolbar::menu_button::MenuItem;

    let mut button = Button::new("File".into(), Rect::new(10, 20, 120, 36));
    button.set_menu(vec![MenuItem::new(1, "Open")]);
    button.open_menu();
    let panel = button.menu_rect();
    assert!(
        button.contains_point(Point::new(panel.x + 2, panel.y + 2)),
        "a press on the popup must route to the button that owns it"
    );

    button.handle_event(&Event::KeyPress { key: key_codes::ESCAPE, modifiers: 0 });
    assert!(!button.is_menu_open(), "Escape dismisses the attached menu");
}

#[cfg(full_widgets)]
#[test]
fn a_context_menu_request_anchors_the_attached_menu_at_the_pointer() {
    use crate::widget::menu_toolbar::menu_button::MenuItem;

    let mut button = Button::new("File".into(), Rect::new(0, 0, 100, 30));
    assert!(
        Widget::context_menu_event(&mut button, Point::new(5, 5)),
        "a button without an attached menu reports the request to the host"
    );
    button.set_menu(vec![MenuItem::new(1, "Open")]);
    assert!(Widget::context_menu_event(&mut button, Point::new(40, 50)));
    assert!(button.is_menu_open());
    assert!(button.menu_rect().contains_point(Point::new(41, 51)));
}

/// A button with no attached menu reports a right-click instead of silently dropping it.
#[test]
fn a_context_menu_request_reaches_the_published_signal() {
    let mut button = Button::new("Tab".into(), Rect::new(0, 0, 100, 30));
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let slot = std::sync::Arc::clone(&seen);
    button.context_menu_requested.connect(move |pos| {
        if let Ok(mut slot) = slot.lock() {
            *slot = Some(*pos);
        }
    });

    assert!(Widget::context_menu_event(&mut button, Point::new(12, 18)));
    assert_eq!(*seen.lock().expect("the test slot locks"), Some(Point::new(12, 18)));
}

// ── Drag and drop (R003) ───────────────────────────────────────────

#[test]
fn a_drag_enabled_button_offers_its_caption() {
    use crate::event::dnd::DragSource;

    let mut button = make_button();
    assert!(DragSource::drag_payload(&button, Point::new(20, 30)).is_none(), "dragging is opt-in");
    button.set_drag_enabled(true);
    let payload = DragSource::drag_payload(&button, Point::new(20, 30))
        .expect("a draggable button carries its caption");
    assert!(payload.is_type("text/plain"));
    assert_eq!(payload.label, "Click");
    assert_eq!(payload.item_id, "Click");
}

#[test]
fn a_drop_accepting_button_takes_the_dropped_caption() {
    use crate::event::dnd::{DragPayload, DropEffect, DropTarget};

    let mut button = make_button();
    let payload = DragPayload::new("text/plain", "ignored").with_label("Dropped");
    assert!(button.drop_target_mut().is_none(), "accepting drops is opt-in");
    button.set_accepts_drops(true);
    assert!(button.drop_target_mut().is_some());
    assert!(DropTarget::can_accept(&button, &payload));
    assert!(!DropTarget::can_accept(&button, &DragPayload::new("image/png", "x")));
    assert_eq!(
        DropTarget::on_drop(&mut button, &payload, Point::new(10, 20)),
        DropEffect::Copy,
        "text is copied, not consumed"
    );
    assert_eq!(button.text(), "Dropped");
}

// ── Delayed press-and-hold popup (B-001) ───────────────────────────

#[cfg(full_widgets)]
#[test]
fn holding_a_menu_button_opens_its_menu_after_the_delay() {
    use crate::widget::menu_toolbar::menu_button::MenuItem;

    let mut button = Button::new("File".into(), Rect::new(0, 0, 100, 30));
    button.set_menu(vec![MenuItem::new(1, "Open")]);
    button.set_popup_delay_ms(200);

    button.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
    assert!(!button.is_menu_open(), "the menu waits for the hold delay");
    assert!(Button::tick(&mut button, 100), "a timing hold owes the animation bus a frame");
    assert!(!button.is_menu_open());
    Button::tick(&mut button, 100);
    assert!(button.is_menu_open(), "the delayed popup opens while the button is held");
}

/// Gated with [`Button::is_menu_open`]: the assertion *is* the popup timer, which only exists
/// in a build that carries the menu machinery.
#[cfg(full_widgets)]
#[test]
fn a_button_without_a_menu_never_arms_the_popup_timer() {
    let mut button = make_button();
    button.press();
    Button::tick(&mut button, 10_000);
    assert!(!button.is_menu_open(), "a button with no menu has nothing to open");
    button.release();
}

// ── Menu keyboard navigation (B-002) ───────────────────────────────

#[cfg(full_widgets)]
#[test]
fn menu_navigation_walks_indices_and_skips_disabled_rows() {
    use crate::widget::menu_toolbar::menu_button::MenuItem;

    let first = MenuItem::new(1, "One");
    let mut skipped = MenuItem::new(2, "Two");
    skipped.enabled = false;
    let last = MenuItem::new(3, "Three");
    let mut button = Button::new("File".into(), Rect::new(0, 0, 100, 30));
    button.set_menu(vec![first, skipped, last]);
    button.open_menu();

    button.move_menu_highlight(true);
    assert_eq!(button.active_menu_index, Some(2), "a disabled row is skipped");
    button.move_menu_highlight(true);
    assert_eq!(button.active_menu_index, Some(0), "the walk wraps");
    button.move_menu_highlight(false);
    assert_eq!(button.active_menu_index, Some(2), "the reverse walk wraps too");
}

// ── Shortcut hint (B-011) ──────────────────────────────────────────

#[test]
fn a_declared_shortcut_is_painted_as_a_bottom_right_hint() {
    use crate::shortcut::{Key, Shortcut};

    let _guard = crate::style::theme_test_guard();
    let mut plain = make_button();
    let plain_svg = crate::widget::svg::render_to_svg(&mut plain);
    let plain_boxes = crate::widget::svg::text_ink_boxes(&plain_svg);
    let (_, _, plain_right, _) = *plain_boxes.first().expect("the caption paints ink");

    let mut hinted = make_button();
    hinted.set_shortcut(Some(Shortcut::ctrl(Key::S)));
    let hinted_svg = crate::widget::svg::render_to_svg(&mut hinted);
    let hinted_boxes = crate::widget::svg::text_ink_boxes(&hinted_svg);
    assert_eq!(hinted_boxes.len(), 2, "the caption and the hint are two text runs");
    let (_, _, hint_right, _) = *hinted_boxes.last().expect("the hint adds ink");

    assert_ne!(plain_svg, hinted_svg, "a declared shortcut must be drawn");
    assert!(
            hint_right > plain_right,
            "the hint sits at the trailing edge, so the ink box grows right ({hint_right} vs {plain_right})"
        );
}

// ── Regression: a disabled mid-press must not keep the painted latch ──

/// Disabling while held abandons the gesture **and** clears the latch behind the enabled gate.
///
/// `set_enabled_state(false)` disabled before cancelling, and `set_pressed` is inert while
/// disabled — so the base press flag and `signaled_pressed` survived. Re-enabling then reported
/// `Pressed` with no pointer down, and the next press emitted no `pressed` because the edge was
/// still latched.
#[test]
fn disabling_mid_press_clears_the_latch_and_announces_the_next_press() {
    let mut b = make_button();
    let pressed = Arc::new(AtomicBool::new(false));
    b.pressed_signal.connect({
        let flag = Arc::clone(&pressed);
        move || flag.store(true, Ordering::SeqCst)
    });

    b.press();
    b.set_enabled_state(false);
    b.set_enabled_state(true);
    assert!(!b.is_pressed(), "the abandoned gesture must not come back painted pressed");
    assert_eq!(b.state(), ButtonState::Normal);

    pressed.store(false, Ordering::SeqCst);
    b.press();
    assert!(pressed.load(Ordering::SeqCst), "the next press must announce its rising edge");
}

/// `command("click")` is the enabled-gated activation, not a press/release pair that bypasses it.
///
/// The arm called `press()`/`release()` — both inert on a disabled button — and then emitted
/// `clicked` unconditionally, so a disabled button reported a click no pointer could produce.
#[test]
fn the_click_command_obeys_the_enabled_gate() {
    let mut b = make_button();
    let clicked = Arc::new(AtomicBool::new(false));
    b.base.clicked.connect({
        let flag = Arc::clone(&clicked);
        move || flag.store(true, Ordering::SeqCst)
    });

    b.set_enabled_state(false);
    b.command("click").expect("the command name is known");
    assert!(!clicked.load(Ordering::SeqCst), "a disabled button cannot be clicked");

    b.set_enabled_state(true);
    b.command("click").expect("the command name is known");
    assert!(clicked.load(Ordering::SeqCst), "an enabled command is an activation");
}

/// The command is the same activation as a click, including the group claim.
#[test]
fn the_click_command_claims_the_auto_exclusive_selection() {
    let mut first = make_button();
    let mut second = Button::new("Other".into(), Rect::new(10, 60, 120, 36));
    first.set_parent(Some(4_242));
    second.set_parent(Some(4_242));
    first.set_auto_exclusive(true);
    second.set_auto_exclusive(true);

    first.click();
    assert!(first.is_default());
    second.command("click").expect("the command name is known");
    assert!(second.is_default(), "a programmatic click moves the emphasis like a real one");
    assert!(!first.is_default());
}

/// A pointer release claims the auto-exclusive emphasis, as `click()` and the keyboard do.
///
/// The release arm emitted `clicked` but never claimed, so a mouse click on a second
/// auto-exclusive button left the emphasis painted on the previous member.
#[test]
fn a_pointer_release_claims_the_auto_exclusive_selection() {
    let mut first = make_button();
    let mut second = Button::new("Other".into(), Rect::new(10, 60, 120, 36));
    first.set_parent(Some(4_243));
    second.set_parent(Some(4_243));
    first.set_auto_exclusive(true);
    second.set_auto_exclusive(true);

    first.click();
    assert!(first.is_default(), "the fixture starts with the first member emphasised");

    let inside = Point::new(20, 70);
    second.handle_event(&Event::MousePress {
        pos: inside,
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    });
    second.handle_event(&Event::MouseRelease {
        pos: inside,
        button: crate::event::mouse_button::PRIMARY,
        modifiers: 0,
    });

    assert!(second.is_default(), "a pointer activation claims the group's emphasis");
    assert!(!first.is_default(), "the previous member must lose it");
}

/// A stacked icon and its label are laid out in the band the face actually paints.
///
/// `draw` clamps the height to `size_hint` and centres it, but the slot derivations read the
/// full geometry: a 200x120 `IconPosition::Top` button painted its face at y=38..82 while the
/// icon floated at y=8..28.
#[test]
fn a_stacked_icon_and_label_stay_inside_the_painted_band() {
    let mut button = Button::new("Go".into(), Rect::new(0, 0, 200, 120));
    button.set_icon_source(Some(IconSource::builtin(crate::widget::IconName::Info)));
    button.set_icon_position(IconPosition::Top);

    let band_height = button.size_hint().height.min(120);
    let band_y = (120 - band_height) as i32 / 2;
    let band_bottom = band_y + band_height as i32;
    let icon = button.icon_rect().expect("the icon has a slot");
    let label = button.label_rect();

    assert!(icon.y >= band_y, "the icon starts inside the painted face: {icon:?}");
    assert!(
        icon.y + icon.height as i32 <= band_bottom,
        "and ends inside it: {icon:?} vs band bottom {band_bottom}"
    );
    assert!(label.y >= icon.y + icon.height as i32, "the label follows the stacked icon");
    assert!(
        label.y + label.height as i32 <= band_bottom,
        "the label stays inside the painted face too: {label:?}"
    );
}

/// The default emphasis reads as the theme accent, not as the face's contrast colour.
///
/// On the default `rgb(240,240,240)` face the contrast colour is black, so the documented
/// "theme accent" ring was less visible than the border it sits next to.
#[cfg(device_profile)]
#[test]
fn the_default_emphasis_ring_uses_the_theme_accent() {
    let _guard = crate::style::theme_test_guard();
    crate::widget::census::install_preset_appearances();
    crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);

    let accent = crate::style::palette_shared().expect("an active palette").accent;
    let button = make_button();
    assert_eq!(
        button.emphasis_ring_color(Color::rgb(240, 240, 240)),
        accent,
        "the emphasis is the theme accent, with the face contrast only as the no-theme fallback"
    );
}
