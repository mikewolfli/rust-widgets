//! Basic Widgets Demo - All basic controls in one dialog with event logging.

use std::cell::RefCell;
// ...existing code...
use std::time::SystemTime;

use rust_widgets::core::ObjectId;
use rust_widgets::i18n::{self, InitOptions};
use rust_widgets::{
    attach_menu_bar_to_window, combo_box_add_item, create_button, create_checkbox,
    create_combo_box, create_label, create_line_edit, create_list_box, create_menu_bar,
    create_panel, create_progress_bar, create_radio_button, create_scroll_area, create_slider,
    create_spin_box, create_status_bar, create_window, init, list_box_add_item, menu_add_item, run,
    set_widget_enabled, show_widget,
};
use rust_widgets::{runtime_gui_mode, RuntimeGuiMode};

thread_local! {
    static LOG_TEXT: RefCell<String> = RefCell::new(String::new());
    static CURRENT_LANG: RefCell<String> = RefCell::new("en".to_string());
}

fn tr(key: &str) -> String {
    i18n::translate(key)
}

fn format_timestamp() -> String {
    if let Ok(now) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        let secs = now.as_secs();
        let hours = (secs % 86400) / 3600;
        let mins = (secs % 3600) / 60;
        let secs = secs % 60;
        let millis = now.subsec_millis();
        format!("[{:02}:{:02}:{:02}.{:03}]", hours, mins, secs, millis)
    } else {
        "[??:??:??.???]".to_string()
    }
}

fn log_event(widget_type: &str, widget_id: ObjectId, event: &str) {
    let entry = format!(
        "{} {}(id={}) {}\n",
        format_timestamp(),
        widget_type,
        widget_id,
        event
    );
    LOG_TEXT.with(|t| {
        let mut text = t.borrow_mut();
        text.push_str(&entry);
        if text.len() > 10000 {
            *text = text[text.len() - 8000..].to_string();
        }
    });
}

// ...existing code...

// ...existing code...

// ...existing code...

