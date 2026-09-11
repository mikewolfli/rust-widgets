# Platform Capability Matrix — R6

> **Auto-generated** by `tools/generate_platform_capability_matrix.py`
> **Legend:** ✅ Native · 🟦 Self-drawn (functional) · 🔶 Limited · ⬜ Placeholder · ➖ N/A

## Symbol semantics（符号语义）

| Symbol | Meaning（符号语义） |
| --- | --- |
| ✅ | Native implementation — the backend creates a real platform primitive for this widget (e.g. Win32 `Button`, GTK `SpinButton`, Android `SeekBar`). 原生实现：后端创建真实平台原语。 |
| 🟦 | Self-drawn implementation (**fully functional**) — the platform ships no primitive for this widget, so this library's custom backend draws it. The widget behaves normally; it is simply not hosted by the OS. 自绘实现（功能完整）：平台无此原语，由本库自绘后端实现，行为正常，只是不由操作系统承载。 |
| 🔶 | Limited — the native path degrades to a *different* primitive, so the widget loses its identity (e.g. `create_chart` returns a panel). 受限：原生路径降级为其它原语，丢失自身身份。 |
| ⬜ | Placeholder — declared but not implemented yet. 已声明但尚未实现。 |
| ➖ | Not applicable on this platform. 该平台不适用。 |

> **How to read this（如何阅读）**
>
> - ✅ means a real platform primitive exists for the widget on that platform.
> - 🟦 means the widget is **implemented and usable**, just self-drawn. It is *not*
>   a defect: the project policy is native-first with a self-drawn fallback
>   （原生优先，自绘兜底）. Every 🟦 widget has a dedicated `create_*`
>   implementation in `src/control_backend/custom/`, not a delegation.
> - 🔶 is reserved for genuine downgrades where the *native* path silently
>   substitutes a different primitive. See "Degradation notes" for the exact map.
>
> 注意：🟦 表示控件**已实现且可用**，只是由本库自绘，并非缺陷；每个 🟦 控件在
> `src/control_backend/custom/` 都有专用 `create_*` 实现（非委托）。🔶 仅用于
> *原生路径*静默替换为其它原语的真实降级情形。

## Matrix

