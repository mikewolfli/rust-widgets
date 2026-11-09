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
use super::model::{FieldKind, FormField, FormPage};
use super::rules::FormRule;
use rust_widgets::core::{ObjectId, Point, Rect};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::style::SemanticState;
use rust_widgets::widget::capability::types::CapabilityValue;
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::input_widgets::input_validator::RangeValidator;
use rust_widgets::widget::runtime;
use rust_widgets::widget::{Widget, WidgetProperties};
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
    set.handle_event(&Event::FocusGained { reason: rust_widgets::event::FocusReason::Tab });

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

// ── RW-25: cross-field rules ────────────────────────────────────────────────

#[test]
fn a_same_as_rule_reports_on_the_judged_field() {
    let factory = factory();
    let password = mount(&factory, "line_edit", "secret");
    let confirm = mount(&factory, "line_edit", "typo");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("password", "Password").with_widget(password)));
    assert!(form.add_field(FormField::new("confirm", "Confirm").with_widget(confirm)));

    assert!(form.set_form_rule("confirm", FormRule::same_as("password")));
    assert_eq!(form.form_rules("confirm").len(), 1);
    assert!(!form.validate());
    assert_eq!(form.field("confirm").unwrap().error(), Some("Must match password"));
    assert_eq!(form.field("password").unwrap().error(), None, "only the judged row errors");

    assert!(form.set_value_of("confirm", "secret"));
    assert!(form.validate(), "matching values satisfy the rule");

    assert!(form.remove_form_rule("confirm"));
    assert!(!form.remove_form_rule("confirm"), "the rule is gone after one removal");
}

#[test]
fn different_from_and_required_when_rules_run_from_validate() {
    let factory = factory();
    let first = mount(&factory, "line_edit", "a");
    let second = mount(&factory, "line_edit", "a");
    let country = mount(&factory, "line_edit", "DE");
    let vat = mount(&factory, "line_edit", "");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("first", "First").with_widget(first)));
    assert!(form.add_field(FormField::new("second", "Second").with_widget(second)));
    assert!(form.add_field(FormField::new("country", "Country").with_widget(country)));
    assert!(form.add_field(FormField::new("vat", "VAT").with_widget(vat)));

    assert!(form.set_form_rule(
        "second",
        FormRule::different_from("first").with_message("Pick another value")
    ));
    assert!(form.set_form_rule("vat", FormRule::required_when("country", ["DE", "FR"])));

    assert!(!form.validate());
    assert_eq!(form.field("second").unwrap().error(), Some("Pick another value"));
    assert_eq!(form.field("vat").unwrap().error(), Some("Required when country is set"));

    assert!(form.set_value_of("second", "b"));
    assert!(form.set_value_of("vat", "DE123"));
    assert!(form.validate());

    // The trigger value decides whether the requirement is active at all.
    assert!(form.set_value_of("country", "US"));
    assert!(form.set_value_of("vat", ""));
    assert!(form.validate());
}

// ── RW-26: asynchronous validation tickets ──────────────────────────────────

#[test]
fn a_validation_ticket_resolves_once_and_emits_its_verdict() {
    let factory = factory();
    let email = mount(&factory, "line_edit", "not-an-address");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("email", "Email").with_widget(email)));

    let seen = Arc::new(Mutex::new(Vec::<(String, bool)>::new()));
    let sink = Arc::clone(&seen);
    form.field_validated.connect_ref(move |(key, valid)| {
        sink.lock().unwrap().push((key.clone(), *valid));
    });

    let mut ticket = form.set_field_async_validator("email", |value| {
        if value.contains('@') {
            None
        } else {
            Some("Invalid address".to_string())
        }
    });
    assert!(!ticket.is_stale(&form));
    assert!(!ticket.is_resolved());
    assert!(form.resolve_async(&mut ticket));
    assert!(ticket.is_resolved());
    assert_eq!(form.field("email").unwrap().error(), Some("Invalid address"));
    assert_eq!(*seen.lock().unwrap(), vec![("email".to_string(), false)]);
    assert!(!form.resolve(&mut ticket, None), "a resolved ticket cannot resolve twice");

    assert!(form.set_value_of("email", "ada@example.com"));
    assert!(form.validate());
}

