# 語言綁定 (Language Bindings)

rust-widgets 透過模組化綁定層將其 API 公開給五種語言。本章涵蓋 C ABI 基礎、Python、C++、Java 與 Node.js 綁定，並包含各語言的建置說明與程式碼範例。

---

## 1. C ABI 層

C ABI 層是所有語言綁定的基礎。它公開了 102 個 `extern "C"` 函式，封裝了 `Platform` 特徵 (trait)，提供與語言無關的 widget 建立、事件輪詢、剪貼簿與平台查詢功能。

### 字串記憶體管理

跨越 FFI 邊界的字串遵循嚴格的擁有權模型：

- **回傳的字串**：由 Rust 分配，必須透過 `rw_free_string()` 釋放。
- **輸入的字串**：以 `*const c_char`（以 null 結尾的 C 字串）形式接受。

```c
// C 標頭檔摘錄 (rust_widgets.h)
typedef uint64_t ObjectId;

void rw_init(void);
void rw_run(void);
void rw_quit(void);

ObjectId rw_create_window(const char* title,
    int32_t x, int32_t y, uint32_t width, uint32_t height);

ObjectId rw_create_button(ObjectId parent, const char* text,
    int32_t x, int32_t y, uint32_t width, uint32_t height);

// ... 還有 96 個以上函式 ...

void rw_free_string(char* s);
```

### `c_try!` 模式

綁定層使用 `c_try!` 巨集來安全地將 Rust 的 `Result` 型別轉換為 C 相容的錯誤回傳：

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

### 完整的 C 範例

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

建置指令：
```sh
gcc -o demo demo.c -Ltarget/release -lrw_ffi -lpthread -ldl
```

---

## 2. Python 綁定

Python 綁定使用 `ctypes` 載入共享函式庫，並將每個 C ABI 函式封裝為 Python 風格的方法。`RustWidgets` 類別公開了 89 個方法，涵蓋 widget 建立、修改、選單操作與平台查詢。

### 安裝

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

### 使用範例

```python
from rust_widgets import RustWidgets

# 初始化
rw = RustWidgets()
rw.init()

# 建立視窗與 widget
window = rw.create_window("Python Demo", 100, 100, 800, 600)
button = rw.create_button(window, "Click Me", 10, 10, 120, 32)
label = rw.create_label(window, "Status: idle", 10, 60, 200, 24)

# 設定 widget
rw.set_widget_text(label, "Status: ready")
rw.set_widget_enabled(button, True)

# 含項目的下拉式方塊
combo = rw.create_combo_box(window, 10, 100, 200, 24)
rw.combo_box_add_item(combo, "Option A")
rw.combo_box_add_item(combo, "Option B")
rw.combo_box_set_current_index(combo, 0)

# 含項目的清單方塊
listbox = rw.create_list_box(window, 10, 140, 200, 120)
rw.list_box_add_item(listbox, "Item 1")
rw.list_box_add_item(listbox, "Item 2")
rw.list_box_remove_item(listbox, 0)

# 選單列
menu_bar = rw.create_menu_bar(window, 0, 0, 800, 24)
rw.attach_menu_bar_to_window(window, menu_bar)
file_menu = rw.create_menu(menu_bar, "File", 0, 0, 60, 24)
new_id = rw.menu_add_item(file_menu, "New", "Ctrl+N")
quit_id = rw.menu_add_item(file_menu, "Quit", "Ctrl+Q")

# 含選單輪詢的事件迴圈
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

### Python API 表面 (89 個方法)

| 類別 | 方法 |
|----------|---------|
| **生命週期** | `init`, `run`, `quit` |
| **視窗** | `create_window` |
| **Widgets** (22 個建立方法) | `create_button`, `create_checkbox`, `create_line_edit`, `create_label`, `create_radio_button`, `create_slider`, `create_progress_bar`, `create_combo_box`, `create_list_box`, `create_panel`, `create_menu_bar`, `create_menu`, `create_tool_bar`, `create_status_bar`, `create_message_box`, `create_file_dialog`, `create_color_dialog`, `create_font_dialog`, `create_spin_box`, `create_list_view`, `create_scroll_area` |
| **Widget 修改** | `set_widget_geometry`, `set_widget_text`, `get_widget_text`, `set_widget_enabled`, `is_widget_enabled`, `set_widget_visible`, `is_widget_visible`, `show_widget`, `hide_widget`, `set_widget_ime_enabled`, `is_widget_ime_enabled`, `set_widget_accessibility_name`, `get_widget_accessibility_name` |
| **下拉式方塊** | `combo_box_add_item`, `combo_box_clear_items`, `combo_box_set_current_index`, `combo_box_current_index`, `combo_box_item_count`, `combo_box_item_text` |
| **清單方塊** | `list_box_add_item`, `list_box_remove_item`, `list_box_clear_items`, `list_box_set_current_index`, `list_box_current_index`, `list_box_item_count`, `list_box_item_text` |
| **選單** | `attach_menu_bar_to_window`, `menu_add_item`, `poll_menu_triggered`, `inject_menu_trigger` |
| **剪貼簿** | `set_clipboard_text`, `get_clipboard_text` |
| **拖放** | `begin_drag`, `poll_drop_event` |
| **事件** | `poll_widget_triggered`, `poll_widget_trigger_event`, `inject_widget_trigger_event` |
| **平台** | `backend_name`, `platform_capabilities`, `platform_dpi_scale_factor`, `bindings_api_version` |

---

## 3. C++ 綁定

C++ 綁定為僅標頭檔 (header-only)，提供 RAII 包裝器以及在 C ABI 之上的類別階層結構。

### 關鍵型別

```cpp
// rust_widgets.hpp — 僅標頭檔的 C++ 綁定

