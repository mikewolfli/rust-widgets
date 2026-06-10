"""Error codes for the rust-widgets C ABI.

These match the `rw_errors.h` C header exactly.
"""

# ------------------------------------------------------------------ #
# General                                                             #
# ------------------------------------------------------------------ #
RW_ERROR_SUCCESS = 0
RW_ERROR_NOT_IMPLEMENTED = 1
RW_ERROR_UNSUPPORTED_OPERATION = 2
RW_ERROR_INVALID_ARGUMENT = 3
RW_ERROR_NULL_POINTER = 4
RW_ERROR_OUT_OF_MEMORY = 5
RW_ERROR_LOCK_POISONED = 6

# ------------------------------------------------------------------ #
# Widget                                                              #
# ------------------------------------------------------------------ #
RW_ERROR_WIDGET_BASE_NOT_IMPL = 100
RW_ERROR_WIDGET_NOT_FOUND = 101
RW_ERROR_WIDGET_INVALID_STATE = 102
RW_ERROR_WIDGET_DEPRECATED = 103

# ------------------------------------------------------------------ #
# Platform                                                            #
# ------------------------------------------------------------------ #
RW_ERROR_PLATFORM_UNSUPPORTED = 200
RW_ERROR_PLATFORM_INIT_FAILED = 201
RW_ERROR_CLIPBOARD_FAILED = 202
RW_ERROR_DRAG_DROP_FAILED = 203

# ------------------------------------------------------------------ #
# Render                                                              #
# ------------------------------------------------------------------ #
RW_ERROR_RENDER_CONTEXT_INVALID = 300
RW_ERROR_RENDER_PIPELINE_FAILED = 301

# ------------------------------------------------------------------ #
# I/O                                                                 #
# ------------------------------------------------------------------ #
RW_ERROR_I18N_LOAD_FAILED = 400
RW_ERROR_FILE_NOT_FOUND = 401

# ------------------------------------------------------------------ #
# Human-readable names                                                #
# ------------------------------------------------------------------ #
ERROR_NAMES: dict[int, str] = {
    RW_ERROR_SUCCESS: "success",
    RW_ERROR_NOT_IMPLEMENTED: "not implemented",
    RW_ERROR_UNSUPPORTED_OPERATION: "unsupported operation",
    RW_ERROR_INVALID_ARGUMENT: "invalid argument",
    RW_ERROR_NULL_POINTER: "null pointer",
    RW_ERROR_OUT_OF_MEMORY: "out of memory",
    RW_ERROR_LOCK_POISONED: "lock poisoned",
    RW_ERROR_WIDGET_BASE_NOT_IMPL: "widget base not implemented",
    RW_ERROR_WIDGET_NOT_FOUND: "widget not found",
    RW_ERROR_WIDGET_INVALID_STATE: "widget invalid state",
    RW_ERROR_WIDGET_DEPRECATED: "widget deprecated",
    RW_ERROR_PLATFORM_UNSUPPORTED: "platform unsupported",
    RW_ERROR_PLATFORM_INIT_FAILED: "platform init failed",
    RW_ERROR_CLIPBOARD_FAILED: "clipboard failed",
    RW_ERROR_DRAG_DROP_FAILED: "drag & drop failed",
    RW_ERROR_RENDER_CONTEXT_INVALID: "render context invalid",
    RW_ERROR_RENDER_PIPELINE_FAILED: "render pipeline failed",
    RW_ERROR_I18N_LOAD_FAILED: "i18n load failed",
    RW_ERROR_FILE_NOT_FOUND: "file not found",
}


def error_name(code: int) -> str:
    """Return a human-readable name for an error code."""
    return ERROR_NAMES.get(code, f"unknown error ({code})")


# ------------------------------------------------------------------ #
# Widget trigger kind codes                                           #
# ------------------------------------------------------------------ #
TRIGGER_NONE = 0
TRIGGER_CLICKED = 1
TRIGGER_VALUE_CHANGED = 2
TRIGGER_SELECTION_CHANGED = 3
TRIGGER_CLOSED = 4

TRIGGER_NAMES: dict[int, str] = {
    TRIGGER_NONE: "none",
    TRIGGER_CLICKED: "clicked",
    TRIGGER_VALUE_CHANGED: "value_changed",
    TRIGGER_SELECTION_CHANGED: "selection_changed",
    TRIGGER_CLOSED: "closed",
}


def trigger_name(code: int) -> str:
    """Return a human-readable name for a trigger kind code."""
    return TRIGGER_NAMES.get(code, f"unknown trigger ({code})")


# ------------------------------------------------------------------ #
# Platform capability bitmask flags                                   #
# ------------------------------------------------------------------ #
CAP_DPI_SCALING = 1 << 0
CAP_IME = 1 << 1
CAP_ACCESSIBILITY = 1 << 2
CAP_NATIVE_MENU = 1 << 3
CAP_TYPED_WIDGET_TRIGGER = 1 << 4

CAPABILITY_NAMES: dict[int, str] = {
    CAP_DPI_SCALING: "dpi_scaling",
    CAP_IME: "ime",
    CAP_ACCESSIBILITY: "accessibility",
    CAP_NATIVE_MENU: "native_menu",
    CAP_TYPED_WIDGET_TRIGGER: "typed_widget_trigger",
}


def capabilities_description(mask: int) -> list[str]:
    """Return list of capability names from a bitmask."""
    return [name for bit, name in CAPABILITY_NAMES.items() if mask & bit]
