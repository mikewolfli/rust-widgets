// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Capability constructors for the `view` widgets.
//
// The property arrays these constructors reference are declared in
// `properties_view.in.rs` and expanded by the parent module. `use super::*` brings
// those arrays and the shared imports into scope; `events_of!` is visible through the
// parent module's textual macro scope.

use super::*;

#[cfg(not(alloc_frugal))]
pub(crate) fn list_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "list_view",
        aliases: &[],
        properties: LIST_VIEW_PROPERTIES,
        events: events_of!("list_view"),
        commands: &["clear_selection", "clear_focused_row"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tree_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TreeView,
        canonical_name: "tree_view",
        aliases: &[],
        properties: TREE_VIEW_PROPERTIES,
        events: events_of!("tree_view"),
        // The payload-free expand/collapse/toggle verbs act on the **focused** node, because a
        // command with no payload cannot name another row. The spellings are the ones the sibling
        // hierarchical control (`cascader`) already publishes, so a host sees one vocabulary for
        // "open the current branch" rather than one per control.
        commands: &["clear_selection", "clear_focused_node", "expand", "collapse", "toggle"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn table_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "table_widget",
        aliases: &[],
        properties: TABLE_WIDGET_PROPERTIES,
        events: events_of!("table_widget"),
        commands: &["clear_selection", "clear_focused_row"],
    }
}

/// The capability for `WidgetKind::Table`, the kind the public `create_table(..)`
/// mounts — and the one every `table` spelling also addresses.
///
/// # Why `table` was an alias of `table_widget` before this row existed
///
/// Because there was no other way to make it resolve. With no entry declaring
/// `WidgetKind::Table` as its canonical name, `capability_by_kind(Table)` fell back
/// to `indices[0]` — **whichever of `table_widget`, `data_grid`, `tree_table`,
/// `virtual_table` happened to be registered first**. That order is invisible to
/// every caller-facing check: the id that comes back is valid, the control mounts,
/// and only the *kind* of control is wrong. A row whose `canonical_name` is `table`
/// removes the ordering from the answer entirely: `capability_by_kind` matches on
/// `canonical_name == "table"`, so the resolution is the same for every registration
/// order (rule #2: fix the class, not the instance).
///
/// The property schema is the one `table_widget` published, so no caller sees a
/// change in behaviour — only a stable answer.
#[cfg(not(alloc_frugal))]
pub(crate) fn table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "table",
        aliases: &[],
        properties: TABLE_WIDGET_PROPERTIES,
        events: events_of!("table"),
        commands: &["clear_selection", "clear_focused_row"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn data_grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "data_grid",
        aliases: &[],
        properties: DATA_GRID_PROPERTIES,
        events: events_of!("data_grid"),
        // Audit 20261026: `select_all` is payload-free and runs the control's real
        // `select_all`, so it is published here rather than left as a name no arm answered.
        // `set_data_source` binds an `Arc<dyn IncrementalTableDataSource>`, which no property value
        // can carry; `set_current_cell` / `move_current_cell` name a position and the published
        // `current_cell` is read-only, so all three stay unpublished.
        commands: &[
            "clear_data_source",
            "set_scroll_row",
            "set_scroll_column",
            "set_row_height",
            "set_column_width",
            "set_frozen_columns",
            "select_all",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn tree_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TreeTable,
        canonical_name: "tree_table",
        aliases: &[],
        properties: TREE_TABLE_PROPERTIES,
        events: events_of!("tree_table"),
        // Audit 20261026: `select_row` stays unpublished — the writable `selected_row` property is
        // the same assignment (principle #18). `set_model` binds an `Arc<dyn ...>` model, which no
        // property value can carry; `expand_row`/`collapse_row` name a row and the table publishes
        // no expandable-row property, so all three stay unpublished.
        commands: &["clear_model"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn virtual_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "virtual_table",
        aliases: &[],
        properties: VIRTUAL_TABLE_PROPERTIES,
        events: events_of!("virtual_table"),
        // Audit 20261026: `set_data_source` binds an `Arc<dyn ...>` source no property value can
        // carry, and `select_cell` names a position while the table publishes only scroll offsets
        // (no cell-selection property), so both stay unpublished (principle #18).
        commands: &[
            "clear_data_source",
            "set_scroll_row",
            "set_scroll_column",
            "set_row_height",
            "set_column_width",
            "set_overscan_rows",
            "set_overscan_columns",
            "fetch_visible_window",
            "clear_selection",
        ],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn virtual_list_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DataView,
        canonical_name: "virtual_list",
        aliases: &[],
        properties: VIRTUAL_LIST_PROPERTIES,
        events: events_of!("virtual_list"),
        commands: &["clear_data_source", "fetch_visible_rows"],
    }
}

/// The capability for `WidgetKind::DataView`, the kind the public
/// `create_data_view(..)` mounts.
///
/// # Why this exists next to `virtual_list_capability`
///
/// `DataView` and `VirtualList` are the same Rust type (`widget::mod.rs` declares
/// `pub type DataView = VirtualList;`), so both kinds are served by one constructor
/// and the factory keeps one entry whose canonical name is `virtual_list`.
///
/// The kind→capability index still has to answer for `DataView` — `create_data_view`
/// mounts exactly that kind — and the answer it used to give was **whichever entry
/// registered first**, because no entry declared the kind and the fallback in
/// `capability_by_kind` takes `indices[0]`. That is the same order-dependent
/// resolution that made `ToolButton` build a `split_button` and `Table` build a
/// `tree_table`; it is silent because the id is valid.
///
/// Rule #54 argues against a second *type*, and this is not one: it registers the
/// kind under its own name while keeping the same type, the same property schema and
/// therefore the same behaviour. The alternative — adding `dataview` as an alias of
/// `virtual_list` — was rejected because `capability_by_kind` matches on
/// `canonical_name`, so an alias would leave the resolution order-dependent and the
/// defect reachable again by a different registration order.
#[cfg(not(alloc_frugal))]
pub(crate) fn data_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DataView,
        canonical_name: "data_view",
        aliases: &[],
        properties: VIRTUAL_LIST_PROPERTIES,
        events: events_of!("data_view"),
        commands: &["clear_data_source", "fetch_visible_rows"],
    }
}

