#ifndef RUST_WIDGETS_HPP
#define RUST_WIDGETS_HPP

/**
 * rust_widgets.hpp  —  C++17 header‑only binding for the rust‑widgets GUI library.
 *
 * Include this file after setting up your include path so that the C headers
 *   <rw_generated.h>   and   <rw_errors.h>
 * are reachable (they ship under <repo>/include/ ).
 *
 * All symbols live in namespace `rust_widgets`.
 *
 * Usage:
 *   rust_widgets::init();
 *   auto win = rust_widgets::Window::create("Hello", 100, 100, 640, 480);
 *   win.show();
 *   rust_widgets::run();
 *
 * ---------------------------------------------------------------------------
 * Compiled with: g++ -std=c++17 -I<repo>/include ...
 */

#include <cstdint>
#include <string>
#include <optional>
#include <tuple>

#include <rw_generated.h>
#include <rw_errors.h>

// ==========================================================================
//  Forward declarations for C ABI functions that exist in the Rust library
//  but may not yet appear in the auto‑generated C header.
// ==========================================================================

#ifdef __cplusplus
extern "C" {
#endif

/* Widget geometry */
void   rw_set_widget_geometry(uint64_t widget_id, int x, int y,
                                        unsigned int width, unsigned int height);
bool   rw_get_widget_geometry(uint64_t widget_id,
                                        int* x_out, int* y_out,
                                        unsigned int* width_out,
                                        unsigned int* height_out);

/* Dialog widgets */
uint64_t rw_create_message_box(uint64_t parent, const char* title,
                                          const char* text, int x, int y,
                                          unsigned int width, unsigned int height);
uint64_t rw_create_file_dialog(uint64_t parent, const char* title,
                                          int x, int y,
                                          unsigned int width, unsigned int height);
uint64_t rw_create_color_dialog(uint64_t parent, const char* title,
                                           int x, int y,
                                           unsigned int width, unsigned int height);
uint64_t rw_create_font_dialog(uint64_t parent, const char* title,
                                          int x, int y,
                                          unsigned int width, unsigned int height);

/* SpinBox, ListView, ScrollArea */
uint64_t rw_create_spin_box(uint64_t parent, int x, int y,
                                       unsigned int width, unsigned int height);
uint64_t rw_create_list_view(uint64_t parent, int x, int y,
                                        unsigned int width, unsigned int height);
uint64_t rw_create_scroll_area(uint64_t parent, int x, int y,
                                          unsigned int width, unsigned int height);

/* ComboBox operations */
bool        rw_combo_box_add_item(uint64_t combo_box, const char* text);
bool        rw_combo_box_clear_items(uint64_t combo_box);
bool        rw_combo_box_set_current_index(uint64_t combo_box,
                                                      unsigned int index);
int         rw_combo_box_current_index(uint64_t combo_box);
unsigned int rw_combo_box_item_count(uint64_t combo_box);
const char* rw_combo_box_item_text(uint64_t combo_box, unsigned int index);

/* ListBox operations */
bool        rw_list_box_add_item(uint64_t list_box, const char* text);
bool        rw_list_box_remove_item(uint64_t list_box, unsigned int index);
bool        rw_list_box_clear_items(uint64_t list_box);
bool        rw_list_box_set_current_index(uint64_t list_box,
                                                     unsigned int index);
int         rw_list_box_current_index(uint64_t list_box);
unsigned int rw_list_box_item_count(uint64_t list_box);
const char* rw_list_box_item_text(uint64_t list_box, unsigned int index);

/* Clipboard */
bool        rw_set_clipboard_text(const char* text);
const char* rw_get_clipboard_text(void);

/* Drag‑and‑drop */
bool rw_begin_drag(uint64_t source, const char* mime_type,
                          const uint8_t* payload, unsigned int payload_len);

/* Binding status & reserved queries */
unsigned int rw_python_binding_status(void);
unsigned int rw_python_reserved(void);
unsigned int rw_cpp_reserved(void);
unsigned int rw_java_binding_status(void);
unsigned int rw_java_jni_skeleton_version(void);
unsigned int rw_java_reserved(void);
unsigned int rw_nodejs_binding_status(void);

/* Platform capability contract */
unsigned int rw_platform_capability_contract(unsigned int profile_code);

/* Advanced drag‑and‑drop (drop‑event polling) */
bool rw_poll_drop_event(uint64_t* source_out, uint64_t* target_out,
                               char** mime_out, uint8_t** payload_out,
                               unsigned int* payload_len_out);

/* Mobile */
const char* rw_mobile_backend_name(void);
bool        rw_mobile_attach_native_view(uint64_t native_handle);

/* Harmony node bridge */
bool     rw_harmony_bind_node(uint64_t node_handle, uint64_t widget_id);
bool     rw_harmony_unbind_node(uint64_t node_handle);
uint64_t rw_harmony_lookup_widget_id(uint64_t node_handle);
void     rw_harmony_clear_node_bindings(void);
bool     rw_harmony_on_click(uint64_t widget_id);
bool     rw_harmony_on_menu_item(uint64_t menu_item_id);
bool     rw_harmony_on_value_changed(uint64_t widget_id);
bool     rw_harmony_on_widget_event(uint64_t widget_id,
                                           unsigned int kind_code);
bool     rw_harmony_on_node_click(uint64_t node_handle);
bool     rw_harmony_on_node_menu_item(uint64_t node_handle);
bool     rw_harmony_on_node_value_changed(uint64_t node_handle);
bool     rw_harmony_on_node_widget_event(uint64_t node_handle,
                                                unsigned int kind_code);

/* Render configuration & embedded engine */
unsigned int rw_set_render_aa_samples_per_axis(unsigned int samples);
unsigned int rw_get_render_aa_samples_per_axis(void);
unsigned int rw_set_embedded_target_fps(unsigned int fps);
unsigned int rw_get_embedded_target_fps(void);
uint64_t     rw_submit_embedded_noop_task(const char* label);
bool         rw_embedded_engine_is_initialized(void);
bool         rw_embedded_engine_is_running(void);
uint64_t     rw_embedded_engine_frame_count(void);
uint64_t     rw_embedded_engine_pending_task_count(void);
uint64_t     rw_embedded_engine_window_count(void);
uint64_t     rw_embedded_engine_button_count(void);

/* Memory */
void rw_free_rust_string(char* s);

#ifdef __cplusplus
}  // extern "C"
#endif

