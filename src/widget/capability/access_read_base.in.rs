#[cfg(not(feature = "mini"))]
pub fn read_base_props(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Button => match property_name {
        "text" => {
        if let Some(button) = widget_as::<Button>(widget) {
        Ok(CapabilityValue::String(button.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "pressed" => {
        if let Some(button) = widget_as::<Button>(widget) {
        Ok(CapabilityValue::Bool(button.is_pressed()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "default" => {
        if let Some(button) = widget_as::<Button>(widget) {
        Ok(CapabilityValue::Bool(button.is_default()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "enabled" => {
        if let Some(button) = widget_as::<Button>(widget) {
        Ok(CapabilityValue::Bool(button.is_enabled()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "tooltip" => {
        if let Some(button) = widget_as::<Button>(widget) {
        Ok(CapabilityValue::String(button.tooltip().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Label => match property_name {
        "text" => {
        if let Some(label) = widget_as::<Label>(widget) {
        Ok(CapabilityValue::String(label.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "alignment" => {
        if let Some(label) = widget_as::<Label>(widget) {
        Ok(CapabilityValue::String(alignment_to_str(label.alignment()).to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CheckBox => match property_name {
        "text" => {
        if let Some(cb) = widget_as::<CheckBox>(widget) {
        Ok(CapabilityValue::String(cb.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "state" => {
        if let Some(cb) = widget_as::<CheckBox>(widget) {
        Ok(CapabilityValue::String(check_state_to_str(cb.state()).to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "checked" => {
        if let Some(cb) = widget_as::<CheckBox>(widget) {
        Ok(CapabilityValue::Bool(cb.is_checked()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "tristate_enabled" => {
        if let Some(cb) = widget_as::<CheckBox>(widget) {
        Ok(CapabilityValue::Bool(cb.is_tristate_enabled()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::RadioButton => match property_name {
        "text" => {
        if let Some(rb) = widget_as::<RadioButton>(widget) {
        Ok(CapabilityValue::String(rb.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "checked" => {
        if let Some(rb) = widget_as::<RadioButton>(widget) {
        Ok(CapabilityValue::Bool(rb.is_checked()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "group_id" => {
        if let Some(rb) = widget_as::<RadioButton>(widget) {
        match rb.group_id() {
        Some(id) => Ok(CapabilityValue::String(id.to_string())),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Window => match property_name {
        "title" => {
        if let Some(win) = widget_as::<Window>(widget) {
        Ok(CapabilityValue::String(win.title().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "title_bar_height" => {
        if let Some(win) = widget_as::<Window>(widget) {
        Ok(CapabilityValue::UInt(win.title_bar_height() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "close_button_size" => {
        if let Some(win) = widget_as::<Window>(widget) {
        Ok(CapabilityValue::UInt(win.close_button_size() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "button_spacing" => {
        if let Some(win) = widget_as::<Window>(widget) {
        Ok(CapabilityValue::UInt(win.button_spacing() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::GroupBox => match property_name {
        "title" => {
        if let Some(gb) = widget_as::<GroupBox>(widget) {
        Ok(CapabilityValue::String(gb.title().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "alignment" => {
        if let Some(gb) = widget_as::<GroupBox>(widget) {
        Ok(CapabilityValue::String(alignment_to_str(gb.alignment()).to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "checkable" => {
        if let Some(gb) = widget_as::<GroupBox>(widget) {
        Ok(CapabilityValue::Bool(gb.is_checkable()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "checked" => {
        if let Some(gb) = widget_as::<GroupBox>(widget) {
        Ok(CapabilityValue::Bool(gb.is_checked()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Panel | WidgetKind::Frame => match property_name {
        "segment_count" => {
        if let Some(bc) = widget_as::<Breadcrumb>(widget) {
        Ok(CapabilityValue::UInt(bc.segments().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "selected_index" => {
        if let Some(bc) = widget_as::<Breadcrumb>(widget) {
        match bc.selected_index() {
        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ToggleButton => match property_name {
        "text" => {
        if let Some(tb) = widget_as::<ToggleButton>(widget) {
        Ok(CapabilityValue::String(tb.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "checked" => {
        if let Some(tb) = widget_as::<ToggleButton>(widget) {
        Ok(CapabilityValue::Bool(tb.is_checked()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "state" => {
        if let Some(tb) = widget_as::<ToggleButton>(widget) {
        let s = match tb.state() {
        ToggleButtonState::Normal => "normal",
        ToggleButtonState::Checked => "checked",
        ToggleButtonState::Disabled => "disabled",
        };
        Ok(CapabilityValue::String(s.to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Line => match property_name {
        "orientation" => {
        if let Some(w) = widget_as::<Line>(widget) {
        let s = match w.orientation() {
        LineOrientation::Horizontal => "horizontal",
        LineOrientation::Vertical => "vertical",
        };
        Ok(CapabilityValue::String(s.to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}
