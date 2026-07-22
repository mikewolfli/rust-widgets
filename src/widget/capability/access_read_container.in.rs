#[cfg(not(feature = "mini"))]
pub fn read_container_props(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Splitter => match property_name {
        "orientation" => {
        if let Some(splitter) = widget_as::<Splitter>(widget) {
        Ok(CapabilityValue::String(
        orientation_to_str(splitter.orientation()).to_string(),
        ))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "pane_count" => {
        if let Some(splitter) = widget_as::<Splitter>(widget) {
        Ok(CapabilityValue::UInt(splitter.pane_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Toolbox => match property_name {
        "item_count" => {
        if let Some(tb) = widget_as::<ToolBox>(widget) {
        Ok(CapabilityValue::UInt(tb.count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "current_index" => {
        if let Some(tb) = widget_as::<ToolBox>(widget) {
        Ok(CapabilityValue::UInt(tb.current_index() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "orientation" => {
        if let Some(tb) = widget_as::<ToolBox>(widget) {
        Ok(CapabilityValue::String(orientation_to_str(tb.orientation()).to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ScrollArea => match property_name {
        "widget_resizable" => {
        if let Some(sa) = widget_as::<ScrollArea>(widget) {
        Ok(CapabilityValue::Bool(sa.widget_resizable()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "horizontal_scroll_bar_policy" => {
        if let Some(sa) = widget_as::<ScrollArea>(widget) {
        let s = scroll_bar_policy_to_str(sa.horizontal_scroll_bar_policy());
        Ok(CapabilityValue::String(s.to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "vertical_scroll_bar_policy" => {
        if let Some(sa) = widget_as::<ScrollArea>(widget) {
        let s = scroll_bar_policy_to_str(sa.vertical_scroll_bar_policy());
        Ok(CapabilityValue::String(s.to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TabWidget => match property_name {
        "tab_count" => {
        if let Some(tw) = widget_as::<TabWidget>(widget) {
        Ok(CapabilityValue::UInt(tw.count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "current_index" => {
        if let Some(tw) = widget_as::<TabWidget>(widget) {
        Ok(CapabilityValue::UInt(tw.current_index() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::StackedWidget => match property_name {
        "widget_count" => {
        if let Some(sw) = widget_as::<StackedWidget>(widget) {
        Ok(CapabilityValue::UInt(sw.widget_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "current_index" => {
        if let Some(sw) = widget_as::<StackedWidget>(widget) {
        Ok(CapabilityValue::UInt(sw.current_index() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CollapsiblePane => match property_name {
        "title" => {
        if let Some(cp) = widget_as::<CollapsiblePane>(widget) {
        Ok(CapabilityValue::String(cp.title().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "collapsed" => {
        if let Some(cp) = widget_as::<CollapsiblePane>(widget) {
        Ok(CapabilityValue::Bool(cp.is_collapsed()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::DockWidget => match property_name {
        "title" => {
        if let Some(dw) = widget_as::<DockWidget>(widget) {
        Ok(CapabilityValue::String(dw.title().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "floating" => {
        if let Some(dw) = widget_as::<DockWidget>(widget) {
        Ok(CapabilityValue::Bool(dw.is_floating()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::MdiArea => match property_name {
        "subwindow_count" => {
        if let Some(ma) = widget_as::<MdiArea>(widget) {
        Ok(CapabilityValue::UInt(ma.sub_window_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "active_subwindow" => {
        if let Some(ma) = widget_as::<MdiArea>(widget) {
        match ma.active_sub_window() {
        Some(id) => Ok(CapabilityValue::UInt(id)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "view_mode" => {
        if let Some(ma) = widget_as::<MdiArea>(widget) {
        let s = match ma.view_mode() {
        crate::widget::container_widgets::mdiarea::ViewMode::SubWindowView => {
        "sub_window_view"
        }
        crate::widget::container_widgets::mdiarea::ViewMode::TabbedView => "tabbed",
        };
        Ok(CapabilityValue::String(s.to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TileView => match property_name {
        "current_page" => {
        if let Some(w) = widget_as::<TileView>(widget) {
        Ok(CapabilityValue::UInt(w.current_page() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "page_count" => {
        if let Some(w) = widget_as::<TileView>(widget) {
        Ok(CapabilityValue::UInt(w.page_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::PagerPageView => match property_name {
        "current_page" => {
        if let Some(w) = widget_as::<PagerPageView>(widget) {
        Ok(CapabilityValue::UInt(w.current_page() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}
