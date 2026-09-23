// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Fills the data-bearing controls with sample data, for the snapshot and census paths.
//!
//! # The gap this closes
//!
//! `WidgetFactory::create(name, geometry, text)` passes **one string**, so every constructor whose doc
//! says "with no rows or columns" produced a control whose own feature was absent from its own snapshot:
//! `list_view.svg` was a filled rectangle, and so were `data_grid.svg`, `grid_table.svg`, `tree_view.svg`,
//! `table_widget.svg` and the rest. A reviewer looking at an empty frame learns nothing about whether the
//! table's columns line up or the tree's indent is right.
//!
//! The charts are **not** in that set: `create_bar_chart`/`create_pie_chart`/`create_line_chart` already
//! seed their own data, which is why their snapshots have 30–2073 shapes. They are the existence proof
//! that seeding is the right fix and the evidence for where it is still missing.
//!
//! # Why this is a separate step and not part of `WidgetFactory::create`
//!
//! `create` is the production construction path: the C ABI, the JSON loader, the designer and every host
//! go through it, and a host that asked for an empty table must get an empty one. Injecting sample rows
//! there would put `Widget / 12 / 12.50 / OK` into a real application's first frame until its own data
//! arrived, and would make "the factory creates what it advertises" untestable because every control would
//! come back pre-populated.
//!
//! So this is applied by the **verification** paths only — `examples/export_control_svgs.rs` and
//! [`crate::widget::census::census_all_controls`] — right after construction and before the first draw.
//! The constructors stay honest and the pictures become informative, which are two requirements that must
//! not be traded against each other.
//!
//! # How the fill reaches a control
//!
//! By **downcast to the concrete type**, through the crate's own
//! [`crate::widget::capability::coercion::widget_as_mut`]. Each control's content API is inherent to it
//! (`Menu::add_action`, `ListBox::add_item`, `CommandPalette::set_entries`) and the crate has never agreed
//! on one property name for "the list of things I hold" — a menu calls them entries, a palette commands, a
//! list items. A property-level fill would have to invent a name and then rewrite every contract to answer
//! it, which is a public-API change made for a snapshot's benefit. The JSON loader reaches these controls
//! by concrete type for the same reason (see its `"combobox"`/`"listbox"` arms), so this follows the
//! established route instead of inventing a second one.
//!
//! The cost is that a newly added data control is not covered until someone writes its arm. That is the
//! honest failure mode — a control whose snapshot stays empty, rather than one silently filled with the
//! wrong shape — and [`apply`] reports how many it filled so the count can be checked rather than assumed.

use crate::compat::Arc;
use crate::widget::capability::coercion::widget_as_mut;
use crate::widget::sample_data as sample;
use crate::widget::Widget;

use crate::compat::{String, Vec};
use crate::widget::advanced_widgets::tab_bar::TabBar;
use crate::widget::container_widgets::groupbox::GroupBox;
use crate::widget::input_widgets::cascader::{Cascader, CascaderOption};
use crate::widget::input_widgets::combobox::ComboBox;
use crate::widget::input_widgets::dropdown::Dropdown;
use crate::widget::input_widgets::editable_combo_box::EditableComboBox;
use crate::widget::input_widgets::font_combo_box::FontComboBox;
use crate::widget::input_widgets::listbox::ListBox;
use crate::widget::input_widgets::multi_select_combo_box::{MultiSelectComboBox, MultiSelectItem};
use crate::widget::menu_toolbar::menu::Menu;
use crate::widget::menu_toolbar::menu_button::{MenuButton, MenuItem};
use crate::widget::special_widgets::command_palette::{CommandEntry, CommandPalette};
use crate::widget::view_widgets::data_grid::DataGrid;
use crate::widget::view_widgets::data_source::IncrementalTableDataSource;
use crate::widget::view_widgets::grid_table::GridTableWidget;
use crate::widget::view_widgets::list_view::{ListView, VecListModel};
use crate::widget::view_widgets::table_widget::{TableModel, TableWidget};
use crate::widget::view_widgets::tree_view::{TreeView, VecTreeModel};
use crate::widget::view_widgets::virtual_table::VirtualTable;

