// Basic C++ demo that drives the rust_widgets C ABI wrapper.
#include "rust_widgets.hpp"

int main() {
    RustWidgets widgets;
    widgets.init();

    const auto window = widgets.createWindow("rust_widgets C++", 120, 120, 420, 260);
    const auto button = widgets.createButton(window, "Click", 24, 24, 120, 36);
    widgets.setWidgetText(button, "Ready");

    // Example only: in real apps call widgets.run().
    widgets.quit();
    return widgets.cppBindingStatus() == 0 ? 1 : 0;
}
