// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the form suite.
//!
//! The suite's completeness claim is tested on five axes, one per contract a designer or host
//! relies on:
//!
//! 1. **registration** — both names exist, aliases resolve, a second install is refused;
//! 2. **properties** — every writable schema round-trips, and every read-only one refuses a write;
//! 3. **events** — every published name resolves to a real signal, and representative events fire
//!    with the payload their schema declares;
//! 4. **commands** — the payload-free action channel reaches the control;
//! 5. **layout** — labels, controls and error lines occupy the boxes the contract says they do.
//!
//! The value/validation tests mount a real parent control, because a form's value lives in the
//! child's property contract and a fake id would test nothing.

use super::capability::{
    default_property_value, install, published_event_names, FORM_PROPERTIES, FORM_SET_PROPERTIES,
};
use super::form::{Form, FormLabelPosition};
use super::formset::{FormSet, FormSetHeaderStyle};
use super::model::{FormField, FormPage};
use rust_widgets::core::{ObjectId, Rect};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::style::SemanticState;
use rust_widgets::widget::capability::types::CapabilityValue;
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::runtime;
use rust_widgets::widget::SimpleRegistry;
use rust_widgets::widget::{Widget, WidgetProperties};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

fn factory() -> WidgetFactory {
    let mut factory = WidgetFactory::new_with_defaults();
    install(&mut factory).expect("the form group installs into a fresh factory");
    factory
}

/// Mounts a parent control and returns its registry id.
fn mount(factory: &WidgetFactory, kind: &str, text: &str) -> ObjectId {
    let widget = factory
        .create(kind, Rect::new(0, 0, 240, 28), text)
        .unwrap_or_else(|| panic!("the parent must still register `{kind}`"));
    runtime::register(widget).unwrap_or_else(|| panic!("`{kind}` must register on this thread"))
}

fn counter() -> (Arc<Mutex<usize>>, Arc<Mutex<Option<CapabilityValue>>>) {
    (Arc::new(Mutex::new(0)), Arc::new(Mutex::new(None)))
}

// ── 1. Registration ─────────────────────────────────────────────────────────

#[test]
fn install_registers_both_controls_and_their_aliases() {
    let factory = factory();
    for name in ["form", "form_panel", "form_layout", "form_set", "formset", "form_pages"] {
        assert!(
            factory.create(name, Rect::new(0, 0, 320, 240), "").is_some(),
            "`{name}` must resolve after install"
        );
    }
}

#[test]
fn a_second_install_is_refused_rather_than_shadowing_the_first() {
    let mut factory = factory();
    assert!(install(&mut factory).is_err(), "the names are already claimed");
}

#[test]
fn the_capability_lists_the_names_the_controls_publish() {
    let factory = factory();
    let form = factory.create("form", Rect::new(0, 0, 320, 240), "").unwrap();
    let mut set = factory.create("form_set", Rect::new(0, 0, 480, 360), "").unwrap();

    let form_names: Vec<&str> = form.properties_dyn().unwrap().property_names().to_vec();
    let declared: Vec<&str> = FORM_PROPERTIES.iter().map(|schema| schema.name).collect();
    assert_eq!(form_names, declared, "the form schema and the property contract must agree");

    let set_names: Vec<&str> = set.properties_dyn().unwrap().property_names().to_vec();
    let declared: Vec<&str> = FORM_SET_PROPERTIES.iter().map(|schema| schema.name).collect();
    assert_eq!(set_names, declared, "the form-set schema and the property contract must agree");

    // Touch the mutable form so the compiler cannot elide the binding.
    assert!(form.properties_dyn().is_some());
    assert!(set.properties_dyn_mut().is_some());
}

// ── 2. Properties ───────────────────────────────────────────────────────────

