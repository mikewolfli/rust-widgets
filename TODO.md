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

## Current Requirements (v31)

### Code Quality and Optimization
- [ ] Refactor and optimize hit-test logic in event system (replace placeholder with real widget hierarchy traversal)
- [ ] Audit all modules for TODO/FIXME comments and implement missing features
- [ ] Remove unused code and redundant logic across modules
- [ ] Improve code structure and modularity for maintainability
- [ ] Review and optimize performance-critical paths (rendering, event dispatch, etc.)

### Documentation and Comments
- [ ] Add/complete API documentation for all public modules and functions
- [ ] Ensure all core modules have up-to-date design and usage documentation
- [ ] Standardize and improve inline code comments

### Testing and Coverage
- [ ] Increase unit test coverage for all major modules (especially event, layout, render, platform)
- [ ] Add integration tests for widget lifecycle and backend compatibility
- [ ] Validate test coverage for edge cases and error handling

### Dependency and Build Management
- [ ] Audit and update dependencies for security and compatibility
- [ ] Ensure Cargo.toml and build scripts are clean and up-to-date
- [ ] Add CI checks for lint, formatting, and test coverage

### API Consistency and Cross-Platform
- [ ] Review API consistency across modules and backends
- [ ] Improve cross-platform compatibility (desktop, embedded, web)
- [ ] Document platform-specific limitations and workarounds

### Miscellaneous
- [ ] Review and update roadmap and changelog
- [ ] Archive obsolete plans and TODOs
- [ ] Solicit community feedback for missing features and improvements

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

### Render & Render Engine Review/Optimization (v31)
- [x] Review `render` and `render_engine` modules for correctness and optimization
- [x] Apply performance improvements to pixel ops
- [x] Refactor redundant geometry checks
- [x] Improve error handling in GPU backend
- [x] Integrate vector font renderer
- [x] Add benchmarks for performance-critical paths
- [x] Evaluate thread safety and lock-free optimizations
    - Summary: Thread safety is managed via Mutex, OnceLock, Arc, and atomics for global state and engine data. No locking in pixel hot paths. Atomics are used for counters. For further optimization, profile lock contention, prefer atomics for simple state, and consider lock-free structures if bottlenecks are found. Current usage is safe and appropriate for most cases.

## Stage Progress
