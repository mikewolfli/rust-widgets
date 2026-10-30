// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! rust_widgets - cross-platform native GUI architecture in pure Rust.
//!
//! # Text coverage
//!
//! **The default build draws Latin/ASCII only.** This is a stated boundary, not a bug
//! report: the crate ships no font data, so glyphs come from a fixed 8x8 bitmap face whose
//! character set is `U+0000`–`U+007F`.
//!
//! ```text
//! src/render/text/glyph_source.rs   Font8x8Source  // U+0000-U+007F: the default's only face
//! src/render/text/glyph_source.rs   TOFU           // the fallback glyph for anything else
//! src/render/text/line.rs           estimate_cluster_advance // the one advance model
//! ```
//!
//! What that means in practice:
//!
//! * A character inside the supported range is drawn from the bitmap face.
//! * Any character outside it — CJK, Cyrillic, Arabic, emoji, and every other script —
//!   is drawn as the **fallback glyph** (a hollow box, "tofu"). The label still lays
//!   out and the control still renders; only the glyph is wrong.
//! * A line is ordered for painting by the Unicode bidirectional algorithm, so an Arabic
//!   or Hebrew run is drawn right to left. Ordering is not glyph *shape*: script-specific
//!   shaping (Arabic joining, Indic reordering) is still not applied.
//!
//! ## Coverage beyond Latin/ASCII is opt-in, and that is the whole contract
//!
//! What a wider script needs is font data, and the default build carries **none** — a
//! `mini` or `embedded` profile must not pay hundreds of kilobytes for glyphs it will
//! never draw. So coverage is a feature a caller asks for by name:
//!
//! ```text
//! cargo build --features "desktop,fonts-cjk-bitmap"
//! ```
//!
//! `fonts-cjk-bitmap` adds a generated 16x16 CJK face — Han, kana, CJK punctuation and
//! fullwidth forms, about 85 KB of read-only data read on demand and never resident. Latin
//! rendering stays byte-identical when it is enabled: a face is *appended* to the fallback
//! stack, so it can only answer for characters the base face has no glyph for.
//!
//! ## What is deliberately not claimed
//!
//! This crate does **not** implement Unicode text rendering, and does not call itself
//! multilingual. A host that passes non-Latin text with no font feature enabled will see
//! the fallback glyph, and that is the honest behaviour of the default configuration.
//! Shaping for complex scripts remains future work rather than something assumed.
//!
//! ## The guarantee this section carries
//!
//! The boundary above is asserted from **both** sides — see
//! `render::pipeline::pixel_ops::text_coverage_tests` and `render::text::glyph_source`'s
//! tests. A non-Latin character must resolve to the fallback glyph on a default build, and
//! enabling the CJK data must move that boundary by exactly the face it adds. A claim about
//! what is *not* supported decays silently in either direction, so both are pinned, and
//! `tools/check_text_coverage_claim_matches_features.sh` fails if this section and the
//! feature graph ever disagree.

// BLUE11 R9.6: Unsafe code audit — unsafe is required for platform FFI
// Note: Removed `#![allow(unsafe_code)]` — default is allow, no-op.
// `missing_docs` is denied rather than allowed: every public item in every profile
// is documented, and this lint is what keeps it that way. It is enforced on the
// `desktop`, `embedded`, `mini` and `--all-features` builds alike, and CI's clippy
// job runs with `-D warnings`, so a new undocumented public item fails the build.
#![deny(missing_docs)]
// The OpenHarmony cross target expands `thread_local!` through a std shim whose const path the
// lint cannot see, so every already-`const` initialiser there is reported as "can be made const".
// The lint is accurate on the host targets (where it stays enabled); scoping the allow to the
// OHOS environment keeps that coverage without silencing the check everywhere.
#![cfg_attr(target_env = "ohos", allow(clippy::missing_const_for_thread_local))]
// `mini` is the no-std-oriented profile: it compiles against `core` + `alloc` only.
// Every shared type is imported through `compat`, so this attribute is the switch
// that turns the abstraction into an enforced contract.
#![cfg_attr(feature = "mini", no_std)]
// BLUE11: Clippy lints enabled for quality enforcement.
// Individual allows are placed next to their specific violations.
#![cfg_attr(
    test,
    allow(
        clippy::needless_pass_by_value,
        clippy::unwrap_used,
        clippy::field_reassign_with_default
    )
)]
// Required unconditionally — `alloc` is available in both std (re-exported)
// and no_std contexts. `core` is always available via `extern crate std` under
// std, but we need direct `alloc::` paths in `compat` for no_std builds.
extern crate alloc;

// `#![no_std]` removes the *standard* prelude, and with it the `thread_local!`
// macro. `thread_local!` is not part of the `core` prelude: it is declared in
// `std` and reaches an ordinary crate only through the `#[prelude_import]`
// `std::prelude::rust_20xx` glob that the compiler injects when `std` is
// implicitly linked. Under `mini` the compiler stops injecting that glob (the
// crate is `no_std`), so every `thread_local!` call site in the crate fails to
// resolve — a *macro*-only regression, which is why it breaks modules such as
// `layout` that never touch `std` directly.
//
// The attribute below makes the compiler re-inject the std prelude even though
// `#![no_std]` is in effect, which is exactly what this crate wants: it is
// no-std-*oriented* and still links `std` on every target it builds for, and the
// glob is purely lexical (no `std` item is brought into scope — `Option`,
// `String`, shadows and the rest stay `core`/`compat`-sourced as before).
//
// Two alternatives were measured and rejected:
// * spelling each call site `std::thread_local!` — the macro is not reachable as
//   a path under `no_std` (verified: `cannot find `thread_local` in `std``), so
//   this does not compile at all;
// * `use std::thread_local;` — a macro-only import of a macro that is *not*
//   exported from the `std` crate root under `no_std`, same failure.
#[cfg(alloc_frugal)]
#[macro_use]
extern crate std;

