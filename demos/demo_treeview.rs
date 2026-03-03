//! TreeView demo.

use rust_widgets::core::Rect;
use rust_widgets::platform::{get_platform, runtime_gui_mode, RuntimeGuiMode};
use rust_widgets::widget::{SortFilterTreeModel, TreeView, VecTreeModel, Widget, Window};
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
        "[demo_treeview] backend='{}' runtime_mode='{}' native_window_expected={} (model/view path)",
        platform.backend_name(),
        runtime_mode_text,
        native_window_expected
    );

    let mut window = Window::new(
        "TreeView Demo".to_string(),
        Rect { x: 120, y: 120, width: 720, height: 460 },
    );

    // Build a source tree model with hierarchical path strings.
    let source_model = Arc::new(VecTreeModel::new(vec![
        "Root".to_string(),
        "Root/Child-1".to_string(),
        "Root/Child-2".to_string(),
        "Settings".to_string(),
    ]));

    // Add a view model layer with filtering and ordering.
    let mut view_model = SortFilterTreeModel::new(source_model);
    view_model.set_filter_text(Some("Root".to_string()));
    view_model.set_sort_ascending(true);

    // Create the tree view and bind the model projection.
    let mut tree = TreeView::new(Rect { x: 24, y: 24, width: 320, height: 260 });
    tree.set_model(Arc::new(view_model));
    let _ = tree.select_node(0);

    println!("visible nodes: {}", tree.node_count());
    if let Some(index) = tree.selected_node() {
        println!("selected node: {}", tree.node_path(index).unwrap_or_default());
    }
    window.add_child(tree.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
