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
//! wrong shape — and `apply` reports how many it filled so the count can be checked rather than assumed.

use crate::compat::Arc;
use crate::core::{Color, ObjectId, Point, Rect, Size};
use crate::widget::base_widgets::label::Label;
use crate::widget::capability::coercion::widget_as_mut;
use crate::widget::sample_data as sample;
use crate::widget::Widget;
use std::cell::RefCell;
use std::rc::Rc;

use crate::compat::{String, Vec};
use crate::widget::advanced_widgets::calendar::Calendar;
use crate::widget::advanced_widgets::dial::Dial;
use crate::widget::advanced_widgets::pie_menu::PieMenu;
use crate::widget::advanced_widgets::ribbon_bar::RibbonBar;
use crate::widget::advanced_widgets::tab_bar::TabBar;
use crate::widget::chart_widgets::finance::candlestick_chart::CandlestickChart;
use crate::widget::chart_widgets::finance::depth_chart::DepthChart;
use crate::widget::chart_widgets::finance::indicator_chart::IndicatorChart;
use crate::widget::chart_widgets::finance::order_book::OrderBookWidget;
use crate::widget::chart_widgets::finance::quote_board::QuoteBoard;
use crate::widget::chart_widgets::finance::volume_chart::VolumeChart;
use crate::widget::chart_widgets::finance::{Bar, BookLevel, OrderBook, PriceSeries, Quote};
use crate::widget::chart_widgets::gantt_widget::{GanttTask, GanttWidget};
use crate::widget::chart_widgets::kanban_board::{KanbanBoard, KanbanCard, KanbanColumn};
use crate::widget::chart_widgets::{ChartReferenceLine, LineChart, ReferenceLineSpec};
use crate::widget::companions::{
    CommandPaletteHost, DragGhostLayer, DrawerSectionHeader, DropDownPopupHost, FindReplaceBar,
    HeaderMenu, LayerEntry, LayerPanel, Legend, LegendEntry, ListState, ListStateHost, MenuPanel,
    MenuPanelItem, PaletteEntryRef, PlaylistPanel, PopupItem, SpeedDial, SpeedDialAction,
    SwipeAction, SwipeActions,
};
use crate::widget::container_widgets::carousel::Carousel;
use crate::widget::container_widgets::groupbox::GroupBox;
use crate::widget::container_widgets::mdiarea::MdiArea;
use crate::widget::container_widgets::scrollarea::ScrollArea;
use crate::widget::container_widgets::splitter::Splitter;
use crate::widget::container_widgets::stackedwidget::StackedWidget;
use crate::widget::container_widgets::toolbox::ToolBox;
use crate::widget::cupertino::core::{MaterialNavigationRail, MaterialSnackbar};
use crate::widget::cupertino::segmented_control::CupertinoSegmentedControl;
use crate::widget::dialog::bottom_sheet::BottomSheet;
use crate::widget::dialog::dialog_widget::Dialog;
use crate::widget::dialog::font_dialog::FontDialog;
use crate::widget::dialog::input_dialog::InputDialog;
use crate::widget::dialog::message_box::MessageBox;
use crate::widget::dialog::modal_bottom_sheet::ModalBottomSheet;
use crate::widget::dialog::popover::Popover;
use crate::widget::dialog::progress_dialog::ProgressDialog;
use crate::widget::dialog::wizard::WizardDialog;
use crate::widget::display_widgets::mini_canvas::MiniCanvas;
use crate::widget::display_widgets::slider::{Slider, TickPosition};
use crate::widget::display_widgets::switch::Switch;
use crate::widget::input_widgets::cascader::{Cascader, CascaderOption};
use crate::widget::input_widgets::combobox::ComboBox;
use crate::widget::input_widgets::dropdown::Dropdown;
use crate::widget::input_widgets::editable_combo_box::EditableComboBox;
use crate::widget::input_widgets::font_combo_box::FontComboBox;
use crate::widget::input_widgets::listbox::ListBox;
use crate::widget::input_widgets::multi_select_combo_box::{MultiSelectComboBox, MultiSelectItem};
use crate::widget::input_widgets::shortcut_editor::{ShortcutEditor, ShortcutEntry};
use crate::widget::media_widgets::chapter_list_panel::ChapterListPanel;
use crate::widget::media_widgets::video_player::VideoChapter;
use crate::widget::menu_toolbar::menu::Menu;
use crate::widget::menu_toolbar::menu_bar::MenuBar;
use crate::widget::menu_toolbar::menu_button::{MenuButton, MenuItem};
use crate::widget::menu_toolbar::tool_bar::ToolBar;
use crate::widget::misc_widgets::avatar::{Avatar, GroupAvatar};
use crate::widget::misc_widgets::date_range_picker::{RangePreset, RangePresetBar};
use crate::widget::misc_widgets::segmented_button::{Segment, SegmentedButton};
use crate::widget::nav_widgets::bottom_navigation_bar::BottomNavigationBar;
use crate::widget::nav_widgets::navigation_drawer::NavigationDrawer;
use crate::widget::nav_widgets::tab_view::TabView;
use crate::widget::overlay_widgets::refresh_control::RefreshControl;
use crate::widget::overlay_widgets::swipe_to_dismiss::SwipeToDismiss;
use crate::widget::special_widgets::breadcrumb::{Breadcrumb, BreadcrumbSegment};
use crate::widget::special_widgets::canvas::Canvas;
use crate::widget::special_widgets::chip::{Chip, ChipItem};
use crate::widget::special_widgets::command_palette::{CommandEntry, CommandPalette};
use crate::widget::special_widgets::grid::GridWidget;
use crate::widget::special_widgets::map_attribution::MapAttributionOverlay;
use crate::widget::special_widgets::notification_center::{
    NotificationCenter, NotificationItem, NotificationLevel,
};
use crate::widget::special_widgets::segmented_control::{SegmentItem, SegmentedControl};
use crate::widget::special_widgets::snackbar::Snackbar;
use crate::widget::special_widgets::terminal_find_bar::TerminalFindBar;
use crate::widget::special_widgets::timeline_widget::{TimelineItem, TimelineWidget};
use crate::widget::special_widgets::toast::{ToastItem, ToastLevel, ToastStack};
use crate::widget::view_widgets::data_grid::DataGrid;
use crate::widget::view_widgets::data_source::IncrementalTableDataSource;
use crate::widget::view_widgets::grid_table::GridTableWidget;
use crate::widget::view_widgets::image_gallery::ImageGallery;
use crate::widget::view_widgets::list_view::{ListView, VecListModel};
use crate::widget::view_widgets::properties_panel::{
    PropertiesPanel, PropertyEntry, PropertyValue,
};
use crate::widget::view_widgets::table_widget::{TableModel, TableWidget};
use crate::widget::view_widgets::tree_table::TreeTable;
use crate::widget::view_widgets::tree_view::{TreeView, VecTreeModel};
use crate::widget::view_widgets::virtual_list::VirtualList;
use crate::widget::view_widgets::virtual_table::VirtualTable;
use crate::widget::{Draw, SimpleRegistry};

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
        // `group_box` ships with `checkable == false`, so the tick is a shape no default
        // construction reaches. This arm turns the flag on so the box is drawn at all, and leaves
        // `checked` at the constructor's default.
        //
        // # Why this no longer forces `set_checked(true)`
        //
        // It used to, reasoning that "the tick was never in any snapshot". That captured the wrong
        // artefact: `GroupBox::new` already starts `checked = true`, so forcing it changed nothing,
        // and the pair `group_box.svg` / `group_box_checked.svg` came out byte-identical — two
        // files proving one thing. The checked state is now an *extra appearance* the exporter
        // declares (`EXTRA_APPEARANCES`), and this arm clears the flag so the default file shows
        // the state a caller actually gets while the extra file shows the toggled one. Both states
        // are then reviewable, and neither is implied by the other's absence.
        "group_box" => match widget_as_mut::<GroupBox>(widget) {
            Some(group_box) => {
                group_box.set_checkable(true);
                group_box.set_checked(false);
                true
            }
            None => false,
        },

        // ── Controls whose *state* is the feature ──
        //
        // `CheckBox`/`Switch`/`Slider` each draw something in one state and nothing in the other, and each
        // shipped in the state that draws nothing. The consequence is not cosmetic: `draw` has a branch for
        // the tick, the travel and the tick marks, and **no snapshot, census colour sample or gate ever made
        // it execute**, so a defect inside those branches was invisible to every automated check the crate
        // has. Turning the state on here is what puts the branch into the picture.
        //
        // This is the same reasoning as the `group_box` arm below, applied to the three controls whose
        // "other half" carries the most drawing. A caller still gets the default state from `create`: this
        // module is only reached from the verification paths (see the module docs).
        "check_box" => match widget_as_mut::<CheckBox>(widget) {
            Some(checkbox) => {
                checkbox.set_checked(true);
                true
            }
            None => false,
        },
        "switch" => match widget_as_mut::<Switch>(widget) {
            Some(switch) => {
                switch.set_checked(true);
                true
            }
            None => false,
        },
        // `Slider` needs `tick_interval > 0` as well as a non-`NoTicks` position: `draw` guards the whole
        // tick loop on both, so setting only the position leaves the marks unrendered and the snapshot
        // would claim a feature that is still not in the picture.
        "slider" => match widget_as_mut::<Slider>(widget) {
            Some(slider) => {
                slider.set_value(30);
                slider.set_tick_position(TickPosition::TicksBelow);
                slider.set_tick_interval(25);
                true
            }
            None => false,
        },

        // ── Controls whose *default* is read from the wall clock ──
        //
        // `Calendar::new` seeds `selected_date` and `display_month` from
        // `chrono::Local::now()`, which is right for a control (a calendar should open on
        // today) and wrong for a **snapshot** (a committed file must be reproducible on any
        // machine, on any day). Without this arm `snapshots/svg/calendar.svg` changed every
        // midnight: the selection highlight is a function of the date, so the file committed
        // on one day did not regenerate on the next and `check_svg_snapshots.sh` reported a
        // diff no edit could explain. It also meant the artifact could only ever be made green
        // by re-committing it — the false-green direction, where a gate passes because the
        // fixture was overwritten rather than because nothing changed.
        //
        // Pinning the date here rather than in the control keeps both halves honest: a host
        // still gets today's date from `create`, and the picture names a fixed day. The date
        // chosen is a Wednesday in a 31-day month shown from a Monday first-day, so the grid's
        // leading blank cells and its full last week are both in the picture.
        "calendar" => match widget_as_mut::<Calendar>(widget) {
            Some(calendar) => {
                match chrono::NaiveDate::from_ymd_opt(2026, 9, 16) {
                    Some(sample_day) => {
                        calendar.set_selected_date(sample_day);
                        // `today` is pinned to the same day. It **used** to be read from the wall
                        // clock inside `draw`, so the accent-tinted "today" cell moved at every
                        // midnight and this file's committed bytes changed with no edit to explain
                        // it — the same rotation the `selected_date` pin above exists to stop, left
                        // unfixed for the other clock read. Pinning it makes the picture a function
                        // of this fixture alone.
                        calendar.set_today(sample_day);
                        // The week-number gutter is turned on for the sample: it is the one of the
                        // four visibility flags that is off by default, so leaving it off means the
                        // census never sees it painted at all -- and a feature no snapshot covers
                        // is a feature nothing can regress.
                        calendar.set_vertical_header_visible(true);
                        true
                    }
                    // The literal is a valid Gregorian date, so this arm is unreachable; it
                    // reports the failure instead of silently leaving the sample date-dependent.
                    None => false,
                }
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

        // ── Companions whose content is a caller-owned list ──
        //
        // A legend and a preset bar draw nothing but their own chrome when they have no
        // entries, so the snapshot would be a two-element rectangle no reviewer could judge.
        // The sample rows are the same kind of fixture as the table models above: they exist
        // for the picture, while `create` still returns an empty control.
        "legend" => match widget_as_mut::<Legend>(widget) {
            Some(legend) => {
                legend.set_entries(vec![
                    LegendEntry::new("Revenue", Color::PRIMARY),
                    LegendEntry::new("Cost", Color::rgb(230, 120, 60)),
                    LegendEntry::new("Margin", Color::rgb(80, 170, 120)),
                ]);
                true
            }
            None => false,
        },
        "range_preset_bar" => match widget_as_mut::<RangePresetBar>(widget) {
            Some(bar) => {
                bar.set_presets(vec![
                    RangePreset::new("Today", (2026, 9, 16), (2026, 9, 16)),
                    RangePreset::new("Last 7 days", (2026, 9, 10), (2026, 9, 16)),
                    RangePreset::new("This month", (2026, 9, 1), (2026, 9, 30)),
                ]);
                bar.select_index(1);
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
                // The sample states the hierarchy through `NodeView`s, so the snapshot shows the
                // control doing its job — disclosure glyphs and real indentation — instead of a
                // flat list whose shape was carried by leading spaces in the text.
                tree.set_model(Arc::new(VecTreeModel::from_views(sample::tree_nodes())));
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
        // `create_tab_widget` seeds two tabs itself and this module leaves it alone: the gap
        // being closed here is *empty* snapshots, and that control does not have one.
        //
        // `tab_bar`'s factory constructor also seeds "Tab 1"/"Tab 2", and an earlier
        // version of this arm **appended** three more titles on top of them. The snapshot was
        // therefore a five-tab strip whose first two labels were constructor placeholders —
        // the very "wrong button text" a reviewer notices first. Clearing first makes the
        // sample the whole content, which is what `apply` means for every other control:
        // it *supplies* the state, it does not add to it.
        "tab_bar" => match widget_as_mut::<TabBar>(widget) {
            Some(bar) => {
                bar.clear();
                for title in sample::tab_titles() {
                    bar.add_tab(title);
                }
                true
            }
            None => false,
        },

        // `segmented_control` is a segmented *set*, so an itemless one paints only its empty bar —
        // which is what its snapshot was: a stadium with no divisions and no selection, i.e. a
        // picture that could not show whether the divider, the label inset or the selection
        // indicator were right. Filling it is what makes the control's own feature reviewable.
        "segmented_control" => match widget_as_mut::<SegmentedControl>(widget) {
            Some(control) => {
                control.set_items(vec![
                    SegmentItem::new("overview", "Overview"),
                    SegmentItem::new("details", "Details"),
                    SegmentItem::new("history", "History"),
                ]);
                true
            }
            None => false,
        },

        // ── Controls whose *closed* state is deliberately blank ──
        //
        // `bottom_sheet`, `modal_bottom_sheet` and `dialog` paint nothing at all while closed: the
        // first two show only a scrim, and a dialog shows a scrim, a frame and a title bar, all of
        // which fade and grow in with its reveal — so a closed sample produces a bare page, which
        // the census reports as "this control painted nothing". That is not what a user sees: a
        // bottom sheet exists *in order to be* opened and a dialog *in order to be* shown, and the
        // state that shows the panel, the handle, the frame and the title strip at full strength is
        // the open one. Opening the sample is what puts the control's own feature into its own
        // picture, exactly as filling the table puts its rows there.
        "bottom_sheet" => match widget_as_mut::<BottomSheet>(widget) {
            Some(sheet) => {
                sheet.open();
                // The rise is what the draw reads, so the sample must be *settled* open rather than
                // merely aimed open — a sheet mid-slide would put a half-arrived panel in the
                // snapshot and make every geometry assertion depend on the frame count. The loop is
                // bounded so a control that never settles cannot hang the census: settling takes a
                // handful of frames, and the cap is far above that.
                settle(widget, |w| {
                    let mut settled = false;
                    if let Some(sheet) = widget_as_mut::<BottomSheet>(w) {
                        settled = !sheet.tick(1000);
                    }
                    settled
                });
                true
            }
            None => false,
        },
        "modal_bottom_sheet" => match widget_as_mut::<ModalBottomSheet>(widget) {
            Some(sheet) => {
                sheet.show();
                true
            }
            None => false,
        },
        "image_gallery" => match widget_as_mut::<ImageGallery>(widget) {
            Some(gallery) => {
                // A gallery whose own snapshot says "No images in gallery" teaches a reviewer nothing
                // about the grid, the captions or the selection ring.
                if gallery.images().is_empty() {
                    for index in 1..=4 {
                        gallery.add_image(
                            &format!("sample-{index}.png"),
                            Some(&format!("Image {index}")),
                        );
                    }
                }
                true
            }
            None => false,
        },
        "dialog" => match widget_as_mut::<Dialog>(widget) {
            Some(dialog) => {
                // A dialog with no action row is an empty frame: the row is half of what a dialog
                // *is*, so the sample states the conventional pair and lets the picture show it.
                if dialog.button_texts().is_empty() {
                    dialog.set_title("Dialog");
                    dialog.add_button("Cancel", crate::widget::composite::ButtonRole::Normal);
                    dialog.add_button("OK", crate::widget::composite::ButtonRole::Primary);
                }
                dialog.open();
                // Same reasoning as the sheet above: the reveal is what the draw reads, so the
                // sample is settled fully shown and not merely aimed there, under a frame cap.
                settle(widget, |w| {
                    let mut settled = false;
                    if let Some(dialog) = widget_as_mut::<Dialog>(w) {
                        settled = !dialog.tick(1000);
                    }
                    settled
                });
                true
            }
            None => false,
        },
        "message_box" => match widget_as_mut::<MessageBox>(widget) {
            Some(message_box) => {
                // A message box is *created hidden* and shown by the runtime
                // (`MessageBoxHandle::show_modal` -> `show_widget`), and a hidden box now paints
                // nothing at all. Showing the sample is what puts a prompt in its own picture, the
                // same remedy as `dialog` above -- the state a message box exists to be seen in is
                // the shown one.
                message_box.show();
                settle(widget, |w| {
                    let mut settled = false;
                    if let Some(message_box) = widget_as_mut::<MessageBox>(w) {
                        settled = !message_box.tick(1000);
                    }
                    settled
                });
                true
            }
            None => false,
        },

        // ── Composites whose content is the feature ──
        //
        // Every arm below fills a control that is *correct* when empty and *unreviewable* when
        // empty: a ribbon with no tabs draws one blank strip, a kanban board with no columns
        // draws a blank page, a wizard with no steps draws "No steps configured". The snapshot,
        // the census ink sample and any visual review therefore saw none of the feature the
        // control exists for. The same reasoning as the table arms above, applied to the
        // composites that were still shipping empty pictures.
        "ribbon_bar" => match widget_as_mut::<RibbonBar>(widget) {
            Some(ribbon) => {
                // The constructor seeds no tab, so `clear` is a no-op that keeps the arm
                // idempotent; the ids returned by `add_tab` are the indices used below.
                ribbon.clear();
                let home = ribbon.add_tab("Home");
                let insert = ribbon.add_tab("Insert");
                let clipboard = ribbon.add_group(home, "Clipboard");
                ribbon.add_large_item(home, clipboard, "Paste");
                let font = ribbon.add_group(home, "Font");
                ribbon.add_item(home, font, "Bold");
                ribbon.add_item(home, font, "Italic");
                let shapes = ribbon.add_group(insert, "Shapes");
                ribbon.add_large_item(insert, shapes, "Chart");
                true
            }
            None => false,
        },

        // A pie menu is constructed closed, so its default picture is a dim ring with no
        // wedges. It has to be *settled* open as well as populated: the reveal scales the
        // radii, and a menu at progress 0.5 would make every wedge's extent depend on the
        // frame the snapshot happened to land on.
        "pie_menu" => match widget_as_mut::<PieMenu>(widget) {
            Some(menu) => {
                for label in ["Copy", "Move", "Rename", "Delete"] {
                    menu.add_item(label);
                }
                menu.set_animation_progress(1.0);
                true
            }
            None => false,
        },

        "kanban_board" => match widget_as_mut::<KanbanBoard>(widget) {
            Some(board) => {
                board.add_column(KanbanColumn::new("todo", "To Do"));
                board.add_column(KanbanColumn::new("doing", "In Progress").with_wip_limit(2));
                board.add_column(KanbanColumn::new("done", "Done"));
                board.add_card(0, KanbanCard::new("card1", "Design the layout"));
                board.add_card(
                    0,
                    KanbanCard::new("card2", "Write the spec").with_description("Owner: Ada"),
                );
                board.add_card(1, KanbanCard::new("card3", "Build the board"));
                board.add_card(2, KanbanCard::new("card4", "Set up the repo").with_done(true));
                true
            }
            None => false,
        },

        "timeline_widget" => match widget_as_mut::<TimelineWidget>(widget) {
            Some(timeline) => {
                // The viewport is stated rather than left at its default so the three items
                // are inside it whatever the default range becomes.
                timeline.set_viewport(0, 100);
                timeline.set_items(vec![
                    TimelineItem::new("t1", "Kickoff", 0, 20),
                    TimelineItem::new("t2", "Design", 20, 55),
                    TimelineItem::new("t3", "Build", 55, 95),
                ]);
                timeline.select_index(1);
                true
            }
            None => false,
        },

        "gantt_widget" => match widget_as_mut::<GanttWidget>(widget) {
            Some(gantt) => {
                gantt.set_viewport(0, 100);
                gantt.set_tasks(vec![
                    GanttTask::new("g1", "Research", 0, 25, 100),
                    GanttTask::new("g2", "Design", 25, 50, 60).depends_on(["g1"]),
                    GanttTask::new("g3", "Build", 50, 90, 10).depends_on(["g2"]),
                    GanttTask::milestone("m1", "Ship", 95),
                ]);
                gantt.select_index(1);
                true
            }
            None => false,
        },

        "breadcrumb" => match widget_as_mut::<Breadcrumb>(widget) {
            Some(breadcrumb) => {
                breadcrumb.set_segments(vec![
                    BreadcrumbSegment::new("root", "Home"),
                    BreadcrumbSegment::new("docs", "Documents"),
                    BreadcrumbSegment::new("project", "Project"),
                ]);
                // Select the last segment: a trail whose current page is marked is the state a
                // breadcrumb exists to show, and it puts the selected style into the picture.
                breadcrumb.set_selected_index(2);
                true
            }
            None => false,
        },

        // The factory already calls `show` with the census text, which only *aims* the bar open:
        // the draw reads the reveal, so at progress 0 a freshly shown snackbar paints nothing.
        // Re-showing with an action and settling it puts the whole bar, its action and its
        // message in the snapshot instead of an empty band.
        "snackbar" => match widget_as_mut::<Snackbar>(widget) {
            Some(snackbar) => {
                snackbar.show_with_action("Draft saved", "Undo");
                settle(widget, |w| {
                    let mut settled = false;
                    if let Some(snackbar) = widget_as_mut::<Snackbar>(w) {
                        settled = !snackbar.tick(1000);
                    }
                    settled
                });
                true
            }
            None => false,
        },

        "notification_center" => match widget_as_mut::<NotificationCenter>(widget) {
            Some(center) => {
                center.push(NotificationItem::new(
                    "n1",
                    "Build finished",
                    "All 7463 tests passed.",
                    NotificationLevel::Info,
                ));
                center.push(NotificationItem::new(
                    "n2",
                    "Low disk space",
                    "2.1 GB left on the root volume.",
                    NotificationLevel::Warning,
                ));
                center.push(NotificationItem::new(
                    "n3",
                    "Update available",
                    "Version 2.9.0 is ready to install.",
                    NotificationLevel::Info,
                ));
                settle(widget, |w| {
                    let mut settled = false;
                    if let Some(center) = widget_as_mut::<NotificationCenter>(w) {
                        settled = !center.tick(1000);
                    }
                    settled
                });
                true
            }
            None => false,
        },

        "shortcut_editor" => match widget_as_mut::<ShortcutEditor>(widget) {
            Some(editor) => {
                editor.add_shortcut(
                    ShortcutEntry::new("save", "Save", "File").with_keys(vec!["Ctrl", "S"]),
                );
                editor.add_shortcut(
                    ShortcutEntry::new("open", "Open", "File").with_keys(vec!["Ctrl", "O"]),
                );
                editor.add_shortcut(
                    ShortcutEntry::new("find", "Find", "Edit").with_keys(vec!["Ctrl", "F"]),
                );
                editor.add_shortcut(
                    ShortcutEntry::new("palette", "Command Palette", "View")
                        .with_keys(vec!["Ctrl", "Shift", "P"]),
                );
                true
            }
            None => false,
        },

        "wizard_dialog" => match widget_as_mut::<WizardDialog>(widget) {
            Some(wizard) => {
                wizard.set_title("Setup".to_string());
                wizard.add_step("Welcome", false);
                wizard.add_step("Options", true);
                wizard.add_step("Finish", false);
                true
            }
            None => false,
        },

        "progress_dialog" => match widget_as_mut::<ProgressDialog>(widget) {
            Some(dialog) => {
                dialog.set_title("Installing");
                dialog.set_label_text("Copying files...");
                dialog.set_value(45);
                true
            }
            None => false,
        },

        // A font dialog enumerates the host's installed families in its constructor, so its
        // snapshot used to be a picture of whichever machine generated it: the chrome matched
        // everywhere, the family column did not. A fixed catalogue makes the committed picture
        // reproducible; a real dialog still starts from `chooser_families`.
        "font_dialog" => match widget_as_mut::<FontDialog>(widget) {
            Some(dialog) => {
                dialog.set_font_families(
                    sample::FONT_NAMES.iter().map(|name| name.to_string()).collect(),
                );
                true
            }
            None => false,
        },

        // An empty input dialog is two blank bands: the label row and the field row have no
        // text to show, so the control's layout (label above field, field above buttons) is
        // unreviewable. The fill supplies one of each.
        "input_dialog" => match widget_as_mut::<InputDialog>(widget) {
            Some(dialog) => {
                dialog.set_title("Rename");
                dialog.set_label_text("Name");
                dialog.set_text_value("Document");
                true
            }
            None => false,
        },

        // A dial without notches is a disc and a needle: the notch ring, which is the control's
        // whole scale, is only drawn when the caller asks for it. Value 65 puts the needle away
        // from the minimum, where "needle at the left stop" and "no value at all" look alike.
        "dial" => match widget_as_mut::<Dial>(widget) {
            Some(dial) => {
                dial.set_notches_visible(true);
                // The default target (3.7 px) yields ~70 notches on a 60 px dial — a starburst
                // rather than a readable scale. 10 px is roughly one notch per 10 degrees.
                dial.set_notch_target(10.0);
                dial.set_value(65);
                true
            }
            None => false,
        },

        // The factory builds a one-segment button from the census text, which is a control with
        // no division to review. Three segments and a selection are the state the compound shape
        // exists for; `segments_mut().clear()` first so the census text is *replaced*, not kept as
        // a fourth segment (the same mistake `tab_bar` made).
        "segmented_button" => match widget_as_mut::<SegmentedButton>(widget) {
            Some(button) => {
                button.segments_mut().clear();
                button.add_segment(Segment::new("Day"));
                button.add_segment(Segment::new("Week"));
                button.add_segment(Segment::new("Month"));
                button.set_selected_index(Some(1));
                true
            }
            None => false,
        },

        "chip" => match widget_as_mut::<Chip>(widget) {
            Some(chip) => {
                let mut items = vec![
                    ChipItem::new("bug", "Bug"),
                    ChipItem::new("feature", "Feature"),
                    ChipItem::new("docs", "Docs"),
                ];
                if let Some(first) = items.first_mut() {
                    first.selected = true;
                }
                chip.set_items(items);
                true
            }
            None => false,
        },

        // Navigation bars are item lists with no default item, so their default picture is an
        // empty surface. The items are named icons (`home`, `search`, …) rather than letters:
        // `draw_icon_named` resolves them to the bundled outlines, so the snapshot shows the
        // same affordances a host gets when it passes those names.
        "bottom_navigation_bar" => match widget_as_mut::<BottomNavigationBar>(widget) {
            Some(bar) => {
                bar.add_item("home", "Home");
                bar.add_item("search", "Search");
                bar.add_item("bell", "Alerts");
                bar.add_item("user", "Profile");
                bar.set_selected_index(0);
                true
            }
            None => false,
        },

        "material_navigation_rail" => match widget_as_mut::<MaterialNavigationRail>(widget) {
            Some(rail) => {
                rail.add_item("home", "Home");
                rail.add_item("search", "Search");
                rail.add_item("settings", "Settings");
                rail.set_selected(1);
                true
            }
            None => false,
        },

        // A refresh wrapper at rest paints its content and nothing else, and the sample has no
        // content to mount. Settling it into the refreshing state puts the one piece of chrome
        // the control owns — the indicator strip — into its own picture.
        "refresh_control" => match widget_as_mut::<RefreshControl>(widget) {
            Some(refresh) => {
                refresh.start_pull();
                refresh.update_pull(refresh.threshold() + 16.0);
                refresh.end_pull();
                settle(widget, |w| {
                    let mut settled = false;
                    if let Some(refresh) = widget_as_mut::<RefreshControl>(w) {
                        settled = !refresh.tick(1000);
                    }
                    settled
                });
                true
            }
            None => false,
        },

        // ── Financial charts ──
        //
        // These six controls have no default data and draw a literal "No data" plate at rest,
        // which is the correct *empty* state and an uninformative *snapshot* one: axes, the
        // candle/liquidity geometry, the depth curve and the quote columns were all absent from
        // the census. The data is deterministic so the snapshots do not rotate.
        "candlestick_chart" => match widget_as_mut::<CandlestickChart>(widget) {
            Some(chart) => {
                chart.set_series(sample_price_series());
                true
            }
            None => false,
        },
        "volume_chart" => match widget_as_mut::<VolumeChart>(widget) {
            Some(chart) => {
                chart.set_series(sample_price_series());
                true
            }
            None => false,
        },
        "indicator_chart" => match widget_as_mut::<IndicatorChart>(widget) {
            Some(chart) => {
                chart.set_series(sample_price_series());
                true
            }
            None => false,
        },
        "depth_chart" => match widget_as_mut::<DepthChart>(widget) {
            Some(chart) => {
                chart.set_book(sample_order_book());
                true
            }
            None => false,
        },
        "order_book" => match widget_as_mut::<OrderBookWidget>(widget) {
            Some(book) => {
                book.set_book(sample_order_book());
                true
            }
            None => false,
        },
        "quote_board" => match widget_as_mut::<QuoteBoard>(widget) {
            Some(board) => {
                board.set_quotes(sample_quotes());
                true
            }
            None => false,
        },

        // Everything else has no data concept, or is already seeded by its own constructor (the charts).
        // ── View controls with a lightweight data entry (VIEW-N05 unblocked these) ──
        //
        // `data_view` is an alias of `virtual_list` in the capability table, so one arm
        // serves the canonical name and its alias. Without it both snapshots were a bare frame.
        "virtual_list" | "data_view" => match widget_as_mut::<VirtualList>(widget) {
            Some(list) => {
                list.set_rows(sample_list_rows());
                list.select_row(0);
                true
            }
            None => false,
        },
        "tree_table" => match widget_as_mut::<TreeTable>(widget) {
            Some(table) => {
                table.set_rows(sample_list_rows());
                true
            }
            None => false,
        },

        // ── Panels and strips whose content is a list of labels ──
        "properties_panel" => match widget_as_mut::<PropertiesPanel>(widget) {
            Some(panel) => {
                panel.add_property(PropertyEntry::new(
                    "Width",
                    PropertyValue::Number(240.0),
                    Some("Layout"),
                    None,
                    true,
                ));
                panel.add_property(PropertyEntry::new(
                    "Height",
                    PropertyValue::Number(120.0),
                    Some("Layout"),
                    None,
                    true,
                ));
                panel.add_property(PropertyEntry::new(
                    "Visible",
                    PropertyValue::Bool(true),
                    Some("State"),
                    None,
                    true,
                ));
                panel.add_property(PropertyEntry::new(
                    "Title",
                    PropertyValue::Text("Hello".to_string()),
                    Some("State"),
                    None,
                    false,
                ));
                true
            }
            None => false,
        },
        "tab_view" => match widget_as_mut::<TabView>(widget) {
            Some(tabs) => {
                tabs.add_tab(
                    "General",
                    Some(Box::new(Label::new(
                        "General page".to_string(),
                        Rect::new(0, 0, 200, 64),
                    ))),
                    None::<String>,
                );
                tabs.add_tab(
                    "Advanced",
                    Some(Box::new(Label::new(
                        "Advanced options".to_string(),
                        Rect::new(0, 0, 200, 64),
                    ))),
                    None::<String>,
                );
                tabs.add_tab("About", None, None::<String>);
                tabs.set_current_index(0);
                true
            }
            None => false,
        },
        "menu_bar" => match widget_as_mut::<MenuBar>(widget) {
            Some(bar) => {
                bar.add_menu("File");
                bar.add_menu("Edit");
                bar.add_menu("View");
                true
            }
            None => false,
        },
        "tool_bar" => match widget_as_mut::<ToolBar>(widget) {
            Some(bar) => {
                bar.add_action("open", "Open");
                bar.add_action("save", "Save");
                bar.add_separator();
                bar.add_action("run", "Run");
                true
            }
            None => false,
        },
        "tool_box" => match widget_as_mut::<ToolBox>(widget) {
            Some(toolbox) => {
                toolbox.add_item("General".to_string(), None);
                toolbox.add_item("Editor".to_string(), None);
                toolbox.add_item("Files".to_string(), None);
                toolbox.set_current_index(0);
                true
            }
            None => false,
        },
        "carousel" => match widget_as_mut::<Carousel>(widget) {
            Some(carousel) => {
                carousel.add_page("Welcome", Color::rgb(33, 150, 243));
                carousel.add_page("Features", Color::rgb(156, 39, 176));
                carousel.add_page("Pricing", Color::rgb(0, 150, 136));
                carousel.set_current(0);
                true
            }
            None => false,
        },
        "toast_stack" => match widget_as_mut::<ToastStack>(widget) {
            Some(stack) => {
                stack.push(ToastItem::new("1", "Saved successfully", ToastLevel::Success, 3000));
                stack.push(ToastItem::new("2", "Check your connection", ToastLevel::Warning, 3000));
                stack.select_index(0);
                true
            }
            None => false,
        },
        "grid" => match widget_as_mut::<GridWidget>(widget) {
            Some(grid) => {
                grid.set_rows(4);
                grid.set_columns(6);
                grid.set_spacing(2);
                grid.set_line_color(Some(Some(Color::rgb(120, 120, 120))));
                true
            }
            None => false,
        },

        // ── Immediate-mode surfaces: a shape is the content ──
        "canvas" => match widget_as_mut::<Canvas>(widget) {
            Some(canvas) => {
                canvas.fill_rect(Rect::new(12, 16, 80, 40), Color::rgb(33, 150, 243));
                canvas.draw_line(
                    Point::new(12, 72),
                    Point::new(228, 72),
                    Color::rgb(200, 200, 200),
                );
                canvas.fill_circle(Point::new(160, 60), 18, Color::rgb(255, 152, 0));
                true
            }
            None => false,
        },
        "mini_canvas" => match widget_as_mut::<MiniCanvas>(widget) {
            Some(canvas) => {
                canvas.fill_rect(Rect::new(12, 16, 70, 32), Color::rgb(33, 150, 243));
                canvas.draw_line(
                    Point::new(12, 70),
                    Point::new(220, 70),
                    Color::rgb(200, 200, 200),
                );
                canvas.fill_circle(Point::new(170, 56), 14, Color::rgb(255, 152, 0));
                true
            }
            None => false,
        },

        // ── Overlays that must be shown to be reviewable ──
        "popover" => match widget_as_mut::<Popover>(widget) {
            Some(popover) => {
                popover.set_content(Box::new(Label::new(
                    "Popover content".to_string(),
                    Rect::new(0, 0, 180, 60),
                )));
                popover.show(Rect::new(20, 20, 80, 30));
                true
            }
            None => false,
        },
        "material_snackbar" => match widget_as_mut::<MaterialSnackbar>(widget) {
            Some(snackbar) => {
                snackbar.set_message("Message sent");
                snackbar.set_action_text("Undo");
                snackbar.show();
                true
            }
            None => false,
        },
        "cupertino_segmented_control" => match widget_as_mut::<CupertinoSegmentedControl>(widget) {
            Some(control) => {
                control.set_segments(vec![
                    "Day".to_string(),
                    "Week".to_string(),
                    "Month".to_string(),
                ]);
                control.set_selected_index(1);
                true
            }
            None => false,
        },
        "navigation_drawer" => match widget_as_mut::<NavigationDrawer>(widget) {
            Some(drawer) => {
                drawer.add_item("H", "Home");
                drawer.add_item("S", "Search");
                drawer.add_item("G", "Settings");
                drawer.set_selected_index(0);
                drawer.open();
                true
            }
            None => false,
        },
        // ── Hosts whose content is mounted, not owned ──
        //
        // Each of these paints only once a host has opened it and given it rows, exactly the way
        // a dialog paints only once something opens it. The fill is what puts them in that state
        // for the snapshot: without it the picture is an empty frame and every branch inside
        // `draw` (pop-up scrim, palette rows, ghost proxy) belongs to no artefact at all.
        "find_replace_bar" => match widget_as_mut::<FindReplaceBar>(widget) {
            Some(bar) => {
                bar.set_query("widget");
                bar.set_replacement("control");
                bar.open();
                true
            }
            None => false,
        },
        "drop_down_popup_host" => match widget_as_mut::<DropDownPopupHost>(widget) {
            Some(host) => {
                host.set_items(vec![
                    PopupItem::new("copy", "Copy"),
                    PopupItem::new("paste", "Paste"),
                    PopupItem::new("delete", "Delete").with_enabled(false),
                ]);
                let anchor = host.geometry();
                host.open_at(anchor);
                true
            }
            None => false,
        },
        "command_palette_host" => match widget_as_mut::<CommandPaletteHost>(widget) {
            Some(host) => {
                host.set_entries(vec![
                    PaletteEntryRef::new("format", "Format Document"),
                    PaletteEntryRef::new("save", "Save All").with_hint("Ctrl+K S"),
                ]);
                host.open();
                true
            }
            None => false,
        },
        "header_menu" => match widget_as_mut::<HeaderMenu>(widget) {
            Some(menu) => {
                let anchor = menu.geometry();
                menu.open_for("name", anchor);
                true
            }
            None => false,
        },
        "list_state_host" => match widget_as_mut::<ListStateHost>(widget) {
            Some(host) => {
                host.set_empty_content(None, "No results", "Try a different filter");
                host.set_state(ListState::Empty);
                true
            }
            None => false,
        },
        "drag_ghost_layer" => match widget_as_mut::<DragGhostLayer>(widget) {
            Some(layer) => {
                let origin = layer.geometry();
                layer.begin(origin, "Row 3");
                true
            }
            None => false,
        },
        "group_avatar" => match widget_as_mut::<GroupAvatar>(widget) {
            Some(group) => {
                let mut first = Avatar::new(Rect::new(0, 0, 40, 40));
                first.set_text("AL");
                first.set_bg_color(Color::rgb(63, 81, 181));
                let mut second = Avatar::new(Rect::new(0, 0, 40, 40));
                second.set_text("MK");
                second.set_bg_color(Color::rgb(0, 150, 136));
                let mut third = Avatar::new(Rect::new(0, 0, 40, 40));
                third.set_text("RJ");
                third.set_bg_color(Color::rgb(255, 152, 0));
                group.set_avatars(vec![first, second, third]);
                // Two visible of three, so the overflow badge is in the picture as well.
                group.set_max_visible(2);
                true
            }
            None => false,
        },
        // A reference line needs both an annotation and a value range; the range comes from the
        // chart it annotates, so the fill attaches a seeded chart rather than only setting lines.
        "chart_reference_line" => match widget_as_mut::<ChartReferenceLine>(widget) {
            Some(line) => {
                let mut chart = LineChart::new(Rect::new(0, 0, 200, 120));
                chart.set_data(vec![(0.0, 10.0), (1.0, 40.0), (2.0, 25.0), (3.0, 60.0)]);
                line.add_line(ReferenceLineSpec::new(30.0).with_label("Target"));
                line.attach_line_chart(&chart);
                true
            }
            None => false,
        },
        "terminal_find_bar" => match widget_as_mut::<TerminalFindBar>(widget) {
            Some(bar) => {
                bar.set_query("widget");
                bar.set_match_count(3, 1);
                bar.open();
                true
            }
            None => false,
        },
        "chapter_list_panel" => match widget_as_mut::<ChapterListPanel>(widget) {
            Some(panel) => {
                panel.set_chapters(vec![
                    VideoChapter::new("Intro", 0.0),
                    VideoChapter::new("Main topic", 90.0),
                    VideoChapter::new("Wrap-up", 240.0),
                ]);
                panel.set_position_ms(95_000);
                true
            }
            None => false,
        },
        "map_attribution" => match widget_as_mut::<MapAttributionOverlay>(widget) {
            Some(overlay) => {
                overlay.add_entry("Map data (c) OpenStreetMap contributors", None::<&str>);
                overlay.add_entry("Tiles (c) CARTO", None::<&str>);
                true
            }
            None => false,
        },
        "menu_panel" => match widget_as_mut::<MenuPanel>(widget) {
            Some(panel) => {
                panel.set_items(vec![
                    MenuPanelItem::new("Cut"),
                    MenuPanelItem::new("Copy").with_icon("copy"),
                    MenuPanelItem::new("Paste").with_enabled(false),
                ]);
                panel.open();
                true
            }
            None => false,
        },
        "speed_dial" => match widget_as_mut::<SpeedDial>(widget) {
            Some(dial) => {
                dial.set_actions(vec![
                    SpeedDialAction::new("Share", "+"),
                    SpeedDialAction::new("Edit", "E"),
                ]);
                dial.set_expanded(true);
                true
            }
            None => false,
        },
        "swipe_actions" => match widget_as_mut::<SwipeActions>(widget) {
            Some(swipe) => {
                swipe.set_leading_actions(vec![SwipeAction::new("Archive")]);
                swipe.set_trailing_actions(vec![SwipeAction::new("Delete")]);
                swipe.set_offset(48.0);
                true
            }
            None => false,
        },
        "drawer_section_header" => match widget_as_mut::<DrawerSectionHeader>(widget) {
            Some(header) => {
                header.set_label("Library");
                true
            }
            None => false,
        },
        "layer_panel" => match widget_as_mut::<LayerPanel>(widget) {
            Some(panel) => {
                panel.set_entries(vec![
                    LayerEntry::new(1, "background"),
                    LayerEntry::new(2, "ink"),
                    LayerEntry::new(3, "notes").with_visible(false),
                ]);
                true
            }
            None => false,
        },
        "playlist_panel" => match widget_as_mut::<PlaylistPanel>(widget) {
            Some(panel) => {
                panel.set_tracks(vec![
                    "/music/one.mp3".to_string(),
                    "/music/two.mp3".to_string(),
                    "/music/three.mp3".to_string(),
                ]);
                panel.set_current_index(Some(0));
                true
            }
            None => false,
        },
        "swipe_to_dismiss" => match widget_as_mut::<SwipeToDismiss>(widget) {
            Some(swipe) => {
                swipe.set_child(Box::new(Label::new(
                    "Swipe me".to_string(),
                    Rect::new(0, 0, 200, 44),
                )));
                swipe.set_action_text("Delete");
                swipe.set_dismiss_threshold(0.4);
                true
            }
            None => false,
        },

        // ── Containers: a registered child is the content ──
        //
        // These four draw through `SimpleRegistry`, so a child is composed here exactly
        // the way a host composes one (register + attach). The label owns its geometry;
        // the registry carries no geometry callback because a snapshot never resizes.
        "mdi_area" => match widget_as_mut::<MdiArea>(widget) {
            Some(area) => {
                let registry = sample_registry();
                register_sample_label(&registry, 1, Rect::new(16, 16, 160, 56), "Document 1");
                register_sample_label(&registry, 2, Rect::new(48, 48, 160, 56), "Document 2");
                area.set_registry(registry);
                area.add_sub_window(1, Rect::new(16, 16, 160, 56));
                area.add_sub_window(2, Rect::new(48, 48, 160, 56));
                true
            }
            None => false,
        },
        "splitter" => match widget_as_mut::<Splitter>(widget) {
            Some(splitter) => {
                let registry = sample_registry();
                register_sample_label(&registry, 1, Rect::new(0, 0, 100, 80), "Left pane");
                register_sample_label(&registry, 2, Rect::new(100, 0, 140, 80), "Right pane");
                splitter.set_registry(registry);
                splitter.add_pane(1, 1);
                splitter.add_pane(2, 1);
                true
            }
            None => false,
        },
        "scroll_area" => match widget_as_mut::<ScrollArea>(widget) {
            Some(area) => {
                let registry = sample_registry();
                register_sample_label(&registry, 1, Rect::new(0, 0, 160, 24), "Scrollable content");
                area.set_registry(registry);
                area.set_content_size(Size::new(160, 240));
                true
            }
            None => false,
        },
        "stacked_widget" => match widget_as_mut::<StackedWidget>(widget) {
            Some(stack) => {
                let registry = sample_registry();
                register_sample_label(&registry, 1, Rect::new(0, 0, 200, 80), "Page one");
                register_sample_label(&registry, 2, Rect::new(0, 0, 200, 80), "Page two");
                stack.set_registry(registry);
                stack.add_widget(1);
                stack.add_widget(2);
                stack.set_current_index(0);
                true
            }
            None => false,
        },

        _ => false,
    }
}

/// Tick a control until it reports it has settled, under a hard frame cap.
///
/// # Why the loop is bounded
///
/// The reveal animations of `bottom_sheet`, `dialog` and `message_box` are what the sample snapshot
/// reads, so the sample must be *settled* rather than merely aimed open. That used to be a bare
/// `while widget.tick(1000) {}`, which has no exit condition of its own: a control that never reports
/// "settled" (a bug in an animation, or a future control whose `tick` never returns `false`) would
/// hang the census with no bound — exactly the unbounded loop the project's rules forbid. The cap is
/// far above any real settle time (a few frames), so it changes nothing for a working control and
/// merely stops a broken one from wedging the process.
fn settle(widget: &mut dyn Widget, mut step: impl FnMut(&mut dyn Widget) -> bool) {
    const MAX_SETTLE_FRAMES: usize = 600;
    for _ in 0..MAX_SETTLE_FRAMES {
        if step(widget) {
            return;
        }
    }
    log::warn!(
        "[sample_fill] control did not settle within {MAX_SETTLE_FRAMES} frames; snapshotting anyway"
    );
}

// ---------------------------------------------------------------------------
// The sample sources
// ---------------------------------------------------------------------------

/// The rows the list-shaped controls share, so their snapshots differ by layout.
fn sample_list_rows() -> Vec<String> {
    ["Alpha", "Bravo", "Charlie", "Delta", "Echo"]
        .iter()
        .map(|label| (*label).to_string())
        .collect()
}

/// A registry for a composed container snapshot.
fn sample_registry() -> Rc<RefCell<SimpleRegistry>> {
    Rc::new(RefCell::new(SimpleRegistry::new()))
}

/// Registers a label as a container child.
///
/// The draw closure owns the label and draws it at the rect it was constructed with.
/// A snapshot never resizes its children, so no geometry callback is registered.
fn register_sample_label(
    registry: &Rc<RefCell<SimpleRegistry>>,
    id: ObjectId,
    rect: Rect,
    text: &str,
) {
    let mut label = Label::new(text.to_string(), rect);
    registry.borrow_mut().register(id, move |context| label.draw(context), |_| {});
}

// `CheckBox` is imported here rather than with the other control types because it belongs to the
// `base_widgets` family and the arms above are its only use in this module.
use crate::widget::base_widgets::checkbox::CheckBox;

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

/// A deterministic 24-bar price series with hour labels.
///
/// One series serves the candlestick, volume and indicator charts, so the three snapshots are
/// three views of the same data — a difference between them is a layout difference, which is
/// what a reviewer is looking for. Deterministic (no clock, no RNG) because these files are
/// committed artifacts.
fn sample_price_series() -> PriceSeries {
    let mut series = PriceSeries::new();
    let mut price: f64 = 100.0;
    for index in 0..24 {
        // A mild saw pattern: rising over each six-bar window, with a pull-back in the middle.
        let drift = match index % 6 {
            0 => 1.8,
            1 => 0.9,
            2 => -0.7,
            3 => 1.4,
            4 => -1.1,
            _ => 0.6,
        };
        let open = price;
        let close = open + drift;
        let high = open.max(close) + 0.6;
        let low = open.min(close) - 0.5;
        let volume = 1200.0 + index as f64 * 137.0;
        series
            .push_with_label(Bar::new(open, high, low, close, volume), format!("{:02}:00", index));
        price = close;
    }
    series
}

/// A five-level book on each side, with size growing away from the touch.
fn sample_order_book() -> OrderBook {
    let mut book = OrderBook::new();
    book.set_bids(
        (0..5)
            .map(|level| BookLevel::new(99.9 - level as f64 * 0.1, 12.0 + level as f64 * 7.0))
            .collect(),
    );
    book.set_asks(
        (0..5)
            .map(|level| BookLevel::new(100.0 + level as f64 * 0.1, 10.0 + level as f64 * 6.0))
            .collect(),
    );
    book
}

/// Five quotes with both a riser and a faller, so the board's up/down colours are both visible.
fn sample_quotes() -> Vec<Quote> {
    let rows: [(&str, f64, f64, f64); 5] = [
        ("AAPL", 189.5, 187.2, 54_120_000.0),
        ("MSFT", 412.8, 416.1, 21_400_000.0),
        ("NVDA", 128.4, 121.9, 310_800_000.0),
        ("TSLA", 246.2, 249.7, 98_600_000.0),
        ("AMD", 162.7, 159.4, 44_900_000.0),
    ];
    rows.iter()
        .map(|(symbol, last, previous_close, volume)| {
            let mut quote = Quote::new(symbol, *last, *previous_close);
            quote.high = (*last).max(*previous_close) + 1.2;
            quote.low = (*last).min(*previous_close) - 1.2;
            quote.volume = *volume;
            quote
        })
        .collect()
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

    /// The sample rows are `&'static str`, so the paint path can copy them into its own buffer.
    fn fill_data(&self, row: usize, column: usize, out: &mut String) -> bool {
        match sample::ROWS.get(row).and_then(|cells| cells.get(column)) {
            Some(cell) => {
                out.push_str(cell);
                true
            }
            None => false,
        }
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

    fn fill_cell(&self, row: usize, column: usize, out: &mut String) -> bool {
        SampleTableModel.fill_data(row, column, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::WidgetFactory;

    /// The names [`apply`] actually handles, derived from its own source.
    ///
    /// # Why this is parsed rather than listed
    ///
    /// The previous literal list had drifted exactly once, and the drift was invisible:
    /// four arms (`bottom_sheet`, `modal_bottom_sheet`, `dialog`, `message_box`) were
    /// added without a name in the list, so nothing asserted their downcast still matched
    /// (SNAP-N02). Reading the arm patterns means "which names are handled" and "which
    /// names are tested" are the same fact, so a new arm cannot be untested by omission.
    ///
    /// The scan is bounded to the body of `apply`: it starts at its signature and stops at
    /// the first column-zero `}`, so the helper functions and the test module below are not
    /// parsed. Arm patterns are the only lines before an `=>` that contain quoted names.
    fn declared_arm_names() -> Vec<&'static str> {
        const SOURCE: &str = include_str!("sample_fill.rs");
        let mut names = Vec::new();
        let mut in_apply = false;
        for line in SOURCE.lines() {
            if line.starts_with("pub fn apply(name: &str") {
                in_apply = true;
                continue;
            }
            if !in_apply {
                continue;
            }
            if line == "}" {
                break;
            }
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            let Some((pattern, _)) = trimmed.split_once("=>") else { continue };
            let mut rest = pattern;
            while let Some(start) = rest.find('"') {
                let after = &rest[start + 1..];
                let Some(end) = after.find('"') else { break };
                names.push(&after[..end]);
                rest = &after[end + 1..];
            }
        }
        names.sort_unstable();
        names.dedup();
        names
    }

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
        let claimed = declared_arm_names();
        assert!(
            claimed.len() >= 60,
            "the arm scan found {} names, which is too few to be the real table",
            claimed.len()
        );
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
    /// A control with no *data* concept is not filled by [`apply`].
    ///
    /// # Why `slider` is no longer in this list
    ///
    /// It was, and correctly: `slider` has no data to load. But it does have a **feature** that is
    /// invisible in its default state — its tick marks are guarded on both a non-`NoTicks` position
    /// and a non-zero interval, and the constructor sets neither, so the marks belonged to no
    /// snapshot. The module fills it for the same reason it fills `group_box`'s tick, which is a
    /// different question from "does it hold data"; `slider` therefore moved out of this list and
    /// into the feature list covered by [`a_state_feature_is_switched_on_for_its_own_snapshot`].
    #[test]
    fn a_control_without_a_data_concept_is_not_filled() {
        let factory = WidgetFactory::new_with_defaults();
        for name in ["button", "label", "divider", "line_edit"] {
            let Some(mut widget) =
                factory.create(name, crate::widget::census::CENSUS_RECT, "Sample")
            else {
                continue;
            };
            assert!(!apply(name, widget.as_mut()), "{name} has no data to fill");
        }
    }

    /// Each control whose *state* is the feature has that state switched on for its snapshot.
    ///
    /// # Why this needs its own assertion
    ///
    /// A control that draws something in one state and nothing in the other makes every check the
    /// crate has agree on a picture that shows no feature: the tick branch, the travel branch and
    /// the tick-mark branch are all unreachable in the default state, so a defect inside them is
    /// invisible to the snapshot, the census colour sample and every gate. This pins the set that
    /// has been switched on, so a control dropping out of it is a failure rather than a quietly
    /// emptier picture.
    #[test]
    fn a_state_feature_is_switched_on_for_its_own_snapshot() {
        use crate::widget::display_widgets::slider::{Slider, TickPosition};
        use crate::widget::display_widgets::switch::Switch;

        let factory = WidgetFactory::new_with_defaults();
        for name in ["check_box", "switch", "slider", "group_box"] {
            let Some(mut widget) =
                factory.create(name, crate::widget::census::CENSUS_RECT, "Sample")
            else {
                panic!("{name} is published by the registry but could not be built");
            };
            assert!(apply(name, widget.as_mut()), "{name} owns a state feature");
        }

        // The state each one depends on is genuinely on, rather than the fill merely returning
        // `true`: a `Slider` needs both halves of its guard, and the constructor sets neither.
        let mut slider = Slider::new(crate::core::Rect::new(0, 0, 120, 8));
        assert_eq!(slider.tick_position(), TickPosition::NoTicks, "the constructor default");
        assert!(apply("slider", &mut slider));
        assert_ne!(slider.tick_position(), TickPosition::NoTicks, "and the fill moved it");
        assert!(slider.tick_interval() > 0, "or the draw guard still refuses the marks");

        let mut switch = Switch::new(crate::core::Rect::new(0, 0, 60, 30));
        assert!(!switch.is_checked(), "the constructor default");
        assert!(apply("switch", &mut switch));
        assert!(switch.is_checked(), "and the fill turned it on");
    }

    /// The calendar's snapshot is reproducible: its date comes from the fill, not the clock.
    ///
    /// # The defect this pins
    ///
    /// `Calendar::new` seeds `selected_date`/`display_month` from `chrono::Local::now()`. That is
    /// right for the control and fatal for a committed artifact: the selection highlight is a
    /// function of the date, so `snapshots/svg/calendar.svg` changed at every midnight and
    /// `check_svg_snapshots.sh` reported a diff on the day after it was committed — a red gate no
    /// edit could explain, which the only available repair (re-commit the regenerated file)
    /// turned into the false-green direction.
    ///
    /// The assertion is what a *snapshot* needs rather than what a *control* needs: the fill must
    /// land on a fixed day, and it must land there from every starting date. The control's own
    /// today-default is asserted alongside, so a fix that pinned the constructor instead of the
    /// sample cannot pass this.
    #[test]
    fn the_calendar_sample_is_pinned_to_a_fixed_day() {
        // Holds the crate-wide theme guard: this test renders twice and compares the two frames,
        // and a concurrent test that switches the active theme would otherwise change the accent
        // colours between them — which made the comparison fail with two perfectly correct pictures
        // that differed only in the theme's reds and the today-highlight fill
        // (`rgba(179,62,60)` vs `rgba(183,50,41)`, `rgba(255,193,7)` vs `rgba(255,213,79)`).
        // Same reason and same precedent as the render tests in `widget::svg` and `avatar`.
        let _theme_guard = crate::style::theme_test_guard();
        let rect = crate::core::Rect::new(0, 0, 240, 120);
        let mut calendar = Calendar::new(rect);
        assert_eq!(
            calendar.selected_date(),
            chrono::Local::now().date_naive(),
            "the constructor still opens on today, which a host depends on"
        );
        assert!(apply("calendar", &mut calendar), "the calendar owns a clock-derived default");
        assert_eq!(
            calendar.selected_date(),
            chrono::NaiveDate::from_ymd_opt(2026, 9, 16).expect("a valid literal date"),
            "the sample must name a fixed day or the snapshot changes every midnight"
        );
        // The displayed month follows the selection, so the grid shows the pinned month too.
        assert_eq!(calendar.display_month(), calendar.selected_date());
        // And the **today** accent is pinned to the same day. `draw` used to read the wall clock
        // itself, so this cell moved at every midnight even with the selection pinned above — the
        // snapshot was still date-dependent, just less obviously so. Without this assertion that
        // half of the rotation is free to come back.
        assert_eq!(
            calendar.today(),
            chrono::NaiveDate::from_ymd_opt(2026, 9, 16).expect("a valid literal date"),
            "the painted day must come from the fixture too, or the picture rotates nightly"
        );

        // Redrawing on a different wall-clock day must not change a single byte: that is the
        // property the pin exists for. The `today` field is the only clock-derived input left.
        let once = crate::widget::svg::render_to_svg(&mut calendar);
        let twice = crate::widget::svg::render_to_svg(&mut calendar);
        assert_eq!(once, twice, "a pinned calendar renders identically on every draw");
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
