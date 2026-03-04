//! MdiArea Demo - MDI (Multiple Document Interface) area widget demonstration with event logging.

use std::cell::RefCell;
use std::time::SystemTime;

use rust_widgets::core::ObjectId;
use rust_widgets::i18n::{self, InitOptions};
use rust_widgets::{runtime_gui_mode, RuntimeGuiMode};
// ...existing code...
use rust_widgets::{
    attach_menu_bar_to_window, create_button, create_line_edit, create_menu_bar, create_panel,
    create_status_bar, create_window, init, menu_add_item, run, show_widget,
};

thread_local! {
    static LOG_TEXT: RefCell<String> = RefCell::new(String::new());
    static DOC_COUNTER: RefCell<usize> = RefCell::new(0);
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

fn main() {
    init();

    let i18n_dir = option_env!("CARGO_MANIFEST_DIR")
        .map(|d| format!("{}/demos/assets", d))
        .unwrap_or_else(|| "demos/assets".to_string());

    let opts = InitOptions {
        language: "en".to_string(),
        preload_dir: Some(i18n_dir),
        diagnostics: true,
    };
    let _report = i18n::init_with_options(opts);

    match runtime_gui_mode() {
        RuntimeGuiMode::NativeInteractive => {
            eprintln!("[demo_mdiarea] running in native-interactive mode");
        }
        RuntimeGuiMode::PreviewOrStub => {
            eprintln!("[demo_mdiarea] preview/stub mode");
        }
    }

    let window = create_window(&tr("mdiarea.title"), 100, 100, 1000, 700);

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
    let btn_height = 28;

    let new_window_btn = create_button(window, &tr("mdiarea.new_window"), 10, y, 150, btn_height);
    log_event("Button", new_window_btn, "Created (New Window)");

    let close_window_btn =
        create_button(window, &tr("mdiarea.close_window"), 170, y, 150, btn_height);
    log_event("Button", close_window_btn, "Created (Close Window)");

    let cascade_btn = create_button(window, &tr("mdiarea.cascade"), 330, y, 120, btn_height);
    log_event("Button", cascade_btn, "Created (Cascade)");

    let tile_btn = create_button(window, &tr("mdiarea.tile"), 460, y, 120, btn_height);
    log_event("Button", tile_btn, "Created (Tile)");

    y += 35;

    let mdi_area_panel = create_panel(window, 10, y, 980, 480);
    log_event("Panel", mdi_area_panel, "Created (MDI Area Panel)");

    y += 490;

    let log_edit = create_line_edit(window, "", 10, y, 980, 100);
    log_event("TextEdit", log_edit, "Created (Event Log)");

    let status_bar = create_status_bar(window, &tr("status.ready"), 0, 676, 1000, 24);
    log_event("StatusBar", status_bar, "Created");

    show_widget(window);

    log_event("Window", window, "Shown - MdiArea Demo Started");

    run();
}
