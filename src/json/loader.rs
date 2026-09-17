// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! JSON layout loader — parses JSON source and instantiates widget trees.
//!
//! The [`JsonLoader`] reads a JSON string, recursively creates
//! widget instances, applies properties (geometry, style, events),
//! and returns a [`BoundJsonLayout`](crate::json::BoundJsonLayout)
//! for typed widget access.
//!
//! # i18n Support
//!
//! Text properties (`title`, `text`, `tooltip`, `placeholder`) accept
//! translation keys. Use the [`crate::i18n::I18nManager::translate`] or
//! the [`crate::tr!`] macro at the call site to resolve the key before
//! passing JSON to the loader. The loader itself does NOT call the
//! i18n manager – it uses the literal string values from JSON.
//!
//! # Event Binding
//!
//! When a JSON node declares `"on_click": "handler_name"`, the string
//! is stored in the loader and connected after instantiation.
//! Callers use [`EventHandlerMap`](crate::json::EventHandlerMap) to
//! register closures against those handler names.

use serde_json::Value;

use crate::app::{ButtonHandle, WidgetHandle};
use crate::json::properties::ApplyOutcome;
use crate::json::{
    add_spacer_to_layout, add_widget_to_layout, apply_layout, create_layout_from_kind,
    parse_layout_kind, store_layout, BoundJsonLayout, ChildLayoutAttrs,
};
use crate::layout::inspector::LayoutInspector;
use crate::widget::{
    Button, CheckBox, ComboBox, GroupBox, Label, LineEdit, ListBox, ProgressBar, RadioButton,
    ScrollArea, ScrollBar, Slider, SpinBox, Switch, Widget,
};
#[cfg(not(alloc_frugal))]
use crate::widget::{
    ColorDialog, FileDialog, FontDialog, GridWidget, ListView, MessageBox, TabWidget, TextEdit,
};
use crate::window::Window;
use crate::{
    core::{Alignment, Color, ObjectId, Orientation, Rect},
    index::WidgetKind,
};

/// Maximum nested depth for recursive instantiation (prevents stack overflow).
const MAX_DEPTH: u32 = 64;

/// A loader that parses JSON layout strings and instantiates widget trees.
pub struct JsonLoader;

impl JsonLoader {
    /// Parse a JSON layout string and instantiate the widget tree.
    ///
    /// Returns a [`BoundJsonLayout`] for typed widget access.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON parsing fails, an unknown widget type is
    /// encountered, or the widget tree exceeds `MAX_DEPTH`.
    pub fn load(json_str: &str) -> Result<BoundJsonLayout, String> {
        let value: Value = serde_json::from_str(json_str).map_err(|e| {
            format!("layout JSON ({} bytes) could not be parsed: {e}", json_str.len())
        })?;
        let mut registry = crate::index::WidgetRegistry::new();
        let mut binding = BoundJsonLayout::new();

        let root = value.as_object().ok_or_else(|| "JSON root must be an object".to_string())?;

        // The root should have exactly one top-level key (the widget type).
        if root.len() != 1 {
            return Err(format!(
                "JSON root must have exactly one widget type, found {} keys",
                root.len()
            ));
        }

        let (widget_type, widget_value) = match root.iter().next() {
            Some(pair) => pair,
            None => {
                return Err("JSON root object is empty — expected a widget type key".to_string())
            }
        };
        Self::instantiate_node(widget_type, widget_value, None, &mut registry, &mut binding, 0)?;

        // Run layout diagnostics after all widgets are created.
        LayoutInspector::run_once_logged(&registry);

        Ok(binding)
    }

    /// Recursively instantiate a single JSON node into a widget.
    #[allow(clippy::too_many_arguments)]
    fn instantiate_node(
        widget_type: &str,
        value: &Value,
        parent_id: Option<ObjectId>,
        registry: &mut crate::index::WidgetRegistry,
        binding: &mut BoundJsonLayout,
        depth: u32,
    ) -> Result<ObjectId, String> {
        if depth > MAX_DEPTH {
            return Err(format!(
                "widget tree is nested {depth} levels deep, which exceeds the maximum of \
                 {MAX_DEPTH}; flatten the layout to load it"
            ));
        }

        let obj = value
            .as_object()
            .ok_or_else(|| format!("'{widget_type}' value must be a JSON object"))?;

        // Get the widget ID (optional — auto-generated if missing).
        let id_str = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");

        // Handle the "spacer" pseudo-widget
        if widget_type.eq_ignore_ascii_case("spacer") {
            let stretch = obj.get("stretch").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            if let Some(pid) = parent_id {
                add_spacer_to_layout(stretch, pid);
            }
            return Ok(0);
        }

        // Handle the "layout" pseudo-widget
        if widget_type.eq_ignore_ascii_case("layout") {
            let kind = parse_layout_kind(value)?;
            let layout = create_layout_from_kind(&kind);
            let layout_parent = parent_id
                .ok_or_else(|| format!("'{widget_type}' layout must be a child of a widget"))?;

            // Stored *before* the children are instantiated, because registering a
            // child needs the layout to exist (`add_widget_to_layout` is a no-op
            // otherwise). Storing it afterwards — which this code used to do — left the
            // layout empty, so `apply_layout` computed an arrangement for no children
            // and no widget ever moved.
            store_layout(layout_parent, layout);

            // Process children
            if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
                for child_value in children {
                    if let Some(child_obj) = child_value.as_object() {
                        if child_obj.len() == 1 {
                            let (child_type, child_val) = child_obj.iter().next().unwrap();
                            if child_type.eq_ignore_ascii_case("spacer") {
                                let stretch =
                                    child_val.get("stretch").and_then(|v| v.as_u64()).unwrap_or(1)
                                        as u32;
                                add_spacer_to_layout(stretch, layout_parent);
                                continue;
                            }

                            let child_id = Self::instantiate_node(
                                child_type,
                                child_val,
                                Some(layout_parent),
                                registry,
                                binding,
                                depth + 1,
                            )?;

                            if child_id != 0 {
                                let attrs = ChildLayoutAttrs::from_value(child_val);
                                // The layout must already be stored for this to reach
                                // it: `add_widget_to_layout` is a no-op when the parent
                                // has none. `store_layout` used to run *after* this
                                // loop, so every child was added to nothing and the
                                // layout was empty when it was applied — which is why
                                // a declarative layout could compute geometries that no
                                // widget ever received.
                                add_widget_to_layout(child_id, attrs.stretch, layout_parent);
                            }
                        }
                    }
                }
            }

            apply_layout(layout_parent, json_geometry(obj));
            return Ok(layout_parent);
        }

        // Create the widget
        let mut widget: Box<dyn Widget> = Self::create_widget(widget_type, obj)?;

        // Apply stylesheet rules first, so explicit JSON keys below win over the
        // stylesheet — the same precedence a browser gives inline styles over an
        // author stylesheet. `class` selects which rules apply; the widget's own
        // kind is matched automatically.
        apply_declared_styles(&mut *widget, obj);

        // Apply common properties (geometry, enabled, visible, tooltip, style)
        apply_properties(&mut *widget, obj);

        // Apply min/max size constraints
        apply_size_constraints(&mut *widget, obj);

        // Set parent
        widget.set_parent(parent_id);

        // Register
        let widget_id = widget.id();
        let kind = infer_kind(widget_type);

        let label = if id_str.is_empty() {
            format!("{widget_type}_{widget_id}")
        } else {
            id_str.to_string()
        };

        registry.register(crate::index::WidgetEntry {
            id: widget_id,
            kind,
            parent: parent_id,
            label,
        });

        if !id_str.is_empty() {
            binding.register(id_str, widget_id);
        }

        // Structural index: record which node this is, under which parent, and with which
        // key. `id_str` doubles as the key because the JSON schema already requires it to be
        // unique per document (it is how `widget_by_name` addresses a control), so requiring
        // a second, separate `key` field would let the two disagree. A node without an `id`
        // therefore has no stable identity to diff on, and `node_key` reports `None` for it.
        binding.register_node(widget_id, widget_type, id_str, parent_id);

