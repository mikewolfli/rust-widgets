// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A recursive filter expression, so a screen built by hand and one built by a
//! user in a query builder filter rows through **the same** model.
//!
//! # Why this replaced a flat list
//!
//! `DataGrid` filtered through `Vec<ColumnFilter>`, where each entry was one
//! column plus one substring, combined with an implicit AND
//! (`data_grid.rs:322-332`). That model cannot express:
//!
//! * **OR** — "status is open *or* status is pending";
//! * **nesting** — "`(a AND b) OR c`", which is not a flat conjunction of anything;
//! * **non-text predicates** — "amount > 100", `ColumnFilter`'s `query: String` is
//!   a `contains` match by construction.
//!
//! Those are exactly the things a user is offered by a query-builder UI, so
//! without a recursive model the UI could produce conditions the grid could not
//! evaluate (principle #54: one definition per semantic, not two).
//!
//! # Why the tree is not boxed
//!
//! `Vec<FilterExpr>` inside the `And`/`Or` variants, rather than
//! `Box<FilterExpr>` or `Rc`: the tree is short (a condition list a person built),
//! it is walked once per row, and `Vec` gives the children an order that a nested
//! box chain would have to reconstruct. `DataGrid` clones it into its cache key,
//! which `Rc` would make cheaper — but a filter tree is tens of nodes, and
//! `Rc<FilterExpr>` would leak the sharing into a public API for no measured gain
//! (rule #28).
//!
//! # Relationship to `ColumnFilter`
//!
//! `ColumnFilter` remains the single-condition spelling and is **not** deprecated:
//! it is what a simple "contains this text in this column" caller wants, and what
//! the `filters` property still accepts. [`FilterExpr::from_conditions`]
//! converts a list of them into the recursive form under one AND, which is the
//! conversion `DataGrid::set_filters` performs, so the two spellings always agree.

/// How a text predicate compares a cell against its operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterOperator {
    /// Cell contains the operand, case-insensitively.
    Contains,
    /// Cell equals the operand, case-insensitively.
    Equals,
    /// Cell begins with the operand, case-insensitively.
    StartsWith,
    /// Cell ends with the operand, case-insensitively.
    EndsWith,
    /// Cell does not contain the operand, case-insensitively.
    NotContains,
    /// Cell parses as a number equal to the operand.
    EqualsNumber,
    /// Cell parses as a number greater than the operand.
    GreaterThan,
    /// Cell parses as a number greater than or equal to the operand.
    GreaterOrEqual,
    /// Cell parses as a number less than the operand.
    LessThan,
    /// Cell parses as a number less than or equal to the operand.
    LessOrEqual,
}