fn main() {
    init();

    let i18n_dir = option_env!("CARGO_MANIFEST_DIR")
        .map(|d| format!("{}/demos/assets", d))
        .unwrap_or_else(|| "demos/assets".to_string());

    let lang = std::env::var("LANG").unwrap_or_else(|_| "en".to_string());
    let lang_code = if lang.starts_with("zh_CN") || lang.starts_with("zh-CN") {
        "zh-CN"
    } else if lang.starts_with("zh_TW") || lang.starts_with("zh-TW") {
        "zh-TW"
    } else if lang.starts_with("fr") {
        "fr"
    } else {
        "en"
    };

    let opts = InitOptions {
        language: lang_code.to_string(),
        preload_dir: Some(i18n_dir),
        diagnostics: true,
    };
    let _report = i18n::init_with_options(opts);

    match runtime_gui_mode() {
        RuntimeGuiMode::NativeInteractive => {
            eprintln!("[demo_basic] running in native-interactive mode");
        }
        RuntimeGuiMode::PreviewOrStub => {
            eprintln!("[demo_basic] preview/stub mode");
        }
    }

    let window = create_window(&tr("basic.title"), 100, 100, 1000, 700);

    let menu_bar = create_menu_bar(window, 0, 0, 1000, 24);
    let file_menu = menu_add_item(menu_bar, &tr("menu.file"), None);
    let _exit_item = menu_add_item(file_menu, &tr("menu.file.exit"), Some("Alt+F4"));

    let lang_menu = menu_add_item(menu_bar, &tr("menu.language"), None);
    let _en_item = menu_add_item(lang_menu, &tr("menu.language.english"), None);
    let _zh_cn_item = menu_add_item(lang_menu, &tr("menu.language.chinese_simplified"), None);
    let _zh_tw_item = menu_add_item(lang_menu, &tr("menu.language.chinese_traditional"), None);
    let _fr_item = menu_add_item(lang_menu, &tr("menu.language.french"), None);

    let help_menu = menu_add_item(menu_bar, &tr("menu.help"), None);
    let _about_item = menu_add_item(help_menu, &tr("menu.help.about"), None);

    attach_menu_bar_to_window(window, menu_bar);

    let mut y: i32 = 30;
    let row_height: u32 = 28;
    let col1_x: i32 = 10;
    let col2_x: i32 = 260;
    let col3_x: i32 = 510;
    let label_width: u32 = 80;
    let control_width: u32 = 160;

    let btn_label = create_label(
        window,
        &tr("basic.buttons"),
        col1_x,
        y,
        label_width,
        row_height,
    );
    log_event("Label", btn_label, "Created");

    let btn = create_button(
        window,
        &tr("basic.button"),
        col1_x + label_width as i32,
        y,
        control_width,
        row_height,
    );
    log_event("Button", btn, "Created");

    let checkbox = create_checkbox(
        window,
        &tr("basic.checkbox"),
        col2_x,
        y,
        control_width,
        row_height,
    );
    log_event("CheckBox", checkbox, "Created");

    let radio1 = create_radio_button(
        window,
        &tr("basic.radiobutton"),
        col3_x,
        y,
        control_width,
        row_height,
    );
    log_event("RadioButton", radio1, "Created");

    y += row_height as i32 + 10;

    let text_label = create_label(
        window,
        &tr("basic.text_inputs"),
        col1_x,
        y,
        label_width,
        row_height,
    );
    log_event("Label", text_label, "Created");

    let line_edit = create_line_edit(
        window,
        &tr("basic.lineedit"),
        col1_x + label_width as i32,
        y,
        control_width,
        row_height,
    );
    log_event("LineEdit", line_edit, "Created");

    let label_demo = create_label(
        window,
        &tr("basic.label"),
        col2_x,
        y,
        control_width,
        row_height,
    );
    log_event("Label", label_demo, "Created");

    let radio2 = create_radio_button(
        window,
        &format!("{} 2", tr("basic.radiobutton")),
        col3_x,
        y,
        control_width,
        row_height,
    );
    log_event("RadioButton", radio2, "Created");

    y += row_height as i32 + 10;

    let value_label = create_label(
        window,
        &tr("basic.values"),
        col1_x,
        y,
        label_width,
        row_height,
    );
    log_event("Label", value_label, "Created");

    let slider = create_slider(
        window,
        col1_x + label_width as i32,
        y,
        control_width,
        row_height,
    );
    log_event("Slider", slider, "Created");

    let spinbox = create_spin_box(window, col2_x, y, control_width, row_height);
    log_event("SpinBox", spinbox, "Created");

    let progress = create_progress_bar(window, col3_x, y, control_width, row_height);
    log_event("ProgressBar", progress, "Created");

    y += row_height as i32 + 10;

    let select_label = create_label(
        window,
        &tr("basic.selection"),
        col1_x,
        y,
        label_width,
        row_height,
    );
    log_event("Label", select_label, "Created");

    let combo = create_combo_box(
        window,
        col1_x + label_width as i32,
        y,
        control_width,
        row_height,
    );
    combo_box_add_item(combo, &format!("{} 1", tr("basic.combobox")));
    combo_box_add_item(combo, &format!("{} 2", tr("basic.combobox")));
    combo_box_add_item(combo, &format!("{} 3", tr("basic.combobox")));
    log_event("ComboBox", combo, "Created");

    let listbox = create_list_box(window, col2_x, y, control_width, row_height * 3);
    list_box_add_item(listbox, &format!("{} 1", tr("basic.listbox")));
    list_box_add_item(listbox, &format!("{} 2", tr("basic.listbox")));
    list_box_add_item(listbox, &format!("{} 3", tr("basic.listbox")));
    log_event("ListBox", listbox, "Created");

    let scroll = create_scroll_area(window, col3_x, y, control_width, row_height * 3);
    log_event("ScrollArea", scroll, "Created");

    y += row_height as i32 * 3 + 20;

    let log_label = create_label(
        window,
        &tr("basic.log.title"),
        col1_x,
        y,
        label_width,
        row_height,
    );
    log_event("Label", log_label, "Created");

    let clear_btn = create_button(
        window,
        &tr("basic.log.clear"),
        col1_x + label_width as i32,
        y,
        control_width,
        row_height,
    );
    log_event("Button", clear_btn, "Created");

    y += row_height as i32 + 5;

    let log_panel = create_panel(window, col1_x, y, 980, 280);
    log_event("Panel", log_panel, "Created");

    let log_text = create_line_edit(log_panel, "", 5, 5, 970, 270);
    set_widget_enabled(log_text, false);
    log_event("TextEdit", log_text, "Created (Event Log)");

    let status_bar = create_status_bar(window, &tr("status.ready"), 0, 670, 1000, 24);
    log_event("StatusBar", status_bar, "Created");

    show_widget(window);

    log_event("Window", window, "Shown - Demo Started");

    run();
}
