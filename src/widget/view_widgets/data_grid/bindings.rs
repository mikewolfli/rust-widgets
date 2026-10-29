// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Companion-widget bindings: pagination, filter row, chooser, status bar, summary, header menu.

use super::*;

/// The live link between a [`DataGrid`] and a [`Pagination`] bar.
///
/// A widget cannot hold a borrow of another widget, so the link is the pagination's
/// `page_changed` slot plus the shared cell it writes. [`DataGrid::poll_pagination`] reads that
/// cell, which is what keeps a page click from requiring the host to forward it by hand.
pub(crate) struct PaginationBinding {
    /// The page the bound bar most recently reported.
    pub(crate) page: Arc<AtomicU64>,
    /// Items per page, copied when the bar was bound.
    pub(crate) page_size: usize,
    /// The subscription, dropped with the binding so a rebound grid leaves no slot behind.
    pub(crate) _connection: ConnectionHandle,
}

/// The live link between a [`DataGrid`] and a [`FilterRow`] (chapter 3, gap 2).
///
/// A row publishes the complete condition list after every change, so the link parks the latest
/// report in a shared slot and [`DataGrid::poll_filter_row`] folds it into the grid's filter model.
/// This is the same host-free shape as [`PaginationBinding`]: a widget cannot hold a borrow of
/// another, so the signal writes state and the grid reads it on its next paint or interaction.
pub(crate) struct FilterBinding {
    /// The condition list the row published since the last poll.
    pub(crate) conditions: Arc<Mutex<Option<Vec<FilterCondition>>>>,
    /// The subscription, dropped with the binding so a detached grid leaves no slot behind.
    pub(crate) _scope: ConnectionScope,
}

/// The live link between a [`DataGrid`] and a [`ColumnChooser`] (V-015).
///
/// The chooser's signals carry persisted column tokens rather than source indices, so the link
/// resolves each token through the column set both controls were configured with and parks the
/// answer in a shared queue. [`DataGrid::poll_column_chooser`] drains that queue, which is the
/// same host-free pattern the pagination binding uses.
pub(crate) struct ColumnChooserBinding {
    /// `(source column, visible)` changes reported since the last poll.
    pub(crate) visibility: Arc<Mutex<Vec<(usize, bool)>>>,
    /// The display order the chooser last reported, mapped to source columns.
    pub(crate) order: Arc<Mutex<Vec<usize>>>,
    /// The subscriptions, dropped with the binding so a re-attach leaves no slot behind.
    pub(crate) _scope: ConnectionScope,
}

/// The live link between a [`DataGrid`] and a [`GridStatusBar`] (COMP-A8).
///
/// The strip consumes no grid signal, so the link has no subscription to hold; it keeps the
/// shared count cell, the repaint request, and the counts last written. The last-write record is
/// what makes the poll idempotent: a second poll with unchanged rows and selection returns
/// `false` instead of re-requesting a repaint.
pub(crate) struct GridStatusBarBinding {
    /// The count cell the strip reads through [`GridStatusBar::row_count`].
    pub(crate) counts: Arc<Mutex<(u64, u64)>>,
    /// The strip's paint request, emitted after a write.
    pub(crate) redraw: GenericSignal,
    /// The `(rows, selected)` pair last written.
    pub(crate) pushed: (u64, u64),
    /// The subscription scope. The strip owns no grid-facing signal, so it holds no connection;
    /// the scope is kept so the binding has the same shape as every other companion link.
    pub(crate) _scope: ConnectionScope,
}

/// The live link between a [`DataGrid`] and a [`SummaryRow`] (COMP-A8).
///
/// The grid computes the aggregates from its projected rows and writes the row's shared value
/// cell, so the row repaints without the host copying numbers. `key` records the projection the
/// values describe, which is what keeps a steady frame from re-reading the model.
pub(crate) struct SummaryRowBinding {
    /// The value cell the row reads through [`SummaryRow::value`].
    pub(crate) values: Arc<Mutex<Vec<(String, f64)>>>,
    /// The row's paint request, emitted after a write.
    pub(crate) redraw: GenericSignal,
    /// The aggregates copied from the row at attach time.
    pub(crate) aggregates: Vec<ColumnAggregate>,
    /// The projection the stored values were computed from, or `None` before the first push.
    pub(crate) key: Option<SummaryProjectionKey>,
    /// The subscription scope; a summary row owns no grid-facing signal.
    pub(crate) _scope: ConnectionScope,
}