#include <string>
#include <cstdint>
#include <memory>

using ObjectId = uint64_t;

// RAII 字串包裝器 (透過 rw_free_string 釋放)
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

// Widget 基底類別
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

### 22 個 Widget 子類別

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

### TriggerKind 列舉

```cpp
enum class TriggerKind {
    Unknown = 0,
    Clicked = 1,
    ValueChanged = 2,
    SelectionChanged = 3,
    Closed = 4,
};
```

### 完整的 C++ 範例

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

建置指令：
```sh
g++ -std=c++17 -o demo example.cpp -Ltarget/release -lrw_ffi -lpthread -ldl
```

---

## 4. Java 綁定

Java 綁定使用 JNI (Java Native Interface) 將 Rust 函式庫橋接到 JVM。`RustWidgets.java` 類別宣告了 `native` 方法，對應到 `java_jni.rs` 中的 `extern "system"` 函式。

### JNI 橋接架構

```rust
// src/bindings/java_jni.rs — JNI 原生方法

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

### 字串轉換輔助函式

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

    // 生命週期
    public static native void nativeInit();
    public static native void nativeRun();
    public static native void nativeQuit();

    // Widget 建立
    public static native long nativeCreateWindow(String title,
        int x, int y, int width, int height);
    public static native long nativeCreateButton(long parent, String text,
        int x, int y, int width, int height);
    public static native long nativeCreateCheckBox(long parent, String text,
        int x, int y, int width, int height);
    // ... 全部 22 個 widget 建立方法 ...

    // Widget 修改
    public static native void nativeSetWidgetText(long id, String text);
    public static native String nativeGetWidgetText(long id);
    public static native void nativeSetWidgetEnabled(long id, boolean enabled);
    public static native boolean nativeIsWidgetEnabled(long id);
    public static native void nativeShowWidget(long id);
    public static native void nativeHideWidget(long id);
    public static native void nativeSetWidgetGeometry(long id,
        int x, int y, int width, int height);

    // 下拉式方塊
    public static native void nativeComboBoxAddItem(long id, String text);
    public static native void nativeComboBoxClearItems(long id);
    public static native void nativeComboBoxSetCurrentIndex(long id, int index);
    public static native int nativeComboBoxCurrentIndex(long id);
    public static native int nativeComboBoxItemCount(long id);
    public static native String nativeComboBoxItemText(long id, int index);

    // 清單方塊
    public static native void nativeListBoxAddItem(long id, String text);
    public static native void nativeListBoxRemoveItem(long id, int index);
    public static native void nativeListBoxClearItems(long id);
    public static native void nativeListBoxSetCurrentIndex(long id, int index);
    public static native int nativeListBoxCurrentIndex(long id);
    public static native int nativeListBoxItemCount(long id);
    public static native String nativeListBoxItemText(long id, int index);

    // 選單
    public static native void nativeAttachMenuBarToWindow(long window, long menuBar);
    public static native long nativeMenuAddItem(long menu, String text, String shortcut);
    public static native Long nativePollMenuTriggered();

    // 剪貼簿
    public static native void nativeSetClipboardText(String text);
    public static native String nativeGetClipboardText();

    // 平台資訊
    public static native String nativeBackendName();
    public static native int nativePlatformCapabilities();
    public static native int nativeBindingsApiVersion();
}
```

### 完整的 Java 示範

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

        // 下拉式方塊
        long combo = RustWidgets.nativeCreateComboBox(
            window, 10, 100, 200, 24);
        RustWidgets.nativeComboBoxAddItem(combo, "Java Option 1");
        RustWidgets.nativeComboBoxAddItem(combo, "Java Option 2");
        RustWidgets.nativeComboBoxSetCurrentIndex(combo, 0);
        System.out.println("Combo items: " +
            RustWidgets.nativeComboBoxItemCount(combo));

        // 剪貼簿
        RustWidgets.nativeSetClipboardText("Copied from Java!");
        String clip = RustWidgets.nativeGetClipboardText();
        System.out.println("Clipboard: " + clip);

        RustWidgets.nativeRun();
        RustWidgets.nativeQuit();
    }
}
```

### 建置 Java 綁定

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

## 5. Node.js 綁定

Node.js 綁定使用 `ffi-napi` 和 `ref-napi` 來載入原生函式庫，並直接從 JavaScript 呼叫 C ABI 函式。

### 套件設定

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

### 單例模式 (index.js)

```javascript
const ffi = require('ffi-napi');
const ref = require('ref-napi');

