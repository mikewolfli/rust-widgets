#[cfg(not(feature = "mini"))]
pub fn write_menu_props(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Action => {
        if let Some(action) = widget_as_mut::<Action>(widget) {
        match property_name {
        "text" => {
        action.set_text(expect_string(value)?);
        Ok(())
        }
        "icon_text" => {
        action.set_icon_text(expect_string(value)?);
        Ok(())
        }
        "shortcut" => {
        action.set_shortcut(expect_string(value)?);
        Ok(())
        }
        "checkable" => {
        action.set_checkable(expect_bool(value)?);
        Ok(())
        }
        "checked" => {
        action.set_checked(expect_bool(value)?);
        Ok(())
        }
        "command_id" => {
        match value {
        CapabilityValue::Null => action.clear_command_id(),
        other => action.set_command_id(expect_string(other)?),
        }
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Menu => {
        if let Some(menu) = widget_as_mut::<Menu>(widget) {
        match property_name {
        "title" => {
        menu.set_title(expect_string(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::ToolBar => {
        if let Some(tool_bar) = widget_as_mut::<ToolBar>(widget) {
        match property_name {
        "movable" => {
        tool_bar.set_movable(expect_bool(value)?);
        Ok(())
        }
        "floatable" => {
        tool_bar.set_floatable(expect_bool(value)?);
        Ok(())
        }
        "icon_size" => {
        tool_bar.set_icon_size(expect_f32(value)?);
        Ok(())
        }
        "orientation" => {
        tool_bar.set_orientation(expect_toolbar_orientation(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::ToolButton => {
        if let Some(w) = widget_as_mut::<ToolButton>(widget) {
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
        WidgetKind::StatusBar => {
        if let Some(w) = widget_as_mut::<StatusBar>(widget) {
        match property_name {
        "message" => {
        w.show_message(expect_string(value)?, 0);
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