/// The inputs that decide a summary row's numbers.
///
/// A data change, a filter edit or a sort edit all move the projected rows, and the aggregate
/// list itself decides which cells are reported. Comparing this key is how
/// [`DataGrid::poll_summary_row`] tells "the projection moved" from "the frame is steady" without
/// walking the model.
#[derive(Clone, PartialEq)]
pub(crate) struct SummaryProjectionKey {
    /// The source revision, or the grid's own generation when the source has none.
    pub(crate) revision: u64,
    /// The filter in force.
    pub(crate) filter: FilterExpr,
    /// The sort order in force.
    pub(crate) sorts: Vec<SortSpec>,
    /// The aggregate list being reported.
    pub(crate) aggregates: Vec<ColumnAggregate>,
}

/// One intent reported by an attached [`HeaderMenu`] (COMP-A8).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HeaderMenuRequest {
    /// Sort `column` in `state`; `None` clears the column's sort.
    Sort { column: usize, state: SortState },
    /// Filter `column` with `operator` against `operand`.
    Filter { column: usize, operator: FilterOperator, operand: String },
    /// Hide `column`.
    Hide { column: usize },
    /// Open a column chooser for `column`, when its token resolved.
    Choose { column: Option<usize> },
}

/// The live link between a [`DataGrid`] and a [`HeaderMenu`] (COMP-A8).
///
/// The menu reports intents; the link resolves its column tokens against the grid's header at
/// attach time and parks the resolved requests in a shared queue. [`DataGrid::poll_header_menu`]
/// drains that queue, the same host-free shape the chooser and filter-row links use.
pub(crate) struct HeaderMenuBinding {
    /// The requests reported since the last poll.
    pub(crate) requests: Arc<Mutex<Vec<HeaderMenuRequest>>>,
    /// The subscriptions, dropped with the binding so a re-attach leaves no slot behind.
    pub(crate) _scope: ConnectionScope,
}

/// What [`DataGrid::poll_header_menu`] did, or why it could not do something (COMP-A8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeaderMenuPoll {
    /// The menu reported nothing since the last poll.
    Idle,
    /// At least one sort, filter or hide request was applied to the grid.
    Applied,
    /// A filter request named an operator this grid's filter path cannot express. The operator is
    /// reported rather than dropped; the other queued requests were still applied.
    UnsupportedFilterOperator(FilterOperator),
    /// The menu asked the host to open a column chooser. No grid state changed; the request is
    /// also kept for [`DataGrid::take_column_chooser_request`].
    ColumnChooserRequested,
}

impl HeaderMenuPoll {
    /// Whether the menu reported nothing.
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Whether a grid state change was applied.
    pub fn changed(&self) -> bool {
        matches!(self, Self::Applied)
    }
}

/// Whether the grid's filter path can express `operator`.
///
/// Every [`FilterOperator`] the expression model publishes today is accepted by
/// [`FilterCondition`]. The predicate is written out rather than assumed so a future operator that
/// only some surfaces understand is reported through
/// [`HeaderMenuPoll::UnsupportedFilterOperator`] instead of being silently ignored.
fn filter_operator_supported(operator: FilterOperator) -> bool {
    matches!(
        operator,
        FilterOperator::Contains
            | FilterOperator::Equals
            | FilterOperator::StartsWith
            | FilterOperator::EndsWith
            | FilterOperator::NotContains
            | FilterOperator::EqualsNumber
            | FilterOperator::GreaterThan
            | FilterOperator::GreaterOrEqual
            | FilterOperator::LessThan
            | FilterOperator::LessOrEqual
    )
}

/// Resolves a chooser token to the grid's source column.
///
/// The exact token is preferred; when a width or capability differs between the two controls, the
/// title alone still identifies the column, so a host that changed a width after attaching keeps
/// its visibility wiring.
fn resolve_column_token(
    token: &str,
    tokens: &HashMap<String, usize>,
    titles: &HashMap<String, usize>,
) -> Option<usize> {
    tokens
        .get(token)
        .copied()
        .or_else(|| ColumnSpec::from_token(token).and_then(|spec| titles.get(&spec.title).copied()))
}

