#[cfg(not(feature = "mini"))]
pub fn read_other_props(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::LCDNumber => match property_name {
        "value" => {
        if let Some(lcd) = widget_as::<LCDNumber>(widget) {
        Ok(CapabilityValue::Float(lcd.value()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "min_value" => {
        if let Some(lcd) = widget_as::<LCDNumber>(widget) {
        Ok(CapabilityValue::Float(lcd.min_value()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "max_value" => {
        if let Some(lcd) = widget_as::<LCDNumber>(widget) {
        Ok(CapabilityValue::Float(lcd.max_value()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "num_digits" => {
        if let Some(lcd) = widget_as::<LCDNumber>(widget) {
        Ok(CapabilityValue::Int(lcd.num_digits() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "small_decimal_point" => {
        if let Some(lcd) = widget_as::<LCDNumber>(widget) {
        Ok(CapabilityValue::Bool(lcd.is_small_decimal_point()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "mode" => {
        if let Some(lcd) = widget_as::<LCDNumber>(widget) {
        Ok(CapabilityValue::String(lcd_mode_to_str(lcd.mode()).to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "segment_style" => {
        if let Some(lcd) = widget_as::<LCDNumber>(widget) {
        Ok(CapabilityValue::String(
        segment_style_to_str(lcd.segment_style()).to_string(),
        ))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Chart => match property_name {
        "task_count" => {
        if let Some(gw) = widget_as::<GanttWidget>(widget) {
        Ok(CapabilityValue::UInt(gw.tasks().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "selected_id" => {
        if let Some(gw) = widget_as::<GanttWidget>(widget) {
        match gw.selected_id() {
        Some(id) => Ok(CapabilityValue::String(id.to_string())),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "viewport_start" => {
        if let Some(gw) = widget_as::<GanttWidget>(widget) {
        Ok(CapabilityValue::Int(gw.viewport().0))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "viewport_end" => {
        if let Some(gw) = widget_as::<GanttWidget>(widget) {
        Ok(CapabilityValue::Int(gw.viewport().1))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "zoom_level" => {
        if let Some(_gw) = widget_as::<GanttWidget>(widget) {
        Ok(CapabilityValue::Null)
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Canvas => match property_name {
        "center_x" => {
        if let Some(mv) = widget_as::<MapView>(widget) {
        Ok(CapabilityValue::Float(mv.center().0 as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "center_y" => {
        if let Some(mv) = widget_as::<MapView>(widget) {
        Ok(CapabilityValue::Float(mv.center().1 as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "zoom" => {
        if let Some(mv) = widget_as::<MapView>(widget) {
        Ok(CapabilityValue::Float(mv.zoom() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "marker_count" => {
        if let Some(mv) = widget_as::<MapView>(widget) {
        Ok(CapabilityValue::UInt(mv.markers().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::WebEngineView => match property_name {
        "url" => {
        if let Some(wv) = widget_as::<WebView>(widget) {
        Ok(CapabilityValue::String(wv.url().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "loading" => {
        if let Some(wv) = widget_as::<WebView>(widget) {
        Ok(CapabilityValue::Bool(wv.is_loading()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "title" => {
        if let Some(wv) = widget_as::<WebView>(widget) {
        Ok(CapabilityValue::String(wv.title().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Grid => match property_name {
        "rows" => {
        if let Some(grid) = widget_as::<GridWidget>(widget) {
        Ok(CapabilityValue::UInt(grid.rows() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "columns" => {
        if let Some(grid) = widget_as::<GridWidget>(widget) {
        Ok(CapabilityValue::UInt(grid.columns() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "spacing" => {
        if let Some(grid) = widget_as::<GridWidget>(widget) {
        Ok(CapabilityValue::UInt(grid.spacing() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::FreeformShape => match property_name {
        "path_kind" => {
        if let Some(fs) = widget_as::<FreeformShapeWidget>(widget) {
        let s = match fs.path() {
        ShapePath::Heart => "heart",
        ShapePath::Star { .. } => "star",
        ShapePath::Polygon(_) => "polygon",
        ShapePath::RoundedRect { .. } => "rounded_rect",
        ShapePath::Bubble { .. } => "bubble",
        ShapePath::Custom(_) => "custom",
        };
        Ok(CapabilityValue::String(s.to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "fill_rgba" => {
        if let Some(fs) = widget_as::<FreeformShapeWidget>(widget) {
        Ok(CapabilityValue::String(fs.fill_color().to_hex_rgba()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "stroke_rgba" => {
        if let Some(fs) = widget_as::<FreeformShapeWidget>(widget) {
        match fs.stroke_color() {
        Some(color) => Ok(CapabilityValue::String(color.to_hex_rgba())),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "stroke_width" => {
        if let Some(fs) = widget_as::<FreeformShapeWidget>(widget) {
        Ok(CapabilityValue::UInt(fs.stroke_width() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Arc => match property_name {
        "value" => {
        if let Some(w) = widget_as::<Arc>(widget) {
        Ok(CapabilityValue::UInt(w.value() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Roller => match property_name {
        "selected_index" => {
        if let Some(w) = widget_as::<Roller>(widget) {
        Ok(CapabilityValue::UInt(w.selected_index() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "visible_count" => {
        if let Some(w) = widget_as::<Roller>(widget) {
        Ok(CapabilityValue::UInt(w.visible_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "item_count" => {
        if let Some(w) = widget_as::<Roller>(widget) {
        Ok(CapabilityValue::UInt(w.options().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Meter => match property_name {
        "value" => {
        if let Some(w) = widget_as::<Meter>(widget) {
        Ok(CapabilityValue::UInt(w.value() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::MiniChart => match property_name {
        "chart_type" => {
        if let Some(w) = widget_as::<MiniChart>(widget) {
        let s = match w.chart_type() {
        ChartType::Line => "line",
        ChartType::Bar => "bar",
        };
        Ok(CapabilityValue::String(s.to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ImageView => match property_name {
        "scaled" => {
        if let Some(w) = widget_as::<ImageView>(widget) {
        Ok(CapabilityValue::Bool(w.is_scaled()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TabView => match property_name {
        "selected_index" => {
        if let Some(w) = widget_as::<TabView>(widget) {
        Ok(CapabilityValue::UInt(w.current_index() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::MaterialNavigationRail => match property_name {
        "selected_index" => {
        if let Some(w) = widget_as::<MaterialNavigationRail>(widget) {
        Ok(CapabilityValue::UInt(w.selected() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::SwipeToDismiss => match property_name {
        "is_dismissed" => {
        if let Some(w) = widget_as::<SwipeToDismiss>(widget) {
        Ok(CapabilityValue::Bool(w.is_dismissed()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::LineChart => match property_name {
        "stroke_width" => {
        if let Some(w) = widget_as::<LineChart>(widget) {
        Ok(CapabilityValue::Float(w.stroke_width() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Sparkline => match property_name {
        "stroke_width" => {
        if let Some(w) = widget_as::<Sparkline>(widget) {
        Ok(CapabilityValue::Float(w.stroke_width() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::BarChart => match property_name {
        "bar_spacing" => {
        if let Some(w) = widget_as::<BarChart>(widget) {
        Ok(CapabilityValue::Float(w.bar_spacing() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::PieChart => match property_name {
        "donut" => {
        if let Some(w) = widget_as::<PieChart>(widget) {
        Ok(CapabilityValue::Bool(w.is_donut()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::BarcodeScanner => match property_name {
        "is_scanning" => {
        if let Some(w) = widget_as::<BarcodeScanner>(widget) {
        Ok(CapabilityValue::Bool(w.is_scanning()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::BezierCurveEditor => match property_name {
        "snap_to_grid" => {
        if let Some(w) = widget_as::<BezierCurveEditor>(widget) {
        Ok(CapabilityValue::Bool(w.snap_to_grid()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CupertinoSlider => match property_name {
        "value" => {
        if let Some(w) = widget_as::<CupertinoSlider>(widget) {
        Ok(CapabilityValue::Float(w.value().into()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "min" => {
        if let Some(w) = widget_as::<CupertinoSlider>(widget) {
        Ok(CapabilityValue::Float(w.min().into()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "max" => {
        if let Some(w) = widget_as::<CupertinoSlider>(widget) {
        Ok(CapabilityValue::Float(w.max().into()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}
