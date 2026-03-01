# Python embedded-engine demo for FPS control and runtime stats.
from rust_widgets import RustWidgets


def main() -> None:
    api = RustWidgets()
    abi_version = api.bindings_api_version()

    api.init()

    applied_fps = api.set_embedded_target_fps(90)
    target_fps = api.embedded_target_fps()
    task_id = api.submit_embedded_noop_task("python-embedded-noop")

    window = api.create_window("Embedded Engine Python Demo", 120, 120, 480, 300)
    button = api.create_button(window, "OK", 24, 24, 96, 36)

    engine_initialized = api.embedded_engine_is_initialized()
    engine_running = api.embedded_engine_is_running()
    frame_count = api.embedded_engine_frame_count()
    pending_task_count = api.embedded_engine_pending_task_count()
    window_count = api.embedded_engine_window_count()
    button_count = api.embedded_engine_button_count()

    print("DEMO_PROFILE=embedded")
    print(f"ABI_VERSION={abi_version}")
    print(f"TARGET_FPS={target_fps}")
    print(f"APPLIED_FPS={applied_fps}")
    print(f"TASK_ID={task_id}")
    print(f"WINDOW_ID={window}")
    print(f"BUTTON_ID={button}")
    print(f"ENGINE_INITIALIZED={int(engine_initialized)}")
    print(f"ENGINE_RUNNING={int(engine_running)}")
    print(f"FRAME_COUNT={frame_count}")
    print(f"PENDING_TASK_COUNT={pending_task_count}")
    print(f"WINDOW_COUNT={window_count}")
    print(f"BUTTON_COUNT={button_count}")

    api.quit()


if __name__ == "__main__":
    main()
