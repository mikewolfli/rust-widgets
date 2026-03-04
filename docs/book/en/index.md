# Rust Widgets Documentation

Welcome to the **rust_widgets** documentation! This is a comprehensive guide to building cross-platform GUI applications using the rust_widgets framework.

## What is rust_widgets?

**rust_widgets** is a powerful, cross-platform GUI framework for Rust that enables you to build native desktop and mobile applications. It provides:

- **Cross-Platform**: Write once, run on Windows, macOS, Linux, HarmonyOS, and embedded systems
- **Native Performance**: Direct platform integration without virtualization overhead
- **Rich Widget Set**: Comprehensive collection of basic and advanced UI components
- **Multi-Language Support**: Built-in internationalization (i18n) with runtime language switching
- **Multiple Language Bindings**: Use from Rust, C, Python, Java, and more
- **GPU Acceleration**: Optional GPU rendering backend for high-performance graphics

## Quick Links

- [Getting Started](getting-started/quickstart.md) - Start building your first application
- [Widget Reference](widgets/basic.md) - Browse available UI components
- [API Documentation](api/rust.md) - Explore the API reference
- [Demos](demos/basic.md) - See example applications

## Example

Here's a simple "Hello World" application:

```rust
use rust_widgets::{create_window, create_label, show_widget, run, init};

fn main() {
    init();
    
    let window = create_window("Hello World", 100, 100, 400, 300);
    let label = create_label(window, "Hello, rust_widgets!", 20, 20, 200, 30);
    
    show_widget(window);
    run();
}
```

## Features

### Widget Types

- **Basic Widgets**: Button, Label, LineEdit, TextEdit, Checkbox, RadioButton, ComboBox, Slider, ProgressBar, SpinBox
- **Container Widgets**: Panel, Splitter, TabWidget, DockPanel, ScrollArea, GroupBox
- **Advanced Widgets**: TreeView, TableView, ListView, MdiArea, Chart, Canvas
- **Dialog Widgets**: MessageBox, FileDialog, InputDialog, Custom Dialogs

### Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Windows | ✅ Supported | Win32 API with DPI awareness |
| macOS | ✅ Supported | Cocoa/AppKit integration |
| Linux | ✅ Supported | GTK3/GTK4 backends |
| HarmonyOS | ✅ Supported | Native ArkUI integration |
| Embedded | ✅ Supported | Custom render engine |

### Language Bindings

- **Rust**: Native API (primary)
- **C**: Full ABI compatibility
- **Python**: Pythonic wrapper with type hints
- **Java**: JNI-based integration
- **C++**: Header-only wrapper

## Architecture

rust_widgets follows a layered architecture:

```
┌─────────────────────────────────────┐
│        Application Layer            │
│    (Your GUI Application)           │
├─────────────────────────────────────┤
│        Widget Layer                 │
│    (Buttons, Labels, etc.)          │
├─────────────────────────────────────┤
│        Platform Abstraction         │
│    (Cross-platform interface)       │
├─────────────────────────────────────┤
│        Native Backends              │
│  (Win32/Cocoa/GTK/ArkUI)            │
└─────────────────────────────────────┘
```

## Getting Help

- **Issues**: Report bugs on [GitHub Issues](https://github.com/your-org/rust-widgets/issues)
- **Discussions**: Join community discussions
- **Documentation**: Browse this comprehensive guide

## License

rust_widgets is licensed under the MIT License. See [LICENSE](../LICENSE) for details.

---

**Ready to start?** Head to the [Quick Start Guide](getting-started/quickstart.md)!
