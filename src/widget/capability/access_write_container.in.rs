#[cfg(not(feature = "mini"))]
pub fn write_container_props(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Splitter => {
        if let Some(splitter) = widget_as_mut::<Splitter>(widget) {
        match property_name {
        "orientation" => {
        splitter.set_orientation(expect_orientation(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Toolbox => {
        if let Some(tool_box) = widget_as_mut::<ToolBox>(widget) {
        match property_name {
        "current_index" => {
        tool_box.set_current_index(expect_usize(value)?);
        Ok(())
        }
        "orientation" => {
        tool_box.set_orientation(expect_orientation(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::TileView => {
        if let Some(w) = widget_as_mut::<TileView>(widget) {
        match property_name {
        "current_page" => {
        w.set_current_page(expect_u32(value)?);
        Ok(())
        }
        "page_count" => {
        w.set_page_count(expect_u32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::PagerPageView => {
        if let Some(w) = widget_as_mut::<PagerPageView>(widget) {
        match property_name {
        "current_page" => {
        w.set_current_page(expect_usize(value)?);
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
