"""setup.py for the rust-widgets Python bindings package."""

import os
import sys
from pathlib import Path

from setuptools import find_packages, setup

# ---------------------------------------------------------------------------
# Read the project README for a long description
# ---------------------------------------------------------------------------
_here = Path(__file__).resolve().parent
_long_description = ""
readme = _here.parent.parent / "README.md"
if readme.is_file():
    _long_description = readme.read_text(encoding="utf-8")


# ---------------------------------------------------------------------------
# Build the shared library via `cargo build`  (optional helper)
# ---------------------------------------------------------------------------
def _build_cdylib() -> None:
    """Build the Rust library in release mode.

    This is run during ``pip install`` if the ``RUST_WIDGETS_BUILD``
    environment variable is set to ``1`` or ``true``.
    """
    build_flag = os.environ.get("RUST_WIDGETS_BUILD", "").lower()
    if build_flag not in ("1", "true", "yes", "on"):
        return

    project_root = _here.parent.parent
    print(f"[rust-widgets] Building cdylib in {project_root} ...", file=sys.stderr)
    ret = os.system(f"cd {project_root} && cargo build --release")
    if ret != 0:
        raise SystemError("cargo build --release failed")
    print("[rust-widgets] Build complete.", file=sys.stderr)


_build_cdylib()


# ---------------------------------------------------------------------------
# Package metadata
# ---------------------------------------------------------------------------
setup(
    name="rust-widgets",
    version="0.9.6",
    description="Python bindings for the rust-widgets native GUI library",
    long_description=_long_description,
    long_description_content_type="text/markdown",
    author="rust-widgets contributors",
    url="https://github.com/your-org/rust-widgets",
    license="MIT OR Apache-2.0",
    packages=find_packages(where="."),
    package_dir={"": "."},
    include_package_data=True,
    package_data={
        "rust_widgets": ["py.typed"],
    },
    python_requires=">=3.9",
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "License :: OSI Approved :: Apache Software License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Programming Language :: Python :: 3.13",
        "Programming Language :: Rust",
        "Topic :: Software Development :: Libraries",
        "Topic :: Software Development :: User Interfaces",
    ],
    # No runtime dependencies — everything uses stdlib ctypes.
    install_requires=[],
    extras_require={
        "dev": [
            "pytest>=7.0",
            "pytest-cov>=4.0",
            "mypy>=1.0",
            "ruff>=0.1",
        ],
    },
)