impl FilterOperator {
    /// The token this operator is spelled as in JSON and in a saved filter.
    pub fn as_str(self) -> &'static str {
        match self {
            FilterOperator::Contains => "contains",
            FilterOperator::Equals => "equals",
            FilterOperator::StartsWith => "starts_with",
            FilterOperator::EndsWith => "ends_with",
            FilterOperator::NotContains => "not_contains",
            FilterOperator::EqualsNumber => "equals_number",
            FilterOperator::GreaterThan => "greater_than",
            FilterOperator::GreaterOrEqual => "greater_or_equal",
            FilterOperator::LessThan => "less_than",
            FilterOperator::LessOrEqual => "less_or_equal",
        }
    }

    /// Parses the token back, rejecting anything else.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "contains" => FilterOperator::Contains,
            "equals" => FilterOperator::Equals,
            "starts_with" => FilterOperator::StartsWith,
            "ends_with" => FilterOperator::EndsWith,
            "not_contains" => FilterOperator::NotContains,
            "equals_number" => FilterOperator::EqualsNumber,
            "greater_than" => FilterOperator::GreaterThan,
            "greater_or_equal" => FilterOperator::GreaterOrEqual,
            "less_than" => FilterOperator::LessThan,
            "less_or_equal" => FilterOperator::LessOrEqual,
            _ => return None,
        })
    }

    /// Whether applying this operator needs the operand parsed as a number.
    ///
    /// Exposed so a query-builder UI can pick the right input widget for an
    /// operator, and so [`Self::accepts`] and the numeric arms stay in one place.
    pub fn is_numeric(self) -> bool {
        matches!(
            self,
            FilterOperator::EqualsNumber
                | FilterOperator::GreaterThan
                | FilterOperator::GreaterOrEqual
                | FilterOperator::LessThan
                | FilterOperator::LessOrEqual
        )
    }

    /// Evaluates this operator against one cell value.
    ///
    /// # The two failure modes, stated rather than conflated
    ///
    /// 1. **The cell is empty.** `None` and `""` both mean "no value here", and a
    ///    row with no value in the filtered column does **not** match — matching it
    ///    would make a filter for "status = open" also list rows whose status is
    ///    absent, which is never what the user meant. `NotContains` is the
    ///    exception: "does not contain" is true of a cell that has nothing in it.
    /// 2. **The cell is not a number.** A numeric operator on a text cell does
    ///    **not** match, and does not error. A query builder offers numeric
    ///    operators for every column (it cannot always know a column's type), so
    ///    "amount > 100" against a row whose amount is `"N/A"` has one sensible
    ///    reading: that row is not greater than 100.
    pub fn accepts(self, cell: Option<&str>, operand: &str) -> bool {
        let Some(cell) = cell else {
            return self == FilterOperator::NotContains;
        };
        if self == FilterOperator::NotContains {
            // An empty cell trivially does not contain the operand.
            if cell.is_empty() {
                return true;
            }
            return !cell.to_lowercase().contains(&operand.to_lowercase());
        }
        if cell.is_empty() {
            return false;
        }
        match self {
            FilterOperator::Contains => cell.to_lowercase().contains(&operand.to_lowercase()),
            FilterOperator::Equals => cell.to_lowercase() == operand.to_lowercase(),
            FilterOperator::StartsWith => cell.to_lowercase().starts_with(&operand.to_lowercase()),
            FilterOperator::EndsWith => cell.to_lowercase().ends_with(&operand.to_lowercase()),
            FilterOperator::NotContains => true, // handled above
            FilterOperator::EqualsNumber
            | FilterOperator::GreaterThan
            | FilterOperator::GreaterOrEqual
            | FilterOperator::LessThan
            | FilterOperator::LessOrEqual => {
                let Ok(cell_number) = cell.trim().parse::<f64>() else {
                    return false;
                };
                let Ok(operand_number) = operand.trim().parse::<f64>() else {
                    // A malformed operand cannot be satisfied by any cell, which is
                    // reported by matching nothing rather than by panicking on a
                    // value the UI let the user type.
                    return false;
                };
                match self {
                    FilterOperator::EqualsNumber => cell_number == operand_number,
                    FilterOperator::GreaterThan => cell_number > operand_number,
                    FilterOperator::GreaterOrEqual => cell_number >= operand_number,
                    FilterOperator::LessThan => cell_number < operand_number,
                    FilterOperator::LessOrEqual => cell_number <= operand_number,
                    _ => false,
                }
            }
        }
    }
}

/// One condition: a column, an operator, and an operand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterCondition {
    /// Zero-based source column index.
    pub column: usize,
    /// How the cell is compared.
    pub operator: FilterOperator,
    /// What it is compared against.
    pub operand: String,
}

impl FilterCondition {
    /// Creates a case-insensitive `contains` condition, the common case.
    pub fn contains(column: usize, operand: impl Into<String>) -> Self {
        Self { column, operator: FilterOperator::Contains, operand: operand.into() }
    }

    /// Creates a condition with an explicit operator.
    pub fn new(column: usize, operator: FilterOperator, operand: impl Into<String>) -> Self {
        Self { column, operator, operand: operand.into() }
    }

    /// Evaluates this condition against one row.
    fn accepts(&self, row: &[Option<String>]) -> bool {
        let cell = row.get(self.column).and_then(|cell| cell.as_deref());
        self.operator.accepts(cell, &self.operand)
    }
}

