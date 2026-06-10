# Language Bindings

rust-widgets exposes its API to five languages through a modular binding layer.
This chapter covers the C ABI foundation, Python, C++, Java, and Node.js bindings,
with build instructions and code examples for each.

---

## 1. C ABI Layer

The C ABI layer is the foundation for all language bindings. It exposes 102
`extern "C"` functions that wrap the `Platform` trait, providing language-neutral
access to widget creation, event polling, clipboard, and platform queries.

### String Memory Management

Strings crossing the FFI boundary follow a strict ownership model:

- **Returned strings**: Allocated by Rust and must be freed via
  `rw_free_string()`.
- **Input strings**: Accepted as `*const c_char` (null-terminated C strings).

```c
// C header excerpt (rust_widgets.h)
typedef uint64_t ObjectId;

void rw_init(void);
void rw_run(void);
void rw_quit(void);

ObjectId rw_create_window(const char* title,
    int32_t x, int32_t y, uint32_t width, uint32_t height);

ObjectId rw_create_button(ObjectId parent, const char* text,
    int32_t x, int32_t y, uint32_t width, uint32_t height);

// ... 96 more functions ...

void rw_free_string(char* s);
```

### The `c_try!` Pattern

The binding layer uses a `c_try!` macro to safely convert Rust `Result` types
into C-compatible error returns:

```rust
macro_rules! c_try {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => {
                log::error!("[C ABI] error: {:?}", e);
                return 0;
            }
        }
    };
}
```

### Complete C Example

```c
#include "rust_widgets.h"
#include <stdio.h>

int main(void) {
    rw_init();

    ObjectId window = rw_create_window("C Demo", 100, 100, 800, 600);

    ObjectId button = rw_create_button(window, "Click Me",
        10, 10, 120, 32);

    char* text = rw_get_widget_text(button);
    printf("Button text: %s\n", text);
    rw_free_string(text);

    rw_run();
    rw_quit();
    return 0;
}
```

Build with:
```sh
gcc -o demo demo.c -Ltarget/release -lrw_ffi -lpthread -ldl
```

---

## 2. Python Bindings

Python bindings use `ctypes` to load the shared library and wrap each C ABI
function in Pythonic methods. The `RustWidgets` class exposes 89 methods covering
widget creation, mutation, menu operations, and platform queries.

### Installation

```python
# setup.py
from setuptools import setup, find_packages

setup(
    name="rust-widgets",
    version="0.9.6",
    packages=find_packages(),
    description="Python bindings for the rust-widgets GUI library",
)
```

### Usage Example

```python
from rust_widgets import RustWidgets

# Initialize
rw = RustWidgets()
rw.init()

# Create window and widgets
window = rw.create_window("Python Demo", 100, 100, 800, 600)
button = rw.create_button(window, "Click Me", 10, 10, 120, 32)
label = rw.create_label(window, "Status: idle", 10, 60, 200, 24)

# Configure widgets
rw.set_widget_text(label, "Status: ready")
rw.set_widget_enabled(button, True)

# Combo box with items
combo = rw.create_combo_box(window, 10, 100, 200, 24)
rw.combo_box_add_item(combo, "Option A")
rw.combo_box_add_item(combo, "Option B")
rw.combo_box_set_current_index(combo, 0)

# List box with items
listbox = rw.create_list_box(window, 10, 140, 200, 120)
rw.list_box_add_item(listbox, "Item 1")
rw.list_box_add_item(listbox, "Item 2")
rw.list_box_remove_item(listbox, 0)

# Menu bar
menu_bar = rw.create_menu_bar(window, 0, 0, 800, 24)
rw.attach_menu_bar_to_window(window, menu_bar)
file_menu = rw.create_menu(menu_bar, "File", 0, 0, 60, 24)
new_id = rw.menu_add_item(file_menu, "New", "Ctrl+N")
quit_id = rw.menu_add_item(file_menu, "Quit", "Ctrl+Q")

# Event loop with menu polling
import time
while True:
    triggered = rw.poll_menu_triggered()
    if triggered is not None:
        if triggered == new_id:
            print("New clicked!")
        elif triggered == quit_id:
            break
    time.sleep(0.016)  # ~60 FPS

rw.quit()
```

