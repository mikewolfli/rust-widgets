# rust_widgets Roadmap TODO

This file mirrors staged execution status.

## Maintenance Rule (Required)

- New requirements are always added at the top under the latest version section.
- Older requirement sets are assigned a version tag (`v1`, `v2`, ...), moved downward, and kept as history.
- Status updates must be done in both this file and the live task panel.
- If old version has no completed line, please add the new todo list to current version requirement list.
- All controls must be implemented with complete runtime behavior (create/state/events/data path) for supported backends; do not ship minimal placeholder implementations.
- Do not satisfy control requirements with visual-only stubs or edit/button fallback substitutions; missing capabilities must be explicit (`unsupported`/`0`) and tracked as pending work.
- Embedded runtime path must evolve to full-weight implementation parity; embedded-lite behavior is transitional only and must be tracked with closure tasks.

## Current Requirements (v30)

### Extended Widget Set Implementation

#### High Priority Widgets
- [ ] Design and implement ToggleButton widget with checked state and auto-exclusive support
- [ ] Design and implement CheckListBox widget with item selection and check state management
- [ ] Design and implement DoubleSpinBox widget for double-precision numeric input
- [ ] Design and implement Dial widget with rotary control and value signals
- [ ] Design and implement Wizard and WizardPage widgets for multi-step dialogs
- [ ] Design and implement DatePicker widget for date selection
- [ ] Design and implement TimePicker widget for time selection
- [ ] Design and implement DirectoryPicker widget for directory selection

#### Medium Priority Widgets
- [ ] Design and implement DataView widget for data visualization
- [ ] Design and implement PropertyGrid widget for property editing interface
- [ ] Design and implement Header widget for column headers
- [ ] Design and implement Listbook widget for list-based notebook
- [ ] Design and implement Treebook widget for tree-based notebook
- [ ] Design and implement Choicebook widget for choice-based notebook
- [ ] Design and implement Toolbox widget for tool palette
- [ ] Design and implement StackedWidget for stacked notebook
- [ ] Design and implement CollapsiblePane widget for collapsible containers

#### Low Priority Widgets
- [ ] Design and implement HTMLListBox widget for HTML rendering list
- [ ] Design and implement WebView widget for web browser integration
- [ ] Design and implement Animation widget for animation control
- [ ] Design and implement ActivityIndicator widget for progress/activity indication
- [ ] Design and implement Calendar widget for calendar display and selection
- [ ] Design and implement ColumnView widget for column-based data view
- [ ] Design and implement UndoView widget for undo/redo stack visualization
- [ ] Design and implement DataWidgetMapper widget for data mapping
- [ ] Design and implement CommandLink widget for command link buttons
- [ ] Design and implement KeySequenceEdit widget for keyboard shortcut editing
- [ ] Design and implement LCDNumber widget for digital number display
- [ ] Design and implement FontComboBox widget for font selection
- [ ] Design and implement ToolBox widget for toolbox functionality

## Stage Progress
