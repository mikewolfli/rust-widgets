# rust_widgets Python adapter

This package provides a minimal `ctypes` wrapper over the `rust_widgets` C ABI.

## Local install

```bash
cd examples/python
python -m pip install -e .
```

## Usage

```python
from rust_widgets import RustWidgets

api = RustWidgets()
print(api.bindings_api_version())
print(api.python_binding_status())
```

## Demos

```bash
# Basic wrapper smoke demo
python examples/python/demo_basic.py

# Embedded engine control/stats demo
python examples/python/demo_embedded_engine.py
```
