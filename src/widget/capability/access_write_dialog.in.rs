#[cfg(not(feature = "mini"))]
pub fn write_dialog_props(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::ColorDialog => {
        if let Some(color_picker) = widget_as_mut::<ColorPicker>(widget) {
        match property_name {
        "hex_rgba" => {
        let hex = expect_string(value)?;
        if color_picker.set_hex(&hex) {
        Ok(())
        } else {
        Err(CapabilityAccessError::TypeMismatch)
        }
        }
        "show_alpha" => {
        color_picker.set_show_alpha(expect_bool(value)?);
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
