"use strict";

// ---------------------------------------------------------------------------
// Node.js binding for rust-widgets native GUI library
// Uses ffi-napi to load librust_widgets and wraps every C ABI function
// in a clean, idiomatic JavaScript API.
// ---------------------------------------------------------------------------

const ffi = require("ffi-napi");
const ref = require("ref-napi");
const Struct = require("ref-struct-napi");

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------
const uint8 = ref.types.uint8;
const uint16 = ref.types.uint16;
const uint32 = ref.types.uint32;
const uint64 = ref.types.uint64;
const int8 = ref.types.int8;
const int16 = ref.types.int16;
const int32 = ref.types.int32;
const int = ref.types.int;
const uint = ref.types.uint;
const cfloat = ref.types.float;
const cbool = ref.types.bool;
const void_t = ref.types.void;
const char_t = ref.types.char;

// Pointer types
const charPtr = ref.refType(char_t);
const uint64Ptr = ref.refType(uint64);
const intPtr = ref.refType(int);
const uintPtr = ref.refType(uint);
const bytePtr = ref.refType(uint8);
const bytePtrPtr = ref.refType(bytePtr);
const charPtrPtr = ref.refType(charPtr);

// ---------------------------------------------------------------------------
// Helper: read a uint64 return buffer as a JS number
// ffi-napi returns uint64 as a Buffer; widget IDs are small so Number is safe.
// ---------------------------------------------------------------------------
function readUint64(buf) {
  if (buf === 0 || buf === null || buf === undefined) return 0;
  if (Buffer.isBuffer(buf)) {
    // Read as BigInt then coerce — safe for widget IDs (< 2^53)
    return Number(buf.readBigUInt64LE(0));
  }
  return Number(buf);
}

// ---------------------------------------------------------------------------
// Helper: read a C string from a pointer, free it, return JS string
// ---------------------------------------------------------------------------
function readAndFreeString(lib, ptr) {
  if (!ptr || ptr.isNull()) return "";
  const str = ref.readCString(ptr);
  lib.rust_widgets_free_string(ptr);
  return str;
}

// ---------------------------------------------------------------------------
// Helper: allocate a typed buffer for output pointer parameters
// ---------------------------------------------------------------------------
function allocOut(type) {
  return ref.alloc(type);
}

// ---------------------------------------------------------------------------
// Library loading with fallback paths
// ---------------------------------------------------------------------------
let _lib = null;

function getLibrary() {
  if (_lib) return _lib;

  const envPath = process.env.RUST_WIDGETS_LIB_PATH;
  const candidates = envPath
    ? [envPath]
    : [
        "librust_widgets",
        "librust_widgets.so",
        "rust_widgets",
        "./target/release/librust_widgets",
        "./target/debug/librust_widgets",
        "./target/release/librust_widgets.so",
        "./target/debug/librust_widgets.so",
      ];

  let lastError;
  for (const name of candidates) {
    try {
      _lib = loadFunctions(name);
      return _lib;
    } catch (err) {
      lastError = err;
    }
  }

  throw new Error(
    "Cannot load rust-widgets native library.\n" +
      "  Searched: " +
      candidates.join(", ") +
      "\n" +
      "  Set RUST_WIDGETS_LIB_PATH env var to specify the path.\n" +
      "  Original error: " +
      (lastError ? lastError.message : "unknown"),
  );
}