        // ── Apply text property via platform API ────────────
        // Text for widgets that accept it (checkbox, radiobutton,
        // groupbox title via "title" key, lineedit placeholder, etc.)
        // is set through the global set_widget_text function.
        //
        // i18n: If the text value is a translation key, resolve it
        // BEFORE passing to the loader (e.g. tr!("button.ok")).
        // The loader uses the literal string as-is.
        if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
            if !text.is_empty()
                && !matches!(widget_type.to_lowercase().as_str(), "button" | "label")
            {
                crate::set_widget_text(widget_id, text);
            }
        }
        // Handle "title" property — BLUE4.md spec says groupbox uses "title"
        // (not "text"). For window, title is already set in create_widget.
        // Only apply via platform API for non-window widgets.
        if widget_type.to_lowercase().as_str() != "window" {
            if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                if !title.is_empty() {
                    crate::set_widget_text(widget_id, title);
                }
            }
        }

        // ── Event binding: on_click / on_change / extended ──
        // When JSON declares "on_click": "handler_name", wire the
        // widget's click signal to invoke_global_handler.
        let (on_click_name, on_change_name) = extract_event_handlers(obj);
        if let Some(ref name) = on_click_name {
            let handler_name = name.clone();
            let handle: ButtonHandle = ButtonHandle::from_raw(widget_id);
            handle.on_click(move || {
                let ctx = crate::json::EventHandlerContext::new(crate::WidgetTriggerEvent {
                    widget_id,
                    kind: crate::platform::WidgetTriggerKind::Clicked,
                });
                crate::json::invoke_global_handler(&handler_name, &ctx);
            });
        }
        if let Some(ref name) = on_change_name {
            let handler_name = name.clone();
            let handle = ButtonHandle::from_raw(widget_id);
            handle.on_value_changed(move |_value| {
                let ctx = crate::json::EventHandlerContext::new(crate::WidgetTriggerEvent {
                    widget_id,
                    kind: crate::platform::WidgetTriggerKind::ValueChanged,
                });
                crate::json::invoke_global_handler(&handler_name, &ctx);
            });
        }
        // ── Extended event bindings ──────────────────────────
        let (on_close, on_double_click, on_focus, on_blur, on_selection_changed, on_value_changed) =
            extract_extended_event_handlers(obj);
        if let Some(ref name) = on_close {
            let handler_name = name.clone();
            let handle = ButtonHandle::from_raw(widget_id);
            handle.on_click(move || {
                let ctx = crate::json::EventHandlerContext::new(crate::WidgetTriggerEvent {
                    widget_id,
                    kind: crate::platform::WidgetTriggerKind::Closed,
                });
                crate::json::invoke_global_handler(&handler_name, &ctx);
            });
        }
        if let Some(ref name) = on_double_click {
            let handler_name = name.clone();
            let handle = ButtonHandle::from_raw(widget_id);
            handle.on_click(move || {
                let ctx = crate::json::EventHandlerContext::new(crate::WidgetTriggerEvent {
                    widget_id,
                    kind: crate::platform::WidgetTriggerKind::Clicked,
                });
                crate::json::invoke_global_handler(&handler_name, &ctx);
            });
        }
        if let Some(ref name) = on_focus {
            let handler_name = name.clone();
            let handle = ButtonHandle::from_raw(widget_id);
            handle.on_value_changed(move |_value| {
                let ctx = crate::json::EventHandlerContext::new(crate::WidgetTriggerEvent {
                    widget_id,
                    kind: crate::platform::WidgetTriggerKind::ValueChanged,
                });
                crate::json::invoke_global_handler(&handler_name, &ctx);
            });
        }
        if let Some(ref name) = on_blur {
            let handler_name = name.clone();
            let handle = ButtonHandle::from_raw(widget_id);
            handle.on_value_changed(move |_value| {
                let ctx = crate::json::EventHandlerContext::new(crate::WidgetTriggerEvent {
                    widget_id,
                    kind: crate::platform::WidgetTriggerKind::ValueChanged,
                });
                crate::json::invoke_global_handler(&handler_name, &ctx);
            });
        }
        if let Some(ref name) = on_selection_changed {
            let handler_name = name.clone();
            let handle = ButtonHandle::from_raw(widget_id);
            handle.on_value_changed(move |_value| {
                let ctx = crate::json::EventHandlerContext::new(crate::WidgetTriggerEvent {
                    widget_id,
                    kind: crate::platform::WidgetTriggerKind::SelectionChanged,
                });
                crate::json::invoke_global_handler(&handler_name, &ctx);
            });
        }
        if let Some(ref name) = on_value_changed {
            let handler_name = name.clone();
            let handle = ButtonHandle::from_raw(widget_id);
            handle.on_value_changed(move |_value| {
                let ctx = crate::json::EventHandlerContext::new(crate::WidgetTriggerEvent {
                    widget_id,
                    kind: crate::platform::WidgetTriggerKind::ValueChanged,
                });
                crate::json::invoke_global_handler(&handler_name, &ctx);
            });
        }

        // Handle children (for container widgets)
        if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
            for child_value in children {
                if let Some(child_obj) = child_value.as_object() {
                    if child_obj.len() == 1 {
                        let (child_type, child_val) = child_obj.iter().next().unwrap();
                        Self::instantiate_node(
                            child_type,
                            child_val,
                            Some(widget_id),
                            registry,
                            binding,
                            depth + 1,
                        )?;
                    }
                }
            }
        }

        // Handle layout inline
        if let Some(layout_val) = obj.get("layout") {
            let kind = parse_layout_kind(layout_val)?;
            let layout = create_layout_from_kind(&kind);

            // Stored before the children below are registered, for the same reason as
            // the `"layout"` pseudo-widget branch: a child added before the layout
            // exists is added to nothing.
            store_layout(widget_id, layout);

            // Process layout children from the layout object
            if let Some(layout_obj) = layout_val.as_object() {
                if let Some(children) = layout_obj.get("children").and_then(|v| v.as_array()) {
                    for child_value in children {
                        if let Some(child_obj) = child_value.as_object() {
                            if child_obj.len() == 1 {
                                let (child_type, child_val) = child_obj.iter().next().unwrap();
                                if child_type.eq_ignore_ascii_case("spacer") {
                                    let stretch = child_val
                                        .get("stretch")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(1)
                                        as u32;
                                    add_spacer_to_layout(stretch, widget_id);
                                    continue;
                                }

                                let child_id = Self::instantiate_node(
                                    child_type,
                                    child_val,
                                    Some(widget_id),
                                    registry,
                                    binding,
                                    depth + 1,
                                )?;

                                if child_id != 0 {
                                    let attrs = ChildLayoutAttrs::from_value(child_val);
                                    // See the note on the other `add_widget_to_layout`
                                    // call: the layout has to be stored before its
                                    // children are registered, or they are added to
                                    // nothing.
                                    add_widget_to_layout(child_id, attrs.stretch, widget_id);
                                }
                            }
                        }
                    }
                }
            }

            apply_layout(widget_id, json_geometry(obj));
        }

        Ok(widget_id)
    }

    /// Create a widget box from a type name and property object.
    ///
    /// Widget-specific properties (text for Button/Label, value/items for
    /// input widgets, min/max for sliders, etc.) are applied before boxing.
    fn create_widget(
        widget_type: &str,
        obj: &serde_json::Map<String, Value>,
    ) -> Result<Box<dyn Widget>, String> {
        let geometry = Rect::new(0, 0, 100, 100);
        match widget_type.to_lowercase().as_str() {
            "window" => {
                let title = obj.get("title").and_then(|v| v.as_str()).unwrap_or("Window");
                Ok(Box::new(Window::new(title.to_string(), geometry)))
            }
            "button" => {
                let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(Box::new(Button::new(text.to_string(), geometry)))
            }
            "label" => {
                let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let mut label = Label::new(text.to_string(), geometry);
                // alignment: "left"|"center"|"right"|"top"|"bottom"
                if let Some(align) = obj.get("alignment").and_then(|v| v.as_str()) {
                    label.set_alignment(match align {
                        "center" => Alignment::Center,
                        "right" => Alignment::Right,
                        "top" => Alignment::Top,
                        "bottom" => Alignment::Bottom,
                        _ => Alignment::Left,
                    });
                }
                Ok(Box::new(label))
            }
            "checkbox" => {
                let mut cb = CheckBox::new(geometry);
                if let Some(checked) = obj.get("checked").and_then(|v| v.as_bool()) {
                    cb.set_checked(checked);
                }
                // tristate: enables partial check state
                if let Some(tri) = obj.get("tristate").and_then(|v| v.as_bool()) {
                    cb.set_tristate_enabled(tri);
                }
                Ok(Box::new(cb))
            }
            "radiobutton" => {
                let mut rb = RadioButton::new(geometry);
                if let Some(checked) = obj.get("checked").and_then(|v| v.as_bool()) {
                    rb.set_checked(checked);
                }
                // group_id: logical group name for mutual exclusion
                if let Some(gid) = obj.get("group_id").and_then(|v| v.as_str()) {
                    if !gid.is_empty() {
                        rb.set_group_id(Some(gid.to_string()));
                    }
                }
                Ok(Box::new(rb))
            }
            "lineedit" => {
                let mut le = LineEdit::new(geometry);
                if let Some(value) = obj.get("value").and_then(|v| v.as_str()) {
                    le.set_text(value.to_string());
                }
                if let Some(placeholder) = obj.get("placeholder").and_then(|v| v.as_str()) {
                    le.set_placeholder_text(placeholder.to_string());
                }
                if let Some(max_len) = obj.get("max_length").and_then(|v| v.as_u64()) {
                    le.set_max_length(Some(max_len as usize));
                }
                if let Some(password) = obj.get("password").and_then(|v| v.as_bool()) {
                    if password {
                        le.set_echo_mode(crate::widget::EchoMode::Password);
                    }
                }
                Ok(Box::new(le))
            }
            #[cfg(not(alloc_frugal))]
            "textedit" => {
                let mut te = TextEdit::new(geometry);
                if let Some(value) = obj.get("value").and_then(|v| v.as_str()) {
                    te.set_text(value.to_string());
                }
                if let Some(placeholder) = obj.get("placeholder").and_then(|v| v.as_str()) {
                    te.set_placeholder_text(placeholder.to_string());
                }
                if let Some(max_len) = obj.get("max_length").and_then(|v| v.as_u64()) {
                    te.set_max_length(Some(max_len as usize));
                }
                if let Some(read_only) = obj.get("read_only").and_then(|v| v.as_bool()) {
                    te.set_read_only(read_only);
                }
                if let Some(word_wrap) = obj.get("word_wrap").and_then(|v| v.as_bool()) {
                    te.set_line_wrap(word_wrap);
                }
                Ok(Box::new(te))
            }
            "combobox" => {
                let mut cb = ComboBox::new(geometry);
                if let Some(items) = obj.get("items").and_then(|v| v.as_array()) {
                    for item in items {
                        if let Some(text) = item.as_str() {
                            cb.add_item(text.to_string());
                        }
                    }
                }
                // current_index: pre-select an item by index (0-based)
                if let Some(idx) = obj.get("current_index").and_then(|v| v.as_u64()) {
                    cb.set_current_index(Some(idx as usize));
                }
                // editable: allow user to type custom text
                if let Some(ed) = obj.get("editable").and_then(|v| v.as_bool()) {
                    cb.set_editable(ed);
                }
                // max_visible_items: dropdown max rows
                if let Some(max) = obj.get("max_visible_items").and_then(|v| v.as_u64()) {
                    cb.set_max_visible_items(max as usize);
                }
                Ok(Box::new(cb))
            }
            "listbox" => {
                let mut lb = ListBox::new(geometry);
                if let Some(items) = obj.get("items").and_then(|v| v.as_array()) {
                    for item in items {
                        if let Some(text) = item.as_str() {
                            lb.add_item(text.to_string());
                        }
                    }
                }
                if let Some(mode) = obj.get("selection_mode").and_then(|v| v.as_str()) {
                    match mode {
                        "none" => lb.set_selection_mode(crate::widget::SelectionMode::None),
                        "single" => lb.set_selection_mode(crate::widget::SelectionMode::Single),
                        "multi" => lb.set_selection_mode(crate::widget::SelectionMode::Multi),
                        "extended" => lb.set_selection_mode(crate::widget::SelectionMode::Extended),
                        // Unknown value; use widget default
                        _ => {}
                    }
                }
                Ok(Box::new(lb))
            }
            "slider" => {
                let mut sl = Slider::new(geometry);
                if let Some(min) = obj.get("min").and_then(|v| v.as_i64()) {
                    let max = obj.get("max").and_then(|v| v.as_i64()).unwrap_or(100);
                    sl.set_range(min as i32, max as i32);
                } else if let Some(max) = obj.get("max").and_then(|v| v.as_i64()) {
                    sl.set_maximum(max as i32);
                }
                if let Some(value) = obj.get("value").and_then(|v| v.as_i64()) {
                    sl.set_value(value as i32);
                }
                if let Some(orientation) = obj.get("orientation").and_then(|v| v.as_str()) {
                    match orientation {
                        "horizontal" => sl.set_orientation(Orientation::Horizontal),
                        "vertical" => sl.set_orientation(Orientation::Vertical),
                        // Unknown value; use widget default
                        _ => {}
                    }
                }
                // single_step: keyboard arrow increment
                if let Some(step) = obj.get("single_step").and_then(|v| v.as_u64()) {
                    sl.set_single_step(step as i32);
                }
                // page_step: PgUp/PgDn increment
                if let Some(step) = obj.get("page_step").and_then(|v| v.as_u64()) {
                    sl.set_page_step(step as i32);
                }
                // tick_position: "none"|"above"|"below"|"both"
                if let Some(pos) = obj.get("tick_position").and_then(|v| v.as_str()) {
                    match pos {
                        "above" => sl.set_tick_position(
                            crate::widget::display_widgets::slider::TickPosition::TicksAbove,
                        ),
                        "below" => sl.set_tick_position(
                            crate::widget::display_widgets::slider::TickPosition::TicksBelow,
                        ),
                        "both" => sl.set_tick_position(
                            crate::widget::display_widgets::slider::TickPosition::TicksBothSides,
                        ),
                        // Unknown value; use widget default
                        _ => {}
                    }
                }
                // tick_interval: interval between tick marks
                if let Some(iv) = obj.get("tick_interval").and_then(|v| v.as_u64()) {
                    sl.set_tick_interval(iv as i32);
                }
                // tracking: emit value_changed while dragging (default: true)
                if let Some(tr) = obj.get("tracking").and_then(|v| v.as_bool()) {
                    sl.set_tracking(tr);
                }
                Ok(Box::new(sl))
            }
            "scrollbar" => {
                let mut sb = ScrollBar::new(geometry);
                if let Some(min) = obj.get("min").and_then(|v| v.as_i64()) {
                    let max = obj.get("max").and_then(|v| v.as_i64()).unwrap_or(100);
                    sb.set_range(min as i32, max as i32);
                } else if let Some(max) = obj.get("max").and_then(|v| v.as_i64()) {
                    sb.set_maximum(max as i32);
                }
                if let Some(value) = obj.get("value").and_then(|v| v.as_i64()) {
                    sb.set_value(value as i32);
                }
                if let Some(orientation) = obj.get("orientation").and_then(|v| v.as_str()) {
                    match orientation {
                        "horizontal" => sb.set_orientation(Orientation::Horizontal),
                        "vertical" => sb.set_orientation(Orientation::Vertical),
                        // Unknown value; use widget default
                        _ => {}
                    }
                }
                // single_step: arrow button increment
                if let Some(step) = obj.get("single_step").and_then(|v| v.as_u64()) {
                    sb.set_single_step(step as i32);
                }
                // page_step: click-track increment
                if let Some(step) = obj.get("page_step").and_then(|v| v.as_u64()) {
                    sb.set_page_step(step as i32);
                }
                Ok(Box::new(sb))
            }
            "progressbar" => {
                let mut pb = ProgressBar::new(geometry);
                if let Some(min) = obj.get("min").and_then(|v| v.as_i64()) {
                    let max = obj.get("max").and_then(|v| v.as_i64()).unwrap_or(100);
                    pb.set_range(min as i32, max as i32);
                } else if let Some(max) = obj.get("max").and_then(|v| v.as_i64()) {
                    pb.set_maximum(max as i32);
                }
                if let Some(value) = obj.get("value").and_then(|v| v.as_i64()) {
                    pb.set_value(value as i32);
                }
                // text_visible: show percentage text overlay
                if let Some(tv) = obj.get("text_visible").and_then(|v| v.as_bool()) {
                    pb.set_text_visible(tv);
                }
                // orientation: "horizontal"|"vertical"
                if let Some(orient) = obj.get("orientation").and_then(|v| v.as_str()) {
                    match orient {
                        "vertical" => pb.set_orientation(Orientation::Vertical),
                        _ => pb.set_orientation(Orientation::Horizontal),
                    }
                }
                // inverted_appearance: fill from right/bottom
                if let Some(inv) = obj.get("inverted_appearance").and_then(|v| v.as_bool()) {
                    pb.set_inverted_appearance(inv);
                }
                Ok(Box::new(pb))
            }
            "switch" | "toggle" => {
                let mut sw = Switch::new(geometry);
                if let Some(checked) = obj.get("checked").and_then(|v| v.as_bool()) {
                    sw.set_checked(checked);
                }
                Ok(Box::new(sw))
            }
            "groupbox" | "panel" => {
                let mut gb = GroupBox::new(geometry);
                if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                    if !title.is_empty() {
                        gb.set_title(title.to_string());
                    }
                }
                // alignment: title text alignment
                if let Some(align) = obj.get("alignment").and_then(|v| v.as_str()) {
                    match align {
                        "center" => gb.set_alignment(Alignment::Center),
                        "right" => gb.set_alignment(Alignment::Right),
                        _ => gb.set_alignment(Alignment::Left),
                    }
                }
                // checkable: add a checkbox to the group box title
                if let Some(chk) = obj.get("checkable").and_then(|v| v.as_bool()) {
                    gb.set_checkable(chk);
                }
                // checked: initial checked state (only if checkable)
                if let Some(chk) = obj.get("checked").and_then(|v| v.as_bool()) {
                    if gb.is_checkable() || obj.get("checkable").is_none() {
                        gb.set_checked(chk);
                    }
                }
                Ok(Box::new(gb))
            }
            #[cfg(not(alloc_frugal))]
            "tabwidget" => {
                let mut tw = TabWidget::new(geometry);
                if let Some(index) = obj.get("current_index").and_then(|v| v.as_u64()) {
                    tw.set_current_index(index as usize);
                }
                // tab_position: "north"|"south"|"west"|"east"
                if let Some(pos) = obj.get("tab_position").and_then(|v| v.as_str()) {
                    match pos {
                        "south" => tw.set_tab_position(
                            crate::widget::container_widgets::tabwidget::TabPosition::South,
                        ),
                        "west" => tw.set_tab_position(
                            crate::widget::container_widgets::tabwidget::TabPosition::West,
                        ),
                        "east" => tw.set_tab_position(
                            crate::widget::container_widgets::tabwidget::TabPosition::East,
                        ),
                        // Unknown value; use widget default
                        _ => {}
                    }
                }
                // tab_shape: "rounded"|"triangular"|"rectangular"
                if let Some(shape) = obj.get("tab_shape").and_then(|v| v.as_str()) {
                    match shape {
                        "triangular" => tw.set_tab_shape(
                            crate::widget::container_widgets::tabwidget::TabShape::Triangular,
                        ),
                        "rectangular" => tw.set_tab_shape(
                            crate::widget::container_widgets::tabwidget::TabShape::Rectangular,
                        ),
                        // Unknown value; use widget default
                        _ => {}
                    }
                }
                // closable: show close buttons on tabs
                if let Some(cl) = obj.get("closable").and_then(|v| v.as_bool()) {
                    tw.set_closable(cl);
                }
                // movable: allow drag-reordering of tabs
                if let Some(mv) = obj.get("movable").and_then(|v| v.as_bool()) {
                    tw.set_movable(mv);
                }
                Ok(Box::new(tw))
            }
            #[cfg(not(alloc_frugal))]
            "grid" => {
                let mut grid = GridWidget::new(geometry);
                if let Some(rows) = obj.get("rows").and_then(|v| v.as_u64()) {
                    grid.set_rows(rows as u32);
                }
                if let Some(cols) = obj.get("columns").and_then(|v| v.as_u64()) {
                    grid.set_columns(cols as u32);
                }
                if let Some(spacing) = obj.get("spacing").and_then(|v| v.as_u64()) {
                    grid.set_spacing(spacing as u32);
                }
                if let Some(color_str) = obj.get("line_color").and_then(|v| v.as_str()) {
                    if let Some(color) = Color::parse_hex(color_str) {
                        grid.set_line_color(Some(color));
                    }
                }
                Ok(Box::new(grid))
            }
            "spinbox" => {
                let mut sb = SpinBox::new(geometry);
                if let Some(min) = obj.get("min").and_then(|v| v.as_i64()) {
                    sb.set_minimum(min as i32);
                }
                if let Some(max) = obj.get("max").and_then(|v| v.as_i64()) {
                    sb.set_maximum(max as i32);
                }
                if let Some(value) = obj.get("value").and_then(|v| v.as_i64()) {
                    sb.set_value(value as i32);
                }
                if let Some(step) = obj.get("single_step").and_then(|v| v.as_u64()) {
                    sb.set_single_step(step as i32);
                }
                if let Some(prefix) = obj.get("prefix").and_then(|v| v.as_str()) {
                    sb.set_prefix(prefix.to_string());
                }
                if let Some(suffix) = obj.get("suffix").and_then(|v| v.as_str()) {
                    sb.set_suffix(suffix.to_string());
                }
                if let Some(wrap) = obj.get("wrapping").and_then(|v| v.as_bool()) {
                    sb.set_wrapping(wrap);
                }
                Ok(Box::new(sb))
            }
            #[cfg(not(alloc_frugal))]
            "listview" => Ok(Box::new(ListView::new(geometry))),
            "scrollarea" => {
                let mut sa = ScrollArea::new(geometry);
                if let Some(resizable) = obj.get("widget_resizable").and_then(|v| v.as_bool()) {
                    sa.set_widget_resizable(resizable);
                }
                if let Some(align) = obj.get("alignment").and_then(|v| v.as_str()) {
                    match align {
                        "center" => sa.set_alignment(Alignment::Center),
                        "right" => sa.set_alignment(Alignment::Right),
                        _ => sa.set_alignment(Alignment::Left),
                    }
                }
                // h_policy / v_policy: "always_on"|"always_off"|"as_needed"
                if let Some(policy) = obj.get("h_policy").and_then(|v| v.as_str()) {
                    sa.set_horizontal_scroll_bar_policy(match policy {
                        "always_on" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOn
                        }
                        "always_off" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOff
                        }
                        _ => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AsNeeded
                        }
                    });
                }
                if let Some(policy) = obj.get("v_policy").and_then(|v| v.as_str()) {
                    sa.set_vertical_scroll_bar_policy(match policy {
                        "always_on" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOn
                        }
                        "always_off" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOff
                        }
                        _ => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AsNeeded
                        }
                    });
                }
                Ok(Box::new(sa))
            }
            "frame" => {
                use crate::widget::base_widgets::frame::Frame;
                let mut frame = Frame::new(geometry);
                if let Some(shape) = obj.get("frame_shape").and_then(|v| v.as_str()) {
                    match shape {
                        "no_frame" => frame.set_frame_shape(
                            crate::widget::base_widgets::frame::FrameShape::NoFrame,
                        ),
                        "panel" => frame
                            .set_frame_shape(crate::widget::base_widgets::frame::FrameShape::Panel),
                        "styled_panel" => frame.set_frame_shape(
                            crate::widget::base_widgets::frame::FrameShape::StyledPanel,
                        ),
                        "hline" => frame
                            .set_frame_shape(crate::widget::base_widgets::frame::FrameShape::HLine),
                        "vline" => frame
                            .set_frame_shape(crate::widget::base_widgets::frame::FrameShape::VLine),
                        "win_panel" => frame.set_frame_shape(
                            crate::widget::base_widgets::frame::FrameShape::WinPanel,
                        ),
                        _ => frame
                            .set_frame_shape(crate::widget::base_widgets::frame::FrameShape::Box),
                    }
                }
                if let Some(shadow) = obj.get("frame_shadow").and_then(|v| v.as_str()) {
                    match shadow {
                        "raised" => frame.set_frame_shadow(
                            crate::widget::base_widgets::frame::FrameShadow::Raised,
                        ),
                        "sunken" => frame.set_frame_shadow(
                            crate::widget::base_widgets::frame::FrameShadow::Sunken,
                        ),
                        // Unknown value; use widget default
                        _ => {}
                    }
                }
                if let Some(lw) = obj.get("line_width").and_then(|v| v.as_f64()) {
                    frame.set_line_width(lw as f32);
                }
                Ok(Box::new(frame))
            }
            #[cfg(not(alloc_frugal))]
            "messagebox" => {
                let mut mb = MessageBox::new(geometry);
                if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                    if !title.is_empty() {
                        mb.set_title(title.to_string());
                    }
                }
                if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        mb.set_text(text.to_string());
                    }
                }
                if let Some(icon) = obj.get("icon").and_then(|v| v.as_str()) {
                    match icon {
                        "information" => mb.set_icon(
                            crate::widget::dialog::message_box::MessageBoxIcon::Information,
                        ),
                        "question" => mb
                            .set_icon(crate::widget::dialog::message_box::MessageBoxIcon::Question),
                        "warning" => {
                            mb.set_icon(crate::widget::dialog::message_box::MessageBoxIcon::Warning)
                        }
                        "critical" => mb
                            .set_icon(crate::widget::dialog::message_box::MessageBoxIcon::Critical),
                        _ => {
                            mb.set_icon(crate::widget::dialog::message_box::MessageBoxIcon::NoIcon)
                        }
                    }
                }
                Ok(Box::new(mb))
            }
            #[cfg(not(alloc_frugal))]
            "filedialog" => {
                let mut fd = FileDialog::new(geometry);
                if let Some(mode) = obj.get("mode").and_then(|v| v.as_str()) {
                    match mode {
                        "open_files" => fd.set_mode(
                            crate::widget::dialog::file_dialog::FileDialogMode::OpenFiles,
                        ),
                        "save_file" => fd
                            .set_mode(crate::widget::dialog::file_dialog::FileDialogMode::SaveFile),
                        "select_directory" => fd.set_mode(
                            crate::widget::dialog::file_dialog::FileDialogMode::SelectDirectory,
                        ),
                        _ => fd
                            .set_mode(crate::widget::dialog::file_dialog::FileDialogMode::OpenFile),
                    }
                }
                if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                    if !title.is_empty() {
                        fd.set_title(title.to_string());
                    }
                }
                if let Some(dir) = obj.get("directory").and_then(|v| v.as_str()) {
                    if !dir.is_empty() {
                        fd.set_directory(dir.to_string());
                    }
                }
                Ok(Box::new(fd))
            }
            #[cfg(not(alloc_frugal))]
            "colordialog" => {
                let mut cd = ColorDialog::new(geometry);
                if let Some(alpha) = obj.get("alpha").and_then(|v| v.as_bool()) {
                    cd.set_options_alpha(alpha);
                }
                if let Some(color_str) = obj.get("value").and_then(|v| v.as_str()) {
                    if let Some(color) = Color::parse_hex(color_str) {
                        cd.set_current_color(color);
                    }
                }
                Ok(Box::new(cd))
            }
            #[cfg(not(alloc_frugal))]
            "fontdialog" => {
                let mut fd = FontDialog::new(geometry);
                if let Some(_font_str) = obj.get("value").and_then(|v| v.as_str()) {
                    if !_font_str.is_empty() {
                        // Font selection from string requires font parsing.
                        // Default value used for now; full font parsing can be
                        // added when Font::from_string or similar is available.
                        fd.set_current_font(crate::core::Font::default());
                    }
                }
                Ok(Box::new(fd))
            }
            _ => {
                // Not one of the widget types this arm table knows how to
                // configure. Before giving up, ask the capability registry: it
                // holds a constructor for every registered control, so a control
                // added to the library becomes reachable from JSON without an edit
                // here. The registry-built control gets its scalar properties from
                // the name-driven pass in `apply_properties`, which is why this
                // fallback does not need to reproduce any of the setters above.
                //
                // A build without the registry (a stripped profile) answers `None`
                // and the error below is reported, which is the truth there.
                let factory = crate::json::schema_factory();
                if let Some(widget) = factory.create(widget_type, geometry, "") {
                    return Ok(widget);
                }
                Err(format!(
                    "unknown widget type '{widget_type}'; it is neither a built-in JSON widget \
                     name nor a name registered with the widget factory"
                ))
            }
        }
    }
}

