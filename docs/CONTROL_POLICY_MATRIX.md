# Control Policy Matrix

This document defines compile-time control routing policies and the v1 widget-kind matrix.

## Policy Presets

- `control-policy-native-strict`
  - Enables: `controls-native`
  - Goal: maximize native route usage; unsupported advanced controls must be explicit.
- `control-policy-hybrid` (default in `full`)
  - Enables: `controls-native`, `controls-custom`
  - Goal: native-first for basic/intermediate controls, custom-required for advanced controls.
- `control-policy-custom-full`
  - Enables: `controls-custom`
  - Goal: custom route for all supported controls.

## v1 Widget Route Matrix

### NativePreferred

- `Window`, `Dialog`, `MessageBox`, `FileDialog`, `ColorDialog`, `FontDialog`, `PopupWindow`
- `Button`, `CheckBox`, `RadioButton`, `Label`, `LineEdit`
- `ComboBox`, `SpinBox`, `ListBox`, `ProgressBar`, `Slider`
- `ScrollBar`, `ScrollArea`, `Panel`, `GroupBox`, `TabWidget`, `Splitter`, `StackWidget`
- `MenuBar`, `Menu`, `ToolBar`, `StatusBar`

### CustomRequired

- `TextEdit`, `RichEdit`
- `ListView`, `TreeView`, `Table`
- `DockPanel`, `MdiArea`
- `Canvas`, `Grid`, `Chart`

## Event + GPU Coordination

- Event semantics are unified at `WidgetTriggerEvent` level (source-agnostic).
- In hybrid mode, route selection is per widget kind; event shape remains unchanged across native/custom sources.
- GPU is applied to custom/render-heavy paths; deterministic CPU render-backend fallback remains required.

## Code Entry Points

- Policy label: `control_backend::active_control_policy()`
- Route preference: `control_backend::route_preference_for_widget_kind(...)`
- Routed backend selector: `control_backend::get_control_backend_for_widget(...)`
- Compatibility selector: `control_backend::get_control_backend()`
