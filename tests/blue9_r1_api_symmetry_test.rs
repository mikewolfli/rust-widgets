use rust_widgets::core::Rect;
use rust_widgets::widget::menu_toolbar::menu::MenuItem;
use rust_widgets::{BaseWidget, ListModel, ListView, Menu, MenuBar, RibbonBar, TableModel};
use rust_widgets::{TableWidget, ToolBar, TreeModel, TreeView, Widget};
use std::sync::Arc;

struct TestListModel {
    rows: Vec<String>,
}

impl ListModel for TestListModel {
    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn data(&self, row: usize) -> Option<String> {
        self.rows.get(row).cloned()
    }
}

struct TestTreeModel {
    nodes: Vec<String>,
}

impl TreeModel for TestTreeModel {
    fn node_count(&self) -> usize {
        self.nodes.len()
    }

    fn node_path(&self, index: usize) -> Option<String> {
        self.nodes.get(index).cloned()
    }
}

struct TestTableModel {
    data: Vec<Vec<String>>,
}

impl TableModel for TestTableModel {
    fn row_count(&self) -> usize {
        self.data.len()
    }

    fn column_count(&self) -> usize {
        self.data.first().map(|row| row.len()).unwrap_or(0)
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        self.data.get(row).and_then(|r| r.get(column)).cloned()
    }
}

struct TestDelegate;

impl rust_widgets::widget::view_widgets::table_widget::ItemDelegate for TestDelegate {
    fn create_editor(
        &self,
        _parent: &mut BaseWidget,
        _row: usize,
        _column: usize,
    ) -> Option<Box<dyn Widget>> {
        None
    }

    fn set_editor_data(&self, _editor: &mut dyn Widget, _row: usize, _column: usize) {}

    fn get_editor_data(&self, _editor: &dyn Widget, _row: usize, _column: usize) -> Option<String> {
        None
    }
}

#[test]
fn list_view_model_query_api_is_symmetric() {
    let mut view = ListView::new(Rect::new(0, 0, 100, 80));
    assert!(!view.has_model());
    assert!(view.model_ref().is_none());

    let model: Arc<dyn ListModel> = Arc::new(TestListModel {
        rows: vec!["a".into(), "b".into()],
    });
    view.set_model(model);

    assert!(view.has_model());
    assert!(view.model_ref().is_some());
    assert_eq!(view.row_count(), 2);
}

#[test]
fn tree_view_model_query_api_is_symmetric() {
    let mut view = TreeView::new(Rect::new(0, 0, 100, 80));
    assert!(!view.has_model());
    assert!(view.model_ref().is_none());

    let model: Arc<dyn TreeModel> = Arc::new(TestTreeModel {
        nodes: vec!["root".into(), "child".into()],
    });
    view.set_model(model);

    assert!(view.has_model());
    assert!(view.model_ref().is_some());
    assert_eq!(view.node_count(), 2);
}

#[test]
fn table_widget_query_api_covers_model_delegate_and_size_overrides() {
    let mut table = TableWidget::new(Rect::new(0, 0, 240, 120));

    assert!(!table.has_model());
    assert!(table.model_ref().is_none());
    assert!(!table.has_delegate());
    assert!(table.delegate_ref().is_none());
    assert_eq!(table.column_width(0), None);
    assert_eq!(table.row_height(0), None);

    let model: Arc<dyn TableModel> = Arc::new(TestTableModel {
        data: vec![vec!["r0c0".into(), "r0c1".into()]],
    });
    table.set_model(model);
    assert!(table.has_model());
    assert!(table.model_ref().is_some());

    table.set_column_width(1, 180);
    table.set_row_height(2, 36);
    assert_eq!(table.column_width(1), Some(180));
    assert_eq!(table.row_height(2), Some(36));
    assert_eq!(table.column_width(9), None);
    assert_eq!(table.row_height(9), None);

    let delegate: Arc<dyn rust_widgets::widget::view_widgets::table_widget::ItemDelegate> =
        Arc::new(TestDelegate);
    table.set_delegate(delegate);
    assert!(table.has_delegate());
    assert!(table.delegate_ref().is_some());
}

#[test]
fn menu_item_state_query_handles_normal_and_out_of_range() {
    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    let mut checkable = MenuItem::new("Auto Save");
    checkable.set_checkable(true);
    menu.add_item(checkable);

    assert_eq!(menu.item_enabled(0), Some(true));
    assert_eq!(menu.item_checked(0), Some(false));

    menu.set_item_enabled(0, false);
    menu.set_item_checked(0, true);
    assert_eq!(menu.item_enabled(0), Some(false));
    assert_eq!(menu.item_checked(0), Some(true));

    assert_eq!(menu.item_enabled(999), None);
    assert_eq!(menu.item_checked(999), None);
}

#[test]
fn toolbar_item_state_query_handles_normal_and_out_of_range() {
    let mut bar = ToolBar::new(Rect::new(0, 0, 280, 40));
    let idx = bar.add_action("open", "Open");

    assert_eq!(bar.item_enabled(idx), Some(true));
    assert_eq!(bar.item_checked(idx), Some(false));

    bar.set_item_enabled(idx, false);
    bar.set_item_checked(idx, true);
    assert_eq!(bar.item_enabled(idx), Some(false));
    // Non-checkable items keep default unchecked state.
    assert_eq!(bar.item_checked(idx), Some(false));

    assert_eq!(bar.item_enabled(999), None);
    assert_eq!(bar.item_checked(999), None);
}

#[test]
fn ribbon_item_state_query_handles_normal_and_out_of_range() {
    let mut ribbon = RibbonBar::new(Rect::new(0, 0, 640, 120));
    let tab = ribbon.add_tab("Home");
    let group = ribbon.add_group(tab, "Clipboard");
    let item = ribbon.add_item(tab, group, "Paste");

    assert_eq!(ribbon.item_enabled(tab, group, item), Some(true));
    assert_eq!(ribbon.item_checked(tab, group, item), Some(false));

    ribbon.set_item_enabled(tab, group, item, false);
    ribbon.set_item_checked(tab, group, item, true);
    assert_eq!(ribbon.item_enabled(tab, group, item), Some(false));
    // Default ribbon item is not checkable.
    assert_eq!(ribbon.item_checked(tab, group, item), Some(false));

    assert_eq!(ribbon.item_enabled(99, 99, 99), None);
    assert_eq!(ribbon.item_checked(99, 99, 99), None);
}

#[test]
fn menubar_menu_enabled_query_handles_normal_and_out_of_range() {
    let mut menubar = MenuBar::new(Rect::new(0, 0, 400, 24));
    let file = menubar.add_menu("File");

    assert_eq!(menubar.menu_enabled(file), Some(true));
    menubar.set_menu_enabled(file, false);
    assert_eq!(menubar.menu_enabled(file), Some(false));

    assert_eq!(menubar.menu_enabled(999), None);
}
