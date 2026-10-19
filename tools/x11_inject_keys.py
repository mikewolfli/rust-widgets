#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Type a path into a focused window using the X server's XTEST extension.

# Why this exists

`examples/probe_file_picker.rs` stages a real file chooser through the library and, until this
tool existed, answered it *programmatically* (a timeout inside the dialog's nested loop called
`response(Cancel)`). That proves the dialog was up and that an answer travels back — but not that
the picker works when a *person* answers it, which is the case the designer's report was about
("无显示会话" meant they could not exercise the input path at all).

This tool supplies the missing half: it injects real X key events into the dialog, so the answer
the probe receives comes from the same code path a human's typing would take (X server → GTK's
key handling → the dialog's location entry → `run()` returns `Accept` with a filename).

# The safety rule, which is the important part

Keyboard injection goes to whatever holds the input focus, and the point of this tool is to run on
a machine a person is using. So it **verifies before it types**, and refuses rather than guesses:

1. the window whose title matches must exist (`wmctrl -l`);
2. the window manager must report it as the active window (`_NET_ACTIVE_WINDOW`);
3. the X server's own input focus (`XGetInputFocus`) must be that window or a descendant of it —
   the focus a WM reports and the focus the server actually set are different facts, and it is the
   second one that decides where the keystrokes land.

Any of those failing exits `2` without sending a single event. Callers treat `2` as "this host
cannot stage typed input", not as a failure.

# Usage

    x11_inject_keys.py <title-substring> <text> [--activate] [--plain] [--key <keysym-name>]...

`--activate` asks the window manager to focus the window first (and still runs the three checks
above before typing). `--key` appends named keysym taps after the text, e.g. `--key Return`.
`--click x y` / `--click-relative x y` moves the pointer and clicks before typing (absolute root
coordinates, or window-relative ones). `--ctrl-l` presses Ctrl+L first, to open GTK's location
popup; it is **opt-in** because that popup can be slow to take focus and a key that arrives too
early is dropped (see the probe for the default, which types into a save dialog's name entry
instead). `--plain` skips that shortcut for an ordinary text entry.

