// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! QueryBuilder — edits a recursive [`FilterExpr`] as a list of condition rows.
//!
//! # Why this is a control and not a data type
//!
//! The *model* it produces ([`FilterExpr`]) is not new: it is the same type
//! `DataGrid` evaluates, so a filter set in code and one a user builds are
//! evaluated by one code path (principle #54, and the plan's §4 B5). What is new
//! is the **rendering shell**: rows that can be added, removed, reordered,
//! conjoined with AND/OR and negated.
//!
//! # Why the shell is genuinely new work
//!
//! `ColumnFilter` is flat and single-column
//! (`data_grid.rs:34-39`: `column: usize`, `query: String`). A nested AND/OR tree
//! with per-row operators cannot be expressed in that shape at all, which is why
//! the builder is a separate control rather than a `DataGrid` mode.
//!
//! # Relationship to the tree
//!
//! The builder edits rows grouped into nested scopes ([`QueryBuilderGroup`]). Each scope has
//! its own conjunction and negation, and scopes nest to any depth, so `(a AND b) OR c` is
//! expressible: the two conditions sit in a group whose conjunction is AND, and the group is
//! one operand of the root OR. A caller that already has a deeper `FilterExpr` can load it with
//! [`QueryBuilder::set_query`], which rebuilds the groups.

//! # Reachability
//!
//! Registered in the widget factory as `query_builder` (aliases `querybuilder`,
//! `filter_builder`), so it is reachable by name from the declarative JSON path
//! (`"querybuilder"`), from a CSS selector (`QueryBuilder`), and through the typed
//! `create_query_builder` on `ControlBackend`.

use core::fmt::Write as _;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::view_widgets::filter_expr::{FilterCondition, FilterExpr, FilterOperator};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// How the builder's rows are combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterConjunction {
    /// Every row must accept (the default, and what a "filter" usually means).
    #[default]
    And,
    /// At least one row must accept.
    Or,
}

impl FilterConjunction {
    /// The token this conjunction is spelled as in JSON and in a saved query.
    pub fn as_str(self) -> &'static str {
        match self {
            FilterConjunction::And => "and",
            FilterConjunction::Or => "or",
        }
    }

    /// Parses the token back, rejecting anything else.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "and" => FilterConjunction::And,
            "or" => FilterConjunction::Or,
            _ => return None,
        })
    }

    /// Returns the other conjunction, for a toggle.
    pub fn toggled(self) -> Self {
        match self {
            FilterConjunction::And => FilterConjunction::Or,
            FilterConjunction::Or => FilterConjunction::And,
        }
    }
}

/// A field the builder offers in its column picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterField {
    /// Zero-based source column index this field maps to.
    pub column: usize,
    /// Display name shown in the picker.
    pub label: String,
    /// Operators this field offers, in menu order.
    ///
    /// Per-field rather than global because a column's type is known by whoever
    /// declares the field: a numeric field should offer `>`, a text field should
    /// not. A field with an empty list offers
    /// [`FilterOperator::Contains`] alone, which is the universal default.
    pub operators: Vec<FilterOperator>,
}

impl FilterField {
    /// Creates a text field offering the textual operators.
    pub fn text(column: usize, label: impl Into<String>) -> Self {
        Self {
            column,
            label: label.into(),
            operators: vec![
                FilterOperator::Contains,
                FilterOperator::Equals,
                FilterOperator::StartsWith,
                FilterOperator::EndsWith,
                FilterOperator::NotContains,
            ],
        }
    }

    /// Creates a numeric field offering the numeric operators.
    pub fn number(column: usize, label: impl Into<String>) -> Self {
        Self {
            column,
            label: label.into(),
            operators: vec![
                FilterOperator::EqualsNumber,
                FilterOperator::GreaterThan,
                FilterOperator::GreaterOrEqual,
                FilterOperator::LessThan,
                FilterOperator::LessOrEqual,
            ],
        }
    }

    /// Creates a field with an explicit operator list.
    pub fn new(column: usize, label: impl Into<String>, operators: Vec<FilterOperator>) -> Self {
        Self { column, label: label.into(), operators }
    }

    /// The operator a new row on this field starts with.
    pub fn default_operator(&self) -> FilterOperator {
        self.operators.first().copied().unwrap_or(FilterOperator::Contains)
    }
}

/// One editable row of the builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryBuilderRow {
    /// Index into the builder's field list.
    pub field: usize,
    /// The operator this row compares with.
    pub operator: FilterOperator,
    /// The operand the user typed.
    pub operand: String,
    /// Whether this row is negated.
    ///
    /// Per-row rather than per-group: "field is **not** empty" is the negation
    /// people actually reach for, and a group-level `Not` would invert the whole
    /// conjunction instead.
    pub negated: bool,
    /// The group this row belongs to, or `None` for the root scope.
    pub group: Option<usize>,
}

impl QueryBuilderRow {
    /// Creates a row on `field` with that field's default operator and no operand.
    pub fn new(field: usize, operator: FilterOperator) -> Self {
        Self { field, operator, operand: String::new(), negated: false, group: None }
    }
}

/// A nested scope inside the builder.
///
/// A group is a set of rows and child groups combined with their own conjunction, and
/// optionally negated. Groups may nest: `parent` names another group, or `None` for the
/// root scope. That is what lets the builder express `(a AND b) OR c`, which a single
/// flat conjunction cannot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryBuilderGroup {
    /// The group's identifier, stable for the lifetime of the builder.
    pub id: usize,
    /// The enclosing group, or `None` when the group sits at the root.
    pub parent: Option<usize>,
    /// How the group's members are combined.
    pub conjunction: FilterConjunction,
    /// Whether the whole group is negated.
    pub negated: bool,
}

/// The height of one condition row, and the header strip above them.
const ROW_HEIGHT: u32 = 30;
const HEADER_HEIGHT: u32 = 34;

/// QueryBuilder — edits a recursive filter as a list of rows.
pub struct QueryBuilder {
    base: BaseWidget,
    /// The fields the column picker offers.
    fields: Vec<FilterField>,
    /// The condition rows, in display order.
    rows: Vec<QueryBuilderRow>,
    /// How the rows are combined.
    conjunction: FilterConjunction,
    /// Which row is focused for editing, if any.
    active_row: Option<usize>,
    /// The row the pointer is over, or `None`.
    ///
    /// Indexed like `active_row`, and kept honest across `remove_row` for the
    /// same reason: a transient highlight must not name a row that is gone.
    hovered_row: Option<usize>,
    /// Emitted whenever the produced expression changes, with the new expression.
    pub query_changed: Signal1<FilterExpr>,
    /// Emitted with the new row list whenever rows are added, removed or reordered.
    pub rows_changed: Signal1<Vec<QueryBuilderRow>>,
    /// The first visible row.
    scroll_offset: usize,
    /// The row being dragged for a reorder.
    drag_row: Option<usize>,
    /// The nested scopes, in creation order.
    groups: Vec<QueryBuilderGroup>,
    /// The next group identifier.
    next_group_id: usize,
}

