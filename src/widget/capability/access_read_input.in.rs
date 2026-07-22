#[cfg(not(feature = "mini"))]
pub fn read_input_props(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Slider => match property_name {
        "minimum" => {
        if let Some(slider) = widget_as::<Slider>(widget) {
        Ok(CapabilityValue::Int(slider.minimum() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "maximum" => {
        if let Some(slider) = widget_as::<Slider>(widget) {
        Ok(CapabilityValue::Int(slider.maximum() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "value" => {
        if let Some(slider) = widget_as::<Slider>(widget) {
        Ok(CapabilityValue::Int(slider.value() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "single_step" => {
        if let Some(slider) = widget_as::<Slider>(widget) {
        Ok(CapabilityValue::Int(slider.single_step() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "page_step" => {
        if let Some(slider) = widget_as::<Slider>(widget) {
        Ok(CapabilityValue::Int(slider.page_step() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "orientation" => {
        if let Some(slider) = widget_as::<Slider>(widget) {
        Ok(CapabilityValue::String(
        orientation_to_str(slider.orientation()).to_string(),
        ))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "tick_position" => {
        if let Some(slider) = widget_as::<Slider>(widget) {
        Ok(CapabilityValue::String(
        tick_position_to_str(slider.tick_position()).to_string(),
        ))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "tick_interval" => {
        if let Some(slider) = widget_as::<Slider>(widget) {
        Ok(CapabilityValue::Int(slider.tick_interval() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "tracking" => {
        if let Some(slider) = widget_as::<Slider>(widget) {
        Ok(CapabilityValue::Bool(slider.tracking()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "slider_position" => {
        if let Some(slider) = widget_as::<Slider>(widget) {
        Ok(CapabilityValue::Int(slider.slider_position() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ProgressBar => match property_name {
        "minimum" => {
        if let Some(pb) = widget_as::<ProgressBar>(widget) {
        Ok(CapabilityValue::Int(pb.minimum() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "maximum" => {
        if let Some(pb) = widget_as::<ProgressBar>(widget) {
        Ok(CapabilityValue::Int(pb.maximum() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "value" => {
        if let Some(pb) = widget_as::<ProgressBar>(widget) {
        Ok(CapabilityValue::Int(pb.value() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "text_visible" => {
        if let Some(pb) = widget_as::<ProgressBar>(widget) {
        Ok(CapabilityValue::Bool(pb.is_text_visible()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "orientation" => {
        if let Some(pb) = widget_as::<ProgressBar>(widget) {
        Ok(CapabilityValue::String(orientation_to_str(pb.orientation()).to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "inverted_appearance" => {
        if let Some(pb) = widget_as::<ProgressBar>(widget) {
        Ok(CapabilityValue::Bool(pb.is_inverted_appearance()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "progress" => {
        if let Some(pb) = widget_as::<ProgressBar>(widget) {
        Ok(CapabilityValue::Float(pb.progress() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ScrollBar => match property_name {
        "minimum" => {
        if let Some(sb) = widget_as::<ScrollBar>(widget) {
        Ok(CapabilityValue::Int(sb.minimum() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "maximum" => {
        if let Some(sb) = widget_as::<ScrollBar>(widget) {
        Ok(CapabilityValue::Int(sb.maximum() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "value" => {
        if let Some(sb) = widget_as::<ScrollBar>(widget) {
        Ok(CapabilityValue::Int(sb.value() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "single_step" => {
        if let Some(sb) = widget_as::<ScrollBar>(widget) {
        Ok(CapabilityValue::Int(sb.single_step() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "page_step" => {
        if let Some(sb) = widget_as::<ScrollBar>(widget) {
        Ok(CapabilityValue::Int(sb.page_step() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "orientation" => {
        if let Some(sb) = widget_as::<ScrollBar>(widget) {
        Ok(CapabilityValue::String(orientation_to_str(sb.orientation()).to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "slider_size" => {
        if let Some(sb) = widget_as::<ScrollBar>(widget) {
        Ok(CapabilityValue::Float(sb.slider_size() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "slider_position" => {
        if let Some(sb) = widget_as::<ScrollBar>(widget) {
        Ok(CapabilityValue::Float(sb.slider_position() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ListBox => match property_name {
        "item_count" => {
        if let Some(lb) = widget_as::<ListBox>(widget) {
        Ok(CapabilityValue::UInt(lb.count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "selection_mode" => {
        if let Some(lb) = widget_as::<ListBox>(widget) {
        Ok(CapabilityValue::String(
        list_box_selection_mode_to_str(lb.selection_mode()).to_string(),
        ))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "current_row" => {
        if let Some(lb) = widget_as::<ListBox>(widget) {
        match lb.current_row() {
        Some(row) => Ok(CapabilityValue::UInt(row as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "item_height" => {
        if let Some(lb) = widget_as::<ListBox>(widget) {
        Ok(CapabilityValue::Float(lb.item_height() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "selected_count" => {
        if let Some(lb) = widget_as::<ListBox>(widget) {
        Ok(CapabilityValue::UInt(lb.selected_indices().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::SpinBox => match property_name {
        "minimum" => {
        if let Some(sb) = widget_as::<SpinBox>(widget) {
        Ok(CapabilityValue::Int(sb.minimum() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "maximum" => {
        if let Some(sb) = widget_as::<SpinBox>(widget) {
        Ok(CapabilityValue::Int(sb.maximum() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "value" => {
        if let Some(sb) = widget_as::<SpinBox>(widget) {
        Ok(CapabilityValue::Int(sb.value() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "single_step" => {
        if let Some(sb) = widget_as::<SpinBox>(widget) {
        Ok(CapabilityValue::Int(sb.single_step() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "prefix" => {
        if let Some(sb) = widget_as::<SpinBox>(widget) {
        Ok(CapabilityValue::String(sb.prefix().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "suffix" => {
        if let Some(sb) = widget_as::<SpinBox>(widget) {
        Ok(CapabilityValue::String(sb.suffix().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "special_value_text" => {
        if let Some(sb) = widget_as::<SpinBox>(widget) {
        match sb.special_value_text() {
        Some(text) => Ok(CapabilityValue::String(text.to_string())),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "wrapping" => {
        if let Some(sb) = widget_as::<SpinBox>(widget) {
        Ok(CapabilityValue::Bool(sb.wrapping()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ComboBox => match property_name {
        "item_count" => {
        if let Some(cb) = widget_as::<ComboBox>(widget) {
        Ok(CapabilityValue::UInt(cb.count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "current_index" => {
        if let Some(cb) = widget_as::<ComboBox>(widget) {
        match cb.current_index() {
        Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "current_text" => {
        if let Some(cb) = widget_as::<ComboBox>(widget) {
        Ok(CapabilityValue::String(cb.current_text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "editable" => {
        if let Some(cb) = widget_as::<ComboBox>(widget) {
        Ok(CapabilityValue::Bool(cb.is_editable()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "max_visible_items" => {
        if let Some(cb) = widget_as::<ComboBox>(widget) {
        Ok(CapabilityValue::UInt(cb.max_visible_items() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Dial => match property_name {
        "minimum" => {
        if let Some(dial) = widget_as::<Dial>(widget) {
        Ok(CapabilityValue::Int(dial.minimum() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "maximum" => {
        if let Some(dial) = widget_as::<Dial>(widget) {
        Ok(CapabilityValue::Int(dial.maximum() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "value" => {
        if let Some(dial) = widget_as::<Dial>(widget) {
        Ok(CapabilityValue::Int(dial.value() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "single_step" => {
        if let Some(dial) = widget_as::<Dial>(widget) {
        Ok(CapabilityValue::Int(dial.single_step() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "page_step" => {
        if let Some(dial) = widget_as::<Dial>(widget) {
        Ok(CapabilityValue::Int(dial.page_step() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "notches_visible" => {
        if let Some(dial) = widget_as::<Dial>(widget) {
        Ok(CapabilityValue::Bool(dial.notches_visible()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "notch_target" => {
        if let Some(dial) = widget_as::<Dial>(widget) {
        Ok(CapabilityValue::Float(dial.notch_target()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "wrapping" => {
        if let Some(dial) = widget_as::<Dial>(widget) {
        Ok(CapabilityValue::Bool(dial.wrapping()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CommandLink => match property_name {
        "text" => {
        if let Some(cl) = widget_as::<CommandLink>(widget) {
        Ok(CapabilityValue::String(cl.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "description" => {
        if let Some(cl) = widget_as::<CommandLink>(widget) {
        Ok(CapabilityValue::String(cl.description().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "enabled" => {
        if let Some(cl) = widget_as::<CommandLink>(widget) {
        Ok(CapabilityValue::Bool(cl.is_enabled()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::FontComboBox => match property_name {
        "current_font_family" => {
        if let Some(fcb) = widget_as::<FontComboBox>(widget) {
        Ok(CapabilityValue::String(fcb.current_text()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "item_count" => {
        if let Some(fcb) = widget_as::<FontComboBox>(widget) {
        Ok(CapabilityValue::Int(fcb.count() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "current_index" => {
        if let Some(fcb) = widget_as::<FontComboBox>(widget) {
        Ok(CapabilityValue::Int(fcb.current_index() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "editable" => {
        if let Some(fcb) = widget_as::<FontComboBox>(widget) {
        Ok(CapabilityValue::Bool(fcb.is_editable()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "max_visible_items" => {
        if let Some(fcb) = widget_as::<FontComboBox>(widget) {
        Ok(CapabilityValue::Int(fcb.max_visible_items() as i64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::LineEdit => match property_name {
        "text" => {
        if let Some(le) = widget_as::<LineEdit>(widget) {
        Ok(CapabilityValue::String(le.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "placeholder_text" => {
        if let Some(le) = widget_as::<LineEdit>(widget) {
        Ok(CapabilityValue::String(le.placeholder_text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "max_length" => {
        if let Some(le) = widget_as::<LineEdit>(widget) {
        match le.max_length() {
        Some(len) => Ok(CapabilityValue::UInt(len as u64)),
        None => Ok(CapabilityValue::Null),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "read_only" => {
        if let Some(le) = widget_as::<LineEdit>(widget) {
        Ok(CapabilityValue::Bool(le.is_read_only()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "cursor_position" => {
        if let Some(le) = widget_as::<LineEdit>(widget) {
        Ok(CapabilityValue::UInt(le.cursor_position() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::RichEdit => match property_name {
        "text" => {
        if let Some(ce) = widget_as::<CodeEditor>(widget) {
        Ok(CapabilityValue::String(ce.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "line_count" => {
        if let Some(ce) = widget_as::<CodeEditor>(widget) {
        Ok(CapabilityValue::UInt(ce.line_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "cursor_line" => {
        if let Some(ce) = widget_as::<CodeEditor>(widget) {
        Ok(CapabilityValue::UInt(ce.cursor().0 as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "cursor_column" => {
        if let Some(ce) = widget_as::<CodeEditor>(widget) {
        Ok(CapabilityValue::UInt(ce.cursor().1 as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TextEdit => match property_name {
        "output_line_count" => {
        if let Some(tv) = widget_as::<TerminalView>(widget) {
        Ok(CapabilityValue::UInt(tv.lines().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "input_line" => {
        if let Some(tv) = widget_as::<TerminalView>(widget) {
        Ok(CapabilityValue::String(tv.input_line().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CheckListBox => match property_name {
        "item_count" => {
        if let Some(chip) = widget_as::<Chip>(widget) {
        Ok(CapabilityValue::UInt(chip.items().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "multi_select" => {
        if let Some(chip) = widget_as::<Chip>(widget) {
        Ok(CapabilityValue::Bool(chip.multi_select()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Spinner => match property_name {
        "active" => {
        if let Some(w) = widget_as::<Spinner>(widget) {
        Ok(CapabilityValue::Bool(w.is_active()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "thickness" => {
        if let Some(w) = widget_as::<Spinner>(widget) {
        Ok(CapabilityValue::UInt(w.thickness() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "speed" => {
        if let Some(w) = widget_as::<Spinner>(widget) {
        Ok(CapabilityValue::Float(w.speed() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "size_ratio" => {
        if let Some(w) = widget_as::<Spinner>(widget) {
        Ok(CapabilityValue::Float(w.size_ratio() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Dropdown => match property_name {
        "text" => {
        if let Some(w) = widget_as::<Dropdown>(widget) {
        match w.selected_text() {
        Some(t) => Ok(CapabilityValue::String(t.to_string())),
        None => Ok(CapabilityValue::String(String::new())),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "selected_index" => {
        if let Some(w) = widget_as::<Dropdown>(widget) {
        Ok(CapabilityValue::UInt(w.selected_index() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "item_count" => {
        if let Some(w) = widget_as::<Dropdown>(widget) {
        Ok(CapabilityValue::UInt(w.items().len() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "expanded" => {
        if let Some(w) = widget_as::<Dropdown>(widget) {
        Ok(CapabilityValue::Bool(w.is_expanded()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TextArea => match property_name {
        "text" => {
        if let Some(w) = widget_as::<TextArea>(widget) {
        Ok(CapabilityValue::String(w.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "placeholder" => {
        if let Some(w) = widget_as::<TextArea>(widget) {
        Ok(CapabilityValue::String(w.placeholder().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "read_only" => {
        if let Some(w) = widget_as::<TextArea>(widget) {
        Ok(CapabilityValue::Bool(w.is_read_only()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Keyboard => match property_name {
        "layout" => {
        if let Some(w) = widget_as::<Keyboard>(widget) {
        let s = match w.layout() {
        KeyboardLayout::Qwerty => "qwerty",
        KeyboardLayout::Numeric => "numeric",
        };
        Ok(CapabilityValue::String(s.to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "lowercase" => {
        if let Some(w) = widget_as::<Keyboard>(widget) {
        Ok(CapabilityValue::Bool(w.lowercase()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Switch => match property_name {
        "checked" => {
        if let Some(w) = widget_as::<Switch>(widget) {
        Ok(CapabilityValue::Bool(w.is_checked()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::SearchBar => match property_name {
        "text" => {
        if let Some(w) = widget_as::<SearchBar>(widget) {
        Ok(CapabilityValue::String(w.text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "placeholder" => {
        if let Some(w) = widget_as::<SearchBar>(widget) {
        Ok(CapabilityValue::String(w.placeholder().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ShortcutEditor => match property_name {
        "filter_text" => {
        if let Some(w) = widget_as::<ShortcutEditor>(widget) {
        Ok(CapabilityValue::String(w.filter_text().to_string()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}