Exit codes: 0 = typed, 2 = refused by the safety rule (nothing sent), 3 = no matching window,
4 = no X display, 5 = XTEST unavailable.
"""

from __future__ import annotations

import ctypes
import ctypes.util
import subprocess
import sys
import time

# Keysyms from `X11/keysymdef.h` that this tool needs by name rather than by character.
KEYSYM = {
    "Control_L": 0xFFE3,
    "Shift_L": 0xFFE1,
    "Return": 0xFF0D,
    "Escape": 0xFF1B,
    "Tab": 0xFF09,
    "BackSpace": 0xFF08,
}


def fail(code: int, message: str):
    """Refuse, with the reason on stderr and an exit code the caller can classify."""
    print(f"x11_inject_keys: {message}", file=sys.stderr)
    sys.exit(code)


class X:
    """The handful of X11/XTEST calls this tool needs, loaded through `ctypes`.

    # Why every prototype is declared

    `ctypes` guesses a function's signature when none is given, and the guess for a pointer
    argument is a 32-bit integer. `Display *` is 64 bits, so an undeclared prototype **truncates
    it** and the first call that dereferences it — `XFlush` here — segfaults the process. Measured:
    the first version of this tool died with SIGSEGV (`exit 139`) and the probe read that as "the
    typist refused", which is why the probe now reports the raw exit code too.
    """

    def __init__(self) -> None:
        x11 = ctypes.CDLL(ctypes.util.find_library("X11") or "libX11.so.6")
        xtst = ctypes.CDLL(ctypes.util.find_library("Xtst") or "libXtst.so.6")

        x11.XOpenDisplay.restype = ctypes.c_void_p
        x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
        x11.XCloseDisplay.restype = ctypes.c_int
        x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
        x11.XFlush.restype = ctypes.c_int
        x11.XFlush.argtypes = [ctypes.c_void_p]
        x11.XKeysymToKeycode.restype = ctypes.c_ubyte
        x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        x11.XKeycodeToKeysym.restype = ctypes.c_ulong
        x11.XKeycodeToKeysym.argtypes = [ctypes.c_void_p, ctypes.c_ubyte, ctypes.c_int]
        x11.XGetInputFocus.restype = ctypes.c_int
        x11.XGetInputFocus.argtypes = [
            ctypes.c_void_p,
            ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.c_int),
        ]
        x11.XQueryTree.restype = ctypes.c_int
        x11.XQueryTree.argtypes = [
            ctypes.c_void_p,
            ctypes.c_ulong,
            ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.POINTER(ctypes.c_ulong)),
            ctypes.POINTER(ctypes.c_uint),
        ]
        x11.XFree.restype = ctypes.c_int
        x11.XFree.argtypes = [ctypes.c_void_p]

        xtst.XTestFakeKeyEvent.restype = ctypes.c_int
        xtst.XTestFakeKeyEvent.argtypes = [
            ctypes.c_void_p,
            ctypes.c_uint,
            ctypes.c_int,
            ctypes.c_ulong,
        ]
        xtst.XTestFakeMotionEvent.restype = ctypes.c_int
        xtst.XTestFakeMotionEvent.argtypes = [
            ctypes.c_void_p,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_ulong,
        ]
        xtst.XTestFakeButtonEvent.restype = ctypes.c_int
        xtst.XTestFakeButtonEvent.argtypes = [
            ctypes.c_void_p,
            ctypes.c_uint,
            ctypes.c_int,
            ctypes.c_ulong,
        ]

        self.x11 = x11
        self.xtst = xtst
        self.display = x11.XOpenDisplay(None)
        if not self.display:
            fail(4, "XOpenDisplay returned null: no X display to inject into")

    def keysym_to_keycode(self, keysym: int) -> int:
        """The keycode that produces `keysym` on this layout, or 0."""
        return int(self.x11.XKeysymToKeycode(self.display, ctypes.c_ulong(keysym)))

    def keycode_unshifted_keysym(self, keycode: int) -> int:
        """The keysym this keycode produces with no modifier held."""
        return int(self.x11.XKeycodeToKeysym(self.display, ctypes.c_ubyte(keycode), 0))

    def tap(self, keysym: int, shift: bool = False) -> None:
        """Press and release `keysym`, holding shift when the character needs it."""
        keycode = self.keysym_to_keycode(keysym)
        if not keycode:
            fail(2, f"no keycode for keysym {keysym:#x} on this layout")
        shift_code = self.keysym_to_keycode(KEYSYM["Shift_L"])
        if shift:
            self.xtst.XTestFakeKeyEvent(self.display, shift_code, True, 0)
        self.xtst.XTestFakeKeyEvent(self.display, keycode, True, 0)
        self.xtst.XTestFakeKeyEvent(self.display, keycode, False, 0)
        if shift:
            self.xtst.XTestFakeKeyEvent(self.display, shift_code, False, 0)
        self.x11.XFlush(self.display)

    def click(self, x: int, y: int) -> None:
        """Move the pointer to `x, y` and click button 1 there."""
        self.xtst.XTestFakeMotionEvent(self.display, -1, x, y, 0)
        self.x11.XFlush(self.display)
        time.sleep(0.05)
        self.xtst.XTestFakeButtonEvent(self.display, 1, True, 0)
        self.xtst.XTestFakeButtonEvent(self.display, 1, False, 0)
        self.x11.XFlush(self.display)

    def input_focus_chain(self) -> list[int]:
        """The input focus window and its ancestors, root last.

        The X server's own focus, not the window manager's `_NET_ACTIVE_WINDOW`: the two are
        different facts, and only the first decides where an injected key event lands.
        """
        focus = ctypes.c_ulong(0)
        revert = ctypes.c_int(0)
        if not self.x11.XGetInputFocus(self.display, ctypes.byref(focus), ctypes.byref(revert)):
            return []
        chain: list[int] = []
        window = int(focus.value)
        while window and window not in chain and len(chain) < 16:
            chain.append(window)
            root = ctypes.c_ulong(0)
            parent = ctypes.c_ulong(0)
            children = ctypes.POINTER(ctypes.c_ulong)()
            count = ctypes.c_uint(0)
            ok = self.x11.XQueryTree(
                self.display,
                ctypes.c_ulong(window),
                ctypes.byref(root),
                ctypes.byref(parent),
                ctypes.byref(children),
                ctypes.byref(count),
            )
            if children:
                self.x11.XFree(children)
            if not ok:
                break
            window = int(parent.value)
        return chain


def window_id_for_title(title: str) -> int | None:
    """The window manager's id for the first window whose title contains `title`."""
    listing = subprocess.run(["wmctrl", "-l"], capture_output=True, text=True)
    for line in listing.stdout.splitlines():
        if title in line:
            return int(line.split()[0], 16)
    return None


def window_geometry(window: int) -> tuple[int, int, int, int] | None:
    """`(x, y, width, height)` of `window` in screen coordinates, from `xwininfo`."""
    info = subprocess.run(["xwininfo", "-id", hex(window)], capture_output=True, text=True)
    fields = {}
    for line in info.stdout.splitlines():
        key, _, value = line.strip().partition(":")
        fields[key.strip()] = value.strip()
    try:
        return (
            int(fields["Absolute upper-left X"]),
            int(fields["Absolute upper-left Y"]),
            int(fields["Width"]),
            int(fields["Height"]),
        )
    except (KeyError, ValueError):
        return None


def active_window_id() -> int | None:
    """`_NET_ACTIVE_WINDOW` from the root window, as an integer id."""
    prop = subprocess.run(["xprop", "-root", "_NET_ACTIVE_WINDOW"], capture_output=True, text=True)
    if "#" not in prop.stdout:
        return None
    return int(prop.stdout.rsplit("#", 1)[1].strip(), 16)


