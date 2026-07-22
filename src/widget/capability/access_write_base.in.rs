#[cfg(not(feature = "mini"))]
pub fn write_base_props(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Button => {
        if let Some(button) = widget_as_mut::<Button>(widget) {
        match property_name {
        "text" => {
        button.set_text(expect_string(value)?);
        Ok(())
        }
        "pressed" => {
        button.set_pressed(expect_bool(value)?);
        Ok(())
        }
        "default" => {
        button.set_default(expect_bool(value)?);
        Ok(())
        }
        "enabled" => {
        button.set_enabled(expect_bool(value)?);
        Ok(())
        }
        "tooltip" => {
        button.set_tooltip(expect_string(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Label => {
        if let Some(label) = widget_as_mut::<Label>(widget) {
        match property_name {
        "text" => {
        label.set_text(expect_string(value)?);
        Ok(())
        }
        "alignment" => {
        label.set_alignment(expect_alignment(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::CheckBox => {
        if let Some(check_box) = widget_as_mut::<CheckBox>(widget) {
        match property_name {
        "text" => {
        check_box.set_text(expect_string(value)?);
        Ok(())
        }
        "state" => {
        check_box.set_state(expect_check_state(value)?);
        Ok(())
        }
        "checked" => {
        check_box.set_checked(expect_bool(value)?);
        Ok(())
        }
        "tristate_enabled" => {
        check_box.set_tristate_enabled(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::RadioButton => {
        if let Some(radio_button) = widget_as_mut::<RadioButton>(widget) {
        match property_name {
        "text" => {
        radio_button.set_text(expect_string(value)?);
        Ok(())
        }
        "checked" => {
        radio_button.set_checked(expect_bool(value)?);
        Ok(())
        }
        "group_id" => {
        match value {
        CapabilityValue::Null => radio_button.set_group_id(None),
        other => radio_button.set_group_id(Some(expect_string(other)?)),
        }
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Window => {
        if let Some(window) = widget_as_mut::<Window>(widget) {
        match property_name {
        "title" => {
        window.set_title(expect_string(value)?);
        Ok(())
        }
        "title_bar_height" => {
        window.set_title_bar_height(expect_usize(value)? as u32);
        Ok(())
        }
        "close_button_size" => {
        window.set_close_button_size(expect_usize(value)? as u32);
        Ok(())
        }
        "button_spacing" => {
        window.set_button_spacing(expect_usize(value)? as u32);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::GroupBox => {
        if let Some(group_box) = widget_as_mut::<GroupBox>(widget) {
        match property_name {
        "title" => {
        group_box.set_title(expect_string(value)?);
        Ok(())
        }
        "alignment" => {
        group_box.set_alignment(expect_alignment(value)?);
        Ok(())
        }
        "checkable" => {
        group_box.set_checkable(expect_bool(value)?);
        Ok(())
        }
        "checked" => {
        group_box.set_checked(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::ToggleButton => {
        if let Some(w) = widget_as_mut::<ToggleButton>(widget) {
        match property_name {
        "text" => {
        w.set_text(expect_string(value)?);
        Ok(())
        }
        "checked" => {
        w.set_checked(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Line => {
        if let Some(w) = widget_as_mut::<Line>(widget) {
        match property_name {
        "orientation" => {
        let s = expect_string(value)?;
        let ori = match s.as_str() {
        "horizontal" => LineOrientation::Horizontal,
        "vertical" => LineOrientation::Vertical,
        _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        w.set_orientation(ori);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}
