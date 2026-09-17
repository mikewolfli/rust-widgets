// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Layout manager parsing for JSON declarative layouts.
//!
//! Converts JSON `"layout"` objects into concrete [`Layout`](crate::layout::Layout) trait objects.

use crate::compat::HashMap;
use core::cell::RefCell;

use serde_json::Value;

use crate::core::{Orientation, Rect};
use crate::layout::{BoxLayout, FormLayout, GridLayout, Layout, SplitterLayout, StackLayout};

// ── Layout kind enum ─────────────────────────────────────────

/// Kinds of layout managers understood by the declarative JSON engine.
#[derive(Debug, Clone, PartialEq)]
pub enum DeclarativeLayoutKind {
    /// Horizontal box layout.
    HBox {
        /// Gap in logical pixels between adjacent children.
        spacing: u32,
        /// Outer inset in logical pixels on all four sides.
        margin: u32,
    },
    /// Vertical box layout.
    VBox {
        /// Gap in logical pixels between adjacent children.
        spacing: u32,
        /// Outer inset in logical pixels on all four sides.
        margin: u32,
    },
    /// Grid layout.
    Grid {
        /// Number of columns; rows are created as needed.
        columns: u32,
        /// Gap in logical pixels between cells.
        spacing: u32,
        /// Outer inset in logical pixels on all four sides.
        margin: u32,
    },
    /// Stack layout (card stack): children overlap, one visible at a time.
    Stack {
        /// Spacing in logical pixels. The stored value is currently ignored: a
        /// stack is constructed with [`StackLayout::new`], which does not
        /// accept a spacing parameter.
        spacing: u32,
    },
    /// Splitter layout: children are separated by user-draggable handles.
    Splitter {
        /// Axis along which the panes are laid out.
        orientation: Orientation,
        /// Margin in logical pixels. The stored value is currently ignored:
        /// the splitter handle width is hard-coded to `0` at construction.
        margin: u32,
    },
    /// Form layout (label-field pairs).
    Form {
        /// Gap in logical pixels between rows.
        spacing: u32,
        /// Outer inset in logical pixels on all four sides.
        margin: u32,
    },
    /// Flow layout: children run along an axis and wrap when the line is full.
    Flow {
        /// Gap in logical pixels between adjacent children.
        spacing: i32,
        /// Padding in logical pixels around the content.
        padding: i32,
        /// Whether children run horizontally (the default) or vertically.
        orientation: Orientation,
    },
    /// Wrap layout: like flow, with an explicit cross-axis alignment.
    Wrap {
        /// Gap in logical pixels between adjacent children.
        spacing: i32,
        /// Padding in logical pixels around the content.
        padding: i32,
        /// Whether children run horizontally (the default) or vertically.
        orientation: Orientation,
    },
    /// Uniform grid: every cell is the same size, unlike [`Self::Grid`].
    UniformGrid {
        /// Number of rows.
        rows: u32,
        /// Number of columns.
        columns: u32,
        /// Gap in logical pixels between cells.
        spacing: u32,
        /// Outer inset in logical pixels on all four sides.
        margin: u32,
    },
    /// Flex layout: main/cross-axis distribution and alignment.
    Flex {
        /// Gap in logical pixels between adjacent children.
        gap: u32,
    },
}

impl DeclarativeLayoutKind {
    /// The JSON type tokens that select this kind.
    ///
    /// Exposed so a diagnostic (and the tests) can enumerate what is supported
    /// without duplicating the list, which is how the previous error message drifted
    /// out of step with the parser.
    pub const SUPPORTED_TYPES: &'static [&'static str] = &[
        "hbox",
        "horizontal",
        "vbox",
        "vertical",
        "grid",
        "uniform_grid",
        "stack",
        "splitter",
        "form",
        "flow",
        "wrap",
        "flex",
    ];
}

// ── Thread-local layout storage ──────────────────────────────

thread_local! {
    static LAYOUT_MAP: RefCell<HashMap<u64, Box<dyn Layout>>> = RefCell::new(HashMap::new());
}

/// Store a layout manager for a parent widget.
///
/// The layout is keyed by `parent_id` in a thread-local map, replacing any
/// layout previously stored for that id. Because the map is thread-local, a
/// layout stored on one thread is invisible to another — declarative layouts
/// are therefore single-threaded by construction.
pub fn store_layout(parent_id: u64, layout: Box<dyn Layout>) {
    LAYOUT_MAP.with(|map| {
        map.borrow_mut().insert(parent_id, layout);
    });
}

