#[cfg(not(feature = "mini"))]
pub fn write_other_props(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::LCDNumber => {
        if let Some(lcd) = widget_as_mut::<LCDNumber>(widget) {
        match property_name {
        "value" => {
        lcd.set_value(expect_f64(value)?);
        Ok(())
        }
        "min_value" => {
        lcd.set_min_value(expect_f64(value)?);
        Ok(())
        }
        "max_value" => {
        lcd.set_max_value(expect_f64(value)?);
        Ok(())
        }
        "num_digits" => {
        lcd.set_num_digits(expect_i64(value)? as i32);
        Ok(())
        }
        "small_decimal_point" => {
        lcd.set_small_decimal_point(expect_bool(value)?);
        Ok(())
        }
        "mode" => {
        lcd.set_mode(expect_lcd_mode(value)?);
        Ok(())
        }
        "segment_style" => {
        lcd.set_segment_style(expect_segment_style(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Chart => {
        if let Some(gantt_widget) = widget_as_mut::<GanttWidget>(widget) {
        match property_name {
        "viewport_start" => {
        let start = expect_i64(value)?;
        let (_, end) = gantt_widget.viewport();
        gantt_widget.set_viewport(start, end);
        Ok(())
        }
        "viewport_end" => {
        let end = expect_i64(value)?;
        let (start, _) = gantt_widget.viewport();
        gantt_widget.set_viewport(start, end);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Canvas => {
        if let Some(map_view) = widget_as_mut::<MapView>(widget) {
        match property_name {
        "center_x" => {
        let x = expect_f32(value)?;
        let (_, y) = map_view.center();
        map_view.set_center(x, y);
        Ok(())
        }
        "center_y" => {
        let y = expect_f32(value)?;
        let (x, _) = map_view.center();
        map_view.set_center(x, y);
        Ok(())
        }
        "zoom" => {
        map_view.set_zoom(expect_f32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::WebEngineView => {
        if let Some(media_player) = widget_as_mut::<MediaPlayer>(widget) {
        match property_name {
        "source" => match value {
        CapabilityValue::Null => {
        media_player.clear_source();
        Ok(())
        }
        other => {
        let source = expect_string(other)?;
        let duration = media_player.duration_ms();
        media_player.set_source(source, duration);
        Ok(())
        }
        },
        "playing" => {
        if expect_bool(value)? {
        let _ = media_player.play();
        } else {
        media_player.pause();
        }
        Ok(())
        }
        "position_ms" => {
        media_player.seek_to(expect_usize(value)? as u64);
        Ok(())
        }
        "volume" => {
        media_player.set_volume(expect_u32(value)? as u8);
        Ok(())
        }
        "muted" => {
        media_player.set_muted(expect_bool(value)?);
        Ok(())
        }
        "fullscreen" => {
        media_player.set_fullscreen(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Grid => {
        if let Some(grid) = widget_as_mut::<GridWidget>(widget) {
        match property_name {
        "rows" => {
        grid.set_rows(expect_u32(value)?);
        Ok(())
        }
        "columns" => {
        grid.set_columns(expect_u32(value)?);
        Ok(())
        }
        "spacing" => {
        grid.set_spacing(expect_u32(value)?);
        Ok(())
        }
        "line_color" => {
        match value {
        CapabilityValue::Null => grid.set_line_color(None),
        CapabilityValue::String(raw) => {
        let Some(color) = crate::core::Color::parse_hex(&raw) else {
        return Err(CapabilityAccessError::TypeMismatch);
        };
        grid.set_line_color(Some(color));
        }
        _ => return Err(CapabilityAccessError::TypeMismatch),
        }
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::FreeformShape => {
        if let Some(shape) = widget_as_mut::<FreeformShapeWidget>(widget) {
        match property_name {
        "fill_rgba" => {
        let raw = expect_string(value)?;
        let Some(color) = crate::core::Color::parse_hex(&raw) else {
        return Err(CapabilityAccessError::TypeMismatch);
        };
        shape.set_fill_color(color);
        Ok(())
        }
        "stroke_rgba" => {
        match value {
        CapabilityValue::Null => shape.set_stroke_color(None),
        CapabilityValue::String(raw) => {
        let Some(color) = crate::core::Color::parse_hex(&raw) else {
        return Err(CapabilityAccessError::TypeMismatch);
        };
        shape.set_stroke_color(Some(color));
        }
        _ => return Err(CapabilityAccessError::TypeMismatch),
        }
        Ok(())
        }
        "stroke_width" => {
        shape.set_stroke_width(expect_u32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Arc => {
        if let Some(w) = widget_as_mut::<Arc>(widget) {
        match property_name {
        "value" => {
        w.set_value(expect_u32(value)?);
        Ok(())
        }
        "thickness" => {
        w.set_thickness(expect_u32(value)?);
        Ok(())
        }
        "sweep_angle" => {
        w.set_sweep_angle(expect_u32(value)? as u16);
        Ok(())
        }
        "indeterminate" => {
        w.set_indeterminate(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Roller => {
        if let Some(w) = widget_as_mut::<Roller>(widget) {
        match property_name {
        "selected_index" => {
        w.set_selected_index(expect_usize(value)?);
        Ok(())
        }
        "visible_count" => {
        w.set_visible_count(expect_u32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Meter => {
        if let Some(w) = widget_as_mut::<Meter>(widget) {
        match property_name {
        "value" => {
        w.set_value(expect_u32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::MiniChart => {
        if let Some(w) = widget_as_mut::<MiniChart>(widget) {
        match property_name {
        "chart_type" => {
        let s = expect_string(value)?;
        let ct = match s.as_str() {
        "line" => ChartType::Line,
        "bar" => ChartType::Bar,
        _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        w.set_chart_type(ct);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::ImageView => {
        if let Some(w) = widget_as_mut::<ImageView>(widget) {
        match property_name {
        "scaled" => {
        w.set_scaled(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::TabView => {
        if let Some(w) = widget_as_mut::<TabView>(widget) {
        match property_name {
        "selected_index" => {
        w.set_current_index(expect_usize(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::MaterialNavigationRail => {
        if let Some(w) = widget_as_mut::<MaterialNavigationRail>(widget) {
        match property_name {
        "selected_index" => {
        w.set_selected(expect_usize(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::SwipeToDismiss => {
        if let Some(_w) = widget_as_mut::<SwipeToDismiss>(widget) {
        match property_name {
        "is_dismissed" => Err(CapabilityAccessError::UnsupportedOnWidget),
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::LineChart => {
        if let Some(w) = widget_as_mut::<LineChart>(widget) {
        match property_name {
        "stroke_width" => {
        w.set_stroke_width(expect_f32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Sparkline => {
        if let Some(w) = widget_as_mut::<Sparkline>(widget) {
        match property_name {
        "stroke_width" => {
        w.set_stroke_width(expect_f32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::BarChart => {
        if let Some(w) = widget_as_mut::<BarChart>(widget) {
        match property_name {
        "bar_spacing" => {
        w.set_bar_spacing(expect_f32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::PieChart => {
        if let Some(w) = widget_as_mut::<PieChart>(widget) {
        match property_name {
        "donut" => {
        w.set_donut_mode(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::BarcodeScanner => {
        if let Some(w) = widget_as_mut::<BarcodeScanner>(widget) {
        match property_name {
        "is_scanning" => {
        if expect_bool(value)? {
        w.start_scanning();
        } else {
        w.stop_scanning();
        }
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::BezierCurveEditor => {
        if let Some(w) = widget_as_mut::<BezierCurveEditor>(widget) {
        match property_name {
        "snap_to_grid" => {
        w.set_snap_to_grid(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::CupertinoSlider => {
        if let Some(w) = widget_as_mut::<CupertinoSlider>(widget) {
        match property_name {
        "value" => {
        w.set_value(expect_f32(value)?);
        Ok(())
        }
        "min" => {
        w.set_min(expect_f32(value)?);
        Ok(())
        }
        "max" => {
        w.set_max(expect_f32(value)?);
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