#[test]
fn a_ticket_goes_stale_when_the_value_changes_or_it_is_cancelled() {
    let factory = factory();
    let email = mount(&factory, "line_edit", "ada@example.com");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("email", "Email").with_widget(email)));

    let mut ticket = form.set_field_async_validator("email", |_| None);
    assert!(form.set_value_of("email", "grace@example.com"));
    assert!(ticket.is_stale(&form));
    assert!(!form.resolve(&mut ticket, None));

    let mut cancelled = form.set_field_async_validator("email", |_| None);
    assert!(cancelled.cancel());
    assert!(cancelled.is_stale(&form));
    assert!(!cancelled.cancel(), "a second cancel is a no-op");
    assert!(!form.resolve(&mut cancelled, None));

    let unknown = form.set_field_async_validator("missing", |_| None);
    assert!(unknown.is_cancelled(), "an unknown key issues a cancelled ticket");
}

// ── RW-27: typed validators bridged from the input widgets ─────────────────

#[test]
fn a_declared_field_kind_installs_the_shared_input_rule() {
    let factory = factory();
    let email = mount(&factory, "line_edit", "not-an-address");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(
        FormField::new("email", "Email").with_widget(email).with_kind(FieldKind::Email)
    ));
    assert!(!form.validate());
    assert!(form.field("email").unwrap().error().is_some());
    assert!(form.set_value_of("email", "ada@example.com"));
    assert!(form.validate());

    assert!(form.set_field_kind("email", FieldKind::Text));
    assert!(!form.set_field_kind("missing", FieldKind::Text));
}

#[test]
fn a_typed_input_validator_replaces_the_string_closure_channel() {
    let factory = factory();
    let age = mount(&factory, "line_edit", "200");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("age", "Age").with_widget(age)));

    assert!(form.set_field_input_validator("age", RangeValidator::int(0, 130)));
    assert!(!form.validate());
    assert_eq!(form.field("age").unwrap().error(), Some("200 is outside 0..=130"));

    assert!(form.set_value_of("age", "30"));
    assert!(form.validate());

    assert!(form.clear_field_input_validator("age"));
    assert!(!form.clear_field_input_validator("age"));
    assert!(form.set_value_of("age", "999"));
    assert!(form.validate(), "without the typed rule the host closure is the only judge");
    assert!(!form.set_field_input_validator("missing", RangeValidator::int(0, 1)));
}

#[test]
fn a_length_kind_judges_characters_not_bytes() {
    let factory = factory();
    let name = mount(&factory, "line_edit", "日本語");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(
        FormField::new("name", "Name")
            .with_widget(name)
            .with_kind(FieldKind::Length { minimum: 1, maximum: 2 })
    ));
    assert!(!form.validate(), "three characters exceed the two-character bound");
    assert!(form.set_value_of("name", "日本"));
    assert!(form.validate());
}

// ── RW-28: cancel is not reset ─────────────────────────────────────────────

#[test]
fn cancel_clears_verdicts_without_restoring_the_baseline() {
    let factory = factory();
    let name = mount(&factory, "line_edit", "Ada");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("name", "Name").with_widget(name).with_required(true)));

    let cancels = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&cancels);
    form.cancel_requested.connect(move || *sink.lock().unwrap() += 1);

    assert!(form.set_value_of("name", ""));
    assert!(!form.validate());
    assert!(form.field("name").unwrap().error().is_some());

    form.cancel();
    assert_eq!(*cancels.lock().unwrap(), 1);
    assert_eq!(form.value_of("name").as_deref(), Some(""), "cancel keeps the edit");
    assert_eq!(form.field("name").unwrap().error(), None, "cancel clears the verdict");

    form.reset();
    assert_eq!(
        form.value_of("name").as_deref(),
        Some("Ada"),
        "reset is the command that restores the baseline"
    );

    assert_eq!(form.cancel_label(), "");
    assert!(form.cancel_rect().is_none(), "an empty caption hides the cancel button");
    form.set_cancel_label("Cancel");
    assert_eq!(form.cancel_label(), "Cancel");
    assert!(form.cancel_rect().is_some());
}

// ── RW-29: removal, adoption and repaint semantics ──────────────────────────

#[test]
fn re_adding_a_removed_field_restores_its_control() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "Ada");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("name", "Name").with_widget(field)));

    assert!(form.remove_field("name").is_some());
    assert_eq!(
        rust_widgets::read_widget_property_by_id(field, "visible").expect("visible is readable"),
        CapabilityValue::Bool(false),
        "a detached control stops claiming to be visible"
    );

    assert!(form.add_field(FormField::new("name", "Name").with_widget(field)));
    assert_eq!(
        rust_widgets::read_widget_property_by_id(field, "visible").expect("visible is readable"),
        CapabilityValue::Bool(true),
        "re-adoption restores the control the layout paints"
    );
}

