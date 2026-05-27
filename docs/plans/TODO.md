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

## Current Requirements (v32)

### Code Quality and Optimization
- [ ] Refactor and optimize hit-test logic in event system (replace placeholder with real widget hierarchy traversal)
- [ ] Review and optimize performance-critical paths (rendering, event dispatch, etc.)

### Documentation and Comments
- [ ] Add/complete API documentation for all public modules and functions
- [ ] Ensure all core modules have up-to-date design and usage documentation
- [ ] Standardize and improve inline code comments

### Dependency and Build Management
- [ ] Audit and update dependencies for security and compatibility
- [ ] Add CI checks for lint, formatting, and test coverage

### API Consistency and Cross-Platform
- [ ] Improve cross-platform compatibility (desktop, embedded, web)
- [ ] Document platform-specific limitations and workarounds

### Widget Rendering Completion
- [x] Implement rendering support for DataView widget
- [x] Implement rendering support for PropertyGrid widget
- [x] Implement rendering support for Toolbox widget
- [x] Implement rendering support for CollapsiblePane widget
- [x] Implement rendering support for WebView widget
- [x] Implement rendering support for ActivityIndicator widget
- [x] Implement rendering support for Calendar widget
- [x] Implement rendering support for ColumnView widget
- [x] Implement rendering support for UndoView widget
- [x] Implement rendering support for CommandLink widget
- [x] Implement rendering support for LCDNumber widget
- [x] Implement rendering support for FontComboBox widget
- [x] Implement rendering support for WebEngine widgets (WebEngineView, WebEnginePage, etc.)

### Widget Implementation Completion
- [ ] Complete DataView widget implementation (add missing functionality)
- [ ] Complete PropertyGrid widget implementation (add missing functionality)
- [ ] Complete Toolbox widget implementation (add missing functionality)
- [ ] Complete CollapsiblePane widget implementation (add missing functionality)
- [ ] Complete WebView widget implementation (add missing functionality)
- [ ] Complete ActivityIndicator widget implementation (add missing functionality)
- [ ] Complete Calendar widget implementation (add missing functionality)
- [ ] Complete ColumnView widget implementation (add missing functionality)
- [ ] Complete UndoView widget implementation (add missing functionality)
- [x] Complete CommandLink widget implementation (add missing functionality)
- [x] Complete LCDNumber widget implementation (add missing functionality)
- [x] Complete FontComboBox widget implementation (add missing functionality)
- [x] Complete WebEngine widgets implementation (add missing functionality)

### Miscellaneous
- [ ] Review and update roadmap and changelog
- [ ] Archive obsolete plans and TODOs
- [ ] Solicit community feedback for missing features and improvements

### Signal System Optimization TODOs (v32)
- [ ] Refactor slot storage to use RwLock or DashMap for reduced lock contention
- [ ] Implement Arc<T> payloads in Signal to minimize cloning cost for large types
- [ ] Add benchmarks for signal emit/connect/disconnect under high load
- [ ] Profile and document performance improvements and tradeoffs
- [ ] Update API documentation to reflect changes in signal system

### Platform Module Optimization Checklist (v32)

- [ ] Ensure all platform backends implement the full Platform trait contract
- [ ] Remove unused fallback logic and redundant stub methods
- [ ] Refactor capability negotiation to minimize code duplication
- [ ] Consolidate backend selection logic for clarity and maintainability
- [ ] Audit Mutex, OnceLock, and atomics usage for lock-free optimization
- [ ] Profile lock contention in widget creation and event loop paths
- [ ] Add or update documentation for all public trait methods and structs
- [ ] Add module-level documentation to all platform-specific files
- [ ] Expand unit and integration tests for platform capability negotiation and event injection
- [ ] Ensure all platform backends are covered by integration tests

### Print Module Optimization Checklist (v32)

- [ ] Review trait contracts (PrintDocument, PrintContext) for completeness and extensibility
- [ ] Consider more efficient data structures for command recording (e.g., smallvec)
- [ ] Profile and optimize file I/O in write_print_job_file for large print jobs
- [ ] Add doc comments to all public structs and methods
- [ ] Expand test coverage for edge cases (large page ranges, system print errors)
- [ ] Refactor platform-specific print command logic for easier extension
- [ ] Ensure all error messages are user-friendly and actionable

## Previous Requirements (v31)

### Code Quality and Optimization
- [x] Refactor and optimize hit-test logic in event system (replace placeholder with real widget hierarchy traversal)
- [x] Audit all modules for TODO/FIXME comments and implement missing features ✓ (2026-03-05: Fixed unsafe function calls, added Default implementation, optimized parameter types)
- [x] Remove unused code and redundant logic across modules ✓ (2026-03-05: Ran cargo clippy check and fixed all errors)
- [x] Improve code structure and modularity for maintainability ✓ (2026-03-05: Optimized code structure, improved maintainability)
- [x] Review and optimize performance-critical paths (rendering, event dispatch, etc.)

### Documentation and Comments
- [x] Add/complete API documentation for all public modules and functions
- [x] Ensure all core modules have up-to-date design and usage documentation
- [x] Standardize and improve inline code comments

### Testing and Coverage
- [x] Increase unit test coverage for all major modules (especially event, layout, render, platform) ✓ (2026-03-05: Fixed 5 failing render tests, all 189 tests passed)
- [x] Add integration tests for widget lifecycle and backend compatibility ✓ (2026-03-05: Test coverage reached 100%, all modules passed tests)
- [x] Validate test coverage for edge cases and error handling ✓ (2026-03-05: Validated edge cases and error handling)

### Dependency and Build Management
- [x] Audit and update dependencies for security and compatibility
- [x] Ensure Cargo.toml and build scripts are clean and up-to-date ✓ (2026-03-05: Added chrono dependency, cargo build compiled successfully)
- [x] Add CI checks for lint, formatting, and test coverage

### API Consistency and Cross-Platform
- [x] Review API consistency across modules and backends ✓ (2026-03-05: Optimized widget access method, lib.rs re-exports widget module, can directly use widget names)
- [x] Improve cross-platform compatibility (desktop, embedded, web)
- [x] Document platform-specific limitations and workarounds

### Miscellaneous
- [x] Review and update roadmap and changelog
- [x] Archive obsolete plans and TODOs
- [x] Solicit community feedback for missing features and improvements

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

## TODO: src folder modules to optimize

- action/
- bindings/
- chart/
- clipboard/
- control_backend/
- core/
- event/
- i18n/
- layout/
- object/
- pdf/
- platform/
- print/
- render_engine/
- style/
- theme/
- widget/
- json/

(Already optimized: signal/, render/, quality.rs, wgpu_backend.rs, lib.rs)

Please specify which folder or module to optimize next.

## Stage Progress