// `#![no_std]` also removes the *alloc* prelude's trait imports, and unlike the
// macros above they cannot be brought back wholesale.
//
// `ToString` is not in the `alloc` prelude at all: it is re-exported by
// `std::prelude::v1` (`library/std/src/prelude/v1.rs`), which a crate receives
// only through the compiler-injected prelude glob that `#![no_std]` suppresses.
// The consequence is that every `s.to_string()` in the crate — on `&str`,
// `char`, `&&str`, `Shortcut`, `io::Error` — stops resolving under `mini`.
//
// This cannot be repaired from here either. Name resolution in Rust is *lexical*:
// a `use` is visible to items lexically nested in the block or module containing
// it, never to a module reached by path. A crate-root `use std::string::ToString;`
// would fix this file and nothing else, and the ninety-odd call sites live in
// child modules. Verified against a `#![no_std]` crate whose root imported the
// trait while a child module called `.to_string()`: the child still fails E0599.
//
// `#[macro_use]` is the mechanism that *does* cross module boundaries, but it
// only carries the macro namespace — and `ToOwned`, the sibling that supplies
// `.to_owned()`, is a trait, not a macro.
//
// So each module that needs these methods imports the trait itself, from
// `compat`, which re-exports it as [`compat::MiniToString`]. The alias keeps the
// import list readable next to `String` and `ToString` on desktop builds, where
// both names resolve.
//
// **BLUE13 Phase 3: Alloc bridge — unified imports for std and no_std ──**
// All crate files import from `compat` instead of directly from std.
pub mod compat;

/// Action/command system.
pub mod action;
/// Generic asset file watcher.
///
/// Gated on the capability it actually needs (`desktop-runtime` supplies
/// `notify` + `crossbeam-channel`), not on the `desktop` profile: `tablet` and
/// `mobile` enable the same capability, so gating on the profile removed a
/// documented public path from builds that can support it (principle #41).
#[cfg(all(feature = "desktop-runtime", not(alloc_frugal)))]
pub mod asset;
/// Audio module — format detection, decoding, encoding, sample processing, and normalization.
#[cfg(feature = "audio")]
pub mod audio;
/// C ABI and language bindings (C / Java-JNI / etc.).
///
/// Available on the `desktop` profile and on any profile that exposes an
/// FFI consumer (`jni` for Java, `mobile-api` for the mobile runtime). The
/// `mini` profile excludes it because it is `alloc`-free / no-std oriented.
#[cfg(all(any(feature = "desktop", feature = "jni", feature = "mobile-api"), not(alloc_frugal)))]
pub mod bindings;
/// Clipboard helpers.
pub mod clipboard;
/// Control backend abstraction for native/custom control implementations.
pub mod control_backend;
/// Core types and shared contracts.
pub mod core;
/// Reactive data binding system — Model → View automatic synchronization.
pub mod data_binding;
/// Designer support: mode 2, generating Rust source from a project (BLUE19 T-23, D7-b-3).
///
/// # Why this has its own gate rather than riding on `full_widgets`
///
/// `full_widgets` answers "does this build have the widget tree and the capability table?", which
/// every device application needs. This module answers a different question — "is this build **also**
/// a design tool?" — and a shipping application does not.
///
/// The generator and its artifact writer are development-time facilities. Leaving them on for every
/// profile would link a code generator plus `std::fs::write` into delivery artifacts, which is the
/// weight mode 2 exists to remove (BLUE19 §5.1.2).
///
/// `desktop` enables the `designer` feature by default — it is the profile a designer **host** runs
/// on. `tablet`/`mobile`/`mini`/`embedded` do not, because they are the **targets** of a generation,
/// not its host (BLUE19 §5.3.4); a caller who wants the generator there asks for it:
/// `--features tablet,designer`.
///
/// The condition is expressed as the `designer_tooling` alias from `build.rs` rather than a
/// hand-written conjunction, per rule #47.
#[cfg(designer_tooling)]
pub mod designer;
/// Unified error system (ErrorId, RwError, c_try!).
pub mod error;
/// Event types and dispatch helpers.
pub mod event;
/// Gesture recognizer system (gated behind `touch` feature).
#[cfg(feature = "touch")]
pub mod gesture;
/// Hardware-adaptive GPU management.
pub mod gpu;
/// Internationalization module.
#[cfg(feature = "i18n")]
pub mod i18n;
/// Image module — format detection, decoding, encoding, transform, and color conversion.
///
/// **Decoding** is real for: PNG (all bit depths, scanline filters, palette), JPEG,
/// BMP, QOI, Farbfeld, PNM (P5/P6). GIF/WebP/TIFF/AVIF/ICO/SVG decode returns an
/// explicit `Err` rather than fabricated pixels until a codec lands.
#[cfg(feature = "image")]
pub mod image;
/// Declarative JSON window engine (declarative-tree style).
///
/// Compiled for a full device build — `desktop`/`tablet`/`mobile` with an unstripped
/// widget set. Stated as the `full_widgets` alias rather than the conjunction, so it
/// cannot drift from `crate::app` and `crate::view`, which need exactly the same thing
/// (BLUE15 rule #57).
#[cfg(full_widgets)]
pub mod json;
/// Layout managers.
pub mod layout;
/// Memory management utilities.
pub mod memory;
/// Advanced widgets (gated behind `advanced-widgets` feature).
#[cfg(feature = "advanced-widgets")]
pub mod menu_config;
/// Object tree and object utilities.
pub mod object;
/// Performance monitoring and optimization.
pub mod performance;
/// Platform abstraction and backend adapters.
pub mod platform;
/// Quality management for adaptive rendering.
pub mod quality;
/// Rendering traits and primitives.
pub mod render;
/// Runtime render-engine abstraction.
pub mod render_engine;
/// Bilinear RGBA8 resampling shared by the image layer and the render layer.
///
/// Ungated because the paint layer needs it in profiles that compile no image decoder: a host tile
/// or captured frame is raw RGBA8, not a `crate::image` value.
pub mod resample;
/// Persistent application settings (`settings store`-equivalent key/value store).
///
/// Gated like the existing menu-config persistence precedent (`advanced-widgets`) plus
/// `serde_json`, and absent from `alloc_frugal` builds where `std::fs` is unavailable.
#[cfg(all(feature = "advanced-widgets", feature = "serde_json", not(alloc_frugal)))]
pub mod settings;
/// Global shortcut system for keyboard shortcuts.
pub mod shortcut;
/// Signal-slot utilities.
pub mod signal;
/// Style system primitives.
pub mod style;
/// Test infrastructure and utilities.
pub mod test;
/// Theme management — the palette, the appearance selector and the role table controls
/// resolve against, plus the process-wide manager that selects among them.
///
/// # Gating (BLUE20 layer 4, 2026-09-21)
///
/// Gated on `device_profile` (`desktop`/`tablet`/`mobile`), which is a *conjunction*: the
/// theme model needs a colour system (`not(alloc_frugal)`) **and** `serde`, and only the
/// three device profiles enable both. `mini` has no palette; the mixed
/// `windows + controls-custom` build has no `serde`.
///
/// # Why the widget layer no longer breaks when this is absent
///
/// ~120 control files read the theme while drawing, and they also compile in
/// `--no-default-features --features "windows desktop-runtime controls-custom"`
/// — a full widget set with no device profile. (`controls-native` is a documented
/// no-op alias and is deliberately omitted, so the example names only what it
/// actually selects.) Referencing
/// `crate::theme` from those files was 78 compile errors at HEAD. They now go through
/// `crate::style`, which exposes the same lookups (and the same type *shape* where the real
/// types are absent) and answers "no theme" there, so one path compiles in every profile
/// and the widgets fall back to their own literals where there is no palette to read
/// (principles #37/#47: a missing capability is reported, and the condition lives in one
/// place rather than at each call site).
#[cfg(device_profile)]
pub mod theme;
/// Undo/Redo framework for undoable commands and cross-widget undo/redo.
pub mod undo;
/// Video module — container format detection, frame extraction, metadata, and playback.
#[cfg(feature = "video")]
pub mod video;
/// Declarative-retained view layer: `state → Node → diff → Patch → retained tree`.
///
/// The declarative half of the hybrid architecture. Additive: it consumes the existing
/// widget factory and property contract rather than changing them.
///
/// Compiled for a device profile (`desktop`/`tablet`/`mobile`) unless the caller opts
/// out with the `no-declarative-view` feature; never compiled for `mini`/`embedded`.
/// The whole condition is the single alias `declarative_view`, so the opt-out and the
/// stripped-profile rule cannot drift apart (BLUE15 rule #57).
#[cfg(declarative_view)]
pub mod view;
/// Optional WGPU GPU acceleration backend (gated behind `gpu-wgpu` feature).
#[cfg(feature = "gpu-wgpu")]
pub mod wgpu_backend;
/// Widget definitions and widget helpers.
pub mod widget;

