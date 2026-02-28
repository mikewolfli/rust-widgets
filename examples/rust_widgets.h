#ifndef RUST_WIDGETS_H
#define RUST_WIDGETS_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void rust_widgets_init(void);
void rust_widgets_run(void);
void rust_widgets_quit(void);

uint64_t rust_widgets_create_window(const char* title, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_button(uint64_t parent, const char* text, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_checkbox(uint64_t parent, const char* text, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_line_edit(uint64_t parent, const char* text, int x, int y, unsigned int width, unsigned int height);

uint64_t rust_widgets_create_menu_bar(uint64_t parent, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_menu(uint64_t parent, const char* text, int x, int y, unsigned int width, unsigned int height);
bool rust_widgets_attach_menu_bar_to_window(uint64_t window, uint64_t menu_bar);
uint64_t rust_widgets_menu_add_item(uint64_t parent_menu, const char* text, const char* shortcut);

uint64_t rust_widgets_poll_menu_triggered(void);
uint64_t rust_widgets_poll_widget_triggered(void);

/*
 * Returns trigger kind:
 * 0 = none
 * 1 = clicked
 * 2 = value-changed
 * If return value != 0 and widget_id_out is non-null, widget id is written to *widget_id_out.
 */
unsigned int rust_widgets_poll_widget_trigger_event(uint64_t* widget_id_out);
bool rust_widgets_inject_menu_trigger(uint64_t menu_item_id);
bool rust_widgets_inject_widget_trigger_event(uint64_t widget_id, unsigned int kind_code);

void rust_widgets_show_widget(uint64_t widget_id);
void rust_widgets_hide_widget(uint64_t widget_id);
void rust_widgets_set_widget_text(uint64_t widget_id, const char* text);
const char* rust_widgets_get_widget_text(uint64_t widget_id);
void rust_widgets_free_string(const char* ptr);

#ifdef __cplusplus
}
#endif

#endif /* RUST_WIDGETS_H */
