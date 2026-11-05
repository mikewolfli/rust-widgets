// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The suite-wide event contract: every published name resolves on a real control.
//!
//! The parent crate's `tests/event_wiring_test.rs` audits the controls the parent registers.
//! When a group moves here its capability row moves with it, so that gate can no longer see it:
//! the row is no longer in the parent's registry. This test closes the same loop from the side
//! that owns the row — it builds every group's controls through the factory and asks each one for
//! every event its own capability publishes.
//!
//! A name that resolves nowhere is the defect principle #97 names: `connect_event` would answer
//! `Ok` for a name nobody can subscribe to, and the wire would be silently inert.

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::WidgetFactory;

/// RW-63: the generic handle's focus subscription works on a control this crate owns.
///
/// The subscription is a default method on `WidgetHandle`, so it covers every control the parent
/// runtime mounts — including the ones whose capability rows live here. This test is the smoke
/// that the trait default is reachable across the crate boundary with a suite control.
#[test]
fn a_suite_control_observes_focus_through_the_generic_handle() {
    use core::sync::atomic::{AtomicUsize, Ordering};
    use rust_widgets::app::{SurfaceHandle, WidgetHandle};
    use rust_widgets::widget::runtime;
    use std::sync::Arc;

    let factory = suite_factory();
    let widget = factory
        .create("form", Rect::new(0, 0, 320, 240), "Account")
        .expect("the suite registers `form`");
    let id = runtime::register(widget).expect("the form mounts");
    let handle = SurfaceHandle::from_raw(id);

    let gained = Arc::new(AtomicUsize::new(0));
    assert!(handle.on_focus_gained({
        let counter = Arc::clone(&gained);
        move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    }));
    assert!(runtime::focus_widget(id));
    assert_eq!(
        gained.load(Ordering::SeqCst),
        1,
        "the suite control's focus signal reaches the handle"
    );

    runtime::unregister(id);
}

/// Builds a factory with every enabled suite group installed.
fn suite_factory() -> WidgetFactory {
    let mut factory = WidgetFactory::new_with_defaults();
    crate::install(&mut factory).expect("the suite installs into a fresh factory");
    factory
}

