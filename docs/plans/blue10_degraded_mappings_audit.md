# BLUE10 R2.4 — NativeControlBackend Degraded Mappings Audit

> Reviewed: 2026-06-08
> Status: Complete
> Classification criteria: "Design" = acceptable architectural simplification; "Needs Backend" = should have dedicated native control

## Audit Table

| # | Control | Current Mapping | Classification | Rationale |
|---|---------|----------------|---------------|-----------|
| 1 | Canvas | → Panel | **Design** | Canvas is fundamentally a custom-paint surface; no native OS control provides a generic drawing canvas. Panel is the correct fallback container. |
| 2 | Table | → Panel | **Design** | Native tables (NSTableView, ListView-GTK) are data-bound and don't map to a generic widget table. Panel provides correct containment. |
| 3 | Grid | → Panel | **Design** | Grid layout is a layout-manager concept, not a native control. Panel correctly serves as the layout container. |
| 4 | Chart | → Panel | **Design** | Charts are custom-rendered visualizations. No OS provides a generic chart native control. Panel is appropriate. |
| 5 | Dial | → Slider | **Needs Backend** | Dial (rotary knob) has distinct interaction semantics from linear Slider. Consider implementing via custom rotation gesture on Panel. |
| 6 | ToolBox | → Panel | **Design** | ToolBox is a container with collapsible sections — a UI pattern, not a native OS control. Panel fallback is appropriate. |
| 7 | Action | → Button | **Design** | Action is an abstract command that triggers via Button. Button is the correct native analog. |
| 8 | ToolButton | → Button | **Design** | ToolButton is semantically a Button with toolbar styling. No OS distinguishes ToolButton from Button at the native level. |
| 9 | ContextMenu | → Menu | **Design** | ContextMenu is a Menu shown at a specific position. Menu is the correct native primitive. |

## Summary

| Classification | Count | Items |
|---------------|-------|-------|
| **Design** (acceptable) | 8 | Canvas, Table, Grid, Chart, ToolBox, Action, ToolButton, ContextMenu |
| **Needs Backend** | 1 | Dial → consider custom rotary gesture implementation |
