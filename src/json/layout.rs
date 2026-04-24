//! Layout manager parsing for JSON declarative layouts.
//!
//! Converts JSON `"layout"` objects into concrete [`Layout`](crate::layout::Layout) trait objects.

use std::cell::RefCell;
use std::collections::HashMap;

use serde_json::Value;

use crate::core::Orientation;
use crate::layout::{
    FormLayout, GridLayout, HBoxLayout, Layout, SplitterLayout, StackLayout, VBoxLayout,
};

// ── Layout kind enum ─────────────────────────────────────────

/// Kinds of layout managers understood by the declarative JSON engine.
#[derive(Debug, Clone, PartialEq)]
pub enum DeclarativeLayoutKind {
    /// Horizontal box layout.
    HBox { spacing: u32, margin: u32 },
    /// Vertical box layout.
    VBox { spacing: u32, margin: u32 },
    /// Grid layout.
    Grid {
        columns: u32,
        spacing: u32,
        margin: u32,
    },
    /// Stack layout (card stack).
    Stack { spacing: u32 },
    /// Splitter layout.
    Splitter {
        orientation: Orientation,
        margin: u32,
    },
    /// Form layout (label-field pairs).
    Form { spacing: u32, margin: u32 },
}

// ── Thread-local layout storage ──────────────────────────────

thread_local! {
    static LAYOUT_MAP: RefCell<HashMap<u64, Box<dyn Layout>>> = RefCell::new(HashMap::new());
}

/// Store a layout manager for a parent widget.
pub fn store_layout(
    parent_id: u64,
    layout: Box<dyn Layout>,
    _registry: &mut crate::index::WidgetRegistry,
) {
    LAYOUT_MAP.with(|map| {
        map.borrow_mut().insert(parent_id, layout);
    });
}

/// Register a widget as a layout child with its stretch factor.
pub fn add_widget_to_layout(
    _layout: &dyn Layout,
    child_id: u64,
    stretch: u32,
    parent_id: u64,
    _registry: &mut crate::index::WidgetRegistry,
) {
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
pub fn add_spacer_to_layout(
    _stretch: u32,
    _parent_id: u64,
    _registry: &mut crate::index::WidgetRegistry,
) {
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

/// Add a widget to a grid layout with position/size attributes.
pub fn add_widget_to_layout_grid(
    _layout: &dyn Layout,
    child_id: u64,
    stretch: u32,
    _col: u32,
    _row: u32,
    _col_span: Option<u32>,
    _row_span: Option<u32>,
    parent_id: u64,
    registry: &mut crate::index::WidgetRegistry,
) {
    add_widget_to_layout(_layout, child_id, stretch, parent_id, registry);
}

// ── Parsing ─────────────────────────────────────────────────

/// Parse a `DeclarativeLayoutKind` from a serde_json `Value` object.
///
/// The expected format:
/// ```json
/// { "type": "hbox", "spacing": 4, "margin": 2 }
/// ```
pub fn parse_layout_kind(value: &Value) -> Result<DeclarativeLayoutKind, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "layout must be a JSON object".to_string())?;

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
            Ok(DeclarativeLayoutKind::Grid {
                columns,
                spacing,
                margin,
            })
        }
        "stack" | "Stack" => Ok(DeclarativeLayoutKind::Stack { spacing }),
        "splitter" | "Splitter" => {
            let orientation = match obj.get("orientation").and_then(|v| v.as_str()) {
                Some("vertical" | "v" | "V") => Orientation::Vertical,
                _ => Orientation::Horizontal,
            };
            Ok(DeclarativeLayoutKind::Splitter {
                orientation,
                margin,
            })
        }
        "form" | "Form" => Ok(DeclarativeLayoutKind::Form { spacing, margin }),
        _ => Err(format!("unknown layout type: '{}'", type_str)),
    }
}

