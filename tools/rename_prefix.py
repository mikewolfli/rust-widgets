#!/usr/bin/env python3
"""Replace rust_widgets_ prefix with rw_ across bindings and generated files."""

import re

FILES = [
    # Rust binding implementations
    "src/bindings/binding_impl.rs",
    "src/bindings/java_jni.rs",
    "src/error/ffi.rs",
    "src/asset/watcher.rs",
    "src/pdf/tests.rs",
    "src/platform/windows/notify.rs",
    "src/platform/windows/types.rs",
    "src/print/print_impl.rs",
    "src/wgpu_backend/renderer.rs",
    # C headers
    "include/rust_widgets_generated.h",
    "include/rust_widgets_errors.h",
    "include/rust_widgets.h",
    "examples/rust_widgets.generated.h",
    "examples/rust_widgets.h",
    # Python bindings
    "bindings/python/rust_widgets/__init__.py",
    "bindings/python/rust_widgets/errors.py",
    # Code generators
    "tools/generate_c_header.py",
    "tools/generate_error_header.py",
]

for fpath in FILES:
    try:
        with open(fpath) as f:
            content = f.read()
        old = content

        # Replace function name prefix (rust_widgets_ -> rw_)
        content = content.replace("rust_widgets_", "rw_")
        # Replace include guard prefix (RUST_WIDGETS_ -> RW_)
        content = content.replace("RUST_WIDGETS_", "RW_")
        # Replace filename references in comments
        content = content.replace("rust_widgets_generated.h", "rw_generated.h")
        content = content.replace("rust_widgets_errors.h", "rw_errors.h")
        content = content.replace("rust_widgets.h", "rw.h")

        if content != old:
            with open(fpath, "w") as f:
                f.write(content)
            print(f"  UPDATED: {fpath}")
        else:
            print(f"  SKIPPED: {fpath} (no changes)")
    except FileNotFoundError:
        print(f"  NOT FOUND: {fpath}")

print("\nDone!")
