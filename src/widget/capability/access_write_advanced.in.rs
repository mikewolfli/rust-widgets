#[cfg(not(feature = "mini"))]
pub fn write_advanced_props(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::TabBar => {
        if let Some(tab_bar) = widget_as_mut::<TabBar>(widget) {
        match property_name {
        "current_index" => {
        tab_bar.set_current_index(expect_usize(value)?);
        Ok(())
        }
        "closable" => {
        tab_bar.set_closable(expect_bool(value)?);
        Ok(())
        }
        "movable" => {
        tab_bar.set_movable(expect_bool(value)?);
        Ok(())
        }
        "tab_min_width" => {
        tab_bar.set_tab_min_width(expect_usize(value)? as u32);
        Ok(())
        }
        "tab_max_width" => {
        tab_bar.set_tab_max_width(expect_usize(value)? as u32);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Calendar => {
        if let Some(calendar) = widget_as_mut::<Calendar>(widget) {
        match property_name {
        "selected_date" => {
        calendar.set_selected_date(expect_naive_date(value)?);
        Ok(())
        }
        "minimum_date" => {
        calendar.set_minimum_date(expect_naive_date(value)?);
        Ok(())
        }
        "maximum_date" => {
        calendar.set_maximum_date(expect_naive_date(value)?);
        Ok(())
        }
        "first_day_of_week" => {
        calendar.set_first_day_of_week(expect_weekday(value)?);
        Ok(())
        }
        "grid_visible" => {
        calendar.set_grid_visible(expect_bool(value)?);
        Ok(())
        }
        "navigation_bar_visible" => {
        calendar.set_navigation_bar_visible(expect_bool(value)?);
        Ok(())
        }
        "horizontal_header_visible" => {
        calendar.set_horizontal_header_visible(expect_bool(value)?);
        Ok(())
        }
        "vertical_header_visible" => {
        calendar.set_vertical_header_visible(expect_bool(value)?);
        Ok(())
        }
        "date_format" => {
        calendar.set_date_format(expect_string(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::DatePicker => {
        if let Some(date_edit) = widget_as_mut::<DateEdit>(widget) {
        match property_name {
        "date" => {
        date_edit.set_date(expect_date(value)?);
        Ok(())
        }
        "minimum_date" => {
        date_edit.set_minimum_date(expect_date(value)?);
        Ok(())
        }
        "maximum_date" => {
        date_edit.set_maximum_date(expect_date(value)?);
        Ok(())
        }
        "display_format" => {
        date_edit.set_display_format(expect_string(value)?);
        Ok(())
        }
        "calendar_popup" => {
        date_edit.set_calendar_popup(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::TimePicker => {
        if let Some(time_edit) = widget_as_mut::<TimeEdit>(widget) {
        match property_name {
        "time" => {
        time_edit.set_time(expect_time(value)?);
        Ok(())
        }
        "minimum_time" => {
        time_edit.set_minimum_time(expect_time(value)?);
        Ok(())
        }
        "maximum_time" => {
        time_edit.set_maximum_time(expect_time(value)?);
        Ok(())
        }
        "display_format" => {
        time_edit.set_display_format(expect_string(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::RibbonBar => {
        if let Some(ribbon_bar) = widget_as_mut::<RibbonBar>(widget) {
        match property_name {
        "current_tab" => {
        ribbon_bar.set_current_tab(expect_usize(value)?);
        Ok(())
        }
        "expanded" => {
        ribbon_bar.set_expanded(expect_bool(value)?);
        Ok(())
        }
        "minimized" => {
        ribbon_bar.set_minimized(expect_bool(value)?);
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