### Python API Surface (89 methods)

| Category | Methods |
|----------|---------|
| **Lifecycle** | `init`, `run`, `quit` |
| **Window** | `create_window` |
| **Widgets** (22 create methods) | `create_button`, `create_checkbox`, `create_line_edit`, `create_label`, `create_radio_button`, `create_slider`, `create_progress_bar`, `create_combo_box`, `create_list_box`, `create_panel`, `create_menu_bar`, `create_menu`, `create_tool_bar`, `create_status_bar`, `create_message_box`, `create_file_dialog`, `create_color_dialog`, `create_font_dialog`, `create_spin_box`, `create_list_view`, `create_scroll_area` |
| **Widget Mutation** | `set_widget_geometry`, `set_widget_text`, `get_widget_text`, `set_widget_enabled`, `is_widget_enabled`, `set_widget_visible`, `is_widget_visible`, `show_widget`, `hide_widget`, `set_widget_ime_enabled`, `is_widget_ime_enabled`, `set_widget_accessibility_name`, `get_widget_accessibility_name` |
| **Combo Box** | `combo_box_add_item`, `combo_box_clear_items`, `combo_box_set_current_index`, `combo_box_current_index`, `combo_box_item_count`, `combo_box_item_text` |
| **List Box** | `list_box_add_item`, `list_box_remove_item`, `list_box_clear_items`, `list_box_set_current_index`, `list_box_current_index`, `list_box_item_count`, `list_box_item_text` |
| **Menu** | `attach_menu_bar_to_window`, `menu_add_item`, `poll_menu_triggered`, `inject_menu_trigger` |
| **Clipboard** | `set_clipboard_text`, `get_clipboard_text` |
| **Drag & Drop** | `begin_drag`, `poll_drop_event` |
| **Events** | `poll_widget_triggered`, `poll_widget_trigger_event`, `inject_widget_trigger_event` |
| **Platform** | `backend_name`, `platform_capabilities`, `platform_dpi_scale_factor`, `bindings_api_version` |

---

## 3. C++ Bindings

The C++ bindings are header-only, providing RAII wrappers and a class hierarchy
over the C ABI.

### Key Types

```cpp
// rust_widgets.hpp — header-only C++ bindings

#include <string>
#include <cstdint>
#include <memory>

using ObjectId = uint64_t;

// RAII string wrapper (frees via rw_free_string)
class RustString {
public:
    explicit RustString(char* s) : ptr_(s) {}
    ~RustString() { if (ptr_) rw_free_string(ptr_); }
    RustString(RustString&& other) noexcept : ptr_(other.ptr_) {
        other.ptr_ = nullptr;
    }
    const char* c_str() const { return ptr_ ? ptr_ : ""; }
private:
    char* ptr_;
};

// Widget base class
class Widget {
public:
    ObjectId id() const { return id_; }
    void set_text(const std::string& text) {
        rw_set_widget_text(id_, text.c_str());
    }
    std::string text() const {
        RustString s(rw_get_widget_text(id_));
        return s.c_str();
    }
    void set_enabled(bool enabled) {
        rw_set_widget_enabled(id_, enabled);
    }
    void set_geometry(int32_t x, int32_t y, uint32_t w, uint32_t h) {
        rw_set_widget_geometry(id_, x, y, w, h);
    }
    void show() { rw_show_widget(id_); }
    void hide() { rw_hide_widget(id_); }
protected:
    ObjectId id_ = 0;
};
```

### 22 Widget Subclasses

