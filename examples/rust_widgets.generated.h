#ifndef RUST_WIDGETS_GENERATED_H
#define RUST_WIDGETS_GENERATED_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Auto-generated from src/bindings/binding_impl.rs */
bool rust_widgets_attach_menu_bar_to_window(uint64_t window, uint64_t menu_bar);
const char* rust_widgets_backend_name(void);
unsigned int rust_widgets_bindings_api_version(void);
unsigned int rust_widgets_cpp_binding_status(void);
unsigned int rust_widgets_cpp_reserved(void);
uint64_t rust_widgets_create_button(uint64_t parent, const char* text, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_checkbox(uint64_t parent, const char* text, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_combo_box(uint64_t parent, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_label(uint64_t parent, const char* text, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_line_edit(uint64_t parent, const char* text, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_list_box(uint64_t parent, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_menu(uint64_t parent, const char* text, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_menu_bar(uint64_t parent, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_panel(uint64_t parent, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_progress_bar(uint64_t parent, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_radio_button(uint64_t parent, const char* text, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_slider(uint64_t parent, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_status_bar(uint64_t parent, const char* text, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_tool_bar(uint64_t parent, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_create_window(const char* title, int x, int y, unsigned int width, unsigned int height);
uint64_t rust_widgets_embedded_engine_button_count(void);
uint64_t rust_widgets_embedded_engine_frame_count(void);
bool rust_widgets_embedded_engine_is_initialized(void);
bool rust_widgets_embedded_engine_is_running(void);
uint64_t rust_widgets_embedded_engine_pending_task_count(void);
uint64_t rust_widgets_embedded_engine_window_count(void);
void rust_widgets_free_rust_string(char* s);
void rust_widgets_free_string(char* s);
unsigned int rust_widgets_get_embedded_target_fps(void);
unsigned int rust_widgets_get_render_aa_samples_per_axis(void);
const char* rust_widgets_get_widget_accessibility_name(uint64_t widget_id);
const char* rust_widgets_get_widget_text(uint64_t widget_id);
bool rust_widgets_harmony_bind_node(uint64_t node_handle, uint64_t widget_id);
void rust_widgets_harmony_clear_node_bindings(void);
uint64_t rust_widgets_harmony_lookup_widget_id(uint64_t node_handle);
bool rust_widgets_harmony_on_click(uint64_t widget_id);
bool rust_widgets_harmony_on_menu_item(uint64_t menu_item_id);
bool rust_widgets_harmony_on_node_click(uint64_t node_handle);
bool rust_widgets_harmony_on_node_menu_item(uint64_t node_handle);
bool rust_widgets_harmony_on_node_value_changed(uint64_t node_handle);
bool rust_widgets_harmony_on_node_widget_event(uint64_t node_handle, unsigned int kind_code);
bool rust_widgets_harmony_on_value_changed(uint64_t widget_id);
bool rust_widgets_harmony_on_widget_event(uint64_t widget_id, unsigned int kind_code);
bool rust_widgets_harmony_unbind_node(uint64_t node_handle);
void rust_widgets_hide_widget(uint64_t widget_id);
void rust_widgets_init(void);
bool rust_widgets_inject_menu_trigger(uint64_t menu_item_id);
bool rust_widgets_inject_widget_trigger_event(uint64_t widget_id, unsigned int kind_code);
bool rust_widgets_is_widget_enabled(uint64_t widget_id);
bool rust_widgets_is_widget_ime_enabled(uint64_t widget_id);
bool rust_widgets_is_widget_visible(uint64_t widget_id);
unsigned int rust_widgets_java_binding_status(void);
unsigned int rust_widgets_java_jni_skeleton_version(void);
unsigned int rust_widgets_java_reserved(void);
uint64_t rust_widgets_menu_add_item(uint64_t parent_menu, const char* text, const char* shortcut);
bool rust_widgets_mobile_attach_native_view(uint64_t native_handle);
const char* rust_widgets_mobile_backend_name(void);
unsigned int rust_widgets_platform_capabilities(void);
unsigned int rust_widgets_platform_capability_contract(unsigned int profile_code);
float rust_widgets_platform_dpi_scale_factor(void);
uint64_t rust_widgets_poll_menu_triggered(void);
unsigned int rust_widgets_poll_widget_trigger_event(uint64_t* widget_id_out);
uint64_t rust_widgets_poll_widget_triggered(void);
unsigned int rust_widgets_python_binding_status(void);
unsigned int rust_widgets_python_reserved(void);
void rust_widgets_quit(void);
void rust_widgets_run(void);
unsigned int rust_widgets_set_embedded_target_fps(unsigned int fps);
unsigned int rust_widgets_set_render_aa_samples_per_axis(unsigned int samples);
bool rust_widgets_set_widget_accessibility_name(uint64_t widget_id, const char* name);
void rust_widgets_set_widget_enabled(uint64_t widget_id, bool enabled);
bool rust_widgets_set_widget_ime_enabled(uint64_t widget_id, bool enabled);
void rust_widgets_set_widget_text(uint64_t widget_id, const char* text);
void rust_widgets_set_widget_visible(uint64_t widget_id, bool visible);
void rust_widgets_show_widget(uint64_t widget_id);
uint64_t rust_widgets_submit_embedded_noop_task(const char* label);

#ifdef __cplusplus
}
#endif

#endif /* RUST_WIDGETS_GENERATED_H */
