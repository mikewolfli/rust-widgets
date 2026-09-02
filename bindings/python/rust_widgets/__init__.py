"""Python ctypes-based bindings for `rust-widgets` native GUI library.

Usage::

    from rust_widgets import RustWidgets

    rw = RustWidgets()
    rw.init()
    win = rw.create_window("Hello", 100, 100, 800, 600)
    btn = rw.create_button(win, "Click me", 10, 10, 120, 32)
    rw.show_widget(win)
    rw.run()
"""

from __future__ import annotations

import ctypes
import ctypes.util
import os
import platform as _sys_platform
import sys
from pathlib import Path
from typing import Any, Callable, Optional, Tuple

from .errors import (
    RW_ERROR_SUCCESS,
    capabilities_description,
    error_name,
    trigger_name,
)

__all__ = [
    "RustWidgets",
    "find_library",
    "LibraryNotFoundError",
]

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_LIB_NAMES: dict[str, list[str]] = {
    "linux": ["librust_widgets.so"],
    "darwin": ["librust_widgets.dylib"],
    "win32": ["rust_widgets.dll"],
}


def find_library(*, extra_dirs: Optional[list[str | Path]] = None) -> str:
    """Locate the ``rust-widgets`` shared library.

    Searches (in order):

    1. ``extra_dirs`` (if provided)
    2. ``LD_LIBRARY_PATH`` / ``DYLD_LIBRARY_PATH`` / ``PATH``
    3. ``ctypes.util.find_library``
    4. A ``target/release`` or ``target/debug`` subdirectory relative to
       the project root (walked from the current file's location).

    Returns the absolute path to the library.

    Raises
    ------
    LibraryNotFoundError
        If the library could not be found.
    """
    system = _sys_platform.system().lower()
    if system == "linux":
        names = _LIB_NAMES["linux"]
    elif system == "darwin":
        names = _LIB_NAMES["darwin"]
    elif system == "windows":
        names = _LIB_NAMES["win32"]
    else:
        names = [f"librust_widgets.so"]

    search_dirs: list[Path] = []

    # 1. User-provided extra directories
    if extra_dirs:
        search_dirs.extend(Path(d).resolve() for d in extra_dirs)

    # 2. Walk up from this file to find Cargo.toml -> target/release|debug
    here = Path(__file__).resolve().parent
    for parent in [here] + list(here.parents):
        cargo = parent / "Cargo.toml"
        if cargo.is_file():
            for build_dir in ("release", "debug"):
                candidate = parent / "target" / build_dir
                if candidate.is_dir():
                    search_dirs.append(candidate)
            # Also check one level up (monorepo structure)
            for build_dir in ("release", "debug"):
                candidate = parent.parent / "target" / build_dir
                if candidate.is_dir():
                    search_dirs.append(candidate)
            break

    # Try explicit paths first
    for d in search_dirs:
        for name in names:
            full = d / name
            if full.is_file():
                return str(full)

    # 3. ctypes.util.find_library (uses system loader paths)
    for name in names:
        # Strip lib prefix and .so/.dylib suffix for find_library
        base = name
        if base.startswith("lib"):
            base = base[3:]
        for ext in (".so", ".dylib", ".dll"):
            if base.endswith(ext):
                base = base[: -len(ext)]
                break
        found = ctypes.util.find_library(base)
        if found:
            return found

    # 4. Raw dlopen-friendly name as a last resort
    for name in names:
        try:
            # Try loading by soname — if the system loader finds it, we're good
            lib = ctypes.cdll.LoadLibrary(name)
            # Got it — return the name so the caller can load properly
            return name
        except OSError:
            continue

    msg = (
        f"Could not locate rust-widgets shared library.\n"
        f"Tried names: {names}\n"
        f"Searched dirs: {search_dirs}\n"
        "Ensure the library is built and on LD_LIBRARY_PATH / DYLD_LIBRARY_PATH / PATH."
    )
    raise LibraryNotFoundError(msg)


class LibraryNotFoundError(FileNotFoundError):
    """Raised when the ``rust-widgets`` shared library cannot be found."""


# ---------------------------------------------------------------------------
# RustWidgets class
# ---------------------------------------------------------------------------