/// Asserts every published event of every row is reachable from a constructed control.
///
/// All rows are checked before the assertion fires, so one run names every control that fails
/// instead of stopping at the first.
fn assert_group(
    factory: &mut WidgetFactory,
    group: &str,
    rows: Vec<(&'static str, Vec<&'static str>)>,
) {
    let mut failures: Vec<String> = Vec::new();
    for (control, events) in rows {
        let Some(widget) = factory.create(control, Rect::new(0, 0, 240, 120), "") else {
            failures
                .push(format!("{group}: `{control}` is published but the factory cannot build it"));
            continue;
        };
        let canonical =
            factory.capability_for_kind_instance(widget.as_ref()).map(|row| row.canonical_name);
        if canonical != Some(control) {
            failures.push(format!(
                "{group}: the factory must resolve `{control}` to its own capability row, not a \
                 sibling that shares its kind (resolved {canonical:?})"
            ));
        }
        for event in events {
            let Some(reference) = widget.event_signal_dyn(event) else {
                failures.push(format!(
                    "{group}: `{control}` publishes `{event}` but `event_signal_dyn` does not \
                     resolve it, so a subscription would succeed and never fire"
                ));
                continue;
            };
            // SUIT-1: the bridge's contract is stronger than "a reference exists" — the
            // reference must also *declare* the name that was requested, or the bridge refuses
            // it (DEV-1 was exactly that: an alias resolved, the published name never fired).
            if reference.name() != event {
                failures.push(format!(
                    "{group}: `{control}` resolves `{event}` to a reference declared as `{}`, so \
                     the bridge refuses the subscription",
                    reference.name()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// Asserts every published readable property is answered by the control that publishes it.
///
/// WV-04(b): the same gate the event rows get, for the read direction — and for a read-only row,
/// the write direction must be refused. `event_signal_dyn` answering is no evidence that a
/// schema row a designer will show can be read, which is how WV-03 landed.
fn assert_properties(
    factory: &mut WidgetFactory,
    group: &str,
    rows: Vec<(&'static str, Vec<&'static str>)>,
) {
    use rust_widgets::widget::capability::types::CapabilityValue;

    for (control, properties) in rows {
        if properties.is_empty() {
            continue;
        }
        // Scoped to groups with a constructible widget in this harness: a property-only table
        // row (an event-less companion) has nothing here to read from.
        let Some(mut widget) = factory.create(control, Rect::new(0, 0, 240, 120), "") else {
            continue;
        };
        let capability = factory
            .capability(control)
            .unwrap_or_else(|| panic!("{group}: `{control}` has properties but no capability row"));
        for name in properties {
            let Some(schema) = capability.properties.iter().find(|schema| schema.name == name)
            else {
                panic!("{group}: `{control}` lists `{name}` without a property schema")
            };
            if schema.readable {
                assert!(
                    factory.read_property(widget.as_ref(), name).is_ok(),
                    "{group}: `{control}` publishes `{name}` as readable but the control does not \
                     answer it, so a declarative read fails on a value the control holds"
                );
            }
            if schema.readable && !schema.writable {
                assert!(
                    factory.write_property(widget.as_mut(), name, CapabilityValue::Null).is_err(),
                    "{group}: `{control}` publishes `{name}` as read-only but accepted a write"
                );
            }
        }
    }
}

#[test]
fn every_published_suite_event_resolves_on_its_control() {
    let mut factory = suite_factory();

    let (form_events, form_set_events) = crate::form_widgets::published_event_names();
    assert_group(&mut factory, "form", vec![("form", form_events), ("form_set", form_set_events)]);

    #[cfg(feature = "webview")]
    assert_group(&mut factory, "webview", crate::webview::published_event_names());

    #[cfg(feature = "finance")]
    assert_group(&mut factory, "finance", crate::finance::published_event_names());

    #[cfg(feature = "maps")]
    assert_group(&mut factory, "maps", crate::maps::published_event_names());

    #[cfg(feature = "project")]
    assert_group(&mut factory, "project", crate::project::published_event_names());

    #[cfg(feature = "devtools")]
    assert_group(&mut factory, "devtools", crate::devtools::published_event_names());

    #[cfg(feature = "media-playback")]
    assert_group(&mut factory, "media-playback", crate::media_playback::published_event_names());
}

/// WV-04(b): every published readable property is answered by the control that publishes it.
#[test]
fn every_published_suite_property_reads_on_its_control() {
    let mut factory = suite_factory();

    let (form_properties, form_set_properties) = crate::form_widgets::published_property_names();
    assert_properties(
        &mut factory,
        "form",
        vec![("form", form_properties), ("form_set", form_set_properties)],
    );

    #[cfg(feature = "webview")]
    assert_properties(&mut factory, "webview", crate::webview::published_property_names());

    #[cfg(feature = "finance")]
    assert_properties(&mut factory, "finance", crate::finance::published_property_names());

    #[cfg(feature = "maps")]
    assert_properties(&mut factory, "maps", crate::maps::published_property_names());

    #[cfg(feature = "project")]
    assert_properties(&mut factory, "project", crate::project::published_property_names());

    #[cfg(feature = "devtools")]
    assert_properties(&mut factory, "devtools", crate::devtools::published_property_names());

    #[cfg(feature = "media-playback")]
    assert_properties(
        &mut factory,
        "media-playback",
        crate::media_playback::published_property_names(),
    );
}

/// RW-105: every control the suites register must export a designer manifest.
///
/// `designer_manifest` answers `IncompleteDefaults` when one advertised property has no default.
/// That failure is invisible to the control's own tests — the control builds and behaves — but it
/// makes the whole row unreadable to a designer, which is how `gantt_task_grid`, `map_overview`,
/// `map_attribution` and `terminal_find_bar` were registered and yet absent from the host's
/// catalog. This walks every published name of every enabled group, so a control that is
/// registered without defaults cannot land again.
#[test]
fn every_registered_suite_control_exports_a_designer_manifest() {
    use rust_widgets::widget::capability::designer_manifest;

    let factory = suite_factory();
    let mut groups: Vec<(&'static str, Vec<&'static str>)> =
        vec![("form", vec!["form", "form_set"])];

    #[cfg(feature = "webview")]
    groups.push((
        "webview",
        crate::webview::published_property_names().into_iter().map(|(name, _)| name).collect(),
    ));
    #[cfg(feature = "finance")]
    groups.push((
        "finance",
        crate::finance::published_property_names().into_iter().map(|(name, _)| name).collect(),
    ));
    #[cfg(feature = "maps")]
    groups.push((
        "maps",
        crate::maps::published_property_names().into_iter().map(|(name, _)| name).collect(),
    ));
    #[cfg(feature = "project")]
    groups.push((
        "project",
        crate::project::published_property_names().into_iter().map(|(name, _)| name).collect(),
    ));
    #[cfg(feature = "devtools")]
    groups.push((
        "devtools",
        crate::devtools::published_property_names().into_iter().map(|(name, _)| name).collect(),
    ));
    #[cfg(feature = "media-playback")]
    groups.push((
        "media-playback",
        crate::media_playback::published_property_names()
            .into_iter()
            .map(|(name, _)| name)
            .collect(),
    ));

    let mut checked = 0usize;
    for (group, controls) in groups {
        for control in controls {
            checked += 1;
            match designer_manifest(&factory, control) {
                Ok(manifest) => {
                    assert_eq!(
                        manifest.control, control,
                        "{group}: the manifest must describe the control that was asked for"
                    );
                    let capability = factory
                        .capability(control)
                        .expect("a manifest exists only for a registered control");
                    for command in capability.commands {
                        assert!(
                            manifest.commands.iter().any(|entry| entry.name == *command),
                            "{group}: `{control}` publishes `{command}` but the manifest cannot \
                             describe it, so a designer would never offer it"
                        );
                    }
                }
                Err(error) => panic!(
                    "{group}: `{control}` is registered but its designer manifest is \
                     unreadable: {error}"
                ),
            }
        }
    }
    assert!(checked > 0, "no suite control was checked, so this test proves nothing");
}

/// RW-106: the data-bearing suite controls have a writable carrier and a published command.
///
/// Each of these controls could be created but not filled from a design document: a count was
/// published where the rows were needed, or the only data entry point was a Rust method. This
/// walks the published surface — `write_property` then `read_property` — and checks the command
/// list names the same write, so a designer can reach it.
#[test]
fn the_design_time_data_carriers_round_trip() {
    use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue as V};
    use rust_widgets::widget::capability::{RecordList, RecordValue};

    use crate::finance::types::BOOK_LEVEL_RECORD_SCHEMA;
    use crate::media_playback::chapter_list_panel::CHAPTER_RECORD_SCHEMA;
    use crate::webview::web_history_panel::HISTORY_RECORD_SCHEMA;

    let factory = suite_factory();
    let rect = Rect::new(0, 0, 320, 180);

    // The two ladders of `order_book` / `depth_chart`.
    let ladder = RecordList::from_items(
        &BOOK_LEVEL_RECORD_SCHEMA,
        vec![RecordValue::new().with("price", V::Float(100.5)).with("quantity", V::Float(3.0))],
    );
    for control in ["order_book", "depth_chart"] {
        let mut widget = factory.create(control, rect, "").expect("the suite registers it");
        factory
            .write_property(widget.as_mut(), "bids", V::Records(ladder.clone()))
            .expect("bids is writable");
        let V::Records(read) = factory.read_property(widget.as_ref(), "bids").expect("readable")
        else {
            panic!("{control}: bids must read back as records")
        };
        assert_eq!(read.items().len(), 1, "{control}: the written level must survive");
        assert_eq!(read.items()[0].get("price"), Some(&V::Float(100.5)));
        assert!(
            matches!(
                factory.invoke_command(widget.as_mut(), "set_bids"),
                Err(CapabilityAccessError::OutOfRange)
            ),
            "{control}: set_bids must be published and reported as value-carrying"
        );
    }

    // `chapter_list_panel`: the rows, with the count under its own name.
    let mut chapters = factory.create("chapter_list_panel", rect, "").expect("registered");
    let rows = RecordList::from_items(
        &CHAPTER_RECORD_SCHEMA,
        vec![
            RecordValue::new()
                .with("title", V::String("Intro".into()))
                .with("start_seconds", V::Float(0.0)),
            RecordValue::new()
                .with("title", V::String("Middle".into()))
                .with("start_seconds", V::Float(42.5)),
        ],
    );
    factory
        .write_property(chapters.as_mut(), "chapters", V::Records(rows))
        .expect("chapters is writable");
    assert_eq!(
        factory.read_property(chapters.as_ref(), "chapter_count"),
        Ok(V::UInt(2)),
        "the count must follow the written rows"
    );

    // `web_history_panel`: sample history, and the `clear` command.
    let mut history = factory.create("web_history_panel", rect, "").expect("registered");
    let entries = RecordList::from_items(
        &HISTORY_RECORD_SCHEMA,
        vec![RecordValue::new()
            .with("url", V::String("https://example.com/".into()))
            .with("title", V::String("Example".into()))
            .with("visited_ms", V::Int(1_700_000_000_000))],
    );
    factory
        .write_property(history.as_mut(), "entries", V::Records(entries))
        .expect("entries is writable");
    assert_eq!(factory.read_property(history.as_ref(), "visible_count"), Ok(V::UInt(1)));
    factory.invoke_command(history.as_mut(), "clear").expect("clear is published");
    let V::Records(cleared) = factory.read_property(history.as_ref(), "entries").expect("readable")
    else {
        panic!("entries must read back as records")
    };
    assert!(cleared.items().is_empty(), "clear must empty the rows");

    // `rive_widget`: the document reference and the scrubbed playhead.
    let mut rive = factory.create("rive_widget", rect, "").expect("registered");
    factory
        .write_property(rive.as_mut(), "source", V::String("intro.riv".into()))
        .expect("source is writable");
    assert_eq!(factory.read_property(rive.as_ref(), "source"), Ok(V::String("intro.riv".into())));
    factory.write_property(rive.as_mut(), "progress", V::Float(0.5)).expect("progress is writable");
    assert_eq!(factory.read_property(rive.as_ref(), "progress"), Ok(V::Float(0.5)));
}