// 型別別名
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
            // ... 全部 102 個函式 ...
        });

        _instance = this;
    }

    // 緩衝區轉字串輔助函式
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

### Node.js 範例

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

    // 下拉式方塊
    const combo = rw.createComboBox(window, 10, 100, 200, 24);
    rw.comboBoxAddItem(combo, 'Node Option 1');
    rw.comboBoxAddItem(combo, 'Node Option 2');
    rw.comboBoxSetCurrentIndex(combo, 1);

    // 剪貼簿
    rw.setClipboardText('Hello from Node.js!');
    console.log('Clipboard:', rw.getClipboardText());

    // 事件迴圈
    setInterval(() => {
        const triggered = rw.pollMenuTriggered();
        if (triggered) {
            console.log('Menu triggered:', triggered);
        }
    }, 16);

    // rw.run();  // 阻塞直到 quit
}

main().catch(console.error);
```

---

## 6. 建置各綁定

### C ABI (共享函式庫)

```sh
cargo build --release
# 產生：target/release/librw_ffi.{so,dylib,dll}
```

### Python

```sh
pip install ctypes  # (stdlib，不需安裝)
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
# 編譯 Rust JNI 函式庫 + Java 類別 + 執行示範
```

### Node.js

```sh
cd bindings/nodejs
npm install
node example.js
```

---

## 7. API 版本追蹤

每個綁定都公開了一個版本查詢功能以確保相容性：

```rust
// C ABI
pub extern "C" fn rw_bindings_api_version() -> u32 {
    1 // 在 ABI 重大變更時遞增
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

### 綁定狀態檢查

```rust
pub extern "C" fn rw_python_binding_status() -> u32 { 1 }
pub extern "C" fn rw_cpp_binding_status() -> u32 { 1 }
pub extern "C" fn rw_java_binding_status() -> u32 { 1 }
pub extern "C" fn rw_java_jni_skeleton_version() -> u32 { 1 }
```

### 平台功能位元遮罩 (C ABI)

```rust
pub extern "C" fn rw_platform_capabilities(caps: *mut u32) {
    // 回傳位元遮罩：
    //   bit 0: dpi_scaling
    //   bit 1: ime
    //   bit 2: accessibility
    //   bit 3: native_menu
    //   bit 4: typed_widget_trigger
}
```

---

## 8. 跨綁定功能矩陣

| 功能 | C | Python | C++ | Java | Node.js |
|---------|:---:|:---:|:---:|:---:|:---:|
| **Widget 建立** (22 種類型) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Widget 修改** (13 個方法) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **下拉式方塊** (6 個方法) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **清單方塊** (7 個方法) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **選單系統** (4 個方法) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **剪貼簿** (2 個方法) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **拖放** (2 個方法) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **型別化觸發** (6 個方法) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **平台查詢** (4 個方法) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **行動 API** (2 個方法) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **嵌入式引擎** (5 個方法) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **渲染設定** (4 個方法) | ✅ | ✅ | ✅ | ✅ | ✅ |

### HarmonyOS 專用擴充

C ABI 包含 HarmonyOS 專用函式，用於 NAPI 橋接整合：

```c
// Widget 觸發注入 (Harmony 事件橋接)
void rw_harmony_on_menu_item(ObjectId widget_id);
void rw_harmony_on_click(ObjectId widget_id);
void rw_harmony_on_value_changed(ObjectId widget_id);
void rw_harmony_on_widget_event(ObjectId widget_id, int trigger_kind);

// 節點綁定註冊
void rw_harmony_bind_node(ObjectId widget_id, const char* node_id);
void rw_harmony_unbind_node(ObjectId widget_id);

// 型別化節點事件 (harmony)
void rw_harmony_on_node_click(ObjectId widget_id, const char* node_id);
void rw_harmony_on_node_value_changed(ObjectId widget_id, const char* node_id);
void rw_harmony_on_node_widget_event(ObjectId widget_id,
    const char* node_id, int trigger_kind);
```

---

## 9. 跨綁定的記憶體管理

| 綁定 | 字串回傳型別 | 生命週期 | 釋放機制 |
|---------|:---:|----------|----------------|
| **C** | `char*` (堆積) | 直到 `free_string` | `rw_free_string()` |
| **Python** | `str` | 立即複製 | `ctypes` 複製到 Python 字串 |
| **C++** | `RustString` (RAII) | 作用域綁定 | 解構子呼叫 `free_string` |
| **Java** | `String` | 立即複製 | JNI 複製到 Java `String` |
| **Node.js** | `Buffer` | 直到讀取 | `ref.readCString()` 然後 `free_string` |

核心原則：**Rust 分配，呼叫者釋放。** 每個綁定都以慣用的模式包裝此邏輯 — C++ 用 RAII，Java 用解構子/`finally`，C 用手動釋放，Python 與 Node.js 用自動複製。