#[test]
fn every_writable_form_property_round_trips_through_the_factory() {
    let factory = factory();
    let mut form = factory.create("form", Rect::new(0, 0, 360, 320), "").unwrap();
    for schema in FORM_PROPERTIES {
        if !schema.writable {
            continue;
        }
        let reported = factory
            .read_property(form.as_ref(), schema.name)
            .unwrap_or_else(|error| panic!("form::{} must be readable: {error:?}", schema.name));
        factory.write_property(form.as_mut(), schema.name, reported.clone()).unwrap_or_else(
            |error| panic!("form::{} must accept its own value: {error:?}", schema.name),
        );
        assert_eq!(
            factory.read_property(form.as_ref(), schema.name),
            Ok(reported),
            "form::{} must round-trip",
            schema.name
        );
    }
}

#[test]
fn every_writable_form_set_property_round_trips_through_its_contract() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(set.add_page(FormPage::new("first", "First")));
    assert!(set.add_page(FormPage::new("second", "Second")));
    for schema in FORM_SET_PROPERTIES {
        if !schema.writable {
            continue;
        }
        let reported = set.get(schema.name).unwrap_or_else(|error| {
            panic!("form_set::{} must be readable: {error:?}", schema.name)
        });
        set.set(schema.name, reported.clone()).unwrap_or_else(|error| {
            panic!("form_set::{} must accept its own value: {error:?}", schema.name)
        });
        assert_eq!(set.get(schema.name), Ok(reported), "form_set::{} must round-trip", schema.name);
    }
}

#[test]
fn the_derived_properties_are_read_only() {
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    for name in ["field_count", "valid", "error_count", "dirty"] {
        assert!(form.set(name, CapabilityValue::Bool(true)).is_err(), "form::{name}");
    }
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    for name in ["page_count", "valid", "dirty", "completed_count", "current_key"] {
        assert!(set.set(name, CapabilityValue::Bool(true)).is_err(), "form_set::{name}");
    }
}

#[test]
fn the_reported_defaults_match_the_declared_ones() {
    let factory = factory();
    let form = factory.create("form", Rect::new(0, 0, 320, 240), "").unwrap();
    for schema in FORM_PROPERTIES {
        if let Some(default) = default_property_value("form", schema.name) {
            assert_eq!(
                factory.read_property(form.as_ref(), schema.name),
                Ok(default),
                "form::{} must report its declared default",
                schema.name
            );
        }
    }
    let set = factory.create("form_set", Rect::new(0, 0, 480, 360), "").unwrap();
    for schema in FORM_SET_PROPERTIES {
        if let Some(default) = default_property_value("form_set", schema.name) {
            assert_eq!(
                factory.read_property(set.as_ref(), schema.name),
                Ok(default),
                "form_set::{} must report its declared default",
                schema.name
            );
        }
    }
}

// ── 3. Events and wiring ────────────────────────────────────────────────────

#[test]
fn every_published_event_resolves_to_a_signal() {
    let factory = factory();
    let form = factory.create("form", Rect::new(0, 0, 320, 240), "").unwrap();
    let set = factory.create("form_set", Rect::new(0, 0, 480, 360), "").unwrap();
    let (form_events, set_events) = published_event_names();
    for name in form_events {
        assert!(
            form.event_signal_dyn(name).is_some(),
            "form publishes `{name}` with no signal arm"
        );
    }
    for name in set_events {
        assert!(
            set.event_signal_dyn(name).is_some(),
            "form_set publishes `{name}` with no signal arm"
        );
    }
}

#[test]
fn submit_publishes_submitted_or_submit_failed_with_their_payloads() {
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    let (submitted, _) = counter();
    let failed = Arc::new(Mutex::new(usize::MAX));

    let hits = Arc::clone(&submitted);
    form.event_signal_dyn("submitted")
        .unwrap()
        .subscribe(Box::new(move |_| *hits.lock().unwrap() += 1));
    let payload = Arc::clone(&failed);
    form.event_signal_dyn("submit_failed").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::UInt(count) = value {
            *payload.lock().unwrap() = *count as usize;
        }
    }));

    // An empty form that requires nothing is valid.
    assert!(form.submit());
    assert_eq!(*submitted.lock().unwrap(), 1);

    // A required field with no control can never be satisfied.
    assert!(form.add_field(FormField::new("name", "Name").with_required(true)));
    assert!(!form.submit());
    assert_eq!(*failed.lock().unwrap(), 1, "submit_failed carries the error count");
    assert_eq!(*submitted.lock().unwrap(), 1, "a refused submit emits no `submitted`");
}

