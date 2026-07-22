#[cfg(not(feature = "mini"))]
pub fn read_dialog_props(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::ColorDialog => match property_name {
        "hex_rgba" => {
        if let Some(cp) = widget_as::<ColorPicker>(widget) {
        Ok(CapabilityValue::String(cp.hex_rgba().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "show_alpha" => {
        if let Some(cp) = widget_as::<ColorPicker>(widget) {
        Ok(CapabilityValue::Bool(cp.show_alpha()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::MessageBox => match property_name {
        "title" => {
        if let Some(mb) = widget_as::<MessageBox>(widget) {
        Ok(CapabilityValue::String(mb.title().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "text" => {
        if let Some(mb) = widget_as::<MessageBox>(widget) {
        Ok(CapabilityValue::String(mb.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::FileDialog => match property_name {
        "title" => {
        if let Some(fd) = widget_as::<FileDialog>(widget) {
        Ok(CapabilityValue::String(fd.title().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "modal" => {
        if let Some(fd) = widget_as::<FileDialog>(widget) {
        Ok(CapabilityValue::Bool(fd.is_modal()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::FontDialog => match property_name {
        "modal" => {
        if let Some(fd) = widget_as::<FontDialog>(widget) {
        Ok(CapabilityValue::Bool(fd.is_modal()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::InputDialog => match property_name {
        "title" => {
        if let Some(id) = widget_as::<InputDialog>(widget) {
        Ok(CapabilityValue::String(id.title().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "label_text" => {
        if let Some(id) = widget_as::<InputDialog>(widget) {
        Ok(CapabilityValue::String(id.label_text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ProgressDialog => match property_name {
        "title" => {
        if let Some(pd) = widget_as::<ProgressDialog>(widget) {
        Ok(CapabilityValue::String(pd.title().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "label_text" => {
        if let Some(pd) = widget_as::<ProgressDialog>(widget) {
        Ok(CapabilityValue::String(pd.label_text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::PopupWindow => match property_name {
        "has_content" => {
        if let Some(pw) = widget_as::<PopupWindow>(widget) {
        Ok(CapabilityValue::Bool(pw.content_widget().is_some()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}