```cpp
class Window : public Widget { /* ... */ };
class Button : public Widget { /* ... */ };
class CheckBox : public Widget { /* ... */ };
class LineEdit : public Widget { /* ... */ };
class Label : public Widget { /* ... */ };
class RadioButton : public Widget { /* ... */ };
class Slider : public Widget { /* ... */ };
class ProgressBar : public Widget { /* ... */ };
class ComboBox : public Widget {
public:
    void add_item(const std::string& text);
    void clear_items();
    void set_current_index(size_t index);
    size_t current_index() const;
    size_t item_count() const;
    std::string item_text(size_t index) const;
};
class ListBox : public Widget { /* ... */ };
class Panel : public Widget { /* ... */ };
class MenuBar : public Widget { /* ... */ };
class Menu : public Widget { /* ... */ };
class ToolBar : public Widget { /* ... */ };
class StatusBar : public Widget { /* ... */ };
class MessageBox : public Widget { /* ... */ };
class FileDialog : public Widget { /* ... */ };
class ColorDialog : public Widget { /* ... */ };
class FontDialog : public Widget { /* ... */ };
class SpinBox : public Widget { /* ... */ };
class ListView : public Widget { /* ... */ };
class ScrollArea : public Widget { /* ... */ };
```

### TriggerKind Enum

```cpp
enum class TriggerKind {
    Unknown = 0,
    Clicked = 1,
    ValueChanged = 2,
    SelectionChanged = 3,
    Closed = 4,
};
```

### Complete C++ Example

```cpp
#include "rust_widgets.hpp"
#include <iostream>

int main() {
    rw_init();

    Window window("C++ Demo", 100, 100, 800, 600);

    Button button(window.id(), "Click Me", 10, 10, 120, 32);
    button.set_enabled(true);

    Label label(window.id(), "Status: idle", 10, 60, 200, 24);
    label.set_text("Status: ready");

    ComboBox combo(window.id(), 10, 100, 200, 24);
    combo.add_item("Option A");
    combo.add_item("Option B");
    combo.add_item("Option C");
    combo.set_current_index(0);

    std::cout << "Combo items: " << combo.item_count() << std::endl;

    MenuBar menu_bar(window.id(), 0, 0, 800, 24);
    rw_attach_menu_bar_to_window(window.id(), menu_bar.id());

    Menu file_menu(menu_bar.id(), "File", 0, 0, 60, 24);
    ObjectId new_id = rw_menu_add_item(file_menu.id(), "New", "Ctrl+N");
    ObjectId quit_id = rw_menu_add_item(file_menu.id(), "Quit", "Ctrl+Q");

    rw_run();
    rw_quit();
    return 0;
}
```

Build with:
```sh
g++ -std=c++17 -o demo example.cpp -Ltarget/release -lrw_ffi -lpthread -ldl
```

---

## 4. Java Bindings

Java bindings use JNI (Java Native Interface) to bridge the Rust library into
the JVM. The `RustWidgets.java` class declares `native` methods that map to
`extern "system"` functions in `java_jni.rs`.

### JNI Bridge Architecture

```rust
// src/bindings/java_jni.rs — JNI native methods

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeInit(
    _env: JNIEnv, _class: JClass) {
    rust_widgets::platform::init();
}

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeCreateWindow(
    mut env: JNIEnv, _class: JClass,
    title: JString, x: jint, y: jint, width: jint, height: jint
) -> jlong {
    let title_str = jstring_to_string(&mut env, &title);
    let id = get_platform().create_window(&title_str, x, y, width as u32, height as u32);
    id as jlong
}
```

### String Conversion Helpers

```rust
fn jstring_to_string(env: &mut JNIEnv, jstr: &JString) -> String {
    env.get_string(jstr).map(|s| s.into()).unwrap_or_default()
}

fn c_string_to_jstring(env: &mut JNIEnv, s: &str) -> JString {
    env.new_string(s).unwrap_or(JString::default())
}
```

### Java API

