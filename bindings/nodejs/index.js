"use strict";

// ---------------------------------------------------------------------------
// Node.js binding for rust-widgets native GUI library
// Uses koffi to load librust_widgets and wraps every C ABI function
// in a clean, idiomatic JavaScript API.
//
// # Why koffi rather than ffi-napi
//
// `ffi-napi@4.0.3` cannot be compiled against Node 18+ headers: its own `src/ffi.cc` stores the
// const-env `get-uv-event-loop` pointer in a non-const slot, so `npm install` fails on 18/20/22/24
// (the maintained-looking fork `@lwahonen/ffi-napi@4.0.12` fails identically; overriding
// `node-addon-api` does not help, measured). koffi ships prebuilt binaries, so it installs on every
// supported Node with no compiler at all.
// ---------------------------------------------------------------------------

const koffi = require("koffi");

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------
const uint8 = "uint8";
const uint16 = "uint16";
const uint32 = "uint32";
const uint64 = "uint64";
const int8 = "int8";
const int16 = "int16";
const int32 = "int32";
const int64 = "int64";
const int = "int";
const uint = "uint";
const cfloat = "float";
const cbool = "bool";
const void_t = "void";
const char_t = "char";

// Pointer types.
//
// `char *` is **opaque** on purpose: koffi auto-converts a plain `char *` to a JS string, which would
// lose the pointer this binding has to hand back to `rw_free_string` — and a lost pointer is a leak.
// Measured against the real library: with `koffi.pointer('char')` the out-slot decoded to a string
// and the free became impossible; with the opaque type it decodes to a pointer the free accepts.
const rawString = koffi.opaque("rw_string");
const charPtr = koffi.pointer(rawString);
const uint64Ptr = koffi.pointer(uint64);
const intPtr = koffi.pointer(int);
const int64Ptr = koffi.pointer(int64);
const uintPtr = koffi.pointer(uint);
const bytePtr = koffi.pointer(uint8);
const bytePtrPtr = koffi.pointer(bytePtr);
const charPtrPtr = koffi.pointer(charPtr);

/** koffi's NULL (there is no `NULL`: koffi spells a null pointer as `null`). */
const NULL = null;

// ---------------------------------------------------------------------------
// Helper: read a uint64 return as an exact id
//
// # Why a BigInt rather than a Number
//
// This library's `ObjectId`s sit far above 2^53 — measured against the real ABI:
//
//   rw_create_window -> 6000285942072475648n
//   rw_create_button -> 6000285942072475649n
//
// Doubles are spaced 1024 apart up there, so a `Number` cannot hold either id: `Number(windowId)`
// and `Number(buttonId)` are the **same** value. A `Number(...)` conversion therefore made every
// id-based call address whichever widget the rounded value happened to land on, and
// `widgetPropertyNames(buttonId)` answered with the window's property list.
//
// koffi returns a `uint64` as a BigInt already (measured), so the id stays exact with no conversion
// on the way in either — koffi accepts the BigInt directly where the old layer needed a Number or a
// decimal String.
//
// A caller that compares an id against a literal must compare against a BigInt
// (`id === 0n`, not `id === 0`).
// ---------------------------------------------------------------------------
function readUint64(buf) {
  if (buf === 0 || buf === null || buf === undefined) return 0n;
  if (typeof buf === "bigint") return buf; // koffi's uint64 form: exact by construction
  if (Buffer.isBuffer(buf)) {
    // The Buffer form carries all 64 bits, so this is exact by construction.
    return buf.readBigUInt64LE(0);
  }
  // The decimal-string form: `BigInt` parses it without the rounding a `Number` would apply.
  return BigInt(buf);
}

/** A pointer koffi handed back, with the accessors this file's call sites use. */
class NativePtr {
  constructor(raw) {
    this.raw = raw;
  }

  isNull() {
    return this.raw === null || this.raw === undefined || this.raw === 0 || this.raw === 0n;
  }

  /** Read `length` bytes into a Buffer (a koffi byte decode yields a plain array). */
  bytes(length) {
    if (this.isNull() || length <= 0) return Buffer.alloc(0);
    return Buffer.from(koffi.decode(this.raw, "uint8", length));
  }
}

/** Read a C string from a pointer. koffi's `-1` length means "until the NUL". */
function readCString(ptr) {
  return koffi.decode(ptr, "char", -1);
}

// ---------------------------------------------------------------------------
// Helper: read a C string from a pointer, free it, return JS string
//
// The pointer arrives in two shapes and both are handled here: a `NativePtr` wrapper (what a slot's
// `deref()` returns) and the raw value koffi returns for a `char *` **return type** (a BigInt, since
// the type is opaque). A `char *` return cannot be auto-converted to a JS string, or the pointer
// needed by `rw_free_string` would be gone and every call would leak.
// ---------------------------------------------------------------------------
function isNullPtr(ptr) {
  return ptr === null || ptr === undefined || ptr === 0 || ptr === 0n;
}

function readAndFreeString(lib, ptr) {
  const raw = ptr instanceof NativePtr ? ptr.raw : ptr;
  if (isNullPtr(raw)) return "";
  const str = readCString(raw);
  lib.rw_free_string(raw);
  return str;
}

/**
 * Read a NUL-terminated string out of an **out-slot**, release it, and clear the slot.
 *
 * # Why the slot is cleared rather than just freed
 *
 * The ABI hands a string property's payload back as a pointer written into a caller-allocated slot,
 * and the caller must release it with `rw_free_string`. Leaving the freed pointer in that slot is a
 * dangling reference, and it is not benign: measured against the real library, the sequence
 * *read a string property → free it → make one more ffi call* aborted the process **8 times out of
 * 8** with
 *
 * ```text
 * Fatal error ... Check failed: result.second.
 *   v8::internal::GlobalBackingStoreRegistry::Register
 *   napi_get_typedarray_info
 * ```
 *
 * while the identical sequence with the free removed was clean 8/8 — which is what identifies the
 * stale slot reference (registered as a typed-array backing store) rather than the free itself.
 * Clearing the slot removes the dangling reference, so the next call cannot find it.
 *
 * It also makes the reason the free exists observable: a caller that forgets it leaks one
 * allocation per read, and a caller that reads the slot after it returns sees NULL rather than a
 * pointer into freed memory.
 */
function readAndFreeSlot(lib, slot) {
  const raw = koffi.decode(slot[SLOT_CELL], charPtr);
  if (!raw) return "";
  const text = readCString(raw);
  lib.rw_free_string(raw);
  koffi.encode(slot[SLOT_CELL], charPtr, NULL);
  return text;
}