| Widget | Windows | Linux/X11 | macOS | Wayland | Mobile | Harmony | Embedded/Stub |
| --- |--- |--- |--- |--- |--- |--- |--- |
| **Action** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **ActivityIndicator** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **AdaptiveScaffold** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **AnimatedImage** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **AppBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Arc** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **AudioVisualizer** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **AutoCompleteEdit** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Avatar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Badge** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **BarChart** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **BarcodeScanner** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **BezierCurveEditor** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **BottomNavigationBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **BottomSheet** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Button** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **Calendar** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **CameraPreview** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Canvas** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Carousel** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Chart** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **CheckBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **CheckListBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Chip** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **CollapsiblePane** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ColorDialog** | 🟦 | 🟦 | ✅ | 🟦 | 🟦 | 🟦 | 🟦 |
| **ColorHistory** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ColorWell** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ColumnView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ComboBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **CommandLink** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **ContextMenu** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **CupertinoAlertDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **CupertinoDatePicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **CupertinoNavigationBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **CupertinoSegmentedControl** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **CupertinoSlider** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **CupertinoSwitch** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **DataView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **DatePicker** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **DateRangePicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **DateTimePicker** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Dial** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **Dialog** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **DirectoryDialog** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Divider** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **DockPanel** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **DockWidget** | ✅ | ✅ | ✅ | ✅ | 🟦 | 🟦 | 🟦 |
| **DoubleSpinBox** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Dropdown** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **DropdownMenu** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **EditableComboBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **EmptyState** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **FAB** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **FileDialog** | 🟦 | 🟦 | ✅ | 🟦 | 🟦 | 🟦 | 🟦 |
| **FindReplaceDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **FloatingLabel** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **FontComboBox** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **FontDialog** | 🟦 | 🟦 | ✅ | 🟦 | 🟦 | 🟦 | 🟦 |
| **FontPreview** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Frame** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **FreeformShape** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Grid** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **GridTable** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **GroupBox** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **HeroAnimation** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Icon** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ImageGallery** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ImageView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ImePreedit** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **InplaceEditor** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **InputDialog** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Keyboard** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **LCDNumber** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **Label** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **Line** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **LineChart** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **LineEdit** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **ListBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **ListView** | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **LottieWidget** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **MaskedEdit** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **MasonryLayout** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **MaterialNavigationRail** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **MaterialSnackbar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **MdiArea** | ✅ | ✅ | ✅ | ✅ | 🟦 | 🟦 | 🟦 |
| **Menu** | ✅ | ✅ | ✅ | ✅ | 🟦 | 🟦 | 🟦 |
| **MenuBar** | ✅ | ✅ | ✅ | ✅ | 🟦 | 🟦 | 🟦 |
| **MenuButton** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **MenuItem** | ✅ | ✅ | ✅ | ✅ | 🟦 | 🟦 | 🟦 |
| **MessageBox** | 🟦 | 🟦 | ✅ | 🟦 | 🟦 | 🟦 | 🟦 |
| **Meter** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **MiniCanvas** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **MiniChart** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **MobileDatePicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ModalBottomSheet** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **MultiSelectComboBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **NavigationDrawer** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **NavigationStack** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **PagerPageView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Panel** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **PieChart** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **PieMenu** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Popover** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **PopupWindow** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ProgressBar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **ProgressCircle** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ProgressDialog** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **PropertiesPanel** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **PropertyGrid** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **QRCode** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **RadioButton** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **RangeSlider** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Rating** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **RefreshControl** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **RibbonBar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **RichEdit** | ✅ | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 |
| **RiveWidget** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Roller** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **SafeArea** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ScrollArea** | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ScrollBar** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **SearchBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **SearchBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **SegmentedButton** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ShortcutEditor** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **SkeletonLoader** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Slider** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **Sparkline** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **SpinBox** | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Spinner** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Splitter** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **StackedWidget** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **StatusBar** | ✅ | ✅ | ✅ | ✅ | 🟦 | 🟦 | 🟦 |
| **Stepper** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **SwipeToDismiss** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Switch** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **TabBar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **TabView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **TabWidget** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Table** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **TagInput** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **TextArea** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **TextEdit** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **TileView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **TimePicker** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ToggleButton** | ✅ | ✅ | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **ToolBar** | ✅ | ✅ | ✅ | ✅ | 🟦 | 🟦 | 🟦 |
| **ToolButton** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **Toolbox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🟦 |
| **Tooltip** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **TreeView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **UndoView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **VideoPlayer** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **WebEngineContextMenuRequest** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **WebEngineCookieStore** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **WebEngineDownloadItem** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **WebEngineFindTextResult** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **WebEngineNotification** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **WebEnginePage** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **WebEngineScriptDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **WebEngineSettings** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **WebEngineView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **WebEngineWebChannel** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **Window** | ✅ | ✅ | ✅ | ✅ | 🟦 | 🟦 | 🟦 |
| **Wizard** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |
| **WizardDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 |

---

Total widgets: 167 (matches 167 WidgetKind variants)

---

## Degradation notes（降级说明）

On the **native/FFI path** (`src/control_backend/native.rs`), the following widget
families are not created as dedicated native controls. Each `create_*` listed
below delegates to a fallback primitive, silently in most cases (`log::warn!` is
emitted only for `data_view`, `property_grid`, `collapsible_pane`, `column_view`,
`undo_view`):

| Fallback created | Widgets (WidgetKind / matrix row names) |
| --- | --- |
| `create_button` | Action, CommandLink, ToolButton |
| `create_checkbox` | ToggleButton |
| `create_double_spin_box` | SpinBox |
| `create_label` | LCDNumber |
| `create_line_edit` | RichEdit, TextEdit |
| `create_list_box` | CheckListBox, TreeView |
| `create_list_view` | ColumnView, UndoView |
| `create_panel` | Canvas, Chart, CollapsiblePane, DataView, DockPanel, DockWidget, Grid, MdiArea, PropertyGrid, ScrollArea, StackedWidget, Table, Toolbox, WebEngineContextMenuRequest, WebEngineCookieStore, WebEngineDownloadItem, WebEngineFindTextResult, WebEngineNotification, WebEnginePage, WebEngineScriptDialog, WebEngineSettings, WebEngineView, WebEngineWebChannel, Wizard |
| `create_progress_bar` | ActivityIndicator |
| `create_slider` | Dial |
| `create_spin_box` | DoubleSpinBox |
| `create_toggle_button` | CheckBox |

