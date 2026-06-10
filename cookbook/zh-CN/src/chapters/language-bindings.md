# 语言绑定

rust-widgets 通过模块化的绑定层向五种语言暴露其 API。本章涵盖 C ABI 基础、Python、C++、Java 和 Node.js 绑定，包括每种语言的构建说明和代码示例。

---

## 1. C ABI 层

C ABI 层是所有语言绑定的基础。它暴露了 102 个 `extern "C"` 函数，这些函数包装了 `Platform` 特质，提供对窗口部件创建、事件轮询、剪贴板和平台查询的语言中立访问。

### 字符串内存管理

跨越 FFI 边界的字符串遵循严格的所有权模型：

- **返回的字符串**：由 Rust 分配，必须通过 `rw_free_string()` 释放。
- **输入的字符串**：接受为 `*const c_char`（以 null 结尾的 C 字符串）。

```c
// C 头文件摘录 (rust_widgets.h)
typedef uint64_t ObjectId;

void rw_init(void);
void rw_run(void);
void rw_quit(void);

ObjectId rw_create_window(const char* title,
    int32_t x, int32_t y, uint32_t width, uint32_t height);

ObjectId rw_create_button(ObjectId parent, const char* text,
    int32_t x, int32_t y, uint32_t width, uint32_t height);

// ... 另外 96 个函数 ...

void rw_free_string(char* s);
```

### `c_try!` 模式

绑定层使用 `c_try!` 宏将 Rust 的 `Result` 类型安全地转换为 C 兼容的错误返回值：

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

### 完整的 C 示例

```c
#include "rust_widgets.h"
#include <stdio.h>

int main(void) {
    rw_init();

    ObjectId window = rw_create_window("C Demo", 100, 100, 800, 600);

    ObjectId button = rw_create_button(window, "Click Me",
        10, 10, 120, 32);

    char* text = rw_get_widget_text(button);
    printf("按钮文本: %s\n", text);
    rw_free_string(text);

    rw_run();
    rw_quit();
    return 0;
}
```

使用以下命令构建：
```sh
gcc -o demo demo.c -Ltarget/release -lrw_ffi -lpthread -ldl
```

---

## 2. Python 绑定

Python 绑定使用 `ctypes` 加载共享库，并将每个 C ABI 函数包装为 Pythonic 方法。`RustWidgets` 类暴露了 89 个方法，涵盖窗口部件创建、修改、菜单操作和平台查询。

### 安装

```python
# setup.py
from setuptools import setup, find_packages

setup(
    name="rust-widgets",
    version="0.9.6",
    packages=find_packages(),
    description="rust-widgets GUI 库的 Python 绑定",
)
```

### 使用示例

```python
from rust_widgets import RustWidgets

# 初始化
rw = RustWidgets()
rw.init()

# 创建窗口和窗口部件
window = rw.create_window("Python Demo", 100, 100, 800, 600)
button = rw.create_button(window, "Click Me", 10, 10, 120, 32)
label = rw.create_label(window, "状态: 空闲", 10, 60, 200, 24)

# 配置窗口部件
rw.set_widget_text(label, "状态: 就绪")
rw.set_widget_enabled(button, True)

# 带项目的组合框
combo = rw.create_combo_box(window, 10, 100, 200, 24)
rw.combo_box_add_item(combo, "选项 A")
rw.combo_box_add_item(combo, "选项 B")
rw.combo_box_set_current_index(combo, 0)

# 带项目的列表框
listbox = rw.create_list_box(window, 10, 140, 200, 120)
rw.list_box_add_item(listbox, "项目 1")
rw.list_box_add_item(listbox, "项目 2")
rw.list_box_remove_item(listbox, 0)

# 菜单栏
menu_bar = rw.create_menu_bar(window, 0, 0, 800, 24)
rw.attach_menu_bar_to_window(window, menu_bar)
file_menu = rw.create_menu(menu_bar, "文件", 0, 0, 60, 24)
new_id = rw.menu_add_item(file_menu, "新建", "Ctrl+N")
quit_id = rw.menu_add_item(file_menu, "退出", "Ctrl+Q")

# 带菜单轮询的事件循环
import time
while True:
    triggered = rw.poll_menu_triggered()
    if triggered is not None:
        if triggered == new_id:
            print("点击了新建！")
        elif triggered == quit_id:
            break
    time.sleep(0.016)  # ~60 FPS

rw.quit()
```

### Python API 表面（89 个方法）

