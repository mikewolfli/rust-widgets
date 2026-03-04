# GPU Control Coverage Matrix

This document tracks GPU visual builder coverage for all custom-painted controls.

## Overview

| Status | Count | Description |
|--------|-------|-------------|
| ✅ Covered | 32 | Controls with GPU visual builders |
| ⏳ Planned | 0 | No additional planned coverage |
| N/A | 3 | Dialog family uses Window visual builder |

**Total Controls**: 35

## Coverage Matrix

| WidgetKind | Visual Builder Function | Status |
|------------|-------------------------|--------|
| Window | `append_window_visual_commands` | ✅ |
| Dialog | `append_window_visual_commands` | ✅ (inherited) |
| MessageBox | `append_window_visual_commands` | ✅ (inherited) |
| FileDialog | `append_window_visual_commands` | ✅ (inherited) |
| ColorDialog | `append_window_visual_commands` | ✅ (inherited) |
| FontDialog | `append_window_visual_commands` | ✅ (inherited) |
| PopupWindow | `append_window_visual_commands` | ✅ (inherited) |
| Button | `append_button_visual_commands` | ✅ |
| CheckBox | `append_checkbox_visual_commands` | ✅ |
| RadioButton | `append_radiobutton_visual_commands` | ✅ |
| Label | `append_label_visual_commands` | ✅ |
| LineEdit | `append_line_edit_visual_commands` | ✅ |
| TextEdit | `append_text_edit_visual_commands` | ✅ |
| RichEdit | `append_rich_edit_visual_commands` | ✅ |
| ComboBox | `append_combo_box_visual_commands` | ✅ |
| SpinBox | `append_spin_box_visual_commands` | ✅ |
| ListBox | `append_list_box_visual_commands` | ✅ |
| ListView | `append_list_view_visual_commands` | ✅ |
| TreeView | `append_tree_view_visual_commands` | ✅ |
| ProgressBar | `append_progress_bar_visual_commands` | ✅ |
| Slider | `append_slider_visual_commands` | ✅ |
| ScrollBar | `append_scroll_bar_visual_commands` | ✅ |
| ScrollArea | `append_scroll_area_visual_commands` | ✅ |
| Panel | `append_panel_visual_commands` | ✅ |
| DockPanel | `append_dock_panel_visual_commands` | ✅ |
| GroupBox | `append_group_box_visual_commands` | ✅ |
| TabWidget | `append_tab_widget_visual_commands` | ✅ |
| Splitter | `append_splitter_visual_commands` | ✅ |
| StackWidget | `append_stack_widget_visual_commands` | ✅ |
| MdiArea | `append_mdi_area_visual_commands` | ✅ |
| MenuBar | `append_menu_bar_visual_commands` | ✅ |
| Menu | `append_menu_visual_commands` | ✅ |
| ToolBar | `append_tool_bar_visual_commands` | ✅ |
| StatusBar | `append_status_bar_visual_commands` | ✅ |
| Canvas | `append_canvas_visual_commands` | ✅ |
| Table | `append_table_widget_visual_commands` | ✅ |
| Grid | `append_grid_widget_visual_commands` | ✅ |
| Chart | `append_chart_widget_visual_commands` | ✅ |

## Implementation Status

### Completed (v22 P0)

- **P0a**: Text editing controls - TextEdit, RichEdit ✅
- **P0b**: Data display controls - TreeView, Table, Grid, Chart ✅
- **P0c**: Dialog family - Dialog, MessageBox, FileDialog, ColorDialog, FontDialog, PopupWindow ✅
- **P0d**: Container/layout controls - DockPanel, GroupBox, Splitter, MdiArea, Canvas ✅
- **P0e**: GPU parity regression tests ✅
- **P0f**: This documentation ✅

### Completed (v23)

- **P0**: Missing widget struct implementations
    - **P0a**: MessageBox widget struct ✅
    - **P0b**: FileDialog widget struct ✅
    - **P0c**: ColorDialog widget struct ✅
    - **P0d**: FontDialog widget struct ✅
    - **P0e**: SpinBox widget struct + GPU visual builder ✅
    - **P0f**: ListView widget struct + GPU visual builder ✅
    - **P0g**: ScrollArea widget struct + GPU visual builder ✅
    - **P0h**: Convenient wrapper functions in lib.rs ✅
    - **P0i**: Platform trait create methods ✅
    - **P0j**: GPU visual builders for new widgets ✅

### Pending (Future Versions)

- **P1**: Custom paint event system closure
- **P2**: Full-custom backend parity gates

## Missing Visual Builders

All 35 controls now have GPU visual builder coverage! 🎉

## Testing

All GPU visual builders are tested via:
- `gpu_parity_covered_controls_auto_compose_runs_with_gpu_or_cpu_backend`
- Unit tests in `render::tests`
- Integration tests in demos

## Adding New Visual Builders

To add a GPU visual builder for a new control:

1. Implement the function in `src/render/mod.rs`:
   ```rust
   pub fn append_<widget>_visual_commands(layer: &mut SceneLayer, widget: &<WidgetType>) {
       // Get geometry
       let rect = widget.geometry();
       if rect.width == 0 || rect.height == 0 {
           return;
       }

       // Draw background/fill
       layer.push(RenderCommand::FillRect { ... });

       // Draw border
       layer.push(RenderCommand::DrawRectStroke { ... });

       // Draw content (text, icons, etc.)
       layer.push(RenderCommand::DrawText { ... });
   }
   ```

2. Register it in `append_all_visual_commands` function.

3. Add test coverage in `render::tests`.

4. Update this matrix.

## Backend Support

GPU visual builders support:
- **GPU Backend**: WebGPU/WGPU for hardware-accelerated rendering
- **CPU Backend**: Software rasterization for fallback
- **PDF Export**: Direct use of render commands
- **Print**: High-resolution output

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-03-04 | Initial coverage matrix, 29/35 controls covered |