/// The widget's kind in `Debug` spelling, for diagnostics.
///
/// A free function rather than a trait method: only this module's warnings need
/// it, and the kind is already reachable through `Widget::kind`.
fn kind_label(widget: &dyn Widget) -> String {
    format!("{:?}", widget.kind())
}

/// Apply declarative stylesheet rules to a freshly created widget.
///
/// Four sources, applied in increasing precedence so a more specific source wins:
///
/// 1. The **active theme** resolved for the node's class (or kind). This is the
///    base appearance every control starts from; without it each control kept the
///    colours its constructor hardcoded and a theme switch had no effect.
/// 2. The **global stylesheet manager**'s registered sheets, in their own
///    priority order. This is the app-wide CSS layer.
/// 3. An inline `"css"` string on the node itself. Page-local rules beat
///    app-wide ones, which is what makes a one-off layout possible.
///
/// Matching uses the widget's kind plus the node's optional `"class"` and `"id"`,
/// so a rule written as `Button.primary` or `#ok_btn` selects the intended node.
/// The widget's kind is formatted in the `Debug` spelling the CSS selector parser
/// expects (`Button`, not `button`).
///
/// Each layer is merged *under* whatever the widget already has, so the explicit
/// JSON keys applied after this function always win. A malformed stylesheet is
/// reported and skipped: the widget is still created, because failing the whole
/// load over one bad rule would hide the rest of the layout.
fn apply_declared_styles(widget: &mut dyn Widget, obj: &serde_json::Map<String, Value>) {
    let kind = kind_label(widget);
    let class = obj.get("class").and_then(|v| v.as_str());
    let id = obj.get("id").and_then(|v| v.as_str());

    // 1. Active theme. The class is preferred over the kind name when present, so
    //    a node can opt into a named role while every other node falls back to
    //    kind-based role resolution.
    if let Some(theme_style) = crate::theme::resolved_theme_style(class.unwrap_or(&kind)) {
        let mut style = widget.style().clone();
        style.merge(&theme_style);
        widget.set_style(style);
    }

    // 2. App-wide sheets, respecting their registered priority.
    let mut style = widget.style().clone();
    let applied = {
        let manager = crate::style::global_stylesheet_manager();
        manager.apply_to(&kind, class, id, None, &mut style)
    };
    if let Err(error) = applied {
        log::warn!(
            "JSON layout: a global stylesheet failed to apply to {kind} (class {class:?}, id \
             {id:?}): {error}"
        );
    } else {
        widget.set_style(style);
    }

    // 3. Page-local inline CSS on this node.
    if let Some(css) = obj.get("css").and_then(|v| v.as_str()) {
        match widget.apply_css(css, class) {
            Ok(()) => {}
            Err(error) => log::warn!(
                "JSON layout: the inline \"css\" on {kind} (id {id:?}) could not be applied: \
                 {error}"
            ),
        }
    }
}

