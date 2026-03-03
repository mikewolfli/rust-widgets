use rust_widgets::core::{Color, Rect, Size};
use rust_widgets::render::{
    append_button_visual_commands, append_checkbox_visual_commands,
    append_combo_box_visual_commands, append_label_visual_commands,
    append_line_edit_visual_commands, append_list_box_visual_commands,
    append_menu_bar_visual_commands, append_menu_visual_commands, append_panel_visual_commands,
    append_progress_bar_visual_commands, append_radiobutton_visual_commands,
    append_scroll_bar_visual_commands, append_slider_visual_commands,
    append_stack_widget_visual_commands, append_status_bar_visual_commands,
    append_tab_widget_visual_commands, append_tool_bar_visual_commands,
    append_window_visual_commands, AutoRenderBackend, RenderScene, SceneLayer, SoftwareSurface,
};
use rust_widgets::widget::{
    Button, CheckBox, CheckState, ComboBox, Label, LineEdit, ListBox, Menu, MenuBar, Panel,
    ProgressBar, RadioButton, ScrollBar, Slider, StackWidget, StatusBar, TabWidget, ToolBar,
    Widget, Window,
};

fn main() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);

    let mut window = Window::new("GPU parity demo".to_string(), Rect::new(0, 0, 360, 280));
    window.set_background_color(Some(Color::rgba(20, 24, 30, 255)));

    let mut panel = Panel::new(Rect::new(10, 34, 190, 120));
    panel.set_background_color(Some(Color::rgba(40, 50, 66, 255)));

    let mut button = Button::new("Apply".to_string(), Rect::new(18, 44, 80, 24));
    button.set_pressed(true);

    let mut checkbox = CheckBox::new(Rect::new(18, 76, 24, 24));
    checkbox.set_state(CheckState::Checked);

    let mut radio = RadioButton::new(Rect::new(48, 76, 24, 24));
    radio.set_checked(true);

    let mut label = Label::new("Label".to_string(), Rect::new(18, 108, 80, 18));
    label.set_background_color(Some(Color::rgba(78, 86, 102, 255)));

    let mut line_edit = LineEdit::new(Rect::new(102, 108, 90, 18));
    line_edit.set_text("text".to_string());

    let mut combo = ComboBox::new(Rect::new(214, 34, 130, 22));
    combo.add_item("Alpha");
    combo.add_item("Beta");
    combo.set_current_index(1);
    combo.open_dropdown();

    let mut list = ListBox::new(Rect::new(214, 62, 130, 92));
    list.add_item("One");
    list.add_item("Two");
    list.add_item("Three");

    let mut progress = ProgressBar::new(Rect::new(10, 162, 190, 14));
    progress.set_range(0, 100);
    progress.set_value(55);

    let mut slider = Slider::new(Rect::new(10, 182, 190, 20));
    slider.set_range(0, 100);
    slider.set_value(40);

    let mut scroll = ScrollBar::new(Rect::new(10, 208, 190, 16));
    scroll.set_range(0, 100);
    scroll.set_page_step(24);
    scroll.set_value(36);

    let mut menu_bar = MenuBar::new(Rect::new(0, 0, 360, 24));
    menu_bar.add_menu(1);
    menu_bar.add_menu(2);
    menu_bar.set_current_menu(2);

    let mut menu = Menu::new(Rect::new(214, 160, 130, 64));
    menu.set_title("File".to_string());
    menu.add_action("Open");
    menu.add_action("Save");

    let mut tool_bar = ToolBar::new(Rect::new(0, 228, 360, 24));
    tool_bar.add_action("Cut");
    tool_bar.add_action("Copy");
    tool_bar.add_action("Paste");

    let mut status_bar = StatusBar::new(Rect::new(0, 252, 360, 20));
    status_bar.set_message("Ready".to_string());

    let mut tabs = TabWidget::new(Rect::new(214, 228, 130, 24));
    tabs.add_tab(10);
    tabs.add_tab(11);
    tabs.set_current_index(1);

    let mut stack = StackWidget::new(Rect::new(214, 194, 130, 30));
    stack.set_background_color(Some(Color::rgba(214, 220, 230, 255)));

    append_window_visual_commands(&mut layer, &window);
    append_panel_visual_commands(&mut layer, &panel);
    append_button_visual_commands(&mut layer, &button);
    append_checkbox_visual_commands(&mut layer, &checkbox);
    append_radiobutton_visual_commands(&mut layer, &radio);
    append_label_visual_commands(&mut layer, &label);
    append_line_edit_visual_commands(&mut layer, &line_edit);
    append_combo_box_visual_commands(&mut layer, &combo);
    append_list_box_visual_commands(&mut layer, &list);
    append_progress_bar_visual_commands(&mut layer, &progress);
    append_slider_visual_commands(&mut layer, &slider);
    append_scroll_bar_visual_commands(&mut layer, &scroll);
    append_menu_bar_visual_commands(&mut layer, &menu_bar);
    append_menu_visual_commands(&mut layer, &menu);
    append_tool_bar_visual_commands(&mut layer, &tool_bar);
    append_status_bar_visual_commands(&mut layer, &status_bar);
    append_tab_widget_visual_commands(&mut layer, &tabs);
    append_stack_widget_visual_commands(&mut layer, &stack);

    scene.add_layer(layer);

    let mut surface = SoftwareSurface::new(
        Size {
            width: 360,
            height: 280,
        },
        1.0,
    );

    let selected = scene.compose_to_config_auto(&mut surface, Color::rgba(0, 0, 0, 0), None);
    let checksum: u64 = surface.frame_rgba().iter().map(|value| *value as u64).sum();

    let backend_name = match selected {
        AutoRenderBackend::GpuWgpu => "gpu-wgpu",
        AutoRenderBackend::CpuSoftware => "cpu-software",
    };

    println!(
        "wgpu control parity demo ok: backend={}, bytes={}, checksum={}",
        backend_name,
        surface.frame_rgba().len(),
        checksum
    );
}
