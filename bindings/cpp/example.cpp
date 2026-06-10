/**
 * example.cpp  —  Example usage of the C++ binding for rust_widgets.
 *
 * Compile with (from repo root):
 *   g++ -std=c++17 -I include -I examples \
 *       -o cpp_example bindings/cpp/example.cpp \
 *       -L target/release -l rust_widgets
 *
 * Run:
 *   LD_LIBRARY_PATH=target/release ./cpp_example
 */

#include "rust_widgets.hpp"
#include <iostream>
#include <cstdlib>

int main() {
    // ── Initialise the GUI backend ────────────────────────────────────
    rust_widgets::init();

    // ── Create a window ───────────────────────────────────────────────
    auto win = rust_widgets::Window::create("C++ Example", 100, 100, 800, 600);
    if (!win) {
        std::cerr << "Failed to create window.\n";
        return 1;
    }
    win.show();

    // ── Create widgets inside the window ──────────────────────────────
    auto button = rust_widgets::Button::create(win.id(), "Click Me", 50, 50, 120, 32);
    button.show();

    auto checkbox = rust_widgets::Checkbox::create(win.id(), "Enable feature", 50, 100, 200, 32);
    checkbox.show();

    auto label = rust_widgets::Label::create(win.id(), "Hello from C++!", 50, 150, 300, 24);
    label.show();

    auto combo = rust_widgets::ComboBox::create(win.id(), 50, 200, 200, 28);
    combo.add_item("Option A");
    combo.add_item("Option B");
    combo.add_item("Option C");
    combo.set_current_index(0);
    combo.show();

    auto list = rust_widgets::ListBox::create(win.id(), 50, 250, 200, 100);
    list.add_item("Item 1");
    list.add_item("Item 2");
    list.add_item("Item 3");
    list.show();

    auto slider = rust_widgets::Slider::create(win.id(), 300, 50, 200, 24);
    slider.show();

    auto progress = rust_widgets::ProgressBar::create(win.id(), 300, 100, 200, 24);
    progress.show();

    // ── Menu bar ──────────────────────────────────────────────────────
    auto menubar = rust_widgets::MenuBar::create(win.id(), 0, 0, 800, 24);
    menubar.show();
    rust_widgets::attach_menu_bar_to_window(win, menubar);

    auto file_menu = rust_widgets::Menu::create(menubar.id(), "File", 0, 0, 80, 24);
    file_menu.show();
    file_menu.add_item("Open", "Ctrl+O");
    file_menu.add_item("Save", "Ctrl+S");
    file_menu.add_item("Quit", "Ctrl+Q");

    auto help_menu = rust_widgets::Menu::create(menubar.id(), "Help", 80, 0, 80, 24);
    help_menu.show();
    help_menu.add_item("About");

    // ── Print platform info ───────────────────────────────────────────
    std::cout << "Backend:            " << rust_widgets::backend_name().str() << "\n";
    std::cout << "Bindings API ver:   " << rust_widgets::bindings_api_version() << "\n";
    std::cout << "DPI scale factor:   " << rust_widgets::platform_dpi_scale_factor() << "\n";

    auto caps = rust_widgets::platform_capabilities();
    std::cout << "Capabilities:       " << caps << " (0x" << std::hex << caps << std::dec << ")\n";
    if (caps & rust_widgets::cap::DpiScaling)
        std::cout << "  - DPI scaling supported\n";
    if (caps & rust_widgets::cap::Ime)
        std::cout << "  - IME supported\n";
    if (caps & rust_widgets::cap::Accessibility)
        std::cout << "  - Accessibility supported\n";
    if (caps & rust_widgets::cap::NativeMenu)
        std::cout << "  - Native menus supported\n";
    if (caps & rust_widgets::cap::TypedWidgetTrigger)
        std::cout << "  - Typed widget triggers supported\n";

    // ── Event loop ────────────────────────────────────────────────────
    std::cout << "\nEntering event loop...\n";
    bool running = true;

    while (running) {
        rust_widgets::run();  // blocks until quit is requested

        // Check for widget triggers after run returns
        uint64_t triggered = rust_widgets::poll_widget_triggered();
        if (triggered != 0) {
            if (triggered == button.id()) {
                std::cout << "Button clicked!\n";
            } else {
                std::cout << "Widget triggered: id=" << triggered << "\n";
            }
        }

        // Poll for typed events
        uint64_t event_widget = 0;
        auto kind = rust_widgets::poll_widget_trigger_event(&event_widget);
        if (kind != rust_widgets::TriggerKind::None) {
            std::cout << "Typed event: widget=" << event_widget
                      << " kind=" << static_cast<unsigned int>(kind) << "\n";

            if (event_widget == checkbox.id() &&
                kind == rust_widgets::TriggerKind::Clicked) {
                bool checked = checkbox.is_enabled();
                std::cout << "Checkbox toggled: " << (checked ? "checked" : "unchecked") << "\n";
            }
        }

        // Poll menu triggers
        uint64_t menu_item = rust_widgets::poll_menu_triggered();
        if (menu_item != 0) {
            std::cout << "Menu item triggered: id=" << menu_item << "\n";
            // Quit if the user somehow triggers quit
        }

        // For this example we break after processing one event.
        // In a real application you'd loop until quit is called.
        break;
    }

    std::cout << "Exiting.\n";
    rust_widgets::quit();
    return 0;
}