#[test]
fn field_lifecycle_and_validity_events_fire() {
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    let added = Arc::new(Mutex::new(None::<String>));
    let removed = Arc::new(Mutex::new(None::<String>));
    let validity = Arc::new(Mutex::new(None::<bool>));

    let sink = Arc::clone(&added);
    form.event_signal_dyn("field_added").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::String(key) = value {
            *sink.lock().unwrap() = Some(key.clone());
        }
    }));
    let sink = Arc::clone(&removed);
    form.event_signal_dyn("field_removed").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::String(key) = value {
            *sink.lock().unwrap() = Some(key.clone());
        }
    }));
    let sink = Arc::clone(&validity);
    form.event_signal_dyn("validity_changed").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::Bool(valid) = value {
            *sink.lock().unwrap() = Some(*valid);
        }
    }));

    form.add_field(FormField::new("a", "A").with_required(true));
    assert_eq!(added.lock().unwrap().as_deref(), Some("a"));
    assert!(!form.validate(), "a required field with no control cannot validate");
    assert_eq!(*validity.lock().unwrap(), Some(false));
    form.clear_errors();
    assert_eq!(
        *validity.lock().unwrap(),
        Some(false),
        "clearing the error does not satisfy the required rule"
    );
    form.set_field_required("a", false);
    assert_eq!(*validity.lock().unwrap(), Some(true), "dropping the rule restores validity");

    assert!(form.remove_field("a").is_some());
    assert_eq!(removed.lock().unwrap().as_deref(), Some("a"));
}

#[test]
fn form_set_events_carry_their_declared_payloads() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    let page_added = Arc::new(Mutex::new(None::<String>));
    let current = Arc::new(Mutex::new(usize::MAX));
    let completed = Arc::new(Mutex::new(None::<CapabilityValue>));

    let sink = Arc::clone(&page_added);
    set.event_signal_dyn("page_added").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::String(key) = value {
            *sink.lock().unwrap() = Some(key.clone());
        }
    }));
    let sink = Arc::clone(&current);
    set.event_signal_dyn("current_changed").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::UInt(index) = value {
            *sink.lock().unwrap() = *index as usize;
        }
    }));
    let sink = Arc::clone(&completed);
    set.event_signal_dyn("completed_changed").unwrap().subscribe(Box::new(move |value| {
        *sink.lock().unwrap() = Some(value.clone());
    }));

    assert!(set.add_page(FormPage::new("one", "One")));
    assert!(set.add_page(FormPage::new("two", "Two")));
    assert_eq!(page_added.lock().unwrap().as_deref(), Some("two"));
    assert!(set.set_current_index(1));
    assert_eq!(*current.lock().unwrap(), 1);
    assert!(set.set_page_completed("one", true));
    assert_eq!(
        *completed.lock().unwrap(),
        Some(CapabilityValue::Tuple(rust_widgets::compat::Vec::from([
            CapabilityValue::String("one".to_string()),
            CapabilityValue::Bool(true),
        ])))
    );
}

// ── 4. Commands ─────────────────────────────────────────────────────────────

#[test]
fn the_command_channel_reaches_both_controls() {
    let factory = factory();
    let mut form = factory.create("form", Rect::new(0, 0, 320, 240), "").unwrap();
    for command in ["validate", "clear_errors", "reset", "submit"] {
        assert!(factory.invoke_command(form.as_mut(), command).is_ok(), "form::{command}");
    }
    assert!(factory.invoke_command(form.as_mut(), "no_such_command").is_err());

    let mut set = factory.create("form_set", Rect::new(0, 0, 480, 360), "").unwrap();
    for command in ["next", "previous", "validate_all", "reset_all", "mark_completed", "submit"] {
        assert!(factory.invoke_command(set.as_mut(), command).is_ok(), "form_set::{command}");
    }
}

// ── 5. Layout ───────────────────────────────────────────────────────────────

