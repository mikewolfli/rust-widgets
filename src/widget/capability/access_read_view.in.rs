#[cfg(not(feature = "mini"))]
pub fn read_view_props(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::ListView => match property_name {
        "has_model" => {
        if let Some(lv) = widget_as::<ListView>(widget) {
        Ok(CapabilityValue::Bool(lv.has_model()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "row_count" => {
        if let Some(lv) = widget_as::<ListView>(widget) {
        Ok(CapabilityValue::UInt(lv.row_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "focused_row" => {
        if let Some(lv) = widget_as::<ListView>(widget) {
        match lv.focused_row() {
        Some(row) => Ok(CapabilityValue::UInt(row as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "selection_mode" => {
        if let Some(lv) = widget_as::<ListView>(widget) {
        Ok(CapabilityValue::String(
        selection_mode_to_str(lv.selection_mode()).to_string(),
        ))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "view_mode" => {
        if let Some(lv) = widget_as::<ListView>(widget) {
        Ok(CapabilityValue::String(view_mode_to_str(lv.view_mode()).to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TreeView => match property_name {
        "has_model" => {
        if let Some(tt) = widget_as::<TreeTable>(widget) {
        Ok(CapabilityValue::Bool(tt.has_model()))
        } else if let Some(tv) = widget_as::<TreeView>(widget) {
        Ok(CapabilityValue::Bool(tv.has_model()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "node_count" => {
        if let Some(tv) = widget_as::<TreeView>(widget) {
        Ok(CapabilityValue::UInt(tv.node_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "focused_node" => {
        if let Some(tv) = widget_as::<TreeView>(widget) {
        match tv.focused_node() {
        Some(node) => Ok(CapabilityValue::UInt(node as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "selected_node" => {
        if let Some(tv) = widget_as::<TreeView>(widget) {
        match tv.selected_node() {
        Some(node) => Ok(CapabilityValue::UInt(node as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "row_count" => {
        if let Some(tt) = widget_as::<TreeTable>(widget) {
        Ok(CapabilityValue::UInt(tt.row_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "column_count" => {
        if let Some(tt) = widget_as::<TreeTable>(widget) {
        Ok(CapabilityValue::UInt(tt.column_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "selected_row" => {
        if let Some(tt) = widget_as::<TreeTable>(widget) {
        match tt.selected_row() {
        Some(row) => Ok(CapabilityValue::UInt(row as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "row_height" => {
        if let Some(tt) = widget_as::<TreeTable>(widget) {
        Ok(CapabilityValue::UInt(tt.row_height() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "column_width" => {
        if let Some(tt) = widget_as::<TreeTable>(widget) {
        Ok(CapabilityValue::UInt(tt.column_width() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "projection_state" => {
        if let Some(tt) = widget_as::<TreeTable>(widget) {
        let selected = match tt.selected_row() {
        Some(r) => format!("Some({r})"),
        None => "None".to_string(),
        };
        Ok(CapabilityValue::String(format!(
        "rows={},selected={}",
        tt.row_count(),
        selected
        )))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Table => match property_name {
        "has_model" => {
        if let Some(tw) = widget_as::<TableWidget>(widget) {
        Ok(CapabilityValue::Bool(tw.has_model()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "has_delegate" => {
        if let Some(tw) = widget_as::<TableWidget>(widget) {
        Ok(CapabilityValue::Bool(tw.has_delegate()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "row_count" => {
        if let Some(tw) = widget_as::<TableWidget>(widget) {
        Ok(CapabilityValue::UInt(tw.row_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "column_count" => {
        if let Some(tw) = widget_as::<TableWidget>(widget) {
        Ok(CapabilityValue::UInt(tw.column_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "selection_mode" => {
        if let Some(tw) = widget_as::<TableWidget>(widget) {
        Ok(CapabilityValue::String(
        selection_mode_to_str(tw.selection_mode()).to_string(),
        ))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "has_data_source" => {
        if let Some(dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::Bool(dg.has_data_source()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "scroll_row" => {
        if let Some(dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::UInt(dg.scroll_row() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "scroll_column" => {
        if let Some(dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::UInt(dg.scroll_column() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "row_height" => {
        if let Some(dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::UInt(dg.row_height() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "column_width" => {
        if let Some(dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::UInt(dg.column_width() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "overscan_rows" => {
        if let Some(vt) = widget_as::<VirtualTable>(widget) {
        Ok(CapabilityValue::UInt(vt.overscan_rows() as u64))
        } else if let Some(_dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::Null)
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "overscan_columns" => {
        if let Some(vt) = widget_as::<VirtualTable>(widget) {
        Ok(CapabilityValue::UInt(vt.overscan_columns() as u64))
        } else if let Some(_dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::Null)
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "frozen_columns" => {
        if let Some(dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::UInt(dg.frozen_columns() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "sort_spec_count" => {
        if let Some(dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::UInt(dg.sort_specs().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "filter_count" => {
        if let Some(dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::UInt(dg.filters().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "sort_specs" => {
        if let Some(dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::String(sort_specs_to_string(dg.sort_specs())))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "filters" => {
        if let Some(dg) = widget_as::<DataGrid>(widget) {
        Ok(CapabilityValue::String(column_filters_to_string(dg.filters())))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "visible_window" => {
        if let Some(dg) = widget_as::<DataGrid>(widget) {
        let (rs, rl, cs, cl) = dg.visible_window();
        Ok(CapabilityValue::String(format!("{rs}:{rl}:{cs}:{cl}")))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::DataView => match property_name {
        "has_data_source" => {
        if let Some(vl) = widget_as::<VirtualList>(widget) {
        Ok(CapabilityValue::Bool(vl.has_data_source()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "row_count" => {
        if let Some(vl) = widget_as::<VirtualList>(widget) {
        Ok(CapabilityValue::UInt(vl.row_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "scroll_row" => {
        if let Some(vl) = widget_as::<VirtualList>(widget) {
        Ok(CapabilityValue::UInt(vl.scroll_row() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "row_height" => {
        if let Some(vl) = widget_as::<VirtualList>(widget) {
        Ok(CapabilityValue::UInt(vl.row_height() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "overscan" => {
        if let Some(vl) = widget_as::<VirtualList>(widget) {
        Ok(CapabilityValue::UInt(vl.overscan() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "selected_row" => {
        if let Some(vl) = widget_as::<VirtualList>(widget) {
        match vl.selected_row() {
        Some(row) => Ok(CapabilityValue::UInt(row as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ImageGallery => match property_name {
        "current_index" => {
        if let Some(w) = widget_as::<ImageGallery>(widget) {
        Ok(CapabilityValue::UInt(w.current_index() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::PropertyGrid => match property_name {
        "property_count" => {
        if let Some(w) = widget_as::<PropertyGrid>(widget) {
        Ok(CapabilityValue::UInt(w.property_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "selected_index" => {
        if let Some(w) = widget_as::<PropertyGrid>(widget) {
        match w.selected_index() {
        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}
