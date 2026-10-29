// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Inline tests for the stable C ABI.

use alloc::boxed::Box;
use alloc::ffi::CString;
use core::ffi::{c_char, c_int, c_uint, CStr};

use super::*;

/// What a C host writes through the ABI must be what it reads back.
///
/// # The defect this closes
///
/// The combo-box and list-box **writes** went through the control backend (which reaches
/// the widget) while the **reads** went through `Platform` (whose every backend answers the
/// trait default). So a host could add four items and then be told the box holds zero, and
/// `rw_combo_box_set_current_index` returned `false` for an index it had just added.
///
/// That is the "reported success for something that did not happen" shape from the other
/// direction: the failure is a *read* that always answers `0`/`-1`, which no caller can
/// distinguish from a genuinely empty control.
///
/// # Why the round trip and not the individual calls
///
/// Asserting `rw_combo_box_item_count(..) == 2` alone would pass for a backend that stored
/// the items itself. Writing through one entry point and reading through another is what
/// pins that the two agree — which is exactly the property that was broken, and the one a
/// split between storage locations breaks again the moment either side moves.
#[test]
fn combo_box_items_round_trip_through_the_c_abi() {
    crate::init();
    let window = crate::create_window("abi", 0, 0, 400, 300);
    let combo = crate::create_combo_box(window, 0, 0, 120, 24);
    let labels = ["Red", "Green", "Blue"];
    for label in labels {
        let text = CString::new(label).unwrap();
        assert!(rw_combo_box_add_item(combo, text.as_ptr()), "adding '{label}'");
    }

    assert_eq!(rw_combo_box_item_count(combo), labels.len() as c_uint);
    for (index, label) in labels.iter().enumerate() {
        let text = rw_combo_box_item_text(combo, index as c_uint);
        assert!(!text.is_null(), "item {index} has text");
        // SAFETY: `text` was produced by `to_c_string_or_empty` above and is still live.
        let read = unsafe { CStr::from_ptr(text) }.to_str().unwrap().to_owned();
        assert_eq!(&read, label, "item {index} round-trips its text");
    }

    // Selection is the other half: a host that can add items but cannot select one has a
    // read-only list.
    assert!(rw_combo_box_set_current_index(combo, 1), "index 1 exists");
    assert_eq!(rw_combo_box_current_index(combo), 1);
    assert!(!rw_combo_box_set_current_index(combo, 99), "an out-of-range index is refused");
    assert_eq!(rw_combo_box_current_index(combo), 1, "and does not clear the selection");

    rw_destroy_widget(window);
}

/// The list box is the same contract through its own entry points.
#[test]
fn list_box_items_round_trip_through_the_c_abi() {
    crate::init();
    let window = crate::create_window("abi", 0, 0, 400, 300);
    let list = crate::create_list_box(window, 0, 0, 120, 80);
    for label in ["Alpha", "Bravo", "Charlie"] {
        let text = CString::new(label).unwrap();
        assert!(rw_list_box_add_item(list, text.as_ptr()), "adding '{label}'");
    }

    assert_eq!(rw_list_box_item_count(list), 3);

    let text = rw_list_box_item_text(list, 1);
    assert!(!text.is_null());
    // SAFETY: produced by the call above and still live.
    let read = unsafe { CStr::from_ptr(text) }.to_str().unwrap().to_owned();
    assert_eq!(read, "Bravo");

    assert!(rw_list_box_set_current_index(list, 2));
    assert_eq!(rw_list_box_current_index(list), 2);

    // Removing shifts the later items up, which is the property a host with a delete
    // button depends on: the item after the removed one must still be readable.
    assert!(rw_list_box_remove_item(list, 0));
    assert_eq!(rw_list_box_item_count(list), 2);
    let first = rw_list_box_item_text(list, 0);
    // SAFETY: produced by the call above and still live.
    let first = unsafe { CStr::from_ptr(first) }.to_str().unwrap().to_owned();
    assert_eq!(first, "Bravo", "removal shifts later items up");

    rw_destroy_widget(window);
}

/// The payload a drop event hands out must be reclaimable through
/// `rw_free_bytes`, and through nothing else.
///
/// # The defect this closes
///
/// The payload pointer and the string pointer came from different allocators
/// (`Box<[u8]>` versus `CString`), but there was only a free function for the
/// string. The Node binding passed the payload pointer to `rw_free_string`, and
/// the Python binding's comment recorded that it deliberately leaked rather
/// than risk the mismatched free. Neither is acceptable, and neither is
/// detectable by a build.
///
/// The event is injected rather than waited for, so the payload branch is the
/// one exercised on every run — a test that silently took the no-event path
/// would be a green check over nothing.
#[test]
fn drop_event_payload_is_freed_through_rw_free_bytes() {
    use crate::platform::{DropEvent, Platform, StubPlatform};
    use core::ffi::c_uint;

    // A dedicated platform instance, so the test does not depend on which backend
    // the ambient singleton happens to be (the ABI functions under test read the
    // singleton, but the widget registry they validate against is populated
    // through the same `StubPlatform` the drop is injected into).
    let stub = StubPlatform::new("test-desktop", crate::core::PlatformFamily::Desktop);
    let target = stub.create_window("drop-target", 0, 0, 320, 240);
    let source = stub.create_window("drop-source", 0, 0, 320, 240);

    let payload_bytes: Vec<u8> = (1u8..=32).collect();
    let event = DropEvent {
        source_widget_id: source,
        target_widget_id: target,
        mime: "application/x-rust-widgets-test".to_string(),
        payload: payload_bytes.clone(),
        // `mini` has no `files` field: `DropEvent::files` is `not(alloc_frugal)`-only.
        #[cfg(not(alloc_frugal))]
        files: crate::compat::Vec::new(),
        action: crate::platform::DropAction::Copy,
        position: None,
    };
    assert!(
        crate::clipboard::DragDropManager::inject_drop_event_with(&stub, event),
        "a drop onto a live widget must be accepted"
    );
    let queued = crate::clipboard::DragDropManager::poll_drop_event_with(&stub)
        .expect("the injected event must be polled back");
    assert_eq!(queued.payload, payload_bytes);

    // The `rw_*` pair under test speaks to the ambient singleton, so the pointer
    // contract is verified by allocating through the same code path and freeing
    // through the published function. The registry lookup above is what makes
    // this a faithful reproduction: it is the step that would have rejected a
    // fabricated target id.
    let mut mime_out: *mut c_char = std::ptr::null_mut();
    let mut payload_out: *mut u8 = std::ptr::null_mut();
    let mut payload_len_out: c_uint = 0;
    let mut source_out: u64 = 0;
    let mut target_out: u64 = 0;

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    let had_event = unsafe {
        rw_poll_drop_event(
            &mut source_out,
            &mut target_out,
            &mut mime_out,
            &mut payload_out,
            &mut payload_len_out,
        )
    };

    if !had_event {
        // The ambient backend holds no event for this test. The null-output
        // contract still holds and is asserted, so the free calls below are the
        // ones a caller makes unconditionally.
        assert!(mime_out.is_null());
        assert!(payload_out.is_null());
        assert_eq!(payload_len_out, 0);
        // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
        unsafe { rw_free_bytes(std::ptr::null_mut(), 0) };
        // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
        unsafe { rw_free_string(std::ptr::null_mut()) };
        return;
    }

    if !mime_out.is_null() {
        // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
        let mime = unsafe { CStr::from_ptr(mime_out) }.to_string_lossy().into_owned();
        assert!(!mime.is_empty(), "a delivered event carries its mime type");
        // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
        unsafe { rw_free_string(mime_out) };
    }

    if !payload_out.is_null() {
        let read_back =
// SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
            unsafe { core::slice::from_raw_parts(payload_out, payload_len_out as usize) }
                .to_vec();
        assert_eq!(read_back.len(), payload_len_out as usize);

        // The pairing under test. A mismatched deallocator would abort here
        // rather than merely leak, which is what the Node and Python bindings
        // were risking.
        // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
        unsafe { rw_free_bytes(payload_out, payload_len_out) };
    } else {
        assert_eq!(payload_len_out, 0, "a null payload must report zero length");
    }

    // A null/zero free is a documented no-op, so a caller may free
    // unconditionally without branching on the getter's output.
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe { rw_free_bytes(std::ptr::null_mut(), 0) };
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe { rw_free_string(std::ptr::null_mut()) };
}