impl DataGrid {
    /// Binds `pagination` to this grid.
    ///
    /// Binding does three things and each one is needed for the pair to be usable: the bar learns
    /// the grid's row count, the grid adopts the bar's page size and current page, and the bar's
    /// `page_changed` stream is subscribed so a page click reaches [`Self::poll_pagination`].
    ///
    /// The bar's page size is authoritative at bind time. Changing it afterwards needs a second
    /// `set_pagination`, because a widget cannot hold a borrow of another and so cannot re-read a
    /// value it was never signalled about.
    pub fn set_pagination(&mut self, pagination: &mut Pagination) {
        pagination.set_total(self.row_count());
        let page_size = pagination.page_size().max(1);
        let page = pagination.page();
        let shared = Arc::new(AtomicU64::new(page as u64));
        let slot = Arc::clone(&shared);
        let connection = pagination.page_changed.connect(move |page| {
            slot.store(*page as u64, Ordering::Relaxed);
        });
        self.pagination =
            Some(PaginationBinding { page: shared, page_size, _connection: connection });
        self.scroll_to_page(page);
        self.pagination_applied_page = page;
    }

    /// Applies a page the bound bar reported since the last poll.
    ///
    /// Called from `draw` and `handle_event`, so a click on the bar is picked up on the next frame
    /// or the next interaction without the host forwarding it. Returns the page that was applied.
    pub fn poll_pagination(&mut self) -> Option<usize> {
        let page = self.pagination.as_ref()?.page.load(Ordering::Relaxed) as usize;
        if page == self.pagination_applied_page {
            return None;
        }
        self.apply_page(page);
        Some(page)
    }

    /// Applies `page` to the scroll position and emits `page_applied`.
    ///
    /// Returns whether the visible rows moved. The signal fires even when the page was already
    /// current: a caller that asked to go to page 3 wants the acknowledgement, not silence.
    pub fn apply_page(&mut self, page: usize) -> bool {
        let moved = self.scroll_to_page(page);
        self.pagination_applied_page = page;
        self.page_applied.emit(page);
        moved
    }

    /// Scrolls so `page`'s first row is at the top, without emitting.
    fn scroll_to_page(&mut self, page: usize) -> bool {
        let page_size = self
            .pagination
            .as_ref()
            .map_or_else(|| self.visible_row_capacity().max(1), |binding| binding.page_size.max(1));
        let first_row = page.saturating_mul(page_size);
        let clamped = first_row.min(self.row_count().saturating_sub(1));
        if self.scroll_row == clamped {
            return false;
        }
        self.set_scroll_row(clamped);
        true
    }

    /// Pushes the grid's current page and row count back into `pagination`.
    ///
    /// This is the other half of the binding: `set_pagination` is bar-to-grid, this is
    /// grid-to-bar. Returns whether the bar's page changed.
    pub fn sync_pagination(&mut self, pagination: &mut Pagination) -> bool {
        let Some(binding) = self.pagination.as_ref() else {
            return false;
        };
        let page_size = binding.page_size.max(1);
        let page = self.scroll_row / page_size;
        pagination.set_total(self.row_count());
        let before = pagination.page();
        pagination.set_page(page);
        binding.page.store(page as u64, Ordering::Relaxed);
        self.pagination_applied_page = page;
        before != page
    }

    /// The bound page and page size, when a pagination bar is bound.
    pub fn pagination_state(&self) -> Option<(usize, usize)> {
        self.pagination.as_ref().map(|binding| (self.pagination_applied_page, binding.page_size))
    }

    // ── FilterRow wiring (chapter 3, gap 2) ──