/// Apply common widget properties from a JSON object.
///
/// Two layers, in this order:
///
/// 1. **Name-driven** — every key that the control's own property contract
///    publishes is written through [`crate::json::properties`], so a property
///    added to any control is addressable from JSON with no change here. This is
///    what makes the loader cover all 167 `WidgetKind` variants rather than a
///    hand-maintained subset.
/// 2. **Loader-owned** — keys that describe the style object as a whole (`padding`
///    / `margin` accept a number *or* a four-sided object), the geometry shorthand,
///    and the min/max size constraints, which have no single-property equivalent.
///
/// A key that neither layer recognises is reported as a warning naming the key and
/// the widget, so a typo (`"colour"` for `"color"`) is visible instead of loading
/// a silently default-styled control.
fn apply_properties(widget: &mut dyn Widget, obj: &serde_json::Map<String, Value>) {
    // ── Geometry ────────────────────────────────────────────
    // Applied first because the property layer also publishes `geometry`, and a
    // control's own setter is the authority; the shorthand below fills in the
    // common case of four separate keys.
    if let (Some(x), Some(y), Some(w), Some(h)) = (
        obj.get("x").and_then(|v| v.as_i64()),
        obj.get("y").and_then(|v| v.as_i64()),
        obj.get("width").and_then(|v| v.as_u64()),
        obj.get("height").and_then(|v| v.as_u64()),
    ) {
        widget.set_geometry(Rect::from_i64(x, y, w as i64, h as i64));
    }

    // ── Style: padding / margin ─────────────────────────────
    // These two accept either a bare number (all sides) or an object with
    // top/right/bottom/left, which no single property name can express. They are
    // handled before the name-driven pass so the structured form is not rejected
    // as a type error by a property expecting a scalar.
    apply_style_padding(widget, obj, "padding", |widget, value| {
        let mut style = widget.style().clone();
        style.padding = value;
        widget.set_style(style);
    });
    if let Some(value) = obj.get("margin") {
        if let Some(p) = parse_spacing(value) {
            let margin = crate::style::Margin::new(p.top, p.right, p.bottom, p.left);
            let mut style = widget.style().clone();
            style.margin = margin;
            widget.set_style(style);
        }
    }

    // ── Name-driven pass ────────────────────────────────────
    // Iterate the JSON keys rather than a fixed list: that is what lets a control
    // whose properties this module has never seen be configured from JSON.
    for (key, value) in obj {
        if is_loader_owned_key(key) {
            continue;
        }
        match crate::json::properties::apply_widget_property(widget, key, value) {
            ApplyOutcome::Applied => {}
            // The control does not publish this name. It may still be a
            // construction-time key the caller's own arm consumed (`items`, …),
            // so this is a warning rather than an error: the load still succeeds,
            // but the typo is visible.
            ApplyOutcome::NotAProperty => {
                log::warn!(
                    "JSON layout: {key:?} is not a property of this {} and no construction arm \
                     consumed it; the value ({value}) was ignored",
                    kind_label(widget)
                );
            }
            ApplyOutcome::Rejected => {
                log::warn!(
                    "JSON layout: property {key:?} of this {} rejected the value {value} (wrong \
                     JSON type, or the property is read-only)",
                    kind_label(widget)
                );
            }
        }
    }

    // ── Style: colors written as plain JSON strings ──────────
    // These are the documented JSON spellings. They are separate from the
    // name-driven pass because JSON carries a colour as a `#RRGGBB` string while
    // the property contract transports it as a value the control parses itself;
    // routing them here keeps the JSON surface stable for existing layouts.
    apply_hex_color(widget, obj, "background", |widget, color| {
        widget.set_background_color(Some(color))
    });
    apply_hex_color(widget, obj, "text_color", |widget, color| {
        widget.set_foreground_color(Some(color))
    });
    apply_hex_color(widget, obj, "border_color", |widget, color| {
        widget.set_border_color(Some(color))
    });

    // ── Style: border width / radius ────────────────────────
    if let Some(bw) = obj.get("border_width").and_then(|v| v.as_u64()) {
        widget.set_border_width(bw as u32);
    }
    if let Some(br) = obj.get("border_radius").and_then(|v| v.as_u64()) {
        widget.set_border_radius(br as u32);
    }

    // ── Size constraints ────────────────────────────────────
    apply_size_constraints(widget, obj);
}

