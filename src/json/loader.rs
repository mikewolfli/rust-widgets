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
use crate::json::{
    add_spacer_to_layout, add_widget_to_layout, create_layout_from_kind, parse_layout_kind,
    store_layout, BoundJsonLayout, ChildLayoutAttrs,
};
use crate::layout::inspector::LayoutInspector;
use crate::widget::{
    Button, CheckBox, ComboBox, GroupBox, Label, LineEdit, ListBox, ProgressBar, RadioButton,
    ScrollArea, ScrollBar, Slider, SpinBox, Switch, Widget,
};
#[cfg(not(feature = "mini"))]
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
        let value: Value =
            serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))?;
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
            return Err(format!("Maximum widget depth ({}) exceeded", MAX_DEPTH));
        }

        let obj = value
            .as_object()
            .ok_or_else(|| format!("'{}' value must be a JSON object", widget_type))?;

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
                .ok_or_else(|| format!("'{}' layout must be a child of a widget", widget_type))?;

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
                                add_widget_to_layout(
                                    layout.as_ref(),
                                    child_id,
                                    attrs.stretch,
                                    layout_parent,
                                );
                            }
                        }
                    }
                }
            }

            store_layout(layout_parent, layout);
            return Ok(layout_parent);
        }

        // Create the widget
        let mut widget: Box<dyn Widget> = Self::create_widget(widget_type, obj)?;

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
            format!("{}_{}", widget_type, widget_id)
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
                                    add_widget_to_layout(
                                        layout.as_ref(),
                                        child_id,
                                        attrs.stretch,
                                        widget_id,
                                    );
                                }
                            }
                        }
                    }
                }
            }

            store_layout(widget_id, layout);
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
            #[cfg(not(feature = "mini"))]
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
                        "none" => lb.set_selection_mode(crate::widget::SelectionMode::NoSelection),
                        "single" => {
                            lb.set_selection_mode(crate::widget::SelectionMode::SingleSelection)
                        }
                        "multi" => {
                            lb.set_selection_mode(crate::widget::SelectionMode::MultiSelection)
                        }
                        "extended" => {
                            lb.set_selection_mode(crate::widget::SelectionMode::ExtendedSelection)
                        }
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
            #[cfg(not(feature = "mini"))]
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
            #[cfg(not(feature = "mini"))]
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
            #[cfg(not(feature = "mini"))]
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
            #[cfg(not(feature = "mini"))]
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
            #[cfg(not(feature = "mini"))]
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
            #[cfg(not(feature = "mini"))]
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
            #[cfg(not(feature = "mini"))]
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
            _ => Err(format!("unknown widget type: '{}'", widget_type)),
        }
    }
}

