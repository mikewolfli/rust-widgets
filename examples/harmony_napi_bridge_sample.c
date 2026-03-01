#include <stdint.h>
#include <stdbool.h>

#include "rust_widgets.h"

/*
 * Harmony ArkUI/NAPI bridge sample (C side sketch)
 *
 * Replace these callback signatures with your actual ArkUI/NAPI runtime types.
 * The key idea is:
 *   1) Create rust_widgets controls and receive widget_id.
 *   2) Bind ArkUI node handle -> widget_id once.
 *   3) In native callbacks, call rust_widgets_harmony_on_node_* APIs.
 */

static uint64_t g_window = 0;
static uint64_t g_button = 0;
static uint64_t g_input = 0;
static uint64_t g_menu_item_quit = 0;

void app_init_ui(void) {
    /* Initialize the rust_widgets runtime. */
    rust_widgets_init();

    /* Build the demo window and controls. */
    g_window = rust_widgets_create_window("Harmony Bridge Sample", 80, 80, 720, 480);
    g_button = rust_widgets_create_button(g_window, "Tap", 20, 40, 120, 36);
    g_input = rust_widgets_create_line_edit(g_window, "", 20, 90, 240, 36);

    /* Build the menu hierarchy with a Quit action. */
    uint64_t menu_bar = rust_widgets_create_menu_bar(g_window, 0, 0, 720, 28);
    rust_widgets_attach_menu_bar_to_window(g_window, menu_bar);
    uint64_t file_menu = rust_widgets_create_menu(menu_bar, "File", 0, 0, 0, 0);
    g_menu_item_quit = rust_widgets_menu_add_item(file_menu, "Quit", "cmd+q");

    /* Show the window after building the UI tree. */
    rust_widgets_show_widget(g_window);
}

/* Called once when ArkUI node is created and associated with a rust_widgets widget id. */
void arkui_on_node_ready(uint64_t node_handle, uint64_t widget_id) {
    rust_widgets_harmony_bind_node(node_handle, widget_id);
}

/* Called when ArkUI node is destroyed. */
void arkui_on_node_dispose(uint64_t node_handle) {
    rust_widgets_harmony_unbind_node(node_handle);
}

/* Button click callback from ArkUI/NAPI. */
void arkui_on_button_click(uint64_t node_handle) {
    rust_widgets_harmony_on_node_click(node_handle);
}

/* Input text changed callback from ArkUI/NAPI. */
void arkui_on_input_changed(uint64_t node_handle) {
    rust_widgets_harmony_on_node_value_changed(node_handle);
}

/* Menu callback from ArkUI/NAPI using direct widget id path. */
void arkui_on_menu_quit(void) {
    rust_widgets_harmony_on_menu_item(g_menu_item_quit);
}

/*
 * Poll bridge events and route to app logic.
 * Run this on your app update tick / dispatcher thread.
 */
void app_pump_events(void) {
    /* Poll and dispatch menu trigger events. */
    uint64_t menu_item = rust_widgets_poll_menu_triggered();
    if (menu_item == g_menu_item_quit) {
        rust_widgets_quit();
        return;
    }

    /* Poll and dispatch typed widget trigger events. */
    uint64_t widget_id = 0;
    unsigned int kind = rust_widgets_poll_widget_trigger_event(&widget_id);
    if (kind == 1 && widget_id == g_button) {
        /* button clicked */
    } else if (kind == 2 && widget_id == g_input) {
        /* value changed */
    }
}