#[test]
fn a_field_added_to_a_disabled_form_starts_disabled() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    form.set("enabled", CapabilityValue::Bool(false)).unwrap();

    assert!(form.add_field(FormField::new("name", "Name").with_widget(field)));
    assert_eq!(
        rust_widgets::read_widget_property_by_id(field, "enabled").expect("enabled is readable"),
        CapabilityValue::Bool(false),
        "a hosted field follows the container's enabled state"
    );
}

#[test]
fn removing_a_field_drops_its_host_rule() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "abc");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("code", "Code").with_widget(field)));
    assert!(form.set_field_validator("code", |value| {
        if value == "abc" {
            Some("Not allowed".to_string())
        } else {
            None
        }
    }));
    assert!(!form.validate());

    assert!(form.remove_field("code").is_some());
    assert!(form.add_field(FormField::new("code", "Code").with_widget(field)));
    assert!(
        form.validate(),
        "a rule installed for the removed field must not judge the re-added row"
    );
    assert_eq!(form.field("code").unwrap().error(), None);
}

#[test]
fn clearing_the_fields_drops_their_host_rules() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "abc");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("code", "Code").with_widget(field)));
    assert!(form.set_field_validator("code", |_| Some("Not allowed".to_string())));
    assert!(!form.validate());

    form.clear_fields();
    assert!(form.add_field(FormField::new("code", "Code").with_widget(field)));
    assert!(form.validate(), "`clear_fields` is the whole lifetime of a field's host rules");
}

#[test]
fn validate_repaints_the_error_lines_it_produces() {
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("a", "A").with_required(true)));

    let redraws = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&redraws);
    form.base().redraw_requested.connect(move || *sink.lock().unwrap() += 1);

    let before = *redraws.lock().unwrap();
    assert!(!form.validate());
    assert!(*redraws.lock().unwrap() > before, "the new error line has to be painted");

    let after_full_pass = *redraws.lock().unwrap();
    assert!(form.validate_field("a"));
    assert!(*redraws.lock().unwrap() > after_full_pass, "a single-field pass repaints too");
}

#[test]
fn retiring_an_async_verdict_reclaims_its_error_line() {
    let factory = factory();
    let email = mount(&factory, "line_edit", "bad");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("email", "Email").with_widget(email)));

    let mut ticket = form.set_field_async_validator("email", |_| Some("Invalid".to_string()));
    assert!(form.resolve_async(&mut ticket));
    assert_eq!(form.field_layouts()[0].error.height, 16, "the verdict owns an error line");

    assert!(form.set_value_of("email", "ada@example.com"));
    assert_eq!(
        form.field_layouts()[0].error.height,
        0,
        "a form-driven write retires the verdict and its line"
    );

    assert!(form.set_value_of("email", "bad"));
    let mut ticket = form.set_field_async_validator("email", |_| Some("Invalid".to_string()));
    assert!(form.resolve_async(&mut ticket));
    assert_eq!(form.field_layouts()[0].error.height, 16);
    assert!(rust_widgets::write_widget_property_by_id(
        email,
        "text",
        CapabilityValue::String("grace@example.com".into())
    )
    .is_ok());
    form.handle_event(&Event::KeyPress { key: 65, modifiers: 0 });
    assert_eq!(
        form.field_layouts()[0].error.height,
        0,
        "a direct child edit retires the verdict and its line too"
    );
}

#[test]
fn focus_first_invalid_skips_a_rule_only_row() {
    let factory = factory();
    let bound = mount(&factory, "line_edit", "");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("rule_only", "Rule only").with_required(true)));
    assert!(form.add_field(FormField::new("bound", "Bound").with_widget(bound).with_required(true)));
    assert!(!form.validate());

    assert!(form.focus_first_invalid(), "the bound row is reachable even though the first is not");
    assert_eq!(runtime::focused_widget(), Some(bound));
}

#[test]
fn a_hidden_error_line_does_not_reserve_height_when_help_is_present() {
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("a", "A").with_help("Hint")));
    assert!(form.set_field_error("a", "Boom"));
    assert_eq!(form.field_layouts()[0].error.height, 16, "the error line is reserved");

    form.set_show_errors(false);
    assert_eq!(
        form.field_layouts()[0].error.height,
        0,
        "an error that is not painted and shadows the help holds no blank line"
    );

    assert!(form.clear_field_error("a"));
    assert_eq!(form.field_layouts()[0].error.height, 16, "the help line is still painted");
}

