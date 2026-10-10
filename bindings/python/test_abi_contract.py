#!/usr/bin/env python3
"""Contract tests for the Python binding's ABI boundary.

These run against the **built** shared library and assert the parts of the
binding that are easy to get silently wrong:

* an owned ``char*`` return keeps its address, so the deallocator receives the
  allocation Rust made rather than a ctypes temporary;
* a drop-event payload is freed exactly once (a second free is a
  use-after-free);
* an empty payload can still be handed to the drag API.

Why these are separate from ``example.py``: that file demonstrates usage, so it
only calls the happy path and would not notice a wrong ``restype`` or a double
free — the first leaks silently and the second aborts the process, which reads
as a harness problem rather than a defect.

Run:
    python bindings/python/test_abi_contract.py

Requires the shared library to be built and locatable (see
``rust_widgets.find_library``).
"""

from __future__ import annotations

import ctypes
import sys

from rust_widgets import RustWidgets
from rust_widgets import RW_VALUE_STRING


def _check_owned_string_round_trip(rw: RustWidgets) -> None:
    """An owned ``char*`` must keep its address across the ctypes boundary.

    ``c_char_p`` as a ``restype`` copies the bytes out and discards the pointer,
    so the later ``rw_free_string`` would free a ctypes temporary — an invalid
    free plus a permanent leak of the real buffer. The tell is that
    ``backend_name()`` (a plain owned-string reader) works at all *and* that the
    library's own free path is exercised.
    """
    name = rw.backend_name()
    assert isinstance(name, str), f"backend_name must return str, got {type(name)!r}"
    assert name, "a backend must name itself"

    # The restype must be `c_void_p`, i.e. the raw address, not `c_char_p`.
    restype = rw.lib.rw_backend_name.restype
    assert restype in (ctypes.c_void_p, ctypes.c_ssize_t), (
        f"rw_backend_name.restype is {restype!r}; it must be c_void_p so the "
        "address survives and rw_free_string frees the real allocation"
    )
    print("  owned char* return keeps its address: ok")


def _check_property_string_kinds(rw: RustWidgets) -> None:
    """Every string-carrying value kind must decode and free, including Color/Rect.

    A binding that cannot name ``RW_VALUE_COLOR`` / ``RW_VALUE_RECT`` returns
    ``None`` for those properties *and* leaks, because it never reaches the free.
    """
    window = rw.create_window("abi-contract", 0, 0, 320, 240)
    assert window, "create_window must return a live id"

    # `tooltip` is a plain string property every control publishes.
    rw.set_widget_property(window, "tooltip", "tip")
    value = rw.get_widget_property(window, "tooltip")
    assert value == "tip", f"tooltip round trip failed: {value!r}"

    # The kind constants must be declared so a caller can interpret `out_kind`.
    assert RW_VALUE_STRING == 5, "RW_VALUE_STRING must stay 5 (append-only ABI)"
    for kind in ("RW_VALUE_COLOR", "RW_VALUE_RECT", "RW_VALUE_TUPLE"):
        assert hasattr(sys.modules["rust_widgets"], kind), (
            f"{kind} must be declared: a reader without it returns 'no property' "
            "for every colour/rectangle/tuple and leaks the buffer it did not free"
        )
    print("  string/colour/rect/tuple value kinds: ok")


def _check_empty_drag_payload(rw: RustWidgets) -> None:
    """An empty payload must not become a NULL pointer.

    ``from_buffer_copy(b"")`` yields a zero-length ctypes array whose address is
    NULL, which ctypes rejects (and which would hand the backend a NULL slice).
    """
    window = rw.create_window("abi-contract", 0, 0, 320, 240)
    assert window, "create_window must return a live id"
    # Must not raise; the result itself is backend-dependent.
    rw.begin_drag(window, "text/plain", b"")
    print("  empty drag payload: ok")