// ==========================================================================
//  N A M E S P A C E    r u s t _ w i d g e t s
// ==========================================================================
namespace rust_widgets {

// ==========================================================================
//  E n u m s
// ==========================================================================

/// Trigger kind codes returned by poll_widget_trigger_event().
enum class TriggerKind : unsigned int {
    None             = 0,
    Clicked          = 1,
    ValueChanged     = 2,
    SelectionChanged = 3,
    Closed           = 4,
};

/// Bit flags returned by platform_capabilities().
namespace cap {
inline constexpr unsigned int DpiScaling       = 1u << 0;
inline constexpr unsigned int Ime              = 1u << 1;
inline constexpr unsigned int Accessibility    = 1u << 2;
inline constexpr unsigned int NativeMenu      = 1u << 3;
inline constexpr unsigned int TypedWidgetTrigger = 1u << 4;
} // namespace cap

// ==========================================================================
//  R u s t S t r i n g    ( R A I I )
// ==========================================================================

/**
 * Owning wrapper for a `const char*` returned by any rust_widgets_* function.
 * The destructor calls `rw_free_string()` automatically.
 *
 * Move‑only (copy would cause double‑free).
 */
class RustString {
public:
    /// Construct from a raw C string returned by the C API.
    explicit RustString(const char* ptr) noexcept
        : ptr_(ptr)
    {}

    RustString() noexcept
        : ptr_(nullptr)
    {}

    ~RustString() noexcept {
        release();
    }

    RustString(const RustString&) = delete;
    RustString& operator=(const RustString&) = delete;

    RustString(RustString&& other) noexcept
        : ptr_(other.ptr_)
    {
        other.ptr_ = nullptr;
    }

