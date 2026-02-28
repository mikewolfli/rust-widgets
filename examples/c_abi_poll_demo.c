#include <stdint.h>
#include <stdio.h>
#include <time.h>

#include "rust_widgets.h"

static void sleep_ms(int ms) {
    struct timespec req;
    req.tv_sec = ms / 1000;
    req.tv_nsec = (long)(ms % 1000) * 1000000L;
    nanosleep(&req, NULL);
}

int main(void) {
    rust_widgets_init();

    uint64_t window = rust_widgets_create_window("C ABI Poll Demo", 120, 120, 860, 560);
    uint64_t button = rust_widgets_create_button(window, "Click me", 24, 40, 140, 36);
    uint64_t line = rust_widgets_create_line_edit(window, "Type here", 24, 92, 280, 34);
    uint64_t checkbox = rust_widgets_create_checkbox(window, "Enable option", 24, 140, 180, 30);

    uint64_t menu_bar = rust_widgets_create_menu_bar(window, 0, 0, 860, 28);
    rust_widgets_attach_menu_bar_to_window(window, menu_bar);
    uint64_t file_menu = rust_widgets_create_menu(menu_bar, "File", 0, 0, 0, 0);
    uint64_t quit_item = rust_widgets_menu_add_item(file_menu, "Quit", "cmd+q");

    rust_widgets_show_widget(window);

    printf("Controls: button=%llu, line=%llu, checkbox=%llu\n",
           (unsigned long long)button,
           (unsigned long long)line,
           (unsigned long long)checkbox);

    int ticks = 0;
    for (;;) {
        if (ticks == 60) {
            rust_widgets_inject_widget_trigger_event(button, 1);
            rust_widgets_inject_widget_trigger_event(line, 2);
            rust_widgets_inject_menu_trigger(quit_item);
        }

        uint64_t menu_item = rust_widgets_poll_menu_triggered();
        if (menu_item != 0) {
            printf("menu triggered: %llu\n", (unsigned long long)menu_item);
            if (menu_item == quit_item) {
                rust_widgets_quit();
                break;
            }
        }

        uint64_t widget_id = 0;
        unsigned int kind = rust_widgets_poll_widget_trigger_event(&widget_id);
        if (kind != 0) {
            const char* kind_name = "unknown";
            if (kind == 1) {
                kind_name = "clicked";
            } else if (kind == 2) {
                kind_name = "value-changed";
            }
            printf("widget triggered: id=%llu, kind=%s\n", (unsigned long long)widget_id, kind_name);
        }

        sleep_ms(16);
        ticks++;
    }

    rust_widgets_run();
    return 0;
}