| 类别 | 方法 |
|----------|---------|
| **生命周期** | `init`, `run`, `quit` |
| **窗口** | `create_window` |
| **窗口部件**（22 个创建方法） | `create_button`, `create_checkbox`, `create_line_edit`, `create_label`, `create_radio_button`, `create_slider`, `create_progress_bar`, `create_combo_box`, `create_list_box`, `create_panel`, `create_menu_bar`, `create_menu`, `create_tool_bar`, `create_status_bar`, `create_message_box`, `create_file_dialog`, `create_color_dialog`, `create_font_dialog`, `create_spin_box`, `create_list_view`, `create_scroll_area` |
| **窗口部件修改** | `set_widget_geometry`, `set_widget_text`, `get_widget_text`, `set_widget_enabled`, `is_widget_enabled`, `set_widget_visible`, `is_widget_visible`, `show_widget`, `hide_widget`, `set_widget_ime_enabled`, `is_widget_ime_enabled`, `set_widget_accessibility_name`, `get_widget_accessibility_name` |
| **组合框** | `combo_box_add_item`, `combo_box_clear_items`, `combo_box_set_current_index`, `combo_box_current_index`, `combo_box_item_count`, `combo_box_item_text` |
| **列表框** | `list_box_add_item`, `list_box_remove_item`, `list_box_clear_items`, `list_box_set_current_index`, `list_box_current_index`, `list_box_item_count`, `list_box_item_text` |
| **菜单** | `attach_menu_bar_to_window`, `menu_add_item`, `poll_menu_triggered`, `inject_menu_trigger` |
| **剪贴板** | `set_clipboard_text`, `get_clipboard_text` |
| **拖放** | `begin_drag`, `poll_drop_event` |
| **事件** | `poll_widget_triggered`, `poll_widget_trigger_event`, `inject_widget_trigger_event` |
| **平台** | `backend_name`, `platform_capabilities`, `platform_dpi_scale_factor`, `bindings_api_version` |

---

## 3. C++ 绑定

C++ 绑定是头文件式的，在 C ABI 之上提供 RAII 封装器和类层次结构。

### 关键类型

```cpp
// rust_widgets.hpp — 头文件式 C++ 绑定

#include <string>
#include <cstdint>
#include <memory>

using ObjectId = uint64_t;

// RAII 字符串封装器（通过 rw_free_string 释放）
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

// Widget 基类
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

### 22 个窗口部件子类

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

### TriggerKind 枚举

```cpp
enum class TriggerKind {
    Unknown = 0,
    Clicked = 1,
    ValueChanged = 2,
    SelectionChanged = 3,
    Closed = 4,
};
```

### 完整的 C++ 示例

```cpp
#include "rust_widgets.hpp"
#include <iostream>

int main() {
    rw_init();

    Window window("C++ Demo", 100, 100, 800, 600);

    Button button(window.id(), "Click Me", 10, 10, 120, 32);
    button.set_enabled(true);

    Label label(window.id(), "状态: 空闲", 10, 60, 200, 24);
    label.set_text("状态: 就绪");

    ComboBox combo(window.id(), 10, 100, 200, 24);
    combo.add_item("选项 A");
    combo.add_item("选项 B");
    combo.add_item("选项 C");
    combo.set_current_index(0);

    std::cout << "组合框项目数: " << combo.item_count() << std::endl;

    MenuBar menu_bar(window.id(), 0, 0, 800, 24);
    rw_attach_menu_bar_to_window(window.id(), menu_bar.id());

    Menu file_menu(menu_bar.id(), "文件", 0, 0, 60, 24);
    ObjectId new_id = rw_menu_add_item(file_menu.id(), "新建", "Ctrl+N");
    ObjectId quit_id = rw_menu_add_item(file_menu.id(), "退出", "Ctrl+Q");

    rw_run();
    rw_quit();
    return 0;
}
```

使用以下命令构建：
```sh
g++ -std=c++17 -o demo example.cpp -Ltarget/release -lrw_ffi -lpthread -ldl
```

---

## 4. Java 绑定

Java 绑定使用 JNI（Java Native Interface）将 Rust 库桥接到 JVM。`RustWidgets.java` 类声明了映射到 `java_jni.rs` 中 `extern "system"` 函数的 `native` 方法。

### JNI 桥接架构

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

### 字符串转换辅助函数

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

    // 生命周期
    public static native void nativeInit();
    public static native void nativeRun();
    public static native void nativeQuit();

    // 窗口部件创建
    public static native long nativeCreateWindow(String title,
        int x, int y, int width, int height);
    public static native long nativeCreateButton(long parent, String text,
        int x, int y, int width, int height);
    public static native long nativeCreateCheckBox(long parent, String text,
        int x, int y, int width, int height);
    // ... 所有 22 个窗口部件创建器 ...

    // 窗口部件修改
    public static native void nativeSetWidgetText(long id, String text);
    public static native String nativeGetWidgetText(long id);
    public static native void nativeSetWidgetEnabled(long id, boolean enabled);
    public static native boolean nativeIsWidgetEnabled(long id);
    public static native void nativeShowWidget(long id);
    public static native void nativeHideWidget(long id);
    public static native void nativeSetWidgetGeometry(long id,
        int x, int y, int width, int height);

    // 组合框
    public static native void nativeComboBoxAddItem(long id, String text);
    public static native void nativeComboBoxClearItems(long id);
    public static native void nativeComboBoxSetCurrentIndex(long id, int index);
    public static native int nativeComboBoxCurrentIndex(long id);
    public static native int nativeComboBoxItemCount(long id);
    public static native String nativeComboBoxItemText(long id, int index);

    // 列表框
    public static native void nativeListBoxAddItem(long id, String text);
    public static native void nativeListBoxRemoveItem(long id, int index);
    public static native void nativeListBoxClearItems(long id);
    public static native void nativeListBoxSetCurrentIndex(long id, int index);
    public static native int nativeListBoxCurrentIndex(long id);
    public static native int nativeListBoxItemCount(long id);
    public static native String nativeListBoxItemText(long id, int index);

    // 菜单
    public static native void nativeAttachMenuBarToWindow(long window, long menuBar);
    public static native long nativeMenuAddItem(long menu, String text, String shortcut);
    public static native Long nativePollMenuTriggered();

    // 剪贴板
    public static native void nativeSetClipboardText(String text);
    public static native String nativeGetClipboardText();

    // 平台信息
    public static native String nativeBackendName();
    public static native int nativePlatformCapabilities();
    public static native int nativeBindingsApiVersion();
}
```