/// Register a widget as a layout child with its stretch factor.
///
/// Looks the layout up by `parent_id`, ignoring `_layout` (the caller's view of
/// the same object). Silent no-op when no layout is stored for `parent_id`. The
/// stretch factor is stored verbatim; `0` is not a valid stretch in most layout
/// implementations and may make the child invisible.
pub fn add_widget_to_layout(_layout: &dyn Layout, child_id: u64, stretch: u32, parent_id: u64) {
    LAYOUT_MAP.with(|map| {
        let mut map = map.borrow_mut();
        if let Some(layout_box) = map.get_mut(&parent_id) {
            layout_box.add_widget(child_id, stretch);
        }
    });
}

/// Record a spacer stretch for a parent box layout.
///
/// Box layouts support stretchable spacers. The spacer is resolved
/// from the stored layout for `_parent_id`.
///
/// Because the generic [`Layout`] trait has no spacer method, the spacer is
/// smuggled through as a child whose id is [`u64::MAX`](u64::MAX) — the widest
/// possible id, which no real widget can hold. A layout implementation that
/// does not know this sentinel will treat it as a normal child; in particular
/// the box and grid layouts used by the declarative engine will allocate it
/// space. Silent no-op when no layout is stored for the parent.
pub fn add_spacer_to_layout(_stretch: u32, _parent_id: u64) {
    LAYOUT_MAP.with(|map| {
        let mut map = map.borrow_mut();
        if let Some(layout_box) = map.get_mut(&_parent_id) {
            // BoxLayout subclasses have add_spacer, but through
            // the `dyn Layout` trait we can only add widgets.
            // Phase 2 will lift this limitation with a specialized spacer API.
            //
            // For now, we call add_widget with ObjectId(0) as a sentinel
            // that signals a spacer. Layout implementations that understand
            // this will handle it appropriately.
            layout_box.add_widget(u64::MAX, _stretch);
        }
    });
}

/// Apply a stored declarative layout to its child widget geometries.
///
/// `rect` is the parent's own rectangle; the layout distributes it among the
/// children and each resulting rectangle is written back through
/// [`crate::set_widget_geometry`]. Spacer children ([`u64::MAX`](u64::MAX)) are
/// filtered out and never written to a widget.
///
/// The stored layout is looked up only for this call. Silent no-op when
/// `parent_id` has no stored layout, so applying an unknown parent is not an
/// error. The map borrow is released before geometers are written, so a
/// geometry callback that re-enters the layout engine will not panic on a
/// double borrow.
pub fn apply_layout(parent_id: u64, rect: Rect) {
    let geometries = LAYOUT_MAP.with(|map| {
        let map = map.borrow();
        let Some(layout) = map.get(&parent_id) else {
            return Vec::new();
        };
        let mut geometries = Vec::new();
        layout.update(rect, &mut |child_id, child_rect| {
            if child_id != u64::MAX {
                geometries.push((child_id, child_rect));
            }
        });
        geometries
    });

    for (child_id, child_rect) in geometries {
        crate::set_widget_geometry(
            child_id,
            child_rect.x,
            child_rect.y,
            child_rect.width,
            child_rect.height,
        );
    }
}

// ── Parsing ─────────────────────────────────────────────────