/// Build a concrete `Layout` trait object from a `DeclarativeLayoutKind`.
pub fn create_layout_from_kind(kind: &DeclarativeLayoutKind) -> Box<dyn Layout> {
    match *kind {
        DeclarativeLayoutKind::HBox { spacing, margin } => {
            Box::new(HBoxLayout::new(spacing, margin))
        }
        DeclarativeLayoutKind::VBox { spacing, margin } => {
            Box::new(VBoxLayout::new(spacing, margin))
        }
        DeclarativeLayoutKind::Grid {
            columns,
            spacing,
            margin,
        } => Box::new(GridLayout::new(1, columns, spacing, margin)),
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
            stretch: obj
                .and_then(|o| o.get("stretch"))
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as u32,
            col: obj
                .and_then(|o| o.get("col"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            row: obj
                .and_then(|o| o.get("row"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
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

/// Information about a stored layout, used by the layout inspector.
#[allow(dead_code)]
pub struct LayoutSnapshot {
    /// Parent widget id.
    pub parent_id: u64,
    /// Number of children in the layout.
    pub item_count: usize,
    /// Human-readable layout type name.
    pub layout_type: String,
}

/// Return the item count for a stored layout by inspecting its concrete type.
///
/// Uses the `as_any()` method on the `Layout` trait for safe downcasting.
#[allow(dead_code)]
fn inspect_layout_item_count(layout: &dyn Layout) -> (usize, &'static str) {
    let any = layout.as_any();

    if let Some(box_layout) = any.downcast_ref::<crate::layout::BoxLayout>() {
        return (box_layout.item_count(), "BoxLayout");
    }
    if let Some(grid) = any.downcast_ref::<crate::layout::GridLayout>() {
        return (grid.cell_count(), "GridLayout");
    }
    if let Some(stack) = any.downcast_ref::<crate::layout::StackLayout>() {
        return (stack.item_count(), "StackLayout");
    }
    if let Some(splitter) = any.downcast_ref::<crate::layout::SplitterLayout>() {
        return (splitter.pane_count(), "SplitterLayout");
    }
    if let Some(form) = any.downcast_ref::<crate::layout::FormLayout>() {
        return (form.row_count(), "FormLayout");
    }

    (0, "UnknownLayout")
}

/// Collect snapshots of all layouts stored in `LAYOUT_MAP`.
///
/// This is the integration point between the JSON layout system and
/// the LayoutInspector. Call it after `JsonLoader::load()` when the
/// inspector is enabled.
#[allow(dead_code)]
pub fn collect_layout_snapshots() -> Vec<LayoutSnapshot> {
    let mut snapshots = Vec::new();
    LAYOUT_MAP.with(|map| {
        for (parent_id, layout) in map.borrow().iter() {
            let (item_count, layout_type) = inspect_layout_item_count(layout.as_ref());
            snapshots.push(LayoutSnapshot {
                parent_id: *parent_id,
                item_count,
                layout_type: layout_type.to_string(),
            });
        }
    });
    snapshots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hbox_layout() {
        let json: Value =
            serde_json::from_str(r#"{"type": "hbox", "spacing": 4, "margin": 2}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(
            kind,
            DeclarativeLayoutKind::HBox {
                spacing: 4,
                margin: 2
            }
        );
    }

    #[test]
    fn parse_vbox_layout() {
        let json: Value = serde_json::from_str(r#"{"type": "vbox", "spacing": 2}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(
            kind,
            DeclarativeLayoutKind::VBox {
                spacing: 2,
                margin: 0
            }
        );
    }

    #[test]
    fn parse_grid_layout() {
        let json: Value =
            serde_json::from_str(r#"{"type": "grid", "columns": 3, "spacing": 2}"#).unwrap();
        let kind = parse_layout_kind(&json).unwrap();
        assert_eq!(
            kind,
            DeclarativeLayoutKind::Grid {
                columns: 3,
                spacing: 2,
                margin: 0
            }
        );
    }

    #[test]
    fn parse_unknown_layout_returns_error() {
        let json: Value = serde_json::from_str(r#"{"type": "bogus"}"#).unwrap();
        assert!(parse_layout_kind(&json).is_err());
    }

    #[test]
    fn create_hbox_from_kind() {
        let kind = DeclarativeLayoutKind::HBox {
            spacing: 4,
            margin: 2,
        };
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
}