    RustString& operator=(RustString&& other) noexcept {
        if (this != &other) {
            release();
            ptr_ = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }

    /// Access the raw C string (may be nullptr).
    const char* c_str() const noexcept { return ptr_ ? ptr_ : ""; }

    /// Convert to std::string.
    std::string str() const {
        return ptr_ ? std::string(ptr_) : std::string();
    }

    /// Implicit conversion to std::string.
    operator std::string() const { return str(); }

    explicit operator bool() const noexcept { return ptr_ != nullptr; }

private:
    void release() noexcept {
        if (ptr_) {
            rw_free_string(const_cast<char*>(ptr_));
            ptr_ = nullptr;
        }
    }
    const char* ptr_ = nullptr;
};

// ==========================================================================
//  W i d g e t    ( b a s e   c l a s s )
// ==========================================================================

/**
 * Lightweight handle wrapper around a uint64_t widget ID.
 *
 * Widgets are owned by the Rust runtime, so `Widget` objects are **copyable**
 * value types – there is no double‑free risk.
 */
class Widget {
public:
    /// Construct an invalid (null) widget.
    Widget() noexcept : id_(0) {}

    /// Construct from a raw uint64_t handle.
    explicit Widget(uint64_t id) noexcept : id_(id) {}

    /// The raw handle (0 means invalid/not found).
    uint64_t id() const noexcept { return id_; }

    /// True when the handle is non‑zero.
    bool valid() const noexcept { return id_ != 0; }
    explicit operator bool() const noexcept { return valid(); }

    bool operator==(const Widget& other) const noexcept { return id_ == other.id_; }
    bool operator!=(const Widget& other) const noexcept { return id_ != other.id_; }

    // ----  Widget manipulation  --------------------------------------------

    void show() const { rw_show_widget(id_); }
    void hide() const { rw_hide_widget(id_); }

    void set_text(const std::string& text) const {
        rw_set_widget_text(id_, text.c_str());
    }

    RustString text() const {
        return RustString(rw_get_widget_text(id_));
    }

    void set_enabled(bool enabled) const {
        rw_set_widget_enabled(id_, enabled);
    }

    bool is_enabled() const {
        return rw_is_widget_enabled(id_);
    }

    void set_visible(bool visible) const {
        rw_set_widget_visible(id_, visible);
    }

    bool is_visible() const {
        return rw_is_widget_visible(id_);
    }

    void set_geometry(int x, int y, unsigned int width, unsigned int height) const {
        rw_set_widget_geometry(id_, x, y, width, height);
    }

    /// Returns (x, y, width, height) if the geometry could be read.
    std::optional<std::tuple<int, int, unsigned int, unsigned int>> geometry() const {
        int x = 0, y = 0;
        unsigned int w = 0, h = 0;
        if (rw_get_widget_geometry(id_, &x, &y, &w, &h)) {
            return std::make_tuple(x, y, w, h);
        }
        return std::nullopt;
    }

    bool set_ime_enabled(bool enabled) const {
        return rw_set_widget_ime_enabled(id_, enabled);
    }

    bool is_ime_enabled() const {
        return rw_is_widget_ime_enabled(id_);
    }

    bool set_accessibility_name(const std::string& name) const {
        return rw_set_widget_accessibility_name(id_, name.c_str());
    }

    RustString accessibility_name() const {
        return RustString(rw_get_widget_accessibility_name(id_));
    }

protected:
    uint64_t id_ = 0;
};

// ==========================================================================
//  W i d g e t   s u b c l a s s e s
// ==========================================================================

class Window final : public Widget {
public:
    using Widget::Widget;

    /// Create a new window. Returns an invalid Widget on failure.
    static Window create(
        const std::string& title,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return Window(rw_create_window(title.c_str(), x, y, width, height));
    }
};

class Button final : public Widget {
public:
    using Widget::Widget;

    static Button create(
        uint64_t parent,
        const std::string& text,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return Button(
            rw_create_button(parent, text.c_str(), x, y, width, height));
    }
};

class Checkbox final : public Widget {
public:
    using Widget::Widget;

    static Checkbox create(
        uint64_t parent,
        const std::string& text,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return Checkbox(
            rw_create_checkbox(parent, text.c_str(), x, y, width, height));
    }
};

class LineEdit final : public Widget {
public:
    using Widget::Widget;