/// Fills `widget` with this crate's sample data, returning `true` when it wrote something.
///
/// `name` is the canonical name the widget was constructed under, taken as an argument rather than read
/// from the widget's kind because 13 kinds are shared by two or more controls: a kind-keyed match would
/// give `split_button` and `tool_button` the same treatment, and `create_table` and `create_table_widget`
/// both build a `TableWidget` and must both be filled.
pub fn apply(name: &str, widget: &mut dyn Widget) -> bool {
    match name {
        // ── A container whose *state* is the feature ──
        //
        // `group_box` ships with `checkable == false`, so its tick — the one part of it a user
        // toggles — was never in any snapshot, and a colour regression in that tick was
        // invisible to both the gallery and its gate. Enabling the state here is what makes
        // the tick part of the checked appearance the exporter renders.
        "group_box" => match widget_as_mut::<GroupBox>(widget) {
            Some(group_box) => {
                group_box.set_checkable(true);
                group_box.set_checked(true);
                true
            }
            None => false,
        },

        // ── Tabular controls that read cells from a source ──
        //
        // `virtual_table` is **not** a `TableWidget`: it is its own type that reads from an
        // `IncrementalTableDataSource` like the two grids. An earlier version of this match listed it with
        // `table`, so the downcast returned `None` and the arm silently did nothing — which the coverage
        // test caught.
        "table" | "table_widget" => match widget_as_mut::<TableWidget>(widget) {
            Some(table) => {
                table.set_model(Arc::new(SampleTableModel));
                true
            }
            None => false,
        },
        "data_grid" => match widget_as_mut::<DataGrid>(widget) {
            Some(grid) => {
                grid.set_data_source(Arc::new(SampleTableSource));
                true
            }
            None => false,
        },
        "grid_table" => match widget_as_mut::<GridTableWidget>(widget) {
            Some(grid) => {
                grid.set_data_source(Arc::new(SampleTableSource));
                true
            }
            None => false,
        },
        "virtual_table" => match widget_as_mut::<VirtualTable>(widget) {
            Some(table) => {
                table.set_data_source(Arc::new(SampleTableSource));
                true
            }
            None => false,
        },

        // ── Single-column lists with their own API ──
        "list_box" => match widget_as_mut::<ListBox>(widget) {
            Some(list) => {
                for item in sample::list_items() {
                    list.add_item(item);
                }
                list.set_current_row(Some(0));
                true
            }
            None => false,
        },
        // ── Combo boxes ──
        //
        // Four *distinct types*, not one type under four names: `EditableComboBox`, `FontComboBox` and
        // `MultiSelectComboBox` are their own structs with their own item shapes. Listing them under one
        // `widget_as_mut::<ComboBox>` arm (as an earlier version did) made three of the four silently do
        // nothing, which the coverage test caught.
        "combo_box" => match widget_as_mut::<ComboBox>(widget) {
            Some(combo) => {
                for item in sample::list_items() {
                    combo.add_item(item);
                }
                combo.set_current_index(Some(0));
                true
            }
            None => false,
        },
        "editable_combo_box" => match widget_as_mut::<EditableComboBox>(widget) {
            Some(combo) => {
                for item in sample::list_items() {
                    combo.add_item(item);
                }
                true
            }
            None => false,
        },
        "font_combo_box" => match widget_as_mut::<FontComboBox>(widget) {
            Some(combo) => {
                // A font combo box holds *font names*, so the sample list here is typeface names rather
                // than the shared business rows — the one place where the sample data is shaped by the
                // control rather than by the table it could have held.
                for name in sample::FONT_NAMES {
                    combo.add_font(name.to_string());
                }
                combo.set_current_index(0);
                true
            }
            None => false,
        },
        "multi_select_combo_box" => match widget_as_mut::<MultiSelectComboBox>(widget) {
            Some(combo) => {
                for (index, item) in sample::list_items().into_iter().enumerate() {
                    combo.add_item(MultiSelectItem::new(index as u64, item));
                }
                true
            }
            None => false,
        },
        "dropdown" => match widget_as_mut::<Dropdown>(widget) {
            Some(dropdown) => {
                dropdown.set_items(sample::list_items());
                dropdown.set_selected_index(0);
                true
            }
            None => false,
        },
        "cascader" => match widget_as_mut::<Cascader>(widget) {
            Some(cascader) => {
                cascader.set_options(sample_cascade());
                // Browse into the **first branch** and open the overlay.
                //
                // # Why both steps are needed, and why this is not just "expand"
                //
                // The tree is drawn only while the overlay is open (`expand`), and the number of columns
                // it draws is `browsed_path.len() + 1` — an open cascader with an **empty** path shows one
                // column, which is indistinguishable from a list box. `expand` alone seeds the browsed path
                // from the *committed selection*, which is empty here, so it drew a single level.
                //
                // Selecting `[0]` is what a user does by clicking the first branch, and it is what makes
                // the control's actual features visible: two level columns side by side and an expanded
                // branch. A snapshot that shows one flat column does not show a cascader.
                cascader.set_selected_path(vec![0]);
                cascader.expand();
                true
            }
            None => false,
        },

        // ── Lists that take a model ──
        //
        // `list_widget` is **not** a registered name: the capability table publishes `list_view` only, and
        // `list_widget` has no entry. Naming it here was dead — the factory can never produce it — which the
        // coverage test caught. The alias list is the registry's business, not this module's.
        "list_view" => match widget_as_mut::<ListView>(widget) {
            Some(list) => {
                list.set_model(Arc::new(VecListModel::new(sample::list_items())));
                list.select_row(0);
                true
            }
            None => false,
        },
        "tree_view" => match widget_as_mut::<TreeView>(widget) {
            Some(tree) => {
                // `node_path` is a *string per visible row*, so the hierarchy is expressed in the text —
                // see `VecTreeModel`. `sample::tree_rows` is the indented form, and the indentation is
                // what makes the snapshot show a tree rather than a flat list under a tree's chrome.
                tree.set_model(Arc::new(VecTreeModel::new(sample::tree_rows())));
                tree.select_node(0);
                true
            }
            None => false,
        },

        // ── Menus, palettes, command lists ──
        //
        // `context_menu` is an **alias** of `menu` in the capability table, not a separate entry, and the
        // factory resolves aliases to the same constructor — so `context_menu` reaches the same `Menu` and
        // is handled by the same arm. It is named here because a caller filtering by the alias would
        // otherwise be able to construct a control that silently is not filled.
        "menu" | "context_menu" => match widget_as_mut::<Menu>(widget) {
            Some(menu) => {
                for item in sample::menu_items() {
                    menu.add_action(item);
                }
                true
            }
            None => false,
        },
        "menu_button" => match widget_as_mut::<MenuButton>(widget) {
            Some(button) => {
                // `MenuItem::new` takes a numeric id as well as its text, so the index is the id here —
                // the control uses it to key the item and nothing in a snapshot depends on the value.
                for (index, item) in sample::menu_items().into_iter().enumerate() {
                    button.add_item(MenuItem::new(index as u64, &item));
                }
                true
            }
            None => false,
        },
        "command_palette" => match widget_as_mut::<CommandPalette>(widget) {
            Some(palette) => {
                palette.set_entries(
                    sample::menu_items()
                        .into_iter()
                        .enumerate()
                        .map(|(index, title)| CommandEntry::new(format!("cmd{index}"), title))
                        .collect(),
                );
                true
            }
            None => false,
        },

        // ── Tabs ──
        //
        // NOT filled. `create_tab_widget` already adds "Tab 1" and "Tab 2" itself, so the control's
        // snapshot was never blank — it was the one data-ish control that already showed its feature. An
        // earlier version of this module appended three more titles, which turned a correct two-tab strip
        // into a five-tab one and made the snapshot disagree with the constructor. Leaving it alone is the
        // fix: the gap being closed is *empty* snapshots, and this control does not have one.
        //
        // `tab_bar`, by contrast, is constructed empty (`create_tab_bar` passes no titles), so it is filled
        // below.
        "tab_bar" => match widget_as_mut::<TabBar>(widget) {
            Some(bar) => {
                for title in sample::tab_titles() {
                    bar.add_tab(title);
                }
                true
            }
            None => false,
        },

        // Everything else has no data concept, or is already seeded by its own constructor (the charts).
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// The sample sources
// ---------------------------------------------------------------------------

/// A two-level cascade, so the control draws its branches *and* a leaf rather than one flat list.
///
/// A single level would not show what a cascader is for: the expand affordance and the indent are the
/// features, and a flat list renders identically to a `list_box`.
fn sample_cascade() -> Vec<CascaderOption> {
    let leaves = |options: &[&str]| -> Vec<CascaderOption> {
        options
            .iter()
            .enumerate()
            .map(|(index, label)| CascaderOption::new(format!("leaf{index}"), (*label).to_string()))
            .collect()
    };
    vec![
        CascaderOption::branch("items", "Items", leaves(&["Widget", "Gadget", "Cog"])),
        CascaderOption::branch("parts", "Parts", leaves(&["Bolt", "Nut", "Washer"])),
    ]
}

/// A `TableModel` over [`crate::widget::sample_data::ROWS`].
///
/// # Why a model rather than rows
///
/// `TableWidget` holds no rows of its own — it reads every cell from its model during `draw` — so handing
/// it one is the only way to give it content. That is also why its snapshot was blank: the JSON loader has
/// no `table_model` concept and so cannot fill it at all.
struct SampleTableModel;

impl TableModel for SampleTableModel {
    fn row_count(&self) -> usize {
        sample::ROWS.len()
    }

    fn column_count(&self) -> usize {
        sample::HEADERS.len()
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        sample::ROWS.get(row).and_then(|cells| cells.get(column)).map(|cell| (*cell).to_string())
    }
}

/// An `IncrementalTableDataSource` over the same rows, for the two controls that read from one.
///
/// Deliberately the same data as [`SampleTableModel`]: `data_grid.svg` and `table_widget.svg` are then two
/// renderings of one table, so a difference between them is a difference in **layout**, which is what a
/// reviewer is looking for. Two datasets would make every comparison a comparison of content as well.
struct SampleTableSource;

impl IncrementalTableDataSource for SampleTableSource {
    fn row_count(&self) -> usize {
        sample::ROWS.len()
    }

    fn column_count(&self) -> usize {
        sample::HEADERS.len()
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        SampleTableModel.data(row, column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::WidgetFactory;

    /// Every name [`apply`] claims to know actually reaches a control that exists.
    ///
    /// # Why this is the load-bearing test
    ///
    /// The fill is by **downcast**: `apply` matches a canonical name, then tries `widget_as_mut` on one
    /// concrete type. If a control is renamed in the registry, or moves to a different type with the same
    /// name, the match arm is still reached and the downcast simply returns `None` — the fill silently
    /// stops working and the snapshot quietly goes back to being an empty frame. That is exactly the defect
    /// this module exists to fix, so it must not be able to return unnoticed.
    ///
    /// Building each named control through the real factory is what closes the loop: the downcast has to
    /// succeed against the type the *registry* actually constructs.
    #[test]
    fn every_filled_name_downcasts_to_the_type_the_registry_builds() {
        let factory = WidgetFactory::new_with_defaults();
        // The names `apply` handles, listed here so a name whose arm stops matching is caught. The list is
        // deliberately literal rather than derived: it is the claim being checked.
        let claimed = [
            "table",
            "table_widget",
            "virtual_table",
            "data_grid",
            "grid_table",
            "list_box",
            "combo_box",
            "editable_combo_box",
            "dropdown",
            "font_combo_box",
            "multi_select_combo_box",
            "cascader",
            "list_view",
            "tree_view",
            "menu",
            "context_menu",
            "menu_button",
            "command_palette",
            "tab_bar",
        ];
        for name in claimed {
            let Some(mut widget) =
                factory.create(name, crate::widget::census::CENSUS_RECT, "Sample")
            else {
                panic!("{name} is claimed as fillable but the factory cannot build it");
            };
            assert!(
                apply(name, widget.as_mut()),
                "{name}: the fill matched nothing, so its snapshot would be an empty frame again"
            );
        }
    }

    /// A name with no data concept is left alone, and the call reports it.
    ///
    /// The negative half matters as much as the positive: a fill that returned `true` for everything would
    /// make the exporter's `sample-filled` count meaningless, and a control whose content was written by the
    /// wrong arm would be counted as a success.
    #[test]
    fn a_control_without_a_data_concept_is_not_filled() {
        let factory = WidgetFactory::new_with_defaults();
        for name in ["button", "label", "slider", "divider", "line_edit"] {
            let Some(mut widget) =
                factory.create(name, crate::widget::census::CENSUS_RECT, "Sample")
            else {
                continue;
            };
            assert!(!apply(name, widget.as_mut()), "{name} has no data to fill");
        }
    }

    /// The table's two routes expose the same data, so `data_grid.svg` and `table_widget.svg` differ only
    /// by layout.
    ///
    /// Two datasets would make every comparison between those snapshots a comparison of content as well,
    /// which is how a layout regression gets read as "the data changed".
    #[test]
    fn both_table_routes_expose_the_same_cells() {
        for row in 0..sample::ROWS.len() {
            for column in 0..sample::HEADERS.len() {
                assert_eq!(
                    SampleTableModel.data(row, column),
                    SampleTableSource.data(row, column),
                    "row {row} column {column} disagrees between the two table routes"
                );
            }
        }
        assert_eq!(SampleTableModel.row_count(), SampleTableSource.row_count());
        assert_eq!(SampleTableModel.column_count(), SampleTableSource.column_count());
        // And an out-of-range cell is absent rather than fabricated.
        assert_eq!(SampleTableModel.data(99, 0), None);
        assert_eq!(SampleTableSource.data(0, 99), None);
    }
}
