//! Table widget demo.

use rust_widgets::core::Rect;
use rust_widgets::platform::{get_platform, runtime_gui_mode, RuntimeGuiMode};
use rust_widgets::widget::{
    SortFilterTableModel, SortOrder, TableWidget, VecTableModel, Widget, Window,
};
use rust_widgets::{init, run};
use std::sync::Arc;

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let runtime_mode = runtime_gui_mode();
    let runtime_mode_text = match runtime_mode {
        RuntimeGuiMode::NativeInteractive => "NativeInteractive",
        RuntimeGuiMode::PreviewOrStub => "PreviewOrStub",
    };
    let native_window_expected = false;
    eprintln!(
        "[demo_table] backend='{}' runtime_mode='{}' native_window_expected={} (model/view path)",
        platform.backend_name(),
        runtime_mode_text,
        native_window_expected
    );

    let mut window = Window::new(
        "Table Demo".to_string(),
        Rect {
            x: 120,
            y: 120,
            width: 800,
            height: 480,
        },
    );

    // Build a source table model with static row data.
    let source_model = Arc::new(VecTableModel::new(
        vec!["Name".to_string(), "Role".to_string(), "Office".to_string()],
        vec![
            vec!["Alice".to_string(), "Engineer".to_string(), "NY".to_string()],
            vec!["Bob".to_string(), "Designer".to_string(), "Berlin".to_string()],
            vec!["Charlie".to_string(), "Engineer".to_string(), "Tokyo".to_string()],
            vec!["Dana".to_string(), "PM".to_string(), "NY".to_string()],
        ],
    ));

    // Add a view model layer with filtering and sorting.
    let mut view_model = SortFilterTableModel::new(source_model);
    view_model.set_filter_text(Some("Engineer".to_string()));
    view_model.set_sort(0, SortOrder::Asc);

    // Create the table widget and bind the view model.
    let mut table = TableWidget::new(Rect {
        x: 16,
        y: 16,
        width: 768,
        height: 420,
    });
    table.set_model(Arc::new(view_model));
    let _ = table.select_row(0);

    if let Some(header) = table.header(0) {
        println!("column[0] header: {header}");
    }
    if let Some(selected) = table.selected_row() {
        let name = table.cell(selected, 0).unwrap_or_default();
        let role = table.cell(selected, 1).unwrap_or_default();
        println!("selected row {selected}: {name} ({role})");
    }
    println!(
        "visible rows: {}, columns: {}",
        table.row_count(),
        table.column_count()
    );

    window.add_child(table.id());
    // Show the demo window and enter the event loop.
    window.show();
    run();
}