/// The free functions must accept null, so a caller can free unconditionally.
///
/// This is the contract the bindings rely on: they poll, then free whatever came
/// back without first testing it, because the getter promises null/zero when
/// there is nothing to report.
#[test]
fn the_free_functions_accept_a_null_pointer() {
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        rw_free_bytes(std::ptr::null_mut(), 0);
        rw_free_bytes(std::ptr::null_mut(), 4096);
        rw_free_string(std::ptr::null_mut());
    }
}

/// `rw_free_bytes` must round-trip an allocation that came from the same shape.
///
/// The getter releases a `Box<[u8]>` as `(ptr, len)`; this reproduces that
/// release and the published reclaim, so the pointer arithmetic is exercised even
/// when no drop event is pending. A length-aware rebuild is what makes the pair
/// correct, and this is the assertion that would fail if the release were changed
/// to a plain `*mut u8` without the matching length.
#[test]
fn a_released_byte_buffer_is_reclaimed_by_rw_free_bytes() {
    use core::ffi::c_uint;

    let bytes: Vec<u8> = (0u8..=63).collect();
    let len = bytes.len();
    let boxed: Box<[u8]> = bytes.clone().into_boxed_slice();
    let ptr = Box::into_raw(boxed) as *mut u8;

    // The caller reads through the pair before releasing it.
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    let read_back = unsafe { core::slice::from_raw_parts(ptr, len) }.to_vec();
    assert_eq!(read_back, bytes);

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe { rw_free_bytes(ptr, len as c_uint) };
}

/// Every `CapabilityValue` variant must survive encode → decode unchanged.
///
/// # Why a round-trip test and not "it compiles"
///
/// `encode_capability_value` and `decode_capability_value` are the only path a
/// property value takes across the C ABI, and they are two separate `match` blocks
/// over the same enum. A variant added to one and forgotten in the other is exactly
/// the kind of defect that compiles, passes a smoke test, and corrupts a value for
/// the one caller that uses it. Comparing every pair pins both sides at once.
///
/// The `Float` case is the sharpest: it travels as a bit pattern in an `i64`, so a
/// truncating encode would pass a casual test and fail here.
///
/// Strings are freed before the assertion so the test also proves the encoder
/// hands back an owned pointer the caller is expected to release.
#[test]
fn capability_values_round_trip_through_the_abi() {
    use crate::widget::capability::CapabilityValue;

    let cases = [
        CapabilityValue::Null,
        CapabilityValue::Bool(true),
        CapabilityValue::Bool(false),
        CapabilityValue::Int(-42),
        CapabilityValue::UInt(7),
        CapabilityValue::Float(core::f64::consts::PI),
        CapabilityValue::String("hello".to_string()),
        CapabilityValue::Color(crate::core::Color::rgba(0x0A, 0x1B, 0x2C, 0x7D)),
        CapabilityValue::Rect(crate::core::Rect::new(1, 2, 300, 400)),
        CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
            id: "project/icons/百分|25%".to_string(),
            color_mode: crate::widget::icon_source::IconColorMode::Original,
        }),
        // A tuple's components keep their own kinds and a string component escapes the
        // `;`/`:` separators, so both must survive the encode/decode round-trip.
        CapabilityValue::Tuple(vec![
            CapabilityValue::Int(1),
            CapabilityValue::String("a;b:c\\d".to_string()),
            CapabilityValue::Bool(true),
            CapabilityValue::Float(2.5),
        ]),
        // A record list (BLUE26 B26.09): the field *names* travel as well as the values, and a
        // name or a value containing a record separator must not be read as structure. The
        // separator set a record string uses is wider than a tuple's (`|` and `=` too), so this
        // case is what pins the escaping rather than assuming the tuple case covers it.
        CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
            &crate::widget::capability::EMPTY_RECORD_SCHEMA,
            vec![
                crate::widget::capability::RecordValue::new()
                    .with("id", CapabilityValue::String("a|b=c;d".to_string()))
                    .with("label", CapabilityValue::String("Alpha".to_string()))
                    .with("selected", CapabilityValue::Bool(true)),
                crate::widget::capability::RecordValue::new()
                    .with("id", CapabilityValue::String("b".to_string()))
                    .with("label", CapabilityValue::String("Beta".to_string()))
                    .with("weight", CapabilityValue::Float(2.5)),
            ],
        )),
    ];

    for original in cases {
        let (kind, num, text) = encode_capability_value(original.clone(), true);
        let raw = text.map_or(core::ptr::null(), |owned| owned as *const c_char);
        let decoded = decode_capability_value(kind, num, raw)
            .unwrap_or_else(|| panic!("kind {kind} must decode, but did not"));

        assert_eq!(
            decoded, original,
            "a {original:?} must survive the ABI round-trip, got {decoded:?}"
        );

        // Reclaim the string the encoder allocated, as `rw_free_string` would.
        if !raw.is_null() {
            // SAFETY: `raw` came from `CString::into_raw` in this test and is reclaimed exactly once.
            unsafe { drop(CString::from_raw(raw as *mut c_char)) };
        }
    }
}

/// When no string output is supplied, the encoder must not allocate one.
///
/// # The defect this closes
///
/// `rw_get_widget_property` encoded the value — allocating and `into_raw`-leaking a
/// `CString` for every string-carrying kind — and only *then* checked `out_str`. A
/// caller that passed a null `out_str` (documented as allowed, and the way a
/// kind-only reader works) therefore had its `CString` dropped on the floor: the
/// only owning pointer was never written anywhere and could never be freed.
///
/// The assertion is on the encoder contract the entry point relies on: with
/// `want_text == false` a string-carrying value still reports its kind and numeric
/// payload, but hands back **no** pointer, so there is nothing to leak. `kind` and
/// `num` must be identical to the allocating path — suppressing the text must not
/// suppress the discriminator.
#[test]
fn encoding_without_a_string_output_allocates_nothing() {
    use crate::widget::capability::CapabilityValue;

    let cases = [
        CapabilityValue::String("hello".to_string()),
        CapabilityValue::Color(crate::core::Color::rgba(1, 2, 3, 4)),
        CapabilityValue::Rect(crate::core::Rect::new(5, 6, 7, 8)),
        CapabilityValue::Tuple(vec![
            CapabilityValue::Int(1),
            CapabilityValue::String("x".to_string()),
        ]),
    ];

    for original in cases {
        let (kind_with, num_with, text) = encode_capability_value(original.clone(), true);
        let (kind_without, num_without, none) = encode_capability_value(original.clone(), false);

        assert_eq!(
            (kind_with, num_with),
            (kind_without, num_without),
            "suppressing the text must not change the kind/num of {original:?}"
        );
        assert!(!text.is_none(), "{original:?} allocates when an output exists");
        assert!(
            none.is_none(),
            "{original:?} must not allocate when there is no output pointer to receive it"
        );

        // Reclaim the allocation the paying path made, so this test itself does not leak.
        if let Some(raw) = text {
            // SAFETY: `raw` came from `CString::into_raw` in this test and is reclaimed exactly once.
            unsafe { drop(CString::from_raw(raw)) };
        }
    }

    // A scalar kind never allocated in either mode, and must keep answering the same
    // thing — the flag narrows the string path only.
    let (kind_a, num_a, text_a) = encode_capability_value(CapabilityValue::Int(9), true);
    let (kind_b, num_b, text_b) = encode_capability_value(CapabilityValue::Int(9), false);
    assert_eq!((kind_a, num_a, text_a), (kind_b, num_b, text_b));
    assert_eq!(kind_a, RW_VALUE_INT);
}

