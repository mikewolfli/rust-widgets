// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! §19.13 case I13: the entry-level `icon_source` channels must round-trip, and a source with no
//! document form must be **refused** rather than lost.
//!
//! # What the case asks for
//!
//! *"JSON round-trip, old/new conflicting fields, a runtime-only source cannot be serialised — the
//! outcome is a clear preservation or a clear refusal, and a handle is never stringified."*
//!
//! Three collections publish a row that carries a typed icon — `Chip` (keyed by its host-supplied
//! text id), `DropdownMenu` (keyed by [`DropdownItem::value`]) and `MenuButton` (keyed by
//! `MenuItem::id`). Each row is the only shape a document can hold: `dropdown_menu` and `menu_button`
//! rows cannot express `children`/`submenu`, so a write is refused rather than accepted-and-truncated.
//!
//! # Why a round trip rather than two independent reads
//!
//! A reader and a writer can each look correct while disagreeing about a row's document shape — the
//! reader accepting `{"kind":"builtin","name":"info"}` while the writer emits `{"kind":"name"}`. Only
//! *read → write → read → write* catches that, because the second write can only match the first if
//! both directions agree. The comparison here is byte-for-byte on the serialised rows, and identity
//! is asserted separately because two rows that both round-trip but swap their ids is exactly the
//! failure a byte comparison of the *icon* alone would miss.
#![cfg(full_widgets)]

use rust_widgets::core::Rect;
use rust_widgets::json::records_to_json;
use rust_widgets::widget::{
    widget_property_get, widget_property_set, CapabilityAccessError, CapabilityValue, DropdownItem,
    DropdownMenu, IconData, IconName, IconSource, MenuButton, MenuItem, Widget, WidgetFactory,
};

/// A source that exists only at runtime: borrowed path data with no document form.
static RING: IconData =
    IconData { name: "ring", grid: 24, paths: &["M4 4h16v16H4Z", "M9 9h6v6H9Z"] };

/// Reads a control's `items` row list.
fn rows_of(widget: &dyn Widget) -> CapabilityValue {
    widget_property_get(widget, "items").expect("`items` must be readable")
}

/// Serialises a control's `items` rows into the JSON text a document would carry.
fn rows_json(widget: &dyn Widget) -> String {
    let value = rows_of(widget);
    let json = records_to_json(&value).expect("the published rows must have document form");
    serde_json::to_string(&json).expect("a record array always serialises")
}

/// The identity field of each row, as text, in row order.
fn row_identities(widget: &dyn Widget) -> Vec<String> {
    let CapabilityValue::Records(list) = rows_of(widget) else {
        panic!("`items` must read as a record list");
    };
    list.items()
        .iter()
        .map(|record| {
            // `Chip` and `DropdownMenu` key by text, `MenuButton` by number; both spell an identity
            // the same way once it reaches the document, which is the point of comparing them.
            match record.get("id").or_else(|| record.get("value")) {
                Some(CapabilityValue::String(text)) => text.clone(),
                Some(CapabilityValue::UInt(number)) => number.to_string(),
                other => panic!("a row must carry a text or numeric identity, got {other:?}"),
            }
        })
        .collect()
}

/// The typed icon each row publishes, or `None` for a row with no icon.
fn row_icons(widget: &dyn Widget) -> Vec<Option<IconName>> {
    let CapabilityValue::Records(list) = rows_of(widget) else {
        panic!("`items` must read as a record list");
    };
    list.items()
        .iter()
        .map(|record| match record.get("icon_source") {
            None | Some(CapabilityValue::Null) => None,
            Some(CapabilityValue::IconSpec(
                rust_widgets::widget::icon_source::IconSpec::Builtin(name),
            )) => Some(*name),
            other => panic!("a row must publish a built-in descriptor or none, got {other:?}"),
        })
        .collect()
}

