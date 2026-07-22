#[cfg(not(feature = "mini"))]
pub fn read_advanced_props(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::TabBar => match property_name {
        "tab_count" => {
        if let Some(tb) = widget_as::<TabBar>(widget) {
        Ok(CapabilityValue::UInt(tb.tab_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "current_index" => {
        if let Some(tb) = widget_as::<TabBar>(widget) {
        match tb.current_index() {
        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "closable" => {
        if let Some(tb) = widget_as::<TabBar>(widget) {
        Ok(CapabilityValue::Bool(tb.closable()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "movable" => {
        if let Some(tb) = widget_as::<TabBar>(widget) {
        Ok(CapabilityValue::Bool(tb.movable()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "tab_min_width" => {
        if let Some(tb) = widget_as::<TabBar>(widget) {
        Ok(CapabilityValue::UInt(tb.tab_min_width() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "tab_max_width" => {
        if let Some(tb) = widget_as::<TabBar>(widget) {
        Ok(CapabilityValue::UInt(tb.tab_max_width() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Calendar => match property_name {
        "selected_date" => {
        if let Some(cal) = widget_as::<Calendar>(widget) {
        Ok(CapabilityValue::String(naive_date_to_string(cal.selected_date())))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "minimum_date" => {
        if let Some(cal) = widget_as::<Calendar>(widget) {
        Ok(CapabilityValue::String(naive_date_to_string(cal.minimum_date())))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "maximum_date" => {
        if let Some(cal) = widget_as::<Calendar>(widget) {
        Ok(CapabilityValue::String(naive_date_to_string(cal.maximum_date())))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "first_day_of_week" => {
        if let Some(cal) = widget_as::<Calendar>(widget) {
        Ok(CapabilityValue::String(weekday_to_str(cal.first_day_of_week()).to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "grid_visible" => {
        if let Some(cal) = widget_as::<Calendar>(widget) {
        Ok(CapabilityValue::Bool(cal.is_grid_visible()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "navigation_bar_visible" => {
        if let Some(cal) = widget_as::<Calendar>(widget) {
        Ok(CapabilityValue::Bool(cal.is_navigation_bar_visible()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "horizontal_header_visible" => {
        if let Some(cal) = widget_as::<Calendar>(widget) {
        Ok(CapabilityValue::Bool(cal.is_horizontal_header_visible()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "vertical_header_visible" => {
        if let Some(cal) = widget_as::<Calendar>(widget) {
        Ok(CapabilityValue::Bool(cal.is_vertical_header_visible()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "date_format" => {
        if let Some(cal) = widget_as::<Calendar>(widget) {
        Ok(CapabilityValue::String(cal.date_format().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::DatePicker => match property_name {
        "date" => {
        if let Some(de) = widget_as::<DateEdit>(widget) {
        Ok(CapabilityValue::String(date_to_string(de.date())))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "minimum_date" => {
        if let Some(de) = widget_as::<DateEdit>(widget) {
        Ok(CapabilityValue::String(date_to_string(de.minimum_date())))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "maximum_date" => {
        if let Some(de) = widget_as::<DateEdit>(widget) {
        Ok(CapabilityValue::String(date_to_string(de.maximum_date())))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "display_format" => {
        if let Some(de) = widget_as::<DateEdit>(widget) {
        Ok(CapabilityValue::String(de.display_format().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "calendar_popup" => {
        if let Some(de) = widget_as::<DateEdit>(widget) {
        Ok(CapabilityValue::Bool(de.calendar_popup()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TimePicker => match property_name {
        "time" => {
        if let Some(te) = widget_as::<TimeEdit>(widget) {
        Ok(CapabilityValue::String(time_to_string(te.time())))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "minimum_time" => {
        if let Some(te) = widget_as::<TimeEdit>(widget) {
        Ok(CapabilityValue::String(time_to_string(te.minimum_time())))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "maximum_time" => {
        if let Some(te) = widget_as::<TimeEdit>(widget) {
        Ok(CapabilityValue::String(time_to_string(te.maximum_time())))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "display_format" => {
        if let Some(te) = widget_as::<TimeEdit>(widget) {
        Ok(CapabilityValue::String(te.display_format().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::RibbonBar => match property_name {
        "tab_count" => {
        if let Some(rb) = widget_as::<RibbonBar>(widget) {
        Ok(CapabilityValue::UInt(rb.tab_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "current_tab" => {
        if let Some(rb) = widget_as::<RibbonBar>(widget) {
        Ok(CapabilityValue::UInt(rb.current_tab() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::PieMenu => match property_name {
        "item_count" => {
        if let Some(pm) = widget_as::<PieMenu>(widget) {
        Ok(CapabilityValue::UInt(pm.item_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "radius" => {
        if let Some(pm) = widget_as::<PieMenu>(widget) {
        Ok(CapabilityValue::Float(pm.radius() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "inner_radius" => {
        if let Some(pm) = widget_as::<PieMenu>(widget) {
        Ok(CapabilityValue::Float(pm.inner_radius() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "current_index" => {
        if let Some(pm) = widget_as::<PieMenu>(widget) {
        Ok(CapabilityValue::UInt(pm.current_index() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::DateTimePicker => match property_name {
        "datetime" => {
        if let Some(dte) = widget_as::<DateTimeEdit>(widget) {
        Ok(CapabilityValue::String(dte.datetime().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "display_format" => {
        if let Some(dte) = widget_as::<DateTimeEdit>(widget) {
        Ok(CapabilityValue::String(dte.display_format().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "calendar_popup" => {
        if let Some(dte) = widget_as::<DateTimeEdit>(widget) {
        Ok(CapabilityValue::Bool(dte.calendar_popup()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}
