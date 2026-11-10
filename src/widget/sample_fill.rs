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
use crate::core::{Color, Point, Rect, Size};
use crate::widget::base_widgets::checkbox::CheckBox;
use crate::widget::base_widgets::label::Label;
use crate::widget::capability::coercion::widget_as_mut;
use crate::widget::sample_data as sample;
use crate::widget::Widget;

use crate::compat::{String, Vec};
use crate::widget::advanced_widgets::calendar::Calendar;
use crate::widget::advanced_widgets::dial::Dial;
use crate::widget::advanced_widgets::pie_menu::PieMenu;
use crate::widget::advanced_widgets::ribbon_bar::RibbonBar;
use crate::widget::advanced_widgets::tab_bar::TabBar;
use crate::widget::advanced_widgets::time_edit::{Time, TimeEdit};
use crate::widget::chart_widgets::gauge::Gauge;
use crate::widget::chart_widgets::pie_chart::{PieChart, PieSlice};
use crate::widget::chart_widgets::{
    ChartCrosshair, ChartReferenceLine, LineChart, ReferenceLineSpec,
};
use crate::widget::companions::{
    AxisEdge, AxisPicker, ChartDataTable, ChartTooltip, ColumnAggregate, ColumnAggregateKind,
    CommandPaletteHost, DragGhostLayer, DrawerSectionHeader, DropDownPopupHost, FilterRow,
    FindReplaceBar, HeaderMenu, LayerEntry, LayerPanel, Legend, LegendEntry, LineNumberGutter,
    ListState, ListStateHost, MenuPanel, MenuPanelItem, PaletteEntryRef, PlaylistPanel, PopupItem,
    SpeedDial, SpeedDialAction, SummaryRow, SwipeAction, SwipeActions, ToastHost, TooltipLine,
    TreeFindBar,
};
use crate::widget::container_widgets::carousel::Carousel;
use crate::widget::container_widgets::groupbox::GroupBox;
use crate::widget::container_widgets::masonry_layout::MasonryLayout;
use crate::widget::container_widgets::mdiarea::MdiArea;
use crate::widget::container_widgets::scrollarea::ScrollArea;
use crate::widget::container_widgets::splitter::Splitter;
use crate::widget::container_widgets::stackedwidget::StackedWidget;
use crate::widget::container_widgets::toolbox::ToolBox;
use crate::widget::cupertino::core::{MaterialNavigationRail, MaterialSnackbar};
use crate::widget::cupertino::segmented_control::CupertinoSegmentedControl;
use crate::widget::dialog::bottom_sheet::BottomSheet;
use crate::widget::dialog::dialog_widget::Dialog;
use crate::widget::dialog::file_dialog::FileDialog;
use crate::widget::dialog::font_dialog::FontDialog;
use crate::widget::dialog::input_dialog::InputDialog;
use crate::widget::dialog::message_box::MessageBox;
use crate::widget::dialog::modal_bottom_sheet::ModalBottomSheet;
use crate::widget::dialog::popover::Popover;
use crate::widget::dialog::popup_window::PopupWindow;
use crate::widget::dialog::progress_dialog::ProgressDialog;
use crate::widget::dialog::wizard::WizardDialog;
use crate::widget::display_widgets::mini_canvas::MiniCanvas;
// `Arc` is the gauge, not the reference-counted pointer imported from `compat`.
use crate::widget::display_widgets::arc::Arc as ArcGauge;
use crate::widget::display_widgets::emoji_picker::{EmojiGlyph, EmojiPicker};
use crate::widget::display_widgets::image_view::ImageView;
use crate::widget::display_widgets::lcd_number::LCDNumber;
use crate::widget::display_widgets::meter::Meter;
use crate::widget::display_widgets::mini_chart::{DataPoint, MiniChart};
use crate::widget::display_widgets::progress_circle::ProgressCircle;
use crate::widget::display_widgets::progressbar::ProgressBar;
use crate::widget::display_widgets::slider::{Slider, TickPosition};
use crate::widget::display_widgets::switch::Switch;
use crate::widget::display_widgets::tag_group::TagGroup;
use crate::widget::input_widgets::cascader::{Cascader, CascaderOption};
use crate::widget::input_widgets::combobox::ComboBox;
use crate::widget::input_widgets::dropdown::Dropdown;
use crate::widget::input_widgets::editable_combo_box::EditableComboBox;
use crate::widget::input_widgets::font_combo_box::FontComboBox;
use crate::widget::input_widgets::listbox::ListBox;
use crate::widget::input_widgets::multi_select_combo_box::{MultiSelectComboBox, MultiSelectItem};
use crate::widget::input_widgets::shortcut_editor::{ShortcutEditor, ShortcutEntry};
use crate::widget::macos::segmented_control::MacSegmentedControl;
use crate::widget::macos::source_list::{MacSourceList, SourceListRow, SourceListSection};
use crate::widget::macos::toolbar::{MacToolbar, MacToolbarItem};
use crate::widget::media_widgets::animated_image::{AnimatedFrame, AnimatedImage};
use crate::widget::media_widgets::hero_animation::HeroAnimation;
use crate::widget::menu_toolbar::dropdown_menu::{DropdownItem as MenuEntryItem, DropdownMenu};
use crate::widget::menu_toolbar::menu::Menu;
use crate::widget::menu_toolbar::menu_bar::MenuBar;
use crate::widget::menu_toolbar::menu_button::{MenuButton, MenuItem};
use crate::widget::menu_toolbar::tool_bar::ToolBar;
use crate::widget::misc_widgets::avatar::{Avatar, GroupAvatar};
use crate::widget::misc_widgets::barcode_scanner::{BarcodeFormat, BarcodeScanner};
use crate::widget::misc_widgets::date_range_picker::{RangePreset, RangePresetBar};
use crate::widget::misc_widgets::drum_column::DrumColumn;
use crate::widget::misc_widgets::mobile_date_picker::MobileDatePicker;
use crate::widget::misc_widgets::preset_chip_bar::PresetChipBar;
use crate::widget::misc_widgets::segmented_button::{Segment, SegmentedButton};
use crate::widget::nav_widgets::bottom_navigation_bar::BottomNavigationBar;
use crate::widget::nav_widgets::navigation_drawer::NavigationDrawer;
use crate::widget::nav_widgets::tab_view::TabView;
use crate::widget::overlay_widgets::refresh_control::RefreshControl;
use crate::widget::overlay_widgets::swipe_to_dismiss::SwipeToDismiss;
use crate::widget::special_widgets::breadcrumb::{Breadcrumb, BreadcrumbSegment};
use crate::widget::special_widgets::canvas::Canvas;
use crate::widget::special_widgets::chip::{Chip, ChipInput, ChipItem};
use crate::widget::special_widgets::command_palette::{CommandEntry, CommandPalette};
#[cfg(any(feature = "code-editor", feature = "markdown-editor"))]
use crate::widget::special_widgets::editor::companions::FindBar;
use crate::widget::special_widgets::freeform_shape::{FreeformShapeWidget, ShapeCanvas, ShapePath};
use crate::widget::special_widgets::grid::GridWidget;
#[cfg(feature = "markdown-editor")]
use crate::widget::special_widgets::markdown_editor::{MarkdownOutlineEntry, MarkdownOutlinePanel};
use crate::widget::special_widgets::notification_center::{
    NotificationCenter, NotificationItem, NotificationLevel,
};
use crate::widget::special_widgets::segmented_control::{SegmentItem, SegmentedControl};
use crate::widget::special_widgets::signature_pad::{SignaturePad, SignatureStroke};
use crate::widget::special_widgets::snackbar::Snackbar;
use crate::widget::special_widgets::timeline_widget::{TimelineItem, TimelineWidget};
use crate::widget::special_widgets::toast::{ToastItem, ToastLevel, ToastStack};
use crate::widget::view_widgets::data_grid::DataGrid;
use crate::widget::view_widgets::data_grid::{ColumnSpec, GridViewHeader};
use crate::widget::view_widgets::data_source::IncrementalTableDataSource;
use crate::widget::view_widgets::grid_table::GridTableWidget;
use crate::widget::view_widgets::image_gallery::ImageGallery;
use crate::widget::view_widgets::list_view::{ListView, VecListModel};
use crate::widget::view_widgets::properties_panel::{
    PropertiesPanel, PropertyEntry, PropertyValue,
};
use crate::widget::view_widgets::query_builder::{FilterField, QueryBuilder};
use crate::widget::view_widgets::table_widget::{TableModel, TableWidget};
use crate::widget::view_widgets::tree_table::TreeTable;
use crate::widget::view_widgets::tree_view::{TreeView, VecTreeModel};
use crate::widget::view_widgets::virtual_list::VirtualList;
use crate::widget::view_widgets::virtual_table::VirtualTable;
use crate::widget::view_widgets::FilterOperator;