### 完整的 Java 演示

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
            window, "状态: 空闲", 10, 60, 200, 24);

        System.out.println("后端: " + RustWidgets.nativeBackendName());

        // 组合框
        long combo = RustWidgets.nativeCreateComboBox(
            window, 10, 100, 200, 24);
        RustWidgets.nativeComboBoxAddItem(combo, "Java 选项 1");
        RustWidgets.nativeComboBoxAddItem(combo, "Java 选项 2");
        RustWidgets.nativeComboBoxSetCurrentIndex(combo, 0);
        System.out.println("组合框项目数: " +
            RustWidgets.nativeComboBoxItemCount(combo));

        // 剪贴板
        RustWidgets.nativeSetClipboardText("从 Java 复制！");
        String clip = RustWidgets.nativeGetClipboardText();
        System.out.println("剪贴板: " + clip);

        RustWidgets.nativeRun();
        RustWidgets.nativeQuit();
    }
}
```

### 构建 Java 绑定

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

## 5. Node.js 绑定

Node.js 绑定使用 `ffi-napi` 和 `ref-napi` 加载原生库并直接从 JavaScript 调用 C ABI 函数。

### 包配置

```json
{
  "name": "rust-widgets",
  "version": "0.9.6",
  "description": "rust-widgets GUI 库的 Node.js 绑定",
  "main": "index.js",
  "dependencies": {
    "ffi-napi": "^4.0.3",
    "ref-napi": "^3.0.3"
  }
}
```

### 单例模式 (index.js)

```javascript
const ffi = require('ffi-napi');
const ref = require('ref-napi');

// 类型别名
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
            // ... 所有 102 个函数 ...
        });

        _instance = this;
    }

    // Buffer 到字符串的转换辅助函数
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

### Node.js 示例

```javascript
const RustWidgets = require('rust-widgets');

async function main() {
    const rw = new RustWidgets('../target/release/librw_ffi.so');

    rw.init();

    const window = rw.createWindow('Node.js Demo', 100, 100, 800, 600);
    const button = rw.createButton(window, 'Click Me', 10, 10, 120, 32);
    const label = rw.createLabel(window, '状态: 空闲', 10, 60, 200, 24);

    console.log('后端:', rw.backendName());
    console.log('按钮文本:', rw.getWidgetText(button));

    // 组合框
    const combo = rw.createComboBox(window, 10, 100, 200, 24);
    rw.comboBoxAddItem(combo, 'Node 选项 1');
    rw.comboBoxAddItem(combo, 'Node 选项 2');
    rw.comboBoxSetCurrentIndex(combo, 1);

    // 剪贴板
    rw.setClipboardText('来自 Node.js 的问候！');
    console.log('剪贴板:', rw.getClipboardText());

    // 事件循环
    setInterval(() => {
        const triggered = rw.pollMenuTriggered();
        if (triggered) {
            console.log('菜单触发:', triggered);
        }
    }, 16);

    // rw.run();  // 阻塞直到退出
}

main().catch(console.error);
```

