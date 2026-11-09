// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Sample-data helpers used by [`super::apply`] and [`super::compose_container`].
//!
//! Split out of `sample_fill.rs` so the module that owns the arm table stays under the
//! 2000-line limit (principle #9). The helpers and the arms still compile as one module:
//! `use super::*` gives these definitions the parent's control imports.

use super::*;

/// A fixed listing for the sample file dialog.
///
/// The dialog reads its rows from a host-supplied source, so a constructed dialog with no source
/// can only show `(file list)`: there is nowhere for a row to come from. This source is the
/// smallest honest host: three entries, one of them a directory, so the name column and the
/// selection have something to address.
pub(super) struct SampleFileSource;

impl crate::widget::dialog::file_dialog::FileEntrySource for SampleFileSource {
    fn entries(&self, _directory: &str) -> Vec<crate::widget::dialog::file_dialog::FileEntry> {
        Vec::from([
            crate::widget::dialog::file_dialog::FileEntry::new("Documents", true),
            crate::widget::dialog::file_dialog::FileEntry::new("report.pdf", false)
                .with_metadata(Some(48_000), None),
            crate::widget::dialog::file_dialog::FileEntry::new("notes.txt", false)
                .with_metadata(Some(1_200), None),
        ])
    }
}

/// Mounts sample children inside a container whose content is a mounted widget.
///
/// # Why this is part of the sample, not of the exporter
///
/// A container's content is a **child widget**, not a value: `ScrollArea`, `StackedWidget`,
/// `Splitter` and `MdiArea` paint what is mounted under them, so [`apply`] alone leaves them
/// blank. This used to live in `examples/export_control_svgs.rs`, which meant the snapshots had
/// content and every other consumer — the wasm gallery is one — still showed an empty box. Moving
/// it here gives both paths one implementation, and the geometry is translated by the container's
/// own origin so the same call works at the census rectangle (origin `0,0`) and on a mounted card.
///
/// Returns the ids that were mounted. A caller that destroys the container must unregister them:
/// the runtime does not walk a child list on teardown.
pub fn compose_container(name: &str, widget: &mut dyn Widget) -> Vec<crate::core::ObjectId> {
    let origin = widget.geometry();
    let mut mounted: Vec<crate::core::ObjectId> = Vec::new();
    let mount = |rect: Rect, text: &str, mounted: &mut Vec<crate::core::ObjectId>| {
        let rect = Rect::new(rect.x + origin.x, rect.y + origin.y, rect.width, rect.height);
        let mut label = Label::new(String::from(text), rect);
        crate::theme::apply_theme_to_widget(&mut label);
        if let Some(id) = crate::widget::runtime::register(Box::new(label)) {
            mounted.push(id);
            Some(id)
        } else {
            None
        }
    };
    match name {
        "splitter" => {
            let Some(splitter) = widget_as_mut::<Splitter>(widget) else {
                return mounted;
            };
            // `apply` already put two *model* panes in place (ids with no widget behind them), so
            // the real children replace those slots rather than adding two more.
            let panes = [
                (Rect::new(0, 0, 100, 80), "Left pane"),
                (Rect::new(100, 0, 140, 80), "Right pane"),
            ];
            for (index, (rect, text)) in panes.into_iter().enumerate() {
                let Some(id) = mount(rect, text, &mut mounted) else {
                    continue;
                };
                if index < splitter.pane_count() {
                    splitter.replace_widget(index, id);
                } else {
                    splitter.add_pane(id, 1);
                }
            }
        }
        "scroll_area" => {
            let Some(area) = widget_as_mut::<ScrollArea>(widget) else {
                return mounted;
            };
            if let Some(id) = mount(Rect::new(0, 0, 160, 24), "Scrollable content", &mut mounted) {
                area.set_widget(Some(id));
            }
        }
        "stacked_widget" => {
            let Some(stack) = widget_as_mut::<StackedWidget>(widget) else {
                return mounted;
            };
            // The two pages `apply` added are model entries; replace them with mounted labels so
            // the current page is painted. Removing first keeps `count` equal to the tree.
            while stack.count() > 0 {
                stack.remove_widget_at(0);
            }
            for text in ["Page one", "Page two"] {
                if let Some(id) = mount(Rect::new(0, 0, 200, 80), text, &mut mounted) {
                    stack.add_widget(id);
                }
            }
            stack.set_current_index(0);
        }
        "mdi_area" => {
            let Some(area) = widget_as_mut::<MdiArea>(widget) else {
                return mounted;
            };
            // Same replacement: the sub-windows `apply` added are model entries.
            area.close_all_sub_windows();
            let windows = [
                (Rect::new(16, 16, 160, 56), "Document 1", "Editor 1"),
                (Rect::new(48, 48, 160, 56), "Document 2", "Editor 2"),
            ];
            for (rect, title, content) in windows {
                let Some(id) = mount(rect, content, &mut mounted) else {
                    continue;
                };
                let index = area.add_sub_window(id, rect);
                if let Some(subwindow) = area.sub_window_mut(index) {
                    subwindow.set_title(String::from(title));
                }
            }
        }
        _ => {}
    }
    mounted
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
pub(super) fn settle(widget: &mut dyn Widget, mut step: impl FnMut(&mut dyn Widget) -> bool) {
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
pub(super) fn sample_list_rows() -> Vec<String> {
    ["Alpha", "Bravo", "Charlie", "Delta", "Echo"]
        .iter()
        .map(|label| (*label).to_string())
        .collect()
}

// `CheckBox` is imported here rather than with the other control types because it belongs to the
// `base_widgets` family and the arms above are its only use in this module.

/// A two-level cascade, so the control draws its branches *and* a leaf rather than one flat list.
///
/// A single level would not show what a cascader is for: the expand affordance and the indent are the
/// features, and a flat list renders identically to a `list_box`.
pub(super) fn sample_cascade() -> Vec<CascaderOption> {
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
pub(super) struct SampleTableModel;

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
pub(super) struct SampleTableSource;

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

    /// The sample table is a fixed array, so its projection never changes.
    ///
    /// Reporting a revision is what tells a virtual table/data grid that its window cache may be
    /// trusted: a source with no revision is refetched on **every** paint, which for the sample
    /// meant rebuilding every visible cell's text per frame. A constant revision is the honest
    /// statement for immutable data.
    fn revision(&self) -> u64 {
        1
    }
}