```java
// RustWidgets.java
package io.github.rustwidgets;

public class RustWidgets {
    static { System.loadLibrary("rw_jni"); }

    // Lifecycle
    public static native void nativeInit();
    public static native void nativeRun();
    public static native void nativeQuit();

    // Widget creation
    public static native long nativeCreateWindow(String title,
        int x, int y, int width, int height);
    public static native long nativeCreateButton(long parent, String text,
        int x, int y, int width, int height);
    public static native long nativeCreateCheckBox(long parent, String text,
        int x, int y, int width, int height);
    // ... all 22 widget creators ...

    // Widget mutation
    public static native void nativeSetWidgetText(long id, String text);
    public static native String nativeGetWidgetText(long id);
    public static native void nativeSetWidgetEnabled(long id, boolean enabled);
    public static native boolean nativeIsWidgetEnabled(long id);
    public static native void nativeShowWidget(long id);
    public static native void nativeHideWidget(long id);
    public static native void nativeSetWidgetGeometry(long id,
        int x, int y, int width, int height);

    // Combo box
    public static native void nativeComboBoxAddItem(long id, String text);
    public static native void nativeComboBoxClearItems(long id);
    public static native void nativeComboBoxSetCurrentIndex(long id, int index);
    public static native int nativeComboBoxCurrentIndex(long id);
    public static native int nativeComboBoxItemCount(long id);
    public static native String nativeComboBoxItemText(long id, int index);

    // List box
    public static native void nativeListBoxAddItem(long id, String text);
    public static native void nativeListBoxRemoveItem(long id, int index);
    public static native void nativeListBoxClearItems(long id);
    public static native void nativeListBoxSetCurrentIndex(long id, int index);
    public static native int nativeListBoxCurrentIndex(long id);
    public static native int nativeListBoxItemCount(long id);
    public static native String nativeListBoxItemText(long id, int index);

    // Menu
    public static native void nativeAttachMenuBarToWindow(long window, long menuBar);
    public static native long nativeMenuAddItem(long menu, String text, String shortcut);
    public static native Long nativePollMenuTriggered();

    // Clipboard
    public static native void nativeSetClipboardText(String text);
    public static native String nativeGetClipboardText();

    // Platform info
    public static native String nativeBackendName();
    public static native int nativePlatformCapabilities();
    public static native int nativeBindingsApiVersion();
}
```

### Complete Java Demo

```java
// RustWidgetsDemo.java
import io.github.rustwidgets.RustWidgets;

public class RustWidgetsDemo {
    public static void main(String[] args) {
        RustWidgets.nativeInit();

        long window = RustWidgets.nativeCreateWindow(
            "Java Demo", 100, 100, 800, 600);

        long button = RustWidgets.nativeCreateButton(
            window, "Click Me", 10, 10, 120, 32);

        long label = RustWidgets.nativeCreateLabel(
            window, "Status: idle", 10, 60, 200, 24);

        System.out.println("Backend: " + RustWidgets.nativeBackendName());

        // Combo box
        long combo = RustWidgets.nativeCreateComboBox(
            window, 10, 100, 200, 24);
        RustWidgets.nativeComboBoxAddItem(combo, "Java Option 1");
        RustWidgets.nativeComboBoxAddItem(combo, "Java Option 2");
        RustWidgets.nativeComboBoxSetCurrentIndex(combo, 0);
        System.out.println("Combo items: " +
            RustWidgets.nativeComboBoxItemCount(combo));

        // Clipboard
        RustWidgets.nativeSetClipboardText("Copied from Java!");
        String clip = RustWidgets.nativeGetClipboardText();
        System.out.println("Clipboard: " + clip);

        RustWidgets.nativeRun();
        RustWidgets.nativeQuit();
    }
}
```

### Building Java Bindings

```makefile
# Makefile
JAVA_HOME ?= /usr/lib/jvm/java-11-openjdk-amd64
LIB_NAME = librw_jni.so

all: $(LIB_NAME) demo

$(LIB_NAME):
	cargo build --release --features jni

demo: RustWidgetsDemo.java
	javac -d . RustWidgetsDemo.java RustWidgets.java
	java -Djava.library.path=target/release io.github.rustwidgets.RustWidgetsDemo
```