/// The modal colour dialog — a window that *hosts* a picker.
///
/// # Why this is separate from `color_picker`
///
/// `ColorDialog` is a `WidgetKind` of its own (it has a window, a title, and
/// accept/reject) and `ColorPicker` is the inline control inside it. Before phase
/// E-6 one capability served both, which is why `color_dialog` was an *alias* of
/// `color_picker` — and why the dialog's kind had no canonical name of its own.
/// Splitting the picker onto its own kind therefore requires the dialog to get a
/// registration too, or `kind_factory_name(ColorDialog)` resolves to nothing and
/// `create_color_dialog` returns 0.
///
/// `color_dialog` is no longer an alias of the picker, so the two names now address
/// two different controls rather than one control under two names.
#[cfg(not(alloc_frugal))]
pub(crate) fn color_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ColorDialog,
        canonical_name: "color_dialog",
        aliases: &["colour_dialog"],
        properties: COLOR_DIALOG_PROPERTIES,
        events: events_of!("color_dialog"),
        // Audit 20261026: `set_hex` and `apply_preset` stay unpublished — the dialog publishes the
        // writable `hex_rgba` and `preset_index` properties, which are exactly the colour and the
        // preset a caller writes, so both names are reachable through the property route
        // (principle #18).
        commands: &[],
    }
}

