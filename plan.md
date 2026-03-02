---
# rust-widgets Development Plan

## Rules

1. This file uses hierarchical TODO management, listing each top-level ITEM one by one.
2. Unfinished items are left blank; completed items are marked "Completed" with a timestamp (e.g., Completed 2026-03-02).
3. Deleted or canceled items are marked "Canceled" or "Deleted" with a timestamp.

## ITEM List

- [x] Signal/Slot System (library core) — Completed 2026-03-02
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

- [ ] Basic Geometry & Style Types
	- Point (x, y)
	- Size (w, h)
	- Rect (x, y, w, h)
	- Color (rgba)
	- Font (name, size, weight)
	- Margin/Padding (per side)
	- Alignment (left/center/right, top/center/bottom)

- [ ] Widget Base Class
	- Position/size/rect methods
	- Show/hide, enable/disable
	- Min/max size constraints
	- Background/foreground/border/font
	- Mouse/keyboard/focus signals (hover, mouse_down/up, key_down/up, focus_gained/lost)
	- Redraw/layout signals
	- All input as signals, no virtual functions or inheritance magic

- [ ] Basic Widgets
	- Button: text/icon, clicked signal, press/release/disable states
	- Label: text/image, word wrap, alignment
	- LineEdit: text_changed, return_pressed, selection/copy/cut/paste, password mode
	- CheckBox: toggled signal, tri-state
	- RadioButton: group logic, selected signal
	- ComboBox: index_changed, dropdown
	- SpinBox/Slider: value_changed
	- ProgressBar: set/get value

- [ ] Layout System
	- HBox (horizontal)
	- VBox (vertical)
	- Grid (grid)
	- Stack (stacked)
	- Auto position/size calculation
	- Stretch factors, spacing, margins

- [ ] Action System
	- Shared logic for menu/button/shortcut
	- triggered signal
	- Checkable, enable/disable

- [ ] Intermediate Widgets
	- ScrollArea/ScrollBar
	- GroupBox
	- TabWidget
	- Splitter
	- MenuBar/Menu/ToolBar
	- StatusBar
	- Dialog/MessageBox/FileDialog/ColorDialog/FontDialog

- [ ] Model/View Architecture
	- ListModel/TableModel/TreeModel
	- Data change signals
	- Auto view refresh

- [ ] Advanced Widgets
	- TreeView/TableView/ListView
	- RichEdit (code editor)
	- DockPanel
	- MdiArea (multi-document)
	- Chart

- [ ] Style & Theme System
	- Dark/light mode
	- QSS-like stylesheet
	- Widget skinning

- [ ] Animation System
	- Property animation
	- Easing functions
	- Animation composition

- [ ] Window & Main Framework
	- Window/MainWindow
	- Modal window
	- Frameless window

- [ ] IDE Capabilities (final goal)
	- Multi-window
	- Dock panels
	- Code editor
	- Menu/toolbar/statusbar
	- Plugin system
	- AI Assist integration