impl QueryBuilder {
    /// Creates a builder with no fields and no rows.
    ///
    /// Defaults: conjunction AND, no active row.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::QueryBuilder, geometry, "QueryBuilder"),
            fields: Vec::new(),
            rows: Vec::new(),
            conjunction: FilterConjunction::And,
            active_row: None,
            hovered_row: None,
            query_changed: Signal1::new(),
            rows_changed: Signal1::new(),
            scroll_offset: 0,
            drag_row: None,
            groups: Vec::new(),
            next_group_id: 0,
        }
    }

    /// Replaces the fields the column picker offers.
    ///
    /// Rows whose field index no longer exists are dropped: an index into a field
    /// list that shrank would point at a different column, and silently filtering on
    /// the wrong column is worse than losing the row.
    pub fn set_fields(&mut self, fields: Vec<FilterField>) {
        self.fields = fields;
        let field_count = self.fields.len();
        self.rows.retain(|row| row.field < field_count);
        self.base.request_redraw();
    }

    /// Returns the fields.
    pub fn fields(&self) -> &[FilterField] {
        &self.fields
    }

    /// Returns the condition rows.
    pub fn rows(&self) -> &[QueryBuilderRow] {
        &self.rows
    }

    /// The number of condition rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Appends a row on `field`, returning its index.
    ///
    /// Returns `None` when `field` names no field, so a caller cannot add a row that
    /// filters on nothing.
    pub fn add_row(&mut self, field: usize) -> Option<usize> {
        let default_operator = self.fields.get(field)?.default_operator();
        let index = self.rows.len();
        self.rows.push(QueryBuilderRow::new(field, default_operator));
        self.notify_rows_changed();
        // A new row becomes the active one, because the user's next act is to type
        // its operand.
        self.active_row = Some(index);
        self.emit_query();
        Some(index)
    }

    /// Appends a row inside `group`, returning its index.
    ///
    /// A group that does not exist is rejected; the row is not silently moved to the root.
    pub fn add_row_in_group(&mut self, field: usize, group: usize) -> Option<usize> {
        if !self.groups.iter().any(|candidate| candidate.id == group) {
            return None;
        }
        let index = self.add_row(field)?;
        if let Some(row) = self.rows.get_mut(index) {
            row.group = Some(group);
        }
        self.notify_rows_changed();
        Some(index)
    }

    /// Creates a nested scope, returning its identifier.
    ///
    /// A parent that does not exist is refused with `None`: the new group would otherwise be
    /// unreachable from the root.
    pub fn add_group(&mut self, parent: Option<usize>) -> Option<usize> {
        if parent.is_some_and(|parent| !self.groups.iter().any(|group| group.id == parent)) {
            return None;
        }
        let id = self.next_group_id;
        self.next_group_id = self.next_group_id.saturating_add(1);
        self.groups.push(QueryBuilderGroup {
            id,
            parent,
            conjunction: FilterConjunction::And,
            negated: false,
        });
        self.emit_query();
        Some(id)
    }

    /// The nested scopes, in creation order.
    pub fn groups(&self) -> &[QueryBuilderGroup] {
        &self.groups
    }

    /// Sets how a group's members are combined.
    pub fn set_group_conjunction(&mut self, id: usize, conjunction: FilterConjunction) -> bool {
        let Some(group) = self.groups.iter_mut().find(|group| group.id == id) else {
            return false;
        };
        if group.conjunction == conjunction {
            return true;
        }
        group.conjunction = conjunction;
        self.emit_query();
        true
    }

    /// Sets whether a group is negated.
    pub fn set_group_negated(&mut self, id: usize, negated: bool) -> bool {
        let Some(group) = self.groups.iter_mut().find(|group| group.id == id) else {
            return false;
        };
        if group.negated == negated {
            return true;
        }
        group.negated = negated;
        self.emit_query();
        true
    }

    /// Removes a group, its child groups and its rows.
    pub fn remove_group(&mut self, id: usize) -> bool {
        if !self.groups.iter().any(|group| group.id == id) {
            return false;
        }
        // The descendant identifiers are collected **before** the removal, because a row in a
        // child group can only be recognised while that child still exists.
        let removed: Vec<usize> = self
            .groups
            .iter()
            .filter(|group| Self::group_is_within(group, id, &self.groups))
            .map(|group| group.id)
            .collect();
        self.groups.retain(|group| !removed.contains(&group.id));
        self.rows.retain(|row| !row.group.is_some_and(|group| removed.contains(&group)));
        self.active_row = None;
        self.notify_rows_changed();
        self.emit_query();
        true
    }

    /// Whether `group` is `ancestor` or nested inside it.
    fn group_is_within(
        group: &QueryBuilderGroup,
        ancestor: usize,
        all: &[QueryBuilderGroup],
    ) -> bool {
        let mut current = Some(group.id);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = all
                .iter()
                .find(|candidate| candidate.id == id)
                .and_then(|candidate| candidate.parent);
        }
        false
    }

    /// Moves `row` into `group`, or to the root when `group` is `None`.
    pub fn set_row_group(&mut self, row: usize, group: Option<usize>) -> bool {
        if group.is_some_and(|group| !self.groups.iter().any(|candidate| candidate.id == group)) {
            return false;
        }
        let Some(entry) = self.rows.get_mut(row) else {
            return false;
        };
        if entry.group == group {
            return true;
        }
        entry.group = group;
        self.notify_rows_changed();
        self.emit_query();
        true
    }

    /// How deeply nested the row's group is, `0` for a root row.
    fn row_group_depth(&self, row: usize) -> usize {
        let mut depth = 0;
        let mut current = self.rows.get(row).and_then(|row| row.group);
        while let Some(id) = current {
            depth += 1;
            current =
                self.groups.iter().find(|group| group.id == id).and_then(|group| group.parent);
        }
        depth
    }

    /// Removes the row at `index`, returning it.
    ///
    /// The active row is cleared or shifted so it never names a row that is gone.
    pub fn remove_row(&mut self, index: usize) -> Option<QueryBuilderRow> {
        if index >= self.rows.len() {
            return None;
        }
        let removed = self.rows.remove(index);
        self.notify_rows_changed();
        self.active_row = match self.active_row {
            Some(active) if active == index => None,
            Some(active) if active > index => Some(active - 1),
            other => other,
        };
        // The hover follows its row the same way, so the highlight cannot land on
        // a different condition than the one the pointer was over.
        self.hovered_row = match self.hovered_row {
            Some(hovered) if hovered == index => None,
            Some(hovered) if hovered > index => Some(hovered - 1),
            other => other,
        };
        self.emit_query();
        Some(removed)
    }

    /// Swaps the rows at `a` and `b`, so a user can reorder conditions.
    pub fn swap_rows(&mut self, a: usize, b: usize) -> bool {
        if a >= self.rows.len() || b >= self.rows.len() || a == b {
            return false;
        }
        self.rows.swap(a, b);
        self.notify_rows_changed();
        self.emit_query();
        true
    }

    /// Sets a row's operand.
    ///
    /// Returns `false` when the index names no row.
    pub fn set_row_operand(&mut self, index: usize, operand: impl Into<String>) -> bool {
        match self.rows.get_mut(index) {
            Some(row) => {
                row.operand = operand.into();
                self.emit_query();
                true
            }
            None => false,
        }
    }

    /// Sets a row's operator, refusing one the row's field does not offer.
    ///
    /// A field declares its operators so the menu cannot offer something absurd
    /// (`starts_with` on a number). Applying the check here as well as in the UI
    /// keeps a programmatic caller from producing a row the UI would not.
    pub fn set_row_operator(&mut self, index: usize, operator: FilterOperator) -> bool {
        let Some(row) = self.rows.get(index) else {
            return false;
        };
        let field_index = row.field;
        let offered =
            self.fields.get(field_index).is_none_or(|field| field.operators.contains(&operator));
        if !offered {
            return false;
        }
        if let Some(row) = self.rows.get_mut(index) {
            row.operator = operator;
        }
        self.emit_query();
        true
    }

    /// Negates or un-negates a row.
    pub fn set_row_negated(&mut self, index: usize, negated: bool) -> bool {
        match self.rows.get_mut(index) {
            Some(row) => {
                row.negated = negated;
                self.emit_query();
                true
            }
            None => false,
        }
    }

    /// Returns the conjunction the rows are combined with.
    pub fn conjunction(&self) -> FilterConjunction {
        self.conjunction
    }

    /// Sets the conjunction, emitting the new query.
    pub fn set_conjunction(&mut self, conjunction: FilterConjunction) {
        if self.conjunction == conjunction {
            return;
        }
        self.conjunction = conjunction;
        self.emit_query();
    }

    /// Swaps AND for OR and back.
    pub fn toggle_conjunction(&mut self) {
        self.set_conjunction(self.conjunction.toggled());
    }

    /// The active row index, if one is being edited.
    pub fn active_row(&self) -> Option<usize> {
        self.active_row
    }

    /// Sets the active row, ignoring an index that names no row.
    pub fn set_active_row(&mut self, index: usize) {
        if index < self.rows.len() {
            if self.active_row == Some(index) {
                return;
            }
            self.active_row = Some(index);
            self.base.request_redraw();
        }
    }

    /// Clears the focused row, so no row is being edited.
    ///
    /// The `None` half of the property round-trip: the getter answers `Null` when
    /// nothing is active, so the setter must accept `Null` to clear it (the same
    /// contract `tree_view`'s `focused_node` publishes).
    pub fn clear_active_row(&mut self) {
        if self.active_row.is_none() {
            return;
        }
        self.active_row = None;
        self.base.request_redraw();
    }

    /// The rows whose operand is empty, which are not yet part of the query.
    ///
    /// A row with no operand is *incomplete*: a user who has chosen a field but not
    /// typed yet must not have every row filtered away in the meantime. Exposed so a
    /// caller can show which rows still need input.
    pub fn incomplete_rows(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.operand.is_empty())
            .map(|(index, _)| index)
            .collect()
    }

    /// Builds the filter expression the current rows describe.
    ///
    /// Rows with an empty operand are skipped, so the query is always an expression
    /// the grid can evaluate — and so a half-typed row does not blank the grid.
    /// A field index that no longer exists is skipped for the same reason.
    pub fn build_query(&self) -> FilterExpr {
        self.build_scope(None)
    }

    /// Builds the expression for one scope: the rows that belong to it plus its child groups.
    fn build_scope(&self, parent: Option<usize>) -> FilterExpr {
        let mut children = Vec::new();
        for row in &self.rows {
            if row.group != parent || row.operand.is_empty() {
                continue;
            }
            let Some(field) = self.fields.get(row.field) else {
                continue;
            };
            let condition = FilterCondition::new(field.column, row.operator, row.operand.clone());
            let predicate = FilterExpr::Predicate(condition);
            children.push(if row.negated { FilterExpr::negate(predicate) } else { predicate });
        }
        // Child groups follow the rows of the same scope. A group is a single operand of the
        // enclosing conjunction, which is exactly what makes `(a AND b) OR c` expressible.
        for group in self.groups.iter().filter(|group| group.parent == parent) {
            let inner = self.build_scope(Some(group.id));
            children.push(if group.negated { FilterExpr::negate(inner) } else { inner });
        }
        let conjunction = match parent {
            None => self.conjunction,
            Some(id) => self
                .groups
                .iter()
                .find(|group| group.id == id)
                .map(|group| group.conjunction)
                .unwrap_or(FilterConjunction::And),
        };
        match conjunction {
            FilterConjunction::And => FilterExpr::and(children),
            FilterConjunction::Or => FilterExpr::or(children),
        }
    }

    /// Loads rows and groups from an expression, replacing the current ones.
    ///
    /// The load is structural: a nested `And`/`Or` becomes a [`QueryBuilderGroup`] whose
    /// conjunction differs from its parent's, and a negated group keeps its negation. A
    /// conjunction that matches the enclosing scope is flattened into that scope, which is
    /// semantics-preserving. Conditions on a column with no declared field cannot be shown;
    /// the load still succeeds for the rest and reports `false`.
    ///
    /// Returns `false` when some part of the expression could not be represented.
    pub fn set_query(&mut self, expr: &FilterExpr) -> bool {
        self.rows.clear();
        self.groups.clear();
        self.next_group_id = 0;
        self.active_row = None;
        self.hovered_row = None;
        self.scroll_offset = 0;
        self.conjunction = FilterConjunction::And;
        let exact = self.load_scope(expr, None, true);
        self.notify_rows_changed();
        self.emit_query();
        exact
    }

    /// Loads one expression into `parent`'s scope.
    ///
    /// `outer` marks the root call, which is allowed to set the root conjunction; a nested
    /// branch of a different conjunction becomes a group instead.
    fn load_scope(&mut self, expr: &FilterExpr, parent: Option<usize>, outer: bool) -> bool {
        match expr {
            FilterExpr::MatchAll | FilterExpr::MatchNothing => true,
            FilterExpr::Predicate(_) => self.push_row_from_expr(expr, parent),
            FilterExpr::Not(inner) => match inner.as_ref() {
                FilterExpr::Predicate(_) => self.push_row_from_expr(expr, parent),
                other => {
                    let id = self.new_group(parent, FilterConjunction::And, true);
                    self.load_scope(other, Some(id), false)
                }
            },
            FilterExpr::And(children) => {
                self.load_branch(children, parent, FilterConjunction::And, outer)
            }
            FilterExpr::Or(children) => {
                self.load_branch(children, parent, FilterConjunction::Or, outer)
            }
        }
    }

    /// Loads a conjunction's children into a scope, creating a subgroup when the
    /// conjunction differs from the enclosing scope's.
    fn load_branch(
        &mut self,
        children: &[FilterExpr],
        parent: Option<usize>,
        conjunction: FilterConjunction,
        outer: bool,
    ) -> bool {
        let target = match parent {
            None if outer => {
                self.conjunction = conjunction;
                None
            }
            None if self.conjunction == conjunction => None,
            None => Some(self.new_group(None, conjunction, false)),
            Some(id) if self.group_conjunction(id) == conjunction => Some(id),
            Some(id) => Some(self.new_group(Some(id), conjunction, false)),
        };
        let mut exact = true;
        for child in children {
            exact &= self.load_scope(child, target, false);
        }
        exact
    }

    /// Appends the row an expression denotes, inside `parent`.
    fn push_row_from_expr(&mut self, expr: &FilterExpr, parent: Option<usize>) -> bool {
        match Self::row_from_expr(expr, &self.fields) {
            Some(mut row) => {
                row.group = parent;
                self.rows.push(row);
                true
            }
            None => false,
        }
    }

    /// A group's conjunction, defaulting to `And` for an unknown id.
    fn group_conjunction(&self, id: usize) -> FilterConjunction {
        self.groups
            .iter()
            .find(|group| group.id == id)
            .map(|group| group.conjunction)
            .unwrap_or(FilterConjunction::And)
    }

    /// Creates a group without emitting; used while loading an expression.
    fn new_group(
        &mut self,
        parent: Option<usize>,
        conjunction: FilterConjunction,
        negated: bool,
    ) -> usize {
        let id = self.next_group_id;
        self.next_group_id = self.next_group_id.saturating_add(1);
        self.groups.push(QueryBuilderGroup { id, parent, conjunction, negated });
        id
    }

    /// Converts one predicate (optionally negated) into a row.
    ///
    /// Returns `None` for a nested group: groups are loaded by [`Self::load_scope`] into
    /// [`QueryBuilderGroup`] entries, not flattened into rows.
    fn row_from_expr(expr: &FilterExpr, fields: &[FilterField]) -> Option<QueryBuilderRow> {
        let (condition, negated) = match expr {
            FilterExpr::Predicate(condition) => (condition, false),
            FilterExpr::Not(inner) => match inner.as_ref() {
                FilterExpr::Predicate(condition) => (condition, true),
                _ => return None,
            },
            _ => return None,
        };
        // Find the field whose source column this condition names. A condition on a
        // column with no field cannot be shown, and inventing a field would make the
        // builder offer something the caller never declared.
        let field = fields.iter().position(|field| field.column == condition.column)?;
        Some(QueryBuilderRow {
            field,
            operator: condition.operator,
            operand: condition.operand.clone(),
            negated,
            group: None,
        })
    }

    /// Emits the current query.
    fn emit_query(&mut self) {
        let query = self.build_query();
        if self.query_changed.slot_count() > 0 {
            self.query_changed.emit(query);
        }
        self.base.request_redraw();
    }

    /// The rectangle of the row at `index`.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.rows.len() || index < self.scroll_offset {
            return None;
        }
        let rect = self.geometry();
        Some(Rect::new(
            rect.x,
            rect.y
                + HEADER_HEIGHT as i32
                + ((index - self.scroll_offset) as i32) * ROW_HEIGHT as i32,
            rect.width,
            ROW_HEIGHT,
        ))
    }

    /// Rows that fit below the header.
    fn visible_row_count(&self) -> usize {
        let rect = self.geometry();
        (rect.height.saturating_sub(HEADER_HEIGHT) / ROW_HEIGHT).max(1) as usize
    }

    /// The first visible row.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Sets the first visible row, clamped to the row list.
    pub fn set_scroll_offset(&mut self, offset: usize) {
        let max = self.rows.len().saturating_sub(self.visible_row_count());
        let next = offset.min(max);
        if next == self.scroll_offset {
            return;
        }
        self.scroll_offset = next;
        self.base.request_redraw();
    }

    fn notify_rows_changed(&self) {
        if self.rows_changed.slot_count() > 0 {
            self.rows_changed.emit(self.rows.clone());
        }
    }

    /// The row under `pos`, when the pointer is over one.
    fn row_at(&self, pos: Point) -> Option<usize> {
        (0..self.rows.len())
            .find(|index| self.row_rect(*index).is_some_and(|rect| rect.contains_point(pos)))
    }
}