#[test]
fn refresh_applies_a_direct_visibility_edit() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("name", "Name").with_widget(field)));

    form.field_mut("name").expect("the field exists").set_visible(false);
    form.refresh();
    assert!(form.field_layouts().is_empty(), "a hidden row leaves the layout");
    assert_eq!(
        rust_widgets::read_widget_property_by_id(field, "visible").expect("visible is readable"),
        CapabilityValue::Bool(false),
        "refresh reconciles the child's own flag with the field's"
    );
}

#[test]
fn size_hint_does_not_count_the_trailing_row_gap() {
    let mut form = Form::new(Rect::new(0, 0, 320, 400));
    form.set_show_title(false);
    form.set_show_buttons(false);
    form.set_content_padding(12);
    form.set_row_height(32);
    form.set_row_spacing(8);

    assert!(form.add_field(FormField::new("a", "A")));
    assert_eq!(form.size_hint().height, 32 + 24, "one row plus both paddings");

    assert!(form.add_field(FormField::new("b", "B")));
    assert_eq!(form.size_hint().height, 32 + 8 + 32 + 24, "only the gap between rows counts");

    form.set_geometry(Rect::new(0, 0, 320, 96));
    assert!(!form.overflows(), "a form that fits its height must not report an overflow");
}

#[test]
fn disabling_a_container_repaints_it() {
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    let redraws = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&redraws);
    form.base().redraw_requested.connect(move || *sink.lock().unwrap() += 1);
    form.set("enabled", CapabilityValue::Bool(false)).unwrap();
    assert!(*redraws.lock().unwrap() > 0, "a disabled form paints its buttons faded");

    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    let redraws = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&redraws);
    set.base().redraw_requested.connect(move || *sink.lock().unwrap() += 1);
    set.set("enabled", CapabilityValue::Bool(false)).unwrap();
    assert!(*redraws.lock().unwrap() > 0, "a disabled set paints its navigation faded");
}

// ── RW-30: hidden rows are outside the validity aggregate ───────────────────

#[test]
fn a_hidden_required_field_does_not_block_the_form() {
    let factory = factory();
    let name = mount(&factory, "line_edit", "");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));

    let edges = Arc::new(Mutex::new(Vec::<bool>::new()));
    let sink = Arc::clone(&edges);
    form.event_signal_dyn("validity_changed").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::Bool(valid) = value {
            sink.lock().unwrap().push(*valid);
        }
    }));

    assert!(form.add_field(FormField::new("name", "Name").with_widget(name).with_required(true)));
    assert!(!form.validate());
    assert_eq!(form.error_count(), 1);
    assert_eq!(form.semantic_state(), SemanticState::Error);

    assert!(form.set_field_visible("name", false));
    assert!(form.is_valid(), "a hidden row is outside the aggregate");
    assert_eq!(form.error_count(), 0);
    assert_eq!(form.semantic_state(), SemanticState::None);
    assert!(!form.focus_first_invalid(), "a hidden row has no control to focus");
    assert!(form.validate_field("name"), "a hidden field is a no-op pass");
    assert!(form.submit(), "a hidden required row does not block the submit");
    assert_eq!(
        *edges.lock().unwrap(),
        vec![false, true],
        "hiding publishes the validity edge it produced"
    );

    assert!(form.set_field_visible("name", true));
    assert!(!form.is_valid(), "showing the row restores the required rule");
    assert!(!form.submit());
    assert_eq!(edges.lock().unwrap().last(), Some(&false));
}

// ── RW-31: resync publishes the edges a direct write produced ───────────────

#[test]
fn resync_publishes_the_edges_a_direct_write_produced() {
    let factory = factory();
    let name = mount(&factory, "line_edit", "Ada");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("name", "Name").with_widget(name)));

    let changed = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&changed);
    form.event_signal_dyn("field_changed").unwrap().subscribe(Box::new(move |_| {
        *sink.lock().unwrap() += 1;
    }));
    let dirty = Arc::new(Mutex::new(None::<bool>));
    let sink = Arc::clone(&dirty);
    form.event_signal_dyn("dirty_changed").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::Bool(flag) = value {
            *sink.lock().unwrap() = Some(*flag);
        }
    }));

    assert!(rust_widgets::write_widget_property_by_id(
        name,
        "text",
        CapabilityValue::String("Grace".into())
    )
    .is_ok());

    form.refresh();
    assert_eq!(
        *changed.lock().unwrap(),
        0,
        "refresh is the presentation half; it publishes nothing"
    );
    assert!(form.dirty(), "the live predicate already sees the write");

    form.resync();
    assert_eq!(*changed.lock().unwrap(), 1, "resync publishes the field edge");
    assert_eq!(*dirty.lock().unwrap(), Some(true), "and the dirty edge");

    // A second resync is an edge detector: nothing changed, so it emits nothing.
    form.resync();
    assert_eq!(*changed.lock().unwrap(), 1);
}