/// A recursive filter: conditions combined by AND/OR, arbitrarily nested.
///
/// # Empty sub-lists
///
/// An `And` with no children accepts every row, and an `Or` with no children
/// accepts none. Those are the identities that make the tree composable: removing
/// the last child of an `And` should widen the result to "no filter", not narrow it
/// to "nothing matches". A caller that means "nothing" uses
/// [`FilterExpr::MatchNothing`], which says so.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterExpr {
    /// A single condition.
    Predicate(FilterCondition),
    /// Every child must accept.
    And(Vec<FilterExpr>),
    /// At least one child must accept.
    Or(Vec<FilterExpr>),
    /// The negation of the child.
    Not(Box<FilterExpr>),
    /// Accepts every row. The identity of `And`.
    MatchAll,
    /// Accepts no row. The identity of `Or`.
    MatchNothing,
}

impl FilterExpr {
    /// A tree with no predicates, which accepts everything.
    ///
    /// What a grid with no filter set holds, so `filters` and `filter_expr` agree
    /// without a separate "no filter" flag.
    pub fn match_all() -> Self {
        FilterExpr::MatchAll
    }

    /// Wraps `conditions` in one `And`, accepting everything when the list is empty.
    ///
    /// The conversion from the legacy flat model: `DataGrid::set_filters` combined
    /// its `Vec<ColumnFilter>` with an implicit AND, so this is that same rule
    /// written down.
    pub fn from_conditions(conditions: Vec<FilterCondition>) -> Self {
        match conditions.len() {
            0 => FilterExpr::MatchAll,
            1 => FilterExpr::Predicate(conditions.into_iter().next().expect("length checked")),
            _ => FilterExpr::And(conditions.into_iter().map(FilterExpr::Predicate).collect()),
        }
    }

    /// Combines `children` with AND, flattening a nested `And` so that
    /// `and(and(a, b), c)` and `and(a, b, c)` are the same tree.
    ///
    /// Flattening matters because a query-builder UI appended to an existing group
    /// would otherwise grow the tree by one level per edit, and two trees that mean
    /// the same thing should compare equal.
    pub fn and(children: Vec<FilterExpr>) -> Self {
        let mut flattened = Vec::new();
        for child in children {
            match child {
                FilterExpr::And(inner) => flattened.extend(inner),
                other => flattened.push(other),
            }
        }
        match flattened.len() {
            0 => FilterExpr::MatchAll,
            1 => flattened.into_iter().next().expect("length checked"),
            _ => FilterExpr::And(flattened),
        }
    }

    /// Combines `children` with OR, flattening a nested `Or`.
    pub fn or(children: Vec<FilterExpr>) -> Self {
        let mut flattened = Vec::new();
        for child in children {
            match child {
                FilterExpr::Or(inner) => flattened.extend(inner),
                other => flattened.push(other),
            }
        }
        match flattened.len() {
            0 => FilterExpr::MatchNothing,
            1 => flattened.into_iter().next().expect("length checked"),
            _ => FilterExpr::Or(flattened),
        }
    }

    /// Negates `expr`, folding a double negation away.
    ///
    /// `negate` rather than `not`: a static `not` shadows the name a reader expects
    /// from `std::ops::Not`, and this is an ordinary associated function, not an
    /// operator overload.
    pub fn negate(expr: FilterExpr) -> Self {
        match expr {
            FilterExpr::Not(inner) => *inner,
            FilterExpr::MatchAll => FilterExpr::MatchNothing,
            FilterExpr::MatchNothing => FilterExpr::MatchAll,
            other => FilterExpr::Not(Box::new(other)),
        }
    }

    /// Evaluates the tree against one row.
    ///
    /// A row is a slice of optional cell strings, which is the shape
    /// `DataGrid::apply_filter_sort` already works with.
    pub fn accepts(&self, row: &[Option<String>]) -> bool {
        match self {
            FilterExpr::Predicate(condition) => condition.accepts(row),
            FilterExpr::And(children) => children.iter().all(|child| child.accepts(row)),
            FilterExpr::Or(children) => children.iter().any(|child| child.accepts(row)),
            FilterExpr::Not(inner) => !inner.accepts(row),
            FilterExpr::MatchAll => true,
            FilterExpr::MatchNothing => false,
        }
    }