impl Widget for QueryBuilder {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(360, 120)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "query_changed" => {
                Some(EventSignalRef::mapped("query_changed", &self.query_changed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            _ => None,
        }
    }
}

/// `QueryBuilder`'s property contract.
///
/// The fields and rows are lists, written through `set_fields` / `add_row` and the
/// per-row setters; the property layer reports the derived counts and the
/// conjunction, following the same convention as the other list-valued controls.
impl WidgetProperties for QueryBuilder {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "incomplete_row_count" => {
                Ok(CapabilityValue::UInt(self.incomplete_rows().len() as u64))
            }
            "field_count" => Ok(CapabilityValue::UInt(self.fields().len() as u64)),
            "conjunction" => Ok(CapabilityValue::String(self.conjunction().as_str().to_string())),
            "active_row" => Ok(match self.active_row() {
                Some(index) => CapabilityValue::UInt(index as u64),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "conjunction" => {
                let token = expect_string(value)?;
                let Some(conjunction) = FilterConjunction::from_name(&token) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_conjunction(conjunction);
                Ok(())
            }
            "active_row" => match value {
                CapabilityValue::Null => {
                    self.clear_active_row();
                    Ok(())
                }
                other => {
                    self.set_active_row(expect_usize(other)?);
                    Ok(())
                }
            },
            // Derived from the row list, which is written through `add_row` /
            // `remove_row` / `set_row_*`.
            "row_count" | "incomplete_row_count" | "field_count" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `QUERY_BUILDER_PROPERTIES`.
        property_names_of![
            "row_count",
            "incomplete_row_count",
            "field_count",
            "conjunction",
            "active_row",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `query_builder` publishes.
    ///
    /// All four mutate a row and need the caller to name it: `add_row` takes the
    /// field, `remove_row` the index, and `toggle_conjunction` the conjunction to
    /// switch to (the widget has `set_conjunction`, but a bare call cannot guess
    /// which of the two the caller means). `set_fields` similarly takes the
    /// fields list, so every name here is answered as needing a payload rather
    /// than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_fields" | "add_row" | "remove_row" | "toggle_conjunction" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for QueryBuilder {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("query_builder");
        // `query_builder` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — the window's own
        // colour. The builder is therefore a step toward the foreground, so it reads
        // as a panel rather than as bare window.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(resolved));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = resolved.blend(&text_color, 0.08);

        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        let chrome = QueryBuilderChrome { background, border, text_color };
        self.draw_header(context, rect, &chrome);
        let fonts = QueryBuilderFonts::new();
        let bottom = rect.y + rect.height as i32;
        for index in self.scroll_offset..self.rows.len() {
            let Some(row_rect) = self.row_rect(index) else {
                break;
            };
            // Rows past the control's bottom are not painted: the loop used to run for every
            // condition in the model, allocating a font per row even for invisible ones.
            if row_rect.y >= bottom {
                break;
            }
            self.draw_row(context, index, &chrome, &fonts);
        }
        if self.rows.is_empty() {
            context.draw_text(
                Point::new(rect.x + 10, rect.y + HEADER_HEIGHT as i32 + 20),
                "No conditions",
                fonts.small,
                chrome.placeholder_text(),
                HorizontalAlignment::Left,
            );
        }
    }
}

/// The builder's fonts, built once.
///
/// `Font::simple` allocates its family `String`, and `draw_row` built four of them per row per
/// frame (marker, field, operator, operand). The shared faces are now initialised once for
/// the process — the three roles are fixed (family and size are literals), so a static per
/// role is the whole cache.
struct QueryBuilderFonts {
    body: &'static Font,
    small: &'static Font,
    tiny: &'static Font,
}

impl QueryBuilderFonts {
    fn new() -> Self {
        static BODY: crate::compat::OnceLock<Font> = crate::compat::OnceLock::new();
        static SMALL: crate::compat::OnceLock<Font> = crate::compat::OnceLock::new();
        static TINY: crate::compat::OnceLock<Font> = crate::compat::OnceLock::new();
        Self {
            body: BODY.get_or_init(|| Font::simple("Sans", 13.0)),
            small: SMALL.get_or_init(|| Font::simple("Sans", 11.0)),
            tiny: TINY.get_or_init(|| Font::simple("Sans", 10.0)),
        }
    }
}

/// The builder's resolved chrome colours, threaded through the drawing helpers.
///
/// Gathered into one value so the header, the rows and the operand field all read
/// the same appearance-resolved colours instead of re-resolving the theme, and so
/// no helper can drift back to a literal.
#[derive(Debug, Clone, Copy)]
struct QueryBuilderChrome {
    /// The builder's own fill, and the base every other colour is derived from.
    background: Color,
    /// The resolved border colour, reused as the input-box outline.
    border: Color,
    /// The resolved foreground, used for text and as the tint direction.
    text_color: Color,
}

impl QueryBuilderChrome {
    /// Ordinary row and header text.
    fn text(&self) -> Color {
        self.text_color
    }

    /// Placeholder and secondary labels.
    /// The builder's de-emphasised ink: dimmed toward the surface, then held to the text floor.
    ///
    /// The fixed 45% blend measured 3.90:1 on the light surface and 4.04:1 on the dark one. It
    /// carries the empty-state hint *and* the live condition count, both of which are read as
    /// text — an empty builder has nothing else on screen to explain itself — so "secondary"
    /// must not mean "below the floor".
    fn placeholder_text(&self) -> Color {
        self.text_color.blend(&self.background, 0.45).legible_on(self.background, 4.5)
    }

    /// The header strip, a step away from the builder body.
    fn header(&self) -> Color {
        self.background.blend(&self.text_color, 0.06)
    }

    /// The row the user is editing.
    fn active_row(&self) -> Color {
        self.background.blend(&self.text_color, 0.08)
    }

    /// The row the pointer is over. Pointer position, not a committed edit, so it
    /// reads the accent slot at the family's lighter hover weight — the same
    /// surface/accent pair the table family uses for its row tint (the accent is
    /// reached through the resolved border, as the AND chip does).
    fn hovered_row(&self) -> Color {
        self.background.blend(&self.border, 0.12)
    }

    /// The row separator lines.
    fn separator(&self) -> Color {
        self.background.blend(&self.text_color, 0.1)
    }

    /// An operand field's interior: one step toward the text colour from the body,
    /// so an editable box reads as raised in both appearances.
    fn field(&self) -> Color {
        self.background.blend(&self.text_color, 0.04)
    }

    /// The add button's surface, a stronger step than a field so it reads as an
    /// affordance rather than an input.
    fn button(&self) -> Color {
        self.background.blend(&self.text_color, 0.14)
    }

    /// The AND conjunction chip. The theme's primary slot, read through the
    /// resolved border colour so it moves with the appearance instead of staying
    /// a fixed blue.
    fn and_chip(&self) -> Color {
        self.border.blend(&self.text_color, 0.3)
    }

    /// The OR conjunction chip. A distinct hue from the AND chip, taken from the
    /// theme's accent slot through the semantic warning token so the two
    /// conjunctions stay tellable apart in both appearances.
    fn or_chip(&self) -> Color {
        crate::style::semantic_color(crate::style::SemanticColor::Warning)
            .map(|token| token.blend(&self.background, 0.15))
            .unwrap_or_else(|| self.border.blend(&self.background, 0.4))
    }

    /// The `!` negation marker and the remove cross: states the user acts on, so
    /// they read the theme's error token rather than a literal red or grey.
    fn remove(&self) -> Color {
        crate::style::semantic_color(crate::style::SemanticColor::Error)
            .map(|token| token.blend(&self.background, 0.3))
            .unwrap_or_else(|| self.text_color.blend(&self.background, 0.3))
    }
}

impl QueryBuilder {
    /// Draws the conjunction toggle and the add button.
    fn draw_header(&self, context: &mut RenderContext, rect: Rect, chrome: &QueryBuilderChrome) {
        let header = Rect::new(rect.x, rect.y, rect.width, HEADER_HEIGHT);
        context.fill_rect(header, chrome.header());

        // The conjunction is a visible chip rather than a menu, because it is the
        // one decision that changes what *every* row means.
        let chip = Rect::new(rect.x + 8, rect.y + 7, 56, 20);
        let chip_color = match self.conjunction {
            FilterConjunction::And => chrome.and_chip(),
            FilterConjunction::Or => chrome.or_chip(),
        };
        context.fill_rounded_rect(chip, 10, chip_color);
        // Each label is centred on its own box by half the line box. The fixed offsets
        // (`chip.y + 14`, `rect.y + 21`, `add.y + 14`) were written for one font: the origin is
        // the glyph's **top** edge, so an 11 px chip label spanned 21..32 in a 20 px chip and
        // ran five pixels past it, and the 14 px `+` spanned 21..35 in the same 20 px button.
        // The chip's face is fixed text, so it is initialised once for the process
        // instead of allocating its family string on every paint.
        static CHIP_FONT: crate::compat::OnceLock<Font> = crate::compat::OnceLock::new();
        let chip_font = CHIP_FONT.get_or_init(|| Font::simple("Sans", 11.0));
        let chip_text_h = context.measure_text("M", chip_font).height;
        context.draw_text_fitted_cow(
            Rect::new(
                chip.x + 6,
                chip.y + (chip.height as i32 - chip_text_h as i32) / 2,
                chip.width.saturating_sub(12),
                chip_text_h,
            ),
            // A static upper-case spelling: `to_uppercase()` allocated a `String` on every
            // repaint to label a two-valued chip.
            match self.conjunction {
                FilterConjunction::And => "AND",
                FilterConjunction::Or => "OR",
            },
            chip_font,
            // The label sits on the chip, so it takes the chip's own contrast
            // colour rather than a literal white.
            chip_color.contrast_color(),
            HorizontalAlignment::Center,
        );

        // Rendered in place: the count is drawn once per frame and never leaves this
        // scope, so it writes into the buffer rather than through `format!`'s copy.
        let mut count_text = String::new();
        let _ = write!(count_text, "{} condition(s)", self.rows.len());
        context.draw_text_fitted_cow(
            Rect::new(
                chip.x + chip.width as i32 + 10,
                rect.y + (HEADER_HEIGHT as i32 - chip_text_h as i32) / 2,
                (rect.width as i32 - (chip.x + chip.width as i32 + 10) - 38).max(1) as u32,
                chip_text_h,
            ),
            count_text.as_str(),
            chip_font,
            chrome.placeholder_text(),
            HorizontalAlignment::Left,
        );

        // The add button, at the right.
        let add = Rect::new(rect.x + rect.width as i32 - 30, rect.y + 7, 22, 20);
        context.fill_rounded_rect(add, 4, chrome.button());
        static ADD_FONT: crate::compat::OnceLock<Font> = crate::compat::OnceLock::new();
        let add_font = ADD_FONT.get_or_init(|| Font::simple("Sans", 14.0));
        let add_text_h = context.measure_text("+", add_font).height;
        context.draw_text_fitted_cow(
            Rect::new(
                add.x,
                add.y + (add.height as i32 - add_text_h as i32) / 2,
                add.width,
                add_text_h,
            ),
            "+",
            add_font,
            chrome.text(),
            HorizontalAlignment::Center,
        );
    }

    /// Draws one condition row: negation, field, operator, operand, remove.
    fn draw_row(
        &self,
        context: &mut RenderContext,
        index: usize,
        chrome: &QueryBuilderChrome,
        fonts: &QueryBuilderFonts,
    ) {
        let Some(row_rect) = self.row_rect(index) else {
            return;
        };
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let active = self.active_row == Some(index);
        if active {
            context.fill_rect(row_rect, chrome.active_row());
        } else if self.hovered_row == Some(index) {
            context.fill_rect(row_rect, chrome.hovered_row());
        }
        context.draw_line_stroke(
            Point::new(row_rect.x, row_rect.y),
            Point::new(row_rect.x + row_rect.width as i32, row_rect.y),
            chrome.separator(),
            1,
        );

        let mut x = row_rect.x + 8;
        // A row inside a nested scope is drawn with a guide bar and an indent, so the scope's
        // boundary is visible without a synthetic header row. Root rows (the default) are
        // laid out exactly as before.
        let depth = self.row_group_depth(index);
        if depth > 0 {
            let guide_x = row_rect.x + 4 + (depth.saturating_sub(1)) as i32 * 10;
            context
                .fill_rect(Rect::new(guide_x, row_rect.y, 1, row_rect.height), chrome.separator());
            x += depth as i32 * 10;
        }
        // Not-negation marker: a `!` shown only when set, so an un-negated row is not
        // cluttered by a control the user rarely wants.
        if row.negated {
            context.draw_text(
                Point::new(x, row_rect.y + 19),
                "!",
                fonts.body,
                chrome.remove(),
                HorizontalAlignment::Left,
            );
        }
        x += 16;

        // Field name. The label is host-supplied, so it is fitted to the column it owns rather
        // than placed and left to run: `x` is the column's leading edge and the column is 84 px
        // wide, so a long field name must elide at 80 rather than spill into the operator. The
        // box top is the old origin (`row_rect.y + 19`), which keeps the label at the same
        // height while the fit supplies the horizontal bound.
        let field_label: &str =
            self.fields.get(row.field).map_or("(field)", |field| field.label.as_str());
        context.draw_text_fitted_cow(
            Rect::new(x, row_rect.y + 19, 80, 11),
            field_label,
            fonts.small,
            chrome.text(),
            HorizontalAlignment::Left,
        );
        x += 84;

        // Operator token.
        context.draw_text(
            Point::new(x, row_rect.y + 19),
            row.operator.as_str(),
            fonts.tiny,
            chrome.placeholder_text(),
            HorizontalAlignment::Left,
        );
        x += 96;

        // Operand field, drawn as an input box so an empty row reads as "type here".
        let operand_box = Rect::new(
            x,
            row_rect.y + 6,
            row_rect.width.saturating_sub(200 + depth as u32 * 10),
            18,
        );
        context.fill_rect(operand_box, chrome.field());
        context.draw_rect(operand_box, chrome.border);
        let (text, color): (&str, Color) = if row.operand.is_empty() {
            ("type a value", chrome.placeholder_text())
        } else {
            (row.operand.as_str(), chrome.text())
        };
        // The operand is host-supplied too, and it is the value most likely to be long, so it is
        // fitted to its own input box: the placement origin was `operand_box.y + 13`, so the box
        // keeps that top edge and a value wider than the box elides at its trailing edge instead
        // of painting over the row's remove button.
        context.draw_text_fitted_cow(
            Rect::new(
                operand_box.x + 5,
                operand_box.y + 13,
                operand_box.width.saturating_sub(8),
                11,
            ),
            text,
            fonts.small,
            color,
            HorizontalAlignment::Left,
        );

        // Remove button.
        let remove = Rect::new(row_rect.x + row_rect.width as i32 - 24, row_rect.y + 8, 16, 16);
        context.draw_line_stroke(
            Point::new(remove.x + 3, remove.y + 3),
            Point::new(remove.x + 13, remove.y + 13),
            chrome.remove(),
            1,
        );
        context.draw_line_stroke(
            Point::new(remove.x + 13, remove.y + 3),
            Point::new(remove.x + 3, remove.y + 13),
            chrome.remove(),
            1,
        );
    }
}

impl EventHandler for QueryBuilder {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos, .. } => {
                // Hover is the row under the pointer, from the same `row_at` the press
                // uses, so a highlight cannot land on a row a click would not reach.
                // `MouseMove` fires far more often than the highlight changes, so the
                // redraw is requested only on an actual change.
                let hovered = self.row_at(*pos);
                if hovered != self.hovered_row {
                    self.hovered_row = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_row.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { pos, button: 1 } => {
                if let Some(from) = self.drag_row.take() {
                    if let Some(to) = self.row_at(*pos) {
                        if from != to {
                            self.swap_rows(from, to);
                            self.active_row = Some(to);
                            self.base.request_redraw();
                        }
                    }
                }
            }
            Event::Wheel { delta, modifiers } => {
                let scroll = crate::event::WheelScroll::from_event(*delta, *modifiers, 1);
                let next = self.scroll_offset as isize - scroll.lines_y as isize;
                self.set_scroll_offset(next.max(0) as usize);
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                let rect = self.geometry();
                let header = Rect::new(rect.x, rect.y, rect.width, HEADER_HEIGHT);
                if header.contains_point(*pos) {
                    // Left half of the header toggles the conjunction; the add button
                    // is at the right edge.
                    let add = Rect::new(rect.x + rect.width as i32 - 30, rect.y + 7, 22, 20);
                    if add.contains_point(*pos) {
                        if !self.fields.is_empty() {
                            self.add_row(0);
                        }
                    } else {
                        self.toggle_conjunction();
                    }
                    return;
                }
                if let Some(index) = self.row_at(*pos) {
                    let Some(row_rect) = self.row_rect(index) else {
                        return;
                    };
                    let remove =
                        Rect::new(row_rect.x + row_rect.width as i32 - 24, row_rect.y + 8, 16, 16);
                    if remove.contains_point(*pos) {
                        self.remove_row(index);
                    } else {
                        self.set_active_row(index);
                        self.drag_row = Some(index);
                    }
                }
            }
            // Typing goes to the active row's operand, so the builder is usable
            // without a separate text control per row.
            Event::TextInput { text }
                if self.active_row.is_some() && text.chars().all(|ch| !ch.is_control()) =>
            {
                if let Some(index) = self.active_row {
                    if let Some(row) = self.rows.get_mut(index) {
                        row.operand.push_str(text);
                    }
                    self.emit_query();
                }
            }
            Event::KeyDown((8, _)) if self.active_row.is_some() => {
                if let Some(index) = self.active_row {
                    if let Some(row) = self.rows.get_mut(index) {
                        row.operand.pop();
                    }
                    self.emit_query();
                }
            }
            Event::KeyDown((38, _)) if self.active_row.is_some() => {
                if let Some(index) = self.active_row {
                    self.set_active_row(index.saturating_sub(1));
                }
            }
            Event::KeyDown((40, _)) if self.active_row.is_some() => {
                if let Some(index) = self.active_row {
                    let last = self.rows.len().saturating_sub(1);
                    self.set_active_row((index + 1).min(last));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    fn fields() -> Vec<FilterField> {
        vec![
            FilterField::text(0, "Name"),
            FilterField::number(1, "Amount"),
            FilterField::text(2, "Status"),
        ]
    }

    fn builder() -> QueryBuilder {
        let mut b = QueryBuilder::new(Rect::new(0, 0, 360, 160));
        b.set_fields(fields());
        b
    }

    /// Renders and returns the RGBA frame.
    fn render(b: &mut QueryBuilder, size: Size) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        b.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    fn row(a: &str, b: &str, c: &str) -> Vec<Option<String>> {
        vec![Some(a.to_string()), Some(b.to_string()), Some(c.to_string())]
    }

    #[test]
    fn query_builder_creation_defaults() {
        let b = QueryBuilder::new(Rect::new(0, 0, 360, 160));
        assert_eq!(b.kind(), WidgetKind::QueryBuilder);
        assert!(b.fields().is_empty());
        assert_eq!(b.row_count(), 0);
        assert_eq!(b.conjunction(), FilterConjunction::And);
        assert_eq!(b.active_row(), None);
    }

    #[test]
    fn query_builder_add_row_uses_the_field_default_operator() {
        let mut b = builder();
        assert_eq!(b.add_row(0), Some(0), "a text field defaults to contains");
        assert_eq!(b.add_row(1), Some(1), "a numeric field defaults to its first operator");
        assert_eq!(b.rows()[0].operator, FilterOperator::Contains);
        assert_eq!(b.rows()[1].operator, FilterOperator::EqualsNumber);
    }

    #[test]
    fn query_builder_add_row_rejects_an_unknown_field() {
        let mut b = builder();
        // A row that filters on nothing is not a row.
        assert_eq!(b.add_row(9), None);
        assert_eq!(b.row_count(), 0);
    }

    #[test]
    fn query_builder_a_new_row_becomes_active() {
        let mut b = builder();
        b.add_row(0);
        assert_eq!(b.active_row(), Some(0), "the user's next act is to type the operand");
        b.add_row(1);
        assert_eq!(b.active_row(), Some(1));
    }

    #[test]
    fn query_builder_remove_row_shifts_the_active_index() {
        let mut b = builder();
        b.add_row(0);
        b.add_row(1);
        b.add_row(2);
        b.set_active_row(2);

        // Removing a row before the active one shifts it down rather than dropping it.
        assert!(b.remove_row(0).is_some());
        assert_eq!(b.active_row(), Some(1));

        // Removing the active row clears it.
        assert!(b.remove_row(1).is_some());
        assert_eq!(b.active_row(), None);
    }

    #[test]
    fn query_builder_remove_row_rejects_an_unknown_index() {
        let mut b = builder();
        assert!(b.remove_row(0).is_none());
    }

    #[test]
    fn query_builder_set_fields_drops_rows_pointing_past_the_new_list() {
        let mut b = builder();
        b.add_row(2);
        b.set_row_operand(0, "open");
        assert_eq!(b.row_count(), 1);

        // Two fields now; the row pointed at index 2, which no longer exists, and an
        // index into a shorter list would address a different column.
        b.set_fields(vec![FilterField::text(0, "A"), FilterField::text(1, "B")]);
        assert_eq!(b.row_count(), 0, "a row on a vanished field is dropped");
    }

    #[test]
    fn query_builder_swap_rows_reorders() {
        let mut b = builder();
        b.add_row(0);
        b.add_row(1);
        b.set_row_operand(0, "first");
        b.set_row_operand(1, "second");

        assert!(b.swap_rows(0, 1));
        assert_eq!(b.rows()[0].operand, "second");
        assert_eq!(b.rows()[1].operand, "first");

        // Swapping a row with itself, or with a nonexistent row, is refused.
        assert!(!b.swap_rows(0, 0));
        assert!(!b.swap_rows(0, 9));
    }

    #[test]
    fn query_builder_set_row_operand_rejects_an_unknown_row() {
        let mut b = builder();
        assert!(!b.set_row_operand(0, "x"));
        b.add_row(0);
        assert!(b.set_row_operand(0, "x"));
        assert_eq!(b.rows()[0].operand, "x");
    }

    #[test]
    fn query_builder_set_row_operator_refuses_one_the_field_does_not_offer() {
        let mut b = builder();
        b.add_row(0); // text field
                      // `starts_with` is offered by a text field.
        assert!(b.set_row_operator(0, FilterOperator::StartsWith));
        // `greater_than` is not, and must be refused rather than applied: the UI
        // would never offer it, so a programmatic caller must not be able to.
        assert!(!b.set_row_operator(0, FilterOperator::GreaterThan));
        assert_eq!(b.rows()[0].operator, FilterOperator::StartsWith);
    }

    #[test]
    fn query_builder_set_row_negated_round_trips() {
        let mut b = builder();
        b.add_row(0);
        assert!(!b.rows()[0].negated);
        assert!(b.set_row_negated(0, true));
        assert!(b.rows()[0].negated);
        assert!(b.set_row_negated(0, false));
        assert!(!b.rows()[0].negated);
        assert!(!b.set_row_negated(9, true));
    }

    #[test]
    fn query_builder_toggle_conjunction_switches() {
        let mut b = builder();
        assert_eq!(b.conjunction(), FilterConjunction::And);
        b.toggle_conjunction();
        assert_eq!(b.conjunction(), FilterConjunction::Or);
        b.set_conjunction(FilterConjunction::Or);
        assert_eq!(b.conjunction(), FilterConjunction::Or, "setting the same value is a no-op");
    }

    // ── The produced expression ──────────────────────────────────────────────

    #[test]
    fn query_builder_empty_rows_produce_no_filter() {
        let b = builder();
        // No conditions means "no filter", not "match nothing": the latter would
        // blank the grid the moment the builder was shown.
        assert_eq!(b.build_query(), FilterExpr::MatchAll);
        assert!(b.build_query().is_noop());
    }

    #[test]
    fn query_builder_one_row_produces_one_predicate() {
        let mut b = builder();
        b.add_row(0);
        b.set_row_operand(0, "alpha");

        let query = b.build_query();
        assert_eq!(query, FilterExpr::Predicate(FilterCondition::contains(0, "alpha")));
        assert!(query.accepts(&row("Alpha", "", "")));
        assert!(!query.accepts(&row("Beta", "", "")));
    }

    #[test]
    fn query_builder_rows_combine_with_the_conjunction() {
        let mut b = builder();
        b.add_row(0);
        b.set_row_operand(0, "a");
        b.add_row(2);
        b.set_row_operand(1, "open");

        // AND: both must match.
        let query = b.build_query();
        assert!(query.accepts(&row("aaa", "", "open")));
        assert!(!query.accepts(&row("aaa", "", "closed")));
        assert!(!query.accepts(&row("zzz", "", "open")));

        // OR: either suffices.
        b.set_conjunction(FilterConjunction::Or);
        let query = b.build_query();
        assert!(query.accepts(&row("aaa", "", "closed")));
        assert!(query.accepts(&row("zzz", "", "open")));
        assert!(!query.accepts(&row("zzz", "", "closed")));
    }

    #[test]
    fn query_builder_a_row_with_no_operand_is_excluded_from_the_query() {
        let mut b = builder();
        b.add_row(0);
        b.set_row_operand(0, "alpha");
        b.add_row(2); // chosen a field, not yet typed
        b.add_row(1);

        // A half-typed row must not blank the grid while the user is still typing.
        let query = b.build_query();
        assert_eq!(query.condition_count(), 1, "only the complete row is in the query");
        assert!(query.accepts(&row("Alpha", "", "")));
        assert_eq!(b.incomplete_rows(), vec![1, 2]);
    }

    #[test]
    fn query_builder_a_numeric_row_uses_its_operator() {
        let mut b = builder();
        b.add_row(1); // Amount, defaults to equals_number
        b.set_row_operator(0, FilterOperator::GreaterThan);
        b.set_row_operand(0, "100");

        let query = b.build_query();
        assert!(query.accepts(&row("", "150", "")));
        assert!(!query.accepts(&row("", "50", "")));
    }

    #[test]
    fn query_builder_a_negated_row_inverts_just_that_row() {
        let mut b = builder();
        b.add_row(0);
        b.set_row_operand(0, "alpha");
        b.set_row_negated(0, true);
        b.add_row(2);
        b.set_row_operand(1, "open");

        // `not(name contains alpha) AND status contains open` — the negation applies
        // to the one row, not to the whole conjunction.
        let query = b.build_query();
        assert!(query.accepts(&row("Beta", "", "open")));
        assert!(!query.accepts(&row("Alpha", "", "open")));
        assert!(!query.accepts(&row("Beta", "", "closed")));
    }

    #[test]
    fn query_builder_query_changed_signal_fires_on_every_edit() {
        let mut b = builder();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = seen.clone();
        b.query_changed.connect(move |query| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(query.condition_count());
            }
        });

        b.add_row(0); // 0 conditions (no operand yet)
        b.set_row_operand(0, "x"); // 1
        b.add_row(2); // 1
        b.set_row_operand(1, "y"); // 2
        b.remove_row(0); // 1

        assert_eq!(*seen.lock().expect("signal lock poisoned"), vec![0, 1, 1, 2, 1]);
    }

    // ── Loading an expression ───────────────────────────────────────────────

    #[test]
    fn query_builder_set_query_round_trips_a_flat_conjunction() {
        let mut b = builder();
        b.add_row(0);
        b.set_row_operand(0, "alpha");
        b.add_row(2);
        b.set_row_operand(1, "open");
        let built = b.build_query();

        // Loading what the builder produced must reproduce the builder.
        let mut fresh = builder();
        assert!(fresh.set_query(&built), "a flat conjunction round-trips exactly");
        assert_eq!(fresh.row_count(), 2);
        assert_eq!(fresh.rows()[0].operand, "alpha");
        assert_eq!(fresh.rows()[1].operand, "open");
        assert_eq!(fresh.build_query(), built);
    }

    #[test]
    fn query_builder_set_query_round_trips_a_negated_row() {
        let mut b = builder();
        b.add_row(0);
        b.set_row_operand(0, "alpha");
        b.set_row_negated(0, true);
        let built = b.build_query();

        let mut fresh = builder();
        assert!(fresh.set_query(&built));
        assert!(fresh.rows()[0].negated, "the negation must survive the round trip");
        assert_eq!(fresh.build_query(), built);
    }

    #[test]
    fn query_builder_set_query_round_trips_the_conjunction() {
        let mut b = builder();
        b.add_row(0);
        b.set_row_operand(0, "a");
        b.add_row(2);
        b.set_row_operand(1, "b");
        b.set_conjunction(FilterConjunction::Or);
        let built = b.build_query();

        let mut fresh = builder();
        assert!(fresh.set_query(&built));
        assert_eq!(fresh.conjunction(), FilterConjunction::Or);
        assert_eq!(fresh.build_query(), built);
    }

    #[test]
    fn query_builder_set_query_round_trips_a_nested_group() {
        let mut b = builder();
        // (name AND amount) OR status — a group inside the root disjunction.
        let nested = FilterExpr::or(vec![
            FilterExpr::and(vec![
                FilterExpr::Predicate(FilterCondition::contains(0, "a")),
                FilterExpr::Predicate(FilterCondition::contains(1, "5")),
            ]),
            FilterExpr::Predicate(FilterCondition::contains(2, "open")),
        ]);

        assert!(b.set_query(&nested), "a nested group is now representable");
        assert!(!b.groups().is_empty(), "the group survived the load");
        // Both sides agree on every row, which is what "round trip" has to mean for a
        // tree whose operand order the loader normalises.
        for row in [
            vec![Some("a".into()), Some("5".into()), Some("closed".into())],
            vec![Some("a".into()), Some("1".into()), Some("open".into())],
            vec![Some("z".into()), Some("0".into()), Some("open".into())],
            vec![Some("z".into()), Some("0".into()), Some("closed".into())],
            vec![Some("a".into()), Some("5".into()), Some("open".into())],
        ] {
            assert_eq!(nested.accepts(&row), b.build_query().accepts(&row), "{row:?}");
        }
    }

    /// Groups nest to any depth and `(a AND b) OR c` evaluates as written.
    #[test]
    fn nested_groups_build_a_parenthesised_expression() {
        let mut b = builder();
        let group = b.add_group(None).expect("a root group");
        let a = b.add_row_in_group(0, group).expect("a row in the group");
        let b_row = b.add_row_in_group(1, group).expect("a row in the group");
        let c = b.add_row(2).expect("a root row");
        b.set_row_operand(a, "a");
        b.set_row_operand(b_row, "5");
        b.set_row_operand(c, "c");
        b.set_conjunction(FilterConjunction::Or);

        let expr = b.build_query();
        assert!(expr.accepts(&[Some("a".into()), Some("5".into()), Some("z".into())]));
        assert!(!expr.accepts(&[Some("a".into()), Some("4".into()), Some("z".into())]));
        assert!(expr.accepts(&[Some("x".into()), Some("0".into()), Some("c".into())]));

        // A deeper group reports its depth, which is what the row indentation draws from.
        let inner = b.add_group(Some(group)).expect("a nested group");
        assert!(b.set_group_conjunction(inner, FilterConjunction::Or));
        let deep = b.add_row_in_group(0, inner).expect("a row in the nested group");
        b.set_row_operand(deep, "d");
        assert_eq!(b.row_group_depth(deep), 2);

        // Removing the outer group removes the nested group and every row inside it.
        assert!(b.remove_group(group));
        assert!(b.groups().is_empty(), "the nested group went with its parent");
        assert_eq!(b.row_count(), 1, "only the root row remains");
    }

    #[test]
    fn query_builder_set_query_on_match_all_empties_the_rows() {
        let mut b = builder();
        b.add_row(0);
        b.set_row_operand(0, "x");
        assert!(b.set_query(&FilterExpr::MatchAll));
        assert_eq!(b.row_count(), 0);
        assert_eq!(b.build_query(), FilterExpr::MatchAll);
    }

    #[test]
    fn query_builder_set_query_skips_a_condition_on_an_undeclared_column() {
        let mut b = builder();
        // Column 7 has no field, so the row cannot be shown; the expression is
        // reported as not exactly representable rather than a field being invented.
        let expr = FilterExpr::Predicate(FilterCondition::contains(7, "x"));
        assert!(!b.set_query(&expr));
        assert_eq!(b.row_count(), 0);
    }

    // ── Interaction ─────────────────────────────────────────────────────────

    #[test]
    fn query_builder_click_selects_a_row() {
        let mut b = builder();
        b.add_row(0);
        b.add_row(2);
        b.active_row = None;

        let rect = b.row_rect(1).expect("row 1");
        b.handle_event(&Event::mouse_press(rect.x + 60, rect.y + 10, 1));
        assert_eq!(b.active_row(), Some(1));
    }

    #[test]
    fn query_builder_click_on_remove_deletes_the_row() {
        let mut b = builder();
        b.add_row(0);
        b.add_row(2);
        let rect = b.row_rect(0).expect("row 0");
        let remove_x = rect.x + rect.width as i32 - 16;
        b.handle_event(&Event::mouse_press(remove_x, rect.y + 16, 1));
        assert_eq!(b.row_count(), 1);
    }

    #[test]
    fn query_builder_click_on_the_header_toggles_the_conjunction() {
        let mut b = builder();
        b.handle_event(&Event::mouse_press(20, 16, 1));
        assert_eq!(b.conjunction(), FilterConjunction::Or);
    }

    #[test]
    fn query_builder_click_on_add_appends_a_row() {
        let mut b = builder();
        let rect = b.geometry();
        let add_x = rect.x + rect.width as i32 - 20;
        b.handle_event(&Event::mouse_press(add_x, rect.y + 16, 1));
        assert_eq!(b.row_count(), 1);
    }

    #[test]
    fn query_builder_typing_goes_to_the_active_row() {
        let mut b = builder();
        b.add_row(0);
        b.handle_event(&Event::TextInput { text: "al".to_string() });
        b.handle_event(&Event::TextInput { text: "pha".to_string() });
        assert_eq!(b.rows()[0].operand, "alpha");

        // Backspace removes one character.
        b.handle_event(&Event::KeyDown((8, 0)));
        assert_eq!(b.rows()[0].operand, "alph");
    }

    #[test]
    fn query_builder_typing_without_an_active_row_is_ignored() {
        let mut b = builder();
        b.add_row(0);
        b.active_row = None;
        b.handle_event(&Event::TextInput { text: "x".to_string() });
        assert_eq!(b.rows()[0].operand, "");
    }

    #[test]
    fn query_builder_arrows_move_the_active_row() {
        let mut b = builder();
        b.add_row(0);
        b.add_row(2);
        b.add_row(1);
        b.set_active_row(1);

        b.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(b.active_row(), Some(2));
        // Clamped at the end.
        b.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(b.active_row(), Some(2));
        b.handle_event(&Event::KeyDown((38, 0)));
        assert_eq!(b.active_row(), Some(1));
    }

    #[test]
    fn query_builder_disabled_ignores_clicks() {
        let mut b = builder();
        b.set_enabled(false);
        b.handle_event(&Event::mouse_press(20, 16, 1));
        assert_eq!(b.conjunction(), FilterConjunction::And, "a disabled builder must not toggle");
    }

    // ── Drawing ─────────────────────────────────────────────────────────────

    #[test]
    fn query_builder_draw_empty_paints_a_hint() {
        let mut b = builder();
        let empty = render(&mut b, Size::new(360, 160));
        b.add_row(0);
        let with_row = render(&mut b, Size::new(360, 160));
        assert_ne!(empty, with_row, "a row must be visible");
    }

    #[test]
    fn query_builder_draw_zero_geometry_does_not_panic() {
        let mut b = builder();
        b.add_row(0);
        let rgba = render(&mut b, Size::new(4, 4));
        assert!(!rgba.is_empty());
    }

    #[test]
    fn query_builder_active_row_is_visible() {
        let mut b = builder();
        b.add_row(0);
        b.active_row = None;
        let inactive = render(&mut b, Size::new(360, 160));
        b.set_active_row(0);
        let active = render(&mut b, Size::new(360, 160));
        assert_ne!(inactive, active, "the active row must be highlighted");
    }

    // ── Property contract ───────────────────────────────────────────────────

    #[test]
    fn query_builder_conjunction_property_round_trips() {
        let mut b = builder();
        assert_eq!(b.get("conjunction").unwrap(), CapabilityValue::String("and".to_string()));
        b.set("conjunction", CapabilityValue::String("or".to_string())).unwrap();
        assert_eq!(b.conjunction(), FilterConjunction::Or);
        // An unknown token is a type mismatch, not a silent no-op.
        assert!(b.set("conjunction", CapabilityValue::String("xor".to_string())).is_err());
    }

    #[test]
    fn query_builder_active_row_property_round_trips() {
        let mut b = builder();
        b.add_row(0);
        assert_eq!(b.get("active_row").unwrap(), CapabilityValue::UInt(0));
        b.set("active_row", CapabilityValue::UInt(0)).unwrap();
        assert_eq!(b.active_row(), Some(0));
        // An out-of-range index is ignored rather than stored.
        b.set("active_row", CapabilityValue::UInt(9)).unwrap();
        assert_eq!(b.active_row(), Some(0));
        // `Null` is the getter's "no active row" answer and the schema default, so the
        // setter must accept it; it is what the writable gate writes back.
        b.set("active_row", CapabilityValue::Null).unwrap();
        assert_eq!(b.active_row(), None);
        assert_eq!(b.get("active_row").unwrap(), CapabilityValue::Null);
    }

    #[test]
    fn query_builder_derived_properties_are_read_only() {
        let mut b = builder();
        b.add_row(0);
        assert_eq!(b.get("row_count").unwrap(), CapabilityValue::UInt(1));
        assert_eq!(b.get("field_count").unwrap(), CapabilityValue::UInt(3));
        assert_eq!(b.get("incomplete_row_count").unwrap(), CapabilityValue::UInt(1));
        for name in ["row_count", "incomplete_row_count", "field_count"] {
            assert_eq!(
                b.set(name, CapabilityValue::UInt(9)),
                Err(CapabilityAccessError::ReadOnlyProperty),
                "{name} must be read-only"
            );
        }
    }

    #[test]
    fn filter_conjunction_round_trips_and_toggles() {
        for conjunction in [FilterConjunction::And, FilterConjunction::Or] {
            assert_eq!(FilterConjunction::from_name(conjunction.as_str()), Some(conjunction));
            assert_ne!(conjunction.toggled(), conjunction);
        }
        assert_eq!(FilterConjunction::from_name("nand"), None);
        assert_eq!(FilterConjunction::default(), FilterConjunction::And);
    }

    #[test]
    fn filter_field_constructors_offer_the_right_operators() {
        let text = FilterField::text(0, "Name");
        assert_eq!(text.default_operator(), FilterOperator::Contains);
        assert!(text.operators.contains(&FilterOperator::StartsWith));
        assert!(!text.operators.contains(&FilterOperator::GreaterThan));

        let number = FilterField::number(1, "Amount");
        assert_eq!(number.default_operator(), FilterOperator::EqualsNumber);
        assert!(number.operators.contains(&FilterOperator::GreaterThan));
        assert!(!number.operators.contains(&FilterOperator::StartsWith));

        // An explicit field with no operators still offers one, so a row can be made.
        let bare = FilterField::new(2, "Bare", Vec::new());
        assert_eq!(bare.default_operator(), FilterOperator::Contains);
    }

    /// Hover is the row under the pointer, from the same `row_at` the press uses,
    /// and leaving the builder clears the highlight instead of freezing it.
    #[test]
    fn query_builder_hover_follows_the_pointer() {
        let mut b = builder();
        b.add_row(0);
        b.add_row(0);

        b.handle_event(&Event::mouse_move(30, 60));
        assert_eq!(b.hovered_row, Some(0));

        b.handle_event(&Event::mouse_move(30, 79));
        assert_eq!(b.hovered_row, Some(1));

        // Above the first row (in the header) and below the last row, nothing hovers.
        b.handle_event(&Event::mouse_move(30, 5));
        assert_eq!(b.hovered_row, None);
        b.handle_event(&Event::mouse_move(30, 140));
        assert_eq!(b.hovered_row, None);

        b.handle_event(&Event::mouse_leave(-1, -1));
        assert_eq!(b.hovered_row, None);
    }

    /// The hover follows its row through a removal exactly as `active_row` does,
    /// so the highlight never names a row that is gone.
    #[test]
    fn query_builder_hover_survives_a_removal_honestly() {
        let mut b = builder();
        b.add_row(0);
        b.add_row(0);
        b.add_row(0);
        b.handle_event(&Event::mouse_move(30, 79));
        assert_eq!(b.hovered_row, Some(1));

        // Removing a row before the hovered one shifts the highlight down with it.
        b.remove_row(0).expect("row 0");
        assert_eq!(b.hovered_row, Some(0));

        // Removing the hovered row clears it.
        b.remove_row(0).expect("row 0");
        assert_eq!(b.hovered_row, None);
    }

    /// Row changes broadcast, the strip scrolls by rows, and a drag reorders the rows.
    #[test]
    fn rows_changed_scroll_and_drag_reorder_are_wired() {
        let mut b = QueryBuilder::new(Rect::new(0, 0, 360, 70));
        b.set_fields(fields());
        let changes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sink = std::sync::Arc::clone(&changes);
        b.rows_changed.connect(move |_| {
            sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        b.add_row(0).expect("a row");
        b.add_row(1).expect("a row");
        assert_eq!(changes.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(b.row_count(), 2);

        let first = b.row_rect(0).expect("the first row is visible");
        let second = b.row_rect(1).expect("the second row is visible");
        b.handle_event(&Event::MousePress {
            pos: Point::new(first.x + 20, first.y + 10),
            button: 1,
            modifiers: 0,
        });
        b.handle_event(&Event::MouseRelease {
            pos: Point::new(second.x + 20, second.y + 10),
            button: 1,
        });
        assert_eq!(
            changes.load(std::sync::atomic::Ordering::SeqCst),
            3,
            "the drag emits one row-change notification"
        );

        b.set_scroll_offset(1);
        assert_eq!(b.scroll_offset(), 1);
        b.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(b.scroll_offset(), 0);
    }
}