// Crate-root re-export groups and the crate-root procedural API live under `api/`.
// Every name they export keeps its historical `rust_widgets::<name>` path.
mod api;
// Root surface, stated explicitly: `tools/check_wildcard_exports.sh` forbids a
// wildcard re-export from the crate root, so each profile gate names its group.
pub use api::{
    base, base_property_get, base_property_set, base_widgets, capability, container_widgets,
    decorations, display_widgets, drain_triggers, draw, draw_bridge, draw_icon_at,
    draw_icon_centered, draw_icon_named, fit_image_into, icon_source, icon_token_is_drawable, init,
    input_widgets, kind, measure_icon, metrics, numeric, paint_icon, quit, registry, run,
    sample_data, svg, text_utils, validate_icon, widget_property_get, widget_property_names,
    widget_property_set, widget_trait, window, Arc, BaseWidget, Button, ButtonState,
    CapabilityAccessError, CapabilityContract, CapabilityValue, CheckBox, CheckState, ComboBox,
    Draw, DropEvent, Dropdown, EchoMode, EmbeddedCapabilityContract, Frame, GroupBox,
    HostedCapabilityContract, IconColorMode, IconData, IconItemError, IconMetrics, IconName,
    IconPaintError, IconPaintOutcome, IconRef, IconResourceResolver, IconSource,
    IconSourceMetadata, IconSpec, Keyboard, Label, Line, LineEdit, LineOrientation, ListBox, Meter,
    MiniCanvas, MiniChart, MobileBackend, OpenError, Panel, PlatformCapabilities, ProgressBar,
    PropertySchema, PropertyValueKind, RadioButton, Roller, RuntimeGuiMode, SaveState, SavedState,
    ScrollArea, ScrollBar, SelectionMode, SimpleRegistry, Slider, SpinBox, Spinner, StickyRegion,
    Switch, TextArea, Widget, WidgetCapability, WidgetKind, WidgetProperties, WidgetTriggerEvent,
    WidgetTriggerKind, Window, WindowStateFlag, BASE_PROPERTY_NAMES,
};

#[cfg(alloc_frugal)]
pub use api::{
    dispatch_event, dispatch_pointer_event, invalidate_surface, invalidate_surface_rect,
    mount_surface, queue_resize_trigger, resize_surface, supports_surfaces, take_pending_repaint,
    unmount_surface, window_client_size,
};