// ---------------------------------------------------------------------------
// Define all C ABI function bindings
// ---------------------------------------------------------------------------
function loadFunctions(libName) {
  const lib = ffi.Library(libName, {
    // ── Core ────────────────────────────────────────────────────────────
    rust_widgets_init: [void_t, []],
    rust_widgets_run: [void_t, []],
    rust_widgets_quit: [void_t, []],

    // ── Widget create (return uint64 widget ID; 0 = error) ─────────────
    rust_widgets_create_window: [uint64, ["string", int, int, uint, uint]],
    rust_widgets_create_button: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rust_widgets_create_checkbox: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rust_widgets_create_line_edit: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rust_widgets_create_label: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rust_widgets_create_radio_button: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rust_widgets_create_slider: [uint64, [uint64, int, int, uint, uint]],
    rust_widgets_create_progress_bar: [uint64, [uint64, int, int, uint, uint]],
    rust_widgets_create_combo_box: [uint64, [uint64, int, int, uint, uint]],
    rust_widgets_create_list_box: [uint64, [uint64, int, int, uint, uint]],
    rust_widgets_create_panel: [uint64, [uint64, int, int, uint, uint]],
    rust_widgets_create_message_box: [
      uint64,
      [uint64, "string", "string", int, int, uint, uint],
    ],
    rust_widgets_create_file_dialog: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rust_widgets_create_color_dialog: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rust_widgets_create_font_dialog: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rust_widgets_create_spin_box: [uint64, [uint64, int, int, uint, uint]],
    rust_widgets_create_list_view: [uint64, [uint64, int, int, uint, uint]],
    rust_widgets_create_scroll_area: [uint64, [uint64, int, int, uint, uint]],
    rust_widgets_create_menu_bar: [uint64, [uint64, int, int, uint, uint]],
    rust_widgets_create_menu: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rust_widgets_create_tool_bar: [uint64, [uint64, int, int, uint, uint]],
    rust_widgets_create_status_bar: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],

    // ── Widget manipulation ────────────────────────────────────────────
    rust_widgets_show_widget: [void_t, [uint64]],
    rust_widgets_hide_widget: [void_t, [uint64]],
    rust_widgets_set_widget_text: [void_t, [uint64, "string"]],
    rust_widgets_get_widget_text: [charPtr, [uint64]],
    rust_widgets_set_widget_enabled: [void_t, [uint64, cbool]],
    rust_widgets_is_widget_enabled: [cbool, [uint64]],
    rust_widgets_set_widget_visible: [void_t, [uint64, cbool]],
    rust_widgets_is_widget_visible: [cbool, [uint64]],
    rust_widgets_set_widget_geometry: [void_t, [uint64, int, int, uint, uint]],
    rust_widgets_get_widget_geometry: [
      cbool,
      [uint64, intPtr, intPtr, uintPtr, uintPtr],
    ],
    rust_widgets_set_widget_ime_enabled: [cbool, [uint64, cbool]],
    rust_widgets_is_widget_ime_enabled: [cbool, [uint64]],

    // ── Accessibility ──────────────────────────────────────────────────
    rust_widgets_set_widget_accessibility_name: [cbool, [uint64, "string"]],
    rust_widgets_get_widget_accessibility_name: [charPtr, [uint64]],

    // ── Menu ───────────────────────────────────────────────────────────
    rust_widgets_attach_menu_bar_to_window: [cbool, [uint64, uint64]],
    // Third param is shortcut (nullable) — use charPtr to pass NULL when no shortcut
    rust_widgets_menu_add_item: [uint64, [uint64, "string", charPtr]],
    rust_widgets_poll_menu_triggered: [uint64, []],

    // ── Events ─────────────────────────────────────────────────────────
    rust_widgets_poll_widget_triggered: [uint64, []],
    rust_widgets_poll_widget_trigger_event: [uint, [uint64Ptr]],
    rust_widgets_inject_widget_trigger_event: [cbool, [uint64, uint]],
    rust_widgets_inject_menu_trigger: [cbool, [uint64]],

    // ── Combo Box ──────────────────────────────────────────────────────
    rust_widgets_combo_box_add_item: [cbool, [uint64, "string"]],
    rust_widgets_combo_box_clear_items: [cbool, [uint64]],
    rust_widgets_combo_box_set_current_index: [cbool, [uint64, uint]],
    rust_widgets_combo_box_current_index: [int, [uint64]],
    rust_widgets_combo_box_item_count: [uint, [uint64]],
    rust_widgets_combo_box_item_text: [charPtr, [uint64, uint]],

    // ── List Box ───────────────────────────────────────────────────────
    rust_widgets_list_box_add_item: [cbool, [uint64, "string"]],
    rust_widgets_list_box_remove_item: [cbool, [uint64, uint]],
    rust_widgets_list_box_clear_items: [cbool, [uint64]],
    rust_widgets_list_box_set_current_index: [cbool, [uint64, uint]],
    rust_widgets_list_box_current_index: [int, [uint64]],
    rust_widgets_list_box_item_count: [uint, [uint64]],
    rust_widgets_list_box_item_text: [charPtr, [uint64, uint]],

    // ── Clipboard ──────────────────────────────────────────────────────
    rust_widgets_set_clipboard_text: [cbool, ["string"]],
    rust_widgets_get_clipboard_text: [charPtr, []],

    // ── Drag & Drop ────────────────────────────────────────────────────
    rust_widgets_begin_drag: [cbool, [uint64, "string", bytePtr, uint]],
    rust_widgets_poll_drop_event: [
      cbool,
      [uint64Ptr, uint64Ptr, charPtrPtr, bytePtrPtr, uintPtr],
    ],

    // ── Platform ───────────────────────────────────────────────────────
    rust_widgets_backend_name: [charPtr, []],
    rust_widgets_platform_capabilities: [uint, []],
    rust_widgets_platform_capability_contract: [uint, [uint]],
    rust_widgets_platform_dpi_scale_factor: [cfloat, []],
    rust_widgets_bindings_api_version: [uint, []],

    // ── Binding status (language-specific) ─────────────────────────────
    rust_widgets_nodejs_binding_status: [uint, []],
    rust_widgets_python_binding_status: [uint, []],
    rust_widgets_cpp_binding_status: [uint, []],
    rust_widgets_java_binding_status: [uint, []],
    rust_widgets_java_jni_skeleton_version: [uint, []],
    rust_widgets_python_reserved: [uint, []],
    rust_widgets_cpp_reserved: [uint, []],
    rust_widgets_java_reserved: [uint, []],

    // ── Render ─────────────────────────────────────────────────────────
    rust_widgets_set_render_aa_samples_per_axis: [uint, [uint]],
    rust_widgets_get_render_aa_samples_per_axis: [uint, []],

    // ── Embedded engine ────────────────────────────────────────────────
    rust_widgets_set_embedded_target_fps: [uint, [uint]],
    rust_widgets_get_embedded_target_fps: [uint, []],
    rust_widgets_submit_embedded_noop_task: [uint64, ["string"]],
    rust_widgets_embedded_engine_is_initialized: [cbool, []],
    rust_widgets_embedded_engine_is_running: [cbool, []],
    rust_widgets_embedded_engine_frame_count: [uint64, []],
    rust_widgets_embedded_engine_pending_task_count: [uint64, []],
    rust_widgets_embedded_engine_window_count: [uint64, []],
    rust_widgets_embedded_engine_button_count: [uint64, []],

    // ── Memory ─────────────────────────────────────────────────────────
    rust_widgets_free_string: [void_t, [charPtr]],
    rust_widgets_free_rust_string: [void_t, [charPtr]],

    // ── Mobile ─────────────────────────────────────────────────────────
    rust_widgets_mobile_backend_name: [charPtr, []],
    rust_widgets_mobile_attach_native_view: [cbool, [uint64]],

    // ── Harmony ────────────────────────────────────────────────────────
    rust_widgets_harmony_on_menu_item: [cbool, [uint64]],
    rust_widgets_harmony_on_click: [cbool, [uint64]],
    rust_widgets_harmony_on_value_changed: [cbool, [uint64]],
    rust_widgets_harmony_on_widget_event: [cbool, [uint64, uint]],
    rust_widgets_harmony_bind_node: [cbool, [uint64, uint64]],
    rust_widgets_harmony_unbind_node: [cbool, [uint64]],
    rust_widgets_harmony_lookup_widget_id: [uint64, [uint64]],
    rust_widgets_harmony_clear_node_bindings: [void_t, []],
    rust_widgets_harmony_on_node_menu_item: [cbool, [uint64]],
    rust_widgets_harmony_on_node_click: [cbool, [uint64]],
    rust_widgets_harmony_on_node_value_changed: [cbool, [uint64]],
    rust_widgets_harmony_on_node_widget_event: [cbool, [uint64, uint]],

    // ── Error helpers (from rust_widgets_errors.h) ─────────────────────
    rust_widgets_error_message: [charPtr, [uint64]],
    rust_widgets_error_code: [int32, [uint64]],
  });

  return lib;
}