// ── RW-32: per-button hover ─────────────────────────────────────────────────

#[test]
fn action_hover_follows_the_pointer_and_the_hit_rects() {
    let mut form = Form::new(Rect::new(0, 0, 420, 300));
    let submit = form.submit_rect();
    let reset = form.reset_rect();

    form.handle_event(&Event::MouseMove { pos: Point::new(submit.x + 1, submit.y + 1) });
    assert_eq!(form.hovered_action(), Some("submit"));

    form.handle_event(&Event::MouseMove { pos: Point::new(reset.x + 1, reset.y + 1) });
    assert_eq!(
        form.hovered_action(),
        Some("reset"),
        "only the button under the pointer is hovered"
    );

    form.handle_event(&Event::MouseMove { pos: Point::new(4, 4) });
    assert_eq!(form.hovered_action(), None, "a point in the form body hovers no action");

    form.set_cancel_label("Cancel");
    let cancel = form.cancel_rect().expect("the caption adds the button");
    form.handle_event(&Event::MouseMove { pos: Point::new(cancel.x + 1, cancel.y + 1) });
    assert_eq!(form.hovered_action(), Some("cancel"));

    // The highlighted box is the box a press reaches.
    let cancels = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&cancels);
    form.cancel_requested.connect(move || *sink.lock().unwrap() += 1);
    form.handle_event(&Event::mouse_press(cancel.x + 1, cancel.y + 1, 1));
    assert_eq!(*cancels.lock().unwrap(), 1);

    form.handle_event(&Event::MouseMove { pos: Point::new(submit.x + 1, submit.y + 1) });
    assert_eq!(form.hovered_action(), Some("submit"));
    form.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
    assert_eq!(form.hovered_action(), None, "leaving the form clears the hover");

    form.handle_event(&Event::MouseMove { pos: Point::new(submit.x + 1, submit.y + 1) });
    form.set_show_buttons(false);
    assert_eq!(form.hovered_action(), None, "a hidden action row hovers nothing");
}

#[test]
fn form_set_navigation_hover_is_per_button() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(set.add_page(FormPage::new("a", "A")));
    assert!(set.add_page(FormPage::new("b", "B")));
    let next = set.next_rect();
    let previous = set.previous_rect();
    let submit = set.submit_rect();

    set.handle_event(&Event::MouseMove { pos: Point::new(next.x + 1, next.y + 1) });
    assert_eq!(set.hovered_action(), Some("next"));
    set.handle_event(&Event::MouseMove { pos: Point::new(previous.x + 1, previous.y + 1) });
    assert_eq!(set.hovered_action(), Some("previous"));
    set.handle_event(&Event::MouseMove { pos: Point::new(submit.x + 1, submit.y + 1) });
    assert_eq!(set.hovered_action(), Some("submit"));

    set.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });
    assert_eq!(set.hovered_action(), None);

    set.set_show_navigation(false);
    set.handle_event(&Event::MouseMove { pos: Point::new(next.x + 1, next.y + 1) });
    assert_eq!(set.hovered_action(), None, "a hidden navigation row hovers nothing");
}

// ── RW-33: the trait `set_enabled` propagates like the property ──────────────

#[test]
fn the_trait_set_enabled_propagates_like_the_property() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("name", "Name").with_widget(field)));

    form.set_enabled(false);
    assert_eq!(
        rust_widgets::read_widget_property_by_id(field, "enabled").expect("enabled is readable"),
        CapabilityValue::Bool(false),
        "the trait call reaches the hosted field"
    );
    form.set_enabled(true);
    assert_eq!(
        rust_widgets::read_widget_property_by_id(field, "enabled").expect("enabled is readable"),
        CapabilityValue::Bool(true)
    );

    let inner = mount(&factory, "form", "");
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(set.add_page_form("one", "One", inner));
    set.set_enabled(false);
    assert_eq!(
        rust_widgets::read_widget_property_by_id(inner, "enabled").expect("enabled is readable"),
        CapabilityValue::Bool(false),
        "the trait call reaches the paged form"
    );
    set.set_enabled(true);
    assert_eq!(
        rust_widgets::read_widget_property_by_id(inner, "enabled").expect("enabled is readable"),
        CapabilityValue::Bool(true)
    );
}