#[test]
fn fields_pack_into_columns_with_label_and_error_boxes() {
    let mut form = Form::new(Rect::new(10, 20, 420, 300));
    form.set_title("Account");
    form.set_columns(2);
    form.add_field(FormField::new("first", "First name").with_required(true));
    form.add_field(FormField::new("last", "Last name"));
    form.add_field(FormField::new("email", "Email").with_help("Never shared"));
    form.set_field_error("last", "Too short");

    let layouts = form.field_layouts();
    assert_eq!(layouts.len(), 3);

    // Row-major packing: two in the first row, the third starts the second.
    assert_eq!(layouts[0].control.y, layouts[1].control.y);
    assert!(layouts[2].control.y > layouts[0].control.y);
    // Columns do not overlap, and every box is non-empty.
    assert!(layouts[0].control.right() <= layouts[1].control.x);
    for layout in &layouts {
        assert!(layout.control.width > 0 && layout.control.height > 0);
        // A left label sits beside its control, never over it.
        assert!(layout.label.right() <= layout.control.x);
        // The error/help line is below the control it describes.
        assert!(layout.error.y >= layout.control.bottom());
        // Nothing escapes the form's own rectangle.
        assert!(layout.control.right() <= form.geometry().right());
        assert!(layout.error.bottom() <= form.geometry().bottom());
    }
    // The field with an actual error reserves a line; the others reserve one for their help or
    // none at all.
    assert_eq!(layouts[1].error.height, 16);
    assert_eq!(layouts[0].error.height, 0);
    assert_eq!(layouts[2].error.height, 16);
}

#[test]
fn the_top_label_position_stacks_the_label_above_the_control() {
    let mut form = Form::new(Rect::new(0, 0, 360, 260));
    form.set_label_position(FormLabelPosition::Top);
    form.add_field(FormField::new("name", "Name"));
    let layout = &form.field_layouts()[0];
    assert_eq!(layout.control.y, layout.label.y + layout.label.height as i32);
    assert_eq!(layout.label.width, layout.control.width);
    assert_eq!(layout.label.height, 18);
}

#[test]
fn the_action_row_sits_inside_the_form_and_to_the_right() {
    let form = Form::new(Rect::new(0, 0, 420, 300));
    let submit = form.submit_rect();
    let reset = form.reset_rect();
    assert!(submit.right() <= form.geometry().right());
    assert!(reset.right() < submit.x, "reset sits to the left of submit");
    assert!(submit.bottom() <= form.geometry().bottom());
    assert_eq!(submit.height, 28);
    // Hiding the row gives the content its height back.
    let without = form.content_rect();
    let mut compact = Form::new(Rect::new(0, 0, 420, 300));
    compact.set_show_buttons(false);
    assert!(compact.content_rect().height > without.height);
}

#[test]
fn the_form_set_header_and_content_bands_share_one_layout() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    set.set_header_style(FormSetHeaderStyle::Steps);
    for (key, title) in [("a", "Account"), ("b", "Shipping"), ("c", "Payment")] {
        assert!(set.add_page(FormPage::new(key, title)));
    }

    let header = set.header_rect();
    let first = set.header_item_rect(0).unwrap();
    let second = set.header_item_rect(1).unwrap();
    let third = set.header_item_rect(2).unwrap();
    assert_eq!(first.y, header.y);
    assert_eq!(first.height, header.height);
    assert_eq!(first.right(), second.x);
    assert_eq!(second.right(), third.x);
    assert!(set.header_item_rect(3).is_none(), "there is no fourth page");

    let content = set.content_rect();
    assert!(content.y > header.bottom(), "content starts below the header band");
    assert!(content.bottom() <= set.geometry().bottom());
    assert!(content.width > 0 && content.height > 0);

    // The navigation buttons are inside the form and do not overlap.
    let submit = set.submit_rect();
    let next = set.next_rect();
    let previous = set.previous_rect();
    assert!(submit.right() <= set.geometry().right());
    assert!(next.right() < submit.x);
    assert!(previous.right() < next.x);
}