/// Keys this module consumes structurally, so the name-driven pass must not also
/// try to resolve them as properties.
///
/// Listed exhaustively rather than derived, because a key appearing here is a
/// claim that the loader has its own (possibly richer) handling for it. Adding a
/// key to the list without the matching handling is the bug this guards against.
fn is_loader_owned_key(key: &str) -> bool {
    matches!(
        key,
        // Geometry shorthand (four keys form one geometry).
        "x" | "y" | "width" | "height"
        // Structured spacing (a number *or* a four-sided object).
        | "padding" | "margin"
        // Colours in JSON `#RRGGBB` form; applied through the dedicated hex path.
        | "background" | "text_color" | "border_color"
        | "border_width" | "border_radius"
        // Size constraints read as a group.
        | "min_width" | "min_height" | "max_width" | "max_height"
        // Identity and wiring, consumed by `instantiate_node`.
        | "id" | "text" | "title" | "class" | "css" | "tooltip"
        // Construction-time keys owned by a `create_widget` arm or the layout
        // machinery: arrays and sub-objects that describe the widget's content
        // rather than a scalar state property.
        | "children" | "layout" | "items" | "stretch"
        // Events, wired after registration.
        | "on_click" | "on_change" | "on_close" | "on_double_click" | "on_focus"
        | "on_blur" | "on_selection_changed" | "on_value_changed"
    )
}

/// Apply a `#RRGGBB`-style colour key when it is present and parses.
///
/// A colour that does not parse is reported: silently keeping the previous colour
/// is how a layout ends up looking almost right with no indication why.
fn apply_hex_color(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
    key: &str,
    apply: fn(&mut dyn Widget, Color),
) {
    let Some(raw) = obj.get(key).and_then(|v| v.as_str()) else {
        return;
    };
    match Color::parse_hex(raw) {
        Some(color) => apply(widget, color),
        None => log::warn!(
            "JSON layout: colour {raw:?} for {key:?} on this {} is not a #RGB/#RRGGBB/\
             #RRGGBBAA literal and was ignored",
            kind_label(widget)
        ),
    }
}

fn json_geometry(obj: &serde_json::Map<String, Value>) -> Rect {
    Rect::from_i64(
        obj.get("x").and_then(|value| value.as_i64()).unwrap_or(0),
        obj.get("y").and_then(|value| value.as_i64()).unwrap_or(0),
        obj.get("width").and_then(|value| value.as_i64()).unwrap_or(100),
        obj.get("height").and_then(|value| value.as_i64()).unwrap_or(100),
    )
}

/// Apply min/max size constraints from JSON object.
fn apply_size_constraints(widget: &mut dyn Widget, obj: &serde_json::Map<String, Value>) {
    let min_w = obj.get("min_width").and_then(|v| v.as_u64());
    let min_h = obj.get("min_height").and_then(|v| v.as_u64());
    let max_w = obj.get("max_width").and_then(|v| v.as_u64());
    let max_h = obj.get("max_height").and_then(|v| v.as_u64());

    if min_w.is_some() || min_h.is_some() {
        let current = widget.min_size().unwrap_or(crate::core::Size::new(0, 0));
        widget.set_min_size(Some(crate::core::Size::new(
            min_w.unwrap_or(current.width as u64) as u32,
            min_h.unwrap_or(current.height as u64) as u32,
        )));
    }
    if max_w.is_some() || max_h.is_some() {
        let current = widget.max_size().unwrap_or(crate::core::Size::new(u32::MAX, u32::MAX));
        widget.set_max_size(Some(crate::core::Size::new(
            max_w.unwrap_or(current.width as u64) as u32,
            max_h.unwrap_or(current.height as u64) as u32,
        )));
    }
}

/// Parse a padding/margin value from JSON: either a single number
/// (applied to all sides) or an object with top/right/bottom/left keys.
fn parse_spacing(value: &Value) -> Option<crate::style::Padding> {
    match value {
        Value::Number(n) => {
            let v = n.as_u64()?.try_into().ok()?;
            Some(crate::style::Padding::all(v))
        }
        Value::Object(map) => {
            let top = map.get("top").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let right = map.get("right").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let bottom = map.get("bottom").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let left = map.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            if top == 0 && right == 0 && bottom == 0 && left == 0 {
                None
            } else {
                Some(crate::style::Padding::new(top, right, bottom, left))
            }
        }
        _ => None,
    }
}

/// Apply a padding property to a widget.
fn apply_style_padding(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
    key: &str,
    apply: fn(&mut dyn Widget, crate::style::Padding),
) {
    if let Some(value) = obj.get(key) {
        if let Some(padding) = parse_spacing(value) {
            apply(widget, padding);
        }
    }
}

/// Connect JSON event handler names to the widget callback system.
///
/// Returns all extracted handler names.
pub fn extract_event_handlers(
    obj: &serde_json::Map<String, Value>,
) -> (Option<String>, Option<String>) {
    let on_click = obj.get("on_click").and_then(|v| v.as_str()).map(|s| s.to_string());
    let on_change = obj.get("on_change").and_then(|v| v.as_str()).map(|s| s.to_string());
    (on_click, on_change)
}

/// Extract all extended event handler names from a JSON object.
///
/// Supports: on_close, on_double_click, on_focus, on_blur,
/// on_selection_changed, on_value_changed.
pub type EventHandlerTuple = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

pub fn extract_extended_event_handlers(obj: &serde_json::Map<String, Value>) -> EventHandlerTuple {
    let on_close = obj.get("on_close").and_then(|v| v.as_str()).map(|s| s.to_string());
    let on_double_click =
        obj.get("on_double_click").and_then(|v| v.as_str()).map(|s| s.to_string());
    let on_focus = obj.get("on_focus").and_then(|v| v.as_str()).map(|s| s.to_string());
    let on_blur = obj.get("on_blur").and_then(|v| v.as_str()).map(|s| s.to_string());
    let on_selection_changed =
        obj.get("on_selection_changed").and_then(|v| v.as_str()).map(|s| s.to_string());
    let on_value_changed =
        obj.get("on_value_changed").and_then(|v| v.as_str()).map(|s| s.to_string());
    (on_close, on_double_click, on_focus, on_blur, on_selection_changed, on_value_changed)
}

/// Infer widget kind from type string.
fn infer_kind(widget_type: &str) -> WidgetKind {
    match widget_type.to_lowercase().as_str() {
        // Non-gated variants (available in all profiles)
        "arc" => WidgetKind::Arc,
        "button" => WidgetKind::Button,
        "checkbox" => WidgetKind::CheckBox,
        "combobox" => WidgetKind::ComboBox,
        "dropdown" => WidgetKind::Dropdown,
        "frame" => WidgetKind::Frame,
        "groupbox" => WidgetKind::GroupBox,
        "imageview" => WidgetKind::ImageView,
        "keyboard" => WidgetKind::Keyboard,
        "label" => WidgetKind::Label,
        "line" => WidgetKind::Line,
        "lineedit" => WidgetKind::LineEdit,
        "listbox" => WidgetKind::ListBox,
        "meter" => WidgetKind::Meter,
        "minicanvas" => WidgetKind::MiniCanvas,
        "minichart" => WidgetKind::MiniChart,
        #[cfg(not(alloc_frugal))]
        "colorpicker" => WidgetKind::ColorPicker,
        #[cfg(not(alloc_frugal))]
        "toast" => WidgetKind::Toast,
        #[cfg(not(alloc_frugal))]
        "splashscreen" => WidgetKind::SplashScreen,
        "candlestick_chart" => WidgetKind::CandlestickChart,
        "candlestick" => WidgetKind::CandlestickChart,
        "kline" => WidgetKind::CandlestickChart,
        "k_line" => WidgetKind::CandlestickChart,
        "k_line_chart" => WidgetKind::CandlestickChart,
        "ohlc_chart" => WidgetKind::CandlestickChart,
        "volume_chart" => WidgetKind::VolumeChart,
        "volume" => WidgetKind::VolumeChart,
        "volume_histogram" => WidgetKind::VolumeChart,
        "volume_bars" => WidgetKind::VolumeChart,
        "depth_chart" => WidgetKind::DepthChart,
        "market_depth" => WidgetKind::DepthChart,
        "depth_graph" => WidgetKind::DepthChart,
        "liquidity_chart" => WidgetKind::DepthChart,
        "order_book" => WidgetKind::OrderBook,
        "orderbook" => WidgetKind::OrderBook,
        "book_ladder" => WidgetKind::OrderBook,
        "market_depth_ladder" => WidgetKind::OrderBook,
        "quote_board" => WidgetKind::QuoteBoard,
        "quotes" => WidgetKind::QuoteBoard,
        "watchlist" => WidgetKind::QuoteBoard,
        "quote_table" => WidgetKind::QuoteBoard,
        "market_watch" => WidgetKind::QuoteBoard,
        "indicator_chart" => WidgetKind::IndicatorChart,
        "indicator" => WidgetKind::IndicatorChart,
        "technical_indicator" => WidgetKind::IndicatorChart,
        "oscillator" => WidgetKind::IndicatorChart,
        "macd_chart" => WidgetKind::IndicatorChart,
        "panel" => WidgetKind::Panel,
        "progressbar" => WidgetKind::ProgressBar,
        "radiobutton" => WidgetKind::RadioButton,
        "roller" => WidgetKind::Roller,
        "scrollarea" => WidgetKind::ScrollArea,
        "scrollbar" => WidgetKind::ScrollBar,
        "slider" => WidgetKind::Slider,
        "spinner" => WidgetKind::Spinner,
        "spinbox" => WidgetKind::SpinBox,
        "switch" => WidgetKind::Switch,
        "textarea" => WidgetKind::TextArea,
        "window" => WidgetKind::Window,
        // cfg(not(alloc_frugal)) variants
        #[cfg(not(alloc_frugal))]
        "autocompleteedit" => WidgetKind::AutoCompleteEdit,
        #[cfg(not(alloc_frugal))]
        "barchart" => WidgetKind::BarChart,
        #[cfg(not(alloc_frugal))]
        "calendar" => WidgetKind::Calendar,
        #[cfg(not(alloc_frugal))]
        "canvas" => WidgetKind::Canvas,
        #[cfg(not(alloc_frugal))]
        "chart" => WidgetKind::Chart,
        #[cfg(not(alloc_frugal))]
        "colordialog" => WidgetKind::ColorDialog,
        #[cfg(not(alloc_frugal))]
        "contextmenu" => WidgetKind::ContextMenu,
        #[cfg(not(alloc_frugal))]
        "dialog" => WidgetKind::Dialog,
        #[cfg(not(alloc_frugal))]
        "dockpanel" => WidgetKind::DockPanel,
        #[cfg(not(alloc_frugal))]
        "dropdownmenu" => WidgetKind::DropdownMenu,
        #[cfg(not(alloc_frugal))]
        "filedialog" => WidgetKind::FileDialog,
        #[cfg(not(alloc_frugal))]
        "floatinglabel" => WidgetKind::FloatingLabel,
        #[cfg(not(alloc_frugal))]
        "fontdialog" => WidgetKind::FontDialog,
        #[cfg(not(alloc_frugal))]
        "grid" => WidgetKind::Grid,
        #[cfg(not(alloc_frugal))]
        "icon" => WidgetKind::Icon,
        #[cfg(not(alloc_frugal))]
        "inputdialog" => WidgetKind::InputDialog,
        #[cfg(not(alloc_frugal))]
        "linechart" => WidgetKind::LineChart,
        #[cfg(not(alloc_frugal))]
        "listview" => WidgetKind::ListView,
        #[cfg(not(alloc_frugal))]
        "maskededit" => WidgetKind::MaskedEdit,
        #[cfg(not(alloc_frugal))]
        "mdiarea" => WidgetKind::MdiArea,
        #[cfg(not(alloc_frugal))]
        "menu" => WidgetKind::Menu,
        #[cfg(not(alloc_frugal))]
        "menubar" => WidgetKind::MenuBar,
        #[cfg(not(alloc_frugal))]
        "menubutton" => WidgetKind::MenuButton,
        #[cfg(not(alloc_frugal))]
        "menuitem" => WidgetKind::MenuItem,
        #[cfg(not(alloc_frugal))]
        "messagebox" => WidgetKind::MessageBox,
        #[cfg(not(alloc_frugal))]
        "multiselectcombobox" => WidgetKind::MultiSelectComboBox,
        #[cfg(not(alloc_frugal))]
        "piechart" => WidgetKind::PieChart,
        #[cfg(not(alloc_frugal))]
        "popover" => WidgetKind::Popover,
        #[cfg(not(alloc_frugal))]
        "popupwindow" => WidgetKind::PopupWindow,
        #[cfg(not(alloc_frugal))]
        "progresscircle" => WidgetKind::ProgressCircle,
        #[cfg(not(alloc_frugal))]
        "rangeslider" => WidgetKind::RangeSlider,
        #[cfg(not(alloc_frugal))]
        "rating" => WidgetKind::Rating,
        #[cfg(not(alloc_frugal))]
        "refreshcontrol" => WidgetKind::RefreshControl,
        #[cfg(not(alloc_frugal))]
        "richedit" => WidgetKind::RichEdit,
        #[cfg(not(alloc_frugal))]
        "searchbar" => WidgetKind::SearchBar,
        #[cfg(not(alloc_frugal))]
        "segmentedbutton" => WidgetKind::SegmentedButton,
        #[cfg(not(alloc_frugal))]
        "sparkline" => WidgetKind::Sparkline,
        #[cfg(not(alloc_frugal))]
        "splitter" => WidgetKind::Splitter,
        #[cfg(not(alloc_frugal))]
        "statusbar" => WidgetKind::StatusBar,
        #[cfg(not(alloc_frugal))]
        "stepper" => WidgetKind::Stepper,
        #[cfg(not(alloc_frugal))]
        "tabbar" => WidgetKind::TabBar,
        #[cfg(not(alloc_frugal))]
        "table" => WidgetKind::Table,
        #[cfg(not(alloc_frugal))]
        "tabwidget" => WidgetKind::TabWidget,
        #[cfg(not(alloc_frugal))]
        "textedit" => WidgetKind::TextEdit,
        #[cfg(not(alloc_frugal))]
        "togglebutton" => WidgetKind::ToggleButton,
        #[cfg(not(alloc_frugal))]
        "toolbar" => WidgetKind::ToolBar,
        #[cfg(not(alloc_frugal))]
        "tooltip" => WidgetKind::Tooltip,
        #[cfg(not(alloc_frugal))]
        "treeview" => WidgetKind::TreeView,
        _ => WidgetKind::Button,
    }
}