// ── RW-34: `current_changed` follows the active page, not only its index ────

fn watch_current_changes(set: &mut FormSet) -> Arc<Mutex<Vec<usize>>> {
    let seen = Arc::new(Mutex::new(Vec::<usize>::new()));
    let sink = Arc::clone(&seen);
    set.event_signal_dyn("current_changed").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::UInt(index) = value {
            sink.lock().unwrap().push(*index as usize);
        }
    }));
    seen
}

#[test]
fn removing_the_active_page_announces_the_page_that_took_its_place() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    for key in ["a", "b", "c"] {
        assert!(set.add_page(FormPage::new(key, key)));
    }
    assert!(set.set_current_index(1));
    let seen = watch_current_changes(&mut set);

    assert!(set.remove_page("b").is_some());
    assert_eq!(set.current_key(), Some("c"));
    assert_eq!(set.current_index(), 1);
    assert_eq!(*seen.lock().unwrap(), vec![1], "the key changed at an unchanged index");

    // Removing a page *before* the active one changes only the index; it still announces.
    assert!(set.remove_page("a").is_some());
    assert_eq!(set.current_key(), Some("c"));
    assert_eq!(set.current_index(), 0);
    assert_eq!(*seen.lock().unwrap(), vec![1, 0], "an index-only change still announces");
}

#[test]
fn clearing_pages_announces_the_lost_active_page() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(set.add_page(FormPage::new("a", "A")));
    assert!(set.add_page(FormPage::new("b", "B")));
    assert_eq!(set.current_index(), 0);
    let seen = watch_current_changes(&mut set);

    set.clear_pages();
    assert_eq!(set.current_key(), None);
    assert_eq!(set.current_index(), 0);
    assert_eq!(
        *seen.lock().unwrap(),
        vec![0],
        "the active page disappeared even though the index stayed 0"
    );

    // An already-empty set has nothing to announce.
    set.clear_pages();
    assert_eq!(*seen.lock().unwrap(), vec![0]);
}

#[test]
fn moving_the_active_page_announces_its_new_index() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    for key in ["a", "b", "c"] {
        assert!(set.add_page(FormPage::new(key, key)));
    }
    assert!(set.set_current_index(1));
    let seen = watch_current_changes(&mut set);

    assert!(set.move_page("b", 2));
    assert_eq!(set.current_key(), Some("b"), "a move keeps the active page");
    assert_eq!(set.current_index(), 2);
    assert_eq!(*seen.lock().unwrap(), vec![2], "the active page's index changed");

    // Moving a page to its own position is a no-op and announces nothing.
    assert!(set.move_page("a", 0));
    assert_eq!(*seen.lock().unwrap(), vec![2]);
}

// ── Cross-field rule lifetime and unreachable rows/pages ────────────────────

/// Removing the field a rule compares against drops that rule, and its verdict with it.
#[test]
fn removing_the_referenced_field_drops_incoming_cross_field_rules() {
    let factory = factory();
    let password = mount(&factory, "line_edit", "s");
    let confirm = mount(&factory, "line_edit", "t");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("password", "Password").with_widget(password)));
    assert!(form.add_field(FormField::new("confirm", "Confirm").with_widget(confirm)));
    assert!(form.set_form_rule("confirm", FormRule::same_as("password")));
    assert!(!form.validate(), "the values differ");

    assert!(form.remove_field("password").is_some());
    assert!(
        form.form_rules("confirm").is_empty(),
        "the rule goes with the field it compared against"
    );
    assert!(form.validate(), "the surviving row is judged on its own value");
    assert_eq!(form.field("confirm").unwrap().error(), None);
}

/// A hidden field cannot be the reason a visible row fails.
#[test]
fn a_hidden_field_cannot_trigger_a_visible_rows_rule() {
    let factory = factory();
    let country = mount(&factory, "line_edit", "DE");
    let vat = mount(&factory, "line_edit", "");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("country", "Country").with_widget(country)));
    assert!(form.add_field(FormField::new("vat", "VAT").with_widget(vat)));
    assert!(form.set_form_rule("vat", FormRule::required_when("country", ["DE"])));
    assert!(!form.validate(), "the visible reference triggers the rule");

    assert!(form.set_field_visible("country", false));
    assert!(form.validate(), "a value the reader cannot see cannot require another row");
    assert_eq!(form.field("vat").unwrap().error(), None);
}