---

## 5. Node.js Bindings

Node.js bindings use `ffi-napi` and `ref-napi` to load the native library
and call C ABI functions directly from JavaScript.

### Package Configuration

```json
{
  "name": "rust-widgets",
  "version": "0.9.6",
  "description": "Node.js bindings for the rust-widgets GUI library",
  "main": "index.js",
  "dependencies": {
    "ffi-napi": "^4.0.3",
    "ref-napi": "^3.0.3"
  }
}
```

### Singleton Pattern (index.js)

```javascript
const ffi = require('ffi-napi');
const ref = require('ref-napi');

// Type aliases
const ObjectId = ref.types.uint64;
const cstr = ref.types.CString;
const voidPtr = ref.refType(ref.types.void);

let _instance = null;

class RustWidgets {
    constructor(libPath) {
        if (_instance) return _instance;

        this.lib = ffi.Library(libPath, {
            rw_init: ['void', []],
            rw_run: ['void', []],
            rw_quit: ['void', []],
            rw_create_window: [ObjectId, [cstr, 'int', 'int', 'uint', 'uint']],
            rw_create_button: [ObjectId, [ObjectId, cstr, 'int', 'int', 'uint', 'uint']],
            rw_free_string: ['void', [voidPtr]],
            // ... all 102 functions ...
        });

        _instance = this;
    }

    // Buffer to string conversion helper
    _readString(fn) {
        const buf = fn();
        if (!buf || buf.isNull()) return '';
        try {
            const s = ref.readCString(buf, 0);
            this.lib.rw_free_string(buf);
            return s;
        } catch (e) {
            return '';
        }
    }

    init() { this.lib.rw_init(); }
    run() { this.lib.rw_run(); }
    quit() { this.lib.rw_quit(); }

    createWindow(title, x, y, w, h) {
        return this.lib.rw_create_window(title, x, y, w, h);
    }

    createButton(parent, text, x, y, w, h) {
        return this.lib.rw_create_button(parent, text, x, y, w, h);
    }

    getWidgetText(id) {
        return this._readString(() =>
            this.lib.rw_get_widget_text(id));
    }

    setWidgetText(id, text) {
        this.lib.rw_set_widget_text(id, text);
    }

    backendName() {
        const buf = this.lib.rw_backend_name();
        return ref.readCString(buf, 0);
    }
}

module.exports = RustWidgets;
```

### Node.js Example

```javascript
const RustWidgets = require('rust-widgets');

async function main() {
    const rw = new RustWidgets('../target/release/librw_ffi.so');

    rw.init();

    const window = rw.createWindow('Node.js Demo', 100, 100, 800, 600);
    const button = rw.createButton(window, 'Click Me', 10, 10, 120, 32);
    const label = rw.createLabel(window, 'Status: idle', 10, 60, 200, 24);

    console.log('Backend:', rw.backendName());
    console.log('Button text:', rw.getWidgetText(button));

    // Combo box
    const combo = rw.createComboBox(window, 10, 100, 200, 24);
    rw.comboBoxAddItem(combo, 'Node Option 1');
    rw.comboBoxAddItem(combo, 'Node Option 2');
    rw.comboBoxSetCurrentIndex(combo, 1);

    // Clipboard
    rw.setClipboardText('Hello from Node.js!');
    console.log('Clipboard:', rw.getClipboardText());

    // Event loop
    setInterval(() => {
        const triggered = rw.pollMenuTriggered();
        if (triggered) {
            console.log('Menu triggered:', triggered);
        }
    }, 16);

    // rw.run();  // Blocks until quit
}

main().catch(console.error);
```

---

## 6. Building Each Binding

### C ABI (Shared Library)

```sh
cargo build --release
# Produces: target/release/librw_ffi.{so,dylib,dll}
```

### Python

