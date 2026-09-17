# Platform Capability Matrix — R6

> **Auto-generated** by `tools/generate_platform_capability_matrix.py`
> **Legend:** ✅ Primitive-mapped · 🟦 Custom-painted (functional) · 🔶 Limited · ⬜ Placeholder · ➖ N/A
> **C**: ✅ when a typed `rw_create_*` function exists for the kind, ⬜ when the only route is the generic `rw_create_widget_of_kind(name)`. Derived from `src/bindings/binding_impl.rs`, never hand-maintained.
> A few ✅ cells are compile-verified only; see [✅ cells not yet verified on a real device](#-cells-not-yet-verified-on-a-real-device未经真机验证的--单元格) under Degradation notes.

## Symbol semantics（符号语义）

| Symbol | Meaning（符号语义） |
| --- | --- |
| ✅ | Primitive-mapped implementation — the backend creates a real platform primitive for this widget (e.g. Win32 `Button`, GTK `SpinButton`, Android `SeekBar`). 映射到平台原语：后端创建真实平台原语。 |
| 🟦 | Custom-painted implementation (**fully functional**) — the platform ships no primitive for this widget, so this library's custom backend draws it. The widget behaves normally. 自绘型实现（功能完整）：平台无此原语，由本库自绘后端绘制，行为正常。 |
| 🔶 | Limited — the primitive path degrades to a *different* primitive, so the widget loses its identity (e.g. `create_chart` returns a panel). 受限：原语路径降级为其它原语，丢失自身身份。 |
| ⬜ | Placeholder — declared but not implemented yet. 已声明但尚未实现。 |
| ➖ | Not applicable on this platform. 该平台不适用。 |

> **How to read this（如何阅读）**
>
> **✅ 与 🟦 的区分是 `src/platform/` 的内部实现策略。调用方从不区分二者**：
> 它调用同一个 API，由平台层选择机制。这两个符号用于审计后端覆盖度，
> 不应出现在面向使用者的代码分支中。
>
> - ✅ means a real platform primitive exists for the widget on that platform.
> - 🟦 means the widget is **implemented and usable**, just custom-painted. It is
>   *not* a defect. Every 🟦 widget has a dedicated `create_*` implementation in
>   `src/control_backend/custom/`, not a delegation.
> - 🔶 is reserved for genuine downgrades where the *primitive* path silently
>   substitutes a different primitive. See "Degradation notes" for the exact map.
>
> 注意：🟦 表示控件**已实现且可用**，只是由本库自绘，并非缺陷；每个 🟦 控件在
> `src/control_backend/custom/` 都有专用 `create_*` 实现（非委托）。🔶 仅用于
> *原语路径*静默替换为其它原语的真实降级情形。

## Matrix

| Widget | Windows | Linux/X11 | macOS | Wayland | Mobile | Harmony | Embedded/Stub | C |
| --- |--- |--- |--- |--- |--- |--- |--- |--- |
| **Action** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ActivityIndicator** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **AdaptiveScaffold** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **AnimatedImage** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **AppBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Arc** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **AudioVisualizer** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **AutoCompleteEdit** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Avatar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Badge** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Banner** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **BarChart** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **BarcodeScanner** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **BezierCurveEditor** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **BottomNavigationBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **BottomSheet** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Button** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **Calendar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **CameraPreview** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Canvas** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Carousel** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Cascader** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Chart** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **CheckBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **CheckListBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Chip** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **CollapsiblePane** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ColorDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **ColorHistory** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ColorPicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ColorWell** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ColumnView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ComboBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **CommandLink** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ContextMenu** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **CupertinoAlertDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **CupertinoDatePicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **CupertinoNavigationBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **CupertinoSegmentedControl** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **CupertinoSlider** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **CupertinoSwitch** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **DataView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **DatePicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **DateRangePicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **DateTimePicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Dial** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Dialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **DirectoryDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Divider** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **DockPanel** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **DockWidget** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **DoubleSpinBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Dropdown** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **DropdownMenu** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **EditableComboBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **EmojiPicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **EmptyState** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **FAB** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **FileDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **FindReplaceDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **FloatingLabel** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **FontComboBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **FontDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **FontPreview** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Frame** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **FreeformShape** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Grid** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **GridTable** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **GroupBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **HeroAnimation** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Icon** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ImageGallery** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ImageView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ImePreedit** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **InplaceEditor** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **InputDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **KanbanBoard** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Keyboard** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **LCDNumber** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Label** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **Line** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **LineChart** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **LineEdit** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **ListBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **ListView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **LottieWidget** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **MaskedEdit** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **MasonryLayout** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **MaterialNavigationRail** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **MaterialSnackbar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **MdiArea** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Mention** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Menu** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **MenuBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **MenuButton** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **MenuItem** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **MessageBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **Meter** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **MiniCanvas** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **MiniChart** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **MobileDatePicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ModalBottomSheet** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **MultiSelectComboBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **NavigationDrawer** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **NavigationStack** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **NumberPicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **OtpInput** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Pagination** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Panel** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **PieChart** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **PieMenu** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Popover** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **PopupWindow** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ProgressBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **ProgressCircle** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ProgressDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **PropertiesPanel** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **PropertyGrid** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **QRCode** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **QueryBuilder** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **RadarChart** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **RadioButton** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **RangeSlider** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Rating** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **RefreshControl** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **RibbonBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **RichEdit** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **RiveWidget** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Roller** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **SafeArea** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ScrollArea** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **ScrollBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **SearchBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **SearchBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **SegmentedButton** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ShortcutEditor** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **SkeletonLoader** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Slider** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **Sparkline** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **SpinBox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **Spinner** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **SplashScreen** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Splitter** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **StackedWidget** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **StatusBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **Stepper** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **SwipeToDismiss** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Switch** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **TabBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **TabView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **TabWidget** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Table** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **TagInput** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **TextArea** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **TextEdit** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **TimePicker** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Toast** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ToggleButton** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **ToolBar** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **ToolButton** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Toolbox** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Tooltip** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **TreeView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **UndoView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **VideoPlayer** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **WebEngineContextMenuRequest** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **WebEngineCookieStore** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **WebEngineDownloadItem** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **WebEngineFindTextResult** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **WebEngineNotification** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **WebEnginePage** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **WebEngineScriptDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **WebEngineSettings** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **WebEngineView** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **WebEngineWebChannel** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **Window** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ✅ |
| **Wizard** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |
| **WizardDialog** | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | 🟦 | ⬜ |

---

Total widgets: 178 (matches 178 WidgetKind variants)

C-ABI typed constructors: 22 of 178 kinds. The remainder are reachable through `rw_create_widget_of_kind`, which takes a factory name at run time.

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
| `create_label` | LCDNumber |
| `create_line_edit` | RichEdit, TextEdit |
| `create_list_box` | CheckListBox, TreeView |
| `create_list_view` | ColumnView, UndoView |
| `create_panel` | Canvas, Cascader, Chart, CollapsiblePane, DataView, DockPanel, DockWidget, EmojiPicker, Grid, KanbanBoard, MdiArea, Mention, PropertyGrid, QueryBuilder, RadarChart, StackedWidget, Table, Toolbox, Wizard |
| `create_progress_bar` | ActivityIndicator |
| `create_slider` | Dial |

Additional facts to keep the matrix consistent with `src/widget/kind.rs`:
- `ToolBox` is not a `WidgetKind` variant (only `Toolbox` is); the duplicate row was removed.
- `WebView` is not a `WidgetKind` variant either — the `WebView`/`WebViewEnhanced`
  aliases live at the handle/render layer and map onto `WidgetKind::WebEngineView`.
- **The `WebEngine*` rows after `WebEngineView` are not `WidgetKind` variants.**
  `WebEnginePage`, `WebEngineSettings`, `WebEngineDownloadItem`,
  `WebEngineCookieStore`, `WebEngineWebChannel`, `WebEngineFindTextResult`,
  `WebEngineNotification`, `WebEngineScriptDialog` and `WebEngineContextMenuRequest`
  are Rust **wrapper types** over the one registered view: each forwards
  `Widget::base()` to what it wraps, so `kind()` answers `WebEngineView` for all of
  them. They were previously `WidgetKind` variants marked `kind-role: base`, which
  made them orphans (rule #22) — nothing could produce them, and because
  `factory_name_for_kind` resolves through `capability_by_kind`,
  `create_web_engine_page(..)` silently produced id `0`. They remain listed here
  because they are real render-pipeline symbols worth tracking, but they are
  **categories of `WebEngineView`**, not kinds of their own.
  `tests/blue9_r6_platform_capability_test.rs` therefore compares matrix rows to
  `WidgetKind` variants modulo exactly this documented set.
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
  `route_preference_for_widget_kind` promotes these three kinds to
  `ControlRoutePreference::NativePreferred` **only under `cfg(target_os = "windows")`**;
  every other OS keeps them on the custom backend, because no other platform provides
  these primitives. Before that routing change the Win32 implementations existed but
  were unreachable — the global `CustomRequired` arm always won — so the ✅ cells
  overstated the reachable behaviour. The Win32 objects are reached through
  `NativeControlBackend::create_spin_box`/`create_list_view`/`create_scroll_area`,
  each of which forwards to its same-named `Platform` method (no aliasing to a Panel).
  `windows_native_controls_route_natively` and
  `non_windows_native_controls_use_custom_backend` pin both sides of that split.

### ✅ cells not yet verified on a real device（未经真机验证的 ✅ 单元格）

A ✅ cell means a real platform primitive is created and reached. The following
cells are **compile-verified only** — they build cleanly for their target and are
covered by clippy, but no one has observed the widget on the running OS. Treat
them as "implemented and wired, pending hardware confirmation", not as confirmed
runtime behaviour:

| Widget | Platform | Status |
| --- | --- | --- |
| `SpinBox` | Windows | compile-verified (`x86_64-pc-windows-msvc`/`gnullvm`), not yet run on Windows |
| `ListView` | Windows | compile-verified, not yet run on Windows |
| `ScrollArea` | Windows | compile-verified, not yet run on Windows |

Everything else marked ✅ has been exercised on a real device or the platform's
native runtime (Windows/Linux/macOS desktop backends are the default build path;
the Android JNI cells were verified on an emulator and a physical arm64 device).
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