    static LineEdit create(
        uint64_t parent,
        const std::string& text,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return LineEdit(
            rw_create_line_edit(parent, text.c_str(), x, y, width, height));
    }

    // Convenience: get/set the line edit text via widget text API
};

class Label final : public Widget {
public:
    using Widget::Widget;

    static Label create(
        uint64_t parent,
        const std::string& text,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return Label(
            rw_create_label(parent, text.c_str(), x, y, width, height));
    }
};

class RadioButton final : public Widget {
public:
    using Widget::Widget;

    static RadioButton create(
        uint64_t parent,
        const std::string& text,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return RadioButton(
            rw_create_radio_button(parent, text.c_str(), x, y, width, height));
    }
};

class Slider final : public Widget {
public:
    using Widget::Widget;

    static Slider create(
        uint64_t parent,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return Slider(rw_create_slider(parent, x, y, width, height));
    }
};

class ProgressBar final : public Widget {
public:
    using Widget::Widget;

    static ProgressBar create(
        uint64_t parent,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return ProgressBar(
            rw_create_progress_bar(parent, x, y, width, height));
    }
};

class Panel final : public Widget {
public:
    using Widget::Widget;

    static Panel create(
        uint64_t parent,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return Panel(rw_create_panel(parent, x, y, width, height));
    }
};

class MessageBox final : public Widget {
public:
    using Widget::Widget;

    static MessageBox create(
        uint64_t parent,
        const std::string& title,
        const std::string& text,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return MessageBox(
            rw_create_message_box(parent, title.c_str(), text.c_str(),
                                            x, y, width, height));
    }
};

class FileDialog final : public Widget {
public:
    using Widget::Widget;

    static FileDialog create(
        uint64_t parent,
        const std::string& title,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return FileDialog(
            rw_create_file_dialog(parent, title.c_str(), x, y, width, height));
    }
};

class ColorDialog final : public Widget {
public:
    using Widget::Widget;

    static ColorDialog create(
        uint64_t parent,
        const std::string& title,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return ColorDialog(
            rw_create_color_dialog(parent, title.c_str(), x, y, width, height));
    }
};

class FontDialog final : public Widget {
public:
    using Widget::Widget;

    static FontDialog create(
        uint64_t parent,
        const std::string& title,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return FontDialog(
            rw_create_font_dialog(parent, title.c_str(), x, y, width, height));
    }
};

class SpinBox final : public Widget {
public:
    using Widget::Widget;

    static SpinBox create(
        uint64_t parent,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return SpinBox(rw_create_spin_box(parent, x, y, width, height));
    }
};

class ListView final : public Widget {
public:
    using Widget::Widget;

    static ListView create(
        uint64_t parent,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return ListView(rw_create_list_view(parent, x, y, width, height));
    }
};

class ScrollArea final : public Widget {
public:
    using Widget::Widget;

    static ScrollArea create(
        uint64_t parent,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return ScrollArea(rw_create_scroll_area(parent, x, y, width, height));
    }
};

// ==========================================================================
//  M e n u   c l a s s e s
// ==========================================================================

class MenuBar final : public Widget {
public:
    using Widget::Widget;

    static MenuBar create(
        uint64_t parent,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return MenuBar(rw_create_menu_bar(parent, x, y, width, height));
    }
};

class Menu final : public Widget {
public:
    using Widget::Widget;

    static Menu create(
        uint64_t parent,
        const std::string& text,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return Menu(rw_create_menu(parent, text.c_str(), x, y, width, height));
    }

    /// Add an item to this menu. Returns the menu item widget ID.
    uint64_t add_item(const std::string& text, const std::string& shortcut = "") const {
        return rw_menu_add_item(id_, text.c_str(), shortcut.c_str());
    }
};

class ToolBar final : public Widget {
public:
    using Widget::Widget;

    static ToolBar create(
        uint64_t parent,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return ToolBar(rw_create_tool_bar(parent, x, y, width, height));
    }
};

class StatusBar final : public Widget {
public:
    using Widget::Widget;