def main(argv: list[str]) -> int:
    if len(argv) < 3:
        print(__doc__)
        return 64
    title, text = argv[1], argv[2]
    activate = "--activate" in argv
    trailing: list[str] = []
    if "--key" in argv:
        index = argv.index("--key")
        trailing = argv[index + 1 :]
    click: tuple[int, int] | None = None
    if "--click" in argv:
        index = argv.index("--click")
        try:
            click = (int(argv[index + 1]), int(argv[index + 2]))
        except (IndexError, ValueError):
            fail(64, "--click needs two integers, x and y")
    click_relative: tuple[int, int] | None = None
    if "--click-relative" in argv:
        index = argv.index("--click-relative")
        try:
            click_relative = (int(argv[index + 1]), int(argv[index + 2]))
        except (IndexError, ValueError):
            fail(64, "--click-relative needs two integers, x and y")
    # Plain typing skips the Ctrl+L that opens GTK's file-chooser location popup. A probe that
    # types into an ordinary entry wants the toolkit's normal key path, not a dialog shortcut.
    plain = "--plain" in argv
    # Staging an input-method composition needs the input method *active* for the focused field.
    # A desktop session may start with it inactive, and `-o` acts on the focused input context —
    # which exists only after the toolkit called `focus_in`.
    ime_activate = "--ime-activate" in argv

    x = X()

    window = window_id_for_title(title)
    if window is None:
        fail(3, f"no window manager window whose title contains {title!r}")

    if click is not None or click_relative is not None:
        # The pointer's own safety rule: the coordinates must be inside the window this tool was
        # pointed at, so a mistake here cannot click on somebody else's application.
        geometry = window_geometry(window)
        if geometry is None:
            fail(3, f"`xwininfo` could not report the geometry of {hex(window)}")
        left, top, width, height = geometry
        if click_relative is not None:
            click = (left + click_relative[0], top + click_relative[1])
        assert click is not None
        inside = left <= click[0] < left + width and top <= click[1] < top + height
        if not inside:
            fail(2, f"refusing to click: {click} is outside {hex(window)} at {geometry}")
        x.click(*click)
        # A click that changes focus makes the checks below meaningful; the delay lets the toolkit
        # finish handling it.
        time.sleep(0.2)

    if activate:
        subprocess.run(["wmctrl", "-i", "-a", hex(window)], capture_output=True)
        time.sleep(0.2)

    # The safety rule: both the window manager's answer and the server's own focus must be our
    # window (or a descendant of it). The server's answer is the one that decides where keys land.
    active = active_window_id()
    if active != window:
        fail(2, f"refusing to type: active window is {active and hex(active)}, not {hex(window)}")
    if window not in x.input_focus_chain():
        fail(2, "refusing to type: the X input focus is not this window or one of its children")

    # Let the dialog settle: a key that arrives before the window has finished mapping and taken
    # keyboard handling is delivered to the window and ignored by the toolkit. Measured: without
    # this wait, Ctrl+L was sometimes dropped, the characters then went to the file list as a
    # type-ahead search, and the dialog ended in whatever the list had selected.
    time.sleep(0.25)

    if ime_activate:
        switched = subprocess.run(["fcitx5-remote", "-o"], capture_output=True, text=True)
        if switched.returncode != 0:
            fail(2, f"could not activate the input method: {switched.stderr.strip()}")
        time.sleep(0.3)

    # Ctrl+L opens GTK's location entry; the map is typed as plain characters, which the server
    # turns into keysyms by its own layout. `--plain` skips it for an ordinary entry.
    if not plain:
        control = x.keysym_to_keycode(KEYSYM["Control_L"])
        x.xtst.XTestFakeKeyEvent(x.display, control, True, 0)
        x.tap(ord("l"))
        x.xtst.XTestFakeKeyEvent(x.display, control, False, 0)
        x.x11.XFlush(x.display)
        # The location entry needs a moment to appear and take focus; typing into the file list
        # instead would be a type-ahead search, which is a different interaction.
        time.sleep(0.6)

    for character in text:
        keysym = ord(character)
        keycode = x.keysym_to_keycode(keysym)
        if not keycode:
            fail(2, f"this keyboard layout cannot produce {character!r}")
        # A character that shares its keycode with a shifted level (a capital, an underscore)
        # needs the shift held for it, which is what the unshifted keysym tells us.
        needs_shift = x.keycode_unshifted_keysym(keycode) != keysym
        x.tap(keysym, shift=needs_shift)
        time.sleep(0.035)

    for name in trailing:
        keysym = KEYSYM.get(name)
        if keysym is None:
            try:
                keysym = int(name, 0)
            except ValueError:
                fail(2, f"unknown key name {name!r}")
        x.tap(keysym)
        time.sleep(0.05)

    print(f"x11_inject_keys: typed {len(text)} character(s) into {hex(window)} and pressed {trailing}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