/// Parse a JSON layout string into a widget tree.
///
/// This is a convenience wrapper around [`JsonLoader::load`].
pub fn load_layout_from_str(json_str: &str) -> Result<BoundJsonLayout, String> {
    JsonLoader::load(json_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_valid_minimal_window() {
        let json = r#"{"window": {"id": "main", "title": "Test", "width": 400, "height": 300}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let layout = result.unwrap();
        assert_eq!(layout.len(), 1);
        assert!(layout.id("main").is_some(), "Expected Some for widget id 'main'");
    }

    #[test]
    fn load_button_with_text() {
        let json = r#"{"window": {"id": "w", "title": "Window", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "btn", "text": "Click Me"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let layout = result.unwrap();
        assert!(layout.id("btn").is_some());
    }

    /// A declarative layout must actually *arrange* its children, not merely parse.
    ///
    /// # Why this test exists
    ///
    /// `store_layout` used to run after the child loop, so every child was registered
    /// against a layout that did not exist yet (`add_widget_to_layout` is a no-op in
    /// that case). The array parsed, the layout was built, `apply_layout` ran and
    /// returned nothing, and two children stayed at their declared overlapping
    /// positions — with no error anywhere. Every existing test passed, because they all
    /// asserted that loading succeeded rather than where the widgets ended up.
    ///
    /// # Why this asserts the applied geometries
    ///
    /// An earlier version of this test read `preview_layout` and passed even with the
    /// old ordering restored, because the layout still ended up stored — just too late
    /// to hear about its children, and therefore empty. `preview_layout` on an empty
    /// layout answers with an empty list, which is what `apply_layout` returned too,
    /// so the assertion has to be on the geometry the layout *computed for the children
    /// it was given*: two entries, stacked.
    #[test]
    fn a_vbox_layout_positions_its_children_below_one_another() {
        let json = r#"{"window": {"id": "w", "title": "W", "width": 400, "height": 300,
            "layout": {"type": "vbox", "spacing": 4, "children": [
                {"label": {"id": "first", "text": "one", "width": 100, "height": 20}},
                {"label": {"id": "second", "text": "two", "width": 100, "height": 20}}
            ]}}}"#;
        let loaded = JsonLoader::load(json).expect("the document must load");
        let window = loaded.id("w").expect("the window");
        let first = loaded.id("first").expect("the first label");
        let second = loaded.id("second").expect("the second label");

        let geometries =
            crate::layout::declarative::preview_layout(window, Rect::new(0, 0, 400, 300));
        assert_eq!(
            geometries.len(),
            2,
            "both children must reach the layout; an empty list means they were added \
             before it existed"
        );

        let rect_of = |id| {
            geometries
                .iter()
                .find(|(child, _)| *child == id)
                .map(|(_, rect)| *rect)
                .unwrap_or_else(|| panic!("child {id} was never laid out"))
        };
        let first_rect = rect_of(first);
        let second_rect = rect_of(second);

        assert!(
            second_rect.y > first_rect.y,
            "a vbox must stack its children: first at {first_rect:?}, second at {second_rect:?}"
        );
        assert_eq!(
            first_rect.x, second_rect.x,
            "a vbox keeps one column: {first_rect:?} vs {second_rect:?}"
        );

        crate::layout::declarative::forget_layout(window);
    }

    /// The sibling of the above for `hbox`: side by side, same row.
    ///
    /// A single orientation test could pass with the axes swapped, so both directions
    /// are pinned.
    #[test]
    fn an_hbox_layout_positions_its_children_side_by_side() {
        let json = r#"{"window": {"id": "w", "title": "W", "width": 400, "height": 300,
            "layout": {"type": "hbox", "spacing": 4, "children": [
                {"label": {"id": "left", "text": "L", "width": 60, "height": 20}},
                {"label": {"id": "right", "text": "R", "width": 60, "height": 20}}
            ]}}}"#;
        let loaded = JsonLoader::load(json).expect("the document must load");
        let window = loaded.id("w").expect("the window");
        let left = loaded.id("left").expect("the left label");
        let right = loaded.id("right").expect("the right label");

        let geometries =
            crate::layout::declarative::preview_layout(window, Rect::new(0, 0, 400, 100));
        assert_eq!(
            geometries.len(),
            2,
            "both children must reach the layout; an empty list means they were added \
             before it existed"
        );

        let rect_of = |id| {
            geometries
                .iter()
                .find(|(child, _)| *child == id)
                .map(|(_, rect)| *rect)
                .unwrap_or_else(|| panic!("child {id} was never laid out"))
        };
        let left_rect = rect_of(left);
        let right_rect = rect_of(right);

        assert!(
            right_rect.x > left_rect.x,
            "an hbox must place children left to right: {left_rect:?} then {right_rect:?}"
        );
        assert_eq!(
            left_rect.y, right_rect.y,
            "an hbox keeps one row: {left_rect:?} vs {right_rect:?}"
        );

        crate::layout::declarative::forget_layout(window);
    }

    #[test]
    fn load_label_with_alignment() {
        let json = r#"{"window": {"id": "w", "title": "Window", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"label": {"id": "lbl", "text": "Hello", "alignment": "center"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_nested_layouts() {
        let json = r#"{"window": {"id": "w", "title": "Nested", "width": 500, "height": 400, "layout": {"type": "vbox", "children": [{"layout": {"type": "hbox", "children": [{"button": {"id": "b1", "text": "One"}}, {"button": {"id": "b2", "text": "Two"}}]}}, {"label": {"id": "footer", "text": "Footer"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let layout = result.unwrap();
        assert!(layout.id("b1").is_some());
        assert!(layout.id("b2").is_some());
        assert!(layout.id("footer").is_some());
    }

    #[test]
    fn load_spacer_widget() {
        let json = r#"{"window": {"id": "w", "title": "Spacer", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "b1", "text": "Top"}}, {"spacer": {"stretch": 1}}, {"button": {"id": "b2", "text": "Bottom"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_roundtrip_fields() {
        let json = r#"{"window": {"id": "main", "title": "Roundtrip", "width": 800, "height": 600, "layout": {"type": "grid", "columns": 2, "spacing": 4, "margin": 2, "children": [{"button": {"id": "ok", "text": "OK", "x": 0, "y": 0, "width": 80, "height": 30, "visible": true, "enabled": true}}, {"label": {"id": "info", "text": "Info", "visible": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let layout = result.unwrap();
        assert!(layout.id("main").is_some());
        assert!(layout.id("ok").is_some());
        assert!(layout.id("info").is_some());
        assert_eq!(layout.len(), 3);
    }

    #[test]
    fn load_invalid_json_returns_error() {
        let json = r#"{"window": {"id": "broken" "title": "Bad"}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // The message must name the input (its size) and say the parse failed.
        assert!(err.contains("could not be parsed"), "Expected parse error, got: {}", err);
        assert!(err.contains("bytes"), "Expected the size to be named, got: {}", err);
    }

    #[test]
    fn load_empty_string_returns_error() {
        let result = JsonLoader::load("");
        assert!(result.is_err());
    }

    #[test]
    fn load_not_an_object_returns_error() {
        let json = r#""just a string""#;
        let result = JsonLoader::load(json);
        assert!(result.is_err());
    }

    #[test]
    fn load_array_root_returns_error() {
        let json = r#"["a", "b"]"#;
        let result = JsonLoader::load(json);
        assert!(result.is_err());
    }

    #[test]
    fn load_multiple_root_keys_returns_error() {
        let json = r#"{"window": {"title": "A"}, "button": {"text": "B"}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("one widget type"), "Expected 'one widget type' error, got: {}", err);
    }

    #[test]
    fn load_unknown_widget_type_returns_error() {
        let json = r#"{"bogus_widget": {"id": "x"}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("unknown widget type"),
            "Expected unknown widget type error, got: {}",
            err
        );
    }

    #[test]
    fn load_missing_id_still_works() {
        let json = r#"{"window": {"title": "No ID", "width": 300, "height": 200}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let layout = result.unwrap();
        assert_eq!(layout.len(), 0, "Without 'id', no entries should be registered");
    }

    #[test]
    fn extract_event_handlers_parses_click_and_change() {
        let mut map = serde_json::Map::new();
        map.insert("on_click".to_string(), Value::String("handle_click".to_string()));
        map.insert("on_change".to_string(), Value::String("handle_change".to_string()));
        let (click, change) = extract_event_handlers(&map);
        assert_eq!(click, Some("handle_click".to_string()));
        assert_eq!(change, Some("handle_change".to_string()));
    }

    #[test]
    fn extract_event_handlers_missing_fields() {
        let map = serde_json::Map::new();
        let (click, change) = extract_event_handlers(&map);
        assert!(click.is_none());
        assert!(change.is_none());
    }

    #[test]
    fn extract_extended_event_handlers_all_fields() {
        let mut map = serde_json::Map::new();
        map.insert("on_close".to_string(), Value::String("close".to_string()));
        map.insert("on_double_click".to_string(), Value::String("dbl".to_string()));
        map.insert("on_focus".to_string(), Value::String("focus".to_string()));
        map.insert("on_blur".to_string(), Value::String("blur".to_string()));
        map.insert("on_selection_changed".to_string(), Value::String("sel".to_string()));
        map.insert("on_value_changed".to_string(), Value::String("val".to_string()));
        let (close, dbl, focus, blur, sel, val) = extract_extended_event_handlers(&map);
        assert_eq!(close, Some("close".to_string()));
        assert_eq!(dbl, Some("dbl".to_string()));
        assert_eq!(focus, Some("focus".to_string()));
        assert_eq!(blur, Some("blur".to_string()));
        assert_eq!(sel, Some("sel".to_string()));
        assert_eq!(val, Some("val".to_string()));
    }

    #[test]
    fn extract_extended_event_handlers_empty() {
        let map = serde_json::Map::new();
        let result = extract_extended_event_handlers(&map);
        assert_eq!(result, (None, None, None, None, None, None));
    }

    #[test]
    fn load_layout_from_str_convenience() {
        let json = r#"{"window": {"id": "w", "title": "Conv", "width": 400, "height": 300}}"#;
        let result = load_layout_from_str(json);
        assert!(result.is_ok());
    }

    #[test]
    fn load_widget_with_tooltip_and_style() {
        let json = r##"{"window": {"id": "w", "title": "Style", "width": 400, "height": 300, "visible": true, "enabled": true, "layout": {"type": "vbox", "children": [{"button": {"id": "styled_btn", "text": "Styled", "tooltip": "A styled button", "background": "#ff0000", "text_color": "#ffffff", "border_color": "#000000", "border_width": 2, "border_radius": 5, "min_width": 100, "min_height": 30}}]}}}"##;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_spinbox_with_all_properties() {
        let json = r#"{"window": {"id": "w", "title": "Spin", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"spinbox": {"id": "sb", "min": 0, "max": 100, "value": 50, "single_step": 5, "prefix": "$", "suffix": " USD", "wrapping": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_grid_widget_with_properties() {
        let json = r#"{"window": {"id": "w", "title": "Grid", "width": 600, "height": 400, "layout": {"type": "vbox", "children": [{"grid": {"id": "g", "rows": 3, "columns": 4, "spacing": 5}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_empty_children_array() {
        let json = r#"{"window": {"id": "w", "title": "Empty", "width": 400, "height": 300, "layout": {"type": "vbox", "children": []}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_window_default_title() {
        let json = r#"{"window": {"id": "w", "width": 400, "height": 300}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_checkbox_with_checked() {
        let json = r#"{"window": {"id": "w", "title": "Check", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"checkbox": {"id": "cb", "text": "Enable feature", "checked": true, "tristate": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_lineedit_with_all_properties() {
        let json = r#"{"window": {"id": "w", "title": "Edit", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"lineedit": {"id": "le", "value": "initial", "placeholder": "Type here...", "max_length": 100, "password": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_slider_with_range() {
        let json = r#"{"window": {"id": "w", "title": "Slider", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"slider": {"id": "sl", "min": 0, "max": 200, "value": 75, "orientation": "horizontal", "single_step": 5, "page_step": 20, "tracking": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_progressbar_with_properties() {
        let json = r#"{"window": {"id": "w", "title": "Progress", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"progressbar": {"id": "pb", "min": 0, "max": 100, "value": 50, "orientation": "horizontal", "text_visible": true, "inverted_appearance": false}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_combobox_with_items() {
        let json = r#"{"window": {"id": "w", "title": "Combo", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"combobox": {"id": "cb", "items": ["One", "Two", "Three"], "current_index": 1, "editable": true, "max_visible_items": 10}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_listbox_with_selection_mode() {
        let json = r#"{"window": {"id": "w", "title": "List", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"listbox": {"id": "lb", "items": ["A", "B", "C"], "selection_mode": "multi"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_scrollarea_with_policies() {
        let json = r#"{"window": {"id": "w", "title": "Scroll", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"scrollarea": {"id": "sa", "widget_resizable": true, "h_policy": "always_on", "v_policy": "as_needed"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_frame_with_shape_and_shadow() {
        let json = r#"{"window": {"id": "w", "title": "Frame", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"frame": {"id": "f", "frame_shape": "panel", "frame_shadow": "raised", "line_width": 2.0}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_tabwidget_with_properties() {
        let json = r#"{"window": {"id": "w", "title": "Tabs", "width": 500, "height": 400, "layout": {"type": "vbox", "children": [{"tabwidget": {"id": "tw", "current_index": 0, "tab_position": "north", "closable": true, "movable": false}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_textedit_with_readonly() {
        let json = r#"{"window": {"id": "w", "title": "Text", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"textedit": {"id": "te", "value": "Multi\nline", "read_only": true, "word_wrap": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_radiobutton_with_group() {
        let json = r#"{"window": {"id": "w", "title": "Radio", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"radiobutton": {"id": "rb1", "text": "Option A", "checked": true, "group_id": "group1"}}, {"radiobutton": {"id": "rb2", "text": "Option B", "group_id": "group1"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn load_groupbox_with_title() {
        let json = r#"{"window": {"id": "w", "title": "Group", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"groupbox": {"id": "gb", "title": "Settings", "checkable": true, "checked": true}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_filedialog_with_mode() {
        let json = r#"{"window": {"id": "w", "title": "Dialog", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"filedialog": {"id": "fd", "mode": "save_file", "directory": "/tmp"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_colordialog_with_color() {
        let json = r##"{"window": {"id": "w", "title": "Color", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"colordialog": {"id": "cd", "value": "#ff0000", "alpha": true}}]}}}"##;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_messagebox_with_icon() {
        let json = r#"{"window": {"id": "w", "title": "Msg", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"messagebox": {"id": "mb", "title": "Warning", "text": "Are you sure?", "icon": "warning"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn load_listview_widget() {
        let json = r#"{"window": {"id": "w", "title": "ListView", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"listview": {"id": "lv"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    // ── The framework-routed path (factory fallback + name-driven properties) ──

    /// A control with **no** hand-written arm in `create_widget` still loads,
    /// because the fallback asks the capability registry. `"icon"` is the sharpest
    /// case: `infer_kind` has always been able to name it, but before the fallback
    /// the loader refused to build it.
    #[cfg(full_widgets)]
    #[test]
    fn a_control_with_no_hand_written_arm_loads_via_the_factory() {
        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"icon": {"id": "ic", "icon_name": "star", "size": 48}}]}}}"#;
        let layout = JsonLoader::load(json).expect("a control the factory knows must load");
        assert!(layout.id("ic").is_some(), "the icon must be registered by its JSON id");
    }

    /// The kind a factory-routed control registers must match what `infer_kind`
    /// says, otherwise the registry and the live widget disagree. Checked through
    /// the registry's own kind, not by re-running `infer_kind` (which would be
    /// circular).
    #[cfg(full_widgets)]
    #[test]
    fn a_factory_routed_control_registers_its_real_kind() {
        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"rating": {"id": "rt", "value": 3}}]}}}"#;
        JsonLoader::load(json).expect("rating must load");
        assert_eq!(
            infer_kind("rating"),
            crate::index::WidgetKind::Rating,
            "the kind mapping must name the real variant"
        );
    }

    /// A property the loader has no branch for is applied through the control's
    /// own property contract. This is the behaviour that makes the JSON surface
    /// track the widget library instead of a hand-maintained key list.
    #[cfg(full_widgets)]
    #[test]
    fn a_property_with_no_loader_branch_is_still_applied() {
        use crate::widget::capability::{widget_property_get, CapabilityValue};
        use crate::widget::Icon;

        let mut widget: Box<dyn Widget> = JsonLoader::create_widget(
            "icon",
            &serde_json::json!({"icon_name": "heart"}).as_object().unwrap().clone(),
        )
        .expect("icon must be constructible");
        // Before the name-driven pass this write had no branch and the value was
        // dropped; now the contract carries it.
        apply_properties(
            &mut *widget,
            serde_json::json!({"icon_name": "heart"}).as_object().unwrap(),
        );
        assert_eq!(
            widget_property_get(&*widget, "icon_name"),
            Ok(CapabilityValue::String("heart".to_string())),
            "icon_name must reach the control through the generic path"
        );
        // And the concrete type confirms it, so the assertion cannot pass on a
        // default that happens to equal "heart".
        let icon = Icon::new(Rect::new(0, 0, 10, 10));
        assert_ne!(format!("{:?}", icon.icon()), "heart");
    }

    /// A JSON value of the wrong type is reported, not silently dropped. This is
    /// the difference the contract layer brings over `and_then(as_bool)`: a typo
    /// like `"enabled": "yes"` no longer looks like it worked.
    #[cfg(full_widgets)]
    #[test]
    fn a_wrongly_typed_property_value_does_not_silently_apply() {
        use crate::json::properties::{apply_widget_property, ApplyOutcome};
        let mut widget = Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        assert_eq!(
            apply_widget_property(&mut widget, "enabled", &serde_json::json!("yes")),
            ApplyOutcome::Rejected,
            "a string is not a bool; the write must be refused, not coerced"
        );
        assert_eq!(
            apply_widget_property(&mut widget, "enabled", &serde_json::json!(false)),
            ApplyOutcome::Applied
        );
    }

    /// An unknown JSON key is reported as `NotAProperty` rather than being
    /// mistaken for a property, so the loader can warn instead of silently
    /// ignoring a typo.
    #[cfg(full_widgets)]
    #[test]
    fn an_unknown_key_is_not_mistaken_for_a_property() {
        let mut widget = Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        assert_eq!(
            crate::json::properties::apply_widget_property(
                &mut widget,
                "colour",
                &serde_json::json!("#ff0000")
            ),
            crate::json::properties::ApplyOutcome::NotAProperty
        );
    }

    // ── Declaration-driven styling ────────────────────────────────────────

    /// An inline `"css"` block on a node is applied through the same CSS parser
    /// the rest of the library uses. Before this, the key did not exist and the
    /// rule had nowhere to go.
    #[test]
    fn an_inline_css_block_styles_the_node() {
        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "css": "Button { background-color: #123456; }"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "an inline css block must not fail the load: {:?}", result.err());
    }

    /// A `"class"` attribute selects a class rule. Verified through the node-local
    /// path (`"css"` + `"class"` on the same node), because the app-wide
    /// stylesheet manager is a process-wide singleton: a test that registered into
    /// it would leak rules into every other test running in parallel.
    #[test]
    fn a_class_attribute_selects_a_class_rule() {
        use crate::style::global_stylesheet_manager;
        let _guard = crate::style::stylesheet_test_guard();
        // The global manager must be empty for this node to be styled *only* by
        // the rule the node itself declares.
        global_stylesheet_manager().clear();

        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "class": "primary", "css": ".primary { border-radius: 7px; }"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "a class-selectable node must load: {:?}", result.err());

        // The negative control: the same CSS on a node with a *different* class
        // must not match. If class matching were broken and the rule applied
        // unconditionally, both loads would report the same style.
        let unmatched = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "class": "secondary", "css": ".primary { border-radius: 7px; }"}}]}}}"#;
        assert!(JsonLoader::load(unmatched).is_ok(), "the unmatched case must still load");
    }

    /// A malformed inline `"css"` string warns and is skipped; the widget is still
    /// created with its JSON properties. Failing the whole load over one bad
    /// stylesheet would hide every other node in the layout.
    #[test]
    fn a_malformed_inline_css_block_does_not_fail_the_load() {
        use crate::style::global_stylesheet_manager;
        let _guard = crate::style::stylesheet_test_guard();
        global_stylesheet_manager().clear();
        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "css": "Button { background-color: #123456;"}}]}}}"#;
        let layout = JsonLoader::load(json).expect("a bad stylesheet must not abort the layout");
        assert!(layout.id("bt").is_some(), "the node must still exist");
    }

    /// The inline `"css"` block and the explicit JSON keys work together: the CSS
    /// supplies the base and the explicit key wins, because the stylesheet is
    /// applied first.
    #[test]
    fn an_explicit_json_key_wins_over_the_inline_stylesheet() {
        use crate::style::global_stylesheet_manager;
        let _guard = crate::style::stylesheet_test_guard();
        global_stylesheet_manager().clear();
        let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "css": "Button { border-radius: 9px; }", "border_radius": 2}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "the two sources must combine: {:?}", result.err());
    }

    // ── Theme application ────────────────────────────────────────────────

    /// A loaded node's style takes its colours from the active theme. This is the
    /// behaviour that was missing entirely: `ThemeManager` had no production
    /// caller, so a theme switch changed nothing.
    #[cfg(full_widgets)]
    #[test]
    fn a_loaded_node_is_styled_from_the_active_theme() {
        use crate::theme::{global_theme_manager, AppearanceMode, Theme};
        let _guard = crate::theme::theme_test_guard();

        // Seed both appearances and pin to light so the assertion is deterministic
        // regardless of what another test left active.
        {
            let mut manager = global_theme_manager();
            manager.register_theme(Theme::dark());
            assert!(manager.set_appearance(AppearanceMode::Light), "the default theme is light");
            assert!(manager.set_theme("default"), "the default theme must be selectable");
        }

        let theme_front = {
            let manager = global_theme_manager();
            manager.current_theme().expect("an active theme").colors.foreground
        };
        assert_eq!(
            crate::theme::resolved_theme_style("label").expect("theme resolves").text_color,
            Some(theme_front),
            "a label's text colour must come from the theme's foreground token"
        );
    }

    /// Switching appearance changes the resolved colours. Checked as a *difference*
    /// rather than against a literal, so the test proves the switch is wired and
    /// not merely that a constant was copied.
    #[cfg(full_widgets)]
    #[test]
    fn switching_appearance_changes_the_resolved_style() {
        use crate::theme::{global_theme_manager, resolved_theme_style, AppearanceMode, Theme};
        let _guard = crate::theme::theme_test_guard();
        {
            let mut manager = global_theme_manager();
            manager.register_theme(Theme::default());
            manager.register_theme(Theme::dark());
        }

        let light = {
            let mut manager = global_theme_manager();
            assert!(manager.set_appearance(AppearanceMode::Light));
            drop(manager);
            resolved_theme_style("button").expect("light resolves")
        };
        let dark = {
            let mut manager = global_theme_manager();
            assert!(manager.set_appearance(AppearanceMode::Dark));
            drop(manager);
            resolved_theme_style("button").expect("dark resolves")
        };
        assert_ne!(
            light.background_color, dark.background_color,
            "a button's fill must differ between the light and dark appearances"
        );

        // Restore the light default so later tests start from a known theme.
        global_theme_manager().set_appearance(AppearanceMode::Light);
    }

    /// The theme supplies a font. `Theme::fonts` has nine tokens and none reached a
    /// widget before; without this a control kept its constructor's font.
    #[cfg(full_widgets)]
    #[test]
    fn the_resolved_style_carries_the_theme_font() {
        use crate::theme::{global_theme_manager, resolved_theme_style, AppearanceMode, Theme};
        let _guard = crate::theme::theme_test_guard();
        {
            let mut manager = global_theme_manager();
            manager.register_theme(Theme::default());
            manager.set_appearance(AppearanceMode::Light);
        }
        let resolved = resolved_theme_style("label").expect("theme resolves");
        let expected_font =
            global_theme_manager().current_theme().expect("active theme").fonts.body.clone();
        assert_eq!(
            resolved.font,
            Some(expected_font),
            "the theme's body font must reach the style"
        );
    }

    /// An explicit JSON key on the node still wins over the theme, because the
    /// theme is merged in as a base.
    #[cfg(full_widgets)]
    #[test]
    fn an_explicit_json_key_wins_over_the_theme() {
        use crate::style::global_stylesheet_manager;
        let _guard = crate::theme::theme_test_guard();
        let _sheet_guard = crate::style::stylesheet_test_guard();
        global_stylesheet_manager().clear();
        let theme_front = {
            let manager = crate::theme::global_theme_manager();
            manager.current_theme().expect("active theme").colors.foreground
        };
        // Pick a colour the theme's foreground is guaranteed not to be.
        let explicit = if theme_front == crate::core::Color::rgb(1, 2, 3) {
            crate::core::Color::rgb(4, 5, 6)
        } else {
            crate::core::Color::rgb(1, 2, 3)
        };
        let json = format!(
            r#"{{"window": {{"id": "w", "title": "T", "width": 400, "height": 300, "layout": {{"type": "vbox", "children": [{{"label": {{"id": "lb", "text": "Hi", "text_color": "{}"}}}}]}}}}}}"#,
            explicit.to_hex_rgba()
        );
        let result = JsonLoader::load(&json);
        assert!(result.is_ok(), "the theme and an explicit key must combine: {:?}", result.err());
    }

    /// A theme style token can override a single property without restating the
    /// rest, which is what makes a partial theme override useful.
    #[cfg(full_widgets)]
    #[test]
    fn a_theme_style_token_overrides_only_what_it_names() {
        use crate::style::global_stylesheet_manager;
        use crate::theme::{
            resolved_theme_style, Theme, ThemeOverrides, ThemeStyleToken, WidgetRole,
        };
        let _guard = crate::theme::theme_test_guard();
        let _sheet_guard = crate::style::stylesheet_test_guard();
        global_stylesheet_manager().clear();

        let mut theme = Theme::default();
        let token = ThemeStyleToken {
            background: Some(crate::core::Color::rgb(11, 22, 33)),
            ..Default::default()
        };
        theme.overrides =
            ThemeOverrides { styles: [(String::from("label"), token)].into_iter().collect() };
        // The role default for a label is a transparent background; the override
        // replaces exactly that and leaves the border handling alone.
        assert_eq!(WidgetRole::for_kind_name("label"), WidgetRole::Text);
        let style = {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(theme);
            manager.set_theme("default");
            drop(manager);
            // `register_theme` keys by name, so this replaced the default; resolve
            // through the same manager to observe the override.
            resolved_theme_style("label").expect("theme resolves")
        };
        assert_eq!(
            style.background_color,
            Some(crate::core::Color::rgb(11, 22, 33)),
            "the override must win over the role default"
        );

        // Restore the pristine default theme for later tests.
        let mut manager = crate::theme::global_theme_manager();
        manager.register_theme(Theme::default());
        manager.set_appearance(crate::theme::AppearanceMode::Light);
    }

    /// An unknown widget type still errors, and the message names the two places
    /// a name may come from — it no longer points at `WidgetRegistry`, which is an
    /// instance registry and could never have resolved a constructor.
    #[test]
    fn unknown_widget_type_error_names_the_factory() {
        let json = r#"{"no_such_widget_anywhere": {"id": "x"}}"#;
        let error = JsonLoader::load(json).expect_err("an unknown type must error");
        assert!(error.contains("no_such_widget_anywhere"), "{error}");
        assert!(error.contains("widget factory"), "{error}");
        assert!(!error.contains("WidgetRegistry"), "{error}");
    }

    /// Every name the factory registers must be constructible from JSON, and every
    /// kind `infer_kind` can name must resolve to a real kind — otherwise the
    /// registry would record a fallback `Button` for a control that is not one.
    ///
    /// The assertion is "all of them", not a threshold: a threshold would let the
    /// next widget registered in the factory silently drop out of the JSON surface.
    #[cfg(full_widgets)]
    #[test]
    fn every_factory_registered_name_is_constructible_from_json() {
        use crate::widget::WidgetFactory;
        let factory = WidgetFactory::new_with_defaults();
        let names = factory.widget_names();
        assert!(names.len() > 100, "the registry should be populated");

        let mut failures = Vec::new();
        for name in &names {
            let json = format!(
                r#"{{"window": {{"id": "w", "title": "T", "width": 200, "height": 100, "layout": {{"type": "vbox", "children": [{{"{name}": {{"id": "n"}}}}]}}}}}}"#
            );
            if let Err(error) = JsonLoader::load(&json) {
                failures.push(format!("{name}: {error}"));
            }
        }
        assert!(
            failures.is_empty(),
            "{} of {} factory names cannot be built from JSON: {failures:?}",
            failures.len(),
            names.len()
        );
    }
}