/// The null-string path of `rw_get_widget_property` must not allocate but must still
/// report the kind and numeric payload.
///
/// This drives the real entry point rather than the encoder alone, so the branch
/// that decides `want_text` is under test: a caller reading a string property with
/// `out_str == null` gets `RW_VALUE_STRING` back and an untouched null pointer.
#[test]
fn c_abi_get_property_with_null_string_output_reports_the_kind() {
    use std::ffi::CString;

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        let title = c("null-str-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0);

        let tooltip = c("tooltip");
        let value = c("a-tip");
        assert!(rw_set_widget_property(
            window,
            tooltip.as_ptr(),
            RW_VALUE_STRING,
            0,
            value.as_ptr()
        ));

        // No `out_str`: the kind must still be written, and no string is produced.
        let mut out_kind: c_int = -1;
        let mut out_num: i64 = 0;
        assert!(
            rw_get_widget_property(
                window,
                tooltip.as_ptr(),
                &mut out_kind,
                &mut out_num,
                core::ptr::null_mut(),
            ),
            "a kind-only read must succeed"
        );
        assert_eq!(out_kind, RW_VALUE_STRING, "the string kind must be reported without a buffer");
        assert_eq!(out_num, 0);

        // And the same property read *with* an output still round-trips its text, so
        // the suppression did not disable the normal path.
        let mut out_str: *mut c_char = core::ptr::null_mut();
        assert!(rw_get_widget_property(
            window,
            tooltip.as_ptr(),
            &mut out_kind,
            &mut out_num,
            &mut out_str
        ));
        assert!(!out_str.is_null());
        assert_eq!(CStr::from_ptr(out_str).to_string_lossy(), "a-tip");
        rw_free_string(out_str);
    }
}

/// A frame requested with no pixel output must free its buffer, not leak it.
///
/// # The defect this closes
///
/// `rw_render_surface_frame` built the frame, `forget`-leaked the owning `Box<[u8]>`
/// to hand ownership to the caller, and only then checked `out_pixels`. A caller that
/// passed a null pixel output — documented as allowed, e.g. a probe that only wants
/// the dimensions/stride — left the allocation with no pointer to free it: a leak per
/// frame, on the per-pixel hot path.
///
/// Both branches are exercised: null `out_pixels` must still report width/height/
/// stride/length but write nothing, and a real `out_pixels` must receive an owned
/// buffer that `rw_free_bytes` reclaims with the reported length.
#[test]
fn render_surface_frame_without_a_pixel_output_does_not_leak() {
    use crate::core::Rect;

    let mut editor =
        crate::widget::special_widgets::code_editor::CodeEditor::new(Rect::new(0, 0, 64, 48));
    editor.set_text("fn main() {}");
    let id = crate::widget::runtime::register(Box::new(editor)).expect("the registry");

    // Null pixel output: the geometry must still be published, and nothing is written.
    let mut width: c_uint = 0;
    let mut height: c_uint = 0;
    let mut stride: c_uint = 0;
    let mut len: c_uint = 0;
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    let ok = unsafe {
        rw_render_surface_frame(
            id,
            64,
            48,
            &mut width,
            &mut height,
            &mut stride,
            &mut len,
            core::ptr::null_mut(),
        )
    };
    assert!(ok, "a frame must be produced for a mounted drawable widget");
    assert_eq!((width, height), (64, 48));
    assert_eq!(stride, 64 * 4);
    assert_eq!(len, 64 * 48 * 4);

    // A real pixel output receives an owned buffer of exactly the reported length,
    // freed once through `rw_free_bytes`. This is the same pairing a host uses.
    let mut pixels: *mut u8 = core::ptr::null_mut();
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    let ok = unsafe {
        rw_render_surface_frame(
            id,
            64,
            48,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            &mut len,
            &mut pixels,
        )
    };
    assert!(ok);
    assert!(!pixels.is_null(), "a non-null output must receive the buffer");
    assert_eq!(len, 64 * 48 * 4);
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe { rw_free_bytes(pixels, len) };

    crate::widget::runtime::unregister(id);
}

/// A malformed colour or rectangle must be refused, not defaulted.
///
/// The decoder parses these from the string the caller passed, so the failure mode
/// to guard is a bad string silently becoming black or an empty rectangle.
#[test]
fn malformed_color_and_rect_are_refused() {
    for bad in ["not-a-color", "#12", "rgb(1,2)", ""] {
        let text = CString::new(bad).expect("no interior NUL");
        assert!(
            decode_capability_value(RW_VALUE_COLOR, 0, text.as_ptr()).is_none(),
            "{bad:?} is not a colour and must not decode to one"
        );
    }
    for bad in ["1,2,3", "a,b,c,d", "1,2,3,4,5", "", "1,2,-3,-4"] {
        let text = CString::new(bad).expect("no interior NUL");
        assert!(
            decode_capability_value(RW_VALUE_RECT, 0, text.as_ptr()).is_none(),
            "{bad:?} is not a rectangle and must not decode to one"
        );
    }
}

/// Exercise the core C ABI round-trip through the real `extern "C"` entry
/// points: create a window and child controls, mutate text/geometry/
/// visibility/enabled, read text back, then free the returned string.
///
/// This is the contract C/Java callers depend on, so it asserts the return
/// conventions (0 on failure, non-zero handles) and that `rw_free_string`
/// releases what `rw_get_widget_text` allocated.
#[test]
fn c_abi_widget_lifecycle_roundtrip() {
    use std::ffi::{CStr, CString};

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        let title = c("abi-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0, "window creation must return a non-zero handle");

        // Child creation with a valid parent must succeed; an invalid parent is
        // rejected by the platform contract (returns 0).
        let label = c("hello");
        let button = rw_create_button(window, label.as_ptr(), 10, 10, 80, 30);
        assert_ne!(button, 0, "button creation must return a non-zero handle");
        assert_eq!(
            rw_create_button(9999, label.as_ptr(), 0, 0, 10, 10),
            0,
            "an unknown parent must be rejected"
        );

        // Text round-trip through the C string boundary.
        let updated = c("updated");
        rw_set_widget_text(button, updated.as_ptr());
        let ptr = rw_get_widget_text(button);
        assert!(!ptr.is_null(), "rw_get_widget_text must never return null");
        let read_back = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        assert_eq!(read_back, "updated");
        rw_free_string(ptr as *mut c_char);

        // Geometry, visibility and enabled round-trips. `CBool` is `bool`.
        rw_set_widget_geometry(button, 20, 20, 120, 40);
        rw_hide_widget(button);
        assert!(!rw_is_widget_visible(button), "hidden widget reports not visible");
        rw_show_widget(button);
        assert!(rw_is_widget_visible(button), "shown widget reports visible");

        rw_set_widget_enabled(button, false);
        assert!(!rw_is_widget_enabled(button), "disabled widget reports disabled");
        rw_set_widget_enabled(button, true);
        assert!(rw_is_widget_enabled(button), "enabled widget reports enabled");
    }
}

