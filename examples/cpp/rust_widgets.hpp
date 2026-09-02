// Lightweight C++ wrapper for the rust_widgets C ABI.
#pragma once

#include <cstdint>
#include <string>
#include "../rw.h"

// Minimal C++ wrapper over the stable C ABI.
class RustWidgets final {
public:
    RustWidgets() = default;

    void init() const { rw_init(); }
    void run() const { rw_run(); }
    void quit() const { rw_quit(); }

    std::uint64_t createWindow(const std::string& title, int x, int y, unsigned int width, unsigned int height) const {
        return rw_create_window(title.c_str(), x, y, width, height);
    }

    std::uint64_t createButton(std::uint64_t parent, const std::string& text, int x, int y, unsigned int width, unsigned int height) const {
        return rw_create_button(parent, text.c_str(), x, y, width, height);
    }

    void setWidgetText(std::uint64_t widgetId, const std::string& text) const {
        rw_set_widget_text(widgetId, text.c_str());
    }

    std::string getWidgetText(std::uint64_t widgetId) const {
        const char* ptr = rw_get_widget_text(widgetId);
        if (ptr == nullptr) {
            return std::string();
        }
        std::string value(ptr);
        rw_free_string(const_cast<char*>(ptr));
        return value;
    }

    unsigned int cppBindingStatus() const { return rw_cpp_binding_status(); }
    unsigned int bindingsApiVersion() const { return rw_bindings_api_version(); }

    unsigned int setRenderAASamplesPerAxis(unsigned int samples) const {
        return rw_set_render_aa_samples_per_axis(samples);
    }

    unsigned int renderAASamplesPerAxis() const {
        return rw_get_render_aa_samples_per_axis();
    }

    unsigned int setEmbeddedTargetFps(unsigned int fps) const {
        return rw_set_embedded_target_fps(fps);
    }

    unsigned int embeddedTargetFps() const {
        return rw_get_embedded_target_fps();
    }

    std::uint64_t submitEmbeddedNoopTask(const std::string& label) const {
        return rw_submit_embedded_noop_task(label.c_str());
    }

    bool embeddedEngineInitialized() const {
        return rw_embedded_engine_is_initialized();
    }

    bool embeddedEngineRunning() const {
        return rw_embedded_engine_is_running();
    }

    std::uint64_t embeddedEngineFrameCount() const {
        return rw_embedded_engine_frame_count();
    }

    std::uint64_t embeddedEnginePendingTaskCount() const {
        return rw_embedded_engine_pending_task_count();
    }

    std::uint64_t embeddedEngineWindowCount() const {
        return rw_embedded_engine_window_count();
    }

    std::uint64_t embeddedEngineButtonCount() const {
        return rw_embedded_engine_button_count();
    }
};