---

## 6. 构建每种绑定

### C ABI（共享库）

```sh
cargo build --release
# 生成: target/release/librw_ffi.{so,dylib,dll}
```

### Python

```sh
pip install ctypes  # （标准库，无需安装）
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
# 编译 Rust JNI 库 + Java 类 + 运行演示
```

### Node.js

```sh
cd bindings/nodejs
npm install
node example.js
```

---

## 7. API 版本跟踪

每种绑定都暴露一个版本查询以确保兼容性：

```rust
// C ABI
pub extern "C" fn rw_bindings_api_version() -> u32 {
    1 // 在破坏性 ABI 更改时递增
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

### 绑定状态检查

```rust
pub extern "C" fn rw_python_binding_status() -> u32 { 1 }
pub extern "C" fn rw_cpp_binding_status() -> u32 { 1 }
pub extern "C" fn rw_java_binding_status() -> u32 { 1 }
pub extern "C" fn rw_java_jni_skeleton_version() -> u32 { 1 }
```

### 平台能力位掩码（C ABI）

```rust
pub extern "C" fn rw_platform_capabilities(caps: *mut u32) {
    // 返回一个位掩码：
    //   bit 0: dpi_scaling
    //   bit 1: ime
    //   bit 2: accessibility
    //   bit 3: native_menu
    //   bit 4: typed_widget_trigger
}
```

---

## 8. 跨绑定功能矩阵

| 功能 | C | Python | C++ | Java | Node.js |
|---------|:---:|:---:|:---:|:---:|:---:|
| **窗口部件创建**（22 种） | ✅ | ✅ | ✅ | ✅ | ✅ |
| **窗口部件修改**（13 个方法） | ✅ | ✅ | ✅ | ✅ | ✅ |
| **组合框**（6 个方法） | ✅ | ✅ | ✅ | ✅ | ✅ |
| **列表框**（7 个方法） | ✅ | ✅ | ✅ | ✅ | ✅ |
| **菜单系统**（4 个方法） | ✅ | ✅ | ✅ | ✅ | ✅ |
| **剪贴板**（2 个方法） | ✅ | ✅ | ✅ | ✅ | ✅ |
| **拖放**（2 个方法） | ✅ | ✅ | ✅ | ✅ | ✅ |
| **带类型触发**（6 个方法） | ✅ | ✅ | ✅ | ✅ | ✅ |
| **平台查询**（4 个方法） | ✅ | ✅ | ✅ | ✅ | ✅ |
| **移动端 API**（2 个方法） | ✅ | ✅ | ✅ | ✅ | ✅ |
| **嵌入式引擎**（5 个方法） | ✅ | ✅ | ✅ | ✅ | ✅ |
| **渲染配置**（4 个方法） | ✅ | ✅ | ✅ | ✅ | ✅ |

### HarmonyOS 特定扩展

C ABI 包含用于 NAPI 桥接集成的 HarmonyOS 特定函数：

```c
// 窗口部件触发注入（Harmony 事件桥接）
void rw_harmony_on_menu_item(ObjectId widget_id);
void rw_harmony_on_click(ObjectId widget_id);
void rw_harmony_on_value_changed(ObjectId widget_id);
void rw_harmony_on_widget_event(ObjectId widget_id, int trigger_kind);

// 节点绑定注册表
void rw_harmony_bind_node(ObjectId widget_id, const char* node_id);
void rw_harmony_unbind_node(ObjectId widget_id);

// 带类型的节点事件（harmony）
void rw_harmony_on_node_click(ObjectId widget_id, const char* node_id);
void rw_harmony_on_node_value_changed(ObjectId widget_id, const char* node_id);
void rw_harmony_on_node_widget_event(ObjectId widget_id,
    const char* node_id, int trigger_kind);
```

---

## 9. 跨绑定内存管理

| 绑定 | 字符串返回 | 生命周期 | 释放机制 |
|---------|:---:|----------|----------------|
| **C** | `char*`（堆） | 直到 `free_string` | `rw_free_string()` |
| **Python** | `str` | 立即复制 | `ctypes` 复制到 Python 字符串 |
| **C++** | `RustString`（RAII） | 作用域绑定 | 析构函数调用 `free_string` |
| **Java** | `String` | 立即复制 | JNI 复制到 Java `String` |
| **Node.js** | `Buffer` | 直到读取 | `ref.readCString()` 然后 `free_string` |

核心原则：**Rust 分配，调用者释放**。每种绑定都将其包装为符合语言习惯的模式——C++ 中的 RAII、Java 中的析构函数/`finally`、C 中手动释放、Python 和 Node.js 中自动复制。