#[test]
fn page_navigation_wraps_and_respects_disabled_pages() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    set.add_page(FormPage::new("a", "A"));
    set.add_page(FormPage::new("b", "B"));
    set.add_page(FormPage::new("c", "C"));
    set.page_mut("b").unwrap().set_enabled(false);

    assert!(set.next_page());
    assert_eq!(set.current_index(), 2, "a disabled page is skipped");
    assert!(set.next_page());
    assert_eq!(set.current_index(), 0, "navigation wraps");
    assert!(set.previous_page());
    assert_eq!(set.current_index(), 2);
    assert!(!set.set_current_index(1), "a disabled page refuses activation");
    assert!(!set.set_current_index(9), "an out-of-range index is refused, not clamped");
}

// ── Values through the child control's contract ────────────────────────────

#[test]
fn validation_reset_and_dirty_use_the_child_property_contract() {
    let factory = factory();
    let name = mount(&factory, "line_edit", "Ada");
    let email = mount(&factory, "line_edit", "");

    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("name", "Name").with_widget(name).with_required(true)));
    assert!(form.add_field(FormField::new("email", "Email").with_widget(email).with_required(true)));

    assert_eq!(form.value_of("name").as_deref(), Some("Ada"));
    assert_eq!(form.value_of("email").as_deref(), Some(""));
    assert!(!form.validate(), "the empty required field refuses the form");
    assert_eq!(form.error_count(), 1);
    assert_eq!(form.field("email").unwrap().error(), Some("Required"));

    assert!(form.set_value_of("email", "ada@example.com"));
    assert!(form.validate());
    assert!(form.is_valid());
    assert!(form.dirty(), "the email differs from the captured baseline");

    assert!(form.submit(), "a valid form submits");
    form.reset();
    assert_eq!(form.value_of("name").as_deref(), Some("Ada"));
    assert_eq!(form.value_of("email").as_deref(), Some(""), "reset restores the baseline");
    assert!(!form.dirty());
}

#[test]
fn a_form_set_drives_its_forms_through_their_own_commands() {
    let factory = factory();
    let form_id = mount(&factory, "form", "");

    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(set.add_page_form("first", "First", form_id));
    assert!(set.add_page(FormPage::new("second", "Second")));
    assert!(set.is_valid(), "a page without a form never blocks the set");
    assert!(set.validate_all());
    assert!(set.submit());
    set.reset_all();
}

// ── 6. Round-3 behaviour: layout contract, keys, focus, dirty ──────────────

#[test]
fn hiding_labels_reclaims_the_label_column_and_row() {
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    form.add_field(FormField::new("name", "Name"));
    let with_labels = form.field_layouts().remove(0);
    assert!(with_labels.label.width > 0);

    form.set_show_labels(false);
    let without = form.field_layouts().remove(0);
    assert_eq!(without.label.width, 0, "a hidden label must take no column");
    assert_eq!(without.label.height, 0);
    assert_eq!(
        without.control.x, with_labels.label.x,
        "the control starts where the label column started"
    );
    assert_eq!(without.control.width, form.content_rect().width, "and takes the whole row");

    // The top position reclaims its own row too.
    form.set_label_position(FormLabelPosition::Top);
    let stacked = form.field_layouts().remove(0);
    assert_eq!(stacked.label.height, 0);
    assert_eq!(stacked.control.y, stacked.label.y, "no label row is reserved");
}

#[test]
fn hiding_a_field_row_removes_it_from_the_layout() {
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    form.add_field(FormField::new("a", "A"));
    form.add_field(FormField::new("b", "B"));
    assert_eq!(form.field_layouts().len(), 2);
    assert!(form.set_field_visible("b", false));
    let layouts = form.field_layouts();
    assert_eq!(layouts.len(), 1);
    assert_eq!(layouts[0].key, "a");
    assert!(!form.set_field_visible("missing", false));
}