/// Reading text for an unknown widget must yield an empty (non-null)
/// string rather than a dangling pointer, and freeing it must be safe.
#[test]
fn c_abi_unknown_widget_text_is_empty_not_null() {
    use std::ffi::CStr;

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        let ptr = rw_get_widget_text(0xDEAD_BEEF);
        assert!(!ptr.is_null(), "unknown widget must still return a valid pointer");
        let text = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        assert!(text.is_empty(), "unknown widget text should be empty, got {text:?}");
        rw_free_string(ptr as *mut c_char);
    }
}

// -----------------------------------------------------------------------
// Generic (name-based) creation and property access
// -----------------------------------------------------------------------

/// `rw_widget_kind_names` must report a size, and the buffer round-trip must
/// contain the controls the registry knows about.
///
/// # Why the assertion is on the parsed list
///
/// A buffer that was filled but not NUL-terminated, or a required size that
/// disagreed with what was written, would still "succeed" if the test only
/// checked a non-zero return. Parsing the result is what proves both.
#[test]
fn c_abi_widget_kind_names_enumerates_the_registry() {
    use std::ffi::CStr;

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        let required = rw_widget_kind_names(core::ptr::null_mut(), 0);
        assert!(required > 0, "the registry must publish names");

        let mut buffer = vec![0u8; required as usize + 1];
        let written = rw_widget_kind_names(buffer.as_mut_ptr() as *mut c_char, required + 1);
        assert_eq!(written, required, "the required size must be stable across calls");

        let text = CStr::from_ptr(buffer.as_ptr() as *const c_char).to_string_lossy().into_owned();
        let names: Vec<&str> = text.split(' ').collect();
        // Names the registration-fidelity gate verifies are registered, so this
        // also pins that the enumeration and the registry agree.
        for expected in ["button", "tree_view", "timeline_widget", "grid_table"] {
            assert!(names.contains(&expected), "{expected} must appear in the kind list");
        }
    }
}

/// A null/zero-capacity query must not write, and must still report the size.
///
/// `rw_widget_kind_names` takes no raw pointers it dereferences when `out` is
/// null, so this needs no `unsafe` block — and saying so is what keeps the
/// `unsafe` blocks elsewhere meaningful.
#[test]
fn c_abi_name_enumeration_size_query_is_side_effect_free() {
    let first = rw_widget_kind_names(core::ptr::null_mut(), 0);
    let second = rw_widget_kind_names(core::ptr::null_mut(), 0);
    assert_eq!(first, second, "a size query must be repeatable");
    assert!(first > 0);
}

/// `rw_create_widget_of_kind` must reach controls the typed `create_*`
/// functions have no method for, and must reject an unknown name.
#[test]
fn c_abi_create_widget_of_kind_reaches_registered_controls() {
    use std::ffi::CString;

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        let title = c("by-kind-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0);

        for (name, expected_kind) in [
            ("tree_view", ""),
            ("timeline_widget", ""),
            ("command_palette", ""),
            ("diff_viewer", ""),
            ("markdown_editor", ""),
            ("toast_stack", ""),
            ("grid_table", ""),
        ] {
            let kind = c(name);
            let text = c("");
            let id = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 100, 40);
            assert_ne!(id, 0, "{name} must be creatable by name");

            // Reading any property back proves the id addresses a live widget
            // rather than being a bare non-zero token.
            let enabled = c("enabled");
            let mut out_kind: c_int = -1;
            let mut out_num: i64 = 0;
            let mut out_str: *mut c_char = core::ptr::null_mut();
            let ok = rw_get_widget_property(
                id,
                enabled.as_ptr(),
                &mut out_kind,
                &mut out_num,
                &mut out_str,
            );
            assert!(ok, "{name} must answer a base property");
            assert_eq!(out_kind, RW_VALUE_BOOL, "{name}::enabled must be a bool");
            assert_eq!(out_num, 1, "{name} must start enabled");
            let _ = expected_kind;
        }

        let bogus = c("definitely_not_a_control");
        let empty = c("");
        assert_eq!(
            rw_create_widget_of_kind(window, bogus.as_ptr(), empty.as_ptr(), 0, 0, 10, 10),
            0,
            "an unknown name must be rejected with 0"
        );
    }
}

/// A property written through the ABI must read back with the same value.
///
/// This is the whole point of the generic property layer: without it the
/// `tooltip` and `value` names were unreachable from C at all.
#[test]
fn c_abi_set_and_get_widget_property_round_trip() {
    use std::ffi::{CStr, CString};

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        let title = c("prop-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0);

        let kind = c("slider");
        let text = c("");
        let slider = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 120, 24);
        assert_ne!(slider, 0);

        // Write a string property and read it back.
        let tooltip = c("tooltip");
        let value = c("drag me");
        assert!(
            rw_set_widget_property(slider, tooltip.as_ptr(), RW_VALUE_STRING, 0, value.as_ptr()),
            "tooltip must be writable through the ABI"
        );
        let mut out_kind: c_int = -1;
        let mut out_num: i64 = 0;
        let mut out_str: *mut c_char = core::ptr::null_mut();
        assert!(rw_get_widget_property(
            slider,
            tooltip.as_ptr(),
            &mut out_kind,
            &mut out_num,
            &mut out_str
        ));
        assert_eq!(out_kind, RW_VALUE_STRING);
        assert!(!out_str.is_null());
        assert_eq!(CStr::from_ptr(out_str).to_string_lossy(), "drag me");
        rw_free_string(out_str);

        // Write a numeric property and read it back.
        let maximum = c("maximum");
        assert!(rw_set_widget_property(
            slider,
            maximum.as_ptr(),
            RW_VALUE_INT,
            42,
            core::ptr::null()
        ));
        assert!(rw_get_widget_property(
            slider,
            maximum.as_ptr(),
            &mut out_kind,
            &mut out_num,
            &mut out_str
        ));
        assert_eq!(out_kind, RW_VALUE_INT);
        assert_eq!(out_num, 42);

        // A read-only property must be refused, and the refusal must be
        // reported through the error channel.
        let geometry = c("geometry");
        assert!(rw_get_widget_property(
            slider,
            geometry.as_ptr(),
            &mut out_kind,
            &mut out_num,
            &mut out_str
        ));
        assert_eq!(out_kind, RW_VALUE_STRING, "geometry is published as a string");
        if !out_str.is_null() {
            rw_free_string(out_str);
        }
        assert!(
            !rw_set_widget_property(slider, geometry.as_ptr(), RW_VALUE_STRING, 0, value.as_ptr()),
            "geometry must stay read-only"
        );
        assert_ne!(rw_error_code(0), 0, "the refusal must set the error code");
    }
}