    static StatusBar create(
        uint64_t parent,
        const std::string& text,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return StatusBar(
            rw_create_status_bar(parent, text.c_str(), x, y, width, height));
    }
};

// ==========================================================================
//  C o m b o B o x   ( w i t h   s p e c i f i c   A P I )
// ==========================================================================

class ComboBox final : public Widget {
public:
    using Widget::Widget;

    static ComboBox create(
        uint64_t parent,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return ComboBox(rw_create_combo_box(parent, x, y, width, height));
    }

    bool add_item(const std::string& text) const {
        return rw_combo_box_add_item(id_, text.c_str());
    }

    bool clear_items() const {
        return rw_combo_box_clear_items(id_);
    }

    bool set_current_index(unsigned int index) const {
        return rw_combo_box_set_current_index(id_, index);
    }

    /// Returns the current index, or -1 if nothing is selected.
    int current_index() const {
        return rw_combo_box_current_index(id_);
    }

    unsigned int item_count() const {
        return rw_combo_box_item_count(id_);
    }

    RustString item_text(unsigned int index) const {
        return RustString(rw_combo_box_item_text(id_, index));
    }
};

// ==========================================================================
//  L i s t B o x   ( w i t h   s p e c i f i c   A P I )
// ==========================================================================

class ListBox final : public Widget {
public:
    using Widget::Widget;

    static ListBox create(
        uint64_t parent,
        int x, int y,
        unsigned int width, unsigned int height
    ) {
        return ListBox(rw_create_list_box(parent, x, y, width, height));
    }

    bool add_item(const std::string& text) const {
        return rw_list_box_add_item(id_, text.c_str());
    }

    bool remove_item(unsigned int index) const {
        return rw_list_box_remove_item(id_, index);
    }

    bool clear_items() const {
        return rw_list_box_clear_items(id_);
    }

    bool set_current_index(unsigned int index) const {
        return rw_list_box_set_current_index(id_, index);
    }

    /// Returns the current index, or -1 if nothing is selected.
    int current_index() const {
        return rw_list_box_current_index(id_);
    }

    unsigned int item_count() const {
        return rw_list_box_item_count(id_);
    }

