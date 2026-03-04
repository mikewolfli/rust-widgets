//! TreeView Demo - Tree structure widget demonstration with event logging.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::SystemTime;

use rust_widgets::core::{ObjectId, Rect};
use rust_widgets::i18n::{self, InitOptions};
use rust_widgets::widget::{TreeView, VecTreeModel};
use rust_widgets::{
    attach_menu_bar_to_window, create_button, create_label, create_line_edit, create_menu_bar,
    create_panel, create_status_bar, create_window, init, menu_add_item, run, set_widget_enabled,
    show_widget,
};
use rust_widgets::{runtime_gui_mode, RuntimeGuiMode};

thread_local! {
    static LOG_TEXT: RefCell<String> = RefCell::new(String::new());
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
            eprintln!("[demo_treeview] running in native-interactive mode");
        }
        RuntimeGuiMode::PreviewOrStub => {
            eprintln!("[demo_treeview] preview/stub mode");
        }
    }

    let window = create_window(&tr("treeview.title"), 100, 100, 900, 600);

    let menu_bar = create_menu_bar(window, 0, 0, 900, 24);
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

    let add_btn = create_button(window, &tr("treeview.add_node"), 10, y, 120, btn_height);
    log_event("Button", add_btn, "Created (Add Node)");

    let remove_btn = create_button(window, &tr("treeview.remove_node"), 140, y, 120, btn_height);
    log_event("Button", remove_btn, "Created (Remove Node)");

    let expand_btn = create_button(window, &tr("treeview.expand_all"), 270, y, 120, btn_height);
    log_event("Button", expand_btn, "Created (Expand All)");

    let collapse_btn = create_button(
        window,
        &tr("treeview.collapse_all"),
        400,
        y,
        120,
        btn_height,
    );
    log_event("Button", collapse_btn, "Created (Collapse All)");

    y += btn_height as i32 + 10;

    let tree_label = create_label(window, &tr("treeview.title"), 10, y, 880, btn_height);
    log_event("Label", tree_label, "Created");

    y += btn_height as i32 + 5;

    let tree_panel = create_panel(window, 10, y, 430, 400);
    log_event("Panel", tree_panel, "Created (Tree Area)");

    let mut tree_view = TreeView::new(Rect::new(10, y, 430, 400));

    let paths = vec![
        tr("treeview.root"),
        format!(
            "{}/{}",
            tr("treeview.root"),
            format!("{} 1", tr("treeview.child"))
        ),
        format!(
            "{}/{}",
            tr("treeview.root"),
            format!("{} 2", tr("treeview.child"))
        ),
        format!(
            "{}/{}/{}",
            tr("treeview.root"),
            format!("{} 1", tr("treeview.child")),
            format!("{} 1.1", tr("treeview.node"))
        ),
        format!(
            "{}/{}/{}",
            tr("treeview.root"),
            format!("{} 1", tr("treeview.child")),
            format!("{} 1.2", tr("treeview.node"))
        ),
    ];
    let model = Arc::new(VecTreeModel::new(paths));
    tree_view.set_model(model);
    log_event("TreeView", 0, "Created with sample data");

    let log_label = create_label(window, &tr("basic.log.title"), 450, y, 440, btn_height);
    log_event("Label", log_label, "Created");

    let log_panel = create_panel(window, 450, y + btn_height as i32 + 5, 440, 380);
    log_event("Panel", log_panel, "Created (Log Area)");

    let log_text = create_line_edit(log_panel, "", 5, 5, 430, 370);
    set_widget_enabled(log_text, false);
    log_event("TextEdit", log_text, "Created (Event Log)");

    let status_bar = create_status_bar(window, &tr("status.ready"), 0, 576, 900, 24);
    log_event("StatusBar", status_bar, "Created");

    show_widget(window);

    log_event("Window", window, "Shown - TreeView Demo Started");

    run();
}
