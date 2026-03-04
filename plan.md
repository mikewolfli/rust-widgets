---
# rust-widgets Development Plan

## Rules

1. This file uses hierarchical TODO management, listing each top-level ITEM one by one.
2. Unfinished items are left blank; completed items are marked "Completed" with a timestamp (e.g., Completed 2026-03-02).
3. Deleted or canceled items are marked "Canceled" or "Deleted" with a timestamp.
4. After all todo list are finished, review and check if there are gaps, added them after current version todo list. 

## ITEM List

- [x] Signal/Slot System (library core) — Completed 2026-03-02 (100%)
	- Generic Signal<T> supporting any number/type of parameters
	- connect(callback): bind closures
	- emit(args): trigger signal
	- Multiple slots per signal
	- Automatic disconnect on widget drop (no dangling, no panic)
	- once: trigger once then disconnect
	- No manual handle management, no raw pointers
	- Pure Rust generics, compile-time safety
	- All widget interactions use signals (clicked, text_changed, selection_changed, closed, etc.)
	- No alternative event system

- [x] Basic Geometry & Style Types — Completed 2026-03-02 (100%)
	- Point (x, y)
	- Size (w, h)
	- Rect (x, y, w, h)
	- Color (rgba)
	- Font (name, size, weight)
	- Margin/Padding (per side)
	- Alignment (left/center/right, top/center/bottom)

 [x] Widget Base Class — Completed 2026-03-04 (100%)
	- Position/size/rect methods
	- Show/hide, enable/disable
	- Min/max size constraints
	- Background/foreground/border/font
	- Mouse/keyboard/focus signals (hover, mouse_down/up, key_down/up, focus_gained/lost)
	- Redraw/layout signals
	- All input as signals, no virtual functions or inheritance magic
	- Gap: unify visible show/hide semantics between widget state and native/custom backend (Completed)
	- Gap: deterministic focus-owner routing and pointer-capture handoff (Completed)
	- Gap: backend-parity regression tests for base-widget enable/visible/focus transitions (Completed)

 [x] Basic Widgets — Completed 2026-03-04 (100%)
	- Button: text/icon, clicked signal, press/release/disable states
	- Label: text/image, word wrap, alignment
	- LineEdit: text_changed, return_pressed, selection/copy/cut/paste, password mode
	- CheckBox: toggled signal, tri-state
	- RadioButton: group logic, selected signal
	- ComboBox: index_changed, dropdown
	- SpinBox/Slider: value_changed
	- ProgressBar: set/get value
	- Gap: custom-backend create/state/event parity for ComboBox/ListBox/SpinBox (Completed)
	- Gap: LineEdit IME/selection/clipboard parity under custom control backend (Completed)
	- Gap: end-to-end demos/tests validating typed trigger ordering for all basic widgets (Completed)

 [x] Layout System — Completed 2026-03-04 (100%)
	 - HBox (horizontal)
	 - VBox (vertical)
	 - Grid (grid)
	 - Stack (stacked)
	 - Auto position/size calculation
	 - Stretch factors, spacing, margins
	 - Gap: bind layout recompute results to runtime-visible widget geometry updates in one unified path (Completed)
	 - Gap: add deterministic relayout on parent resize and constraint change for nested layouts (Completed)
	 - Gap: align layout demos with explicit runtime visibility intent matrix and diagnostics (Completed)
	- HBox (horizontal)
	- VBox (vertical)
	- Grid (grid)
	- Stack (stacked)
	- Auto position/size calculation
	- Stretch factors, spacing, margins
	- Gap: bind layout recompute results to runtime-visible widget geometry updates in one unified path
	- Gap: add deterministic relayout on parent resize and constraint change for nested layouts
	- Gap: align layout demos with explicit runtime visibility intent matrix and diagnostics

 [x] Action System — Completed 2026-03-04 (100%)
   - Shared logic for menu/button/shortcut
   - triggered signal
   - Checkable, enable/disable
   - Gap: complete cross-platform shortcut dispatch parity through one action-trigger source (Completed)
   - Gap: finalize action ownership/lifetime contract across menu/toolbar/button hosts (Completed)
   - Gap: add action regression suite for checkable-state sync and enabled-state propagation (Completed)

 [x] Intermediate Widgets — Completed 2026-03-04 (100%)
	- ScrollArea/ScrollBar
	- GroupBox
	- TabWidget
	- Splitter
	- MenuBar/Menu/ToolBar
	- StatusBar
	- Dialog/MessageBox/FileDialog/ColorDialog/FontDialog
	- Gap: finish dialog host behavior parity (modal lifecycle, return-value contract, close semantics) (Completed)
	- Gap: complete menu/toolbar/statusbar trigger and host-attachment parity under custom backend (Completed)
	- Gap: replace preview-only dialog paths with explicit supported/unsupported capability contracts per backend (Completed)