// Found by the registration-fidelity gate rather than by hand: `GridTableWidget`
// is a complete virtualised table that was exported but never registered, so it
// joins the six above as a seventh wiring-only fix. It reports `WidgetKind::GridTable`,
// which no other capability claims.
#[cfg(not(alloc_frugal))]
pub(crate) fn otp_input_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::OtpInput,
        canonical_name: "otp_input",
        aliases: &["otp"],
        properties: OTP_INPUT_PROPERTIES,
        events: events_of!("otp_input"),
        commands: &["insert_char", "backspace", "paste", "clear"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn banner_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Banner,
        canonical_name: "banner",
        aliases: &["notice"],
        properties: BANNER_PROPERTIES,
        events: events_of!("banner"),
        // Audit 20261026: `set_actions` is published now that the `actions` StringList property is
        // its payload channel (B26.03). `show` stays published — it is payload-free and runs the
        // control's real `show`.
        commands: &["dismiss", "show", "activate_action", "set_actions"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn toast_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Toast,
        canonical_name: "toast",
        aliases: &["notification", "toast_message"],
        properties: TOAST_PROPERTIES,
        events: events_of!("toast"),
        commands: &["dismiss", "set_message", "set_level"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn splash_screen_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SplashScreen,
        canonical_name: "splash_screen",
        aliases: &["splash"],
        properties: SPLASH_SCREEN_PROPERTIES,
        // Dismissal is by the program, so the events are lifecycle rather than
        // interaction: `finished` when initialisation completes, `skipped` when the
        // user takes the optional skip affordance.
        events: events_of!("splash_screen"),
        // `set_logo` carries its descriptor through the writable `logo` property, so the command
        // form is the payload-free half (B26.03).
        commands: &["finish", "set_progress", "set_title", "set_logo"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn pagination_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Pagination,
        canonical_name: "pagination",
        aliases: &["pager", "page_numbers"],
        properties: PAGINATION_PROPERTIES,
        events: events_of!("pagination"),
        commands: &["next_page", "previous_page", "set_page", "set_total"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn number_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::NumberPicker,
        canonical_name: "number_picker",
        aliases: &["picker"],
        properties: NUMBER_PICKER_PROPERTIES,
        events: events_of!("number_picker"),
        // Audit 20261026: `set_range` stays unpublished — the range is the writable
        // `minimum`/`maximum` pair, so a whole-range write is reachable through the property route
        // (principle #18).
        commands: &["scroll_rows", "set_value", "set_step"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn grid_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::GridTable,
        canonical_name: "grid_table",
        aliases: &[],
        properties: GRID_TABLE_WIDGET_PROPERTIES,
        events: events_of!("grid_table"),
        // Audit 20261026: `set_data_source` binds an `Arc<dyn IncrementalTableDataSource>` no
        // property value can carry; `toggle_sort_column` names a column and the table's sort state
        // is published as read-only derived names, so both stay unpublished (principle #18).
        commands: &["clear_data_source", "set_scroll_row", "set_scroll_column", "clear_selection"],
    }
}

pub(crate) fn image_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ImageView,
        canonical_name: "image_view",
        aliases: &["image_viewer"],
        properties: IMAGE_VIEW_PROPERTIES,
        events: events_of!("image_view"),
        // Audit 20261026: `set_image` stays unpublished — its payload is a decoded image and the
        // control publishes only the view state (`scaled`, rotation, zoom, pan), with no property
        // that carries the image itself; the control's own `set_image` API is the write path
        // (principle #18).
        commands: &["set_scaled"],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn properties_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PropertiesPanel,
        canonical_name: "properties_panel",
        aliases: &["property_panel"],
        properties: PROPERTIES_PANEL_PROPERTIES,
        events: events_of!("properties_panel"),
        commands: &[],
    }
}

#[cfg(not(alloc_frugal))]
pub(crate) fn image_gallery_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ImageGallery,
        canonical_name: "image_gallery",
        aliases: &["gallery"],
        properties: IMAGE_GALLERY_PROPERTIES,
        events: events_of!("image_gallery"),
        commands: &["set_current_index"],
    }
}

/// Property grid properties: property_count (read-only), selected_index (r/w).
#[cfg(not(alloc_frugal))]
pub(crate) fn property_grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PropertyGrid,
        canonical_name: "property_grid",
        aliases: &["inspector"],
        properties: PROPERTY_GRID_PROPERTIES,
        events: events_of!("property_grid"),
        // Audit 20261026: `set_properties` is published now that the `properties` records property
        // is its payload channel (B26.03). `add_property` and `set_value` stay unpublished — their
        // payloads (a row, an index+value) have no property route of their own; a whole-list write
        // covers both.
        commands: &["clear", "set_selected_index", "set_properties"],
    }
}

// ── Layout family ───────────────────────────────────────────────────────────
//
// The wrapper controls (`Spacer`, `Center`, `Align`, `SizedBox`, `Padding`, `Expanded`,
// `Flexible`, `IntrinsicWidth`, `AspectRatio`) and the distribution controls (`Flex`,
// `FlowLayout`, `WrapLayout`). Each owns a single- or multi-item model and positions it by the
// rule its name states; see `container_widgets::layout_family` and `container_widgets::
// linear_layouts` for why each is a control rather than a mode of another.
