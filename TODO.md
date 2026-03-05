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
- [x] Design and implement ToggleButton widget with checked state and auto-exclusive support
- [x] Design and implement CheckListBox widget with item selection and check state management
- [x] Design and implement DoubleSpinBox widget for double-precision numeric input
- [x] Design and implement Dial widget with rotary control and value signals
- [x] Design and implement Wizard widget for multi-step dialogs
- [x] Design and implement DatePicker widget for date selection
- [x] Design and implement TimePicker widget for time selection
- [x] Design and implement DateTimePicker widget for date and time selection
- [x] Design and implement DirectoryPicker widget for directory selection

#### Medium Priority Widgets
- [x] Design and implement DataView widget for data visualization
- [x] Design and implement PropertyGrid widget for property editing interface
- [x] Design and implement Toolbox widget for tool palette
- [x] Design and implement StackedWidget for stacked notebook
- [x] Design and implement CollapsiblePane widget for collapsible containers
- [x] Design and implement DockWidget widget for dockable panels

#### Low Priority Widgets
- [x] Design and implement WebView widget for web browser integration
- [x] Design and implement ActivityIndicator widget for progress/activity indication
- [x] Design and implement Calendar widget for calendar display and selection
- [x] Design and implement ColumnView widget for column-based data view
- [x] Design and implement UndoView widget for undo/redo stack visualization
- [x] Design and implement CommandLink widget for command link buttons
- [x] Design and implement LCDNumber widget for digital number display
- [x] Design and implement FontComboBox widget for font selection

#### Web Engine Widgets
- [x] Design and implement WebEngineView widget for web content display
- [x] Design and implement WebEnginePage widget for web content management
- [x] Design and implement WebEngineSettings widget for web engine configuration
- [x] Design and implement WebEngineDownloadItem widget for download management
- [x] Design and implement WebEngineCookieStore widget for cookie management
- [x] Design and implement WebEngineWebChannel widget for JavaScript communication
- [x] Design and implement WebEngineFindTextResult widget for text search results
- [x] Design and implement WebEngineNotification widget for web notifications
- [x] Design and implement WebEngineScriptDialog widget for JavaScript dialogs
- [x] Design and implement WebEngineContextMenuRequest widget for context menu handling

## Stage Progress