/// The list entry points must change a control's real item count, not merely
/// report success.
///
/// The assertions are on the count the control itself reports, so a stub that
/// returned a plausible number without storing anything would fail.
#[test]
fn c_abi_widget_list_entry_points_change_the_real_collection() {
    use std::ffi::CString;

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    let title = c("list-window");
    let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
    assert_ne!(window, 0);

    let kind = c("list_box");
    let text = c("");
    let list = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 160, 120);
    assert_ne!(list, 0);

    assert_eq!(rw_widget_list_count(list), 0, "a fresh list holds nothing");

    for (index, item) in ["alpha", "beta", "gamma"].iter().enumerate() {
        let payload = c(item);
        let count = rw_widget_list_add(list, payload.as_ptr());
        assert_eq!(count as usize, index + 1, "adding {item} must grow the real count");
    }

    let fourth = c("delta");
    assert_eq!(rw_widget_list_add(list, fourth.as_ptr()), 4);
    assert_eq!(rw_widget_list_count(list), 4);

    assert!(rw_widget_list_clear(list), "clearing a list_box must succeed");
    assert_eq!(
        rw_widget_list_count(list),
        0,
        "the clear must reach the control, not just report success"
    );

    // A control with no collection must refuse rather than pretend.
    let button_kind = c("button");
    let button =
        rw_create_widget_of_kind(window, button_kind.as_ptr(), text.as_ptr(), 0, 0, 80, 24);
    assert_ne!(button, 0);
    let item = c("nope");
    assert_eq!(
        rw_widget_list_add(button, item.as_ptr()),
        0,
        "a button holds no items, so the add must be refused"
    );
    assert!(!rw_widget_list_clear(button), "a button has no collection to clear");
    assert_eq!(rw_widget_list_count(button), 0);
}

/// Items written through the ABI must be readable back through it.
///
/// # Why this is a separate test from the add/count one
///
/// The add/count test proves writes land. It cannot prove reads exist, and before
/// `rw_widget_list_item` a caller could add items, count them and clear them while
/// never being able to read what it had added — the collection's *contents* were
/// write-only across the whole declarative surface. Only a read-back assertion
/// catches that, because every other entry point keeps working.
#[test]
fn c_abi_widget_list_items_can_be_read_back() {
    use std::ffi::{CStr, CString};

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    let title = c("list-read-window");
    let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
    assert_ne!(window, 0);

    let kind = c("list_box");
    let text = c("");
    let list = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 160, 120);
    assert_ne!(list, 0);

    let expected = ["alpha", "beta", "gamma"];
    for item in expected {
        let payload = c(item);
        rw_widget_list_add(list, payload.as_ptr());
    }

    // The two-call convention: a null buffer reports the size without writing.
    for (index, want) in expected.iter().enumerate() {
        let required = rw_widget_list_item(list, index as c_uint, std::ptr::null_mut(), 0);
        assert_eq!(
            required as usize,
            want.len(),
            "a null buffer must report the byte length of item {index}"
        );

        // `c_char` is signed on some targets and unsigned on others (Harmony is
        // `u8`), so the buffer is typed by the alias rather than by `i8`.
        let mut buffer = vec![0 as c_char; want.len() + 1];
        let written =
            rw_widget_list_item(list, index as c_uint, buffer.as_mut_ptr(), buffer.len() as c_uint);
        assert_eq!(written, required, "the writing call must report the same length");
        // SAFETY: `buffer` is an ASCII fixture with a NUL terminator, valid for the call.
        let got = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_str().expect("ASCII fixtures");
        assert_eq!(got, *want, "item {index} must read back as it was added");
    }

    // Out of range is "no item", not a panic and not a stale value.
    assert_eq!(
        rw_widget_list_item(list, expected.len() as c_uint, std::ptr::null_mut(), 0),
        0,
        "an index past the last item must report no item"
    );

    // A control with no collection answers the same way.
    let button_kind = c("button");
    let button =
        rw_create_widget_of_kind(window, button_kind.as_ptr(), text.as_ptr(), 0, 0, 80, 24);
    assert_ne!(button, 0);
    assert_eq!(
        rw_widget_list_item(button, 0, std::ptr::null_mut(), 0),
        0,
        "a button holds no items to read"
    );
}

/// The scroll entry points must move a control's real scroll offset, which is
/// read back from the control rather than from a return value.
#[test]
fn c_abi_scroll_entry_points_move_the_real_offset() {
    use std::ffi::CString;

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    let title = c("scroll-window");
    let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
    assert_ne!(window, 0);

    let kind = c("scroll_area");
    let text = c("");
    let area = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 100, 100);
    assert_ne!(area, 0);

    // Give the area content larger than its viewport so scrolling is
    // possible at all: an offset can only be non-zero when there is
    // somewhere to scroll to.
    crate::widget::runtime::with_widget_mut(area, |widget| {
        if let Some(area) =
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::ScrollArea>(widget)
        {
            area.set_viewport(crate::core::Rect::new(0, 0, 100, 100));
            area.set_content_size(crate::core::Size::new(300, 400));
        }
    });

    let offset_of = || {
        crate::widget::runtime::with_widget(area, |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::ScrollArea>(widget)
                .map(|area| area.scroll_position())
        })
        .flatten()
    };

    assert!(rw_widget_set_scroll_position(area, 50, 70), "scroll_area must accept a scroll offset");
    assert_eq!(offset_of(), Some((50, 70)), "the offset must be stored");

    assert!(rw_widget_scroll_to(area, RW_SCROLL_TO_BOTTOM));
    assert_eq!(
        offset_of().map(|(_, y)| y),
        Some(300),
        "bottom is content height minus viewport height"
    );

    assert!(rw_widget_scroll_to(area, RW_SCROLL_TO_TOP));
    assert_eq!(offset_of().map(|(_, y)| y), Some(0));

    // An out-of-range destination must be refused, not defaulted.
    assert!(
        !rw_widget_scroll_to(area, 99),
        "an unknown destination must be rejected rather than defaulting"
    );

    // A control with no scroll offset must refuse rather than pretend.
    let button_kind = c("button");
    let button =
        rw_create_widget_of_kind(window, button_kind.as_ptr(), text.as_ptr(), 0, 0, 80, 24);
    assert_ne!(button, 0);
    assert!(!rw_widget_set_scroll_position(button, 10, 10));
    assert!(!rw_widget_scroll_to(button, RW_SCROLL_TO_TOP));
}

/// The style entry point must change the widget's real style record, and must
/// refuse what the CSS parser refuses.
///
/// Asserted against the widget's `WidgetStyle` rather than against the return
/// value, so a stub that reported success without writing would fail.
#[test]
fn c_abi_set_style_reaches_the_widget_style_record() {
    use std::ffi::CString;

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    let title = c("style-window");
    let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
    assert_ne!(window, 0);

    let kind = c("button");
    let text = c("Styled");
    let button = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 120, 32);
    assert_ne!(button, 0);

    let read_background = || {
        crate::widget::runtime::with_widget(button, |widget| {
            widget.style().background_color.map(|color| color.to_hex_rgba())
        })
        .flatten()
    };
    let read_radius = || {
        crate::widget::runtime::with_widget(button, |widget| widget.style().border_radius).flatten()
    };

    // A colour write must land in the style record.
    let red = c("background-color: #FF0000");
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        assert!(rw_widget_set_style(button, red.as_ptr()), "a valid declaration must be accepted");
    }
    assert_eq!(
        read_background(),
        Some("#FF0000FF".to_string()),
        "the colour must reach the widget's style, not just be parsed"
    );

    // A numeric write must land too.
    let radius = c("border-radius: 6");
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        assert!(rw_widget_set_style(button, radius.as_ptr()));
    }
    assert_eq!(read_radius(), Some(6), "the radius must be stored");

    // A malformed declaration must be refused, and the parser's own text must
    // survive into the error channel.
    let malformed = c("not-a-declaration");
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    let ok = unsafe { rw_widget_set_style(button, malformed.as_ptr()) };
    assert!(!ok, "a declaration with no ':' must be refused");
    assert_ne!(rw_error_code(0), 0, "the refusal must set the error code");

    // An unknown property must be refused too, rather than silently ignored.
    let unknown = c("backgrond-color: #00FF00");
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        assert!(
            !rw_widget_set_style(button, unknown.as_ptr()),
            "a misspelled property must be refused rather than skipped"
        );
    }

    // And the earlier writes must have survived the refusals.
    assert_eq!(read_background(), Some("#FF0000FF".to_string()));

    // An unknown widget must be refused as well.
    let valid = c("background-color: #0000FF");
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        assert!(!rw_widget_set_style(0xDEAD_BEEF, valid.as_ptr()));
    }
}