[x] Model/View Architecture — Completed 2026-03-04 (100%)
	- ListModel/TableModel/TreeModel
	- Data change signals
	- Auto view refresh
	- Gap: complete incremental change notifications (insert/remove/update ranges) and deterministic view refresh behavior (Completed)
	- Gap: finalize editable model contract with typed commit/cancel flows (Completed)
	- Gap: add backend-agnostic data-path parity tests for model binding + selection synchronization (Completed)

[x] Advanced Widgets — Completed 2026-03-04 (100%)
	- TreeView/TableView/ListView
	- RichEdit (code editor)
	- DockPanel
	- MdiArea (multi-document)
	- Chart
	- Gap: complete interactive parity for TreeView/TableView/ListView (hit-test, keyboard navigation, selection ranges) (Completed)
	- Gap: extend RichEdit from baseline to code-editor level features (undo/redo, multi-cursor readiness, syntax hooks) (Completed)
	- Gap: integrate DockPanel/MdiArea with runtime window-management contracts and persistent layout restore (Completed)

[x] Style & Theme System — Completed 2026-03-04 (100%)
	- Dark/light mode
	- QSS-like stylesheet. -RWSS
	- Widget skinning
	- Gap: propagate runtime theme switch to all active widgets with deterministic repaint sequencing (Completed)
	- Gap: implement RWSS parser + selector resolution pipeline and document supported selector grammar (Completed)
	- Gap: complete per-widget skin token mapping and fallback rules across native/custom backends (Completed)

[x] Animation System — Completed 2026-03-04 (100%)
	- Property animation
	- Easing functions
	- Animation composition
	- Gap: add core timeline/clock scheduler and animation lifecycle state machine (Completed)
	- Gap: implement easing catalog and interpolation primitives for numeric/color/rect properties (Completed)
	- Gap: bind animation ticks into render/event loop with deterministic frame-step testing (Completed)

[x] Window & Main Framework — Completed 2026-03-04 (100%)
	- Window/MainWindow
	- Modal window
	- Frameless window
	- Gap: finalize MainWindow abstraction contract (menu/tool/status dock ownership and resize lifecycle) (Completed)
	- Gap: complete modal window behavior parity across native and custom control routes (Completed)
	- Gap: implement frameless window interaction contract (drag/resize/hit regions) with backend diagnostics (Completed)

[x] Dual Control Backend & Event Routing (Native + Custom Paint) — Completed 2026-03-04 (100%)
	- Current status snapshot (2026-03-03)
		- Done: unified `ControlBackend` scaffold is landed (`native` + `custom` kinds)
		- Done: compile-time policy presets are defined (`native-strict` / `hybrid` / `custom-full`)
		- Done: `full` profile defaults to hybrid policy (`controls-native` + `controls-custom`)
		- Done: v1 widget-kind route matrix is landed (`NativePreferred` + `CustomRequired`)
		- Done: compile-time selection features added (`controls-native` / `controls-custom`)
		- Done: event source routing scaffold landed (platform source + control-backend source)
		- Decision: use policy + phase route (`hybrid` for rollout, `custom-full` as final)
		- Done: custom controls completed for full-weight parity
	- Gap: complete custom backend full-weight create/state/event/data-path parity for supported controls (Completed)
	- Gap: close IME/accessibility routing parity plan (retain platform bridge where required, document contract) (Completed)
	- Gap: add explicit unsupported diagnostics matrix + parity gates for remaining uncovered controls (Completed)
	- Policy matrix (compile-time)
		- `control-policy-native-strict`: basic/intermediate controls use native backend only; unsupported advanced controls must be explicit
		- `control-policy-hybrid` (default): basic controls native-first, advanced controls migrate to custom paint path
		- `control-policy-custom-full`: all supported controls resolved by custom backend + unified GPU/CPU render-backend strategy
	- Phase A (hybrid policy stabilization + event unification) — Completed
		- Feature combo: `control-policy-hybrid,gpu-wgpu`
		- Base-control native behavior and event semantics unified
		- GPU only used for custom/render-heavy surfaces, CPU backend can fallback
		- hybrid profile checks, routing parity tests, diagnostics passed
	- Phase B (custom-full + GPU primary path) — Completed
		- Feature combo: `control-policy-custom-full,gpu-wgpu`
		- High-frequency redraw/animation/complex scene controls migrated to custom+GPU path
		- CPU backend can fallback, uncovered controls have explicit diagnostics
		- custom-full profile checks, parity tests, render-backend fallback checks passed
	- Custom full-weight closure criteria
		- deterministic hit-test, pointer capture, focus owner routing (Completed)
		- keyboard/IME/accessibility event surface parity (Completed)
		- create/state/event/data-path parity for supported controls (Completed)
		- explicit unsupported diagnostics for uncovered controls (Completed)

- [ ] IDE Capabilities (final goal) — 22%
	- Multi-window
	- Dock panels
	- Code editor
	- Menu/toolbar/statusbar
	- Plugin system
	- AI Assist integration
	- Gap: define plugin API boundary (lifecycle, command surface, sandbox/ABI strategy)
	- Gap: deliver multi-window + docking orchestration atop stable MainWindow contracts
	- Gap: integrate code-editor workflow primitives (project tree, diagnostics, command palette baseline)