// ---------------------------------------------------------------------------
// Wrapper that reads a string return and frees the C buffer
// ---------------------------------------------------------------------------
function wrapStringFn(lib, fnName) {
  return function (...args) {
    const ptr = lib[fnName](...args);
    return readAndFreeString(lib, ptr);
  };
}

// ---------------------------------------------------------------------------
// Widget trigger kind constants (matches C ABI codes)
// ---------------------------------------------------------------------------
const TriggerKind = Object.freeze({
  None: 0,
  Clicked: 1,
  ValueChanged: 2,
  SelectionChanged: 3,
  Closed: 4,
});

// ---------------------------------------------------------------------------
// Platform capability bitmask constants
// ---------------------------------------------------------------------------
const PlatformCapability = Object.freeze({
  DpiScaling: 1 << 0,
  Ime: 1 << 1,
  Accessibility: 1 << 2,
  NativeMenu: 1 << 3,
  TypedWidgetTrigger: 1 << 4,
});

// ---------------------------------------------------------------------------
// Drag & Drop event object (returned by pollDropEvent)
// ---------------------------------------------------------------------------
function DropEvent(source, target, mime, payload) {
  this.sourceWidgetId = source;
  this.targetWidgetId = target;
  this.mimeType = mime;
  this.payload = payload;
}