    /// Follows `row`'s `filter_changed` stream (chapter 3, gap 2).
    ///
    /// A filter row publishes complete condition lists — a commit, a clear and `filter_cleared`'s
    /// empty list all arrive on `filter_changed` — so the link parks the newest list and
    /// [`Self::poll_filter_row`] installs it as one conjunction, the same semantics
    /// [`Self::set_filters`] has. Attaching makes the row authoritative: its current conditions
    /// replace the grid's filter immediately, and every later report does the same. Conditions
    /// address source columns exactly as the row reports them; the grid does not remap them.
    ///
    /// Programmatic filtering is untouched: [`Self::set_filters`] and [`Self::set_filter_expr`]
    /// keep their behaviour, and a grid with no row attached never polls. Attaching replaces any
    /// previous row, so calling this twice leaves one subscription rather than two.
    pub fn attach_filter_row(&mut self, row: &FilterRow) {
        let conditions = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&conditions);
        let scope = ConnectionScope::new();
        let redraw = self.base.redraw_requested_signal().clone();
        row.filter_changed.connect_scoped(&scope, move |reported| {
            *sink.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
                Some((*reported).clone());
            // A row edit redraws the grid, so the poll that applies it happens next frame rather
            // than on whatever event arrives afterwards.
            redraw.emit();
        });
        self.filter_row = Some(FilterBinding { conditions, _scope: scope });
        // Adopt the row's current state now, so attaching does not wait for the next edit.
        self.apply_filter_row_conditions(row.conditions().to_vec());
    }

    /// Stops following the attached row; the last applied filter stays.
    pub fn detach_filter_row(&mut self) {
        if self.filter_row.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a [`FilterRow`] is attached.
    pub fn is_filter_row_attached(&self) -> bool {
        self.filter_row.is_some()
    }

    /// Applies the condition list the attached row published since the last poll.
    ///
    /// Called from `draw` and `handle_event`, so a row edit is reflected by the next frame or
    /// interaction without the host forwarding it. Returns whether a report was applied.
    pub fn poll_filter_row(&mut self) -> bool {
        let reported = {
            let Some(binding) = self.filter_row.as_ref() else {
                return false;
            };
            binding.conditions.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take()
        };
        match reported {
            Some(conditions) => {
                self.apply_filter_row_conditions(conditions);
                true
            }
            None => false,
        }
    }

    /// Installs `conditions` as one conjunction and refreshes the projection.
    ///
    /// The flat list is mirrored into `filters` so `summary_text` and the `filters` property report
    /// the grid as filtered, then installed as the tree `apply_filter_sort` evaluates. Going
    /// through [`Self::set_filter_expr`] is what invalidates the projection caches and announces
    /// the change on `filter_changed`.
    fn apply_filter_row_conditions(&mut self, conditions: Vec<FilterCondition>) {
        self.filters = conditions
            .iter()
            .map(|condition| ColumnFilter {
                column: condition.column,
                query: condition.operand.clone(),
            })
            .collect();
        self.set_filter_expr(FilterExpr::from_conditions(conditions));
    }

    /// A one-line overview of the grid's data state.
    ///
    /// Row count, selection count and whether a filter is active — the three facts a status strip
    /// below a table needs, written once here rather than re-derived by every host.
    pub fn summary_text(&self) -> String {
        let rows = self.row_count();
        let mut text = format!("{rows} rows");
        let selected = self.selected_indexes().len();
        if selected > 0 {
            text.push_str(&format!(" \u{00b7} {selected} selected"));
        }
        if !self.filters.is_empty() {
            text.push_str(" \u{00b7} filtered");
        }
        text
    }

    // ── ColumnChooser wiring (V-015) ──

    /// Follows `chooser`'s visibility and order signals (V-015).
    ///
    /// A widget cannot hold a borrow of another, so the chooser writes into a shared queue and
    /// [`Self::poll_column_chooser`] drains it from the paint and event paths. Attaching replaces
    /// any previous chooser, so calling this twice leaves one subscription rather than two.
    ///
    /// The column set both controls were configured with is captured at attach time: the chooser
    /// reports persisted tokens, and the grid needs their source indices. A token this grid does
    /// not know is ignored rather than guessed at, so a chooser covering more columns than the
    /// bound source cannot hide an unrelated column.
    pub fn attach_column_chooser(&mut self, chooser: &ColumnChooser) {
        let (token_map, title_map) = self.column_token_maps(chooser);
        let token_map = Arc::new(token_map);
        let title_map = Arc::new(title_map);
        let visibility = Arc::new(Mutex::new(Vec::new()));
        let order = Arc::new(Mutex::new(Vec::new()));
        let scope = ConnectionScope::new();

        let visibility_sink = Arc::clone(&visibility);
        let lookup_tokens = Arc::clone(&token_map);
        let lookup_titles = Arc::clone(&title_map);
        chooser.column_visibility_changed.connect_scoped(&scope, move |change| {
            let (token, visible) = &*change;
            let Some(column) = resolve_column_token(token, &lookup_tokens, &lookup_titles) else {
                return;
            };
            let mut guard = visibility_sink.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.push((column, *visible));
        });

        let order_sink = Arc::clone(&order);
        let lookup_tokens = Arc::clone(&token_map);
        let lookup_titles = Arc::clone(&title_map);
        chooser.column_order_changed.connect_scoped(&scope, move |reported| {
            let mapped: Vec<usize> = reported
                .iter()
                .filter_map(|token| resolve_column_token(token, &lookup_tokens, &lookup_titles))
                .collect();
            let mut guard = order_sink.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = mapped;
        });

        self.column_chooser = Some(ColumnChooserBinding { visibility, order, _scope: scope });
        self.base.request_redraw();
    }

    /// Stops following the attached chooser.
    pub fn detach_column_chooser(&mut self) {
        if self.column_chooser.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a [`ColumnChooser`] is attached.
    pub fn is_column_chooser_attached(&self) -> bool {
        self.column_chooser.is_some()
    }

    /// Applies every chooser change reported since the last poll.
    ///
    /// Called from `draw` and `handle_event`, so a chooser click is reflected by the next frame
    /// without the host forwarding it. Returns whether any grid state changed.
    pub fn poll_column_chooser(&mut self) -> bool {
        let Some(binding) = self.column_chooser.as_ref() else {
            return false;
        };
        let changes: Vec<(usize, bool)> = {
            let mut guard =
                binding.visibility.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            core::mem::take(&mut *guard)
        };
        let reported_order: Vec<usize> =
            binding.order.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();

        let mut changed = false;
        for (column, visible) in changes {
            changed |= self.set_column_hidden(column, !visible);
        }
        if !reported_order.is_empty() && reported_order != self.column_order {
            self.column_order = reported_order;
            self.base.request_redraw();
            changed = true;
        }
        changed
    }

    // ── Companion wiring (COMP-A8) ──

    /// Attaches `bar` to this grid (COMP-A8).
    ///
    /// The strip learns the grid's current row and selection counts immediately and then follows
    /// them: both are facts the grid already maintains, so there is no signal to subscribe to and
    /// no work for the host between frames. [`Self::poll_grid_status_bar`] compares the grid's
    /// counts with the last push and writes only when they move.
    ///
    /// Attaching replaces any previous strip, so calling this twice leaves one link rather than
    /// two. The strip's own `status_clicked` signal is a strip-to-host fact; the grid neither
    /// consumes nor forwards it.
    pub fn attach_grid_status_bar(&mut self, bar: &GridStatusBar) {
        let (counts, redraw) = bar.counts_handle();
        let current = self.grid_status_counts();
        {
            let mut guard = counts.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = current;
        }
        // The strip is a different control, so it needs its own paint request; the grid's redraw
        // does not cover it.
        redraw.emit();
        self.grid_status_bar = Some(GridStatusBarBinding {
            counts,
            redraw,
            pushed: current,
            _scope: ConnectionScope::new(),
        });
        self.base.request_redraw();
    }

    /// Stops following the attached strip; the counts it last showed stay.
    pub fn detach_grid_status_bar(&mut self) {
        if self.grid_status_bar.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a [`GridStatusBar`] is attached.
    pub fn is_grid_status_bar_attached(&self) -> bool {
        self.grid_status_bar.is_some()
    }

    /// Writes the grid's current counts into the attached strip when they moved.
    ///
    /// Called from `draw` and `handle_event`, so a row-count or selection change is reflected by
    /// the next frame or interaction without the host forwarding it. Returns whether a write
    /// happened.
    pub fn poll_grid_status_bar(&mut self) -> bool {
        let current = self.grid_status_counts();
        let Some(binding) = self.grid_status_bar.as_mut() else {
            return false;
        };
        if binding.pushed == current {
            return false;
        }
        binding.pushed = current;
        {
            let mut guard = binding.counts.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = current;
        }
        binding.redraw.emit();
        true
    }

    /// The `(row count, selected count)` pair a [`GridStatusBar`] displays.
    fn grid_status_counts(&self) -> (u64, u64) {
        let rows = self.row_count() as u64;
        let selected = (self.selected_indexes().len() as u64).min(rows);
        (rows, selected)
    }

    /// Attaches `row` to this grid (COMP-A8).
    ///
    /// The grid computes each configured aggregate from its **projected** rows — the rows left by
    /// the current filter, in the current sort order — and writes the result through the row's
    /// shared value cell. The aggregate list is copied from the row at attach time: a widget cannot
    /// hold a borrow of another, so a later [`SummaryRow::set_aggregates`] needs a re-attach, the
    /// same rule `set_pagination` documents for the bar's page size.
    ///
    /// `Sum`, `Min`, `Max`, `Count` and `Average` are computed here. [`ColumnAggregateKind::Custom`]
    /// is deliberately not: it means a host-defined aggregate, and the row's `set_value` remains
    /// the path for that number. Cells are read as display text; a non-numeric cell contributes to
    /// `Count` but not to a numeric aggregate.
    pub fn attach_summary_row(&mut self, row: &SummaryRow) {
        let (values, redraw) = row.values_handle();
        let aggregates = row.aggregates().to_vec();
        self.summary_row = Some(SummaryRowBinding {
            values,
            redraw,
            aggregates,
            key: None,
            _scope: ConnectionScope::new(),
        });
        // Adopt the grid's current numbers now, so attaching does not wait for a frame.
        self.apply_summary_row();
        self.base.request_redraw();
    }

    /// Stops following the attached summary row; the values it last showed stay.
    pub fn detach_summary_row(&mut self) {
        if self.summary_row.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a [`SummaryRow`] is attached.
    pub fn is_summary_row_attached(&self) -> bool {
        self.summary_row.is_some()
    }

    /// Recomputed the attached row's aggregates when the projection moved.
    ///
    /// Called from `draw` and `handle_event`. Returns whether the row was written.
    pub fn poll_summary_row(&mut self) -> bool {
        let Some(binding) = self.summary_row.as_ref() else {
            return false;
        };
        let key = self.summary_projection_key();
        if binding.key.as_ref() == Some(&key) {
            return false;
        }
        self.apply_summary_row();
        true
    }

    /// The projection the attached row's numbers depend on, or a key that never matches when no
    /// row is attached.
    fn summary_projection_key(&self) -> SummaryProjectionKey {
        SummaryProjectionKey {
            revision: self.data_projection_revision(),
            filter: self.filter_expr.clone(),
            sorts: self.sort_specs.clone(),
            aggregates: self
                .summary_row
                .as_ref()
                .map(|binding| binding.aggregates.clone())
                .unwrap_or_default(),
        }
    }

    /// Computes the attached row's aggregates and writes them into its shared cell.
    fn apply_summary_row(&mut self) {
        let Some(binding) = self.summary_row.as_ref() else {
            return;
        };
        let aggregates = binding.aggregates.clone();
        let values = Arc::clone(&binding.values);
        let redraw = binding.redraw.clone();
        let key = SummaryProjectionKey {
            revision: self.data_projection_revision(),
            filter: self.filter_expr.clone(),
            sorts: self.sort_specs.clone(),
            aggregates: aggregates.clone(),
        };
        let computed = self.compute_summary_values(&aggregates);
        {
            let mut guard = values.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = computed;
        }
        redraw.emit();
        if let Some(binding) = self.summary_row.as_mut() {
            binding.key = Some(key);
        }
    }

    /// Aggregates `aggregates` over the projected rows.
    ///
    /// Reads the source one cell at a time through [`IncrementalTableDataSource::data`] in
    /// projected order. The projection (`index_projection`) is already cached by the source
    /// revision, so this walk is what a steady frame avoids rather than what it repeats.
    fn compute_summary_values(&mut self, aggregates: &[ColumnAggregate]) -> Vec<(String, f64)> {
        let Some(source) = self.data_source.clone() else {
            return Vec::new();
        };
        let order = self.index_projection();
        let (tokens, titles) = self.column_token_lookup();
        let mut values = Vec::new();
        for aggregate in aggregates {
            // A custom aggregate is host-defined; the grid has no rule to apply to it.
            if aggregate.kind == ColumnAggregateKind::Custom {
                continue;
            }
            let Some(column) = resolve_column_token(&aggregate.column_token, &tokens, &titles)
            else {
                continue;
            };
            let mut populated = 0usize;
            let mut numeric = 0usize;
            let mut sum = 0.0f64;
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            for &row in order.iter() {
                let Some(cell) = source.data(row, column) else {
                    continue;
                };
                let cell = cell.trim();
                if cell.is_empty() {
                    continue;
                }
                populated += 1;
                let Ok(number) = cell.parse::<f64>() else {
                    continue;
                };
                if !number.is_finite() {
                    continue;
                }
                numeric += 1;
                sum += number;
                min = min.min(number);
                max = max.max(number);
            }
            let value = match aggregate.kind {
                ColumnAggregateKind::Sum => Some(sum),
                ColumnAggregateKind::Min => (numeric > 0).then_some(min),
                ColumnAggregateKind::Max => (numeric > 0).then_some(max),
                ColumnAggregateKind::Count => Some(populated as f64),
                ColumnAggregateKind::Average => (numeric > 0).then_some(sum / numeric as f64),
                ColumnAggregateKind::Custom => None,
            };
            if let Some(value) = value.filter(|value| value.is_finite()) {
                values.push((aggregate.column_token.clone(), value));
            }
        }
        values
    }

    /// The token and title maps built from this grid's header columns and labels.
    ///
    /// Tokens are the published `ColumnSpec::to_token` spelling, which is what a `ColumnChooser`
    /// and this grid's header both report. Titles are kept as a fallback so a token written with a
    /// different width or capability still finds its column.
    fn column_token_lookup(&self) -> (HashMap<String, usize>, HashMap<String, usize>) {
        let mut tokens = HashMap::new();
        let mut titles = HashMap::new();
        for index in 0..self.column_count() {
            if let Some(spec) = self.header_columns.get(index) {
                tokens.insert(spec.to_token(), index);
            }
            let title = self
                .header_columns
                .get(index)
                .map(|spec| spec.title.clone())
                .or_else(|| self.header_labels.get(index).cloned());
            if let Some(title) = title {
                titles.entry(title).or_insert(index);
            }
        }
        (tokens, titles)
    }

    /// Attaches `menu` to this grid (COMP-A8).
    ///
    /// The menu's four request signals are subscribed; each resolved request is parked in a shared
    /// queue and drained by [`Self::poll_header_menu`]. Column tokens are resolved against the
    /// column set present at attach time, so a token this grid does not know is ignored rather than
    /// guessed at (the rule the chooser link documents). `filter_requested` carries only an
    /// operator, so the column and operand come from the menu's shared context at signal time.
    pub fn attach_header_menu(&mut self, menu: &HeaderMenu) {
        let (tokens, titles) = self.column_token_lookup();
        let tokens = Arc::new(tokens);
        let titles = Arc::new(titles);
        let context = menu.context_handle();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let scope = ConnectionScope::new();
        let redraw = self.base.redraw_requested_signal().clone();

        let sink = Arc::clone(&requests);
        let lookup_tokens = Arc::clone(&tokens);
        let lookup_titles = Arc::clone(&titles);
        let request_redraw = redraw.clone();
        menu.sort_requested.connect_scoped(&scope, move |reported| {
            let (token, state) = &*reported;
            let Some(column) = resolve_column_token(token, &lookup_tokens, &lookup_titles) else {
                return;
            };
            sink.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(HeaderMenuRequest::Sort { column, state: *state });
            request_redraw.emit();
        });

        let sink = Arc::clone(&requests);
        let lookup_tokens = Arc::clone(&tokens);
        let lookup_titles = Arc::clone(&titles);
        let filter_context = Arc::clone(&context);
        let request_redraw = redraw.clone();
        menu.filter_requested.connect_scoped(&scope, move |operator| {
            let (token, operand) = {
                let guard = filter_context.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                (guard.0.clone(), guard.1.clone())
            };
            let Some(column) = resolve_column_token(&token, &lookup_tokens, &lookup_titles) else {
                return;
            };
            sink.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(HeaderMenuRequest::Filter { column, operator: *operator, operand });
            request_redraw.emit();
        });

        let sink = Arc::clone(&requests);
        let lookup_tokens = Arc::clone(&tokens);
        let lookup_titles = Arc::clone(&titles);
        let request_redraw = redraw.clone();
        menu.hide_column_requested.connect_scoped(&scope, move |token| {
            let Some(column) = resolve_column_token(token.as_str(), &lookup_tokens, &lookup_titles)
            else {
                return;
            };
            sink.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(HeaderMenuRequest::Hide { column });
            request_redraw.emit();
        });

        let sink = Arc::clone(&requests);
        let request_redraw = redraw;
        menu.column_chooser_requested.connect_scoped(&scope, move |token| {
            let column = resolve_column_token(token.as_str(), &tokens, &titles);
            sink.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(HeaderMenuRequest::Choose { column });
            request_redraw.emit();
        });

        self.header_menu = Some(HeaderMenuBinding { requests, _scope: scope });
        self.base.request_redraw();
    }

    /// Stops following the attached header menu; the state its last requests produced stays.
    pub fn detach_header_menu(&mut self) {
        if self.header_menu.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a [`HeaderMenu`] is attached.
    pub fn is_header_menu_attached(&self) -> bool {
        self.header_menu.is_some()
    }

    /// Drops the queued column-chooser request, returning whether the menu asked for one.
    ///
    /// The grid does not own a chooser, so the intent is left to the host. Both this flag and the
    /// poll's [`HeaderMenuPoll::ColumnChooserRequested`] report it; neither opens a chooser by
    /// itself.
    pub fn take_column_chooser_request(&mut self) -> bool {
        core::mem::take(&mut self.column_chooser_requested)
    }

    /// Applies every header-menu request reported since the last poll (COMP-A8).
    ///
    /// Called from `draw` and `handle_event`, so a menu activation takes effect on the next frame
    /// or interaction. Returns [`HeaderMenuPoll::Idle`] when nothing was queued; a filter operator
    /// the filter path cannot express is reported rather than dropped.
    pub fn poll_header_menu(&mut self) -> HeaderMenuPoll {
        let Some(binding) = self.header_menu.as_ref() else {
            return HeaderMenuPoll::Idle;
        };
        let requests: Vec<HeaderMenuRequest> = {
            let mut guard =
                binding.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            core::mem::take(&mut *guard)
        };
        if requests.is_empty() {
            return HeaderMenuPoll::Idle;
        }
        let mut changed = false;
        let mut unsupported = None;
        let mut chooser = false;
        for request in requests {
            match request {
                HeaderMenuRequest::Sort { column, state } => {
                    changed |= self.apply_header_sort(column, state);
                }
                HeaderMenuRequest::Filter { column, operator, operand } => {
                    if !filter_operator_supported(operator) {
                        unsupported = Some(operator);
                        continue;
                    }
                    changed |= self.apply_header_filter(column, operator, operand);
                }
                HeaderMenuRequest::Hide { column } => {
                    if column < self.column_count() && !self.is_column_hidden(column) {
                        self.set_column_hidden(column, true);
                        changed = true;
                    }
                }
                HeaderMenuRequest::Choose { .. } => {
                    self.column_chooser_requested = true;
                    chooser = true;
                }
            }
        }
        if let Some(operator) = unsupported {
            return HeaderMenuPoll::UnsupportedFilterOperator(operator);
        }
        if changed {
            return HeaderMenuPoll::Applied;
        }
        if chooser {
            return HeaderMenuPoll::ColumnChooserRequested;
        }
        HeaderMenuPoll::Idle
    }

    /// Applies one menu sort request, returning whether the sort order moved.
    fn apply_header_sort(&mut self, column: usize, state: SortState) -> bool {
        if column >= self.column_count() {
            return false;
        }
        match state {
            SortState::None => {
                let before = self.sort_specs.len();
                self.sort_specs.retain(|spec| spec.column != column);
                if self.sort_specs.len() == before {
                    return false;
                }
                self.invalidate_projection_cache();
                self.clear_cache();
                self.base.request_redraw();
                true
            }
            SortState::Asc | SortState::Desc => {
                let descending = state == SortState::Desc;
                let specs = Vec::from([SortSpec { column, descending }]);
                if self.sort_specs == specs {
                    return false;
                }
                self.set_sort_specs(specs);
                // The same fact a header click announces, so a host listening to `sort_changed`
                // sees menu and header sorting through one channel.
                self.sort_changed.emit((column, descending));
                true
            }
        }
    }

    /// Applies one menu filter request, returning whether the filter expression moved.
    ///
    /// The column's previous condition is replaced; conditions on other columns stay, which is
    /// what [`FilterExpr::without_column`] exists for. The flat `filters` mirror is rebuilt from
    /// the resulting tree so `summary_text` and the `filters` property agree with it.
    fn apply_header_filter(
        &mut self,
        column: usize,
        operator: FilterOperator,
        operand: String,
    ) -> bool {
        if column >= self.column_count() {
            return false;
        }
        let condition = FilterCondition::new(column, operator, operand);
        let remaining = self.filter_expr.without_column(column);
        let expr = match remaining {
            FilterExpr::MatchAll => FilterExpr::Predicate(condition),
            remaining => FilterExpr::and(vec![remaining, FilterExpr::Predicate(condition)]),
        };
        if expr == self.filter_expr {
            return false;
        }
        self.filters = expr
            .conditions()
            .into_iter()
            .map(|condition| ColumnFilter {
                column: condition.column,
                query: condition.operand.clone(),
            })
            .collect();
        self.set_filter_expr(expr);
        true
    }
}
