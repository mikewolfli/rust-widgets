#!/usr/bin/env python3
"""Example usage of the rust-widgets Python bindings.

Prerequisites:
    The ``librust_widgets.so`` (or ``.dylib`` / ``.dll``) shared library
    must be built and locatable (see ``rust_widgets.find_library``).

Run:
    python example.py
"""

from __future__ import annotations

import sys
import time

from rust_widgets import LibraryNotFoundError, RustWidgets
from rust_widgets.errors import (
    TRIGGER_CLICKED,
    TRIGGER_CLOSED,
    TRIGGER_SELECTION_CHANGED,
    TRIGGER_VALUE_CHANGED,
)


def main() -> None:
    # ------------------------------------------------------------------ #
    # 1. Initialise the library                                          #
    # ------------------------------------------------------------------ #
    try:
        rw = RustWidgets()
    except LibraryNotFoundError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        print(
            "Build the Rust library first:  cargo build --release",
            file=sys.stderr,
        )
        sys.exit(1)

    # Check API version
    api_ver = rw.bindings_api_version()
    print(f"[rust-widgets] C ABI version: {api_ver}")

    # Check backend
    backend = rw.backend_name()
    caps_mask = rw.platform_capabilities()
    caps_names = rw.platform_capability_names()
    dpi = rw.platform_dpi_scale_factor()
    print(f"[rust-widgets] Backend: {backend}")
    print(f"[rust-widgets] Capabilities mask: {caps_mask} -> {caps_names}")
    print(f"[rust-widgets] DPI scale factor: {dpi:.2f}")

    # Binding status
    py_status = rw.python_binding_status()
    print(f"[rust-widgets] Python binding status: {py_status:#06b}")

    # ------------------------------------------------------------------ #
    # 2. Initialise the library and create a window                       #
    # ------------------------------------------------------------------ #
    rw.init()
    print("[rust-widgets] Library initialised.")

    win = rw.create_window("Rust Widgets Demo", 100, 100, 640, 480)
    if win == 0:
        print("ERROR: Failed to create window", file=sys.stderr)
        sys.exit(1)
    print(f"[rust-widgets] Window created (id={win})")

    # ------------------------------------------------------------------ #
    # 3. Create a few widgets inside the window                           #
    # ------------------------------------------------------------------ #

    # Label
    label = rw.create_label(win, "Hello from Python!", 10, 10, 300, 24)
    print(f"[rust-widgets] Label created (id={label})")

    # Button
    btn = rw.create_button(win, "Click Me", 10, 40, 120, 32)
    print(f"[rust-widgets] Button created (id={btn})")

    # Checkbox
    cb = rw.create_checkbox(win, "Enable feature", 10, 80, 180, 24)
    print(f"[rust-widgets] Checkbox created (id={cb})")

    # Line edit (text input)
    line = rw.create_line_edit(win, "Type here...", 10, 110, 200, 24)
    print(f"[rust-widgets] Line edit created (id={line})")

    # Combo box
    combo = rw.create_combo_box(win, 10, 140, 160, 24)
    rw.combo_box_add_item(combo, "Option A")
    rw.combo_box_add_item(combo, "Option B")
    rw.combo_box_add_item(combo, "Option C")
    rw.combo_box_set_current_index(combo, 1)  # select "Option B"
    item_count = rw.combo_box_item_count(combo)
    current_idx = rw.combo_box_current_index(combo)
    current_text = rw.combo_box_item_text(combo, current_idx)
    print(
        f"[rust-widgets] Combo box created (id={combo}, items={item_count}, "
        f"selected={current_idx} -> '{current_text}')"
    )

    # List box
    lst = rw.create_list_box(win, 10, 170, 160, 80)
    rw.list_box_add_item(lst, "Item 1")
    rw.list_box_add_item(lst, "Item 2")
    rw.list_box_add_item(lst, "Item 3")
    rw.list_box_set_current_index(lst, 0)
    print(f"[rust-widgets] List box created (id={lst})")

    # Slider
    slider = rw.create_slider(win, 10, 260, 200, 24)
    print(f"[rust-widgets] Slider created (id={slider})")

    # Progress bar
    prog = rw.create_progress_bar(win, 10, 290, 200, 24)
    print(f"[rust-widgets] Progress bar created (id={prog})")

    # ------------------------------------------------------------------ #
    # 4. Demonstrate widget manipulation                                  #
    # ------------------------------------------------------------------ #
    rw.set_widget_text(label, "Welcome! Widgets are ready.")
    retrieved_text = rw.get_widget_text(label)
    print(f"[rust-widgets] Label text: '{retrieved_text}'")

    is_enabled = rw.is_widget_enabled(btn)
    print(f"[rust-widgets] Button enabled: {is_enabled}")

    is_visible = rw.is_widget_visible(label)
    print(f"[rust-widgets] Label visible: {is_visible}")

    # ------------------------------------------------------------------ #
    # 5. Set clipboard text                                               #
    # ------------------------------------------------------------------ #
    ok = rw.set_clipboard_text("Hello from rust-widgets!")
    if ok:
        clip = rw.get_clipboard_text()
        print(f"[rust-widgets] Clipboard: '{clip}'")

    # ------------------------------------------------------------------ #
    # 6. Menu bar example                                                 #
    # ------------------------------------------------------------------ #
    menu_bar = rw.create_menu_bar(win, 10, 320, 600, 24)
    file_menu = rw.create_menu(menu_bar, "&File", 0, 0, 80, 24)
    help_menu = rw.create_menu(menu_bar, "&Help", 80, 0, 80, 24)
    rw.attach_menu_bar_to_window(win, menu_bar)

    item_open = rw.menu_add_item(file_menu, "Open", "Ctrl+O")
    item_save = rw.menu_add_item(file_menu, "Save", "Ctrl+S")
    rw.menu_add_item(file_menu, "---", None)  # separator
    item_quit = rw.menu_add_item(file_menu, "Quit", "Ctrl+Q")
    item_about = rw.menu_add_item(help_menu, "About", None)
    print(f"[rust-widgets] Menu bar set up")

    # ------------------------------------------------------------------ #
    # 7. Show the window                                                  #
    # ------------------------------------------------------------------ #
    rw.show_widget(win)
    print("[rust-widgets] Window shown. Starting event loop...")
    print("[rust-widgets] Press Ctrl+C to exit.\n")

    # ------------------------------------------------------------------ #
    # 8. Event loop with polling                                          #
    # ------------------------------------------------------------------ #
    start_time = time.time()
    poll_count = 0
    try:
        while True:
            # Poll for simple widget triggers (returns widget id)
            triggered = rw.poll_widget_triggered()
            if triggered != 0:
                if triggered == btn:
                    print("[event] Button clicked!")
                elif triggered == cb:
                    print("[event] Checkbox toggled!")
                elif triggered == line:
                    print("[event] Line edit changed!")
                elif triggered == combo:
                    print("[event] Combo box selection changed!")
                elif triggered == slider:
                    print("[event] Slider value changed!")

            # Poll for typed trigger events
            wid, kind = rw.poll_widget_trigger_event()
            if wid != 0:
                if kind == TRIGGER_CLICKED:
                    print(f"[event] Clicked (widget={wid})")
                elif kind == TRIGGER_VALUE_CHANGED:
                    print(f"[event] Value Changed (widget={wid})")
                elif kind == TRIGGER_SELECTION_CHANGED:
                    print(f"[event] Selection Changed (widget={wid})")
                elif kind == TRIGGER_CLOSED:
                    print(f"[event] Closed (widget={wid})")

            # Poll for menu triggers
            menu_item = rw.poll_menu_triggered()
            if menu_item != 0:
                if menu_item == item_quit:
                    print("[event] Quit menu selected. Exiting...")
                    break
                elif menu_item == item_open:
                    print("[event] Open menu selected")
                elif menu_item == item_save:
                    print("[event] Save menu selected")
                elif menu_item == item_about:
                    print("[event] About menu selected")

            # Print a heartbeat every 2 seconds
            elapsed = time.time() - start_time
            poll_count += 1
            if poll_count % 100 == 0:
                print(f"[heartbeat] {elapsed:.1f}s elapsed, {poll_count} polls")

            # Small sleep to avoid busy-waiting
            time.sleep(0.01)

    except KeyboardInterrupt:
        print("\n[rust-widgets] Ctrl+C pressed.")

    # ------------------------------------------------------------------ #
    # 9. Cleanup                                                          #
    # ------------------------------------------------------------------ #
    rw.quit()
    print("[rust-widgets] Quit signal sent. Goodbye!")


if __name__ == "__main__":
    main()
