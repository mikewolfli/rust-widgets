#[cfg(not(feature = "mini"))]
pub fn read_menu_props(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Action => match property_name {
        "text" => {
        if let Some(action) = widget_as::<Action>(widget) {
        Ok(CapabilityValue::String(action.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "icon_text" => {
        if let Some(action) = widget_as::<Action>(widget) {
        Ok(CapabilityValue::String(action.icon_text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "shortcut" => {
        if let Some(action) = widget_as::<Action>(widget) {
        Ok(CapabilityValue::String(action.shortcut().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "checkable" => {
        if let Some(action) = widget_as::<Action>(widget) {
        Ok(CapabilityValue::Bool(action.is_checkable()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "checked" => {
        if let Some(action) = widget_as::<Action>(widget) {
        Ok(CapabilityValue::Bool(action.is_checked()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "separator" => {
        if let Some(action) = widget_as::<Action>(widget) {
        Ok(CapabilityValue::Bool(action.is_separator()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "command_id" => {
        if let Some(action) = widget_as::<Action>(widget) {
        match action.command_id() {
        Some(id) => Ok(CapabilityValue::String(id.to_string())),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Menu => match property_name {
        "title" => {
        if let Some(menu) = widget_as::<Menu>(widget) {
        Ok(CapabilityValue::String(menu.title().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "item_count" => {
        if let Some(menu) = widget_as::<Menu>(widget) {
        Ok(CapabilityValue::UInt(menu.items().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "hovered_index" => {
        if let Some(menu) = widget_as::<Menu>(widget) {
        match menu.hovered_index() {
        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::MenuBar => match property_name {
        "entry_count" => {
        if let Some(mb) = widget_as::<MenuBar>(widget) {
        Ok(CapabilityValue::UInt(mb.entries().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "active_index" => {
        if let Some(mb) = widget_as::<MenuBar>(widget) {
        match mb.active_index() {
        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ToolBar => match property_name {
        "orientation" => {
        if let Some(tb) = widget_as::<ToolBar>(widget) {
        Ok(CapabilityValue::String(
        tool_bar_orientation_to_str(tb.orientation()).to_string(),
        ))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "icon_size" => {
        if let Some(tb) = widget_as::<ToolBar>(widget) {
        Ok(CapabilityValue::Float(tb.icon_size() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "floatable" => {
        if let Some(tb) = widget_as::<ToolBar>(widget) {
        Ok(CapabilityValue::Bool(tb.is_floatable()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "movable" => {
        if let Some(tb) = widget_as::<ToolBar>(widget) {
        Ok(CapabilityValue::Bool(tb.is_movable()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ToolButton => match property_name {
        "text" => {
        if let Some(w) = widget_as::<ToolButton>(widget) {
        Ok(CapabilityValue::String(w.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "checked" => {
        if let Some(w) = widget_as::<ToolButton>(widget) {
        Ok(CapabilityValue::Bool(w.is_checked()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::StatusBar => match property_name {
        "message" => {
        if let Some(w) = widget_as::<StatusBar>(widget) {
        Ok(CapabilityValue::String(w.message().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}
