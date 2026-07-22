#[cfg(not(feature = "mini"))]
pub fn write_view_props(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::ListView => {
        if let Some(list_view) = widget_as_mut::<ListView>(widget) {
        match property_name {
        "focused_row" => match value {
        CapabilityValue::Null => {
        list_view.clear_focused_row();
        Ok(())
        }
        other => {
        let row = expect_usize(other)?;
        if list_view.set_focused_row(row) {
        Ok(())
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        },
        "selection_mode" => {
        list_view.set_selection_mode(expect_selection_mode(value)?);
        Ok(())
        }
        "view_mode" => {
        list_view.set_view_mode(expect_view_mode(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::TreeView => {
        if let Some(tree_table) = widget_as_mut::<TreeTable>(widget) {
        match property_name {
        "selected_row" => match value {
        CapabilityValue::Null => {
        if let Some(selected) = tree_table.selected_row() {
        let _ = tree_table.select_row(selected);
        }
        Ok(())
        }
        other => {
        let row = expect_usize(other)?;
        if tree_table.select_row(row) || tree_table.row_count() == 0 {
        Ok(())
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        },
        "row_height" => {
        tree_table.set_row_height(expect_u32(value)?);
        Ok(())
        }
        "column_width" => {
        tree_table.set_column_width(expect_u32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else if let Some(tree_view) = widget_as_mut::<TreeView>(widget) {
        match property_name {
        "focused_node" => match value {
        CapabilityValue::Null => {
        tree_view.clear_focused_node();
        Ok(())
        }
        other => {
        let node = expect_usize(other)?;
        if tree_view.set_focused_node(node) {
        Ok(())
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        },
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::DataView => {
        if let Some(virtual_list) = widget_as_mut::<VirtualList>(widget) {
        match property_name {
        "scroll_row" => {
        virtual_list.set_scroll_row(expect_usize(value)?);
        Ok(())
        }
        "row_height" => {
        let row_height = expect_u32(value)?;
        virtual_list.set_row_height(row_height);
        Ok(())
        }
        "overscan" => {
        virtual_list.set_overscan(expect_usize(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Table => {
        if let Some(data_grid) = widget_as_mut::<DataGrid>(widget) {
        match property_name {
        "scroll_row" => {
        data_grid.set_scroll_row(expect_usize(value)?);
        Ok(())
        }
        "scroll_column" => {
        data_grid.set_scroll_column(expect_usize(value)?);
        Ok(())
        }
        "row_height" => {
        data_grid.set_row_height(expect_u32(value)?);
        Ok(())
        }
        "column_width" => {
        data_grid.set_column_width(expect_u32(value)?);
        Ok(())
        }
        "frozen_columns" => {
        data_grid.set_frozen_columns(expect_usize(value)?);
        Ok(())
        }
        "sort_specs" => {
        data_grid.set_sort_specs(expect_sort_specs(value)?);
        Ok(())
        }
        "filters" => {
        data_grid.set_filters(expect_column_filters(value)?);
        Ok(())
        }
        "sort_spec_count" | "filter_count" | "visible_window" => {
        Err(CapabilityAccessError::ReadOnlyProperty)
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else if let Some(virtual_table) = widget_as_mut::<VirtualTable>(widget) {
        match property_name {
        "scroll_row" => {
        virtual_table.set_scroll_row(expect_usize(value)?);
        Ok(())
        }
        "scroll_column" => {
        virtual_table.set_scroll_column(expect_usize(value)?);
        Ok(())
        }
        "row_height" => {
        virtual_table.set_row_height(expect_u32(value)?);
        Ok(())
        }
        "column_width" => {
        virtual_table.set_column_width(expect_u32(value)?);
        Ok(())
        }
        "overscan_rows" => {
        virtual_table.set_overscan_rows(expect_usize(value)?);
        Ok(())
        }
        "overscan_columns" => {
        virtual_table.set_overscan_columns(expect_usize(value)?);
        Ok(())
        }
        "visible_window" => Err(CapabilityAccessError::ReadOnlyProperty),
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else if let Some(table_widget) = widget_as_mut::<TableWidget>(widget) {
        match property_name {
        "selection_mode" => {
        table_widget.set_selection_mode(expect_selection_mode(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::ImageGallery => {
        if let Some(w) = widget_as_mut::<ImageGallery>(widget) {
        match property_name {
        "current_index" => {
        w.set_current_index(expect_usize(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::PropertyGrid => {
        if let Some(w) = widget_as_mut::<PropertyGrid>(widget) {
        match property_name {
        "selected_index" => {
        match value {
        CapabilityValue::Null => w.set_selected_index(None),
        other => w.set_selected_index(Some(expect_usize(other)?)),
        }
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