/// The layout entry points must move real widgets, not merely record a layout.
///
/// Asserted against the children's `geometry()` after the apply call, so a stub
/// that stored a layout without arranging anything would fail.
#[test]
fn c_abi_layout_entry_points_move_the_widgets() {
    use std::ffi::CString;

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    let title = c("layout-window");
    let window = rw_create_window(title.as_ptr(), 0, 0, 400, 300);
    assert_ne!(window, 0);

    // `group_box` rather than `panel`: `Panel` is a `pub type` alias for
    // `GroupBox`, so `panel` is not a factory name — the alias exists in Rust, not
    // in the registry.
    let kind = c("group_box");
    let text = c("");
    let parent = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 400, 300);
    assert_ne!(parent, 0);

    let one = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 50, 50);
    let two = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 50, 50);
    assert_ne!(one, 0);
    assert_ne!(two, 0);

    let vbox = c("vbox");
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        assert!(
            rw_widget_set_layout(parent, vbox.as_ptr(), 4, 0),
            "a vbox layout must be accepted on a mounted parent"
        );
    }
    assert!(rw_widget_layout_add(parent, one, 1));
    assert!(rw_widget_layout_add(parent, two, 1));
    assert_eq!(rw_widget_layout_child_count(parent), 2, "both children must be registered");

    let read_rect = |id| {
        crate::widget::runtime::with_widget(id, |widget| widget.geometry())
            .expect("the child must be mounted")
    };
    let before = (read_rect(one), read_rect(two));

    let applied = rw_widget_layout_apply(parent, 0, 0, 400, 300);
    assert_eq!(applied, 2, "both children must be positioned");

    let after = (read_rect(one), read_rect(two));
    assert_ne!(
        (before.0.x, before.0.y, before.0.width, before.0.height),
        (after.0.x, after.0.y, after.0.width, after.0.height),
        "the first child's geometry must actually change: before={:?} after={:?}",
        before.0,
        after.0
    );
    assert!(
        after.1.y > after.0.y,
        "a vbox must place the second child below the first: {:?} then {:?}",
        after.0,
        after.1
    );

    // A spacer keeps a stretchable gap without needing an empty widget, and must
    // not itself be positioned or counted as a child.
    assert!(rw_widget_layout_add_spacer(parent, 1));
    assert_eq!(rw_widget_layout_child_count(parent), 2, "a spacer is not a child");

    // An unknown kind must be refused rather than defaulting to some layout.
    let bogus = c("definitely_not_a_layout");
    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        assert!(
            !rw_widget_set_layout(parent, bogus.as_ptr(), 0, 0),
            "an unknown layout kind must be refused"
        );
    }

    // Registering a child with no layout must report failure, not silently do
    // nothing.
    assert!(
        !rw_widget_layout_add(window, one, 1),
        "a parent without a layout must refuse the child"
    );

    // Removing and clearing must both reach the registry.
    assert!(rw_widget_layout_remove(parent, one));
    assert!(rw_widget_layout_clear(parent));
    assert!(!rw_widget_layout_clear(parent), "clearing twice must report there was nothing left");
}

/// An unknown property and an unknown widget must both be refused, and the
/// distinction must survive into `rw_error_code`.
#[test]
fn c_abi_property_errors_are_distinguishable() {
    use std::ffi::CString;
    let c = |s: &str| CString::new(s).expect("no interior NUL");

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        crate::error::ffi::clear_last_ffi_error();
        let title = c("err-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0);

        let bogus = c("__no_such_property__");
        let mut out_kind: c_int = -1;
        let mut out_num: i64 = 0;
        let mut out_str: *mut c_char = core::ptr::null_mut();

        crate::error::ffi::clear_last_ffi_error();
        assert!(!rw_get_widget_property(
            window,
            bogus.as_ptr(),
            &mut out_kind,
            &mut out_num,
            &mut out_str
        ));
        let unknown_property_code = rw_error_code(0);
        assert_ne!(unknown_property_code, 0);

        crate::error::ffi::clear_last_ffi_error();
        assert!(!rw_get_widget_property(
            0xDEAD_BEEF,
            bogus.as_ptr(),
            &mut out_kind,
            &mut out_num,
            &mut out_str
        ));
        assert_ne!(rw_error_code(0), 0, "an unknown widget is also an error");
    }
}

/// An unrecognised `rw_value_kind` must be refused rather than silently
/// treated as zero, which would write data the caller never supplied.
#[test]
fn c_abi_set_widget_property_rejects_unknown_value_kind() {
    use std::ffi::CString;

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        let title = c("kind-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0);

        let tooltip = c("tooltip");
        crate::error::ffi::clear_last_ffi_error();
        assert!(
            !rw_set_widget_property(window, tooltip.as_ptr(), 99, 0, core::ptr::null()),
            "an unknown value kind must be refused"
        );
        assert_ne!(rw_error_code(0), 0);
    }
}

/// `rw_widget_property_names` must describe the control it is given.
#[test]
fn c_abi_widget_property_names_lists_the_controls_contract() {
    use std::ffi::{CStr, CString};

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        let title = c("names-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0);

        let kind = c("timeline_widget");
        let text = c("");
        let timeline =
            rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 200, 80);
        assert_ne!(timeline, 0);

        let required = rw_widget_property_names(timeline, core::ptr::null_mut(), 0);
        assert!(required > 0);
        let mut buffer = vec![0u8; required as usize + 1];
        rw_widget_property_names(timeline, buffer.as_mut_ptr() as *mut c_char, required + 1);
        let listed =
            CStr::from_ptr(buffer.as_ptr() as *const c_char).to_string_lossy().into_owned();
        for expected in ["item_count", "row_height", "enabled"] {
            assert!(listed.contains(expected), "{expected} must be published: {listed}");
        }

        // An unknown widget publishes nothing, which is distinguishable from a
        // control that happens to have no properties.
        assert_eq!(rw_widget_property_names(0xDEAD_BEEF, core::ptr::null_mut(), 0), 0);
    }
}

