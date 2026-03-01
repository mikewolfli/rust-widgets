# Basic Python demo using the rust_widgets ctypes facade.
from rust_widgets import RustWidgets


def main() -> None:
    api = RustWidgets()
    print(f"ABI version: {api.bindings_api_version()}")

    api.init()
    window = api.create_window("Python Binding Demo", 100, 100, 640, 360)
    button = api.create_button(window, "Click Me", 24, 24, 140, 36)

    api.set_widget_text(button, "Python says hi")
    print("button text:", api.get_widget_text(button))
    print("capabilities mask:", api.platform_capabilities())
    print("capability contract(full):", api.capability_contract(embedded=False))

    # For CI/demo environments this script intentionally does not enter `run()`.


if __name__ == "__main__":
    main()
