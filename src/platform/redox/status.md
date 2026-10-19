# Redox OS (Orbital) backend — status

Reference: winit's `winit-orbital` backend (protocol role and event vocabulary), used as
documentation only; this crate does not depend on winit.

| Area | Status | Evidence / boundary |
|---|---|---|
| Window creation | ✅ Implemented | `OrbitalPlatform::create_window` opens a real `orbclient::Window` with the async/resizable/scalable flags. |
| Window painting | ✅ Implemented | `render_frame_tree` → `Window::data_mut` → `Window::sync`, one frame per tick. |
| Pointer input and focus | ✅ Implemented | Orbital mouse/button records are routed through `dispatch_pointer_event`, which also focuses the hit control (the same entry point every backend uses). |
| Keyboard / text input | ✅ Implemented | Key records become `KeyPress`/`KeyRelease`; Orbital `TextInput` records become `Event::TextInput` and are never forwarded as a second key event. Control chords suppress text so `Ctrl+C` stays a command. |
| Tab traversal | ✅ Implemented | `K_TAB` calls `runtime::focus_next` (Shift reverses), matching the GTK/Windows hosts. |
| Window resize | ✅ Implemented | `Resize` records queue the library's `Resized` trigger and update `Window::set_size`. |
| Clipboard (text) | ✅ Implemented | `Window::set_clipboard` / `Window::clipboard` with `MediaKind::Text`. |
| Display enumeration | ✅ Implemented | `get_display_size` + `EVENT_SCALE`; `displays()` reports the screen the compositor described. |
| IME composition | ⛔ Not supported by Orbital | Orbital defines no input-method client (`winit-orbital` also reports `ime_capabilities() -> None`); `capabilities().ime` is `false`. |
| Accessibility | ⛔ Not supported by Orbital | No accessibility protocol; `capabilities().accessibility` is `false`. |
| Native menus | ⛔ Not supported by Orbital | No menu protocol; `capabilities().native_menu` is `false`. |
| System file picker | ⛔ Not supported by Orbital | No picker protocol; `capabilities().file_dialog` is `false`. |
| Runtime execution | 🔶 Compiled, not run on this host | `tools/check_redox_cross.sh` type-checks x86_64 and aarch64 Redox with `-Z build-std`. No Redox host/Orbital session is available here, so no window has been shown and no event has been read at runtime. |
| Dynamic hide/show | 🔶 Create-time only | `orbclient` 0.4 exposes no show/hide command; the state flag is recorded but a created window stays mapped. |