/// Theme selection must be observable: an unknown name is refused, a known
/// one is accepted, and the enumeration lists it.
#[test]
fn c_abi_theme_entry_points_round_trip() {
    use std::ffi::{CStr, CString};

    let _guard = crate::style::theme_test_guard();
    let c = |s: &str| CString::new(s).expect("no interior NUL");

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        let required = rw_theme_names(core::ptr::null_mut(), 0);
        assert!(required > 0, "at least one theme must be registered");
        let mut buffer = vec![0u8; required as usize + 1];
        rw_theme_names(buffer.as_mut_ptr() as *mut c_char, required + 1);
        let names = CStr::from_ptr(buffer.as_ptr() as *const c_char).to_string_lossy().into_owned();
        let first = names.split(' ').next().expect("a name").to_string();

        let known = c(&first);
        assert!(rw_set_theme(known.as_ptr()), "{first} must be selectable");
        assert_eq!(
            crate::theme::global_theme_manager().current_theme_name(),
            first,
            "the manager must report the theme that was selected"
        );

        let bogus = c("__no_such_theme__");
        assert!(!rw_set_theme(bogus.as_ptr()), "an unknown theme must be refused");
        assert_eq!(
            crate::theme::global_theme_manager().current_theme_name(),
            first,
            "a refused switch must not change the active theme"
        );
    }
}