// ---------------------------------------------------------------------------
// Helper: allocate a typed buffer for output pointer parameters
//
// koffi pointer values are opaque (a BigInt for an opaque `char *`, an external for the rest), so the
// ref-napi-shaped accessors the call sites use — `deref()`, `writePointer()`, and `.isNull()` on
// what `deref()` returns — live on this wrapper instead of on the value itself. `withWireArguments`
// unwraps the wrapper at the call boundary, so a slot is passed exactly like the raw pointer it
// holds and no call site had to change shape.
// ---------------------------------------------------------------------------
const SLOT_CELL = Symbol("koffi cell");
const SLOT_TYPE = Symbol("koffi slot type");

function allocOut(type) {
  const cell = koffi.alloc(type === charPtr ? "void *" : type, 1);
  const slot = {
    [SLOT_CELL]: cell,
    [SLOT_TYPE]: type,
    deref() {
      const value = koffi.decode(cell, type);
      // A string slot holds a `char *`: wrap it so `.isNull()` keeps working (koffi hands the
      // pointer back as a BigInt, which has no methods at all).
      return type === charPtr ? new NativePtr(value) : value;
    },
    writePointer(value) {
      koffi.encode(cell, type, value === undefined ? NULL : value);
    },
  };
  return slot;
}

/** True for the wrapper `allocOut` returns (used by the call boundary to unwrap it). */
function isSlot(value) {
  return value !== null && typeof value === "object" && SLOT_CELL in value;
}

// ---------------------------------------------------------------------------
// Property value kinds (mirror `rw_value_kind` in the C header)
// ---------------------------------------------------------------------------
const RW_VALUE_NULL = 0;
const RW_VALUE_BOOL = 1;
const RW_VALUE_INT = 2;
const RW_VALUE_UINT = 3;
const RW_VALUE_FLOAT = 4;
const RW_VALUE_STRING = 5;
// Colour and rectangle properties travel as their CSS-style string form
// (`#RRGGBBAA` and `x,y,w,h`) in the string slot, with a distinct kind so a
// caller can tell one from free text. Every binding must accept these and free
// the payload, or a colour read returns "no property" *and* leaks the buffer.
const RW_VALUE_COLOR = 6;
const RW_VALUE_RECT = 7;
// A composite payload (a tuple/list of values); `num` carries the component count
// and the string holds `kind:value;…`.
const RW_VALUE_TUPLE = 8;
// A record list; `num` carries the record count and the string holds
// `name=kind:value;…` per record with `|` between records.
const RW_VALUE_RECORDS = 9;
const RW_VALUE_ICON_SPEC = 10;

/**
 * Read an int64 out-parameter, which ffi-napi may expose as a Buffer.
 *
 * The float kind is transported as a bit pattern in the same int64, so this has
 * to preserve every bit; `Number()` on a Buffer would not.
 */
function readInt64(value) {
  if (Buffer.isBuffer(value)) {
    return value.readBigInt64LE(0);
  }
  if (typeof value === 'bigint') return value;
  return BigInt(value === undefined || value === null ? 0 : value);
}

/** Rebuild a float from the bit pattern the C ABI transports. */
function f64FromBits(bits) {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(BigInt.asUintN(64, BigInt(bits)));
  return buffer.readDoubleLE(0);
}

// The largest integer a JS Number represents exactly, as a BigInt (kept for `f64FromBits` callers
// and for the doc below).
const MAX_SAFE = BigInt(Number.MAX_SAFE_INTEGER);

/**
 * The value to hand to the ABI's `int64` argument slot.
 *
 * # Why this is now the identity
 *
 * With ffi-napi this had to translate a BigInt into a Number or a decimal String, because `ref-napi`
 * rejected a BigInt outright:
 *
 * ```text
 * $ node -e "require('ref-napi').types.int64.set(Buffer.alloc(8), 0, 0n)"
 * writeInt64: Number/String 64-bit value required
 * ```
 *
 * koffi accepts a BigInt for a 64-bit slot and keeps all 64 bits (measured: `6000285942072475648n`
 * reaches the ABI intact), while it **rejects** a decimal String for a numeric slot
 * (`TypeError: Unexpected String value, expected number`). So the conversion is not only unnecessary
 * but wrong for this layer, and it is kept as the single named place where the decision lives.
 */
function int64Argument(value) {
  return value;
}

/**
 * The loaded library, with output slots unwrapped at the call boundary.
 *
 * Two things happen here and nowhere else:
 *
 *   1. **Slots are unwrapped.** `allocOut` returns a wrapper carrying the koffi cell plus the
 *      ref-napi-shaped accessors the call sites use; the ABI has to receive the raw cell. Doing it
 *      here keeps the ~14 out-parameter call sites unchanged.
 *   2. **BigInt arguments pass through.** koffi accepts a BigInt for any integer parameter width
 *      (measured: 16n into `uint`, and a full 64-bit id into `uint64`), so no conversion is applied
 *      — the old layer had to convert because `ref-napi` rejected BigInt, and koffi rejects the
 *      String that conversion produced.
 */
function withWireArguments(lib) {
  return new Proxy(lib, {
    get(target, property) {
      // Read straight off the target: passing the proxy as the receiver would run a native
      // accessor with the wrong `this`, which the addon rejects.
      const value = target[property];
      if (typeof value !== "function") return value;
      return function (...args) {
        return value.apply(
          target,
          args.map((arg) => (isSlot(arg) ? arg[SLOT_CELL] : arg)),
        );
      };
    },
  });
}

/**
 * Map a JS value onto the `(kind, num, text)` triple the ABI expects.
 *
 * `null`/`undefined` mean "clear an optional property" rather than "error",
 * which is what the `RW_VALUE_NULL` kind exists for.
 */
function encodePropertyValue(value) {
  if (value === null || value === undefined) {
    return [RW_VALUE_NULL, int64Argument(0n), NULL];
  }
  if (typeof value === 'boolean') {
    return [RW_VALUE_BOOL, int64Argument(value ? 1n : 0n), NULL];
  }
  if (typeof value === 'number') {
    if (!Number.isInteger(value)) {
      const buffer = Buffer.alloc(8);
      buffer.writeDoubleLE(value);
      return [RW_VALUE_FLOAT, int64Argument(buffer.readBigInt64LE(0)), NULL];
    }
    if (value < 0) return [RW_VALUE_INT, int64Argument(BigInt(value)), NULL];
    return [RW_VALUE_UINT, int64Argument(BigInt(value)), NULL];
  }
  if (typeof value === 'string') {
    return [RW_VALUE_STRING, int64Argument(0n), value];
  }
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    let wire;
    if (
      value.kind === 'builtin' &&
      Object.keys(value).length === 2 &&
      typeof value.name === 'string' &&
      value.name.length > 0
    ) {
      wire = `builtin|${value.name}`;
    } else if (
      value.kind === 'resource' &&
      Object.keys(value).length === 3 &&
      typeof value.id === 'string' &&
      value.id.trim().length > 0 &&
      (value.color_mode === 'monochrome' || value.color_mode === 'original')
    ) {
      const escapedId = value.id.replace(/%/g, '%25').replace(/\|/g, '%7C');
      wire = `resource|${value.color_mode}|${escapedId}`;
    } else {
      throw new TypeError('invalid icon descriptor fields or kind');
    }
    return [RW_VALUE_ICON_SPEC, int64Argument(0n), wire];
  }
  throw new TypeError(`unsupported property value type: ${typeof value}`);
}