    /// Returns whether the tree can reject any row.
    ///
    /// `false` means the filter is a no-op, which is what lets
    /// `apply_filter_sort` skip the whole `retain` pass instead of walking every
    /// cell to conclude the same thing.
    pub fn is_noop(&self) -> bool {
        match self {
            FilterExpr::MatchAll => true,
            FilterExpr::MatchNothing => false,
            FilterExpr::Predicate(_) => false,
            FilterExpr::And(children) => children.iter().any(FilterExpr::is_noop),
            FilterExpr::Or(children) => {
                // An empty `Or` is `MatchNothing`, which is not a no-op; a non-empty
                // one is a no-op only if some child is.
                !children.is_empty() && children.iter().any(FilterExpr::is_noop)
            }
            // A negation is never a no-op: it always rejects at least one row.
            FilterExpr::Not(_) => false,
        }
    }

    /// The number of predicates in the tree.
    ///
    /// What a query-builder UI shows as a condition count, and what
    /// `DataGrid` publishes as `filter_condition_count`.
    pub fn condition_count(&self) -> usize {
        match self {
            FilterExpr::Predicate(_) => 1,
            FilterExpr::And(children) | FilterExpr::Or(children) => {
                children.iter().map(FilterExpr::condition_count).sum()
            }
            FilterExpr::Not(inner) => inner.condition_count(),
            FilterExpr::MatchAll | FilterExpr::MatchNothing => 0,
        }
    }

    /// Every condition in the tree, in left-to-right order.
    ///
    /// The flattening a query-builder UI needs to populate its rows from a model,
    /// and what `DataGrid::set_filters`' inverse uses.
    pub fn conditions(&self) -> Vec<&FilterCondition> {
        let mut out = Vec::new();
        self.collect_conditions(&mut out);
        out
    }

    /// Appends this tree's conditions to `out`, depth-first and left to right.
    fn collect_conditions<'a>(&'a self, out: &mut Vec<&'a FilterCondition>) {
        match self {
            FilterExpr::Predicate(condition) => out.push(condition),
            FilterExpr::And(children) | FilterExpr::Or(children) => {
                for child in children {
                    child.collect_conditions(out);
                }
            }
            FilterExpr::Not(inner) => inner.collect_conditions(out),
            FilterExpr::MatchAll | FilterExpr::MatchNothing => {}
        }
    }

    /// Returns the tree with every condition on `column` dropped.
    ///
    /// What a UI calls when a column is hidden or a condition row is deleted.
    /// Groups that become empty collapse to their identity, so removing the last
    /// condition narrows the tree back to "no filter" rather than to a group that
    /// matches nothing.
    pub fn without_column(&self, column: usize) -> FilterExpr {
        match self {
            FilterExpr::Predicate(condition) => {
                if condition.column == column {
                    FilterExpr::MatchAll
                } else {
                    self.clone()
                }
            }
            FilterExpr::And(children) => {
                FilterExpr::and(children.iter().map(|child| child.without_column(column)).collect())
            }
            FilterExpr::Or(children) => {
                FilterExpr::or(children.iter().map(|child| child.without_column(column)).collect())
            }
            FilterExpr::Not(inner) => {
                let inner = inner.without_column(column);
                // `Not(MatchAll)` is `MatchNothing`, which is not what "the column
                // was removed" should mean, so a negation whose child vanished is
                // dropped rather than inverted.
                if inner.is_noop() {
                    FilterExpr::MatchAll
                } else {
                    FilterExpr::negate(inner)
                }
            }
            FilterExpr::MatchAll | FilterExpr::MatchNothing => self.clone(),
        }
    }
}

