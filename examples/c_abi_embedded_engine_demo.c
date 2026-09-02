// C ABI embedded engine demo for FPS control and runtime stats.
#include <stdint.h>
#include <stdio.h>

#include "rw.h"

int main(void) {
    // Initialize runtime before reading embedded engine diagnostics.
    rw_init();

    unsigned int abi_version = rw_bindings_api_version();

    // Configure embedded target FPS (clamped to 1..=240) and submit one noop task.
    unsigned int applied_fps = rw_set_embedded_target_fps(90);
    unsigned int target_fps = rw_get_embedded_target_fps();
    uint64_t task_id = rw_submit_embedded_noop_task("embedded-demo-noop");

    // Create one window and one button to populate resource registry counters.
    uint64_t window = rw_create_window("Embedded Engine C ABI Demo", 120, 120, 480, 300);
    uint64_t button = rw_create_button(window, "OK", 24, 24, 96, 36);

    printf("DEMO_PROFILE=embedded\n");
    printf("ABI_VERSION=%u\n", abi_version);
    printf("TARGET_FPS=%u\n", target_fps);
    printf("APPLIED_FPS=%u\n", applied_fps);
    printf("TASK_ID=%llu\n", (unsigned long long)task_id);
    printf("WINDOW_ID=%llu\n", (unsigned long long)window);
    printf("BUTTON_ID=%llu\n", (unsigned long long)button);
    printf("ENGINE_INITIALIZED=%d\n", rw_embedded_engine_is_initialized());
    printf("ENGINE_RUNNING=%d\n", rw_embedded_engine_is_running());
    printf("FRAME_COUNT=%llu\n", (unsigned long long)rw_embedded_engine_frame_count());
    printf("PENDING_TASK_COUNT=%llu\n", (unsigned long long)rw_embedded_engine_pending_task_count());
    printf("WINDOW_COUNT=%llu\n", (unsigned long long)rw_embedded_engine_window_count());
    printf("BUTTON_COUNT=%llu\n", (unsigned long long)rw_embedded_engine_button_count());

    rw_quit();
    return 0;
}