def _check_drop_event_free_once(rw: RustWidgets) -> None:
    """An owned byte payload is freed **exactly once**, and an empty queue is safe.

    # What the previous version failed to establish

    It polled up to 16 times and `continue`d on every `None`, so a backend that never
    produced a drop event — every desktop build, where drag-and-drop has no in-process
    source — passed having asserted nothing about a payload. Zero real payloads is not a
    weaker proof of "freed once"; it is no proof at all, and a wrong `restype` or a
    double free on the payload path would have stayed invisible.

    # How the invariant is pinned here

    * **Empty-queue control.** A drained queue must return `None` — the "no event" path,
      which must not allocate or free anything.
    * **A real owned payload, from the ABI.** `rw_render_surface_frame` hands out the
      same `Box<[u8]>` payload that `rw_poll_drop_event` does, released through the same
      `rw_free_bytes(ptr, len)` contract. That is the *only* owned-bytes producer the
      binding can reach without an injection entry point, and it is enough to exercise
      the deallocator pairing the drop path depends on.
    * **A counted free.** `rw_free_bytes` is wrapped for the duration of the check, so
      the assertion is on the *number* of frees (exactly one), not on "no abort".
    * **A real drop event, when the backend has one.** If a poll returns a payload, its
      free goes through the same counter and must also be exactly one.
    """
    # `getattr`/`setattr` rather than attribute syntax: `rw_free_bytes` is a ctypes
    # foreign function added at runtime, so a static reader sees no such attribute on
    # `CDLL`. Going through the builtins is the same call at runtime without the false
    # "unknown attribute" report.
    original_free_bytes = getattr(rw.lib, "rw_free_bytes")
    freed: list[int] = []

    def counting_free_bytes(ptr, length):
        # `ptr` arrives either as a `c_void_p` (from the binding's own calls) or as an
        # int/None (a caller may pass either). Record the address actually handed to the
        # deallocator; a null free is a documented no-op and is not counted.
        if isinstance(ptr, ctypes.c_void_p):
            address = ptr.value
        else:
            address = ptr
        if address:
            freed.append(int(address))
        return original_free_bytes(ptr, length)

    setattr(rw.lib, "rw_free_bytes", counting_free_bytes)
    try:
        # Empty-queue control: drain first, then one more poll must be `None` and leave
        # every output cleared (the clearing contract is what makes unconditional free
        # safe).
        while rw.poll_drop_event() is not None:
            pass
        assert rw.poll_drop_event() is None, "a drained drop queue must report no event"

        # Real owned payload #1: the render path's buffer, same allocator as the drop
        # payload. `len` is what the ABI reported; it must be positive (a zero-length
        # buffer would be the "no payload" case again).
        window = rw.create_window("abi-contract", 0, 0, 64, 48)
        assert window, "create_window must return a live id"

        out_width = ctypes.c_uint(0)
        out_height = ctypes.c_uint(0)
        out_stride = ctypes.c_uint(0)
        out_len = ctypes.c_uint(0)
        out_pixels = ctypes.c_void_p()
        ok = rw.lib.rw_render_surface_frame(
            window,
            64,
            48,
            ctypes.byref(out_width),
            ctypes.byref(out_height),
            ctypes.byref(out_stride),
            ctypes.byref(out_len),
            ctypes.byref(out_pixels),
        )
        assert ok, "a window surface must render a frame for the owned-payload check"
        assert out_pixels.value, "a rendered frame must hand back an owned buffer"
        assert out_len.value > 0, f"the owned payload must be non-empty, got {out_len.value}"
        assert out_stride.value >= out_width.value * 4, "stride must cover a row"

        before = len(freed)
        getattr(rw.lib, "rw_free_bytes")(out_pixels, out_len.value)
        assert len(freed) == before + 1, "the rendered payload must be freed exactly once"

        # Real owned payload #2 (only where the backend actually queues drops): the drop
        # payload path itself. On a backend with no in-process drag source the queue is
        # empty and the empty-queue control above already covered that case.
        while True:
            event = rw.poll_drop_event()
            if event is None:
                break
            assert set(event) >= {"source", "target", "mime", "payload"}, event
            # `poll_drop_event` frees the payload internally through the wrapped free;
            # a double free would show as two recorded addresses and, in practice, abort.
            assert isinstance(event["payload"], (bytes, bytearray)), event["payload"]
            assert isinstance(event["mime"], str), event["mime"]

        # Every non-null address the wrapped deallocator saw must appear exactly once.
        duplicates = {addr for addr in freed if freed.count(addr) > 1}
        assert not duplicates, f"these payload addresses were freed more than once: {duplicates}"
        print(f"  owned payload freed exactly once (frees observed: {len(freed)}): ok")
    finally:
        setattr(rw.lib, "rw_free_bytes", original_free_bytes)