function decodeIconSpec(wire) {
  const separator = wire.indexOf('|');
  if (separator < 0) return null;
  const kind = wire.slice(0, separator);
  const value = wire.slice(separator + 1);
  if (kind === 'builtin' && value.length > 0) {
    return {kind: 'builtin', name: value};
  }
  if (kind !== 'resource') return null;
  const modeSeparator = value.indexOf('|');
  if (modeSeparator < 0) return null;
  const color_mode = value.slice(0, modeSeparator);
  const escapedId = value.slice(modeSeparator + 1);
  if (
    (color_mode !== 'monochrome' && color_mode !== 'original') ||
    escapedId.length === 0
  ) {
    return null;
  }
  const id = escapedId.replace(/%7C/g, '|').replace(/%25/g, '%');
  if (id.trim().length === 0) return null;
  return {kind: 'resource', id, color_mode};
}

/**
 * Calls a `(out, cap) -> required` enumerator and splits the result.
 *
 * The ABI always reports the full byte length, so one size query followed by one
 * read suffices; a list that grew in between is simply truncated, and the next
 * call corrects it.
 */
function readNameList(lib, symbol, custom) {
  const required = custom
    ? custom(lib, NULL, 0)
    : lib[symbol](NULL, 0);
  if (!required) return [];
  const buffer = Buffer.alloc(required + 1);
  if (custom) {
    custom(lib, buffer, required + 1);
  } else {
    lib[symbol](buffer, required + 1);
  }
  // The ABI writes a NUL terminator after `required` bytes of payload. Decoding the
  // whole `required + 1` buffer folded that terminator into the last item
  // (`["light", "dark\u0000"]`), so the final name never matched a plain string
  // comparison (D08-B-02). Decode exactly the payload length instead.
  return buffer.toString('utf8', 0, required).split(' ').filter(Boolean);
}

// ---------------------------------------------------------------------------
// Library loading with fallback paths
// ---------------------------------------------------------------------------
let _lib = null;

