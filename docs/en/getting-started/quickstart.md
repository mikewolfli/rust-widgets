# Quick Start Guide

This guide will help you get started with rust_widgets in just a few minutes.

## Prerequisites

Before you begin, make sure you have:

- **Rust** (1.70 or later) installed - [Install Rust](https://rustup.rs/)
- A C compiler for your platform (usually included with Rust)

## Installation

Add rust_widgets to your `Cargo.toml`:

```toml
[dependencies]
rust_widgets = "0.5"
```

Or use cargo add:

```bash
cargo add rust_widgets
```

## Your First Application

Create a new Rust project:

```bash
cargo new my_first_app
cd my_first_app
```

Edit `src/main.rs`:

```rust
use rust_widgets::{
    create_window, create_label, create_button, show_widget, run, init,
    connect_clicked, set_widget_text
};

fn main() {
    // Initialize the framework
    init();
    
    // Create main window
    let window = create_window("My First App", 100, 100, 400, 300);
    
    // Create a label
    let label = create_label(window, "Hello, rust_widgets!", 20, 20, 200, 30);
    
    // Create a button
    let button = create_button(window, "Click Me!", 20, 60, 100, 30);
    
    // Connect button click event
    connect_clicked(button, move || {
        set_widget_text(label, "Button clicked!");
    });
    
    // Show the window and start event loop
    show_widget(window);
    run();
}
```

Run your application:

```bash
cargo run
```

## Next Steps

- Learn about [Basic Widgets](../widgets/basic.md)
- Explore [Event Handling](../concepts/events.md)
- Check out the [Demos](../demos/basic.md)
- Read the [Architecture Overview](../concepts/architecture.md)

## Troubleshooting

### Build Errors

If you encounter build errors:

1. Ensure your Rust version is up to date: `rustup update`
2. Check that you have required system libraries installed
3. See platform-specific notes in [Installation](installation.md)

### Runtime Issues

If the application doesn't start:

1. Check that your display environment is properly configured
2. On Linux, ensure you have GTK development libraries installed
3. On Windows, ensure you have the Windows SDK installed

## Getting Help

- Browse the [FAQ](../appendix/faq.md)
- Check [GitHub Issues](https://github.com/your-org/rust-widgets/issues)
- Join our community discussions
