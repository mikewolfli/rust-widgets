//! Layout manager parsing for JSON declarative layouts.
//!
//! Converts JSON `"layout"` objects into concrete [`Layout`](crate::layout::Layout) trait objects.

use crate::compat::HashMap;
use core::cell::RefCell;

use serde_json::Value;

use crate::core::Orientation;
use crate::layout::{BoxLayout, FormLayout, GridLayout, Layout, SplitterLayout, StackLayout};

// ── Layout kind enum ─────────────────────────────────────────

/// Kinds of layout managers understood by the declarative JSON engine.
#[derive(Debug, Clone, PartialEq)]
pub enum DeclarativeLayoutKind {
    /// Horizontal box layout.
    HBox { spacing: u32, margin: u32 },
    /// Vertical box layout.
    VBox { spacing: u32, margin: u32 },
    /// Grid layout.
    Grid { columns: u32, spacing: u32, margin: u32 },
    /// Stack layout (card stack).
    Stack { spacing: u32 },
    /// Splitter layout.
    Splitter { orientation: Orientation, margin: u32 },
    /// Form layout (label-field pairs).
    Form { spacing: u32, margin: u32 },
}

// ── Thread-local layout storage ──────────────────────────────

thread_local! {
    static LAYOUT_MAP: RefCell<HashMap<u64, Box<dyn Layout>>> = RefCell::new(HashMap::new());
}

/// Store a layout manager for a parent widget.
pub fn store_layout(parent_id: u64, layout: Box<dyn Layout>) {
    LAYOUT_MAP.with(|map| {
        map.borrow_mut().insert(parent_id, layout);
    });
}

/// Register a widget as a layout child with its stretch factor.
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

    let spacing = obj.get("spacing").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let margin = obj.get("margin").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    match type_str {
        "hbox" | "HBox" | "horizontal" => Ok(DeclarativeLayoutKind::HBox { spacing, margin }),
        "vbox" | "VBox" | "vertical" => Ok(DeclarativeLayoutKind::VBox { spacing, margin }),
        "grid" | "Grid" => {
            let columns = obj.get("columns").and_then(|v| v.as_u64()).unwrap_or(2) as u32;
            Ok(DeclarativeLayoutKind::Grid { columns, spacing, margin })
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
        _ => Err(format!("unknown layout type: '{}'", type_str)),
    }
}

/// Build a concrete `Layout` trait object from a `DeclarativeLayoutKind`.
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
        DeclarativeLayoutKind::Stack { .. } => Box::new(StackLayout::new()),
        DeclarativeLayoutKind::Splitter { orientation, .. } => {
            Box::new(SplitterLayout::new(orientation, 0))
        }
        DeclarativeLayoutKind::Form { spacing, margin } => {
            Box::new(FormLayout::new(spacing, margin))
        }
    }
}

/// Attributes for a child widget within a layout.
pub struct ChildLayoutAttrs {
    /// Stretch factor (0 = default).
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
}