```sh
pip install ctypes  # (stdlib, no install needed)
python example.py
```

### C++

```sh
g++ -std=c++17 -Iinclude -o demo examples/cpp/example.cpp \
    -Ltarget/release -lrw_ffi -lpthread -ldl
```

### Java

```sh
make -C bindings/java
# Compiles Rust JNI lib + Java classes + runs demo
```

### Node.js

```sh
cd bindings/nodejs
npm install
node example.js
```

---

## 7. API Version Tracking

Each binding exposes a version query to ensure compatibility:

```rust
// C ABI
pub extern "C" fn rw_bindings_api_version() -> u32 {
    1 // Incremented on breaking ABI changes
}

// Python
version = rw.bindings_api_version()

// C++
uint32_t version = rw_bindings_api_version();

// Java
int version = RustWidgets.nativeBindingsApiVersion();

// Node.js
const version = rw.lib.rw_bindings_api_version();
```

### Binding Status Checks

```rust
pub extern "C" fn rw_python_binding_status() -> u32 { 1 }
pub extern "C" fn rw_cpp_binding_status() -> u32 { 1 }
pub extern "C" fn rw_java_binding_status() -> u32 { 1 }
pub extern "C" fn rw_java_jni_skeleton_version() -> u32 { 1 }
```

### Platform Capability Bitmask (C ABI)

```rust
pub extern "C" fn rw_platform_capabilities(caps: *mut u32) {
    // Returns a bitmask:
    //   bit 0: dpi_scaling
    //   bit 1: ime
    //   bit 2: accessibility
    //   bit 3: native_menu
    //   bit 4: typed_widget_trigger
}
```

---

## 8. Cross-Binding Feature Matrix

| Feature | C | Python | C++ | Java | Node.js |
|---------|:---:|:---:|:---:|:---:|:---:|
| **Widget creation** (22 types) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Widget mutation** (13 methods) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Combo box** (6 methods) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **List box** (7 methods) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Menu system** (4 methods) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Clipboard** (2 methods) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Drag & drop** (2 methods) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Typed triggers** (6 methods) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Platform queries** (4 methods) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Mobile API** (2 methods) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Embedded engine** (5 methods) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Render config** (4 methods) | ✅ | ✅ | ✅ | ✅ | ✅ |

### Harmony-Specific Extensions

The C ABI includes HarmonyOS-specific functions for NAPI bridge integration:

```c
// Widget trigger injection (Harmony event bridge)
void rw_harmony_on_menu_item(ObjectId widget_id);
void rw_harmony_on_click(ObjectId widget_id);
void rw_harmony_on_value_changed(ObjectId widget_id);
void rw_harmony_on_widget_event(ObjectId widget_id, int trigger_kind);

// Node binding registry
void rw_harmony_bind_node(ObjectId widget_id, const char* node_id);
void rw_harmony_unbind_node(ObjectId widget_id);

// Typed node events (harmony)
void rw_harmony_on_node_click(ObjectId widget_id, const char* node_id);
void rw_harmony_on_node_value_changed(ObjectId widget_id, const char* node_id);
void rw_harmony_on_node_widget_event(ObjectId widget_id,
    const char* node_id, int trigger_kind);
```

---

## 9. Memory Management Across Bindings

| Binding | String Return | Lifetime | Free Mechanism |
|---------|:---:|----------|----------------|
| **C** | `char*` (heap) | Until `free_string` | `rw_free_string()` |
| **Python** | `str` | Immediate copy | `ctypes` copies to Python string |
| **C++** | `RustString` (RAII) | Scope-bound | Destructor calls `free_string` |
| **Java** | `String` | Immediate copy | JNI copies to Java `String` |
| **Node.js** | `Buffer` | Until read | `ref.readCString()` then `free_string` |

The core principle: **Rust allocates, the caller frees.** Each binding wraps this
in an idiomatic pattern — RAII in C++, destructor/`finally` in Java, manual
free in C, automatic copy in Python and Node.js.