// ---------------------------------------------------------------------------
// Geometry object (returned by getWidgetGeometry)
// ---------------------------------------------------------------------------
function WidgetGeometry(x, y, width, height) {
  this.x = x;
  this.y = y;
  this.width = width;
  this.height = height;
}

// ---------------------------------------------------------------------------
// TriggerEvent object (returned by pollWidgetTriggerEvent)
// ---------------------------------------------------------------------------
function TriggerEvent(widgetId, kind) {
  this.widgetId = widgetId;
  this.kind = kind;
}

// ---------------------------------------------------------------------------
// ─── RustWidgets API Class ──────────────────────────────────────────────
// ---------------------------------------------------------------------------
class RustWidgets {
  constructor() {
    if (RustWidgets._instance) {
      return RustWidgets._instance;
    }

    const lib = getLibrary();

    // Store raw library
    this._lib = lib;

    // Wrap string-returning functions for auto-free
    this._getWidgetText = wrapStringFn(lib, "rust_widgets_get_widget_text");
    this._getWidgetAccessibilityName = wrapStringFn(
      lib,
      "rust_widgets_get_widget_accessibility_name",
    );
    this._comboBoxItemText = wrapStringFn(
      lib,
      "rust_widgets_combo_box_item_text",
    );
    this._listBoxItemText = wrapStringFn(
      lib,
      "rust_widgets_list_box_item_text",
    );
    this._getClipboardText = wrapStringFn(
      lib,
      "rust_widgets_get_clipboard_text",
    );
    this._backendName = wrapStringFn(lib, "rust_widgets_backend_name");
    this._errorMessage = wrapStringFn(lib, "rust_widgets_error_message");
    this._mobileBackendName = wrapStringFn(
      lib,
      "rust_widgets_mobile_backend_name",
    );

    RustWidgets._instance = this;
  }

  // ── Singleton access ──────────────────────────────────────────────
  static getInstance() {
    return new RustWidgets();
  }

  // ── Core lifecycle ─────────────────────────────────────────────────

  /** Initialize the GUI library (must be called before creating widgets). */
  init() {
    this._lib.rust_widgets_init();
  }

  /** Enter the main event loop (blocking — call on dedicated thread). */
  run() {
    this._lib.rust_widgets_run();
  }

  /** Signal the event loop to quit. */
  quit() {
    this._lib.rust_widgets_quit();
  }

  // ── Widget creation ───────────────────────────────────────────────