/// The day every date-bearing sample is pinned to.
///
/// A committed snapshot must regenerate on any machine on any day, so the sample data — not the
/// control — names the date it shows. The exporter pins the thread-local clock to the same day
/// (RW-56), so a control that marks "today" and a control that shows a selected day agree, and
/// both stay byte-identical tomorrow.
pub const SNAPSHOT_DAY: (i32, u32, u32) = (2026, 9, 16);

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

        // `MobileDatePicker::new` starts on a fixed literal date and `draw` marks the wall-clock
        // day with an accent dot, so the picture used to depend on the day it was generated: the
        // dot wandered or vanished as the calendar moved. The arm names the pinned snapshot day,
        // which puts the selected value and the marker in the same picture and on a fixed date.
        "mobile_date_picker" => match widget_as_mut::<MobileDatePicker>(widget) {
            Some(picker) => {
                picker.set_date(SNAPSHOT_DAY.0, SNAPSHOT_DAY.1, SNAPSHOT_DAY.2);
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
                sheet.set_title("Filters");
                sheet.set_content(Box::new(Label::new(
                    "Sheet content".to_string(),
                    Rect::new(0, 0, 200, 40),
                )));
                sheet.show();
                // The panel rises in; a half-arrived sheet is a blank card. See `bottom_sheet`.
                settle(widget, |w| {
                    let mut settled = false;
                    if let Some(sheet) = widget_as_mut::<ModalBottomSheet>(w) {
                        settled = !sheet.tick(1000);
                    }
                    settled
                });
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

        // A group paints its child tags, so an empty group is an empty frame. Three tags put the
        // wrap, the spacing and the chip chrome into the snapshot.
        "tag_group" => match widget_as_mut::<TagGroup>(widget) {
            Some(group) => {
                group.add_tag("Alpha");
                group.add_tag("Bravo");
                group.add_tag("Charlie");
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
        // The commands are in the widget's own space, so they are offset by its geometry. Fixed
        // literals put them at `(12, 16)` **of the window**, and the control's own viewport test
        // then discarded every command outside its rectangle — a mounted canvas drew an empty box.
        "canvas" => {
            let origin = widget.geometry();
            match widget_as_mut::<Canvas>(widget) {
                Some(canvas) => {
                    let (dx, dy) = (origin.x, origin.y);
                    canvas.fill_rect(Rect::new(12 + dx, 16 + dy, 80, 40), Color::rgb(33, 150, 243));
                    canvas.draw_line(
                        Point::new(12 + dx, 72 + dy),
                        Point::new(228 + dx, 72 + dy),
                        Color::rgb(200, 200, 200),
                    );
                    canvas.fill_circle(Point::new(160 + dx, 60 + dy), 18, Color::rgb(255, 152, 0));
                    true
                }
                None => false,
            }
        }
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
                drawer.set_title("Library");
                drawer.add_section_header("Browse");
                drawer.add_item("home", "Home");
                drawer.add_item("search", "Search");
                drawer.add_section_header("Library");
                drawer.add_item("star", "Favourites");
                drawer.add_item("folder", "Documents");
                drawer.set_selected_index(0);
                drawer.open();
                // The drawer slides in; without settling it the card shows the bare page.
                settle(widget, |w| {
                    let mut settled = false;
                    if let Some(drawer) = widget_as_mut::<NavigationDrawer>(w) {
                        settled = !drawer.tick(1000);
                    }
                    settled
                });
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
        // Emit the tree find strip in its open state with a query, so its painted field and text
        // are exercised by the census/snapshot rather than skipped as "not opened yet".
        "tree_find_bar" => match widget_as_mut::<TreeFindBar>(widget) {
            Some(bar) => {
                bar.set_query("widget");
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

        // ── Containers: the *state* is the content ──
        //
        // These four are composed through their own content model (sub-windows, panes, a
        // content extent, pages). A snapshot renders the control itself, so the state below is
        // what puts the container's own chrome — frames, dividers, the scroll bar, the page
        // count — into the picture. The ids are model entries, not mounted controls: the tree
        // painter draws mounted children, and a snapshot has no window to mount them into.
        "mdi_area" => match widget_as_mut::<MdiArea>(widget) {
            Some(area) => {
                let first = area.add_sub_window(1, Rect::new(16, 16, 160, 56));
                let second = area.add_sub_window(2, Rect::new(48, 48, 160, 56));
                if let Some(subwindow) = area.sub_window_mut(first) {
                    subwindow.set_title(crate::compat::String::from("Document 1"));
                }
                if let Some(subwindow) = area.sub_window_mut(second) {
                    subwindow.set_title(crate::compat::String::from("Document 2"));
                }
                true
            }
            None => false,
        },
        "splitter" => match widget_as_mut::<Splitter>(widget) {
            Some(splitter) => {
                splitter.add_pane(1, 1);
                splitter.add_pane(2, 1);
                true
            }
            None => false,
        },
        "scroll_area" => match widget_as_mut::<ScrollArea>(widget) {
            Some(area) => {
                area.set_content_size(Size::new(160, 240));
                true
            }
            None => false,
        },
        "stacked_widget" => match widget_as_mut::<StackedWidget>(widget) {
            Some(stack) => {
                stack.add_widget(1);
                stack.add_widget(2);
                stack.set_current_index(0);
                true
            }
            None => false,
        },

        // ── Companion chrome that only paints once it has content (RW-77/RW-81/RW-100) ──
        //
        // These three are constructible through the factory but carry no data of their own at
        // rest, so the verification paths would photograph an empty frame. The arms give each
        // the smallest state that makes its own drawing branch run.
        "chart_crosshair" => match widget_as_mut::<ChartCrosshair>(widget) {
            Some(crosshair) => {
                crosshair.set_position(Some(Point::new(120, 60)));
                true
            }
            None => false,
        },
        "preset_chip_bar" => match widget_as_mut::<PresetChipBar>(widget) {
            Some(bar) => {
                let first = bar.add_preset("Preset A");
                let _ = bar.add_preset("Preset B");
                bar.set_selected(first);
                true
            }
            None => false,
        },

        // ── Data-bearing controls that were still given nothing ──
        //
        // Each of these draws content it does not own and was constructed with none, so a gallery
        // of every control showed an empty box or its own "nothing here" placeholder. The data set
        // is the same small, fixed one the rest of this module uses.
        "animated_image" => match widget_as_mut::<AnimatedImage>(widget) {
            Some(image) => {
                // Two 24×24 RGBA frames: a picture and a frame counter, a few kilobytes of data.
                const SIDE: usize = 24;
                let mut frames = Vec::new();
                for color in [Color::rgb(33, 150, 243), Color::rgb(255, 152, 0)] {
                    let mut data = Vec::with_capacity(SIDE * SIDE * 4);
                    for _ in 0..(SIDE * SIDE) {
                        data.push(color.r);
                        data.push(color.g);
                        data.push(color.b);
                        data.push(color.a);
                    }
                    frames.push(AnimatedFrame {
                        data,
                        width: SIDE as u32,
                        height: SIDE as u32,
                        delay_ms: 400,
                    });
                }
                image.load_frames(frames).is_ok()
            }
            None => false,
        },
        "arc" => match widget_as_mut::<ArcGauge>(widget) {
            Some(arc) => {
                arc.set_range(0, 100);
                arc.set_value(72);
                true
            }
            None => false,
        },
        "axis_picker" => match widget_as_mut::<AxisPicker>(widget) {
            Some(picker) => {
                picker.set_series(vec![
                    "Revenue".to_string(),
                    "Cost".to_string(),
                    "Margin".to_string(),
                ]);
                picker.set_edge_label(AxisEdge::Left, "Value");
                picker.set_edge_label(AxisEdge::Bottom, "Time");
                true
            }
            None => false,
        },
        "barcode_scanner" => match widget_as_mut::<BarcodeScanner>(widget) {
            Some(scanner) => {
                scanner.detect_barcode("RW-2026-09-16".to_string(), BarcodeFormat::QRCode);
                true
            }
            None => false,
        },
        "chart_data_table" => match widget_as_mut::<ChartDataTable>(widget) {
            Some(table) => {
                let series: Vec<(String, Vec<(f64, f64)>)> = sample::SERIES
                    .iter()
                    .map(|(name, points)| {
                        ((*name).to_string(), points.iter().map(|(x, y)| (*x, *y)).collect())
                    })
                    .collect();
                table.set_from_series(&series);
                true
            }
            None => false,
        },
        "gauge" => match widget_as_mut::<Gauge>(widget) {
            Some(gauge) => {
                let _ = gauge.set_maximum(100.0);
                gauge.set_value(65.0);
                gauge.set_unit_label("deg".to_string());
                true
            }
            None => false,
        },
        "meter" => match widget_as_mut::<Meter>(widget) {
            Some(meter) => {
                meter.set_range(0, 100);
                meter.set_value(65);
                true
            }
            None => false,
        },
        "lcd_number" => match widget_as_mut::<LCDNumber>(widget) {
            Some(lcd) => {
                lcd.set_range(0.0, 9999.0);
                lcd.set_value(2026.0);
                true
            }
            None => false,
        },
        "progress_bar" => match widget_as_mut::<ProgressBar>(widget) {
            Some(bar) => {
                bar.set_range(0, 100);
                bar.set_value(45);
                true
            }
            None => false,
        },
        "progress_circle" => match widget_as_mut::<ProgressCircle>(widget) {
            Some(circle) => {
                circle.set_value(0.45);
                // The groove is themed from the accent role and the arc is the accent too, so at
                // the default colours the progress was invisible against its own track.
                circle.set_track_color(Color::rgb(224, 224, 224));
                // The ring paints the driver's current value, so the tween is advanced here or a
                // still frame shows an empty ring. Bounded, so a stuck driver cannot wedge us.
                for _ in 0..600 {
                    let _ = circle.tick(16);
                    if !circle.is_animating() {
                        break;
                    }
                }
                true
            }
            None => false,
        },
        "mini_chart" => match widget_as_mut::<MiniChart>(widget) {
            Some(chart) => {
                chart.set_data(Vec::from([
                    DataPoint::new("Mon", 12),
                    DataPoint::new("Tue", 48),
                    DataPoint::new("Wed", 31),
                    DataPoint::new("Thu", 7),
                    DataPoint::new("Fri", 25),
                ]));
                true
            }
            None => false,
        },
        "pie_chart" => match widget_as_mut::<PieChart>(widget) {
            Some(chart) => {
                // The constructor seeds values but no names, which is why the slices read as "...".
                chart.set_slices(Vec::from([
                    PieSlice::new("Alpha", 40.0, Color::rgb(33, 150, 243)),
                    PieSlice::new("Beta", 30.0, Color::rgb(255, 152, 0)),
                    PieSlice::new("Gamma", 20.0, Color::rgb(76, 175, 80)),
                    PieSlice::new("Delta", 10.0, Color::rgb(244, 67, 54)),
                ]));
                true
            }
            None => false,
        },
        "time_edit" | "time_picker" => match widget_as_mut::<TimeEdit>(widget) {
            Some(edit) => {
                edit.set_time(Time::new(9, 30, 0, 0));
                true
            }
            None => false,
        },
        "line_number_gutter" => match widget_as_mut::<LineNumberGutter>(widget) {
            Some(gutter) => {
                // `set_range` is the document, `set_visible_lines` is the viewport.
                gutter.set_range(1, 12);
                gutter.set_visible_lines(1, 12);
                gutter.set_current_line(3);
                true
            }
            None => false,
        },
        "summary_row" => match widget_as_mut::<SummaryRow>(widget) {
            Some(row) => {
                row.set_aggregates(Vec::from([
                    ColumnAggregate::new("qty", ColumnAggregateKind::Sum, "Sum"),
                    ColumnAggregate::new("price", ColumnAggregateKind::Average, "Avg"),
                ]));
                row.set_value("qty", 102.0);
                row.set_value("price", 88.75);
                true
            }
            None => false,
        },
        "filter_row" => match widget_as_mut::<FilterRow>(widget) {
            Some(row) => {
                row.set_columns(Vec::from([
                    "Name".to_string(),
                    "Qty".to_string(),
                    "Status".to_string(),
                ]));
                true
            }
            None => false,
        },
        "drum_column" => match widget_as_mut::<DrumColumn>(widget) {
            Some(column) => {
                column.set_items(sample::list_items());
                column.set_selected_index(2);
                true
            }
            None => false,
        },
        "chip_input" => match widget_as_mut::<ChipInput>(widget) {
            Some(input) => {
                input.set_items(Vec::from([
                    ChipItem::new("alpha", "Alpha"),
                    ChipItem::new("bravo", "Bravo"),
                    ChipItem::new("charlie", "Charlie"),
                ]));
                true
            }
            None => false,
        },
        "dropdown_menu" => match widget_as_mut::<DropdownMenu>(widget) {
            Some(menu) => {
                menu.add_item(MenuEntryItem::new("new", "New"));
                menu.add_item(MenuEntryItem::new("open", "Open"));
                menu.add_item(MenuEntryItem::new("save", "Save").with_shortcut("Ctrl+S"));
                menu.select("open");
                true
            }
            None => false,
        },
        "file_dialog" => match widget_as_mut::<FileDialog>(widget) {
            Some(dialog) => {
                // List mode, not Details: at 240 × 120 a Details view spends its whole list band
                // on the column-header row, so no data row could ever fit.
                dialog.set_view_mode(crate::widget::dialog::file_dialog::ViewMode::List);
                dialog.set_directory("Documents");
                dialog.set_source(Box::new(SampleFileSource));
                true
            }
            None => false,
        },
        "emoji_picker" => match widget_as_mut::<EmojiPicker>(widget) {
            Some(picker) => {
                // The bundled faces have no pictographs (☺ ♥ ✓ ★ all paint as tofu), so the sample
                // uses the text emoticons these faces can actually draw.
                picker.set_glyphs(Vec::from([
                    EmojiGlyph::new("smile", ":)", "Faces").keyword("happy"),
                    EmojiGlyph::new("grin", ":D", "Faces").keyword("laugh"),
                    EmojiGlyph::new("wink", ";)", "Faces").keyword("wink"),
                    EmojiGlyph::new("heart", "<3", "Symbols").keyword("love"),
                ]));
                picker.set_categories(Vec::from(["Faces".to_string(), "Symbols".to_string()]));
                true
            }
            None => false,
        },
        "mac_segmented_control" => match widget_as_mut::<MacSegmentedControl>(widget) {
            Some(control) => {
                control.set_segments(Vec::from([
                    "Day".to_string(),
                    "Week".to_string(),
                    "Month".to_string(),
                ]));
                true
            }
            None => false,
        },
        "mac_source_list" => match widget_as_mut::<MacSourceList>(widget) {
            Some(list) => {
                list.set_sections(Vec::from([
                    SourceListSection::new("Library").with_rows(Vec::from([
                        SourceListRow::new("all", "All"),
                        SourceListRow::new("recent", "Recent"),
                    ])),
                    SourceListSection::new("Folders").with_rows(Vec::from([
                        SourceListRow::new("documents", "Documents"),
                        SourceListRow::new("downloads", "Downloads"),
                    ])),
                ]));
                true
            }
            None => false,
        },
        "chart_tooltip" => {
            let origin = widget.geometry();
            match widget_as_mut::<ChartTooltip>(widget) {
                Some(tooltip) => {
                    tooltip.show_at(
                        Point::new(origin.x + 16, origin.y + 16),
                        Vec::from([
                            TooltipLine::new("Revenue 12.0"),
                            TooltipLine::colored("Cost 8.0", Color::rgb(255, 152, 0)),
                        ]),
                    );
                    true
                }
                None => false,
            }
        }
        "toast_host" => match widget_as_mut::<ToastHost>(widget) {
            Some(host) => {
                host.push_item(ToastItem::new(
                    "saved",
                    "Saved successfully",
                    ToastLevel::Info,
                    4000,
                ));
                true
            }
            None => false,
        },

        // ── Content that needs a host: shapes, strokes, images, rows ──
        //
        // These render data they do not own. Constructed with nothing they paint their empty state,
        // which teaches a reader nothing. The data is generated here rather than loaded, so the
        // snapshots stay byte-identical on any machine.
        "freeform_shape" => match widget_as_mut::<FreeformShapeWidget>(widget) {
            Some(shape) => {
                shape.set_path(ShapePath::Star { points: 6, inner_radius: 0.5 });
                true
            }
            None => false,
        },
        "shape_canvas" => {
            let origin = widget.geometry();
            match widget_as_mut::<ShapeCanvas>(widget) {
                Some(canvas) => {
                    canvas.add_shape(FreeformShapeWidget::new(
                        Rect::new(origin.x + 14, origin.y + 18, 70, 70),
                        ShapePath::Star { points: 5, inner_radius: 0.45 },
                    ));
                    canvas.add_shape(FreeformShapeWidget::new(
                        Rect::new(origin.x + 110, origin.y + 26, 90, 70),
                        ShapePath::Heart,
                    ));
                    true
                }
                None => false,
            }
        }
        "signature_pad" => {
            let origin = widget.geometry();
            match widget_as_mut::<SignaturePad>(widget) {
                Some(pad) => {
                    let stroke = SignatureStroke::from_points(Vec::from([
                        Point::new(origin.x + 24, origin.y + 76),
                        Point::new(origin.x + 60, origin.y + 40),
                        Point::new(origin.x + 100, origin.y + 80),
                        Point::new(origin.x + 148, origin.y + 36),
                    ]));
                    pad.set_strokes(Vec::from([stroke]));
                    true
                }
                None => false,
            }
        }
        "image_view" => match widget_as_mut::<ImageView>(widget) {
            Some(view) => {
                // A generated checkerboard: no asset, no decoder, visible at any size.
                const SIDE: u32 = 32;
                let mut pixels = Vec::with_capacity((SIDE * SIDE * 4) as usize);
                for y in 0..SIDE {
                    for x in 0..SIDE {
                        let light = (x / 8 + y / 8) % 2 == 0;
                        let (r, g, b) = if light { (33, 150, 243) } else { (236, 240, 245) };
                        pixels.extend_from_slice(&[r, g, b, 255]);
                    }
                }
                view.set_image(crate::image::Image::from_raw(
                    crate::image::ImageData::Rgba8(pixels),
                    SIDE,
                    SIDE,
                ));
                true
            }
            None => false,
        },
        "hero_animation" => {
            let origin = widget.geometry();
            match widget_as_mut::<HeroAnimation>(widget) {
                Some(hero) => {
                    let source_rect = Rect::new(origin.x + 12, origin.y + 16, 80, 56);
                    let target_rect = Rect::new(origin.x + 132, origin.y + 16, 80, 56);
                    hero.set_source(Box::new(Label::new("Source".to_string(), source_rect)));
                    hero.set_target(Box::new(Label::new("Target".to_string(), target_rect)));
                    hero.set_transition_rects(Some(source_rect), Some(target_rect));
                    hero.set_progress(0.5);
                    true
                }
                None => false,
            }
        }
        "query_builder" => match widget_as_mut::<QueryBuilder>(widget) {
            Some(builder) => {
                builder.set_fields(Vec::from([
                    FilterField::text(0, "Name"),
                    FilterField::number(1, "Qty"),
                ]));
                if let Some(row) = builder.add_row(0) {
                    let _ = builder.set_row_operator(row, FilterOperator::Contains);
                    let _ = builder.set_row_operand(row, "Widget");
                }
                if let Some(row) = builder.add_row(1) {
                    let _ = builder.set_row_operator(row, FilterOperator::GreaterThan);
                    let _ = builder.set_row_operand(row, "10");
                }
                true
            }
            None => false,
        },
        "masonry_layout" => match widget_as_mut::<MasonryLayout>(widget) {
            Some(layout) => {
                layout.add_item("Alpha", 48, Color::rgb(33, 150, 243));
                layout.add_item("Bravo", 72, Color::rgb(255, 152, 0));
                layout.add_item("Charlie", 40, Color::rgb(76, 175, 80));
                layout.add_item("Delta", 60, Color::rgb(156, 39, 176));
                true
            }
            None => false,
        },
        #[cfg(feature = "markdown-editor")]
        "markdown_outline_panel" => match widget_as_mut::<MarkdownOutlinePanel>(widget) {
            Some(panel) => {
                panel.set_entries(Vec::from([
                    MarkdownOutlineEntry { line: 0, level: 1, title: "Overview".to_string() },
                    MarkdownOutlineEntry { line: 12, level: 2, title: "Details".to_string() },
                    MarkdownOutlineEntry { line: 24, level: 2, title: "History".to_string() },
                ]));
                panel.set_active_index(Some(0));
                true
            }
            None => false,
        },
        #[cfg(any(feature = "code-editor", feature = "markdown-editor"))]
        "find_bar" => match widget_as_mut::<FindBar>(widget) {
            Some(bar) => {
                bar.set_query("widget");
                true
            }
            None => false,
        },
        "grid_view_header" => match widget_as_mut::<GridViewHeader>(widget) {
            Some(header) => {
                header.set_columns(Vec::from([
                    ColumnSpec::new("Name", 110),
                    ColumnSpec::new("Qty", 60),
                    ColumnSpec::new("Status", 70),
                ]));
                true
            }
            None => false,
        },
        "popup_window" => match widget_as_mut::<PopupWindow>(widget) {
            Some(window) => {
                window.set_title("Popup".to_string());
                // Register the sample label only once. A second `apply` on the same popup used to
                // mount a second label and abandon the first (the popup unlinks, but does not
                // unregister, the replaced content), leaking a mounted widget and its a11y node on
                // every call. A host that already supplied content also keeps it.
                if window.content_widget().is_none() {
                    if let Some(id) = crate::widget::runtime::register(Box::new(Label::new(
                        "Popup content".to_string(),
                        Rect::new(0, 0, 160, 24),
                    ))) {
                        window.set_content_widget(Some(id));
                    }
                }
                true
            }
            None => false,
        },
        "mac_toolbar" => match widget_as_mut::<MacToolbar>(widget) {
            Some(toolbar) => {
                toolbar.set_items(Vec::from([
                    MacToolbarItem::new("back", "Back"),
                    MacToolbarItem::new("forward", "Forward"),
                    MacToolbarItem::new("share", "Share"),
                ]));
                true
            }
            None => false,
        },

        _ => false,
    }
}

mod helpers;

pub use helpers::compose_container;
use helpers::{
    sample_cascade, sample_list_rows, settle, SampleFileSource, SampleTableModel, SampleTableSource,
};

#[cfg(test)]
mod tests;