#[cfg(not(alloc_frugal))]
pub use api::{
    a11y_submit, animation_suspended, apply_environment_appearance, attach_menu_bar_to_window,
    backend_handle, backend_name, begin_drag, capabilities, choose_path, clipboard_text,
    close_widget, collect_save_state, combo_box_add_item, combo_box_clear_items,
    combo_box_current_index, combo_box_item_count, combo_box_item_text,
    combo_box_set_current_index, create_button, create_checkbox, create_color_dialog,
    create_combo_box, create_file_dialog, create_font_dialog, create_label, create_line_edit,
    create_list_box, create_list_view, create_menu, create_menu_bar, create_message_box,
    create_panel, create_progress_bar, create_radio_button, create_scroll_area, create_slider,
    create_spin_box, create_status_bar, create_tool_bar, create_widget_of_kind, create_window,
    dispatch_event, dispatch_pointer_event, dpi_scale_factor, drain_widget_triggers_for,
    drive_frame, format_shortcut, get_clipboard_text, get_platform, get_widget_accessibility_name,
    get_widget_text, hide_widget, inject_menu_trigger, inject_widget_trigger_event,
    invalidate_surface, invalidate_surface_rect, is_widget_checked, is_widget_enabled,
    is_widget_indeterminate, is_widget_read_only, is_widget_tristate, is_widget_visible,
    is_window_in_state, last_frame_stats, list_box_add_item, list_box_clear_items,
    list_box_current_index, list_box_item_count, list_box_item_text, list_box_remove_item,
    list_box_set_current_index, menu_add_item, menu_item_icons_supported, menu_item_shortcut,
    mount_surface, notify_native_redraw, open_path, platform_clipboard, platform_init,
    platform_quit, platform_run, poll_drop_event, poll_menu_triggered, poll_widget_trigger_event,
    poll_widget_triggered, queue_resize_trigger, reapply_active_theme, report_state,
    request_repaint_because, resize_surface, restore_save_state, runtime, runtime_gui_mode,
    runtime_gui_mode_for, save_path, schedule_on_main, select_directory, set_animation_suspended,
    set_clipboard_text, set_slider_orientation, set_widget_accessibility_name, set_widget_checked,
    set_widget_echo_mode, set_widget_enabled, set_widget_geometry, set_widget_group,
    set_widget_indeterminate, set_widget_max_length, set_widget_placeholder, set_widget_range,
    set_widget_read_only, set_widget_scroll_position, set_widget_selected_index,
    set_widget_selection, set_widget_step, set_widget_text, set_widget_tristate, set_widget_value,
    set_widget_visible, set_window_icon, set_window_min_size, set_window_state, show_widget,
    slider_orientation, supports_surfaces, take_pending_repaint, unmount_surface,
    wait_for_host_events, widget_echo_mode, widget_geometry, widget_group, widget_max_length,
    widget_placeholder, widget_range, widget_scroll_position, widget_selected_index,
    widget_selection, widget_step, widget_value, window_client_size, window_icon, window_min_size,
    FrameOutcome, FrameStats, RepaintReason, StateFact,
};

#[cfg(full_widgets)]
pub use api::{
    advanced_widgets, census, chart_widgets, composite, cupertino, dialog, media_widgets,
    menu_close, menu_open_at, menu_toolbar, misc_widgets, nav_widgets, overlay_widgets,
    sample_fill, special_widgets, view_widgets, Action, ActivityIndicator, AdaptiveScaffold,
    AnimatedFrame, AnimatedImage, AnimatedImageFormat, AppBar, AudioVisualizer, Avatar, Banner,
    BarChart, BarEntry, BarcodeFormat, BarcodeResult, BarcodeScanner, BezierCurveEditor,
    BottomNavigationBar, BottomSheet, Breadcrumb, BreadcrumbSegment, Calendar, CameraPreview,
    Canvas, Cascader, CascaderOption, Chart, ChartWidget, CheckListBox, Chip, ChipItem, CodeEditor,
    ColorDialog, ColorPicker, ColumnFilter, ColumnView, CommandEntry, CommandPalette, Compass,
    CompletedMention, ContextMenu, CupertinoAlertDialog, CupertinoDatePicker,
    CupertinoNavigationBar, CupertinoSegmentedControl, CupertinoSlider, CupertinoSwitch, DataGrid,
    DataView, DateEdit, DateFormat, DatePicker, DateRangePicker, DateTimeEdit, DateTimePicker,
    Dependency, DependencyEditor, DiagnosticMarker, Dial, Dialog, DiffFileList, DiffKind, DiffLine,
    DiffViewer, DirectoryDialog, DoubleSpinBox, DropZone, DropdownItem, DropdownMenu, EmojiGlyph,
    EmojiPicker, FileDialog, FilterCondition, FilterConjunction, FilterExpr, FilterField,
    FilterOperator, FindBar, FindReplaceDialog, FontDialog, FreeformShape, FreeformShapeWidget,
    GalleryImage, GanttTask, GanttWidget, Grid, GridTable, GridTableWidget, GridViewHeader,
    GridWidget, Heatmap, HeatmapCell, HeroAnimation, ImageGallery, InputDialog, KanbanBoard,
    KanbanCard, KanbanCardView, KanbanColumn, KanbanColumnHeader, KeySequenceEdit, LineChart,
    ListModel, ListView, LottieWidget, MapMarker, MapView, MarkdownEditor, MarkdownOutlinePanel,
    MarkerSeverity, MaterialNavigationRail, MaterialSnackbar, MediaPlayer, Mention,
    MentionCandidate, Menu, MenuBar, MenuButton, MenuItem, MessageBox, MobileDatePicker,
    ModalBottomSheet, NavItem, NavigationDrawer, NavigationEvent, NavigationStack,
    NotificationCenter, NotificationItem, NotificationLevel, Pagination, PieChart, PieMenu,
    PieMenuItem, PieSlice, Popover, PopupWindow, ProgressDialog, PropertiesPanel, PropertyEntry,
    PropertyGrid, PropertyItem, PropertyValue, PullToRefresh, QRCode, QueryBuilder,
    QueryBuilderRow, RadarChart, RailItem, RangePresetBar, RefreshControl, RibbonBar, RibbonGroup,
    RibbonItem, RiveInput, RiveInputValue, RiveWidget, ScaleBar, ScaleBarStyle, ScaleUnits,
    Segment, SegmentItem, SegmentedButton, SegmentedControl, ShapeCanvas, SignaturePad,
    SignatureStroke, Snackbar, SortSpec, Sparkline, SplashScreen, SplitAction, SplitButton,
    StatusBar, SwipeToDismiss, TabBar, TabBarTab, TabPage, TabView, Table, TableModel, TableWidget,
    TerminalView, TileKey, TileLoadError, TileLoader, TileRequest, TileViewport, TimeEdit,
    TimePicker, TimelineAxis, TimelineItem, TimelineWidget, Toast, ToastItem, ToastLevel,
    ToastStack, ToolBar, ToolButton, Tooltip, TreeModel, TreeTable, TreeTableModel, TreeView,
    UndoView, VecListModel, VideoPlayer, VirtualList, VirtualTable, Wizard, WizardDialog,
    WizardStep, ZoomButton, ZoomControl, FAB,
};