/// Parse a `DeclarativeLayoutKind` from a serde_json `Value` object.
///
/// The expected format:
/// ```json
/// { "type": "hbox", "spacing": 4, "margin": 2 }
/// ```
pub fn parse_layout_kind(value: &Value) -> Result<DeclarativeLayoutKind, String> {
    let obj = value.as_object().ok_or_else(|| "layout must be a JSON object".to_string())?;

    let type_str = obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "layout must have a 'type' field (string)".to_string())?;

    // Both numeric fields are forgiving: a missing value, a non-numeric value,
    // or a negative value all fall back to 0.
    let spacing = obj.get("spacing").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let margin = obj.get("margin").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    match type_str {
        "hbox" | "HBox" | "horizontal" => Ok(DeclarativeLayoutKind::HBox { spacing, margin }),
        "vbox" | "VBox" | "vertical" => Ok(DeclarativeLayoutKind::VBox { spacing, margin }),
        "grid" | "Grid" => {
            let columns = obj.get("columns").and_then(|v| v.as_u64()).unwrap_or(2) as u32;
            Ok(DeclarativeLayoutKind::Grid { columns, spacing, margin })
        }
        "uniform_grid" | "uniformGrid" | "uniform-grid" => {
            let rows = obj.get("rows").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let columns = obj.get("columns").and_then(|v| v.as_u64()).unwrap_or(2) as u32;
            Ok(DeclarativeLayoutKind::UniformGrid { rows, columns, spacing, margin })
        }
        "stack" | "Stack" => Ok(DeclarativeLayoutKind::Stack { spacing }),
        "splitter" | "Splitter" => {
            let orientation = match obj.get("orientation").and_then(|v| v.as_str()) {
                Some("vertical" | "v" | "V") => Orientation::Vertical,
                _ => Orientation::Horizontal,
            };
            Ok(DeclarativeLayoutKind::Splitter { orientation, margin })
        }
        "form" | "Form" => Ok(DeclarativeLayoutKind::Form { spacing, margin }),
        "flow" | "Flow" => {
            // `flow` and `wrap` take signed gaps: the layout accepts a negative
            // spacing (children overlap), so reading these as `u64` would reject a
            // value the layout can honour.
            let spacing = obj.get("spacing").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let padding = obj.get("padding").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            Ok(DeclarativeLayoutKind::Flow {
                spacing,
                padding,
                orientation: parse_axis(obj.get("orientation")),
            })
        }
        "wrap" | "Wrap" => {
            let spacing = obj.get("spacing").and_then(|v| v.as_i64()).unwrap_or(8) as i32;
            let padding = obj.get("padding").and_then(|v| v.as_i64()).unwrap_or(8) as i32;
            Ok(DeclarativeLayoutKind::Wrap {
                spacing,
                padding,
                orientation: parse_axis(obj.get("orientation")),
            })
        }
        "flex" | "Flex" => {
            let gap = obj.get("gap").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            Ok(DeclarativeLayoutKind::Flex { gap })
        }
        _ => Err(format!(
            "unknown layout type '{type_str}'; supported types are {}",
            DeclarativeLayoutKind::SUPPORTED_TYPES.join(", ")
        )),
    }
}

/// Parse a flow/wrap axis from the JSON `orientation` field.
///
/// Horizontal is the default because that is what both layouts default to, so an
/// omitted field and an explicit `"horizontal"` produce the same layout.
fn parse_axis(value: Option<&Value>) -> Orientation {
    match value.and_then(|v| v.as_str()) {
        Some("vertical" | "v" | "V" | "column") => Orientation::Vertical,
        _ => Orientation::Horizontal,
    }
}