    RustString item_text(unsigned int index) const {
        return RustString(rw_list_box_item_text(id_, index));
    }
};

// ==========================================================================
//  F r e e   f u n c t i o n s
// ==========================================================================

// ----  Core lifecycle  ----------------------------------------------------

/// Initialise the GUI backend.
inline void init() {
    rw_init();
}

/// Enter the GUI event loop (blocks until quit() is called).
inline void run() {
    rw_run();
}

/// Signal the event loop to exit.
inline void quit() {
    rw_quit();
}

// ----  Menu  --------------------------------------------------------------

/// Attach a menu bar to a window.
inline bool attach_menu_bar_to_window(uint64_t window, uint64_t menu_bar) {
    return rw_attach_menu_bar_to_window(window, menu_bar);
}

inline bool attach_menu_bar_to_window(const Widget& window, const Widget& menu_bar) {
    return rw_attach_menu_bar_to_window(window.id(), menu_bar.id());
}

/// Poll for a triggered menu item. Returns 0 if none.
inline uint64_t poll_menu_triggered() {
    return rw_poll_menu_triggered();
}

/// Inject a menu trigger programmatically.
inline bool inject_menu_trigger(uint64_t menu_item_id) {
    return rw_inject_menu_trigger(menu_item_id);
}

// ----  Widget events  -----------------------------------------------------

/// Poll for a widget trigger event. Returns the widget ID that was triggered,
/// or 0 if no events are pending.
inline uint64_t poll_widget_triggered() {
    return rw_poll_widget_triggered();
}

/**
 * Poll for a typed widget trigger event.
 *
 * Returns the trigger kind, and writes the widget ID into `widget_id_out`
 * (which may be nullptr if only the kind is needed).
 */
inline TriggerKind poll_widget_trigger_event(uint64_t* widget_id_out = nullptr) {
    return static_cast<TriggerKind>(
        rw_poll_widget_trigger_event(widget_id_out));
}

/// Convenience: poll with a Widget reference.
inline TriggerKind poll_widget_trigger_event(Widget& out) {
    uint64_t id = 0;
    auto kind = rw_poll_widget_trigger_event(&id);
    out = Widget(id);
    return static_cast<TriggerKind>(kind);
}

/// Inject a synthetic widget trigger event.
inline bool inject_widget_trigger_event(uint64_t widget_id, TriggerKind kind) {
    return rw_inject_widget_trigger_event(
        widget_id, static_cast<unsigned int>(kind));
}

inline bool inject_widget_trigger_event(const Widget& widget, TriggerKind kind) {
    return rw_inject_widget_trigger_event(
        widget.id(), static_cast<unsigned int>(kind));
}

// ----  Clipboard  ---------------------------------------------------------

inline bool set_clipboard_text(const std::string& text) {
    return rw_set_clipboard_text(text.c_str());
}

inline RustString clipboard_text() {
    return RustString(rw_get_clipboard_text());
}

/// Shorthand: use `clipboard_text()` and then `.str()` to get a std::string.
inline std::string get_clipboard_text() {
    return clipboard_text().str();
}

// ----  Drag‑and‑drop  -----------------------------------------------------

inline bool begin_drag(
    uint64_t source,
    const std::string& mime_type,
    const uint8_t* payload,
    unsigned int payload_len
) {
    return rw_begin_drag(source, mime_type.c_str(), payload, payload_len);
}

inline bool begin_drag(
    const Widget& source,
    const std::string& mime_type,
    const uint8_t* payload,
    unsigned int payload_len
) {
    return rw_begin_drag(source.id(), mime_type.c_str(), payload, payload_len);
}

// ----  Platform  ----------------------------------------------------------

inline RustString backend_name() {
    return RustString(rw_backend_name());
}

inline unsigned int platform_capabilities() {
    return rw_platform_capabilities();
}

inline float platform_dpi_scale_factor() {
    return rw_platform_dpi_scale_factor();
}

inline unsigned int bindings_api_version() {
    return rw_bindings_api_version();
}

inline unsigned int cpp_binding_status() {
    return rw_cpp_binding_status();
}

// ----  Error helpers  -----------------------------------------------------

/// Convert an error handle to a human‑readable string (must be freed).
inline RustString error_message(uint64_t handle) {
    return RustString(rw_error_message(handle));
}

inline int32_t error_code(uint64_t handle) {
    return rw_error_code(handle);
}

/// Error category constants (from rw_errors.h).
namespace error {
inline constexpr int32_t SUCCESS              = RW_ERROR_SUCCESS;
inline constexpr int32_t NOT_IMPLEMENTED      = RW_ERROR_NOT_IMPLEMENTED;
inline constexpr int32_t UNSUPPORTED_OPERATION = RW_ERROR_UNSUPPORTED_OPERATION;
inline constexpr int32_t INVALID_ARGUMENT     = RW_ERROR_INVALID_ARGUMENT;
inline constexpr int32_t NULL_POINTER         = RW_ERROR_NULL_POINTER;
inline constexpr int32_t OUT_OF_MEMORY        = RW_ERROR_OUT_OF_MEMORY;
inline constexpr int32_t LOCK_POISONED        = RW_ERROR_LOCK_POISONED;
inline constexpr int32_t WIDGET_NOT_FOUND     = RW_ERROR_WIDGET_NOT_FOUND;
inline constexpr int32_t WIDGET_INVALID_STATE = RW_ERROR_WIDGET_INVALID_STATE;
inline constexpr int32_t PLATFORM_UNSUPPORTED = RW_ERROR_PLATFORM_UNSUPPORTED;
inline constexpr int32_t PLATFORM_INIT_FAILED = RW_ERROR_PLATFORM_INIT_FAILED;
inline constexpr int32_t CLIPBOARD_FAILED     = RW_ERROR_CLIPBOARD_FAILED;
inline constexpr int32_t DRAG_DROP_FAILED     = RW_ERROR_DRAG_DROP_FAILED;
inline constexpr int32_t FILE_NOT_FOUND       = RW_ERROR_FILE_NOT_FOUND;
} // namespace error

} // namespace rust_widgets

#endif // RUST_WIDGETS_HPP