#[cfg(widgets_unstripped)]
pub use api::{
    clear_registered_icons, create_dialog, create_grid_widget, create_splitter, create_tab_widget,
    icon_registry, is_registered_icon, read_widget_property_by_id, register_icon,
    register_icon_on_grid, registered_icon, registered_icon_count, try_register_icon,
    try_register_icon_on_grid, write_widget_property_by_id, AutoCompleteEdit, Badge, Carousel,
    CarouselIndicatorPosition, CarouselIndicatorStyle, CarouselPage, CollapsiblePane, ColorHistory,
    ColorWell, CommandLink, Divider, DockPanel, DockWidget, EditableComboBox, EmptyState,
    FloatingLabel, FontComboBox, FontPreview, Icon, IconRegistrationError, ImePreedit,
    InplaceEditor, LCDNumber, MaskedEdit, MasonryItem, MasonryLayout, MdiArea, MultiSelectComboBox,
    MultiSelectItem, NumberPicker, OtpInput, ProgressCircle, RangeSlider, RangeSliderOrientation,
    Rating, RegisteredIcon, RichEdit, SafeArea, SafeAreaInsets, SearchBar, SearchBox,
    ShortcutEditor, ShortcutEntry, SkeletonLoader, Splitter, StackedWidget, Stepper, TabWidget,
    TagInput, TextEdit, ToggleButton, ToggleButtonState, ToolBox, Toolbox, WidgetAndDraw,
    WidgetFactory, MAX_REGISTERED_ICONS,
};

#[cfg(full_widgets)]
pub use widget::{
    register_shared_extension, shared_extension_failures, SharedFactoryError,
    SharedFactoryExtension,
};

#[cfg(feature = "image")]
pub use api::{draw_icon_image, Image, ImageFormat, ImageView};

#[cfg(all(feature = "image", not(alloc_frugal)))]
pub use api::SharedIconPixels;

#[cfg(all(feature = "advanced-widgets", feature = "serde_json", not(alloc_frugal)))]
pub use api::{JsonSettings, MemorySettings, SettingValue, Settings, SettingsError, SettingsStore};

#[allow(deprecated)]
pub use api::{
    mount_custom_widget, request_custom_repaint, resize_custom_widget, supports_custom_widgets,
    unmount_custom_widget, NativeCapabilityContract,
};
#[cfg(not(alloc_frugal))]
#[allow(deprecated)]
pub use api::{native_handle, native_menu_item_icons_supported};

/// Stops or restarts the animation clock in the allocation-frugal profile.
///
/// `mini`/`embedded` hold no animation bus (see the [`FrameOutcome`] note below), so there is
/// nothing to stop; accepting the call keeps the mobile lifecycle code free of a profile `cfg`
/// at every call site (principle #53).
#[cfg(alloc_frugal)]
pub fn set_animation_suspended(_suspended: bool) {}

/// Whether the animation clock is stopped in the allocation-frugal profile; always `false`,
/// because there is no clock to stop.
#[cfg(alloc_frugal)]
pub fn animation_suspended() -> bool {
    false
}

/// What one frame did, in the profile that has no animation bus.
///
/// # Why the allocation-frugal profile needs a *type* and not just a function
///
/// `mini` has no `widget::runtime` (`Rc`/`RefCell` widget state is what that profile exists
/// to avoid), so it has no animation bus and no driver to speak of. But the platform loops
/// are **shared** -- `android`/`ios`/`harmony`/`wasm` compile into `mini` as well as into the
/// desktop profiles -- and each of them calls `crate::drive_frame` once per iteration.
///
/// The alternative would be a `cfg` at every one of those call sites (or a second, differently
/// named function for one profile), which is the API fork principle #53 forbids: a capability
/// difference is a runtime fact, not two function signatures. So the frame entry point exists
/// in every profile, and in this one it reports the truth -- nothing was ticked, and no frame
/// is owed -- rather than pretending to have driven a bus that is not there.
#[cfg(alloc_frugal)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameOutcome {
    /// Events drained from the trigger queue; always `0` in this profile, which has no queue.
    pub events_dispatched: usize,
    /// Controls advanced; always `0`, because this profile holds no widget objects.
    pub controls_ticked: usize,
    /// Repaint requests submitted; always `0`, for the same reason.
    pub repaints_submitted: usize,
    /// Whether another frame is owed; always `false` -- `mini` repaints whole frames on demand
    /// rather than running an animation loop, so it never owes one.
    pub needs_another_frame: bool,
}

/// Drives one frame in the allocation-frugal profile.
///
/// # Why this is a real function and not a stub
///
/// The plan's wording for this profile is "`drive_frame` is a no-op on `mini`/`embedded`, and no
/// stub is provided" (BLUE24 §1.3). The half of that which matters is the *behaviour*: this
/// profile must not run an animation bus, because it has no clock and no widget state to advance.
/// It still needs an entry point, because the platform loops it shares with the desktop profiles
/// call one.
///
/// So this is not a placeholder: it answers honestly for every field. There is nothing to
/// interpolate toward -- a control in this profile is painted whole, on demand, from its program
/// state -- so `needs_another_frame` is `false` and a loop built on it sleeps until the next event.
///
/// # Why it does not drain the trigger queue here
///
/// It deliberately does **not** call [`drain_triggers`], even though that function compiles in this
/// profile (it returns `0`, because there is no trigger queue). Two reasons, and the first is the
/// one that matters:
///
/// 1. `drain_triggers` has exactly **one** owner -- this crate's frame driver. The gate
///    `check_single_frame_driver` asserts that structurally, and a second caller here would make
///    the assertion false: the order the plan fixes (input → advance → report) would be split
///    across two places, and a desktop build that later copied this arm would silently introduce
///    the "two drains per frame" defect the gate exists to catch.
/// 2. In this profile the drain is a no-op anyway, so calling it would buy nothing and would make
///    the code *look* like it did something. An honest `0` says the same thing without pretending.
#[cfg(alloc_frugal)]
pub fn drive_frame(_delta_ms: u32) -> FrameOutcome {
    FrameOutcome::default()
}