/// Apply common widget properties from a JSON object.
///
/// Handles: x/y/width/height, visible, enabled, tooltip,
/// background/text_color/border_color/border_width/border_radius,
/// padding, margin, and event bindings (on_click, on_change).
fn apply_properties(widget: &mut dyn Widget, obj: &serde_json::Map<String, Value>) {
    // ── Geometry ────────────────────────────────────────────
    if let (Some(x), Some(y), Some(w), Some(h)) = (
        obj.get("x").and_then(|v| v.as_i64()),
        obj.get("y").and_then(|v| v.as_i64()),
        obj.get("width").and_then(|v| v.as_u64()),
        obj.get("height").and_then(|v| v.as_u64()),
    ) {
        widget.set_geometry(Rect::from_i64(x, y, w as i64, h as i64));
    }

    // ── Visibility ──────────────────────────────────────────
    if let Some(visible) = obj.get("visible").and_then(|v| v.as_bool()) {
        widget.set_visible(visible);
    }

    // ── Enabled ─────────────────────────────────────────────
    if let Some(enabled) = obj.get("enabled").and_then(|v| v.as_bool()) {
        widget.set_enabled(enabled);
    }

    // ── Tooltip ─────────────────────────────────────────────
    if let Some(tooltip) = obj.get("tooltip").and_then(|v| v.as_str()) {
        widget.set_tooltip(tooltip.to_string());
    }

    // ── Style: colors ───────────────────────────────────────
    if let Some(bg) = obj.get("background").and_then(|v| v.as_str()) {
        if let Some(color) = Color::parse_hex(bg) {
            widget.set_background_color(Some(color));
        }
    }
    if let Some(tc) = obj.get("text_color").and_then(|v| v.as_str()) {
        if let Some(color) = Color::parse_hex(tc) {
            widget.set_foreground_color(Some(color));
        }
    }
    if let Some(bc) = obj.get("border_color").and_then(|v| v.as_str()) {
        if let Some(color) = Color::parse_hex(bc) {
            widget.set_border_color(Some(color));
        }
    }

    // ── Style: border width / radius ────────────────────────
    if let Some(bw) = obj.get("border_width").and_then(|v| v.as_u64()) {
        widget.set_border_width(bw as u32);
    }
    if let Some(br) = obj.get("border_radius").and_then(|v| v.as_u64()) {
        widget.set_border_radius(br as u32);
    }

    // ── Style: padding / margin ─────────────────────────────
    // Padding accepts either a single number (all sides) or an object
    // with top/right/bottom/left keys.
    apply_style_padding(widget, obj, "padding", |widget, value| {
        let mut style = widget.style().clone();
        style.padding = value;
        widget.set_style(style);
    });
    // Margin is a `Margin` type — convert from Padding-compatible values.
    if let Some(value) = obj.get("margin") {
        if let Some(p) = parse_spacing(value) {
            let margin = crate::style::Margin::new(p.top, p.right, p.bottom, p.left);
            let mut style = widget.style().clone();
            style.margin = margin;
            widget.set_style(style);
        }
    }
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
        "window" => WidgetKind::Window,
        "button" => WidgetKind::Button,
        "label" => WidgetKind::Label,
        "checkbox" => WidgetKind::CheckBox,
        "radiobutton" => WidgetKind::RadioButton,
        "lineedit" => WidgetKind::LineEdit,
        #[cfg(not(feature = "mini"))]
        "textedit" => WidgetKind::TextEdit,
        "combobox" => WidgetKind::ComboBox,
        "listbox" => WidgetKind::ListBox,
        "slider" => WidgetKind::Slider,
        "scrollbar" => WidgetKind::ScrollBar,
        "progressbar" => WidgetKind::ProgressBar,
        "groupbox" => WidgetKind::Panel,
        #[cfg(not(feature = "mini"))]
        "tabwidget" => WidgetKind::TabWidget,
        #[cfg(not(feature = "mini"))]
        "grid" => WidgetKind::GridWidget,
        "panel" => WidgetKind::Panel,
        "spinbox" => WidgetKind::SpinBox,
        #[cfg(not(feature = "mini"))]
        "listview" => WidgetKind::ListView,
        "scrollarea" => WidgetKind::ScrollArea,
        "frame" => WidgetKind::Frame,
        "dialog" => WidgetKind::Dialog,
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
        assert!(err.contains("JSON parse error"), "Expected parse error, got: {}", err);
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

    #[cfg(not(feature = "mini"))]
    #[test]
    fn load_tabwidget_with_properties() {
        let json = r#"{"window": {"id": "w", "title": "Tabs", "width": 500, "height": 400, "layout": {"type": "vbox", "children": [{"tabwidget": {"id": "tw", "current_index": 0, "tab_position": "north", "closable": true, "movable": false}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(feature = "mini"))]
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

    #[cfg(not(feature = "mini"))]
    #[test]
    fn load_filedialog_with_mode() {
        let json = r#"{"window": {"id": "w", "title": "Dialog", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"filedialog": {"id": "fd", "mode": "save_file", "directory": "/tmp"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(feature = "mini"))]
    #[test]
    fn load_colordialog_with_color() {
        let json = r##"{"window": {"id": "w", "title": "Color", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"colordialog": {"id": "cd", "value": "#ff0000", "alpha": true}}]}}}"##;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(feature = "mini"))]
    #[test]
    fn load_messagebox_with_icon() {
        let json = r#"{"window": {"id": "w", "title": "Msg", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"messagebox": {"id": "mb", "title": "Warning", "text": "Are you sure?", "icon": "warning"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[cfg(not(feature = "mini"))]
    #[test]
    fn load_listview_widget() {
        let json = r#"{"window": {"id": "w", "title": "ListView", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"listview": {"id": "lv"}}]}}}"#;
        let result = JsonLoader::load(json);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }
}