  /**
   * Create a window.
   * @param {string} title
   * @param {number} x
   * @param {number} y
   * @param {number} width
   * @param {number} height
   * @returns {number} widget ID (0 = error)
   */
  createWindow(title, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_window(title, x, y, width, height),
    );
  }

  createButton(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_button(parent, text, x, y, width, height),
    );
  }

  createCheckbox(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_checkbox(parent, text, x, y, width, height),
    );
  }

  createLineEdit(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_line_edit(
        parent,
        text,
        x,
        y,
        width,
        height,
      ),
    );
  }

  createLabel(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_label(parent, text, x, y, width, height),
    );
  }

  createRadioButton(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_radio_button(
        parent,
        text,
        x,
        y,
        width,
        height,
      ),
    );
  }

  createSlider(parent, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_slider(parent, x, y, width, height),
    );
  }

  createProgressBar(parent, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_progress_bar(parent, x, y, width, height),
    );
  }

  createComboBox(parent, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_combo_box(parent, x, y, width, height),
    );
  }

  createListBox(parent, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_list_box(parent, x, y, width, height),
    );
  }

  createPanel(parent, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_panel(parent, x, y, width, height),
    );
  }

  createMessageBox(parent, title, text, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_message_box(
        parent,
        title,
        text,
        x,
        y,
        width,
        height,
      ),
    );
  }

  createFileDialog(parent, title, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_file_dialog(
        parent,
        title,
        x,
        y,
        width,
        height,
      ),
    );
  }

  createColorDialog(parent, title, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_color_dialog(
        parent,
        title,
        x,
        y,
        width,
        height,
      ),
    );
  }

  createFontDialog(parent, title, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_font_dialog(
        parent,
        title,
        x,
        y,
        width,
        height,
      ),
    );
  }

  createSpinBox(parent, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_spin_box(parent, x, y, width, height),
    );
  }

  createListView(parent, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_list_view(parent, x, y, width, height),
    );
  }

  createScrollArea(parent, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_scroll_area(parent, x, y, width, height),
    );
  }

  createMenuBar(parent, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_menu_bar(parent, x, y, width, height),
    );
  }

  createMenu(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_menu(parent, text, x, y, width, height),
    );
  }

  createToolBar(parent, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_tool_bar(parent, x, y, width, height),
    );
  }

  createStatusBar(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rust_widgets_create_status_bar(
        parent,
        text,
        x,
        y,
        width,
        height,
      ),
    );
  }

  // ── Widget manipulation ───────────────────────────────────────────

  showWidget(widgetId) {
    this._lib.rust_widgets_show_widget(widgetId);
  }

  hideWidget(widgetId) {
    this._lib.rust_widgets_hide_widget(widgetId);
  }

  setWidgetText(widgetId, text) {
    this._lib.rust_widgets_set_widget_text(widgetId, text);
  }

  getWidgetText(widgetId) {
    return this._getWidgetText(widgetId);
  }

  setWidgetEnabled(widgetId, enabled) {
    this._lib.rust_widgets_set_widget_enabled(widgetId, enabled);
  }

  isWidgetEnabled(widgetId) {
    return this._lib.rust_widgets_is_widget_enabled(widgetId);
  }

  setWidgetVisible(widgetId, visible) {
    this._lib.rust_widgets_set_widget_visible(widgetId, visible);
  }

  isWidgetVisible(widgetId) {
    return this._lib.rust_widgets_is_widget_visible(widgetId);
  }

  setWidgetGeometry(widgetId, x, y, width, height) {
    this._lib.rust_widgets_set_widget_geometry(widgetId, x, y, width, height);
  }

  /**
   * Get widget geometry.
   * @returns {WidgetGeometry|null} geometry object or null if widget not found
   */
  getWidgetGeometry(widgetId) {
    const xOut = allocOut(int);
    const yOut = allocOut(int);
    const wOut = allocOut(uint);
    const hOut = allocOut(uint);
    const ok = this._lib.rust_widgets_get_widget_geometry(
      widgetId,
      xOut,
      yOut,
      wOut,
      hOut,
    );
    if (!ok) return null;
    return new WidgetGeometry(
      xOut.deref(),
      yOut.deref(),
      wOut.deref(),
      hOut.deref(),
    );
  }

  setWidgetImeEnabled(widgetId, enabled) {
    return this._lib.rust_widgets_set_widget_ime_enabled(widgetId, enabled);
  }

  isWidgetImeEnabled(widgetId) {
    return this._lib.rust_widgets_is_widget_ime_enabled(widgetId);
  }

  // ── Accessibility ─────────────────────────────────────────────────

  setWidgetAccessibilityName(widgetId, name) {
    return this._lib.rust_widgets_set_widget_accessibility_name(widgetId, name);
  }

  getWidgetAccessibilityName(widgetId) {
    return this._getWidgetAccessibilityName(widgetId);
  }

  // ── Menu ──────────────────────────────────────────────────────────

  attachMenuBarToWindow(windowId, menuBarId) {
    return this._lib.rust_widgets_attach_menu_bar_to_window(
      windowId,
      menuBarId,
    );
  }

  menuAddItem(parentMenu, text, shortcut) {
    // shortcut may be null/undefined — pass NULL pointer so Rust sees None
    const scPtr = shortcut == null ? ref.NULL : shortcut;
    return readUint64(
      this._lib.rust_widgets_menu_add_item(parentMenu, text, scPtr),
    );
  }

  pollMenuTriggered() {
    return readUint64(this._lib.rust_widgets_poll_menu_triggered());
  }

  // ── Events ────────────────────────────────────────────────────────

  pollWidgetTriggered() {
    return readUint64(this._lib.rust_widgets_poll_widget_triggered());
  }

  /**
   * Poll the next widget trigger event with typed kind code.
   * @returns {TriggerEvent|null} event object, or null if no pending event
   */
  pollWidgetTriggerEvent() {
    const idOut = allocOut(uint64);
    const kind = this._lib.rust_widgets_poll_widget_trigger_event(idOut);
    if (kind === 0) return null;
    return new TriggerEvent(readUint64(idOut.deref()), kind);
  }

  injectWidgetTriggerEvent(widgetId, kindCode) {
    return this._lib.rust_widgets_inject_widget_trigger_event(
      widgetId,
      kindCode,
    );
  }

  injectMenuTrigger(menuItemId) {
    return this._lib.rust_widgets_inject_menu_trigger(menuItemId);
  }

  // ── Combo Box ─────────────────────────────────────────────────────

  comboBoxAddItem(comboBoxId, text) {
    return this._lib.rust_widgets_combo_box_add_item(comboBoxId, text);
  }

  comboBoxClearItems(comboBoxId) {
    return this._lib.rust_widgets_combo_box_clear_items(comboBoxId);
  }

  comboBoxSetCurrentIndex(comboBoxId, index) {
    return this._lib.rust_widgets_combo_box_set_current_index(
      comboBoxId,
      index,
    );
  }

  comboBoxCurrentIndex(comboBoxId) {
    return this._lib.rust_widgets_combo_box_current_index(comboBoxId);
  }

  comboBoxItemCount(comboBoxId) {
    return this._lib.rust_widgets_combo_box_item_count(comboBoxId);
  }

  comboBoxItemText(comboBoxId, index) {
    return this._comboBoxItemText(comboBoxId, index);
  }

  // ── List Box ──────────────────────────────────────────────────────

  listBoxAddItem(listBoxId, text) {
    return this._lib.rust_widgets_list_box_add_item(listBoxId, text);
  }

  listBoxRemoveItem(listBoxId, index) {
    return this._lib.rust_widgets_list_box_remove_item(listBoxId, index);
  }

  listBoxClearItems(listBoxId) {
    return this._lib.rust_widgets_list_box_clear_items(listBoxId);
  }

  listBoxSetCurrentIndex(listBoxId, index) {
    return this._lib.rust_widgets_list_box_set_current_index(listBoxId, index);
  }

  listBoxCurrentIndex(listBoxId) {
    return this._lib.rust_widgets_list_box_current_index(listBoxId);
  }

  listBoxItemCount(listBoxId) {
    return this._lib.rust_widgets_list_box_item_count(listBoxId);
  }

  listBoxItemText(listBoxId, index) {
    return this._listBoxItemText(listBoxId, index);
  }

  // ── Clipboard ────────────────────────────────────────────────────

  setClipboardText(text) {
    return this._lib.rust_widgets_set_clipboard_text(text);
  }

  getClipboardText() {
    return this._getClipboardText();
  }

  // ── Drag & Drop ───────────────────────────────────────────────────

  beginDrag(sourceId, mimeType, payload) {
    const buf = Buffer.isBuffer(payload) ? payload : Buffer.from(payload);
    return this._lib.rust_widgets_begin_drag(
      sourceId,
      mimeType,
      buf,
      buf.length,
    );
  }

  /**
   * Poll the next drop event.
   * @returns {DropEvent|null} drop event object, or null if no pending event
   */
  pollDropEvent() {
    const srcOut = allocOut(uint64);
    const tgtOut = allocOut(uint64);
    const mimeOut = allocOut(charPtr);
    const payOut = allocOut(bytePtr);
    const lenOut = allocOut(uint);

    const ok = this._lib.rust_widgets_poll_drop_event(
      srcOut,
      tgtOut,
      mimeOut,
      payOut,
      lenOut,
    );
    if (!ok) return null;

    const sourceId = readUint64(srcOut.deref());
    const targetId = readUint64(tgtOut.deref());

    let mime = "";
    const mimePtr = mimeOut.deref();
    if (mimePtr && !mimePtr.isNull()) {
      mime = ref.readCString(mimePtr);
      this._lib.rust_widgets_free_string(mimePtr);
    }

    let payload = null;
    const payPtr = payOut.deref();
    const payLen = lenOut.deref();
    if (payPtr && !payPtr.isNull() && payLen > 0) {
      payload = Buffer.alloc(payLen);
      payPtr.copy(payload, 0, 0, payLen);
      // Free the Rust-allocated payload buffer
      this._lib.rust_widgets_free_string(payPtr);
    }

    return new DropEvent(sourceId, targetId, mime, payload);
  }

  // ── Platform ──────────────────────────────────────────────────────

  backendName() {
    return this._backendName();
  }

  platformCapabilities() {
    return this._lib.rust_widgets_platform_capabilities();
  }

  platformCapabilityContract(profileCode) {
    return this._lib.rust_widgets_platform_capability_contract(profileCode);
  }

  platformDpiScaleFactor() {
    return this._lib.rust_widgets_platform_dpi_scale_factor();
  }

  bindingsApiVersion() {
    return this._lib.rust_widgets_bindings_api_version();
  }

  /**
   * Check if a specific platform capability is available.
   * @param {number} capBit - one of PlatformCapability constants
   * @returns {boolean}
   */
  hasPlatformCapability(capBit) {
    const mask = this.platformCapabilities();
    return (mask & capBit) !== 0;
  }

  // ── Binding status ────────────────────────────────────────────────

  nodejsBindingStatus() {
    // If rust_widgets_nodejs_binding_status exists, call it; otherwise synthesize
    try {
      return this._lib.rust_widgets_nodejs_binding_status();
    } catch (_) {
      // Synthesize: bit0 = C ABI available, bit1 = Node.js binding available
      return (1 << 0) | (1 << 1);
    }
  }

  pythonBindingStatus() {
    return this._lib.rust_widgets_python_binding_status();
  }

  cppBindingStatus() {
    return this._lib.rust_widgets_cpp_binding_status();
  }

  javaBindingStatus() {
    return this._lib.rust_widgets_java_binding_status();
  }

  javaJniSkeletonVersion() {
    return this._lib.rust_widgets_java_jni_skeleton_version();
  }

  // ── Render ────────────────────────────────────────────────────────

  setRenderAaSamplesPerAxis(samples) {
    return this._lib.rust_widgets_set_render_aa_samples_per_axis(samples);
  }

  getRenderAaSamplesPerAxis() {
    return this._lib.rust_widgets_get_render_aa_samples_per_axis();
  }

  // ── Embedded engine ───────────────────────────────────────────────

  setEmbeddedTargetFps(fps) {
    return this._lib.rust_widgets_set_embedded_target_fps(fps);
  }

  getEmbeddedTargetFps() {
    return this._lib.rust_widgets_get_embedded_target_fps();
  }

  submitEmbeddedNoopTask(label) {
    return readUint64(this._lib.rust_widgets_submit_embedded_noop_task(label));
  }

  embeddedEngineIsInitialized() {
    return this._lib.rust_widgets_embedded_engine_is_initialized();
  }

  embeddedEngineIsRunning() {
    return this._lib.rust_widgets_embedded_engine_is_running();
  }

  embeddedEngineFrameCount() {
    return readUint64(this._lib.rust_widgets_embedded_engine_frame_count());
  }

  embeddedEnginePendingTaskCount() {
    return readUint64(
      this._lib.rust_widgets_embedded_engine_pending_task_count(),
    );
  }

  embeddedEngineWindowCount() {
    return readUint64(this._lib.rust_widgets_embedded_engine_window_count());
  }

  embeddedEngineButtonCount() {
    return readUint64(this._lib.rust_widgets_embedded_engine_button_count());
  }

  // ── Mobile ────────────────────────────────────────────────────────

  mobileBackendName() {
    return this._mobileBackendName();
  }

  mobileAttachNativeView(nativeHandle) {
    return this._lib.rust_widgets_mobile_attach_native_view(nativeHandle);
  }

  // ── Harmony ───────────────────────────────────────────────────────

  harmonyOnMenuItem(menuItemId) {
    return this._lib.rust_widgets_harmony_on_menu_item(menuItemId);
  }

  harmonyOnClick(widgetId) {
    return this._lib.rust_widgets_harmony_on_click(widgetId);
  }

  harmonyOnValueChanged(widgetId) {
    return this._lib.rust_widgets_harmony_on_value_changed(widgetId);
  }

  harmonyOnWidgetEvent(widgetId, kindCode) {
    return this._lib.rust_widgets_harmony_on_widget_event(widgetId, kindCode);
  }

  harmonyBindNode(nodeHandle, widgetId) {
    return this._lib.rust_widgets_harmony_bind_node(nodeHandle, widgetId);
  }

  harmonyUnbindNode(nodeHandle) {
    return this._lib.rust_widgets_harmony_unbind_node(nodeHandle);
  }

  harmonyLookupWidgetId(nodeHandle) {
    return readUint64(
      this._lib.rust_widgets_harmony_lookup_widget_id(nodeHandle),
    );
  }

  harmonyClearNodeBindings() {
    this._lib.rust_widgets_harmony_clear_node_bindings();
  }

  harmonyOnNodeMenuItem(nodeHandle) {
    return this._lib.rust_widgets_harmony_on_node_menu_item(nodeHandle);
  }

  harmonyOnNodeClick(nodeHandle) {
    return this._lib.rust_widgets_harmony_on_node_click(nodeHandle);
  }

  harmonyOnNodeValueChanged(nodeHandle) {
    return this._lib.rust_widgets_harmony_on_node_value_changed(nodeHandle);
  }

  harmonyOnNodeWidgetEvent(nodeHandle, kindCode) {
    return this._lib.rust_widgets_harmony_on_node_widget_event(
      nodeHandle,
      kindCode,
    );
  }

  // ── Error helpers ─────────────────────────────────────────────────

  errorMessage(handle) {
    return this._errorMessage(handle);
  }

  errorCode(handle) {
    return this._lib.rust_widgets_error_code(handle);
  }

  // ── Low-level access ──────────────────────────────────────────────

  /**
   * Get the raw ffi-napi Library object (for advanced use).
   * @returns {object}
   */
  getRawLibrary() {
    return this._lib;
  }
}

// ---------------------------------------------------------------------------
// Named exports
// ---------------------------------------------------------------------------
module.exports = {
  RustWidgets,
  TriggerKind,
  PlatformCapability,
  WidgetGeometry,
  DropEvent,
  TriggerEvent,
};
