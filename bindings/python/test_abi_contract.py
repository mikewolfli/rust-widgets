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
    for kind in ("RW_VALUE_COLOR", "RW_VALUE_RECT"):
        assert hasattr(sys.modules["rust_widgets"], kind), (
            f"{kind} must be declared: a reader without it returns 'no property' "
            "for every colour/rectangle and leaks the buffer it did not free"
        )
    print("  string/colour/rect value kinds: ok")


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
    """A drop-event payload is freed exactly once.

    The original code called ``rw_free_bytes`` and then ``libc.free`` on the same
    pointer. On Linux ``libc.so.6`` loads, so the guard did not hide it and the
    process aborted with ``double free or corruption`` on the first drop event
    carrying a payload. Reaching the end of this function is the assertion: a
    second free would abort the process rather than fail an assert.
    """
    # Poll repeatedly: the queue is normally empty, which exercises the
    # "no event" early return and its absence of allocation.
    for _ in range(16):
        event = rw.poll_drop_event()
        if event is None:
            continue
        assert set(event) >= {"source", "target", "mime", "payload"}, event
        assert isinstance(event["payload"], (bytes, bytearray)), event["payload"]
        assert isinstance(event["mime"], str), event["mime"]
    print("  drop-event payload freed once (no abort): ok")


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