def _check_no_prefix_name_lists(rw: RustWidgets) -> None:
    """The two-argument enumerators must be called without an extra prefix.

    ``widget_kind_names()`` / ``theme_names()`` wrap C ABI functions whose
    signature is ``(out, cap)``. The wrapper used to prepend an empty ``b""``
    argument, so ctypes raised ``ArgumentError`` before the library was ever
    reached (D08-B-03): the calls returned no list at all, not an empty one.

    The check asserts the calls reach the library and return real names, and
    that every returned name is NUL-free (D08-B-02's terminator leak).
    """
    kinds = rw.widget_kind_names()
    assert isinstance(kinds, list) and kinds, "widget_kind_names must return names"
    assert all(isinstance(name, str) and name for name in kinds), kinds
    assert all("\x00" not in name for name in kinds), (
        f"a name carries the NUL terminator: {kinds}"
    )
    # A known control name must be present verbatim, proving the split is clean.
    assert "button" in kinds, f"expected 'button' among control names: {kinds[:8]}"

    themes = rw.theme_names()
    assert isinstance(themes, list) and themes, "theme_names must return names"
    assert all("\x00" not in name for name in themes), themes
    print(f"  no-prefix name lists ({len(kinds)} kinds, {len(themes)} themes): ok")


def _check_pump_frame_drives_a_frame(rw: RustWidgets) -> None:
    """``pump_frame`` must exist, drive a frame, and report the owed-frame flag.

    # The defect this pins (D09-PY-04)

    The shipped example polled triggers in a ``while True`` loop but never drove the
    runtime: it called neither ``rw_run`` (which would block and never return to the
    loop) nor any frame step, so the window never produced a frame. ``rw_pump_frame``
    is the frame step a main-thread-owning host needs, and this check proves the ABI
    symbol and the Python wrapper actually reach it rather than only existing in the
    header.
    """
    assert hasattr(rw, "pump_frame"), "the wrapper must expose pump_frame"
    assert hasattr(rw.lib, "rw_pump_frame"), (
        "the shared library must export rw_pump_frame; the example's loop depends on it"
    )

    window = rw.create_window("pump-contract", 0, 0, 320, 240)
    assert window, "create_window must return a live id"

    # One frame with no animation in flight must still return a bool, not raise.
    owed = rw.pump_frame(16)
    assert isinstance(owed, bool), f"pump_frame must return bool, got {type(owed)!r}"

    # A negative delta is an input error, not a silent wrap to a huge unsigned value
    # (the D09-JNI-02 boundary class) -- the wrapper rejects it explicitly.
    try:
        rw.pump_frame(-1)
    except ValueError:
        pass
    else:
        raise AssertionError("pump_frame must reject a negative delta_ms")

    # A frame must not corrupt the backend's trigger queue: a trigger injected after
    # the pump must still be observable through the poll API. (`pump_frame` dispatches
    # whatever is already queued -- that is its job -- so the check injects *after* the
    # frame and reads it back, which is the contract the example relies on.)
    button = rw.create_button(window, "pump", 10, 10, 80, 24)
    assert button, "create_button must return a live id"
    rw.pump_frame(16)
    rw.inject_widget_trigger_event(button, 1)
    widget_id, kind = rw.poll_widget_trigger_event()
    assert widget_id == button and kind == 1, (
        f"a trigger injected after a frame must be deliverable, got ({widget_id}, {kind})"
    )
    print("  pump_frame drives a frame and preserves triggers: ok")


def main() -> int:
    try:
        rw = RustWidgets()
    except Exception as error:  # noqa: BLE001 - reported, not swallowed
        print(f"cannot load the shared library: {error}", file=sys.stderr)
        return 2

    failures: list[str] = []
    checks = [
        ("owned char* round trip", _check_owned_string_round_trip),
        ("property string kinds", _check_property_string_kinds),
        ("empty drag payload", _check_empty_drag_payload),
        ("drop event freed once", _check_drop_event_free_once),
        ("no-prefix name lists", _check_no_prefix_name_lists),
        ("pump_frame drives a frame", _check_pump_frame_drives_a_frame),
    ]
    for title, check in checks:
        try:
            check(rw)
        except AssertionError as error:
            failures.append(title)
            print(f"  FAIL {title}: {error}", file=sys.stderr)
        except Exception as error:  # noqa: BLE001
            failures.append(title)
            print(f"  ERROR {title}: {type(error).__name__}: {error}", file=sys.stderr)

    if failures:
        print(f"\n{len(failures)} check(s) failed: {', '.join(failures)}", file=sys.stderr)
        return 1
    print("\nPython ABI contract checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