/// Build a concrete `Layout` trait object from a `DeclarativeLayoutKind`.
///
/// Not every field of the kind survives the conversion: [`StackLayout`] and
/// [`SplitterLayout`] have no spacing/margin constructor arguments here, so
/// those fields are dropped (see the variant documentation), and a
/// [`DeclarativeLayoutKind::Grid`] is always built with a single row — the row
/// count grows as children are added.
pub fn create_layout_from_kind(kind: &DeclarativeLayoutKind) -> Box<dyn Layout> {
    match *kind {
        DeclarativeLayoutKind::HBox { spacing, margin } => {
            Box::new(BoxLayout::new(Orientation::Horizontal, spacing, margin))
        }
        DeclarativeLayoutKind::VBox { spacing, margin } => {
            Box::new(BoxLayout::new(Orientation::Vertical, spacing, margin))
        }
        DeclarativeLayoutKind::Grid { columns, spacing, margin } => {
            Box::new(GridLayout::new(1, columns, spacing, margin))
        }
        DeclarativeLayoutKind::UniformGrid { rows, columns, spacing, margin } => {
            // Unlike `Grid`, a uniform grid fixes both dimensions: every cell is the
            // same size, which is the whole point of the kind.
            Box::new(crate::layout::UniformGridLayout::new(rows, columns, spacing, margin))
        }
        DeclarativeLayoutKind::Stack { .. } => Box::new(StackLayout::new()),
        DeclarativeLayoutKind::Splitter { orientation, .. } => {
            Box::new(SplitterLayout::new(orientation, 0))
        }
        DeclarativeLayoutKind::Form { spacing, margin } => {
            Box::new(FormLayout::new(spacing, margin))
        }
        DeclarativeLayoutKind::Flow { spacing, padding, orientation } => {
            let config = crate::layout::FlowLayoutConfig {
                direction: match orientation {
                    Orientation::Horizontal => crate::layout::FlowDirection::Horizontal,
                    Orientation::Vertical => crate::layout::FlowDirection::Vertical,
                },
                spacing,
                padding,
                // A flow layout that does not wrap would overflow a narrow parent
                // instead of reflowing, which is the whole reason to choose `flow`
                // over `wrap` in the first place. `wrap: false` behaviour is
                // reachable through the flex layout's no-wrap mode.
                wrap: true,
                ..Default::default()
            };
            Box::new(crate::layout::FlowLayout::with_config(config))
        }
        DeclarativeLayoutKind::Wrap { spacing, padding, orientation } => {
            let direction = match orientation {
                Orientation::Horizontal => crate::layout::WrapDirection::Horizontal,
                Orientation::Vertical => crate::layout::WrapDirection::Vertical,
            };
            Box::new(crate::layout::WrapLayout::new(
                direction,
                crate::layout::WrapAlignment::Start,
                spacing,
                padding,
            ))
        }
        DeclarativeLayoutKind::Flex { gap } => {
            // `with_params` is the constructor that takes a gap; the default
            // `new()` has no gap parameter. The alignment arguments are the enum
            // defaults restated, so a JSON `flex` with only a `gap` behaves like the
            // programmatic default.
            Box::new(crate::layout::FlexLayout::with_params(
                crate::layout::FlexDirection::Row,
                crate::layout::FlexWrap::NoWrap,
                crate::layout::JustifyContent::FlexStart,
                crate::layout::AlignItems::Stretch,
                gap as i32,
                0,
            ))
        }
    }
}

/// Attributes for a child widget within a layout.
///
/// Every field is optional in the JSON sense; see
/// [`ChildLayoutAttrs::from_value`] for the defaults applied when a key is
/// absent.
pub struct ChildLayoutAttrs {
    /// Stretch factor (0 = default).
    ///
    /// The JSON parser defaults this to `1`, not `0`, when the key is absent.
    pub stretch: u32,
    /// For grid layouts: column position.
    pub col: Option<u32>,
    /// For grid layouts: row position.
    pub row: Option<u32>,
    /// For grid layouts: column span.
    pub col_span: Option<u32>,
    /// For grid layouts: row span.
    pub row_span: Option<u32>,
}