/// Translates a message key, or returns it verbatim when i18n is not compiled in.
///
/// `tr!` needs a **literal** key, so it cannot serve a caller that holds a key at
/// runtime (a control's `set_translated_tooltip`, a data-driven label). Without this
/// function such a caller has to write its own `cfg` pair, which is exactly how
/// `set_translated_tooltip` ended up gating i18n on `desktop` and silently losing
/// translations on `tablet`/`mobile` (principle #41).
///
/// Having one feature-independent entry point means a call site never needs to know
/// whether the catalogue exists.
///
/// # The no-`i18n` behaviour
///
/// Returns the key itself and warns, matching [`tr!`]: a build without the catalogue
/// must not look localized, and an empty string would be worse than the raw key(which
/// at least names what is missing).
#[cfg(feature = "i18n")]
pub fn translate_key(key: &str) -> crate::compat::String {
    crate::i18n::translate(key)
}

/// Translates a message key, or returns it verbatim when i18n is not compiled in.
///
/// See the `i18n` variant's documentation for why this function exists alongside the
/// [`tr!`] macro.
#[cfg(not(feature = "i18n"))]
pub fn translate_key(key: &str) -> crate::compat::String {
    log::warn!("i18n translate_key called but the i18n feature is disabled, key={key}");
    crate::compat::String::from(key)
}
// NOTE: there is no top-level `chart` module. The chart *engine* (layout, axes,
// ticks, SVG context, adapter) and the chart *widgets* live together under
// `crate::widget::chart_widgets`, because they are two layers of one feature.
// The engine is reachable as `rust_widgets::widget::chart_widgets::charts`
// (and `::types`/`::layout`/`::svg`/`::adapter`).
/// Translates a message key — the no-`i18n` fallback spelling.
///
/// This macro exists so that code which calls `tr!` still compiles when the `i18n`
/// feature is off. It performs **no translation**: it logs a warning naming the key
/// and returns the key itself. That makes a missing translation loud during
/// development instead of quietly rendering an empty string, but it also means the
/// returned text is a message *key*, not user-facing copy — a build without `i18n`
/// must not be shipped as a localized one.
///
/// Accepts the same three arities as the real macro (`$key`, `$key, $count`, and
/// `$key, $context, $count`) so call sites need no `cfg` of their own; the plural
/// and context arguments are ignored here.
#[cfg(not(feature = "i18n"))]
#[macro_export]
macro_rules! tr {
    ($key:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
    ($key:expr, $count:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
    ($key:expr, $context:expr, $count:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
}
#[cfg(not(feature = "i18n"))]
/// `tr_into!` appends a translation to a caller-provided buffer.
///
/// This build has no catalogue, so it logs the missing-feature warning that [`tr!`] logs and
/// appends the key. It accepts the same arities as the `i18n` build's macro so call sites need
/// no `cfg` of their own.
#[macro_export]
macro_rules! tr_into {
    ($key:expr, $out:expr) => {{
        log::warn!("i18n tr_into! called but the i18n feature is disabled, key={}", $key);
        $out.push_str($key);
    }};
    ($key:expr, $count:expr, $out:expr) => {{
        log::warn!("i18n tr_into! called but the i18n feature is disabled, key={}", $key);
        let _ = $count;
        $out.push_str($key);
    }};
    ($key:expr, $context:expr, $count:expr, $out:expr) => {{
        log::warn!("i18n tr_into! called but the i18n feature is disabled, key={}", $key);
        let _ = ($context, $count);
        $out.push_str($key);
    }};
}
/// Application lifecycle wrapper and type-safe widget handles (not available in mini mode).
///
/// Same condition as `crate::json`: `full_widgets`.
#[cfg(full_widgets)]
pub mod app;
/// Index-based widget registry for runtime lookup.
pub mod index;
#[cfg(feature = "pdf")]
/// PDF rendering/export support.
pub mod pdf;
#[cfg(feature = "print")]
/// Print and preview support.
pub mod print;

/// Logs the resolved profile/backend/route when `RUST_WIDGETS_TRACE_RUNTIME=1`.
///
/// # Why this writes to stderr as well as the log
///
/// `log::info!` goes to the `log` facade, and this crate installs no logger on
/// desktop builds (`src/platform/android_jni.rs` does, via logcat, and that is the
/// only one). A program that has not installed one therefore saw **nothing** when it
/// asked for the trace, so the runtime audit BLUE15 #55 requires could not actually
/// be performed: `RUST_WIDGETS_TRACE_RUNTIME=1` printed an empty line and the route
/// stayed unverified. Writing the same record to stderr when the variable is set
/// makes the audit work out of the box, while the `log` record keeps the event
/// available to a host that does install a logger. The stderr write is opt-in — the
/// variable has to be exactly `1` — so a normal run prints nothing.
pub(crate) fn trace_runtime_route(stage: &str) {
    if std::env::var("RUST_WIDGETS_TRACE_RUNTIME").ok().as_deref() != Some("1") {
        return;
    }

    let line = format!(
        "[rust_widgets.runtime] stage={} profile={} backend={} route={} host={}",
        stage,
        platform::profile::profile_name(),
        platform::platform_facts().backend_name(),
        platform::profile::route_name(),
        platform::profile::host_name()
    );
    log::info!("{line}");
    eprintln!("{line}");
}
// Convenient wrapper functions for platform operations
// Users can call these directly without manually getting a platform instance

/// Resolve the backend that should create a widget of this kind.
///
/// # Why this exists
///
/// A widget kind may map onto a real platform primitive (a `Button` on Win32/
/// AppKit/GTK) or have no primitive at all (`Chart`, `CodeEditor`, …), in which
/// case the platform layer supplies the surface. **Which of the two happens is a
/// backend decision and must not leak to callers**: this function is the single
/// place the creation path asks for it, so the choice cannot drift between call
/// sites (see `control_backend::dispatcher`).
///
/// On a profile without an OS runtime (`mini`, `embedded`) the custom state
/// backend answers instead, so the same call works everywhere.
#[cfg(not(alloc_frugal))]
fn backend_for_kind(kind: widget::WidgetKind) -> &'static dyn control_backend::ControlBackend {
    control_backend::get_control_backend_for_widget(kind)
}

// ── Absent kinds do not need a stand-in ──
//
// `mini`/`embedded` compile out `MenuBar`, `Menu`, `ToolBar`, `StatusBar`,
// `ListView`, the dialogs and their `MessageBox`. The `create_*` functions that
// route on them must still **exist** in every profile so the public surface does
// not vary (principle #53), and the way that is expressed here is a single
// `#[cfg(widgets_unstripped)]` gate on the body plus an honest failure otherwise.
//
// # The dead layer this replaced
//
// There used to be nine `KIND_*` const pairs, each naming the variant under
// `widgets_unstripped` and substituting `WidgetKind::Panel` otherwise, all fed
// into `backend_for_kind(...)`. **That value was never read.**
// `get_control_backend_for_widget` discards its argument (there is one creation
// mechanism, so there is nothing to resolve), and every backend `create_*`
// hardcodes the kind it mounts (`create_menu_bar` mounts `WidgetKind::MenuBar`
// inside `control_backend/custom`). So the stand-in never reached a widget, and
// repainting its gate — to `widgets_unstripped`, to `full_widgets`, to anything —
// changed nothing observable. A prior round recorded a `KIND_LIST_VIEW` gate fix
// as a behaviour change; it was not one.
//
// The reachable defect was the opposite of what the alias pair implied: on `mini`
// and `embedded` the body still called `create_menu_bar`, which the custom backend
// used to answer with a valid id addressing **nothing**, so a caller on those
// profiles held an id that failed every later call. The body is now gated, so those
// profiles get `0` — the documented "no such control here" answer, matching what
// `tests/kind_alias_gate_test.rs` pins for the profiles that do ship the control.

/// The id returned when the running profile does not ship the requested control.
///
/// Not `Panel`'s id, and not any id that would address a different control: a
/// caller that receives `0` can test for it, whereas a valid id addressing the
/// wrong thing cannot be detected at all. `widget::runtime` documents `0` as "no
/// widget", and the C ABI already reports it the same way.
///
/// Gated `not(widgets_unstripped)` because that is exactly where it is consumed:
/// on a profile that ships the whole widget set every `create_*` reaches its real
/// constructor and the fallback branch is compiled out.
#[cfg(all(not(alloc_frugal), not(widgets_unstripped)))]
const NO_SUCH_CONTROL: crate::core::ObjectId = 0;

/// Mounts a widget object on the platform's surface, or reports why it cannot.
///
/// This is the **single implementation** of the register → mount → roll back on
/// failure sequence, used by both [`create_widget_of_kind`] and
/// [`app::WindowHandle::mount_surface`]. Keeping one
/// copy is what stops the two paths from disagreeing about ownership: on any
/// failure the widget is unregistered, so the registry never holds a widget the
/// backend is not showing.
///
/// # Resolving the parent
///
/// `parent` may be either a **window widget** (what [`create_window`] returns and
/// what `App::new_window` hands out) or a **host window** (what
/// `Platform::create_window` returns). `mount_surface` is a `Platform` method, so
/// it can only resolve the latter — the platform knows nothing of the widget
/// registry. Translating here, at the one place that already owns both the widget
/// registry and the platform handle, is what lets a caller mount controls on the
/// window it created. Without it the library's own windows could not carry the
/// library's own controls, because the two id spaces never met.
///
/// Applies the active theme to a widget that is about to be registered.
///
/// # Why this shim exists at the crate root
///
/// Both creation funnels (`mount_widget_object` here and
/// `CustomPaintControlBackend::mount_widget_of_kind`) must apply the theme, and
/// neither may carry its own `#[cfg]` — a call-site gate is exactly where the
/// condition drifts away from the module's own gate. That drift broke the
/// `embedded` build twice: once for the JSON loader, once here.
///
/// So the call is unconditional and the *body* is gated, which means the condition
/// is written once, next to the module it describes.
#[cfg(not(alloc_frugal))]
fn apply_active_theme(widget: &mut Box<dyn widget::Widget>) {
    #[cfg(device_profile)]
    {
        crate::theme::apply_active_theme(&mut **widget);
    }
    #[cfg(not(device_profile))]
    {
        // No theme module in this profile, so there is nothing to apply. Naming the
        // parameter keeps the signature identical in every build.
        let _ = widget;
    }
}

/// Returns `Ok(id)` with the widget live in the registry, or `Err(reason)`.
#[cfg(not(alloc_frugal))]
fn mount_widget_object(
    parent: crate::core::ObjectId,
    mut widget: Box<dyn widget::Widget>,
    rect: crate::core::Rect,
) -> Result<crate::core::ObjectId, widget::runtime::SurfaceMountError> {
    use widget::runtime::SurfaceMountError;

    // Apply the active theme before the widget becomes visible. This is one of the
    // two funnels every created control passes through (the other is
    // `CustomPaintControlBackend::mount_widget_of_kind`), which is what makes a
    // theme switch affect controls created through the C ABI and not only those the
    // JSON loader builds. An explicit style the caller or the constructor already
    // set is preserved; see `theme::apply`.
    apply_active_theme(&mut widget);

    // Register first: the backend looks the widget up by id on every repaint.
    let id = widget::runtime::register(widget).ok_or(SurfaceMountError::NoRegistryOnThread)?;
    widget::runtime::set_geometry(id, rect);
    // The surface is not a child in the widget tree, so this is the only record that
    // it belongs to `parent`. Cursor dispatch and repaint routing both need it.
    widget::runtime::set_surface_window(id, parent);

    let host_parent = widget::runtime::host_window_for(parent).unwrap_or(parent);

    if !platform::get_platform().mount_surface(host_parent, id, rect) {
        // Do not leave a widget stranded when the backend refused to show it.
        widget::runtime::unregister(id);
        if !supports_surfaces() {
            return Err(SurfaceMountError::UnsupportedByBackend(platform::backend_name()));
        }
        return Err(SurfaceMountError::RejectedByBackend(platform::backend_name()));
    }
    Ok(id)
}

// ═══════════════════════════════════════════════════════
// Deprecated aliases kept for API compatibility (principle #21)
// ═══════════════════════════════════════════════════════

/// Deprecated spellings of the surface API.
///
/// These names said "custom", which described a rendering mechanism — "painted
/// by the library rather than mapped onto an OS control". That distinction no
/// longer exists: the host platform supplies only a window and a drawing surface
/// and the library paints every widget, so the mechanism vocabulary has no
/// referent. Use the `*_surface` names instead.
///
/// The forwards are re-exported at the crate root below, so an existing
/// `rust_widgets::mount_custom_widget(...)` call keeps compiling and only gains a
/// deprecation warning — the caller is told the name changed rather than being
/// broken by it (principle #21).
///
/// # Capability vocabulary (H-318/H-319)
///
/// "Native" described an implementation mechanism, not the capability split the type
/// carries. The canonical spellings are `HostedCapabilityContract`,
/// `CapabilityContract::Hosted` and `Platform::hosted_capability_contract`; the type alias
/// and trait method keep a deprecated forward for one release. The enum variant itself was
/// renamed (an enum variant cannot be aliased), so `CapabilityContract::Native(..)` becomes
/// `CapabilityContract::Hosted(..)`. `native_menu_item_icons_supported()` is now
/// [`menu_item_icons_supported()`], with the old name forwarded and deprecated.
#[allow(deprecated)]
/// # Reachability
///
/// **State:** Reserved: the compatibility shim for APIs replaced in earlier
/// rounds; it exists so old call sites keep compiling. Removal condition: on the
/// next major version.
///
/// Deprecated aliases for APIs renamed in earlier rounds.
///
/// Every function here forwards to its replacement and carries a `#[deprecated]`
/// note naming that replacement, so a caller gets a compiler warning that says
/// what to use instead.
pub mod deprecated {
    /// Deprecated alias of [`crate::mount_surface`].
    #[deprecated(
        note = "renamed to `mount_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn mount_custom_widget(
        parent: crate::core::ObjectId,
        id: crate::core::ObjectId,
        rect: crate::core::Rect,
    ) -> bool {
        crate::mount_surface(parent, id, rect)
    }

    /// Deprecated alias of [`crate::resize_surface`].
    #[deprecated(
        note = "renamed to `resize_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn resize_custom_widget(id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
        crate::resize_surface(id, rect)
    }

    /// Deprecated alias of [`crate::unmount_surface`].
    #[deprecated(
        note = "renamed to `unmount_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn unmount_custom_widget(id: crate::core::ObjectId) -> bool {
        crate::unmount_surface(id)
    }

    /// Deprecated alias of [`crate::invalidate_surface`].
    #[deprecated(
        note = "renamed to `invalidate_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn request_custom_repaint(id: crate::core::ObjectId) -> bool {
        crate::invalidate_surface(id)
    }

    /// Deprecated alias of [`crate::supports_surfaces`].
    #[deprecated(
        note = "renamed to `supports_surfaces`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn supports_custom_widgets() -> bool {
        crate::supports_surfaces()
    }

    /// Deprecated alias of [`crate::backend_handle`].
    ///
    /// Gated with its replacement: the `mini` profile has no platform singleton to ask,
    /// so `backend_handle` does not exist there and an ungated alias would name a
    /// missing function.
    #[cfg(not(alloc_frugal))]
    #[deprecated(
        note = "renamed to `backend_handle`; 'native' named a mechanism, and which object a \
                backend holds is its own business (principle #52)"
    )]
    pub fn native_handle(widget: crate::core::ObjectId) -> Option<usize> {
        crate::backend_handle(widget)
    }
}

#[cfg(test)]
mod compat_path_tests {
    /// The old crate-root spellings must keep resolving after the rename.
    ///
    /// The rename is a *deprecation*, not a break (principle #21): existing callers
    /// must still compile. Referring to the functions here fails the build if a
    /// re-export is ever dropped, which is otherwise invisible because nothing else
    /// in the crate uses the old names.
    #[test]
    #[allow(deprecated)]
    fn old_surface_names_still_resolve_at_the_crate_root() {
        let _: fn(crate::core::ObjectId, crate::core::ObjectId, crate::core::Rect) -> bool =
            crate::mount_custom_widget;
        let _: fn(crate::core::ObjectId, crate::core::Rect) -> bool = crate::resize_custom_widget;
        let _: fn(crate::core::ObjectId) -> bool = crate::unmount_custom_widget;
        let _: fn(crate::core::ObjectId) -> bool = crate::request_custom_repaint;
        let _: fn() -> bool = crate::supports_custom_widgets;
    }

    /// The deprecated forwards must reach the same function, not a stale copy.
    #[test]
    #[allow(deprecated)]
    fn deprecated_aliases_delegate_to_the_surface_api() {
        assert_eq!(crate::supports_custom_widgets(), crate::supports_surfaces());
    }
}