/// Hiding an invalid row retires its verdict, so showing it again cannot resurrect it.
#[test]
fn showing_a_row_after_fixing_it_while_hidden_reports_valid() {
    let factory = factory();
    let name = mount(&factory, "line_edit", "");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("name", "Name").with_required(true).with_widget(name)));
    assert!(!form.validate());
    assert!(form.field("name").unwrap().error().is_some());

    assert!(form.set_field_visible("name", false));
    assert_eq!(
        form.field("name").unwrap().error(),
        None,
        "a hidden row's verdict is retired with its visibility"
    );
    assert!(form.set_value_of("name", "Ada"));
    assert!(form.set_field_visible("name", true));
    assert_eq!(form.field("name").unwrap().error(), None);
    assert!(form.is_valid());
    assert_eq!(form.error_count(), 0);
}

/// Removing a rule retires the verdict it produced and repaints.
#[test]
fn removing_a_rule_retires_its_verdict() {
    let factory = factory();
    let other = mount(&factory, "line_edit", "x");
    let code = mount(&factory, "line_edit", "y");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("other", "Other").with_widget(other)));
    assert!(form.add_field(FormField::new("code", "Code").with_widget(code)));
    assert!(form.set_form_rule("code", FormRule::same_as("other")));
    assert!(!form.validate());

    let redraws = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&redraws);
    form.base().redraw_requested.connect(move || *sink.lock().unwrap() += 1);

    assert!(form.remove_form_rule("code"));
    assert_eq!(
        form.field("code").unwrap().error(),
        None,
        "the verdict belonged to the removed rule"
    );
    assert!(*redraws.lock().unwrap() > 0, "the repaint is requested");
    assert!(form.is_valid());
}

/// A disabled page is outside the set's aggregate: the reader cannot reach it to fix it.
#[test]
fn a_disabled_page_does_not_block_the_set() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "");
    let mut inner = Form::new(Rect::new(0, 0, 240, 120));
    assert!(inner.add_field(FormField::new("v", "V").with_required(true).with_widget(field)));
    assert!(!inner.validate());
    let form_id = runtime::register(Box::new(inner)).expect("the form registers");

    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(set.add_page_form("one", "One", form_id));
    assert!(set.add_page(FormPage::new("two", "Two")));
    set.page_mut("one").unwrap().set_enabled(false);
    assert!(!set.goto_page("one"), "a disabled page refuses activation");

    assert!(set.is_valid(), "an unreachable page never blocks the set");
    assert!(set.validate_all());
    assert_eq!(set.error_count(), 0);
    assert!(set.submit());
}

/// Removing a page hides the form it hosted, as `clear_pages` and `remove_field` do.
#[test]
fn removing_a_page_hides_its_form() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "");
    let mut inner = Form::new(Rect::new(0, 0, 240, 120));
    assert!(inner.add_field(FormField::new("v", "V").with_widget(field)));
    let form_id = runtime::register(Box::new(inner)).expect("the form registers");

    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(set.add_page_form("one", "One", form_id));
    assert!(set.remove_page("one").is_some());
    assert_eq!(
        rust_widgets::read_widget_property_by_id(form_id, "visible").expect("visible is readable"),
        CapabilityValue::Bool(false),
        "a removed page's form stops claiming to be visible"
    );
}

/// Clearing a closure rule retires the verdict it produced.
///
/// The sibling `remove_form_rule` was fixed for this; the validator setters only dropped the map
/// entry, so `is_valid()` kept reporting a rule that no longer existed.
#[test]
fn clearing_a_validator_retires_its_verdict() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "abc");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("v", "V").with_widget(field)));
    assert!(form.set_field_validator("v", |value| {
        if value.is_empty() {
            None
        } else {
            Some(String::from("Blocked"))
        }
    }));
    assert!(!form.validate());
    assert_eq!(form.field("v").unwrap().error(), Some("Blocked"));
    assert!(!form.is_valid());

    assert!(form.clear_field_validator("v"));
    assert_eq!(form.field("v").unwrap().error(), None, "the verdict belonged to the removed rule");
    assert!(form.is_valid(), "the aggregate follows the remaining rules");
}