impl ChildLayoutAttrs {
    /// Parse layout child attributes from a JSON object.
    ///
    /// A non-object value, or an object without `stretch`, yields a stretch of
    /// `1`. `col`, `row`, `col_span`, and `row_span` stay `None` unless present
    /// as non-negative integers, so a caller must distinguish "unset" from
    /// "explicitly 0".
    pub fn from_value(value: &serde_json::Value) -> Self {
        let obj = value.as_object();
        Self {
            stretch: obj.and_then(|o| o.get("stretch")).and_then(|v| v.as_u64()).unwrap_or(1)
                as u32,
            col: obj.and_then(|o| o.get("col")).and_then(|v| v.as_u64()).map(|v| v as u32),
            row: obj.and_then(|o| o.get("row")).and_then(|v| v.as_u64()).map(|v| v as u32),
            col_span: obj
                .and_then(|o| o.get("col_span"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            row_span: obj
                .and_then(|o| o.get("row_span"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hbox_layout() {
        let json: Value =
            serde_json::from_str(r#"{"type": "hbox", "spacing": 4, "margin": 2}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::HBox { spacing: 4, margin: 2 });
    }

    #[test]
    fn parse_vbox_layout() {
        let json: Value = serde_json::from_str(r#"{"type": "vbox", "spacing": 2}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::VBox { spacing: 2, margin: 0 });
    }

    #[test]
    fn parse_grid_layout() {
        let json: Value =
            serde_json::from_str(r#"{"type": "grid", "columns": 3, "spacing": 2}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::Grid { columns: 3, spacing: 2, margin: 0 });
    }

    #[test]
    fn parse_unknown_layout_returns_error() {
        let json: Value = serde_json::from_str(r#"{"type": "bogus"}"#).unwrap();
        assert!(parse_layout_kind(&json).is_err());
    }

    #[test]
    fn create_hbox_from_kind() {
        let kind = DeclarativeLayoutKind::HBox { spacing: 4, margin: 2 };
        let _layout = create_layout_from_kind(&kind);
        // Verify it creates without error
    }

    #[test]
    fn child_layout_attrs_parses_stretch() {
        let json: Value = serde_json::from_str(r#"{"stretch": 3}"#).unwrap();
        let attrs = ChildLayoutAttrs::from_value(&json);
        assert_eq!(attrs.stretch, 3);
    }

    #[test]
    fn child_layout_attrs_defaults() {
        let json: Value = serde_json::from_str(r#"{}"#).unwrap();
        let attrs = ChildLayoutAttrs::from_value(&json);
        assert_eq!(attrs.stretch, 1);
        assert!(attrs.col.is_none());
    }

    #[test]
    fn parse_layout_not_an_object_error() {
        let json: Value = serde_json::from_str(r#""not an object""#).unwrap();
        let result = parse_layout_kind(&json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be a JSON object"));
    }

    #[test]
    fn parse_layout_missing_type_error() {
        let json: Value = serde_json::from_str(r#"{"spacing": 4}"#).unwrap();
        let result = parse_layout_kind(&json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("'type' field"));
    }

    #[test]
    fn parse_layout_type_not_string_error() {
        let json: Value = serde_json::from_str(r#"{"type": 123}"#).unwrap();
        let result = parse_layout_kind(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_hbox_aliases() {
        let hbox_json: Value = serde_json::from_str(r#"{"type": "HBox", "spacing": 6}"#).unwrap();
        let kind = parse_layout_kind(&hbox_json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::HBox { spacing: 6, margin: 0 });

        let horiz_json: Value =
            serde_json::from_str(r#"{"type": "horizontal", "spacing": 2, "margin": 1}"#).unwrap();
        let kind2 = parse_layout_kind(&horiz_json).unwrap();
        assert_eq!(kind2, DeclarativeLayoutKind::HBox { spacing: 2, margin: 1 });
    }

    #[test]
    fn parse_vbox_aliases() {
        let vbox_json: Value = serde_json::from_str(r#"{"type": "VBox", "margin": 3}"#).unwrap();
        let kind = parse_layout_kind(&vbox_json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::VBox { spacing: 0, margin: 3 });

        let vert_json: Value = serde_json::from_str(r#"{"type": "vertical"}"#).unwrap();
        let kind2 = parse_layout_kind(&vert_json).unwrap();
        assert_eq!(kind2, DeclarativeLayoutKind::VBox { spacing: 0, margin: 0 });
    }

    #[test]
    fn parse_grid_default_columns() {
        let json: Value = serde_json::from_str(r#"{"type": "grid"}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::Grid { columns: 2, spacing: 0, margin: 0 });
    }

    #[test]
    fn parse_stack_layout() {
        let json: Value = serde_json::from_str(r#"{"type": "stack", "spacing": 8}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::Stack { spacing: 8 });
    }

    #[test]
    fn parse_splitter_layout_default_horizontal() {
        let json: Value = serde_json::from_str(r#"{"type": "splitter", "margin": 2}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(
            kind,
            DeclarativeLayoutKind::Splitter { orientation: Orientation::Horizontal, margin: 2 }
        );
    }

    #[test]
    fn parse_splitter_layout_vertical() {
        let json: Value =
            serde_json::from_str(r#"{"type": "splitter", "orientation": "vertical", "margin": 4}"#)
                .unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(
            kind,
            DeclarativeLayoutKind::Splitter { orientation: Orientation::Vertical, margin: 4 }
        );
    }

    #[test]
    fn parse_form_layout() {
        let json: Value =
            serde_json::from_str(r#"{"type": "form", "spacing": 6, "margin": 2}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::Form { spacing: 6, margin: 2 });
    }

    #[test]
    fn create_all_layout_kinds() {
        let kinds = vec![
            DeclarativeLayoutKind::HBox { spacing: 4, margin: 2 },
            DeclarativeLayoutKind::VBox { spacing: 4, margin: 2 },
            DeclarativeLayoutKind::Grid { columns: 3, spacing: 2, margin: 1 },
            DeclarativeLayoutKind::Stack { spacing: 8 },
            DeclarativeLayoutKind::Splitter { orientation: Orientation::Horizontal, margin: 3 },
            DeclarativeLayoutKind::Form { spacing: 6, margin: 2 },
        ];
        for kind in &kinds {
            let _layout = create_layout_from_kind(kind);
            // Verify each creates without error
        }
    }

    #[test]
    fn child_layout_attrs_parses_grid_position() {
        let json: Value = serde_json::from_str(
            r#"{"stretch": 2, "col": 1, "row": 3, "col_span": 2, "row_span": 1}"#,
        )
        .unwrap();
        let attrs = ChildLayoutAttrs::from_value(&json);
        assert_eq!(attrs.stretch, 2);
        assert_eq!(attrs.col, Some(1));
        assert_eq!(attrs.row, Some(3));
        assert_eq!(attrs.col_span, Some(2));
        assert_eq!(attrs.row_span, Some(1));
    }

    #[test]
    fn child_layout_attrs_non_object_value() {
        let json: Value = serde_json::from_str(r#""string value""#).unwrap();
        let attrs = ChildLayoutAttrs::from_value(&json);
        assert_eq!(attrs.stretch, 1);
        assert!(attrs.col.is_none());
        assert!(attrs.row.is_none());
    }

    #[test]
    fn parse_splitter_layout_with_orientation_v_alias() {
        let json: Value =
            serde_json::from_str(r#"{"type": "splitter", "orientation": "v"}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(
            kind,
            DeclarativeLayoutKind::Splitter { orientation: Orientation::Vertical, margin: 0 }
        );
    }

    #[test]
    fn parse_splitter_layout_with_orientation_capital_v() {
        let json: Value =
            serde_json::from_str(r#"{"type": "splitter", "orientation": "V"}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(
            kind,
            DeclarativeLayoutKind::Splitter { orientation: Orientation::Vertical, margin: 0 }
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn parse_grid_with_Grid_alias() {
        let json: Value =
            serde_json::from_str(r#"{"type": "Grid", "columns": 5, "spacing": 3}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::Grid { columns: 5, spacing: 3, margin: 0 });
    }

    #[test]
    fn parse_stack_with_alias() {
        let json: Value = serde_json::from_str(r#"{"type": "Stack"}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::Stack { spacing: 0 });
    }

    #[test]
    fn parse_form_with_alias() {
        let json: Value = serde_json::from_str(r#"{"type": "Form"}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::Form { spacing: 0, margin: 0 });
    }

    // ── Layout kinds added after the initial six ────────────────────────────
    //
    // The library ships 14 layout managers; the parser used to accept six, so a
    // layout the engine could perform was not expressible in a declaration.

    #[test]
    fn parse_flow_layout() {
        let json: Value =
            serde_json::from_str(r#"{"type": "flow", "spacing": 6, "padding": 3}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(
            kind,
            DeclarativeLayoutKind::Flow {
                spacing: 6,
                padding: 3,
                orientation: Orientation::Horizontal
            }
        );
    }

    /// Flow and wrap take **signed** gaps: those layouts honour a negative spacing by
    /// overlapping children, so reading the field as unsigned would reject a value the
    /// layout supports.
    #[test]
    fn flow_layout_accepts_a_negative_spacing() {
        let json: Value = serde_json::from_str(r#"{"type": "flow", "spacing": -4}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        match kind {
            DeclarativeLayoutKind::Flow { spacing, .. } => assert_eq!(spacing, -4),
            other => panic!("expected a flow layout, got {other:?}"),
        }
    }

    #[test]
    fn parse_flow_layout_vertical() {
        let json: Value =
            serde_json::from_str(r#"{"type": "flow", "orientation": "vertical"}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        match kind {
            DeclarativeLayoutKind::Flow { orientation, .. } => {
                assert_eq!(orientation, Orientation::Vertical)
            }
            other => panic!("expected a flow layout, got {other:?}"),
        }
    }

    #[test]
    fn parse_wrap_layout_defaults_to_an_8px_gap() {
        let json: Value = serde_json::from_str(r#"{"type": "wrap"}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(
            kind,
            DeclarativeLayoutKind::Wrap {
                spacing: 8,
                padding: 8,
                orientation: Orientation::Horizontal
            }
        );
    }

    #[test]
    fn parse_uniform_grid_layout() {
        let json: Value = serde_json::from_str(
            r#"{"type": "uniform_grid", "rows": 3, "columns": 4, "spacing": 2, "margin": 1}"#,
        )
        .unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(
            kind,
            DeclarativeLayoutKind::UniformGrid { rows: 3, columns: 4, spacing: 2, margin: 1 }
        );
    }

    #[test]
    fn parse_flex_layout() {
        let json: Value = serde_json::from_str(r#"{"type": "flex", "gap": 5}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(kind, DeclarativeLayoutKind::Flex { gap: 5 });
    }

    /// Every newly accepted kind also builds a real layout object.
    #[test]
    fn the_added_kinds_build_a_layout() {
        for type_str in ["flow", "wrap", "flex", "uniform_grid"] {
            let json: Value =
                serde_json::from_str(&format!(r#"{{"type": "{type_str}"}}"#)).unwrap();
            let kind = parse_layout_kind(&json)
                .unwrap_or_else(|error| panic!("{type_str} must parse: {error}"));
            // Building must not panic and must produce a usable layout: laying out a
            // rect exercises the implementation rather than only the constructor.
            let layout = create_layout_from_kind(&kind);
            layout.update(Rect::new(0, 0, 100, 100), &mut |_id, _rect| {});
        }
    }

    /// The error message lists exactly the supported types, so it cannot drift out of
    /// step with the parser (it used to name a subset).
    #[test]
    fn the_unknown_type_error_lists_every_supported_type() {
        let json: Value = serde_json::from_str(r#"{"type": "bogus"}"#).unwrap();
        let error = parse_layout_kind(&json).expect_err("an unknown type must be rejected");
        for supported in DeclarativeLayoutKind::SUPPORTED_TYPES {
            assert!(error.contains(supported), "the error must name {supported:?}: {error}");
        }
    }

    /// Every required type in the list is actually accepted, checked by parsing it.
    #[test]
    fn every_listed_supported_type_parses() {
        for type_str in DeclarativeLayoutKind::SUPPORTED_TYPES {
            let json: Value =
                serde_json::from_str(&format!(r#"{{"type": "{type_str}"}}"#)).unwrap();
            assert!(
                parse_layout_kind(&json).is_ok(),
                "{type_str} is listed as supported but does not parse"
            );
        }
    }

    /// The added kinds perform a real layout, not merely construct.
    ///
    /// Asserting only that `update` does not panic would pass for a layout that
    /// assigned every child the whole rectangle. This checks the geometry the flow
    /// layout actually produces: with three children in a box narrower than two of
    /// them, wrapping must put them on distinct rows.
    #[test]
    fn flow_layout_really_wraps() {
        let json: Value =
            serde_json::from_str(r#"{"type": "flow", "spacing": 0, "padding": 0}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        let mut layout = create_layout_from_kind(&kind);
        for id in 1..=3u64 {
            layout.add_widget(id, 0);
        }

        let mut rects: Vec<(u64, Rect)> = Vec::new();
        layout.update(Rect::new(0, 0, 90, 200), &mut |id, rect| rects.push((id, rect)));

        assert_eq!(rects.len(), 3, "every child must be placed");
        let rows: std::collections::HashSet<i32> = rects.iter().map(|(_, rect)| rect.y).collect();
        assert!(
            rows.len() >= 2,
            "flow must wrap in a box narrower than its children; got rows {rows:?} from {rects:?}"
        );
    }

    /// A uniform grid fixes both dimensions, unlike the plain grid which grows rows.
    #[test]
    fn uniform_grid_places_children_in_a_fixed_grid() {
        let json: Value = serde_json::from_str(
            r#"{"type": "uniform_grid", "rows": 2, "columns": 2, "spacing": 0, "margin": 0}"#,
        )
        .unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        let mut layout = create_layout_from_kind(&kind);
        for id in 1..=4u64 {
            layout.add_widget(id, 0);
        }

        let mut rects: Vec<(u64, Rect)> = Vec::new();
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| rects.push((id, rect)));

        assert_eq!(rects.len(), 4, "all four cells must be filled");
        // Four cells in a 2x2 grid over 100x100: two distinct x and two distinct y.
        let xs: std::collections::HashSet<i32> = rects.iter().map(|(_, rect)| rect.x).collect();
        let ys: std::collections::HashSet<i32> = rects.iter().map(|(_, rect)| rect.y).collect();
        assert_eq!(xs.len(), 2, "two columns: {rects:?}");
        assert_eq!(ys.len(), 2, "two rows: {rects:?}");
    }
}
