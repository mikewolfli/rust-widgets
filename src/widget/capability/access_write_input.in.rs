#[cfg(not(feature = "mini"))]
pub fn write_input_props(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Slider => {
        if let Some(slider) = widget_as_mut::<Slider>(widget) {
        match property_name {
        "minimum" => {
        slider.set_minimum(expect_i64(value)? as i32);
        Ok(())
        }
        "maximum" => {
        slider.set_maximum(expect_i64(value)? as i32);
        Ok(())
        }
        "value" => {
        slider.set_value(expect_i64(value)? as i32);
        Ok(())
        }
        "single_step" => {
        slider.set_single_step(expect_i64(value)? as i32);
        Ok(())
        }
        "page_step" => {
        slider.set_page_step(expect_i64(value)? as i32);
        Ok(())
        }
        "orientation" => {
        slider.set_orientation(expect_orientation(value)?);
        Ok(())
        }
        "tick_position" => {
        slider.set_tick_position(expect_tick_position(value)?);
        Ok(())
        }
        "tick_interval" => {
        slider.set_tick_interval(expect_i64(value)? as i32);
        Ok(())
        }
        "tracking" => {
        slider.set_tracking(expect_bool(value)?);
        Ok(())
        }
        "slider_position" => {
        slider.set_slider_position(expect_i64(value)? as i32);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::ProgressBar => {
        if let Some(progress_bar) = widget_as_mut::<ProgressBar>(widget) {
        match property_name {
        "minimum" => {
        progress_bar.set_minimum(expect_i64(value)? as i32);
        Ok(())
        }
        "maximum" => {
        progress_bar.set_maximum(expect_i64(value)? as i32);
        Ok(())
        }
        "value" => {
        progress_bar.set_value(expect_i64(value)? as i32);
        Ok(())
        }
        "text_visible" => {
        progress_bar.set_text_visible(expect_bool(value)?);
        Ok(())
        }
        "orientation" => {
        progress_bar.set_orientation(expect_orientation(value)?);
        Ok(())
        }
        "inverted_appearance" => {
        progress_bar.set_inverted_appearance(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::ScrollBar => {
        if let Some(scroll_bar) = widget_as_mut::<ScrollBar>(widget) {
        match property_name {
        "minimum" => {
        scroll_bar.set_minimum(expect_i64(value)? as i32);
        Ok(())
        }
        "maximum" => {
        scroll_bar.set_maximum(expect_i64(value)? as i32);
        Ok(())
        }
        "value" => {
        scroll_bar.set_value(expect_i64(value)? as i32);
        Ok(())
        }
        "single_step" => {
        scroll_bar.set_single_step(expect_i64(value)? as i32);
        Ok(())
        }
        "page_step" => {
        scroll_bar.set_page_step(expect_i64(value)? as i32);
        Ok(())
        }
        "orientation" => {
        scroll_bar.set_orientation(expect_orientation(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::ListBox => {
        if let Some(list_box) = widget_as_mut::<ListBox>(widget) {
        match property_name {
        "selection_mode" => {
        list_box.set_selection_mode(expect_list_box_selection_mode(value)?);
        Ok(())
        }
        "current_row" => {
        match value {
        CapabilityValue::Null => list_box.set_current_row(None),
        other => list_box.set_current_row(Some(expect_usize(other)?)),
        }
        Ok(())
        }
        "item_height" => {
        list_box.set_item_height(expect_f32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::SpinBox => {
        if let Some(spin_box) = widget_as_mut::<SpinBox>(widget) {
        match property_name {
        "minimum" => {
        spin_box.set_minimum(expect_i64(value)? as i32);
        Ok(())
        }
        "maximum" => {
        spin_box.set_maximum(expect_i64(value)? as i32);
        Ok(())
        }
        "value" => {
        spin_box.set_value(expect_i64(value)? as i32);
        Ok(())
        }
        "single_step" => {
        spin_box.set_single_step(expect_i64(value)? as i32);
        Ok(())
        }
        "prefix" => {
        spin_box.set_prefix(expect_string(value)?);
        Ok(())
        }
        "suffix" => {
        spin_box.set_suffix(expect_string(value)?);
        Ok(())
        }
        "special_value_text" => {
        match value {
        CapabilityValue::Null => spin_box.set_special_value_text(None),
        other => {
        spin_box.set_special_value_text(Some(expect_string(other)?));
        }
        }
        Ok(())
        }
        "wrapping" => {
        spin_box.set_wrapping(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::ComboBox => {
        if let Some(combo_box) = widget_as_mut::<ComboBox>(widget) {
        match property_name {
        "current_index" => {
        match value {
        CapabilityValue::Null => combo_box.set_current_index(None),
        other => combo_box.set_current_index(Some(expect_usize(other)?)),
        }
        Ok(())
        }
        "current_text" => {
        combo_box.set_current_text(expect_string(value)?);
        Ok(())
        }
        "editable" => {
        combo_box.set_editable(expect_bool(value)?);
        Ok(())
        }
        "max_visible_items" => {
        combo_box.set_max_visible_items(expect_usize(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Dial => {
        if let Some(dial) = widget_as_mut::<Dial>(widget) {
        match property_name {
        "minimum" => {
        dial.set_minimum(expect_i64(value)? as i32);
        Ok(())
        }
        "maximum" => {
        dial.set_maximum(expect_i64(value)? as i32);
        Ok(())
        }
        "value" => {
        dial.set_value(expect_i64(value)? as i32);
        Ok(())
        }
        "single_step" => {
        dial.set_single_step(expect_i64(value)? as i32);
        Ok(())
        }
        "page_step" => {
        dial.set_page_step(expect_i64(value)? as i32);
        Ok(())
        }
        "notches_visible" => {
        dial.set_notches_visible(expect_bool(value)?);
        Ok(())
        }
        "notch_target" => {
        dial.set_notch_target(expect_f64(value)?);
        Ok(())
        }
        "wrapping" => {
        dial.set_wrapping(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::CommandLink => {
        if let Some(command_link) = widget_as_mut::<CommandLink>(widget) {
        match property_name {
        "text" => {
        command_link.set_text(expect_string(value)?);
        Ok(())
        }
        "description" => {
        command_link.set_description(expect_string(value)?);
        Ok(())
        }
        "enabled" => {
        command_link.set_enabled(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::FontComboBox => {
        if let Some(font_combo) = widget_as_mut::<FontComboBox>(widget) {
        match property_name {
        "current_index" => {
        font_combo.set_current_index(expect_i64(value)? as i32);
        Ok(())
        }
        "editable" => {
        font_combo.set_editable(expect_bool(value)?);
        Ok(())
        }
        "max_visible_items" => {
        font_combo.set_max_visible_items(expect_i64(value)? as i32);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::LineEdit => {
        if let Some(line_edit) = widget_as_mut::<LineEdit>(widget) {
        match property_name {
        "text" => {
        line_edit.set_text(expect_string(value)?);
        Ok(())
        }
        "placeholder_text" => {
        line_edit.set_placeholder_text(expect_string(value)?);
        Ok(())
        }
        "max_length" => {
        match value {
        CapabilityValue::Null => line_edit.set_max_length(None),
        other => line_edit.set_max_length(Some(expect_usize(other)?)),
        }
        Ok(())
        }
        "read_only" => {
        line_edit.set_read_only(expect_bool(value)?);
        Ok(())
        }
        "cursor_position" => {
        line_edit.set_cursor_position(expect_usize(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::RichEdit => {
        if let Some(code_editor) = widget_as_mut::<CodeEditor>(widget) {
        match property_name {
        "text" => {
        code_editor.set_text(expect_string(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::TextEdit => {
        if let Some(terminal_view) = widget_as_mut::<TerminalView>(widget) {
        match property_name {
        "input_line" => {
        terminal_view.set_input_line(expect_string(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::CheckListBox => {
        if let Some(chip) = widget_as_mut::<Chip>(widget) {
        match property_name {
        "multi_select" => {
        chip.set_multi_select(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Spinner => {
        if let Some(w) = widget_as_mut::<Spinner>(widget) {
        match property_name {
        "active" => {
        w.set_active(expect_bool(value)?);
        Ok(())
        }
        "thickness" => {
        w.set_thickness(expect_u32(value)?);
        Ok(())
        }
        "speed" => {
        w.set_speed(expect_f32(value)?);
        Ok(())
        }
        "size_ratio" => {
        w.set_size_ratio(expect_f32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Dropdown => {
        if let Some(w) = widget_as_mut::<Dropdown>(widget) {
        match property_name {
        "selected_index" => {
        w.set_selected_index(expect_usize(value)?);
        Ok(())
        }
        "expanded" => {
        w.set_expanded(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::TextArea => {
        if let Some(w) = widget_as_mut::<TextArea>(widget) {
        match property_name {
        "text" => {
        w.set_text(expect_string(value)?);
        Ok(())
        }
        "placeholder" => {
        w.set_placeholder(expect_string(value)?);
        Ok(())
        }
        "read_only" => {
        w.set_read_only(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Keyboard => {
        if let Some(w) = widget_as_mut::<Keyboard>(widget) {
        match property_name {
        "layout" => {
        let s = expect_string(value)?;
        let layout = match s.as_str() {
        "qwerty" => KeyboardLayout::Qwerty,
        "numeric" => KeyboardLayout::Numeric,
        _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        w.set_layout(layout);
        Ok(())
        }
        "lowercase" => {
        w.set_lowercase(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::Switch => {
        if let Some(w) = widget_as_mut::<Switch>(widget) {
        match property_name {
        "checked" => {
        w.set_checked(expect_bool(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::SearchBar => {
        if let Some(w) = widget_as_mut::<SearchBar>(widget) {
        match property_name {
        "text" => {
        w.set_text(expect_string(value)?);
        Ok(())
        }
        "placeholder" => {
        w.set_placeholder(expect_string(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::ShortcutEditor => {
        if let Some(w) = widget_as_mut::<ShortcutEditor>(widget) {
        match property_name {
        "filter_text" => {
        let text = expect_string(value)?;
        w.set_filter(&text);
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
