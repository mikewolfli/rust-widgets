# Platform Capability Matrix — R6

> **Document ID:** R6-PCM-001
> **Last updated:** 2026-05-29
> **Scope:** Every widget in [`WidgetKind`](../../src/widget/kind.rs) × all target platforms.
> **Legend:** ✅ Native · 🔶 StateBacked · ⬜ Placeholder · ➖ NotApplicable

---

## Matrix

| Widget | Windows | Linux/X11 | macOS | Wayland | Mobile | Harmony | Embedded/Stub |
|---|---|---|---|---|---|---|---|
| **Window** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **Dialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **MessageBox** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **FileDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **ColorDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **FontDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **InputDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **ProgressDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **PopupWindow** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **Button** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **CheckBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **RadioButton** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Label** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **LineEdit** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **TextEdit** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **RichEdit** | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 | 🔶 |
| **ComboBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **SpinBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ListBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ListView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **TreeView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ProgressBar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Slider** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ScrollBar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ScrollArea** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Panel** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **DockPanel** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **GroupBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **TabWidget** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Splitter** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **MdiArea** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **MenuBar** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **Menu** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **MenuItem** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **ContextMenu** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **ToolBar** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **StatusBar** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **Canvas** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Table** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Grid** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Chart** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ToggleButton** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **CheckListBox** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DoubleSpinBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Dial** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Wizard** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DatePicker** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **TimePicker** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DateTimePicker** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DirectoryDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **DataView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **PropertyGrid** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Toolbox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **StackedWidget** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **CollapsiblePane** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DockWidget** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **WebView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ActivityIndicator** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Calendar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ColumnView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **UndoView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **CommandLink** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **LCDNumber** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **FontComboBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **WebEngineView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEnginePage** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineSettings** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineDownloadItem** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineCookieStore** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineWebChannel** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineFindTextResult** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineNotification** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineScriptDialog** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineContextMenuRequest** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Action** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ToolButton** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ToolBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **FreeformShape** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **TabBar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **PieMenu** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **RibbonBar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |

---

## Platform Descriptions

| Platform | Family | Backend | Notes |
|---|---|---|---|
| **Windows** | Desktop | `WindowsPlatform` (Win32) | Full native window/control creation via platform trait. |
| **Linux/X11** | Desktop | `LinuxPlatform` (GTK) | GTK-based backend on X11 sessions. |
| **macOS** | Desktop | `MacOSPlatform` (Cocoa) or `MacOSObjc2Platform` (preview) | Cocoa/AppKit native widgets. |
| **Wayland** | Desktop | `WaylandPlatform` | Native Wayland session, gated by `wayland-native` feature. |
| **Mobile** | Mobile | Android / iOS / HarmonyMobile | Gated by `mobile-api` feature; limited native create paths. |
| **Harmony** | Desktop / Mobile | HarmonyDesktop / HarmonyMobile | Cross-platform Harmony OS backends; preview quality. |
| **Embedded/Stub** | Embedded | `StubPlatform` | Constrained runtime; most controls are state-backed simulation. |

---

## Classification Rules

1. **✅ Native** — Platform provides a `create_*` method on the `Platform` trait with a direct native widget implementation (e.g. `create_button` → Win32 `CreateWindow`, GTK `gtk_button_new`). Required for core desktop controls on Windows, Linux/X11, macOS, and Wayland.

2. **🔶 StateBacked** — Widget implements `impl Widget for` via the state-backed widget model (`BaseWidget` + property system). All widgets that implement the `Widget` trait with a `base()` delegating to `BaseWidget` are at minimum StateBacked on all platforms.

3. **⬜ Placeholder** — Widget exists in `WidgetKind` but has no `impl Widget for` in the codebase. These are future stubs (not applicable to this project).

4. **➖ NotApplicable** — Widget is semantically unsupported on the target (e.g., native menu on mobile). Currently no cases in the matrix.

### Desktop native controls (✅ Native)
Window, Dialog*, MessageBox, FileDialog, ColorDialog, FontDialog, InputDialog, ProgressDialog, PopupWindow,
Button, CheckBox, RadioButton, Label, LineEdit, ComboBox, SpinBox, ListBox, ProgressBar, Slider, ScrollBar,
Panel, GroupBox, TabWidget, Splitter, MdiArea,
MenuBar, Menu, MenuItem, ContextMenu, ToolBar, StatusBar,
RichEdit (desktop only),
ToggleButton, DoubleSpinBox, Dial,
DirectoryDialog, Toolbox, DockWidget,
CommandLink, LCDNumber, FontComboBox,
Action, ToolButton, ToolBox, TabBar, Calendar, RibbonBar

### Desktop dialogs (✅ Native on desktop, 🔶 on mobile/Harmony)
All dialog types (Dialog, MessageBox, FileDialog, ColorDialog, FontDialog, InputDialog, ProgressDialog, PopupWindow, DirectoryDialog)

### State-backed controls (🔶 on all platforms)
TextEdit, ListView, TreeView, ScrollArea, DockPanel,
Canvas, Table, Grid, Chart, CheckListBox, Wizard,
DatePicker, TimePicker, DateTimePicker,
DataView, PropertyGrid, StackedWidget, CollapsiblePane,
WebView, ActivityIndicator, ColumnView, UndoView,
WebEngineView, WebEnginePage, WebEngineSettings,
WebEngineDownloadItem, WebEngineCookieStore,
WebEngineWebChannel, WebEngineFindTextResult,
WebEngineNotification, WebEngineScriptDialog,
WebEngineContextMenuRequest,
FreeformShape, PieMenu

---

## Version History

| Date | Author | Changes |
|---|---|---|
| 2026-05-29 | AI Assistant | Initial R6 capability matrix; 81 WidgetKind variants × 7 platforms.