Additional facts to keep the matrix consistent with `src/widget/kind.rs`:
- `ToolBox` is not a `WidgetKind` variant (only `Toolbox` is); the duplicate row was removed.
- `WebView` is not a `WidgetKind` variant either — the `WebView`/`WebViewEnhanced`
  aliases live at the handle/render layer and map onto `WidgetKind::WebEngineView`.
  The matrix therefore lists only the WebEngine rows.
- `MessageBox`/`FileDialog`/`ColorDialog`/`FontDialog` are 🟦 (self-drawn) on
  Windows/Linux/Wayland because the **default** runtime of those platform impls
  creates a state/surrogate handle (Windows: `Panel` surrogate; Linux/Wayland:
  state-only) rather than a dedicated native dialog; only macOS (objc2 +
  cocoa-legacy) creates real `NSAlert`/`NSOpenPanel`/`NSColorPanel`/`NSFontPanel` (✅).
  The widgets themselves are fully usable — the custom backend provides a dedicated
  dialog implementation — they are simply not OS-hosted. Under the `gtk-native`
  feature the Linux backend additionally constructs real
  `gtk::MessageDialog`/`FileChooserDialog`/`ColorChooserDialog`/`FontChooserDialog`
  objects, so a `gtk-native` build upgrades those cells to ✅ in practice.
- `SpinBox`/`ListView`/`ScrollArea` are ✅ on Windows as of 2026-09-11: the backend now
  creates real Win32 objects (`msctls_updown32` with `UDS_SETBUDDYINT`;`SysListView32`
  in report view with an inserted column and `LVS_EX_FULLROWSELECT`; a
  `WS_HSCROLL | WS_VSCROLL` child window with an initial scroll range). These are
  **compile-verified** for `x86_64-pc-windows-msvc`/`gnullvm` and clippy-clean via
  the `windows-cross-check` CI job, but have not yet been observed on a running
  Windows machine.
- `SpinBox` is 🟦 (self-drawn) on Linux/X11, Wayland, Mobile and Harmony: the macOS
  objc2 backend creates a native `NSStepper` under the `macos` feature, but the
  default cocoa-legacy path is not native. Under `gtk-native` the Linux backend
  creates a real `gtk::SpinButton`.
- `ListView`/`ScrollArea` are 🟦 (self-drawn) on Linux/X11, macOS, Wayland, Mobile
  and Harmony in the default build.
- `DatePicker`/`TimePicker`/`DateTimePicker` gained dedicated `create_*` native
  implementations on 2026-09-11 (GTK composites over `gtk::Calendar`/`SpinButton`;
  Win32 `SysDateTimePick32`), so they no longer appear in the fallback table above.
- Mobile (Android) creation is state-backed by default; when `android-jni` is
  enabled **and** an Activity `Context` is stored, `platform_impl` constructs real
  Android `View` objects for Button/TextView/EditText/CheckBox/RadioButton/
  SeekBar/ProgressBar/Spinner/ListView/ScrollView/NumberPicker/FrameLayout.
  `MessageBox` also becomes a real `android.app.AlertDialog` (create / show /
  dismiss / `setMessage`) on that path — verified on an Android emulator, and on a
  physical arm64 device (Xiaomi M2102J2SC, Android 13).
  `FileDialog` becomes a real system `ACTION_OPEN_DOCUMENT` picker on that path
  (also verified on the physical device). `ColorDialog`/`FontDialog` (no platform
  picker on Android) and menus/toolbars remain logical-only because they need an
  Activity callback rather than a standalone View; the backend logs an explicit
  diagnostic on that path instead of silently degrading.