#[test]
fn a_disabled_form_ignores_its_buttons_and_enter() {
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    let submitted = Arc::new(Mutex::new(0usize));
    let hits = Arc::clone(&submitted);
    form.event_signal_dyn("submitted")
        .unwrap()
        .subscribe(Box::new(move |_| *hits.lock().unwrap() += 1));

    form.set("enabled", CapabilityValue::Bool(false)).unwrap();
    let submit = form.submit_rect();
    form.handle_event(&Event::mouse_press(submit.x + 1, submit.y + 1, 1));
    form.handle_event(&Event::KeyPress {
        key: rust_widgets::event::key_codes::ENTER,
        modifiers: 0,
    });
    assert_eq!(*submitted.lock().unwrap(), 0, "a disabled form must not submit");

    form.set("enabled", CapabilityValue::Bool(true)).unwrap();
    form.handle_event(&Event::mouse_press(submit.x + 1, submit.y + 1, 1));
    assert_eq!(*submitted.lock().unwrap(), 1, "the enabled form submits again");
}

#[test]
fn writing_a_value_through_the_form_probes_the_child_contract() {
    let factory = factory();
    // A control with no text/value property cannot accept a write; the form must report the
    // refusal rather than pretend it wrote something.
    let inner = mount(&factory, "form", "");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("inner", "Inner").with_widget(inner)));
    assert!(!form.set_value_of("inner", "value"), "no text/current_text/value, no write");
    assert!(!form.set_value_of("missing", "value"));
}

#[test]
fn validate_field_runs_only_the_named_fields_rule() {
    let factory = factory();
    let name = mount(&factory, "line_edit", "Ada");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("name", "Name").with_widget(name).with_required(true)));
    assert!(form.add_field(FormField::new("email", "Email").with_required(true)));

    assert!(form.validate_field("name"));
    assert_eq!(form.field("name").unwrap().error(), None);
    assert_eq!(
        form.field("email").unwrap().error(),
        None,
        "the other field is not touched by a single-field pass"
    );
    assert!(!form.validate_field("missing"));

    assert!(!form.validate());
    assert!(form.field("email").unwrap().error().is_some());
}

#[test]
fn focus_first_invalid_moves_focus_to_the_first_broken_field() {
    let factory = factory();
    let first = mount(&factory, "line_edit", "");
    let second = mount(&factory, "line_edit", "");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("first", "First").with_widget(first).with_required(true)));
    assert!(
        form.add_field(FormField::new("second", "Second").with_widget(second).with_required(true))
    );

    assert!(!form.focus_first_invalid(), "nothing is in error yet");
    assert!(!form.validate());
    assert!(form.focus_first_invalid());
    assert_eq!(
        runtime::focused_widget(),
        Some(first),
        "focus lands on the first field that is in error"
    );
}

#[test]
fn form_set_dirty_aggregates_its_pages() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "Ada");

    // Dirty the form before it joins the set, so the set's own add is the edge that publishes.
    let mut inner = Form::new(Rect::new(0, 0, 240, 120));
    assert!(inner.add_field(FormField::new("v", "V").with_widget(field)));
    assert!(inner.set_value_of("v", "Grace"));
    assert!(inner.dirty());
    let form_id = runtime::register(Box::new(inner)).expect("the form registers");

    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    let dirty = Arc::new(Mutex::new(None::<bool>));
    let sink = Arc::clone(&dirty);
    set.event_signal_dyn("dirty_changed").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::Bool(flag) = value {
            *sink.lock().unwrap() = Some(*flag);
        }
    }));

    assert!(set.add_page_form("one", "One", form_id));
    assert!(set.dirty(), "a page's form reports uncommitted changes");
    assert_eq!(*dirty.lock().unwrap(), Some(true));

    set.reset_all();
    assert!(!set.dirty());
    assert_eq!(*dirty.lock().unwrap(), Some(false), "reset publishes the clearing edge");
}

#[test]
fn goto_page_activates_by_key_and_shares_the_refusal_rule() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(set.add_page(FormPage::new("a", "A")));
    assert!(set.add_page(FormPage::new("b", "B")));
    set.page_mut("b").unwrap().set_enabled(false);

    assert!(!set.goto_page("b"), "a disabled page refuses activation");
    assert!(!set.goto_page("missing"));
    assert!(set.goto_page("a"));
    assert_eq!(set.current_key(), Some("a"));
}