function getLibrary() {
  if (_lib) return _lib;

  // The cdylib's extension is decided by the host, not by a literal: cargo writes
  // `librust_widgets.dylib` on macOS and `librust_widgets.so` on Linux. The list used to name only
  // the `.so` spelling, so a macOS consumer could not load a locally built library at all — and the
  // failure mode was a dlopen list of paths that were never going to exist. Both spellings are
  // offered, platform-appropriate one first, because a cross-built Linux artifact on a macOS host is
  // unusual but not something this loader needs to forbid.
  const cdylibExt = process.platform === "darwin" ? "dylib" : "so";
  const otherExt = cdylibExt === "dylib" ? "so" : "dylib";
  const envPath = process.env.RUST_WIDGETS_LIB_PATH;
  const candidates = envPath
    ? [envPath]
    : [
        `librust_widgets.${cdylibExt}`,
        "librust_widgets",
        `librust_widgets.${otherExt}`,
        "rust_widgets",
        `./target/release/librust_widgets.${cdylibExt}`,
        `./target/debug/librust_widgets.${cdylibExt}`,
        "./target/release/librust_widgets",
        "./target/debug/librust_widgets",
        `./target/release/librust_widgets.${otherExt}`,
        `./target/debug/librust_widgets.${otherExt}`,
      ];

  let lastError;
  for (const name of candidates) {
    try {
      _lib = withWireArguments(loadFunctions(name));
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
//
// The declarations stay a table of `name: [returnType, [argTypes]]`, exactly as they were: koffi
// takes them per function (`lib.func(name, ret, args)`) instead of once per library, so the table is
// walked here once and every call site keeps calling `this._lib.rw_x(...)`.
//
// `"string"` is ffi-napi's spelling for a `char *` input; koffi spells it `"str"`.
// ---------------------------------------------------------------------------
function loadFunctions(libName) {
  const koffiLib = koffi.load(libName);
  const table = {
    // ── Core ────────────────────────────────────────────────────────────
    rw_init: [void_t, []],
    rw_run: [void_t, []],
    rw_quit: [void_t, []],
    rw_pump_frame: [cbool, [uint]],

    // ── Widget create (return uint64 widget ID; 0 = error) ─────────────
    rw_create_window: [uint64, ["string", int, int, uint, uint]],
    rw_create_button: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rw_create_checkbox: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rw_create_line_edit: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rw_create_label: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rw_create_radio_button: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rw_create_slider: [uint64, [uint64, int, int, uint, uint]],
    rw_create_progress_bar: [uint64, [uint64, int, int, uint, uint]],
    rw_create_combo_box: [uint64, [uint64, int, int, uint, uint]],
    rw_create_list_box: [uint64, [uint64, int, int, uint, uint]],
    rw_create_panel: [uint64, [uint64, int, int, uint, uint]],
    rw_create_message_box: [
      uint64,
      [uint64, "string", "string", int, int, uint, uint],
    ],
    rw_create_file_dialog: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rw_create_color_dialog: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rw_create_font_dialog: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rw_create_spin_box: [uint64, [uint64, int, int, uint, uint]],
    rw_create_list_view: [uint64, [uint64, int, int, uint, uint]],
    rw_create_scroll_area: [uint64, [uint64, int, int, uint, uint]],
    rw_create_menu_bar: [uint64, [uint64, int, int, uint, uint]],
    rw_create_menu: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],
    rw_create_tool_bar: [uint64, [uint64, int, int, uint, uint]],
    rw_create_status_bar: [
      uint64,
      [uint64, "string", int, int, uint, uint],
    ],

    // ── Widget manipulation ────────────────────────────────────────────
    rw_show_widget: [void_t, [uint64]],
    rw_hide_widget: [void_t, [uint64]],
    rw_destroy_widget: [void_t, [uint64]],
    // ── Generic (name-based) creation and property access ───────────────
    rw_create_widget_of_kind: [
      uint64,
      [uint64, "string", "string", int, int, uint, uint],
    ],
    rw_widget_kind_names: [uint, [charPtr, uint]],
    rw_widget_property_names: [uint, [uint64, charPtr, uint]],
    rw_get_widget_property: [
      cbool,
      [uint64, "string", intPtr, int64Ptr, charPtrPtr],
    ],
    rw_set_widget_property: [
      cbool,
      [uint64, "string", int, int64, "string"],
    ],
    rw_set_theme: [cbool, ["string"]],
    rw_theme_names: [uint, [charPtr, uint]],
    rw_set_high_contrast: [void_t, [int]],
    rw_widget_set_scroll_position: [cbool, [uint64, int, int]],
    rw_widget_scroll_to: [cbool, [uint64, int]],
    rw_widget_list_add: [uint, [uint64, "string"]],
    rw_widget_list_clear: [cbool, [uint64]],
    rw_widget_list_count: [uint, [uint64]],
    rw_widget_list_item: [uint, [uint64, uint, charPtr, uint]],
    // `name` is `"string"`, not `charPtr`: the wrapper passes a JS string, and
    // `ffi-napi`'s `charPtr` accepts only a Buffer/pointer, so with `charPtr` every
    // call to `propertyTokens` raised `TypeError: Argument "name" must be of type
    // "pointer"` — the method was completely unusable.
    rw_widget_property_tokens: [uint, [uint64, "string", charPtr, uint]],
    rw_widget_set_style: [cbool, [uint64, "string"]],
    rw_widget_set_layout: [cbool, [uint64, "string", int, int]],
    rw_widget_layout_add: [cbool, [uint64, uint64, uint]],
    rw_widget_layout_add_spacer: [cbool, [uint64, uint]],
    rw_widget_layout_remove: [cbool, [uint64, uint64]],
    rw_widget_layout_clear: [cbool, [uint64]],
    rw_widget_layout_apply: [uint, [uint64, int, int, uint, uint]],
    rw_widget_layout_child_count: [uint, [uint64]],
    rw_set_widget_text: [void_t, [uint64, "string"]],
    rw_get_widget_text: [charPtr, [uint64]],
    rw_set_widget_enabled: [void_t, [uint64, cbool]],
    rw_is_widget_enabled: [cbool, [uint64]],
    rw_set_widget_visible: [void_t, [uint64, cbool]],
    rw_is_widget_visible: [cbool, [uint64]],
    rw_set_widget_geometry: [void_t, [uint64, int, int, uint, uint]],
    rw_get_widget_geometry: [
      cbool,
      [uint64, intPtr, intPtr, uintPtr, uintPtr],
    ],
    rw_set_widget_ime_enabled: [cbool, [uint64, cbool]],
    rw_is_widget_ime_enabled: [cbool, [uint64]],

    // ── Accessibility ──────────────────────────────────────────────────
    rw_set_widget_accessibility_name: [cbool, [uint64, "string"]],
    rw_get_widget_accessibility_name: [charPtr, [uint64]],

    // ── Menu ───────────────────────────────────────────────────────────
    rw_attach_menu_bar_to_window: [cbool, [uint64, uint64]],
    // Third param is shortcut (nullable) — use charPtr to pass NULL when no shortcut
    rw_menu_add_item: [uint64, [uint64, "string", charPtr]],
    rw_poll_menu_triggered: [uint64, []],

    // ── Events ─────────────────────────────────────────────────────────
    rw_poll_widget_triggered: [uint64, []],
    rw_poll_widget_trigger_event: [uint, [uint64Ptr]],
    rw_inject_widget_trigger_event: [cbool, [uint64, uint]],
    rw_inject_menu_trigger: [cbool, [uint64]],

    // ── Combo Box ──────────────────────────────────────────────────────
    rw_combo_box_add_item: [cbool, [uint64, "string"]],
    rw_combo_box_clear_items: [cbool, [uint64]],
    rw_combo_box_set_current_index: [cbool, [uint64, uint]],
    rw_combo_box_current_index: [int, [uint64]],
    rw_combo_box_item_count: [uint, [uint64]],
    rw_combo_box_item_text: [charPtr, [uint64, uint]],

    // ── List Box ───────────────────────────────────────────────────────
    rw_list_box_add_item: [cbool, [uint64, "string"]],
    rw_list_box_remove_item: [cbool, [uint64, uint]],
    rw_list_box_clear_items: [cbool, [uint64]],
    rw_list_box_set_current_index: [cbool, [uint64, uint]],
    rw_list_box_current_index: [int, [uint64]],
    rw_list_box_item_count: [uint, [uint64]],
    rw_list_box_item_text: [charPtr, [uint64, uint]],

    // ── Clipboard ──────────────────────────────────────────────────────
    rw_set_clipboard_text: [cbool, ["string"]],
    rw_mount_surface: [cbool, [uint64, uint64, int, int, uint, uint]],
    rw_resize_surface: [cbool, [uint64, int, int, uint, uint]],
    rw_unmount_surface: [cbool, [uint64]],
    rw_invalidate_surface: [cbool, [uint64]],
    rw_supports_surfaces: [cbool, []],
    rw_take_pending_repaint: [uint64, []],
    rw_report_window_resize: [cbool, [uint64, uint, uint]],
    // The two input-dispatch entry points. A host that owns its own event loop (an
    // embedded runtime, an engine integration) delivers a pointer or a synthetic event
    // through these; without them the only way in is a surface the library created,
    // which a host-driven loop does not have.
    //
    // `event_code` is an `unsigned int` code, not a pointer — the ABI takes an `int`
    // because an enum travels as one, and the widget layer maps it to its own event
    // type. `button` uses the same constants as the press/release codes.
    rw_dispatch_pointer_event: [cbool, [uint64, uint, int, int, uint]],
    rw_dispatch_event_to_widget: [cbool, [uint64, uint, int, int, uint]],
    // The direct input carriers: key code + modifiers, a committed text string, and
    // a wheel delta. A pointer's `(x, y, button)` cannot express these.
    rw_dispatch_key_event: [cbool, [uint64, uint, uint, cbool]],
    rw_dispatch_text_event: [cbool, [uint64, charPtr]],
    rw_dispatch_wheel_event: [cbool, [uint64, int, int, uint]],
    // `out_pixels` is a `uint8**`: the callee allocates the frame and the caller
    // releases it with `rw_free_bytes(ptr, len)`, so the raw address has to come back
    // as a pointer rather than being copied into a Buffer here.
    rw_render_surface_frame: [
        cbool,
        [uint64, uint, uint, uintPtr, uintPtr, uintPtr, uintPtr, bytePtrPtr],
    ],
    rw_harmony_bind_xcomponent: [cbool, [uint64]],
    rw_harmony_xcomponent_id: [charPtr, []],
    rw_get_clipboard_text: [charPtr, []],

    // ── Drag & Drop ────────────────────────────────────────────────────
    rw_begin_drag: [cbool, [uint64, "string", bytePtr, uint]],
    rw_poll_drop_event: [
      cbool,
      [uint64Ptr, uint64Ptr, charPtrPtr, bytePtrPtr, uintPtr],
    ],

    // ── Platform ───────────────────────────────────────────────────────
    rw_backend_name: [charPtr, []],
    rw_platform_capabilities: [uint, []],
    rw_platform_capability_contract: [uint, [uint]],
    rw_platform_dpi_scale_factor: [cfloat, []],
    rw_bindings_api_version: [uint, []],

    // ── Binding status (language-specific) ─────────────────────────────
    rw_nodejs_binding_status: [uint, []],
    rw_python_binding_status: [uint, []],
    rw_cpp_binding_status: [uint, []],
    rw_java_binding_status: [uint, []],
    rw_java_jni_skeleton_version: [uint, []],
    rw_python_reserved: [uint, []],
    rw_cpp_reserved: [uint, []],
    rw_java_reserved: [uint, []],

    // ── Render ─────────────────────────────────────────────────────────
    rw_set_render_aa_samples_per_axis: [uint, [uint]],
    rw_get_render_aa_samples_per_axis: [uint, []],

    // ── Embedded engine ────────────────────────────────────────────────
    rw_set_embedded_target_fps: [uint, [uint]],
    rw_get_embedded_target_fps: [uint, []],
    rw_submit_embedded_noop_task: [uint64, ["string"]],
    rw_embedded_engine_is_initialized: [cbool, []],
    rw_embedded_engine_is_running: [cbool, []],
    rw_embedded_engine_frame_count: [uint64, []],
    rw_embedded_engine_pending_task_count: [uint64, []],
    rw_embedded_engine_window_count: [uint64, []],
    rw_embedded_engine_button_count: [uint64, []],

    // ── Memory ─────────────────────────────────────────────────────────
    rw_free_string: [void_t, [charPtr]],
    // Frees a `uint8_t*` plus length pair handed out by `rw_poll_drop_event`.
    // Distinct from `rw_free_string` on purpose: the payload is a byte allocation,
    // so freeing it as a string passes a mismatched length to the deallocator.
    rw_free_bytes: [void_t, [bytePtr, uint]],
    rw_free_rust_string: [void_t, [charPtr]],

    // ── Mobile ─────────────────────────────────────────────────────────
    rw_mobile_backend_name: [charPtr, []],
    rw_mobile_attach_native_view: [cbool, [uint64]],

    // ── Harmony ────────────────────────────────────────────────────────
    rw_harmony_on_menu_item: [cbool, [uint64]],
    rw_harmony_on_click: [cbool, [uint64]],
    rw_harmony_on_value_changed: [cbool, [uint64]],
    rw_harmony_on_widget_event: [cbool, [uint64, uint]],
    rw_harmony_bind_node: [cbool, [uint64, uint64]],
    rw_harmony_unbind_node: [cbool, [uint64]],
    rw_harmony_lookup_widget_id: [uint64, [uint64]],
    rw_harmony_clear_node_bindings: [void_t, []],
    rw_harmony_on_node_menu_item: [cbool, [uint64]],
    rw_harmony_on_node_click: [cbool, [uint64]],
    rw_harmony_on_node_value_changed: [cbool, [uint64]],
    rw_harmony_on_node_widget_event: [cbool, [uint64, uint]],

    // ── Error helpers (from rw_errors.h) ─────────────────────
    rw_error_message: [charPtr, [uint64]],
    rw_error_code: [int32, [uint64]],
  };

  const lib = {};
  for (const name of Object.keys(table)) {
    const [returns, args] = table[name];
    lib[name] = koffiLib.func(
      name,
      returns,
      args.map((arg) => (arg === "string" ? "str" : arg)),
    );
  }
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
    this._getWidgetText = wrapStringFn(lib, "rw_get_widget_text");
    this._getWidgetAccessibilityName = wrapStringFn(
      lib,
      "rw_get_widget_accessibility_name",
    );
    this._comboBoxItemText = wrapStringFn(
      lib,
      "rw_combo_box_item_text",
    );
    this._listBoxItemText = wrapStringFn(
      lib,
      "rw_list_box_item_text",
    );
    this._getClipboardText = wrapStringFn(
      lib,
      "rw_get_clipboard_text",
    );
    this._backendName = wrapStringFn(lib, "rw_backend_name");
    this._errorMessage = wrapStringFn(lib, "rw_error_message");
    this._mobileBackendName = wrapStringFn(
      lib,
      "rw_mobile_backend_name",
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
    this._lib.rw_init();
  }

  /** Enter the main event loop (blocking — call on dedicated thread). */
  run() {
    this._lib.rw_run();
  }

  /** Signal the event loop to quit. */
  quit() {
    this._lib.rw_quit();
  }

  /**
   * Drive one frame of the library without entering the blocking loop.
   *
   * `run()` hands the calling thread to the native loop and does not return until
   * the loop exits, so it cannot be combined with a polling loop in the same thread.
   * A host that must keep its own main thread calls this once per frame instead and
   * then polls the trigger queues (D09-PY-04).
   *
   * It advances the library frame (drains the trigger queue, advances animations,
   * applies pending translations) but does **not** pump a toolkit's native message
   * queue; a backend whose input comes from such a queue still needs `run()`.
   *
   * @param {number} [deltaMs=16] elapsed milliseconds since the previous frame.
   * @returns {boolean} whether another frame is owed (an animation is still settling).
   */
  pumpFrame(deltaMs = 16) {
    if (!Number.isInteger(deltaMs) || deltaMs < 0) {
      throw new RangeError("deltaMs must be a non-negative integer");
    }
    return this._lib.rw_pump_frame(deltaMs);
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
      this._lib.rw_create_window(title, x, y, width, height),
    );
  }

  createButton(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_button(parent, text, x, y, width, height),
    );
  }

  createCheckbox(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_checkbox(parent, text, x, y, width, height),
    );
  }

  createLineEdit(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_line_edit(
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
      this._lib.rw_create_label(parent, text, x, y, width, height),
    );
  }

  createRadioButton(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_radio_button(
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
      this._lib.rw_create_slider(parent, x, y, width, height),
    );
  }

  createProgressBar(parent, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_progress_bar(parent, x, y, width, height),
    );
  }

  createComboBox(parent, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_combo_box(parent, x, y, width, height),
    );
  }

  createListBox(parent, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_list_box(parent, x, y, width, height),
    );
  }

  createPanel(parent, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_panel(parent, x, y, width, height),
    );
  }

  createMessageBox(parent, title, text, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_message_box(
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
      this._lib.rw_create_file_dialog(
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
      this._lib.rw_create_color_dialog(
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
      this._lib.rw_create_font_dialog(
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
      this._lib.rw_create_spin_box(parent, x, y, width, height),
    );
  }

  createListView(parent, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_list_view(parent, x, y, width, height),
    );
  }

  createScrollArea(parent, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_scroll_area(parent, x, y, width, height),
    );
  }

  createMenuBar(parent, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_menu_bar(parent, x, y, width, height),
    );
  }

  createMenu(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_menu(parent, text, x, y, width, height),
    );
  }

  createToolBar(parent, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_tool_bar(parent, x, y, width, height),
    );
  }

  createStatusBar(parent, text, x, y, width, height) {
    return readUint64(
      this._lib.rw_create_status_bar(
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
    this._lib.rw_show_widget(widgetId);
  }

  hideWidget(widgetId) {
    this._lib.rw_hide_widget(widgetId);
  }

  destroyWidget(widgetId) {
    this._lib.rw_destroy_widget(widgetId);
  }

  setWidgetText(widgetId, text) {
    this._lib.rw_set_widget_text(widgetId, text);
  }

  getWidgetText(widgetId) {
    return this._getWidgetText(widgetId);
  }

  setWidgetEnabled(widgetId, enabled) {
    this._lib.rw_set_widget_enabled(widgetId, enabled);
  }

  isWidgetEnabled(widgetId) {
    return this._lib.rw_is_widget_enabled(widgetId);
  }

  setWidgetVisible(widgetId, visible) {
    this._lib.rw_set_widget_visible(widgetId, visible);
  }

  isWidgetVisible(widgetId) {
    return this._lib.rw_is_widget_visible(widgetId);
  }

  setWidgetGeometry(widgetId, x, y, width, height) {
    this._lib.rw_set_widget_geometry(widgetId, x, y, width, height);
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
    const ok = this._lib.rw_get_widget_geometry(
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
    return this._lib.rw_set_widget_ime_enabled(widgetId, enabled);
  }

  // ── Generic (name-based) creation and property access ─────────────

  /**
   * Create a control of any registered kind by name.
   *
   * `kindName` is a canonical factory name or alias (`"button"`,
   * `"tree_view"`, `"command_palette"`, ...); see `widgetKindNames()`.
   * Returns the new widget id, or `0n` when the name matches no control.
   *
   * Ids are BigInts: this library's `ObjectId`s exceed `Number.MAX_SAFE_INTEGER`, so a `Number`
   * cannot tell two of them apart (see `readUint64`). Treat an id as opaque and pass it back
   * unchanged.
   */
  createWidgetOfKind(
    parent,
    kindName,
    text = "",
    x = 0,
    y = 0,
    width = 100,
    height = 30,
  ) {
    return readUint64(
      this._lib.rw_create_widget_of_kind(
        parent,
        kindName,
        text,
        x,
        y,
        width,
        height,
      ),
    );
  }

  /** Every control name the library can create. */
  widgetKindNames() {
    return readNameList(this._lib, "rw_widget_kind_names", null);
  }

  /** The property names `widgetId` publishes. */
  widgetPropertyNames(widgetId) {
    return readNameList(this._lib, "rw_widget_property_names", (lib, out, cap) =>
      lib.rw_widget_property_names(widgetId, out, cap),
    );
  }

  /**
   * Read a property by name.
   *
   * Returns a JS boolean / number / string matching the property's declared
   * type, an IconSpec object for icon sources, `null` for a null value, and
   * `undefined` when the widget or property is unknown.
   */
  getWidgetProperty(widgetId, name) {
    const kindOut = allocOut(int);
    const numOut = allocOut(int64);
    const strOut = allocOut(charPtr);
    const ok = this._lib.rw_get_widget_property(
      widgetId,
      name,
      kindOut,
      numOut,
      strOut,
    );
    if (!ok) return undefined;

    const kind = kindOut.deref();
    const num = readInt64(numOut.deref());
    if (kind === RW_VALUE_NULL) return null;
    // `num` is a BigInt, so it must be compared against the BigInt literal `0n`.
    // `num !== 0` compared a BigInt with a Number, which is always true (the `!==`
    // operator never coerces), so every ABI BOOL decoded as `true` regardless of the
    // payload — a `false` property (wire `0`) read back as `true` (D08-B-01).
    if (kind === RW_VALUE_BOOL) return num !== 0n;
    if (kind === RW_VALUE_INT || kind === RW_VALUE_UINT) return num;
    if (kind === RW_VALUE_FLOAT) return f64FromBits(num);
    if (
      kind === RW_VALUE_STRING ||
      kind === RW_VALUE_COLOR ||
      kind === RW_VALUE_RECT ||
      kind === RW_VALUE_TUPLE ||
      kind === RW_VALUE_RECORDS ||
      kind === RW_VALUE_ICON_SPEC
    ) {
      // Structured string-slot variants carry their payload here. Freeing on each
      // them is the point: `rw_get_widget_property` allocates for each, so a branch
      // that returned without freeing leaked one buffer per call. A tuple's string
      // is the `kind:value;…` wire form, returned verbatim so a caller can decode
      // it with the component kinds. `readAndFreeSlot` also clears the slot: the
      // dangling pointer it would otherwise leave behind aborts the next ffi call
      // (see that helper).
      const text = readAndFreeSlot(this._lib, strOut);
      return kind === RW_VALUE_ICON_SPEC ? decodeIconSpec(text) : text;
    }
    // An unknown kind is a newer ABI than this binding knows about. Free the payload
    // if there is one, so an unrecognised kind cannot leak either.
    if (!strOut.deref().isNull()) {
      readAndFreeSlot(this._lib, strOut);
    }
    return undefined;
  }

  /**
   * Write a property by name.
   *
   * `value` may be `null`, a boolean, a number, a string or an IconSpec object;
   * the matching `rw_value_kind` is chosen from its JS type. Returns false
   * when the widget or property is unknown, the property is read-only, or the
   * type differs.
   */
  setWidgetProperty(widgetId, name, value) {
    const [kind, num, text] = encodePropertyValue(value);
    return this._lib.rw_set_widget_property(
      widgetId,
      name,
      kind,
      num,
      text,
    );
  }

  /** Select the active theme by name; false when it is not registered. */
  setTheme(name) {
    return this._lib.rw_set_theme(name);
  }

  /** Names of the registered themes. */
  themeNames() {
    return readNameList(this._lib, "rw_theme_names", null);
  }

  /** Enable or disable the high-contrast override for every control. */
  setHighContrast(enabled) {
    this._lib.rw_set_high_contrast(enabled ? 1 : 0);
  }

  /**
   * Scroll a control to `(x, y)`, clamped to its content extent.
   * Returns false when the control does not scroll.
   */
  setScrollPosition(widgetId, x, y) {
    return this._lib.rw_widget_set_scroll_position(widgetId, x, y);
  }

  /**
   * Scroll a control to an edge: `'top'`, `'bottom'`, `'left'` or `'right'`.
   * An unrecognised name throws rather than silently defaulting to an edge.
   */
  scrollTo(widgetId, where) {
    const codes = {top: 0, bottom: 1, left: 2, right: 3};
    if (!(where in codes)) {
      throw new RangeError(
        `unknown scroll destination ${where}; expected one of ${Object.keys(codes)}`,
      );
    }
    return this._lib.rw_widget_scroll_to(widgetId, codes[where]);
  }

  /** Append an item to a list-like control; returns the new count (0 if refused). */
  listAdd(widgetId, item) {
    return this._lib.rw_widget_list_add(widgetId, item);
  }

  /** Remove every item from a list-like control; false if it holds no items. */
  listClear(widgetId) {
    return this._lib.rw_widget_list_clear(widgetId);
  }

  /** How many items a list-like control holds. */
  listCount(widgetId) {
    return this._lib.rw_widget_list_count(widgetId);
  }

  /**
   * The text of item `index` in a list-like control.
   *
   * Returns `''` when the control holds no items or `index` is past the last one;
   * compare against {@link listCount} to tell those apart. Without this, items were
   * write-only from Node: `listAdd` and `listCount` let you build a list but never
   * read back what you put in.
   */
  listItem(widgetId, index) {
    // Two-call convention: a null buffer reports the byte length without writing.
    const required = this._lib.rw_widget_list_item(widgetId, index, null, 0);
    if (required === 0) {
      return '';
    }
    const buffer = Buffer.alloc(required + 1);
    this._lib.rw_widget_list_item(widgetId, index, buffer, required + 1);
    return buffer.toString('utf8', 0, required);
  }

  /**
   * The accepted spellings for an enum property, or `[]`.
   *
   * An empty array means the property declares no fixed set of values, which covers
   * both a non-enum property and a name the control does not publish. The list comes
   * from the control's own schema, so it cannot drift from what the property accepts.
   */
  propertyTokens(widgetId, name) {
    return readNameList(this._lib, 'rw_widget_property_tokens', (lib, out, cap) =>
      lib.rw_widget_property_tokens(widgetId, name, out, cap),
    );
  }

  /**
   * Apply one style declaration written as `'property: value'`.
   *
   * Same property names and value syntax as a stylesheet, so
   * `'background-color: #FF0000'` works here as it would in CSS. Returns false when
   * the widget is unknown, the declaration is malformed, or the property is not one
   * the style layer knows — a misspelled property is refused, not ignored.
   */
  setWidgetStyle(widgetId, declaration) {
    return this._lib.rw_widget_set_style(widgetId, declaration);
  }

  /**
   * Create a layout for `parent`: 'hbox'/'vbox'/'grid'/'stack'/… Returns false for
   * an unknown kind, so a misspelling fails rather than arranging differently.
   */
  setWidgetLayout(parent, kind, spacing = 0, margin = 0) {
    return this._lib.rw_widget_set_layout(parent, kind, spacing, margin);
  }

  /** Add `child` to `parent`'s layout. `stretch` below 1 is raised to 1. */
  layoutAdd(parent, child, stretch = 1) {
    return this._lib.rw_widget_layout_add(parent, child, Math.max(1, stretch));
  }

  /** Add a stretchable gap — no placeholder widget needed. */
  layoutAddSpacer(parent, stretch = 1) {
    return this._lib.rw_widget_layout_add_spacer(parent, Math.max(1, stretch));
  }

  /** Stop laying `child` out inside `parent`. */
  layoutRemove(parent, child) {
    return this._lib.rw_widget_layout_remove(parent, child);
  }

  /** Discard `parent`'s layout; false when there was none. */
  layoutClear(parent) {
    return this._lib.rw_widget_layout_clear(parent);
  }

  /**
   * Recompute the layout inside the rectangle and move its children.
   * Returns how many children were positioned.
   */
  layoutApply(parent, x, y, width, height) {
    return this._lib.rw_widget_layout_apply(parent, x, y, width, height);
  }

  /** How many children `parent`'s layout holds, without applying it. */
  layoutChildCount(parent) {
    return this._lib.rw_widget_layout_child_count(parent);
  }

  isWidgetImeEnabled(widgetId) {
    return this._lib.rw_is_widget_ime_enabled(widgetId);
  }

  // ── Accessibility ─────────────────────────────────────────────────

  setWidgetAccessibilityName(widgetId, name) {
    return this._lib.rw_set_widget_accessibility_name(widgetId, name);
  }

  getWidgetAccessibilityName(widgetId) {
    return this._getWidgetAccessibilityName(widgetId);
  }

  // ── Menu ──────────────────────────────────────────────────────────

  attachMenuBarToWindow(windowId, menuBarId) {
    return this._lib.rw_attach_menu_bar_to_window(
      windowId,
      menuBarId,
    );
  }

  menuAddItem(parentMenu, text, shortcut) {
    // shortcut may be null/undefined — pass NULL pointer so Rust sees None
    const scPtr = shortcut == null ? NULL : shortcut;
    return readUint64(
      this._lib.rw_menu_add_item(parentMenu, text, scPtr),
    );
  }

  pollMenuTriggered() {
    return readUint64(this._lib.rw_poll_menu_triggered());
  }

  // ── Events ────────────────────────────────────────────────────────

  pollWidgetTriggered() {
    return readUint64(this._lib.rw_poll_widget_triggered());
  }

  /**
   * Poll the next widget trigger event with typed kind code.
   * @returns {TriggerEvent|null} event object, or null if no pending event
   */
  pollWidgetTriggerEvent() {
    const idOut = allocOut(uint64);
    const kind = this._lib.rw_poll_widget_trigger_event(idOut);
    if (kind === 0) return null;
    return new TriggerEvent(readUint64(idOut.deref()), kind);
  }

  injectWidgetTriggerEvent(widgetId, kindCode) {
    return this._lib.rw_inject_widget_trigger_event(
      widgetId,
      kindCode,
    );
  }

  injectMenuTrigger(menuItemId) {
    return this._lib.rw_inject_menu_trigger(menuItemId);
  }

  // ── Combo Box ─────────────────────────────────────────────────────

  comboBoxAddItem(comboBoxId, text) {
    return this._lib.rw_combo_box_add_item(comboBoxId, text);
  }

  comboBoxClearItems(comboBoxId) {
    return this._lib.rw_combo_box_clear_items(comboBoxId);
  }

  comboBoxSetCurrentIndex(comboBoxId, index) {
    return this._lib.rw_combo_box_set_current_index(
      comboBoxId,
      index,
    );
  }

  comboBoxCurrentIndex(comboBoxId) {
    return this._lib.rw_combo_box_current_index(comboBoxId);
  }

  comboBoxItemCount(comboBoxId) {
    return this._lib.rw_combo_box_item_count(comboBoxId);
  }

  comboBoxItemText(comboBoxId, index) {
    return this._comboBoxItemText(comboBoxId, index);
  }

  // ── List Box ──────────────────────────────────────────────────────

  listBoxAddItem(listBoxId, text) {
    return this._lib.rw_list_box_add_item(listBoxId, text);
  }

  listBoxRemoveItem(listBoxId, index) {
    return this._lib.rw_list_box_remove_item(listBoxId, index);
  }

  listBoxClearItems(listBoxId) {
    return this._lib.rw_list_box_clear_items(listBoxId);
  }

  listBoxSetCurrentIndex(listBoxId, index) {
    return this._lib.rw_list_box_set_current_index(listBoxId, index);
  }

  listBoxCurrentIndex(listBoxId) {
    return this._lib.rw_list_box_current_index(listBoxId);
  }

  listBoxItemCount(listBoxId) {
    return this._lib.rw_list_box_item_count(listBoxId);
  }

  listBoxItemText(listBoxId, index) {
    return this._listBoxItemText(listBoxId, index);
  }

  // ── Clipboard ────────────────────────────────────────────────────

  setClipboardText(text) {
    return this._lib.rw_set_clipboard_text(text);
  }

  getClipboardText() {
    return this._getClipboardText();
  }

  // ── Drag & Drop ───────────────────────────────────────────────────

  beginDrag(sourceId, mimeType, payload) {
    const buf = Buffer.isBuffer(payload) ? payload : Buffer.from(payload);
    return this._lib.rw_begin_drag(
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

    const ok = this._lib.rw_poll_drop_event(
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
      // Clears the slot as it frees; a live pointer to freed memory aborts the next ffi call.
      mime = readAndFreeSlot(this._lib, mimeOut);
    }

    let payload = null;
    const payPtr = payOut.deref();
    const payLen = lenOut.deref();
    if (payPtr && !payPtr.isNull() && payLen > 0) {
      // koffi decodes bytes to a plain array, so the wrapper turns them into the Buffer callers
      // receive (the ref-napi pointer this replaced had a `.copy` that did the same).
      payload = payPtr.bytes(payLen);
      // Free the Rust-allocated payload with its own deallocator: the buffer is a
      // byte allocation, not a NUL-terminated string, so `rw_free_string` would
      // pass a length that does not match the allocation.
      this._lib.rw_free_bytes(payPtr.raw, payLen);
    }

    return new DropEvent(sourceId, targetId, mime, payload);
  }

  // ── Platform ──────────────────────────────────────────────────────

  backendName() {
    return this._backendName();
  }

  platformCapabilities() {
    return this._lib.rw_platform_capabilities();
  }

  platformCapabilityContract(profileCode) {
    return this._lib.rw_platform_capability_contract(profileCode);
  }

  platformDpiScaleFactor() {
    return this._lib.rw_platform_dpi_scale_factor();
  }

  bindingsApiVersion() {
    return this._lib.rw_bindings_api_version();
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
    // Unlike `platformCapabilities` / `hasPlatformCapability`, this asks the ABI
    // for its Node binding status.
    //
    // There used to be a `catch` that synthesized `(1 << 0) | (1 << 1)` — i.e. it
    // reported "C ABI available, Node binding available" for a symbol whose *call
    // had just failed*. A capability probe that answers "yes" when it cannot
    // answer at all is worse than one that throws, because the caller has no way
    // to tell the two apart. Let the error surface.
    return this._lib.rw_nodejs_binding_status();
  }

  pythonBindingStatus() {
    return this._lib.rw_python_binding_status();
  }

  cppBindingStatus() {
    return this._lib.rw_cpp_binding_status();
  }

  javaBindingStatus() {
    return this._lib.rw_java_binding_status();
  }

  javaJniSkeletonVersion() {
    return this._lib.rw_java_jni_skeleton_version();
  }

  // ── Render ────────────────────────────────────────────────────────

  setRenderAaSamplesPerAxis(samples) {
    return this._lib.rw_set_render_aa_samples_per_axis(samples);
  }

  getRenderAaSamplesPerAxis() {
    return this._lib.rw_get_render_aa_samples_per_axis();
  }

  // ── Embedded engine ───────────────────────────────────────────────

  setEmbeddedTargetFps(fps) {
    return this._lib.rw_set_embedded_target_fps(fps);
  }

  getEmbeddedTargetFps() {
    return this._lib.rw_get_embedded_target_fps();
  }

  submitEmbeddedNoopTask(label) {
    return readUint64(this._lib.rw_submit_embedded_noop_task(label));
  }

  embeddedEngineIsInitialized() {
    return this._lib.rw_embedded_engine_is_initialized();
  }

  embeddedEngineIsRunning() {
    return this._lib.rw_embedded_engine_is_running();
  }

  embeddedEngineFrameCount() {
    return readUint64(this._lib.rw_embedded_engine_frame_count());
  }

  embeddedEnginePendingTaskCount() {
    return readUint64(
      this._lib.rw_embedded_engine_pending_task_count(),
    );
  }

  embeddedEngineWindowCount() {
    return readUint64(this._lib.rw_embedded_engine_window_count());
  }

  embeddedEngineButtonCount() {
    return readUint64(this._lib.rw_embedded_engine_button_count());
  }

  // ── Mobile ────────────────────────────────────────────────────────

  mobileBackendName() {
    return this._mobileBackendName();
  }

  mobileAttachNativeView(nativeHandle) {
    return this._lib.rw_mobile_attach_native_view(nativeHandle);
  }

  // ── Harmony ───────────────────────────────────────────────────────

  harmonyOnMenuItem(menuItemId) {
    return this._lib.rw_harmony_on_menu_item(menuItemId);
  }

  harmonyOnClick(widgetId) {
    return this._lib.rw_harmony_on_click(widgetId);
  }

  harmonyOnValueChanged(widgetId) {
    return this._lib.rw_harmony_on_value_changed(widgetId);
  }

  harmonyOnWidgetEvent(widgetId, kindCode) {
    return this._lib.rw_harmony_on_widget_event(widgetId, kindCode);
  }

  harmonyBindNode(nodeHandle, widgetId) {
    return this._lib.rw_harmony_bind_node(nodeHandle, widgetId);
  }

  harmonyUnbindNode(nodeHandle) {
    return this._lib.rw_harmony_unbind_node(nodeHandle);
  }

  harmonyLookupWidgetId(nodeHandle) {
    return readUint64(
      this._lib.rw_harmony_lookup_widget_id(nodeHandle),
    );
  }

  harmonyClearNodeBindings() {
    this._lib.rw_harmony_clear_node_bindings();
  }

  harmonyOnNodeMenuItem(nodeHandle) {
    return this._lib.rw_harmony_on_node_menu_item(nodeHandle);
  }

  harmonyOnNodeClick(nodeHandle) {
    return this._lib.rw_harmony_on_node_click(nodeHandle);
  }

  harmonyOnNodeValueChanged(nodeHandle) {
    return this._lib.rw_harmony_on_node_value_changed(nodeHandle);
  }

  harmonyOnNodeWidgetEvent(nodeHandle, kindCode) {
    return this._lib.rw_harmony_on_node_widget_event(
      nodeHandle,
      kindCode,
    );
  }

  // ── Error helpers ─────────────────────────────────────────────────

  errorMessage(handle) {
    return this._errorMessage(handle);
  }

  errorCode(handle) {
    return this._lib.rw_error_code(handle);
  }

  // ── Low-level access ──────────────────────────────────────────────

  /**
   * Get the raw ffi-napi Library object (for advanced use).
   *
   * The object is wrapped so a BigInt argument is converted to the Number or decimal String
   * ffi-napi accepts; every function is otherwise the loaded one, callable directly.
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
  RW_VALUE_NULL,
  RW_VALUE_BOOL,
  RW_VALUE_INT,
  RW_VALUE_UINT,
  RW_VALUE_FLOAT,
  RW_VALUE_STRING,
  RW_VALUE_ICON_SPEC,
};
