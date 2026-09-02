// C ABI polling demo for runtime events and capability checks.
#include <stdint.h>
#include <stdio.h>
#include <time.h>

#include "rw.h"

static void sleep_ms(int ms) {
    /* Sleep helper used by the polling loop. */
    struct timespec req;
    req.tv_sec = ms / 1000;
    req.tv_nsec = (long)(ms % 1000) * 1000000L;
    nanosleep(&req, NULL);
}

int main(void) {
    /* Initialize the rust_widgets runtime. */
    rw_init();

    /* Build the demo window and controls. */
    uint64_t window = rw_create_window("C ABI Poll Demo", 120, 120, 860, 560);
    uint64_t button = rw_create_button(window, "Click me", 24, 40, 140, 36);
    uint64_t line = rw_create_line_edit(window, "Type here", 24, 92, 280, 34);
    uint64_t checkbox = rw_create_checkbox(window, "Enable option", 24, 140, 180, 30);

    /* Build the menu hierarchy with a Quit action. */
    uint64_t menu_bar = rw_create_menu_bar(window, 0, 0, 860, 28);
    rw_attach_menu_bar_to_window(window, menu_bar);
    uint64_t file_menu = rw_create_menu(menu_bar, "File", 0, 0, 0, 0);
    uint64_t quit_item = rw_menu_add_item(file_menu, "Quit", "cmd+q");

    /* Show the window before polling events. */
    rw_show_widget(window);

    printf("Controls: button=%llu, line=%llu, checkbox=%llu\n",
           (unsigned long long)button,
           (unsigned long long)line,
           (unsigned long long)checkbox);

    /* Poll and dispatch both menu and widget trigger queues. */
    int ticks = 0;
    for (;;) {
        if (ticks == 60) {
            /* Inject synthetic events so the demo remains deterministic. */
            rw_inject_widget_trigger_event(button, 1);
            rw_inject_widget_trigger_event(line, 2);
            rw_inject_menu_trigger(quit_item);
        }

        /* Poll menu trigger queue and stop on Quit. */
        uint64_t menu_item = rw_poll_menu_triggered();
        if (menu_item != 0) {
            printf("menu triggered: %llu\n", (unsigned long long)menu_item);
            if (menu_item == quit_item) {
                rw_quit();
                break;
            }
        }

        /* Poll typed widget trigger queue (clicked/value-changed). */
        uint64_t widget_id = 0;
        unsigned int kind = rw_poll_widget_trigger_event(&widget_id);
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

    /* Run the platform loop and return when quit is requested. */
    rw_run();
    return 0;
}