#[test]
fn removing_a_page_before_the_current_one_keeps_the_current_page() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    for key in ["a", "b", "c"] {
        assert!(set.add_page(FormPage::new(key, key)));
    }
    assert!(set.set_current_index(2));
    assert!(set.remove_page("a").is_some());
    assert_eq!(set.current_key(), Some("c"));
    assert_eq!(set.current_index(), 1);
}

#[test]
fn the_form_set_walks_pages_with_the_arrow_keys_and_submits_with_enter() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(set.add_page(FormPage::new("a", "A")));
    assert!(set.add_page(FormPage::new("b", "B")));

    set.handle_event(&Event::KeyPress { key: rust_widgets::event::key_codes::RIGHT, modifiers: 0 });
    assert_eq!(set.current_index(), 1);
    set.handle_event(&Event::KeyPress { key: rust_widgets::event::key_codes::LEFT, modifiers: 0 });
    assert_eq!(set.current_index(), 0);

    let submitted = Arc::new(Mutex::new(0usize));
    let hits = Arc::clone(&submitted);
    set.event_signal_dyn("submitted")
        .unwrap()
        .subscribe(Box::new(move |_| *hits.lock().unwrap() += 1));
    set.handle_event(&Event::KeyPress { key: rust_widgets::event::key_codes::ENTER, modifiers: 0 });
    assert_eq!(*submitted.lock().unwrap(), 1);
}

#[test]
fn a_form_set_with_no_pages_is_not_focusable_and_announces_its_page() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(!set.is_focusable(), "an empty set has nothing to walk");
    assert!(set.add_page(FormPage::new("a", "Account")));
    assert!(set.is_focusable());
    assert_eq!(set.accessible_name(), "Account");

    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    form.set_title("Account");
    assert_eq!(form.accessible_name(), "Account");
}

#[test]
fn the_form_reports_the_error_meaning_while_a_field_is_in_error() {
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert_eq!(form.semantic_state(), SemanticState::None);
    form.add_field(FormField::new("a", "A").with_required(true));
    assert!(!form.validate());
    assert_eq!(form.semantic_state(), SemanticState::Error);
    form.set_field_required("a", false);
    form.clear_errors();
    assert_eq!(form.semantic_state(), SemanticState::None);
}

#[test]
fn a_direct_child_edit_publishes_its_edges_on_the_next_event() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "Ada");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("name", "Name").with_widget(field)));
    form.set_registry(Rc::new(RefCell::new(SimpleRegistry::new())));

    let changed = Arc::new(Mutex::new(None::<String>));
    let sink = Arc::clone(&changed);
    form.event_signal_dyn("field_changed").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::String(key) = value {
            *sink.lock().unwrap() = Some(key.clone());
        }
    }));
    let dirty = Arc::new(Mutex::new(None::<bool>));
    let sink = Arc::clone(&dirty);
    form.event_signal_dyn("dirty_changed").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::Bool(flag) = value {
            *sink.lock().unwrap() = Some(*flag);
        }
    }));

    // The user edits the control directly: the form is not involved.
    assert!(rust_widgets::write_widget_property_by_id(
        field,
        "text",
        CapabilityValue::String("Grace".into())
    )
    .is_ok());
    assert_eq!(*changed.lock().unwrap(), None, "nothing is published until the form sees an event");

    form.handle_event(&Event::KeyPress { key: 65, modifiers: 0 });
    assert_eq!(changed.lock().unwrap().as_deref(), Some("name"));
    assert_eq!(*dirty.lock().unwrap(), Some(true));
    assert!(form.dirty());
}