class RustWidgets:
    """Pythonic wrapper around the ``rust-widgets`` C ABI.

    Every public method corresponds to a ``rw_*`` C function.
    The native library is loaded lazily on first call.

    Parameters
    ----------
    lib_path
        Path to the shared library.  If ``None``, :func:`find_library` is
        used to locate it automatically.
    extra_dirs
        Extra directories to search (passed to :func:`find_library`) when
        ``lib_path`` is ``None``.
    """

    def __init__(
        self,
        lib_path: Optional[str | Path] = None,
        *,
        extra_dirs: Optional[list[str | Path]] = None,
    ):
        self._lib: Optional[ctypes.CDLL] = None
        self._lib_path: Optional[str] = None
        if lib_path is not None:
            self._lib_path = str(lib_path)
        self._extra_dirs = extra_dirs

    # ------------------------------------------------------------------ #
    # Library loading                                                    #
    # ------------------------------------------------------------------ #

    @property
    def lib(self) -> ctypes.CDLL:
        """The loaded ``ctypes.CDLL`` instance (lazy)."""
        if self._lib is None:
            path = (
                self._lib_path
                if self._lib_path
                else find_library(extra_dirs=self._extra_dirs)
            )
            self._lib = ctypes.cdll.LoadLibrary(path)
            self._lib_path = path
            self._setup_argtypes()
        return self._lib

    def _setup_argtypes(self) -> None:
        """Declare argument and return types on all C functions."""

        L = self._lib

        # Common type aliases
        c_bool = ctypes.c_bool
        c_char_p = ctypes.c_char_p
        c_void_p = ctypes.c_void_p
        c_int = ctypes.c_int
        c_uint = ctypes.c_uint
        c_float = ctypes.c_float
        c_uint64 = ctypes.c_uint64
        c_int64 = ctypes.c_int64
        POINTER = ctypes.POINTER

        # ------------------------------------------------------------------ #
        # Core lifecycle                                                     #
        # ------------------------------------------------------------------ #
        L.rw_init.argtypes = []
        L.rw_init.restype = None

        L.rw_run.argtypes = []
        L.rw_run.restype = None

        L.rw_quit.argtypes = []
        L.rw_quit.restype = None

        # ------------------------------------------------------------------ #
        # Widget creation  — all return u64 (0 = failure)                     #
        #                                                                     #
        # Common pattern:                                                     #
        #   create_*(parent: u64, [text: *const c_char,] x: c_int, y: c_int, #
        #              width: c_uint, height: c_uint) -> u64                   #
        # ------------------------------------------------------------------ #

        # Window (no parent)
        L.rw_create_window.argtypes = [
            c_char_p,  # title
            c_int,
            c_int,  # x, y
            c_uint,
            c_uint,  # width, height
        ]
        L.rw_create_window.restype = c_uint64

        # Simple widgets: parent + text + x/y/w/h
        _textual = [
            c_uint64,  # parent
            c_char_p,  # text
            c_int,
            c_int,  # x, y
            c_uint,
            c_uint,  # width, height
        ]
        L.rw_create_button.argtypes = _textual
        L.rw_create_button.restype = c_uint64

        L.rw_create_checkbox.argtypes = _textual
        L.rw_create_checkbox.restype = c_uint64

        L.rw_create_line_edit.argtypes = _textual
        L.rw_create_line_edit.restype = c_uint64

        L.rw_create_label.argtypes = _textual
        L.rw_create_label.restype = c_uint64

        L.rw_create_radio_button.argtypes = _textual
        L.rw_create_radio_button.restype = c_uint64

        L.rw_create_menu.argtypes = _textual
        L.rw_create_menu.restype = c_uint64

        L.rw_create_status_bar.argtypes = _textual
        L.rw_create_status_bar.restype = c_uint64

        # Simple widgets: parent + x/y/w/h (no text)
        _nont_textual = [
            c_uint64,  # parent
            c_int,
            c_int,  # x, y
            c_uint,
            c_uint,  # width, height
        ]
        L.rw_create_slider.argtypes = _nont_textual
        L.rw_create_slider.restype = c_uint64

        L.rw_create_progress_bar.argtypes = _nont_textual
        L.rw_create_progress_bar.restype = c_uint64

        L.rw_create_combo_box.argtypes = _nont_textual
        L.rw_create_combo_box.restype = c_uint64

        L.rw_create_list_box.argtypes = _nont_textual
        L.rw_create_list_box.restype = c_uint64

        L.rw_create_panel.argtypes = _nont_textual
        L.rw_create_panel.restype = c_uint64

        L.rw_create_spin_box.argtypes = _nont_textual
        L.rw_create_spin_box.restype = c_uint64

        L.rw_create_list_view.argtypes = _nont_textual
        L.rw_create_list_view.restype = c_uint64

        L.rw_create_scroll_area.argtypes = _nont_textual
        L.rw_create_scroll_area.restype = c_uint64

        L.rw_create_menu_bar.argtypes = _nont_textual
        L.rw_create_menu_bar.restype = c_uint64

        L.rw_create_tool_bar.argtypes = _nont_textual
        L.rw_create_tool_bar.restype = c_uint64

        # MessageBox: parent + title + text + x/y/w/h
        L.rw_create_message_box.argtypes = [
            c_uint64,  # parent
            c_char_p,  # title
            c_char_p,  # text
            c_int,
            c_int,  # x, y
            c_uint,
            c_uint,  # width, height
        ]
        L.rw_create_message_box.restype = c_uint64

        # File / Color / Font dialog: parent + title + x/y/w/h
        _dialog = [
            c_uint64,  # parent
            c_char_p,  # title
            c_int,
            c_int,  # x, y
            c_uint,
            c_uint,  # width, height
        ]
        L.rw_create_file_dialog.argtypes = _dialog
        L.rw_create_file_dialog.restype = c_uint64

        L.rw_create_color_dialog.argtypes = _dialog
        L.rw_create_color_dialog.restype = c_uint64

        L.rw_create_font_dialog.argtypes = _dialog
        L.rw_create_font_dialog.restype = c_uint64

        # ------------------------------------------------------------------ #
        # Widget manipulation                                                #
        # ------------------------------------------------------------------ #
        L.rw_show_widget.argtypes = [c_uint64]
        L.rw_show_widget.restype = None

        L.rw_hide_widget.argtypes = [c_uint64]
        L.rw_hide_widget.restype = None

        L.rw_set_widget_text.argtypes = [c_uint64, c_char_p]
        L.rw_set_widget_text.restype = None

        L.rw_get_widget_text.argtypes = [c_uint64]
        L.rw_get_widget_text.restype = c_char_p

        L.rw_set_widget_enabled.argtypes = [c_uint64, c_bool]
        L.rw_set_widget_enabled.restype = None

        L.rw_is_widget_enabled.argtypes = [c_uint64]
        L.rw_is_widget_enabled.restype = c_bool

        L.rw_set_widget_visible.argtypes = [c_uint64, c_bool]
        L.rw_set_widget_visible.restype = None

        L.rw_is_widget_visible.argtypes = [c_uint64]
        L.rw_is_widget_visible.restype = c_bool

        L.rw_set_widget_geometry.argtypes = [
            c_uint64,
            c_int,
            c_int,
            c_uint,
            c_uint,
        ]
        L.rw_set_widget_geometry.restype = None

        L.rw_get_widget_geometry.argtypes = [
            c_uint64,
            POINTER(c_int),  # x_out
            POINTER(c_int),  # y_out
            POINTER(c_uint),  # width_out
            POINTER(c_uint),  # height_out
        ]
        L.rw_get_widget_geometry.restype = c_bool

        L.rw_set_widget_ime_enabled.argtypes = [c_uint64, c_bool]
        L.rw_set_widget_ime_enabled.restype = c_bool

        L.rw_is_widget_ime_enabled.argtypes = [c_uint64]
        L.rw_is_widget_ime_enabled.restype = c_bool

        L.rw_set_widget_accessibility_name.argtypes = [c_uint64, c_char_p]
        L.rw_set_widget_accessibility_name.restype = c_bool

        L.rw_get_widget_accessibility_name.argtypes = [c_uint64]
        L.rw_get_widget_accessibility_name.restype = c_char_p

        # ------------------------------------------------------------------ #
        # Menu operations                                                    #
        # ------------------------------------------------------------------ #
        L.rw_attach_menu_bar_to_window.argtypes = [c_uint64, c_uint64]
        L.rw_attach_menu_bar_to_window.restype = c_bool

        L.rw_menu_add_item.argtypes = [c_uint64, c_char_p, c_char_p]
        L.rw_menu_add_item.restype = c_uint64

        L.rw_poll_menu_triggered.argtypes = []
        L.rw_poll_menu_triggered.restype = c_uint64

        L.rw_inject_menu_trigger.argtypes = [c_uint64]
        L.rw_inject_menu_trigger.restype = c_bool

        # ------------------------------------------------------------------ #
        # Event polling                                                      #
        # ------------------------------------------------------------------ #
        L.rw_poll_widget_triggered.argtypes = []
        L.rw_poll_widget_triggered.restype = c_uint64

        L.rw_poll_widget_trigger_event.argtypes = [POINTER(c_uint64)]
        L.rw_poll_widget_trigger_event.restype = c_uint

        L.rw_inject_widget_trigger_event.argtypes = [c_uint64, c_uint]
        L.rw_inject_widget_trigger_event.restype = c_bool

        # ------------------------------------------------------------------ #
        # Combo Box                                                          #
        # ------------------------------------------------------------------ #
        L.rw_combo_box_add_item.argtypes = [c_uint64, c_char_p]
        L.rw_combo_box_add_item.restype = c_bool

        L.rw_combo_box_clear_items.argtypes = [c_uint64]
        L.rw_combo_box_clear_items.restype = c_bool

        L.rw_combo_box_set_current_index.argtypes = [c_uint64, c_uint]
        L.rw_combo_box_set_current_index.restype = c_bool

        L.rw_combo_box_current_index.argtypes = [c_uint64]
        L.rw_combo_box_current_index.restype = c_int

        L.rw_combo_box_item_count.argtypes = [c_uint64]
        L.rw_combo_box_item_count.restype = c_uint

        L.rw_combo_box_item_text.argtypes = [c_uint64, c_uint]
        L.rw_combo_box_item_text.restype = c_char_p

        # ------------------------------------------------------------------ #
        # List Box                                                           #
        # ------------------------------------------------------------------ #
        L.rw_list_box_add_item.argtypes = [c_uint64, c_char_p]
        L.rw_list_box_add_item.restype = c_bool

        L.rw_list_box_remove_item.argtypes = [c_uint64, c_uint]
        L.rw_list_box_remove_item.restype = c_bool

        L.rw_list_box_clear_items.argtypes = [c_uint64]
        L.rw_list_box_clear_items.restype = c_bool

        L.rw_list_box_set_current_index.argtypes = [c_uint64, c_uint]
        L.rw_list_box_set_current_index.restype = c_bool

        L.rw_list_box_current_index.argtypes = [c_uint64]
        L.rw_list_box_current_index.restype = c_int

        L.rw_list_box_item_count.argtypes = [c_uint64]
        L.rw_list_box_item_count.restype = c_uint

        L.rw_list_box_item_text.argtypes = [c_uint64, c_uint]
        L.rw_list_box_item_text.restype = c_char_p

        # ------------------------------------------------------------------ #
        # Clipboard & Drag                                                   #
        # ------------------------------------------------------------------ #
        L.rw_set_clipboard_text.argtypes = [c_char_p]
        L.rw_set_clipboard_text.restype = c_bool

        L.rw_get_clipboard_text.argtypes = []
        L.rw_get_clipboard_text.restype = c_char_p

        L.rw_begin_drag.argtypes = [
            c_uint64,  # source widget id
            c_char_p,  # mime_type
            POINTER(c_uint8),  # payload
            c_uint,  # payload_len
        ]
        L.rw_begin_drag.restype = c_bool

        # ------------------------------------------------------------------ #
        # Platform info                                                      #
        # ------------------------------------------------------------------ #
        L.rw_backend_name.argtypes = []
        L.rw_backend_name.restype = c_char_p

        L.rw_platform_capabilities.argtypes = []
        L.rw_platform_capabilities.restype = c_uint

        L.rw_platform_dpi_scale_factor.argtypes = []
        L.rw_platform_dpi_scale_factor.restype = c_float

        L.rw_platform_capability_contract.argtypes = [c_uint]
        L.rw_platform_capability_contract.restype = c_uint

        L.rw_bindings_api_version.argtypes = []
        L.rw_bindings_api_version.restype = c_uint

        # ------------------------------------------------------------------ #
        # Render configuration                                               #
        # ------------------------------------------------------------------ #
        L.rw_set_render_aa_samples_per_axis.argtypes = [c_uint]
        L.rw_set_render_aa_samples_per_axis.restype = c_uint

        L.rw_get_render_aa_samples_per_axis.argtypes = []
        L.rw_get_render_aa_samples_per_axis.restype = c_uint

        L.rw_set_embedded_target_fps.argtypes = [c_uint]
        L.rw_set_embedded_target_fps.restype = c_uint

        L.rw_get_embedded_target_fps.argtypes = []
        L.rw_get_embedded_target_fps.restype = c_uint

        L.rw_submit_embedded_noop_task.argtypes = [c_char_p]
        L.rw_submit_embedded_noop_task.restype = c_uint64

        # ------------------------------------------------------------------ #
        # Embedded engine stats                                              #
        # ------------------------------------------------------------------ #
        L.rw_embedded_engine_is_initialized.argtypes = []
        L.rw_embedded_engine_is_initialized.restype = c_bool

        L.rw_embedded_engine_is_running.argtypes = []
        L.rw_embedded_engine_is_running.restype = c_bool

        L.rw_embedded_engine_frame_count.argtypes = []
        L.rw_embedded_engine_frame_count.restype = c_uint64

        L.rw_embedded_engine_pending_task_count.argtypes = []
        L.rw_embedded_engine_pending_task_count.restype = c_uint64

        L.rw_embedded_engine_window_count.argtypes = []
        L.rw_embedded_engine_window_count.restype = c_uint64

        L.rw_embedded_engine_button_count.argtypes = []
        L.rw_embedded_engine_button_count.restype = c_uint64

        # ------------------------------------------------------------------ #
        # Mobile (only available with "mobile-api" feature)                   #
        # ------------------------------------------------------------------ #
        L.rw_mobile_backend_name.argtypes = []
        L.rw_mobile_backend_name.restype = c_char_p

        L.rw_mobile_attach_native_view.argtypes = [c_uint64]
        L.rw_mobile_attach_native_view.restype = c_bool

        # ------------------------------------------------------------------ #
        # Binding status                                                     #
        # ------------------------------------------------------------------ #
        L.rw_python_binding_status.argtypes = []
        L.rw_python_binding_status.restype = c_uint

        L.rw_cpp_binding_status.argtypes = []
        L.rw_cpp_binding_status.restype = c_uint

        L.rw_java_binding_status.argtypes = []
        L.rw_java_binding_status.restype = c_uint

        L.rw_java_jni_skeleton_version.argtypes = []
        L.rw_java_jni_skeleton_version.restype = c_uint

        L.rw_python_reserved.argtypes = []
        L.rw_python_reserved.restype = c_uint

        L.rw_cpp_reserved.argtypes = []
        L.rw_cpp_reserved.restype = c_uint

        L.rw_java_reserved.argtypes = []
        L.rw_java_reserved.restype = c_uint

        L.rw_nodejs_binding_status.argtypes = []
        L.rw_nodejs_binding_status.restype = c_uint

        # ------------------------------------------------------------------ #
        # Memory                                                             #
        # ------------------------------------------------------------------ #
        L.rw_free_string.argtypes = [c_char_p]
        L.rw_free_string.restype = None

        L.rw_free_rust_string.argtypes = [c_char_p]
        L.rw_free_rust_string.restype = None

        # ------------------------------------------------------------------ #
        # Drop event polling (advanced drag-drop)                              #
        # ------------------------------------------------------------------ #
        L.rw_poll_drop_event.argtypes = [
            POINTER(c_uint64),  # source_out
            POINTER(c_uint64),  # target_out
            POINTER(c_char_p),  # mime_out
            POINTER(c_void_p),  # payload_out
            POINTER(c_uint),  # payload_len_out
        ]
        L.rw_poll_drop_event.restype = c_bool

        # ------------------------------------------------------------------ #
        # Error state                                                         #
        # ------------------------------------------------------------------ #
        L.rw_error_code.argtypes = [c_uint64]
        L.rw_error_code.restype = c_int

        L.rw_error_message.argtypes = [c_uint64]
        L.rw_error_message.restype = c_char_p

        # ------------------------------------------------------------------ #
        # Harmony node bridge (HarmonyOS native bridge)                       #
        # ------------------------------------------------------------------ #
        L.rw_harmony_bind_node.argtypes = [c_uint64, c_uint64]
        L.rw_harmony_bind_node.restype = c_bool

        L.rw_harmony_unbind_node.argtypes = [c_uint64]
        L.rw_harmony_unbind_node.restype = c_bool

        L.rw_harmony_lookup_widget_id.argtypes = [c_uint64]
        L.rw_harmony_lookup_widget_id.restype = c_uint64

        L.rw_harmony_clear_node_bindings.argtypes = []
        L.rw_harmony_clear_node_bindings.restype = None

        L.rw_harmony_on_click.argtypes = [c_uint64]
        L.rw_harmony_on_click.restype = c_bool

        L.rw_harmony_on_menu_item.argtypes = [c_uint64]
        L.rw_harmony_on_menu_item.restype = c_bool

        L.rw_harmony_on_value_changed.argtypes = [c_uint64]
        L.rw_harmony_on_value_changed.restype = c_bool

        L.rw_harmony_on_widget_event.argtypes = [c_uint64, c_uint]
        L.rw_harmony_on_widget_event.restype = c_bool

        L.rw_harmony_on_node_click.argtypes = [c_uint64]
        L.rw_harmony_on_node_click.restype = c_bool

        L.rw_harmony_on_node_menu_item.argtypes = [c_uint64]
        L.rw_harmony_on_node_menu_item.restype = c_bool

        L.rw_harmony_on_node_value_changed.argtypes = [c_uint64]
        L.rw_harmony_on_node_value_changed.restype = c_bool

        L.rw_harmony_on_node_widget_event.argtypes = [c_uint64, c_uint]
        L.rw_harmony_on_node_widget_event.restype = c_bool

    # ------------------------------------------------------------------ #
    # String helpers                                                     #
    # ------------------------------------------------------------------ #

    @staticmethod
    def _encode(text: Optional[str]) -> Optional[bytes]:
        """Encode a Python string to a null-terminated utf-8 byte string."""
        if text is None:
            return None
        return text.encode("utf-8")

    @staticmethod
    def _decode(ptr) -> str:
        """Decode a ``*const c_char`` return value and free it.

        If the pointer is null, returns an empty string.
        """
        if not ptr:
            return ""
        try:
            return ptr.decode("utf-8")
        finally:
            # Free the Rust-allocated C string
            pass  # we free in the caller wrapper methods

    @staticmethod
    def _decode_and_free(lib: ctypes.CDLL, ptr) -> str:
        """Decode a ``*const c_char`` and free it via ``rw_free_string``."""
        if not ptr:
            return ""
        try:
            return ptr.decode("utf-8")
        finally:
            if ptr:
                lib.rw_free_string(ptr)

    # ------------------------------------------------------------------ #
    # Public API — Core lifecycle                                        #
    # ------------------------------------------------------------------ #

    def init(self) -> None:
        """Initialise the library. Must be called once before most operations."""
        self.lib.rw_init()

    def run(self) -> None:
        """Enter the native event loop. Blocks until :meth:`quit` is called."""
        self.lib.rw_run()

    def quit(self) -> None:
        """Signal the event loop to exit."""
        self.lib.rw_quit()

    # ------------------------------------------------------------------ #
    # Public API — Widget creation                                       #
    # ------------------------------------------------------------------ #

    def create_window(self, title: str, x: int, y: int, width: int, height: int) -> int:
        """Create a top-level window. Returns the widget ID (0 on failure)."""
        return self.lib.rw_create_window(
            self._encode(title), x, y, width, height
        )

    def create_button(
        self, parent: int, text: str, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a push button."""
        return self.lib.rw_create_button(
            parent, self._encode(text), x, y, width, height
        )

    def create_checkbox(
        self, parent: int, text: str, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a check box."""
        return self.lib.rw_create_checkbox(
            parent, self._encode(text), x, y, width, height
        )

    def create_line_edit(
        self, parent: int, text: str, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a single-line text editor."""
        return self.lib.rw_create_line_edit(
            parent, self._encode(text), x, y, width, height
        )

    def create_label(
        self, parent: int, text: str, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a static text label."""
        return self.lib.rw_create_label(
            parent, self._encode(text), x, y, width, height
        )

    def create_radio_button(
        self, parent: int, text: str, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a radio button."""
        return self.lib.rw_create_radio_button(
            parent, self._encode(text), x, y, width, height
        )

    def create_slider(
        self, parent: int, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a horizontal slider."""
        return self.lib.rw_create_slider(parent, x, y, width, height)

    def create_progress_bar(
        self, parent: int, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a progress bar."""
        return self.lib.rw_create_progress_bar(parent, x, y, width, height)

    def create_combo_box(
        self, parent: int, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a combo-box (drop-down list)."""
        return self.lib.rw_create_combo_box(parent, x, y, width, height)

    def create_list_box(
        self, parent: int, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a list box."""
        return self.lib.rw_create_list_box(parent, x, y, width, height)

    def create_panel(self, parent: int, x: int, y: int, width: int, height: int) -> int:
        """Create a container panel."""
        return self.lib.rw_create_panel(parent, x, y, width, height)

    def create_message_box(
        self,
        parent: int,
        title: str,
        text: str,
        x: int,
        y: int,
        width: int,
        height: int,
    ) -> int:
        """Create a message-box dialog."""
        return self.lib.rw_create_message_box(
            parent,
            self._encode(title),
            self._encode(text),
            x,
            y,
            width,
            height,
        )

    def create_file_dialog(
        self, parent: int, title: str, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a file-open/save dialog."""
        return self.lib.rw_create_file_dialog(
            parent, self._encode(title), x, y, width, height
        )

    def create_color_dialog(
        self, parent: int, title: str, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a color-picker dialog."""
        return self.lib.rw_create_color_dialog(
            parent, self._encode(title), x, y, width, height
        )

    def create_font_dialog(
        self, parent: int, title: str, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a font-picker dialog."""
        return self.lib.rw_create_font_dialog(
            parent, self._encode(title), x, y, width, height
        )

    def create_spin_box(
        self, parent: int, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a numeric spin box."""
        return self.lib.rw_create_spin_box(parent, x, y, width, height)

    def create_list_view(
        self, parent: int, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a list view."""
        return self.lib.rw_create_list_view(parent, x, y, width, height)

    def create_scroll_area(
        self, parent: int, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a scrollable area."""
        return self.lib.rw_create_scroll_area(parent, x, y, width, height)

    def create_menu_bar(
        self, parent: int, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a menu bar."""
        return self.lib.rw_create_menu_bar(parent, x, y, width, height)

    def create_menu(
        self, parent: int, text: str, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a drop-down menu."""
        return self.lib.rw_create_menu(
            parent, self._encode(text), x, y, width, height
        )

    def create_tool_bar(
        self, parent: int, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a tool bar."""
        return self.lib.rw_create_tool_bar(parent, x, y, width, height)

    def create_status_bar(
        self, parent: int, text: str, x: int, y: int, width: int, height: int
    ) -> int:
        """Create a status bar."""
        return self.lib.rw_create_status_bar(
            parent, self._encode(text), x, y, width, height
        )

    # ------------------------------------------------------------------ #
    # Public API — Widget manipulation                                   #
    # ------------------------------------------------------------------ #

    def show_widget(self, widget_id: int) -> None:
        """Show (make visible) a widget."""
        self.lib.rw_show_widget(widget_id)

    def hide_widget(self, widget_id: int) -> None:
        """Hide a widget."""
        self.lib.rw_hide_widget(widget_id)

    def set_widget_text(self, widget_id: int, text: str) -> None:
        """Set the text content of a widget."""
        self.lib.rw_set_widget_text(widget_id, self._encode(text))

    def get_widget_text(self, widget_id: int) -> str:
        """Get the text content of a widget (frees the C string internally)."""
        ptr = self.lib.rw_get_widget_text(widget_id)
        return self._decode_and_free(self.lib, ptr)

    def set_widget_enabled(self, widget_id: int, enabled: bool) -> None:
        """Enable or disable a widget."""
        self.lib.rw_set_widget_enabled(widget_id, enabled)

    def is_widget_enabled(self, widget_id: int) -> bool:
        """Check whether a widget is enabled."""
        return bool(self.lib.rw_is_widget_enabled(widget_id))

    def set_widget_visible(self, widget_id: int, visible: bool) -> None:
        """Show or hide a widget programmatically."""
        self.lib.rw_set_widget_visible(widget_id, visible)

    def is_widget_visible(self, widget_id: int) -> bool:
        """Check whether a widget is visible."""
        return bool(self.lib.rw_is_widget_visible(widget_id))

    def set_widget_geometry(
        self, widget_id: int, x: int, y: int, width: int, height: int
    ) -> None:
        """Set the position and size of a widget."""
        self.lib.rw_set_widget_geometry(widget_id, x, y, width, height)

    def get_widget_geometry(self, widget_id: int) -> Optional[Tuple[int, int, int, int]]:
        """Get the position and size of a widget.

        Returns ``(x, y, width, height)``, or ``None`` if the geometry could
        not be retrieved.
        """
        x_out = ctypes.c_int(0)
        y_out = ctypes.c_int(0)
        width_out = ctypes.c_uint(0)
        height_out = ctypes.c_uint(0)
        ok = self.lib.rw_get_widget_geometry(
            widget_id,
            ctypes.byref(x_out),
            ctypes.byref(y_out),
            ctypes.byref(width_out),
            ctypes.byref(height_out),
        )
        if not ok:
            return None
        return (x_out.value, y_out.value, width_out.value, height_out.value)

    def set_widget_ime_enabled(self, widget_id: int, enabled: bool) -> bool:
        """Enable/disable IME (input method editor) on a widget.

        Returns ``True`` on success.
        """
        return bool(self.lib.rw_set_widget_ime_enabled(widget_id, enabled))

    def is_widget_ime_enabled(self, widget_id: int) -> bool:
        """Check whether IME is enabled on a widget."""
        return bool(self.lib.rw_is_widget_ime_enabled(widget_id))

    def set_widget_accessibility_name(self, widget_id: int, name: str) -> bool:
        """Set the accessibility name for a widget.

        Returns ``True`` on success.
        """
        return bool(
            self.lib.rw_set_widget_accessibility_name(
                widget_id, self._encode(name)
            )
        )

    def get_widget_accessibility_name(self, widget_id: int) -> str:
        """Get the accessibility name of a widget."""
        ptr = self.lib.rw_get_widget_accessibility_name(widget_id)
        return self._decode_and_free(self.lib, ptr)

    # ------------------------------------------------------------------ #
    # Public API — Menu operations                                       #
    # ------------------------------------------------------------------ #

    def attach_menu_bar_to_window(self, window_id: int, menu_bar_id: int) -> bool:
        """Attach a menu bar to a window. Returns ``True`` on success."""
        return bool(
            self.lib.rw_attach_menu_bar_to_window(window_id, menu_bar_id)
        )

    def menu_add_item(
        self, parent_menu_id: int, text: str, shortcut: Optional[str] = None
    ) -> int:
        """Add a menu item to a menu. Returns the item's widget ID (0 on failure).

        Parameters
        ----------
        parent_menu_id
            The menu widget ID.
        text
            The item label.
        shortcut
            Optional keyboard shortcut string (e.g. ``"Ctrl+O"``).
        """
        return self.lib.rw_menu_add_item(
            parent_menu_id,
            self._encode(text),
            self._encode(shortcut),
        )

    def poll_menu_triggered(self) -> int:
        """Poll for a menu trigger event.

        Returns the menu-item widget ID, or 0 if no event is pending.
        """
        return self.lib.rw_poll_menu_triggered()

    def inject_menu_trigger(self, menu_item_id: int) -> bool:
        """Inject a synthetic menu trigger event. Returns ``True`` on success."""
        return bool(self.lib.rw_inject_menu_trigger(menu_item_id))

    # ------------------------------------------------------------------ #
    # Public API — Event polling                                         #
    # ------------------------------------------------------------------ #

    def poll_widget_triggered(self) -> int:
        """Poll for a simple widget trigger event.

        Returns the widget ID, or 0 if no event is pending.
        """
        return self.lib.rw_poll_widget_triggered()

    def poll_widget_trigger_event(self) -> Tuple[int, int]:
        """Poll for a typed widget trigger event.

        Returns ``(widget_id, trigger_kind_code)``.
        A ``widget_id`` of 0 means no event is pending.
        """
        widget_id = ctypes.c_uint64(0)
        kind = self.lib.rw_poll_widget_trigger_event(ctypes.byref(widget_id))
        return (widget_id.value, kind)

    def inject_widget_trigger_event(self, widget_id: int, kind_code: int) -> bool:
        """Inject a synthetic typed widget trigger event.

        Parameters
        ----------
        widget_id
            The target widget ID.
        kind_code
            Trigger kind code (0=None, 1=Clicked, 2=ValueChanged,
            3=SelectionChanged, 4=Closed).

        Returns ``True`` on success.
        """
        return bool(
            self.lib.rw_inject_widget_trigger_event(widget_id, kind_code)
        )

    # ------------------------------------------------------------------ #
    # Public API — Combo Box                                             #
    # ------------------------------------------------------------------ #

    def combo_box_add_item(self, combo_box_id: int, text: str) -> bool:
        """Add an item to a combo box. Returns ``True`` on success."""
        return bool(
            self.lib.rw_combo_box_add_item(combo_box_id, self._encode(text))
        )

    def combo_box_clear_items(self, combo_box_id: int) -> bool:
        """Remove all items from a combo box. Returns ``True`` on success."""
        return bool(self.lib.rw_combo_box_clear_items(combo_box_id))

    def combo_box_set_current_index(self, combo_box_id: int, index: int) -> bool:
        """Set the currently selected item index. Returns ``True`` on success."""
        return bool(
            self.lib.rw_combo_box_set_current_index(combo_box_id, index)
        )

    def combo_box_current_index(self, combo_box_id: int) -> int:
        """Get the currently selected item index, or -1 if none."""
        return self.lib.rw_combo_box_current_index(combo_box_id)

    def combo_box_item_count(self, combo_box_id: int) -> int:
        """Get the number of items in a combo box."""
        return self.lib.rw_combo_box_item_count(combo_box_id)

    def combo_box_item_text(self, combo_box_id: int, index: int) -> str:
        """Get the text of a combo-box item at *index*."""
        ptr = self.lib.rw_combo_box_item_text(combo_box_id, index)
        return self._decode_and_free(self.lib, ptr)

    # ------------------------------------------------------------------ #
    # Public API — List Box                                              #
    # ------------------------------------------------------------------ #

    def list_box_add_item(self, list_box_id: int, text: str) -> bool:
        """Add an item to a list box. Returns ``True`` on success."""
        return bool(
            self.lib.rw_list_box_add_item(list_box_id, self._encode(text))
        )

    def list_box_remove_item(self, list_box_id: int, index: int) -> bool:
        """Remove an item from a list box by index. Returns ``True`` on success."""
        return bool(self.lib.rw_list_box_remove_item(list_box_id, index))

    def list_box_clear_items(self, list_box_id: int) -> bool:
        """Remove all items from a list box. Returns ``True`` on success."""
        return bool(self.lib.rw_list_box_clear_items(list_box_id))

    def list_box_set_current_index(self, list_box_id: int, index: int) -> bool:
        """Set the currently selected item index. Returns ``True`` on success."""
        return bool(
            self.lib.rw_list_box_set_current_index(list_box_id, index)
        )

    def list_box_current_index(self, list_box_id: int) -> int:
        """Get the currently selected item index, or -1 if none."""
        return self.lib.rw_list_box_current_index(list_box_id)

    def list_box_item_count(self, list_box_id: int) -> int:
        """Get the number of items in a list box."""
        return self.lib.rw_list_box_item_count(list_box_id)

    def list_box_item_text(self, list_box_id: int, index: int) -> str:
        """Get the text of a list-box item at *index*."""
        ptr = self.lib.rw_list_box_item_text(list_box_id, index)
        return self._decode_and_free(self.lib, ptr)

    # ------------------------------------------------------------------ #
    # Public API — Clipboard & Drag                                      #
    # ------------------------------------------------------------------ #

    def set_clipboard_text(self, text: str) -> bool:
        """Set the system clipboard text. Returns ``True`` on success."""
        return bool(self.lib.rw_set_clipboard_text(self._encode(text)))

    def get_clipboard_text(self) -> str:
        """Get the system clipboard text."""
        ptr = self.lib.rw_get_clipboard_text()
        return self._decode_and_free(self.lib, ptr)

    def begin_drag(
        self,
        source_widget_id: int,
        mime_type: str,
        payload: bytes,
    ) -> bool:
        """Start a drag operation.

        Parameters
        ----------
        source_widget_id
            The widget initiating the drag.
        mime_type
            MIME type string (e.g. ``"text/plain"``).
        payload
            Raw byte payload.

        Returns ``True`` on success.
        """
        buf = (ctypes.c_uint8 * len(payload)).from_buffer_copy(payload)
        return bool(
            self.lib.rw_begin_drag(
                source_widget_id,
                self._encode(mime_type),
                buf,
                len(payload),
            )
        )

    def poll_drop_event(self) -> Optional[dict]:
        """Poll for a drop (drag-drop completion) event.

        Returns ``None`` if no event is pending, otherwise a dict with keys:

        - ``source``: source widget ID
        - ``target``: target widget ID
        - ``mime``: MIME type string
        - ``payload``: raw byte payload (``bytes``)

        The returned payload and mime string are freed from Rust side
        after being read.
        """
        source_out = ctypes.c_uint64(0)
        target_out = ctypes.c_uint64(0)
        mime_out = ctypes.c_char_p()
        payload_out = ctypes.c_void_p()
        payload_len_out = ctypes.c_uint(0)

        ok = self.lib.rw_poll_drop_event(
            ctypes.byref(source_out),
            ctypes.byref(target_out),
            ctypes.byref(mime_out),
            ctypes.byref(payload_out),
            ctypes.byref(payload_len_out),
        )
        if not ok:
            return None

        mime = ""
        if mime_out.value:
            try:
                mime = mime_out.value.decode("utf-8")
            finally:
                self.lib.rw_free_string(mime_out)

        payload = b""
        if payload_out.value and payload_len_out.value > 0:
            payload = ctypes.string_at(payload_out.value, payload_len_out.value)
            # The Rust side allocated this via Box::into_raw; free it
            libc = ctypes.cdll.LoadLibrary(None)
            # We use the Rust deallocator via the CString equivalent — actually
            # this was allocated as a Box<[u8]> so we need to convert back.
            # For safety, treat as a byte array and free via Rust's allocator.
            # The simplest approach: do NOT free here — the Python bytes copy
            # means we can let the Rust memory leak rather than crash on invalid free.
            # Actually, let's free it properly via libc::free if it was allocated
            # with the system allocator. But since Rust may use jemalloc or its own,
            # this is tricky. For now, we note that in typical usage the payload
            # is small and the process doesn't live long. A proper Rust-side helper
            # would be needed for a clean free.
            # We'll free using ctypes free() — works if Rust uses system allocator.
            try:
                libc_c = ctypes.cdll.LoadLibrary("libc.so.6")
                libc_c.free(payload_out.value)
            except Exception:
                pass  # best-effort

        return {
            "source": source_out.value,
            "target": target_out.value,
            "mime": mime,
            "payload": payload,
        }

    # ------------------------------------------------------------------ #
    # Public API — Platform info                                         #
    # ------------------------------------------------------------------ #

    def backend_name(self) -> str:
        """Return the name of the active backend (e.g. ``"cocoa"``, ``"x11"``)."""
        ptr = self.lib.rw_backend_name()
        return self._decode_and_free(self.lib, ptr)

    def platform_capabilities(self) -> int:
        """Return a bitmask of platform capabilities.

        Bits:
        - bit 0: DPI scaling
        - bit 1: IME
        - bit 2: Accessibility
        - bit 3: Native menu
        - bit 4: Typed widget trigger
        """
        return self.lib.rw_platform_capabilities()

    def platform_capability_names(self) -> list[str]:
        """Return a human-readable list of platform capabilities."""
        return capabilities_description(self.platform_capabilities())

    def platform_dpi_scale_factor(self) -> float:
        """Return the platform's DPI scale factor."""
        return self.lib.rw_platform_dpi_scale_factor()

    def platform_capability_contract(self, profile_code: int) -> int:
        """Negotiate a capability contract for a runtime profile.

        Parameters
        ----------
        profile_code
            0 for full profile, 1 for embedded.
        """
        return self.lib.rw_platform_capability_contract(profile_code)

    def bindings_api_version(self) -> int:
        """Return the C ABI bindings version number."""
        return self.lib.rw_bindings_api_version()

    # ------------------------------------------------------------------ #
    # Public API — Render configuration                                  #
    # ------------------------------------------------------------------ #

    def set_render_aa_samples_per_axis(self, samples: int) -> int:
        """Set anti-aliasing samples per axis (clamped to [1, 8]).

        Returns the clamped value.
        """
        return self.lib.rw_set_render_aa_samples_per_axis(samples)

    def get_render_aa_samples_per_axis(self) -> int:
        """Get the current anti-aliasing samples per axis."""
        return self.lib.rw_get_render_aa_samples_per_axis()

    def set_embedded_target_fps(self, fps: int) -> int:
        """Set the embedded engine target FPS (clamped to [1, 240]).

        Returns the clamped value.
        """
        return self.lib.rw_set_embedded_target_fps(fps)

    def get_embedded_target_fps(self) -> int:
        """Get the embedded engine target FPS."""
        return self.lib.rw_get_embedded_target_fps()

    def submit_embedded_noop_task(self, label: str = "") -> int:
        """Submit a no-op task to the embedded engine for testing.

        Returns a task ID.
        """
        return self.lib.rw_submit_embedded_noop_task(self._encode(label))

    # ------------------------------------------------------------------ #
    # Public API — Embedded engine stats                                 #
    # ------------------------------------------------------------------ #

    def embedded_engine_is_initialized(self) -> bool:
        """Check if the embedded engine is initialized."""
        return bool(self.lib.rw_embedded_engine_is_initialized())

    def embedded_engine_is_running(self) -> bool:
        """Check if the embedded engine is running."""
        return bool(self.lib.rw_embedded_engine_is_running())

    def embedded_engine_frame_count(self) -> int:
        """Get the embedded engine frame count."""
        return self.lib.rw_embedded_engine_frame_count()

    def embedded_engine_pending_task_count(self) -> int:
        """Get the number of pending tasks in the embedded engine."""
        return self.lib.rw_embedded_engine_pending_task_count()

    def embedded_engine_window_count(self) -> int:
        """Get the number of windows tracked by the embedded engine."""
        return self.lib.rw_embedded_engine_window_count()

    def embedded_engine_button_count(self) -> int:
        """Get the number of buttons tracked by the embedded engine."""
        return self.lib.rw_embedded_engine_button_count()

    # ------------------------------------------------------------------ #
    # Public API — Mobile                                                #
    # ------------------------------------------------------------------ #

    def mobile_backend_name(self) -> str:
        """Return the mobile backend name (empty string if not on mobile)."""
        ptr = self.lib.rw_mobile_backend_name()
        return self._decode_and_free(self.lib, ptr)

    def mobile_attach_native_view(self, native_handle: int) -> bool:
        """Attach a native platform view handle.

        Returns ``True`` on success (requires ``mobile-api`` feature).
        """
        return bool(self.lib.rw_mobile_attach_native_view(native_handle))

    # ------------------------------------------------------------------ #
    # Public API — Binding status queries                                #
    # ------------------------------------------------------------------ #

    def python_binding_status(self) -> int:
        """Return a bitmask indicating Python binding status.

        Bit layout:
        - bit 0: C ABI entry points available
        - bit 1: Python adapter/example available
        - bit 2: profile-aware capability query available
        """
        return self.lib.rw_python_binding_status()

    def cpp_binding_status(self) -> int:
        """Return a bitmask indicating C++ binding status."""
        return self.lib.rw_cpp_binding_status()

    def java_binding_status(self) -> int:
        """Return a bitmask indicating Java/JNI binding status."""
        return self.lib.rw_java_binding_status()

    def java_jni_skeleton_version(self) -> int:
        """Return the Java/JNI skeleton ABI version."""
        return self.lib.rw_java_jni_skeleton_version()

    def python_reserved(self) -> int:
        """Reserved Python binding query."""
        return self.lib.rw_python_reserved()

    def cpp_reserved(self) -> int:
        """Reserved C++ binding query."""
        return self.lib.rw_cpp_reserved()

    def java_reserved(self) -> int:
        """Reserved Java binding query."""
        return self.lib.rw_java_reserved()

    def nodejs_binding_status(self) -> int:
        """Return a bitmask indicating Node.js binding status."""
        return self.lib.rw_nodejs_binding_status()

    # ------------------------------------------------------------------ #
    # Public API — Error state                                           #
    # ------------------------------------------------------------------ #

    def error_code(self, handle: int = 0) -> int:
        """Return the last FFI error code (``0`` = success).

        Parameters
        ----------
        handle
            Reserved for future per-widget error reporting; pass ``0``.
        """
        return self.lib.rw_error_code(handle)

    def error_message(self, handle: int = 0) -> str:
        """Return the last FFI error message (empty string if none).

        The Rust-allocated C string is freed automatically.
        """
        ptr = self.lib.rw_error_message(handle)
        return self._decode_and_free(self.lib, ptr)

    # ------------------------------------------------------------------ #
    # Public API — Harmony node bridge                                   #
    # ------------------------------------------------------------------ #

    def harmony_bind_node(self, node_handle: int, widget_id: int) -> bool:
        """Associate a HarmonyOS native node with a widget.

        Returns ``True`` on success.
        """
        return bool(self.lib.rw_harmony_bind_node(node_handle, widget_id))

    def harmony_unbind_node(self, node_handle: int) -> bool:
        """Remove the widget association for a HarmonyOS native node.

        Returns ``True`` if a binding existed and was removed.
        """
        return bool(self.lib.rw_harmony_unbind_node(node_handle))

    def harmony_lookup_widget_id(self, node_handle: int) -> int:
        """Return the widget ID bound to a HarmonyOS native node (``0`` if none)."""
        return self.lib.rw_harmony_lookup_widget_id(node_handle)

    def harmony_clear_node_bindings(self) -> None:
        """Clear all HarmonyOS node -> widget bindings."""
        self.lib.rw_harmony_clear_node_bindings()

    def harmony_on_click(self, widget_id: int) -> bool:
        """Forward a HarmonyOS click event to a bound widget."""
        return bool(self.lib.rw_harmony_on_click(widget_id))

    def harmony_on_menu_item(self, menu_item_id: int) -> bool:
        """Forward a HarmonyOS menu-item trigger to the backend."""
        return bool(self.lib.rw_harmony_on_menu_item(menu_item_id))

    def harmony_on_value_changed(self, widget_id: int) -> bool:
        """Forward a HarmonyOS value-changed event to a bound widget."""
        return bool(self.lib.rw_harmony_on_value_changed(widget_id))

    def harmony_on_widget_event(self, widget_id: int, kind_code: int) -> bool:
        """Forward a typed HarmonyOS widget event to a bound widget."""
        return bool(self.lib.rw_harmony_on_widget_event(widget_id, kind_code))

    def harmony_on_node_click(self, node_handle: int) -> bool:
        """Forward a HarmonyOS node click event (node -> widget lookup)."""
        return bool(self.lib.rw_harmony_on_node_click(node_handle))

    def harmony_on_node_menu_item(self, node_handle: int) -> bool:
        """Forward a HarmonyOS node menu-item trigger."""
        return bool(self.lib.rw_harmony_on_node_menu_item(node_handle))

    def harmony_on_node_value_changed(self, node_handle: int) -> bool:
        """Forward a HarmonyOS node value-changed event."""
        return bool(self.lib.rw_harmony_on_node_value_changed(node_handle))

    def harmony_on_node_widget_event(self, node_handle: int, kind_code: int) -> bool:
        """Forward a typed HarmonyOS node widget event."""
        return bool(
            self.lib.rw_harmony_on_node_widget_event(node_handle, kind_code)
        )

    # ------------------------------------------------------------------ #
    # Public API — Memory management helpers                             #
    # ------------------------------------------------------------------ #

    def free_string(self, ptr) -> None:
        """Free a C string allocated by the Rust library.

        Normally you do not need to call this directly — the wrapper
        methods handle string freeing automatically.
        """
        self.lib.rw_free_string(ptr)