impl Default for FilterExpr {
    /// No filter, which accepts every row.
    fn default() -> Self {
        FilterExpr::MatchAll
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A row with three cells, some empty.
    fn row(a: &str, b: &str, c: &str) -> Vec<Option<String>> {
        vec![Some(a.to_string()), Some(b.to_string()), Some(c.to_string())]
    }

    #[test]
    fn match_all_accepts_every_row_and_match_nothing_accepts_none() {
        let rows = [row("a", "b", "c"), vec![None, None, None]];
        let all = FilterExpr::MatchAll;
        assert!(rows.iter().all(|r| all.accepts(r)));
        let none = FilterExpr::MatchNothing;
        assert!(rows.iter().all(|r| !none.accepts(r)));
    }

    #[test]
    fn a_predicate_filters_one_column_case_insensitively() {
        let expr = FilterExpr::Predicate(FilterCondition::contains(0, "ALP"));
        assert!(expr.accepts(&row("Alpha", "b", "c")));
        assert!(!expr.accepts(&row("Beta", "b", "c")));
    }

    #[test]
    fn an_empty_cell_does_not_match_a_contains_filter() {
        let expr = FilterExpr::Predicate(FilterCondition::contains(0, "x"));
        // A row with nothing in the column is not "containing x", and listing it
        // would make the filter look broken.
        assert!(!expr.accepts(&[None, None, None]));
        assert!(!expr.accepts(&row("", "", "")));
    }

    #[test]
    fn an_empty_cell_does_match_not_contains() {
        let expr = FilterExpr::Predicate(FilterCondition::new(0, FilterOperator::NotContains, "x"));
        assert!(expr.accepts(&[None, None, None]));
        assert!(expr.accepts(&row("", "", "")));
        assert!(expr.accepts(&row("abc", "", "")));
        assert!(!expr.accepts(&row("axc", "", "")));
    }

    #[test]
    fn text_operators_compare_as_documented() {
        let cases = [
            (FilterOperator::Equals, "alpha", true),
            (FilterOperator::Equals, "alph", false),
            (FilterOperator::StartsWith, "alp", true),
            (FilterOperator::StartsWith, "lpha", false),
            (FilterOperator::EndsWith, "pha", true),
            (FilterOperator::EndsWith, "alp", false),
            (FilterOperator::Contains, "LPH", true),
        ];
        for (operator, operand, expected) in cases {
            let expr = FilterExpr::Predicate(FilterCondition::new(0, operator, operand));
            assert_eq!(
                expr.accepts(&row("Alpha", "", "")),
                expected,
                "{operator:?} {operand} on Alpha"
            );
        }
    }

    #[test]
    fn numeric_operators_compare_numbers_not_strings() {
        // "9" vs "10": as strings, "9" > "10" (because '9' > '1'); as numbers it is
        // the other way. The numeric operator must give the *numeric* answer, which
        // is the whole reason it parses rather than compares text.
        let expr =
            FilterExpr::Predicate(FilterCondition::new(0, FilterOperator::GreaterThan, "10"));
        assert!(!expr.accepts(&row("9", "", "")), "9 is not greater than 10 numerically");
        assert!(!expr.accepts(&row("2", "", "")));
        assert!(expr.accepts(&row("10.5", "", "")));
        assert!(expr.accepts(&row("11", "", "")));
    }

    #[test]
    fn a_numeric_operator_on_a_non_numeric_cell_does_not_match() {
        let expr =
            FilterExpr::Predicate(FilterCondition::new(0, FilterOperator::GreaterThan, "10"));
        // A query builder cannot know a column's type, so this is a real case: the
        // one sensible reading is that the row is not greater than 10.
        assert!(!expr.accepts(&row("N/A", "", "")));
        assert!(!expr.accepts(&row("", "", "")));
    }

    #[test]
    fn a_malformed_operand_matches_nothing_rather_than_panicking() {
        let expr = FilterExpr::Predicate(FilterCondition::new(
            0,
            FilterOperator::EqualsNumber,
            "not a number",
        ));
        assert!(!expr.accepts(&row("5", "", "")));
        assert!(!expr.accepts(&row("abc", "", "")));
    }

    #[test]
    fn every_numeric_operator_round_trips_through_its_token() {
        for operator in [
            FilterOperator::Contains,
            FilterOperator::Equals,
            FilterOperator::StartsWith,
            FilterOperator::EndsWith,
            FilterOperator::NotContains,
            FilterOperator::EqualsNumber,
            FilterOperator::GreaterThan,
            FilterOperator::GreaterOrEqual,
            FilterOperator::LessThan,
            FilterOperator::LessOrEqual,
        ] {
            assert_eq!(
                FilterOperator::from_name(operator.as_str()),
                Some(operator),
                "{operator:?} must round-trip"
            );
        }
        assert_eq!(FilterOperator::from_name("like"), None);
    }

    #[test]
    fn is_numeric_matches_the_operators_that_parse_numbers() {
        for operator in [
            FilterOperator::EqualsNumber,
            FilterOperator::GreaterThan,
            FilterOperator::GreaterOrEqual,
            FilterOperator::LessThan,
            FilterOperator::LessOrEqual,
        ] {
            assert!(operator.is_numeric(), "{operator:?}");
        }
        for operator in [
            FilterOperator::Contains,
            FilterOperator::Equals,
            FilterOperator::StartsWith,
            FilterOperator::EndsWith,
            FilterOperator::NotContains,
        ] {
            assert!(!operator.is_numeric(), "{operator:?}");
        }
    }

    #[test]
    fn all_numeric_operators_compare_as_documented() {
        let cases = [
            (FilterOperator::EqualsNumber, "5", true),
            (FilterOperator::EqualsNumber, "6", false),
            (FilterOperator::GreaterOrEqual, "5", true),
            (FilterOperator::GreaterOrEqual, "6", false),
            (FilterOperator::GreaterOrEqual, "4", true),
            (FilterOperator::LessThan, "6", true),
            (FilterOperator::LessThan, "5", false),
            (FilterOperator::LessOrEqual, "5", true),
            (FilterOperator::LessOrEqual, "4", false),
        ];
        for (operator, operand, expected) in cases {
            let expr = FilterExpr::Predicate(FilterCondition::new(0, operator, operand));
            assert_eq!(expr.accepts(&row("5", "", "")), expected, "{operator:?} {operand} on 5");
        }
    }

    #[test]
    fn and_requires_every_child() {
        let expr = FilterExpr::and(vec![
            FilterExpr::Predicate(FilterCondition::contains(0, "a")),
            FilterExpr::Predicate(FilterCondition::contains(1, "b")),
        ]);
        assert!(expr.accepts(&row("aaa", "bbb", "")));
        assert!(!expr.accepts(&row("aaa", "zzz", "")));
        assert!(!expr.accepts(&row("zzz", "bbb", "")));
    }

    #[test]
    fn or_requires_one_child() {
        let expr = FilterExpr::or(vec![
            FilterExpr::Predicate(FilterCondition::contains(0, "a")),
            FilterExpr::Predicate(FilterCondition::contains(1, "b")),
        ]);
        assert!(expr.accepts(&row("aaa", "zzz", "")));
        assert!(expr.accepts(&row("zzz", "bbb", "")));
        assert!(!expr.accepts(&row("zzz", "zzz", "")));
    }

    #[test]
    fn nesting_expresses_what_a_flat_list_cannot() {
        // (a AND b) OR c — impossible to write as a conjunction of predicates, which
        // is the reason this type exists.
        let expr = FilterExpr::or(vec![
            FilterExpr::and(vec![
                FilterExpr::Predicate(FilterCondition::contains(0, "a")),
                FilterExpr::Predicate(FilterCondition::contains(1, "b")),
            ]),
            FilterExpr::Predicate(FilterCondition::contains(2, "c")),
        ]);
        assert!(expr.accepts(&row("aaa", "bbb", "")));
        assert!(expr.accepts(&row("", "", "ccc")));
        assert!(!expr.accepts(&row("aaa", "zzz", "")));
    }

    #[test]
    fn not_inverts_and_double_negation_folds() {
        let expr = FilterExpr::negate(FilterExpr::Predicate(FilterCondition::contains(0, "a")));
        assert!(!expr.accepts(&row("aaa", "", "")));
        assert!(expr.accepts(&row("zzz", "", "")));

        // not(not(x)) is x, folded at construction so the tree stays small.
        let double = FilterExpr::negate(expr.clone());
        assert_eq!(
            double,
            *match expr {
                FilterExpr::Not(inner) => inner,
                other => unreachable!("expected Not, got {other:?}"),
            }
        );
    }

    #[test]
    fn not_of_the_identities_is_the_other_identity() {
        assert_eq!(FilterExpr::negate(FilterExpr::MatchAll), FilterExpr::MatchNothing);
        assert_eq!(FilterExpr::negate(FilterExpr::MatchNothing), FilterExpr::MatchAll);
    }

    #[test]
    fn an_empty_and_is_match_all_and_an_empty_or_is_match_nothing() {
        // The identities that make the tree composable: removing the last condition
        // must widen, not narrow.
        assert_eq!(FilterExpr::and(Vec::new()), FilterExpr::MatchAll);
        assert_eq!(FilterExpr::or(Vec::new()), FilterExpr::MatchNothing);
        assert!(FilterExpr::and(Vec::new()).accepts(&row("", "", "")));
        assert!(!FilterExpr::or(Vec::new()).accepts(&row("", "", "")));
    }

    #[test]
    fn a_single_child_group_collapses_to_that_child() {
        let predicate = FilterExpr::Predicate(FilterCondition::contains(0, "a"));
        assert_eq!(FilterExpr::and(vec![predicate.clone()]), predicate);
        assert_eq!(FilterExpr::or(vec![predicate.clone()]), predicate);
    }

    #[test]
    fn nested_groups_of_the_same_kind_flatten() {
        // A query builder that appends to an existing group must not grow the tree
        // by a level per edit, and two trees that mean the same thing must compare
        // equal.
        let a = FilterExpr::Predicate(FilterCondition::contains(0, "a"));
        let b = FilterExpr::Predicate(FilterCondition::contains(1, "b"));
        let c = FilterExpr::Predicate(FilterCondition::contains(2, "c"));

        let nested = FilterExpr::and(vec![FilterExpr::and(vec![a.clone(), b.clone()]), c.clone()]);
        let flat = FilterExpr::and(vec![a, b, c]);
        assert_eq!(nested, flat);

        let nested_or = FilterExpr::or(vec![
            FilterExpr::or(vec![
                FilterExpr::Predicate(FilterCondition::contains(0, "a")),
                FilterExpr::Predicate(FilterCondition::contains(1, "b")),
            ]),
            FilterExpr::Predicate(FilterCondition::contains(2, "c")),
        ]);
        assert!(matches!(nested_or, FilterExpr::Or(children) if children.len() == 3));
    }

    #[test]
    fn from_conditions_matches_the_flat_and_semantics() {
        // The conversion from the legacy model: `DataGrid` combined its
        // `Vec<ColumnFilter>` with an implicit AND.
        let expr = FilterExpr::from_conditions(vec![
            FilterCondition::contains(0, "a"),
            FilterCondition::contains(1, "b"),
        ]);
        assert!(matches!(expr, FilterExpr::And(ref children) if children.len() == 2));
        assert!(expr.accepts(&row("aaa", "bbb", "")));
        assert!(!expr.accepts(&row("aaa", "zzz", "")));

        // And the empty case is the no-op the legacy model also meant.
        assert_eq!(FilterExpr::from_conditions(Vec::new()), FilterExpr::MatchAll);
        assert_eq!(
            FilterExpr::from_conditions(vec![FilterCondition::contains(0, "a")]),
            FilterExpr::Predicate(FilterCondition::contains(0, "a"))
        );
    }

    #[test]
    fn condition_count_counts_predicates_through_groups() {
        let expr = FilterExpr::or(vec![
            FilterExpr::and(vec![
                FilterExpr::Predicate(FilterCondition::contains(0, "a")),
                FilterExpr::Predicate(FilterCondition::contains(1, "b")),
            ]),
            FilterExpr::Predicate(FilterCondition::contains(2, "c")),
        ]);
        assert_eq!(expr.condition_count(), 3);
        assert_eq!(FilterExpr::MatchAll.condition_count(), 0);
        assert_eq!(
            FilterExpr::negate(FilterExpr::Predicate(FilterCondition::contains(0, "a")))
                .condition_count(),
            1
        );
    }

    #[test]
    fn conditions_returns_every_predicate_in_reading_order() {
        let expr = FilterExpr::or(vec![
            FilterExpr::and(vec![
                FilterExpr::Predicate(FilterCondition::contains(0, "a")),
                FilterExpr::Predicate(FilterCondition::contains(1, "b")),
            ]),
            FilterExpr::Predicate(FilterCondition::contains(2, "c")),
        ]);
        let operands: Vec<&str> =
            expr.conditions().iter().map(|condition| condition.operand.as_str()).collect();
        assert_eq!(operands, vec!["a", "b", "c"]);
    }

    #[test]
    fn is_noop_identifies_filters_that_cannot_reject() {
        assert!(FilterExpr::MatchAll.is_noop());
        assert!(!FilterExpr::MatchNothing.is_noop());
        assert!(!FilterExpr::Predicate(FilterCondition::contains(0, "a")).is_noop());
        // An AND containing a no-op child is a no-op: that child cannot reject.
        assert!(FilterExpr::and(vec![
            FilterExpr::MatchAll,
            FilterExpr::Predicate(FilterCondition::contains(0, "a")),
        ])
        .is_noop());
        // An OR containing a no-op child is a no-op, because that child accepts all.
        assert!(FilterExpr::or(vec![
            FilterExpr::MatchAll,
            FilterExpr::Predicate(FilterCondition::contains(0, "a")),
        ])
        .is_noop());
        // A negation always rejects something.
        assert!(
            !FilterExpr::negate(FilterExpr::Predicate(FilterCondition::contains(0, "a"))).is_noop()
        );
    }

    #[test]
    fn without_column_drops_the_matching_predicates() {
        let expr = FilterExpr::and(vec![
            FilterExpr::Predicate(FilterCondition::contains(0, "a")),
            FilterExpr::Predicate(FilterCondition::contains(1, "b")),
            FilterExpr::Predicate(FilterCondition::contains(0, "c")),
        ]);
        let pruned = expr.without_column(0);
        assert_eq!(pruned.condition_count(), 1);
        assert_eq!(pruned.conditions()[0].operand, "b");
    }

    #[test]
    fn without_column_widens_rather_than_narrowing() {
        // Removing the only condition must leave "no filter", not "match nothing":
        // the latter would blank the grid the moment a column was hidden.
        let expr = FilterExpr::Predicate(FilterCondition::contains(0, "a"));
        let pruned = expr.without_column(0);
        assert_eq!(pruned, FilterExpr::MatchAll);
        assert!(pruned.accepts(&row("", "", "")));
    }

    #[test]
    fn without_column_drops_a_negation_whose_child_vanished() {
        // `Not(MatchAll)` is `MatchNothing`; that would empty the grid when a column
        // was removed, so the negation goes instead.
        let expr = FilterExpr::negate(FilterExpr::Predicate(FilterCondition::contains(0, "a")));
        let pruned = expr.without_column(0);
        assert!(pruned.is_noop(), "the filter must widen, not reject everything");
    }

    #[test]
    fn without_column_leaves_unrelated_columns_alone() {
        let expr = FilterExpr::Predicate(FilterCondition::contains(0, "a"));
        assert_eq!(expr.without_column(7), expr);
    }

    #[test]
    fn default_is_no_filter() {
        assert_eq!(FilterExpr::default(), FilterExpr::MatchAll);
    }

    #[test]
    fn a_three_level_tree_evaluates_correctly() {
        // ((a AND b) OR (c AND NOT d)) AND e — deep enough to prove the recursion.
        let expr = FilterExpr::and(vec![
            FilterExpr::or(vec![
                FilterExpr::and(vec![
                    FilterExpr::Predicate(FilterCondition::contains(0, "a")),
                    FilterExpr::Predicate(FilterCondition::contains(1, "b")),
                ]),
                FilterExpr::and(vec![
                    FilterExpr::Predicate(FilterCondition::contains(0, "c")),
                    FilterExpr::negate(FilterExpr::Predicate(FilterCondition::contains(1, "d"))),
                ]),
            ]),
            FilterExpr::Predicate(FilterCondition::contains(2, "e")),
        ]);

        // First branch of the OR, plus e.
        assert!(expr.accepts(&row("aaa", "bbb", "eee")));
        // Second branch: c present, d absent, plus e.
        assert!(expr.accepts(&row("ccc", "zzz", "eee")));
        // Second branch fails because d is present.
        assert!(!expr.accepts(&row("ccc", "ddd", "eee")));
        // Neither branch matches.
        assert!(!expr.accepts(&row("zzz", "zzz", "eee")));
        // The AND with e fails.
        assert!(!expr.accepts(&row("aaa", "bbb", "zzz")));
    }
}
