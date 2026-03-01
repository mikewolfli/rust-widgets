# Python ctypes wrapper for the rust_widgets C ABI.
import ctypes
import os
import platform
from ctypes import c_char_p, c_float, c_int, c_uint, c_uint64, c_void_p


def _default_library_name() -> str:
    system = platform.system().lower()
    if system == "darwin":
        return "librust_widgets.dylib"
    if system == "windows":
        return "rust_widgets.dll"
    return "librust_widgets.so"


def _resolve_library_path() -> str:
    env_path = os.environ.get("RUST_WIDGETS_LIB")
    if env_path:
        return env_path

    root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    candidate = os.path.join(root, "target", "debug", _default_library_name())
    if os.path.exists(candidate):
        return candidate
    return _default_library_name()


class RustWidgets:
    def __init__(self, library_path: str | None = None):
        self._lib = ctypes.CDLL(library_path or _resolve_library_path())
        self._bind_signatures()

    def _bind_signatures(self) -> None:
        self._lib.rust_widgets_init.argtypes = []
        self._lib.rust_widgets_init.restype = None

        self._lib.rust_widgets_run.argtypes = []
        self._lib.rust_widgets_run.restype = None

        self._lib.rust_widgets_quit.argtypes = []
        self._lib.rust_widgets_quit.restype = None

        self._lib.rust_widgets_create_window.argtypes = [c_char_p, c_int, c_int, c_uint, c_uint]
        self._lib.rust_widgets_create_window.restype = c_uint64

        self._lib.rust_widgets_create_button.argtypes = [c_uint64, c_char_p, c_int, c_int, c_uint, c_uint]
        self._lib.rust_widgets_create_button.restype = c_uint64

        self._lib.rust_widgets_set_widget_text.argtypes = [c_uint64, c_char_p]
        self._lib.rust_widgets_set_widget_text.restype = None

        self._lib.rust_widgets_get_widget_text.argtypes = [c_uint64]
        self._lib.rust_widgets_get_widget_text.restype = c_void_p

        self._lib.rust_widgets_free_string.argtypes = [c_char_p]
        self._lib.rust_widgets_free_string.restype = None

        self._lib.rust_widgets_platform_capabilities.argtypes = []
        self._lib.rust_widgets_platform_capabilities.restype = c_uint

        self._lib.rust_widgets_platform_dpi_scale_factor.argtypes = []
        self._lib.rust_widgets_platform_dpi_scale_factor.restype = c_float

        self._lib.rust_widgets_set_render_aa_samples_per_axis.argtypes = [c_uint]
        self._lib.rust_widgets_set_render_aa_samples_per_axis.restype = c_uint

        self._lib.rust_widgets_get_render_aa_samples_per_axis.argtypes = []
        self._lib.rust_widgets_get_render_aa_samples_per_axis.restype = c_uint

        self._lib.rust_widgets_set_embedded_target_fps.argtypes = [c_uint]
        self._lib.rust_widgets_set_embedded_target_fps.restype = c_uint

        self._lib.rust_widgets_get_embedded_target_fps.argtypes = []
        self._lib.rust_widgets_get_embedded_target_fps.restype = c_uint

        self._lib.rust_widgets_submit_embedded_noop_task.argtypes = [c_char_p]
        self._lib.rust_widgets_submit_embedded_noop_task.restype = c_uint64

        self._lib.rust_widgets_embedded_engine_is_initialized.argtypes = []
        self._lib.rust_widgets_embedded_engine_is_initialized.restype = c_uint

        self._lib.rust_widgets_embedded_engine_is_running.argtypes = []
        self._lib.rust_widgets_embedded_engine_is_running.restype = c_uint

        self._lib.rust_widgets_embedded_engine_frame_count.argtypes = []
        self._lib.rust_widgets_embedded_engine_frame_count.restype = c_uint64

        self._lib.rust_widgets_embedded_engine_pending_task_count.argtypes = []
        self._lib.rust_widgets_embedded_engine_pending_task_count.restype = c_uint64

        self._lib.rust_widgets_embedded_engine_window_count.argtypes = []
        self._lib.rust_widgets_embedded_engine_window_count.restype = c_uint64

        self._lib.rust_widgets_embedded_engine_button_count.argtypes = []
        self._lib.rust_widgets_embedded_engine_button_count.restype = c_uint64

        self._lib.rust_widgets_platform_capability_contract.argtypes = [c_uint]
        self._lib.rust_widgets_platform_capability_contract.restype = c_uint

        self._lib.rust_widgets_bindings_api_version.argtypes = []
        self._lib.rust_widgets_bindings_api_version.restype = c_uint

        self._lib.rust_widgets_python_binding_status.argtypes = []
        self._lib.rust_widgets_python_binding_status.restype = c_uint

        self._lib.rust_widgets_cpp_binding_status.argtypes = []
        self._lib.rust_widgets_cpp_binding_status.restype = c_uint

        self._lib.rust_widgets_java_binding_status.argtypes = []
        self._lib.rust_widgets_java_binding_status.restype = c_uint

        self._lib.rust_widgets_java_jni_skeleton_version.argtypes = []
        self._lib.rust_widgets_java_jni_skeleton_version.restype = c_uint

    def init(self) -> None:
        self._lib.rust_widgets_init()

    def run(self) -> None:
        self._lib.rust_widgets_run()

    def quit(self) -> None:
        self._lib.rust_widgets_quit()

    def create_window(self, title: str, x: int, y: int, width: int, height: int) -> int:
        return int(self._lib.rust_widgets_create_window(title.encode("utf-8"), x, y, width, height))

    def create_button(self, parent: int, text: str, x: int, y: int, width: int, height: int) -> int:
        return int(self._lib.rust_widgets_create_button(parent, text.encode("utf-8"), x, y, width, height))

    def set_widget_text(self, widget_id: int, text: str) -> None:
        self._lib.rust_widgets_set_widget_text(widget_id, text.encode("utf-8"))

    def get_widget_text(self, widget_id: int) -> str:
        ptr = self._lib.rust_widgets_get_widget_text(widget_id)
        if not ptr:
            return ""
        value = ctypes.string_at(ptr).decode("utf-8")
        self._lib.rust_widgets_free_string(ctypes.cast(ptr, c_char_p))
        return value

    def platform_capabilities(self) -> int:
        return int(self._lib.rust_widgets_platform_capabilities())

    def dpi_scale_factor(self) -> float:
        return float(self._lib.rust_widgets_platform_dpi_scale_factor())

    def set_render_aa_samples_per_axis(self, samples: int) -> int:
        return int(self._lib.rust_widgets_set_render_aa_samples_per_axis(samples))

    def render_aa_samples_per_axis(self) -> int:
        return int(self._lib.rust_widgets_get_render_aa_samples_per_axis())

    def set_embedded_target_fps(self, fps: int) -> int:
        return int(self._lib.rust_widgets_set_embedded_target_fps(fps))

    def embedded_target_fps(self) -> int:
        return int(self._lib.rust_widgets_get_embedded_target_fps())

    def submit_embedded_noop_task(self, label: str = "python-noop") -> int:
        return int(self._lib.rust_widgets_submit_embedded_noop_task(label.encode("utf-8")))

    def embedded_engine_is_initialized(self) -> bool:
        return bool(self._lib.rust_widgets_embedded_engine_is_initialized())

    def embedded_engine_is_running(self) -> bool:
        return bool(self._lib.rust_widgets_embedded_engine_is_running())

    def embedded_engine_frame_count(self) -> int:
        return int(self._lib.rust_widgets_embedded_engine_frame_count())

    def embedded_engine_pending_task_count(self) -> int:
        return int(self._lib.rust_widgets_embedded_engine_pending_task_count())

    def embedded_engine_window_count(self) -> int:
        return int(self._lib.rust_widgets_embedded_engine_window_count())

    def embedded_engine_button_count(self) -> int:
        return int(self._lib.rust_widgets_embedded_engine_button_count())

    def capability_contract(self, embedded: bool = False) -> int:
        profile_code = 1 if embedded else 0
        return int(self._lib.rust_widgets_platform_capability_contract(profile_code))

    def bindings_api_version(self) -> int:
        return int(self._lib.rust_widgets_bindings_api_version())

    def python_binding_status(self) -> int:
        return int(self._lib.rust_widgets_python_binding_status())

    def cpp_binding_status(self) -> int:
        return int(self._lib.rust_widgets_cpp_binding_status())

    def java_binding_status(self) -> int:
        return int(self._lib.rust_widgets_java_binding_status())

    def java_jni_skeleton_version(self) -> int:
        return int(self._lib.rust_widgets_java_jni_skeleton_version())