/// Dropping the typed rule re-derives the row instead of leaving its message behind.
#[test]
fn clearing_a_typed_validator_retires_its_verdict() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "not-a-number");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("v", "V").with_widget(field)));
    assert!(form.set_field_kind("v", FieldKind::Integer { minimum: 1, maximum: 10 }));
    assert!(!form.validate());
    assert!(form.field("v").unwrap().error().is_some());

    assert!(form.clear_field_input_validator("v"));
    assert_eq!(form.field("v").unwrap().error(), None);
    assert!(form.is_valid());
}

/// Writing the value a field already holds is not a change and reports nothing.
///
/// The child-driven path (`resync_derived`) is an edge detector; the form-driven path emitted on
/// every successful write, so `field_changed` meant two different things.
#[test]
fn writing_the_same_value_emits_no_field_changed() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "same");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("v", "V").with_widget(field)));

    let changes = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&changes);
    form.event_signal_dyn("field_changed").unwrap().subscribe(Box::new(move |_| {
        *sink.lock().unwrap() += 1;
    }));

    assert!(form.set_value_of("v", "same"));
    assert_eq!(*changes.lock().unwrap(), 0, "an unchanged write is not an edge");
    assert!(form.set_value_of("v", "other"));
    assert_eq!(*changes.lock().unwrap(), 1);
}

/// Removing a comparison target re-derives its dependants; it must not wipe a verdict their own
/// rule still produces.
#[test]
fn removing_a_field_keeps_a_dependants_own_verdict() {
    let factory = factory();
    let other = mount(&factory, "line_edit", "x");
    let code = mount(&factory, "line_edit", "y");
    let mut form = Form::new(Rect::new(0, 0, 320, 240));
    assert!(form.add_field(FormField::new("other", "Other").with_widget(other)));
    assert!(form.add_field(FormField::new("code", "Code").with_widget(code)));
    assert!(form.set_form_rule("code", FormRule::same_as("other")));
    assert!(form.set_field_validator("code", |_| Some(String::from("Blocked"))));
    assert!(!form.validate());
    assert_eq!(form.field("code").unwrap().error(), Some("Blocked"));

    assert!(form.remove_field("other").is_some());
    assert_eq!(
        form.field("code").unwrap().error(),
        Some("Blocked"),
        "the closure rule is still installed and still fails"
    );
    assert!(!form.is_valid());
}

/// A page removed from under the active index lands on an enabled page, not a disabled one.
#[test]
fn removing_a_page_lands_on_an_enabled_page() {
    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    for key in ["a", "b", "c"] {
        assert!(set.add_page(FormPage::new(key, key)));
    }
    set.page_mut("b").unwrap().set_enabled(false);
    assert!(set.set_current_index(0));

    assert!(set.remove_page("a").is_some());
    assert_eq!(
        set.current_key(),
        Some("c"),
        "the disabled page must not become the active (and shown) page"
    );
}

/// `refresh` republishes the aggregate after a page's form changed behind the set's back.
#[test]
fn form_set_refresh_publishes_a_page_edit() {
    let factory = factory();
    let field = mount(&factory, "line_edit", "filled");
    let mut inner = Form::new(Rect::new(0, 0, 240, 120));
    assert!(inner.add_field(FormField::new("v", "V").with_required(true).with_widget(field)));
    let form_id = runtime::register(Box::new(inner)).expect("the form registers");

    let mut set = FormSet::new(Rect::new(0, 0, 480, 360));
    assert!(set.add_page_form("one", "One", form_id));
    assert!(set.is_valid());

    let validity = Arc::new(Mutex::new(None::<bool>));
    let sink = Arc::clone(&validity);
    set.event_signal_dyn("validity_changed").unwrap().subscribe(Box::new(move |value| {
        if let CapabilityValue::Bool(valid) = value {
            *sink.lock().unwrap() = Some(*valid);
        }
    }));

    // The hosted form's value is cleared behind the set's back, then the form validates itself.
    rust_widgets::write_widget_property_by_id(
        field,
        "text",
        CapabilityValue::String(String::new()),
    )
    .expect("the field's text is writable");
    rust_widgets::widget::runtime::with_widget_mut(form_id, |widget| {
        if let Some(properties) = widget.properties_dyn_mut() {
            let _ = properties.command("validate");
        }
    })
    .expect("the form is mounted");
    assert!(!set.is_valid());

    set.refresh();
    assert_eq!(
        *validity.lock().unwrap(),
        Some(false),
        "refresh publishes the aggregate edge a page edit produced"
    );
}