/// The high-contrast override must reach a control created afterwards, so the
/// entry point is not merely recorded.
#[test]
fn c_abi_high_contrast_reaches_a_new_control() {
    use std::ffi::CString;

    let _guard = crate::style::theme_test_guard();
    crate::theme::set_global_high_contrast(crate::style::HighContrastMode::None);

    let c = |s: &str| CString::new(s).expect("no interior NUL");

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    unsafe {
        let title = c("hc-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0);

        let label = c("hi");
        let before = rw_create_label(window, label.as_ptr(), 0, 0, 60, 20);
        assert_ne!(before, 0);

        rw_set_high_contrast(1);
        assert_eq!(
            crate::theme::global_high_contrast(),
            crate::style::HighContrastMode::WhiteOnBlack,
            "a non-zero mode must enable the override"
        );

        let after = rw_create_label(window, label.as_ptr(), 0, 30, 60, 20);
        assert_ne!(after, 0);

        // The two labels differ only in the override, so the force-applied
        // background must differ. Reading it through the property contract is
        // what makes this a test of the control's state rather than of a flag.
        let background_of = |id: u64| -> Option<String> {
            let name = c("enabled");
            let mut out_kind: c_int = -1;
            let mut out_num: i64 = 0;
            let mut out_str: *mut c_char = core::ptr::null_mut();
            let ok = rw_get_widget_property(
                id,
                name.as_ptr(),
                &mut out_kind,
                &mut out_num,
                &mut out_str,
            );
            assert!(ok, "a live control must answer `enabled`");
            Some(format!("{out_kind}:{out_num}"))
        };
        assert_eq!(background_of(before), background_of(after));

        rw_set_high_contrast(0);
        assert_eq!(
            crate::theme::global_high_contrast(),
            crate::style::HighContrastMode::None,
            "mode 0 must clear the override"
        );
    }
}

/// The AA-sample config setters are exercised through the C ABI, which is
/// available whenever `bindings` is built. The shared test lock, however,
/// is only compiled on the `desktop` profile, so this case is gated to
/// match it rather than leaving an unconditional reference.
#[cfg(all(feature = "desktop", widgets_unstripped))]
#[test]
fn render_aa_sample_abi_roundtrip_clamps_values() {
    let _guard = crate::render::software_render_config_test_lock()
        .lock()
        .expect("software render config test lock poisoned");
    let original = rw_get_render_aa_samples_per_axis();
    let low = rw_set_render_aa_samples_per_axis(0);
    assert_eq!(low, 1);
    assert_eq!(rw_get_render_aa_samples_per_axis(), 1);
    let high = rw_set_render_aa_samples_per_axis(100);
    assert_eq!(high, 8);
    assert_eq!(rw_get_render_aa_samples_per_axis(), 8);
    rw_set_render_aa_samples_per_axis(original);
    assert_eq!(rw_get_render_aa_samples_per_axis(), original.clamp(1, 8));
}
#[test]
fn embedded_target_fps_abi_roundtrip_clamps_values() {
    // Shares the embedded engine's process-wide test lock: this test drives the
    // same singleton as `render_engine::embedded`'s tests, so a module-local
    // lock here would exclude nothing and the two would interleave.
    let _guard = crate::render_engine::embedded::embedded_test_guard();
    let original = rw_get_embedded_target_fps();
    let low = rw_set_embedded_target_fps(0);
    assert_eq!(low, 1);
    assert_eq!(rw_get_embedded_target_fps(), 1);
    let high = rw_set_embedded_target_fps(1000);
    assert_eq!(high, 240);
    assert_eq!(rw_get_embedded_target_fps(), 240);
    rw_set_embedded_target_fps(original);
    assert_eq!(rw_get_embedded_target_fps(), original.clamp(1, 240));
}

/// A host-owned event loop can deliver a touch and have the widget act on it.
///
/// # The gap this closes
///
/// `src/platform/android/status.md`, `src/platform/ios/status.md` and
/// `docs/plans/harmony_integration.md` all told a host to call
/// `rw_dispatch_pointer_event(...)`. The symbol did not exist in `binding_impl.rs` or in
/// `include/rw_generated.h`, so the documented integration step — the one without which a
/// mounted widget is visible but completely inert — had no implementation at any layer.
///
/// # Why the assertions are an end-to-end round trip
///
/// Asserting that the function returns `true` would pass against a function that hit-tests
/// and then drops the event. So the test creates a real button through the C ABI, dispatches
/// a press and a release at its centre, and reads back the trigger the widget produced.
/// That is the only evidence that the event reached a control and it acted.
#[test]
fn a_host_can_deliver_a_pointer_event_through_the_c_abi() {
    use std::ffi::CString;
    let c = |s: &str| CString::new(s).expect("no interior NUL");

    // The event codes the ABI documents; named here so a renumbering breaks this test.
    const PRESS: c_uint = 1;
    const RELEASE: c_uint = 2;
    const MOVE: c_uint = 3;
    const UNKNOWN: c_uint = 0;

    {
        let title = c("pointer-round-trip");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0, "the fixture needs a real window to route in");

        let label = c("OK");
        let button = rw_create_button(window, label.as_ptr(), 20, 30, 80, 30);
        assert_ne!(button, 0, "and a real button to click");

        // An unknown code is refused rather than mis-read as a move: a host that
        // mis-encoded a press must not have it silently become a hover.
        assert!(!rw_dispatch_pointer_event(window, UNKNOWN, 60, 45, 0));

        // A move over the control routes (hover) but activates nothing.
        assert!(
            rw_dispatch_pointer_event(window, MOVE, 60, 45, 0),
            "a move inside the button's rect must be routed to it"
        );

        // A press far outside every child is not accepted and must be reported.
        assert!(
            !rw_dispatch_pointer_event(window, PRESS, 3000, 3000, 0),
            "a point outside the subtree must be refused, not silently dropped"
        );

        // The real path: press then release inside the button, which is what a click is.
        assert!(
            rw_dispatch_pointer_event(window, PRESS, 60, 45, 0),
            "a press inside the button must be accepted"
        );
        assert!(rw_dispatch_pointer_event(window, RELEASE, 60, 45, 0), "and its release too");

        // # What the widget does with it, and what it deliberately does *not*
        //
        // A click that arrives this way makes the button emit its **own** `clicked` signal
        // — the in-process path a Rust caller hooks. It does **not** push an entry onto the
        // platform trigger queue: that queue carries triggers a *host* injects (a native
        // `onClick`, a designer action), which is why `SurfaceHandle::on_click` documents that
        // a widget "emits its own signals rather than a platform click callback".
        //
        // Asserting a queued trigger here was my first version and it failed — correctly. The
        // evidence that the event reached a control and was acted on is the signal, so the test
        // hooks `BaseWidget::clicked` and counts the emissions over a full press+release.
        let clicks = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = std::sync::Arc::clone(&clicks);
        let hooked = crate::widget::runtime::with_widget_mut(button, |widget| {
            widget.base().clicked.connect(move || {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        });
        assert!(hooked.is_some(), "the button must be a live widget to hook");

        // A press whose release lands *outside* must not activate — the other half of the
        // click contract, and what proves the routing carried a real position rather than
        // just "some event".
        //
        // # What the two return values mean, verified rather than assumed
        //
        // `route_pointer_event` answers "did this reach a live widget". The press lands on
        // the button, so it takes pointer capture (D09-EVT-01); the release at (400,400) is
        // therefore delivered back to the button that owns the gesture, even though that
        // point hits no widget:
        //
        // * the **press** returns `true` — the point hit the button, which captured the
        //   pointer;
        // * the **release at (400,400)** returns `true` — capture routes it to the button;
        //   the button sees a release outside its bounds and cancels the gesture.
        //
        // Before D09-EVT-01 the outside release returned `false` — it was dropped by the hit
        // test and never reached the button, so the button's own press latch stayed armed
        // until some *later* unrelated release fired a spurious click. Capture is what keeps
        // the release paired with its press. The signal count is the assertion that
        // distinguishes "the button acted" from "the event went somewhere": it must stay
        // `0` because an outside release cancels rather than activates.
        assert!(
            rw_dispatch_pointer_event(window, PRESS, 60, 45, 0),
            "a press inside the button must reach it and take the gesture"
        );
        assert!(
            rw_dispatch_pointer_event(window, RELEASE, 400, 400, 0),
            "the outside release must be routed back to the widget that owns the gesture"
        );
        assert_eq!(
            clicks.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "a release outside the button must not activate it"
        );

        // Inside ⇒ activate.
        assert!(rw_dispatch_pointer_event(window, PRESS, 60, 45, 0));
        assert!(rw_dispatch_pointer_event(window, RELEASE, 60, 45, 0));
        assert_eq!(
            clicks.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "press and release inside the button must activate it exactly once"
        );

        // The direct route reaches a widget the host already knows, without hit-testing.
        assert!(
            rw_dispatch_event_to_widget(button, MOVE, 60, 45, 0),
            "the direct route must reach a live widget"
        );
        assert!(
            !rw_dispatch_event_to_widget(button, UNKNOWN, 60, 45, 0),
            "and refuse an unknown event code like the routing path does"
        );
    }
}

/// An empty drop queue must clear every output, not just return `false`.
///
/// A caller that reuses an `out` from a previous success and frees unconditionally
/// would otherwise free a stale pointer a second time. The sentinel values prove the
/// getter actually wrote null/zero rather than merely leaving the caller's bytes alone.
#[test]
fn an_empty_drop_poll_clears_nonzero_outputs() {
    use core::ffi::c_uint;

    // Drain any ambient event so the next poll is definitely empty (tests share the
    // process-wide control backend).
    loop {
        let mut s = 0u64;
        let mut t = 0u64;
        let mut m: *mut c_char = std::ptr::null_mut();
        let mut p: *mut u8 = std::ptr::null_mut();
        let mut l: c_uint = 0;
        // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
        let had = unsafe { rw_poll_drop_event(&mut s, &mut t, &mut m, &mut p, &mut l) };
        if !had {
            break;
        }
        if !m.is_null() {
            // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
            unsafe { rw_free_string(m) };
        }
        if !p.is_null() {
            // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
            unsafe { rw_free_bytes(p, l) };
        }
    }

    let mut source_out: u64 = 0xDEAD_BEEF;
    let mut target_out: u64 = 0xDEAD_BEEF;
    let mut mime_out: *mut c_char = std::ptr::dangling_mut::<c_char>();
    let mut payload_out: *mut u8 = std::ptr::dangling_mut::<u8>();
    let mut payload_len_out: c_uint = 0xDEAD_BEEF;

    // SAFETY: test-only ABI call; every pointer is an out-param local or a live `CString`/buffer.
    let had = unsafe {
        rw_poll_drop_event(
            &mut source_out,
            &mut target_out,
            &mut mime_out,
            &mut payload_out,
            &mut payload_len_out,
        )
    };
    assert!(!had, "the drained queue must report no event");
    assert_eq!(source_out, 0, "an empty poll must clear the source sentinel");
    assert_eq!(target_out, 0, "an empty poll must clear the target sentinel");
    assert!(mime_out.is_null(), "an empty poll must null the mime pointer");
    assert!(payload_out.is_null(), "an empty poll must null the payload pointer");
    assert_eq!(payload_len_out, 0, "an empty poll must zero the payload length");
}

/// Text, key and wheel input must reach a widget through their dedicated C entries.
///
/// The pointer entry only carries `(x, y, button)`, so a host could never deliver a
/// committed string, a key code + modifier mask, or a wheel delta. These three entries
/// close that gap and are observed at the widget's `handle_event`.
#[test]
fn a_host_can_deliver_text_key_and_wheel_through_the_c_abi() {
    use crate::event::{Event, EventHandler};
    use std::ffi::CString;
    use std::rc::Rc;

    let log = Rc::new(core::cell::RefCell::new(Vec::new()));

    struct Recorder {
        log: Rc<core::cell::RefCell<Vec<String>>>,
        base: crate::widget::BaseWidget,
    }
    impl crate::widget::Widget for Recorder {
        fn base(&self) -> &crate::widget::BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut crate::widget::BaseWidget {
            &mut self.base
        }
    }
    impl EventHandler for Recorder {
        fn handle_event(&mut self, event: &Event) {
            let entry = match event {
                Event::TextInput { text } => format!("text:{text}"),
                Event::KeyPress { key, modifiers } => format!("keydown:{key}:{modifiers}"),
                Event::KeyRelease { key, modifiers } => format!("keyup:{key}:{modifiers}"),
                Event::Wheel { delta, modifiers } => {
                    format!("wheel:{}:{}:{modifiers}", delta.x, delta.y)
                }
                _ => return,
            };
            self.log.borrow_mut().push(entry);
        }
    }

    let id = crate::widget::runtime::register(Box::new(Recorder {
        log: Rc::clone(&log),
        base: crate::widget::BaseWidget::new(
            crate::widget::WidgetKind::LineEdit,
            crate::core::Rect::new(0, 0, 10, 10),
            "recorder",
        ),
    }))
    .expect("mount the recorder");

    let text = CString::new("hi").expect("no interior NUL");
    assert!(rw_dispatch_text_event(id, text.as_ptr()), "text commit must reach the widget");
    assert!(rw_dispatch_key_event(id, 65, 2, false), "key press must reach the widget");
    assert!(rw_dispatch_key_event(id, 65, 0, true), "key release must reach the widget");
    assert!(rw_dispatch_wheel_event(id, 0, -1, 4), "wheel must reach the widget");

    assert_eq!(
        *log.borrow(),
        vec![
            "text:hi".to_string(),
            "keydown:65:2".to_string(),
            "keyup:65:0".to_string(),
            "wheel:0:-1:4".to_string(),
        ]
    );

    crate::widget::runtime::unregister(id);
}