#[test]
fn disabling_the_container_disables_the_fields_it_hosts() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("name", "Name").with_widget(field)));

    form.set("enabled", CapabilityValue::Bool(false)).unwrap();
    let enabled =
        rust_widgets::read_widget_property_by_id(field, "enabled").expect("enabled is readable");
    assert_eq!(enabled, CapabilityValue::Bool(false));
    form.set("enabled", CapabilityValue::Bool(true)).unwrap();
    let enabled =
        rust_widgets::read_widget_property_by_id(field, "enabled").expect("enabled is readable");
    assert_eq!(enabled, CapabilityValue::Bool(true));

    // A form set forwards its own flag to the forms it pages.
    let inner = mount(&factory, "form", "");
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(set.add_page_form("one", "One", inner));
    set.set("enabled", CapabilityValue::Bool(false)).unwrap();
    let enabled =
        rust_widgets::read_widget_property_by_id(inner, "enabled").expect("enabled is readable");
    assert_eq!(enabled, CapabilityValue::Bool(false));
}

#[test]
fn a_host_rule_runs_after_the_required_check() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "abc");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("code", "Code").with_widget(field).with_required(true)));
    assert!(form.set_field_validator("code", |value| {
        if value == "abc" {
            Some("Not allowed".to_string())
        } else {
            None
        }
    }));
    assert!(!form.set_field_validator("missing", |_| None), "an unknown key is refused");

    assert!(!form.validate());
    assert_eq!(form.field("code").unwrap().error(), Some("Not allowed"));
    assert!(form.set_value_of("code", "xyz"));
    assert!(form.validate());
    assert!(form.field("code").unwrap().error().is_none());

    assert!(form.clear_field_validator("code"));
    assert!(!form.clear_field_validator("code"));
}

#[test]
fn move_page_reorders_and_keeps_the_active_page() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    for key in ["a", "b", "c"] {
        assert!(set.add_page(FormPage::new(key, key)));
    }
    assert!(set.set_current_index(1));
    assert!(!set.move_page("missing", 0));
    assert!(!set.move_page("c", 3), "an out-of-range index is refused");
    assert!(set.move_page("c", 0));
    assert_eq!(set.page_keys().iter().map(String::as_str).collect::<Vec<_>>(), vec!["c", "a", "b"]);
    assert_eq!(set.current_key(), Some("b"), "the active page keeps its identity");
    assert_eq!(set.current_index(), 2);
    assert!(set.move_page("b", 2), "moving to its current position succeeds");
}

#[test]
fn overflows_reports_a_form_that_needs_more_height() {
    let mut form = Form::new(Rect::new(0, 0, 320, 60));
    assert!(!form.overflows(), "an empty form fits");
    for index in 0..6 {
        assert!(form.add_field(FormField::new(format!("f{index}"), "Field")));
    }
    assert!(form.overflows(), "six rows need more than 60 px");
    form.set_geometry(Rect::new(0, 0, 320, 1000));
    assert!(!form.overflows());
}

#[test]
fn the_layout_cache_follows_every_input() {
    let mut form = Form::new(Rect::new(0, 0, 320, 400));
    form.add_field(FormField::new("a", "A"));
    form.add_field(FormField::new("b", "B"));
    let before = form.field_layouts();
    let gap = before[1].control.y - before[0].control.y;
    assert_eq!(before, form.field_layouts(), "a cache hit returns the same boxes");

    form.set_row_spacing(40);
    let spaced = form.field_layouts();
    assert!(spaced[1].control.y - spaced[0].control.y > gap, "spacing invalidates the cache");

    form.set_columns(2);
    let columns = form.field_layouts();
    assert_eq!(columns[0].control.y, columns[1].control.y, "columns re-pack the row");

    form.set_geometry(Rect::new(100, 50, 320, 400));
    let moved = form.field_layouts();
    assert_eq!(moved[0].label.x, form.content_rect().x, "the row moved with the geometry");
    assert_eq!(form.content_rect().x, 112, "the padding moved with the geometry");
}

#[test]
fn the_factory_answers_the_schema_defaults_it_was_taught() {
    let factory = factory();
    assert_eq!(
        factory.default_property_value("form", "title").expect("the package declared a default"),
        CapabilityValue::String(String::new())
    );
    assert_eq!(
        factory
            .default_property_value("form_set", "navigation_height")
            .expect("the package declared a default"),
        CapabilityValue::UInt(36)
    );
    assert!(factory.default_property_value("form", "no_such_property").is_err());
}