/// Builds one `chip` document holding `items`, under the chip id `id`.
///
/// The id is a parameter because the widget runtime registers ids process-wide: loading two
/// documents that both call their chip `chips` makes the second registration collide with the first,
/// which would test the registry rather than the round trip.
fn chip_document(id: &str, items: &str) -> String {
    format!(
        r#"{{"window": {{"id": "w", "title": "Chips", "width": 400, "height": 300, "layout": {{"type": "vbox", "children": [{{"chip": {{"id": "{id}", "items": {items}}}}}]}}}}}}"#
    )
}

/// Loads `document` and runs `read` against the chip it names.
///
/// The widget runtime registers ids process-wide and does not release them when a layout is
/// dropped, so a document can only be mounted once per process. Everything a test needs from one
/// mount is therefore read in a single call.
fn with_chip<R>(id: &str, document: &str, read: impl FnOnce(&dyn Widget) -> R) -> R {
    let layout = rust_widgets::json::JsonLoader::load(document).expect("the document must load");
    let chip = layout.id(id).expect("the document names the chip");
    rust_widgets::widget::runtime::with_widget(chip, |widget| read(widget))
        .expect("the chip is mounted")
}

// ---------------------------------------------------------------------------------------------
// 1. The document channel: chip rows survive document → rows → document → rows.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_chip_document_round_trips_its_item_icons_and_identity() {
    let first = chip_document(
        "chips_one",
        r#"[{"id": "a", "label": "Alpha", "icon_source": {"kind": "builtin", "name": "info"}},
            {"id": "b", "label": "Beta"}]"#,
    );
    let rows_once = with_chip("chips_one", &first, rows_json);

    // The dumped rows are themselves a document value: loading them must reproduce them exactly.
    let second = chip_document("chips_two", &rows_once);
    let (rows_twice, identities, icons) = with_chip("chips_two", &second, |widget| {
        (rows_json(widget), row_identities(widget), row_icons(widget))
    });

    assert_eq!(
        rows_once, rows_twice,
        "the row the chip publishes must be the row it reads back, byte for byte"
    );
    assert_eq!(identities, vec!["a", "b"], "identity must not drift across the round trip");
    assert_eq!(
        icons,
        vec![Some(IconName::Info), None],
        "the icon must survive, and a row that had none must still have none"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. The menu channels: rows survive control → rows → control → rows.
// ---------------------------------------------------------------------------------------------

/// Rebuilds a `DropdownMenu` from the rows a previous one published.
fn rebuild_dropdown(rows: &str) -> DropdownMenu {
    let mut menu = DropdownMenu::new(Rect::new(0, 0, 200, 28));
    let parsed: Vec<serde_json::Value> = serde_json::from_str(rows).expect("rows are JSON");
    for row in parsed {
        let value = row["value"].as_str().expect("a dropdown row carries `value`");
        menu.add_item(DropdownItem::new(value, row["label"].as_str().unwrap_or(value)));
        if let Some(icon) = row.get("icon_source") {
            menu.set_item_icon_source(value, Some(spec_to_source(icon)))
                .expect("the row names an item that exists");
        }
    }
    menu
}

/// Rebuilds a `MenuButton` from the rows a previous one published.
fn rebuild_menu_button(rows: &str) -> MenuButton {
    let mut button = MenuButton::new("Menu", Rect::new(0, 0, 120, 28));
    let parsed: Vec<serde_json::Value> = serde_json::from_str(rows).expect("rows are JSON");
    for row in parsed {
        let id = row["id"].as_u64().expect("a menu row carries a numeric `id`");
        button.add_item(MenuItem::new(id, row["text"].as_str().unwrap_or("")));
        if let Some(icon) = row.get("icon_source") {
            button
                .set_item_icon_source(id, Some(spec_to_source(icon)))
                .expect("the row names an item that exists");
        }
    }
    button
}

/// The `IconSource` a published row's `icon_source` object describes.
///
/// Reads the object with the same two keys the loader reads, rather than deriving `Deserialize` for
/// the descriptor: the document form is the loader's business, and a second parser here would be a
/// copy of it that could accept a shape the loader refuses.
fn spec_to_source(icon: &serde_json::Value) -> IconSource {
    assert_eq!(icon["kind"].as_str(), Some("builtin"), "only a built-in row is published here");
    let name = icon["name"].as_str().expect("a built-in descriptor names its token");
    IconSource::Builtin(IconName::from_name(name).expect("the published token is a built-in name"))
}

#[test]
fn the_two_menu_channels_round_trip_through_the_rows_they_publish() {
    let factory = WidgetFactory::new_with_defaults();

    let mut dropdown = DropdownMenu::new(Rect::new(0, 0, 200, 28));
    dropdown.add_item(DropdownItem::new("save", "Save"));
    dropdown.add_item(DropdownItem::new("open", "Open"));
    dropdown
        .set_item_icon_source("save", Some(IconSource::Builtin(IconName::Save)))
        .expect("the item exists");
    let dropdown_rows = rows_json(&dropdown);
    let rebuilt_dropdown = rebuild_dropdown(&dropdown_rows);
    assert_eq!(
        dropdown_rows,
        rows_json(&rebuilt_dropdown),
        "a dropdown row must be rebuildable from what the control published"
    );
    assert_eq!(row_identities(&rebuilt_dropdown), vec!["save", "open"]);
    assert_eq!(row_icons(&rebuilt_dropdown), vec![Some(IconName::Save), None]);

    let mut button = MenuButton::new("Menu", Rect::new(0, 0, 120, 28));
    button.add_item(MenuItem::new(1, "New"));
    button.add_item(MenuItem::new(2, "Open"));
    button
        .set_item_icon_source(2, Some(IconSource::Builtin(IconName::Info)))
        .expect("the item exists");
    let button_rows = rows_json(&button);
    let rebuilt_button = rebuild_menu_button(&button_rows);
    assert_eq!(
        button_rows,
        rows_json(&rebuilt_button),
        "a menu-button row must be rebuildable from what the control published"
    );
    assert_eq!(row_identities(&rebuilt_button), vec!["1", "2"]);
    assert_eq!(row_icons(&rebuilt_button), vec![None, Some(IconName::Info)]);

    // The same factory must construct both channels, so a document can name them at all.
    assert!(
        factory.create("dropdown_menu", Rect::new(0, 0, 200, 28), "").is_some(),
        "`dropdown_menu` must be constructible by name"
    );
    assert!(
        factory.create("menu_button", Rect::new(0, 0, 120, 28), "").is_some(),
        "`menu_button` must be constructible by name"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. A runtime-only source refuses the read rather than dropping the icon.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_runtime_only_source_refuses_the_row_read_on_every_channel() {
    let mut chip = rust_widgets::widget::Chip::new(Rect::new(0, 0, 200, 36));
    chip.set_items(vec![rust_widgets::widget::ChipItem::new("a", "Alpha")]);
    chip.set_item_icon_source("a", Some(IconSource::static_geometry(&RING)))
        .expect("the item exists");
    assert_eq!(
        widget_property_get(&chip, "items"),
        Err(CapabilityAccessError::IconSourceNotPersistable),
        "a chip row must refuse a source with no document form instead of losing it"
    );

    let mut dropdown = DropdownMenu::new(Rect::new(0, 0, 200, 28));
    dropdown.add_item(DropdownItem::new("save", "Save"));
    dropdown.set_item_icon_source("save", Some(IconSource::static_geometry(&RING))).unwrap();
    assert_eq!(
        widget_property_get(&dropdown, "items"),
        Err(CapabilityAccessError::IconSourceNotPersistable),
        "a dropdown row must refuse it too"
    );

    let mut button = MenuButton::new("Menu", Rect::new(0, 0, 120, 28));
    button.add_item(MenuItem::new(1, "New"));
    button.set_item_icon_source(1, Some(IconSource::static_geometry(&RING))).unwrap();
    assert_eq!(
        widget_property_get(&button, "items"),
        Err(CapabilityAccessError::IconSourceNotPersistable),
        "a menu-button row must refuse it too"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. The old/new conflict: a row that declares both channels is refused, not resolved by key order.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_row_declaring_a_legacy_icon_beside_the_typed_one_is_refused_not_last_wins() {
    use rust_widgets::widget::capability::{RecordList, RecordValue, EMPTY_RECORD_SCHEMA};
    use rust_widgets::widget::icon_source::IconSpec;

    let mut chip = rust_widgets::widget::Chip::new(Rect::new(0, 0, 200, 36));

    // The document declares the legacy text channel *and* the typed one for the same item.
    // Accepting either — or letting the later JSON key win — would make the row's meaning depend on
    // key order. The row is built with the record API rather than parsed from text because the
    // document reader copies a row's fields verbatim and leaves the judgement to this validator, so
    // this value is exactly what a document holding both keys produces.
    let conflicting = RecordValue::new()
        .with("id", CapabilityValue::String(String::from("a")))
        .with("label", CapabilityValue::String(String::from("Alpha")))
        .with("icon_text", CapabilityValue::String(String::from("★")))
        .with("icon_source", CapabilityValue::IconSpec(IconSpec::Builtin(IconName::Info)));
    let refused = widget_property_set(
        &mut chip,
        "items",
        CapabilityValue::Records(RecordList::from_items(&EMPTY_RECORD_SCHEMA, vec![conflicting])),
    );
    match refused {
        Err(CapabilityAccessError::ConstraintViolation { reason, .. }) => assert!(
            reason.contains("icon_text"),
            "the refusal must name the conflicting field, got `{reason}`"
        ),
        other => panic!("a conflicting row must be refused, got {other:?}"),
    }
    assert!(chip.items().is_empty(), "a refused row must leave the control untouched");

    // The contrast case: the same row *without* the legacy field is accepted, so the refusal above
    // is about the conflict and not about the typed descriptor being rejected in general.
    let clean = RecordValue::new()
        .with("id", CapabilityValue::String(String::from("a")))
        .with("label", CapabilityValue::String(String::from("Alpha")))
        .with("icon_source", CapabilityValue::IconSpec(IconSpec::Builtin(IconName::Info)));
    widget_property_set(
        &mut chip,
        "items",
        CapabilityValue::Records(RecordList::from_items(&EMPTY_RECORD_SCHEMA, vec![clean])),
    )
    .expect("a row with only the typed descriptor is accepted");
    assert_eq!(chip.items().len(), 1);
    assert_eq!(chip.item_icon_source("a"), Some(IconSource::Builtin(IconName::Info)));
}

// ---------------------------------------------------------------------------------------------
// 5. A menu row is read-only, because a row cannot carry a subtree.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_menu_row_refuses_a_write_so_a_subtree_cannot_be_dropped_silently() {
    let mut dropdown = DropdownMenu::new(Rect::new(0, 0, 200, 28));
    dropdown.add_item(DropdownItem::new("parent", "Parent"));
    let rows = rows_of(&dropdown);
    assert_eq!(
        widget_property_set(&mut dropdown, "items", rows.clone()),
        Err(CapabilityAccessError::ReadOnlyProperty),
        "a dropdown row cannot express `children`, so a write must be refused"
    );

    let mut button = MenuButton::new("Menu", Rect::new(0, 0, 120, 28));
    button.add_item(MenuItem::new(1, "Parent"));
    let rows = rows_of(&button);
    assert_eq!(
        widget_property_set(&mut button, "items", rows),
        Err(CapabilityAccessError::ReadOnlyProperty),
        "a menu-button row cannot express `submenu`, so a write must be refused"
    );
}
