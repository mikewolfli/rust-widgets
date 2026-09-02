# Platform Capability Matrix — R6

> **Auto-generated** by `tools/generate_platform_capability_matrix.py`
> **Legend:** ✅ Usable · 🔶 Backend-limited · ⬜ Placeholder · ➖ NotApplicable

## Symbol semantics（符号语义）

| Symbol | Meaning（符号语义） |
| --- | --- |
| ✅ | Usable control path on this platform — either a real native primitive or a state/self-drawn backend implementation that behaves normally. 该平台提供可用的控件路径（原生原语或 state/自绘后端均可正常工作）。 |
| 🔶 | Limited by backend capability — mapped/degraded/partial implementation. 受限于后端能力（映射/降级/部分实现）。 |
| ⬜ | Placeholder — declared but not implemented yet. |
| ➖ | Not applicable on this platform. |

> Note: ✅ only means *a working creation path exists*. For the rows listed under
> "Degradation notes（降级说明）" below, the native/FFI path returns a fallback
> primitive (Panel/Slider/Label/…), so their ✅ cells do **not** imply a dedicated
> native control implementation. 注：✅ 仅表示存在可用创建路径；文末“降级说明”
> 所列控件在 native/FFI 路径上实际创建为回退原语。

## Matrix

| Widget | Windows | Linux/X11 | macOS | Wayland | Mobile | Harmony | Embedded/Stub |
| --- |--- |--- |--- |--- |--- |--- |--- |
| **Action** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ActivityIndicator** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **AdaptiveScaffold** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **AnimatedImage** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **AppBar** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Arc** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **AudioVisualizer** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **AutoCompleteEdit** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Avatar** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Badge** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **BarChart** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **BarcodeScanner** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **BezierCurveEditor** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **BottomNavigationBar** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **BottomSheet** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Button** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Calendar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **CameraPreview** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Canvas** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Carousel** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Chart** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **CheckBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **CheckListBox** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Chip** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **CollapsiblePane** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ColorDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **ColorHistory** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ColorWell** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ColumnView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ComboBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **CommandLink** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ContextMenu** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **CupertinoAlertDialog** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **CupertinoDatePicker** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **CupertinoNavigationBar** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **CupertinoSegmentedControl** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **CupertinoSlider** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **CupertinoSwitch** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DataView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DatePicker** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DateRangePicker** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DateTimePicker** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Dial** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Dialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **DirectoryDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **Divider** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DockPanel** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DockWidget** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **DoubleSpinBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Dropdown** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **DropdownMenu** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **EditableComboBox** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **EmptyState** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **FAB** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **FileDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **FindReplaceDialog** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **FloatingLabel** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **FontComboBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **FontDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **FontPreview** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Frame** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **FreeformShape** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Grid** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **GridTable** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **GroupBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **HeroAnimation** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Icon** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ImageGallery** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ImageView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ImePreedit** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **InplaceEditor** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **InputDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **Keyboard** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **LCDNumber** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Label** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Line** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **LineChart** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **LineEdit** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ListBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ListView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **LottieWidget** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **MaskedEdit** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **MasonryLayout** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **MaterialNavigationRail** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **MaterialSnackbar** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **MdiArea** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **Menu** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **MenuBar** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **MenuButton** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **MenuItem** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **MessageBox** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **Meter** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **MiniCanvas** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **MiniChart** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **MobileDatePicker** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ModalBottomSheet** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **MultiSelectComboBox** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **NavigationDrawer** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **NavigationStack** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **PagerPageView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Panel** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **PieChart** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **PieMenu** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Popover** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **PopupWindow** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **ProgressBar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ProgressCircle** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ProgressDialog** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **PropertiesPanel** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **PropertyGrid** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **QRCode** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **RadioButton** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **RangeSlider** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Rating** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **RefreshControl** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **RibbonBar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **RichEdit** | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 | 🔶 |
| **RiveWidget** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Roller** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **SafeArea** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ScrollArea** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ScrollBar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **SearchBar** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **SearchBox** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **SegmentedButton** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ShortcutEditor** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **SkeletonLoader** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Slider** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Sparkline** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **SpinBox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Spinner** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Splitter** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **StackedWidget** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **StatusBar** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **Stepper** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **SwipeToDismiss** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Switch** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **TabBar** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **TabView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **TabWidget** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Table** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **TagInput** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **TextArea** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **TextEdit** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **TileView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **TimePicker** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **ToggleButton** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **ToolBar** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **ToolButton** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Toolbox** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 |
| **Tooltip** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **TreeView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **UndoView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **VideoPlayer** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineContextMenuRequest** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineCookieStore** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineDownloadItem** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineFindTextResult** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineNotification** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEnginePage** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineScriptDialog** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineSettings** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineView** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WebEngineWebChannel** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **Window** | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 | 🔶 |
| **Wizard** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |
| **WizardDialog** | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 | 🔶 |

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
| `create_panel` | ScrollArea, DockPanel, GroupBox, TabWidget, Splitter, StackedWidget, MdiArea, Canvas, Table, Grid, Chart, Wizard, DatePicker, TimePicker, DateTimePicker, DataView, PropertyGrid, Toolbox, CollapsiblePane, DockWidget, Calendar, WebView/WebEngine family (WebEngineView, WebEnginePage, WebEngineSettings, WebEngineDownloadItem, WebEngineCookieStore, WebEngineWebChannel, WebEngineFindTextResult, WebEngineNotification, WebEngineScriptDialog, WebEngineContextMenuRequest) |
| `create_slider` | ScrollBar, Dial |
| `create_label` | LCDNumber (rendered as a label showing `"0"`) |
| `create_line_edit` | TextEdit, RichEdit |
| `create_button` | CommandLink, Action, ToolButton |
| `create_combo_box` | FontComboBox |
| `create_list_box` | TreeView |
| `create_list_view` | ColumnView, UndoView |
| `create_checkbox` | ToggleButton |
| `create_spin_box` | DoubleSpinBox |
| `create_message_box` | Dialog |
| `create_file_dialog` | DirectoryDialog |
| `create_menu` | ContextMenu |
| `create_progress_bar` | ActivityIndicator |
| `create_window` | PopupWindow |

Additional facts to keep the matrix consistent with `src/widget/kind.rs`:
- `ToolBox` is not a `WidgetKind` variant (only `Toolbox` is); the duplicate row was removed.
- `WebView` is not a `WidgetKind` variant either — the `WebView`/`WebViewEnhanced`
  aliases live at the handle/render layer and map onto `WidgetKind::WebEngineView`.
  The matrix therefore lists only the WebEngine rows.
