# BLUE28 — 全平台平台层完整性补齐

> **来源**：用户要求「还有要补充的吗？完整的全平台」。
>
> **证据基线**：2026-10-19 收盘的工作树（`apple_uikit`、`unix_desktop`、安全区/主题/文字缩放、
> Android 12 位探针均已在树内；见 [`../log/log-20261019-1.md`](../log/log-20261019-1.md) §10–§11）。
> 本文是**提案**，不是完成声明：所有条目均为 `[ ]`，每一条都附当前 `file:function` 证据与可执行的
> 验收条件。行号会随实现漂移，引用以函数/模块名为准。
>
> **判定口径**（沿用 `principle.md` #1/#2/#37/#103）：
> **运行** > **CI 运行** > **交叉检查（含依赖类型检查）** > **仅类型检查** > **未验证**。
> 没有来源的事实一律返回 `None`/`Unsupported`，不写“看起来合理”的常量。

---

## 0. 范围

### 0.1 目标

让每一个受支持的平台后端，对 `Platform` 的每个事实要么给出**真实来源**，要么给出**具名的
“不适用/读不到”**，并且同一套探针能在该平台上复跑。覆盖：

| 家族 | 目标三元组（代表） | 当前后端 |
|---|---|---|
| Windows | `x86_64-pc-windows-msvc`、`aarch64-pc-windows-msvc` | `WindowsPlatform` |
| macOS | `x86_64`/`aarch64-apple-darwin` | cocoa（`desktop`）与 objc2（`macos`） |
| Linux X11 | `x86_64`/`aarch64-unknown-linux-gnu`、musl | `LinuxPlatform`（GTK） |
| Wayland | 同上（`WAYLAND_DISPLAY`） | `WaylandPlatform` |
| BSD | `x86_64-unknown-freebsd`、`netbsd` | `unix_desktop`（GTK/Wayland） |
| Android | 4 ABI | `AndroidPlatform` + `android-jni` |
| HarmonyOS | `aarch64/armv7/x86_64-unknown-linux-ohos` | `HarmonyPlatform` + XComponent |
| iOS / tvOS / visionOS | `aarch64-apple-{ios,tvos,visionos}`（含 Simulator） | `IosMobilePlatform`（`apple_uikit`） |
| Redox | `x86_64`/`aarch64-unknown-redox` | `OrbitalPlatform` |
| Web | `wasm32-unknown-unknown`（浏览器） | `WasmPlatform` + canvas |
| Portable / Embedded | 任意宿主 | `portable` / `stub` |

### 0.2 已完成基线（**不要在本计划重复实现**）

| 事实 | 现状 | 证据 |
|---|---|---|
| UIKit 家族选择（iOS/tvOS/visionOS/Catalyst） | `apple_uikit` 别名；tvOS/visionOS 已真实 `cargo check` | `build.rs::declare_cfg_aliases`；日志 §10.2 |
| BSD 后端 | `unix_desktop`；115 处 `target_os = "linux"` 已收口；FreeBSD/NetBSD GTK+Wayland 检查通过 | `build.rs`；`tools/check_bsd_cross.sh` |
| 显示器/窗口归属/变更信号 | Android/Harmony/iOS/cocoa/**objc2**/GTK/Windows/Redox 已实现 | 日志 §10.3 |
| 系统主题 | Windows/macOS 双后端/iOS 家族/Android | 日志 §10.3；Android 探针 bit 9 |
| 安全区 | iOS 家族、macOS 双后端、Android；Windows/GTK/Wayland/Orbital 返回 `Some(0)` | 日志 §10.3；Android 探针 bit 11 |
| 文字缩放 | Windows `TextScaleFactor`、Android `fontScale` | 探针 bit 10 |
| Android 运行探针 | 12 位 `nativePlatformProbe`，模拟器 `4095` 12/12 | `src/platform/android_jni.rs`；日志 §11 |

---

## 1. 全平台缺口矩阵

**图例**：✅ 已实现（该平台有真实来源） · 🟡 部分（缺一个维度，或只有交叉/类型检查证据） ·
❌ 未实现 · ➖ 该平台无此概念或本文判定为非目标 · ❓ 实现存在但运行证据缺失。

| 能力 | Win | macOS cocoa | macOS objc2 | GTK/X11 | Wayland | BSD | Android | Harmony | iOS 家族 | Redox | WASM |
|---|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| 窗口 + 绘制表面 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 宿主输入转发 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ | ✅ | ❌ |
| 显示器枚举/归属/变更 | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| DPI | ✅ | ❌ | ❌ | ✅ | 🟡 环境变量 | 🟡 | ✅ | ✅ | ✅ | ✅ | ❌ |
| 系统主题 | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | ✅ | ❌ | ❌ |
| 安全区 | ✅ 0 | ✅ | ✅ | ✅ 0 | ✅ 0 | ✅ 0 | ✅ | ❌ | ✅ | ✅ 0 | ❌ |
| 文字缩放 | ✅ | ➖ | ➖ | ➖ | ➖ | ➖ | ✅ | ❌ | ❌ | ➖ | ➖ |
| 剪贴板（文本+MIME） | ✅ | ✅ | ✅ | ✅ | 🟡 仅进程内 | ✅ | ✅ | ✅ | ✅ | 🟡 仅文本 | ❌ |
| IME 桥 | ✅ | ✅ | ❌ | ✅ | ❌ | ✅ | ✅ | 🟡 直接按键 | ❌ | ❌ | ❌ |
| 无障碍桥 | ✅ | ✅ | ❌ | ✅ | ❌ | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| 文件选择 | ✅ | ✅ | ❌ | ✅ | ❌ | ✅ | 🟡 独立 JNI 入口 | ❌ | ❌ | ❌ | ➖ |
| 外部打开 | ✅ | ✅ | ❌ | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ | ➖ | ❌ |
| 高对比/减少动效/语言 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 光标意图/形状 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 原生窗口句柄 | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ |
| 挂起/恢复 | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ | ➖ |

### 1.1 取证说明（为什么是这些格子）

- **Wayland 显示器/DPI/剪贴板**：`src/platform/wayland/platform_impl.rs` 只有
  `safe_area_insets`（返回 0）与 `dpi_scale_factor`（读 `GDK_SCALE`/`QT_SCALE_FACTOR`/
  `RUST_WIDGETS_DPI_SCALE` 环境变量），没有 `displays`/`display_of_window`/
  `display_generation`；`clipboard` 能力显式为 `false`，剪贴板方法只写 `BackendState`。
- **macOS DPI**：`src/platform/macos/platform_impl.rs::capabilities` 与
  `src/platform/macos_objc2/platform_impl.rs::capabilities` 都写 `dpi_scaling: false`，且两者都没有
  `dpi_scale_factor` 覆盖。AppKit 的 `NSScreen.backingScaleFactor` / `NSWindow.backingScaleFactor`
  可用，只是没接。
- **GTK/BSD 主题**：`src/platform/linux/platform_impl.rs` 没有 `system_theme` 覆盖；BSD 复用同一代码。
- **HarmonyOS**：`capabilities`/trait 实现里只有 `displays`（`platform_impl.rs`）与 UDMF 剪贴板；
  没有 `system_theme`、`safe_area_insets`、`text_scale`。SDK 有
  `OH_WindowManager_GetWindowAvoidArea` 与 `OH_ArkUI_RegisterSystemColorModeChangeEvent`，但前者需要
  ArkTS 侧 window id，后者是回调、不是同步查询。
- **Android/iOS 输入转发**：`src/platform/android/status.md` 与 `src/platform/ios/status.md` 都明确写
  “库侧 `dispatch_pointer_event` 已实现，宿主 `onTouchEvent`/`touchesBegan` 尚未转发”。这是
  “画得出来但点不动”的运行时缺口。
- **WASM**：`src/platform/wasm/platform_impl.rs::capabilities` 全 false；`create_window` 只设置
  `<canvas>` 尺寸，没有 `dpi_scale_factor`/`displays`/`system_theme`/`safe_area_insets`。表面表
  （`supports_surfaces`）和 `ResizeObserver` 已有，缺的是平台事实与事件入口。
- **环境事实**：`src/style/environment.rs::PlatformEnvironment` 只转发 `text_scale()` 与
  `color_scheme()`；`high_contrast`/`motion_preference`/`locale`/`text_direction` 仍是中性默认。
- **文件/外部打开**：`choose_path` 只在 Windows（`windows/platform_impl.rs`）、Linux
  （`linux/platform_impl.rs`）、cocoa（`macos/platform_impl.rs`）实现；objc2、Wayland、Android、
  Harmony、iOS 都没有。Android 的 `launch_file_dialog` 是独立 JNI 入口，不是 `choose_path` 契约。
  `open_external` 在 Wayland/objc2/Android/Harmony/iOS/WASM 未实现（Redox 诚实返回 `Unsupported`）。
- **探针**：`nativePlatformProbe` 只存在于 `src/platform/android_jni.rs`；其它平台没有同等入口，
  所以“同一组事实在设备上复跑”的能力目前只有 Android 有。

### 1.2 `Platform` 166 个方法的逐项审计

`src/platform/types.rs` 的 `Platform` trait 共 **166** 个方法，其中：

| 分类 | 数量 | 说明 |
|---|---:|---|
| 必需（无默认体） | 6 | `backend_name`、`family`、`init`、`run`、`quit`、`create_window` |
| 有默认体 | 160 | 本文审计对象 |

160 个默认方法中：

| 组 | 数量 | 默认值的含义 | 是否缺口 |
|---|---:|---|---|
| `create_*` | 50 | 返回 `0`（没有创建原生控件） | **不是**：生产路径是 `CustomPaintControlBackend`，库自绘每个 `WidgetKind`（`control_backend/dispatcher.rs::get_control_backend`） |
| combo/list 数据 | 14 | 返回 `false`/`None`/`0` | **不是**：公开 API 走 `ControlBackend`（`lib.rs::combo_box_add_item` 等） |
| menu 构造/查询 | 7 | 返回 `0`/`false`/`None` | **不是**：同一原因；仅 `poll_*`/`inject_*`/触发队列仍由平台后端使用 |
| 状态属性宏 | 52 | 写 `BackendState` | **普通控件不是**（自绘无原生对象）；其中影响真实窗口的那几个见 §1.3/B28.19 |
| 家族/能力/格式契约 | 8 | 按 family/平台推断，或格式化的语义答案 | **不是** |
| **事实 / 服务 / 窗口** | **29** | `None`/`false`/`0` | **逐条判定**（§1.3）：有的需要补，有的事设计边界 |

> 这张分组表的作用是阻止把“自绘架构下本来就该返回 0”的 50 个 `create_*` 误登记成 50 个缺陷。
> 反过来说，§1.3 的 29 个方法里，默认值不是设计，而是“这个后端还没接”。

### 1.3 事实 / 服务 / 窗口方法的缺口（逐方法）

下表只列**至少一个后端没有覆盖**的方法；“缺失后端”是静态审计结果，已扣除宏覆盖。

| 方法 | 缺失后端 | 判定 |
|---|---|---|
| `dpi_scale_factor` | macOS cocoa、macOS objc2、WASM | 需要（B28.03/B28.05） |
| `displays` | Wayland、WASM | 需要（B28.04/B28.05） |
| `display_of_window` | Wayland、WASM | 需要（B28.04/B28.05） |
| `display_generation` | Wayland、WASM | 需要（B28.04/B28.05） |
| `safe_area_insets` | HarmonyOS、WASM | 需要（B28.07/B28.05） |
| `text_scale` | iOS、HarmonyOS | 需要（B28.10/B28.07）；macOS/GTK/BSD/Redox/WASM 没有系统文字缩放概念，保持 `1.0` |
| `system_theme` | GTK/BSD、Wayland、HarmonyOS、Redox、WASM | 需要（B28.04/B28.05/B28.06/B28.07）；Redox 可诚实 `None` |
| `clipboard_backend` | Wayland、Redox、WASM | Wayland 需要（B28.11）；Redox 可只做文本；WASM 保持 `None`（异步边界） |
| `clipboard_text` | macOS cocoa、WASM | cocoa 需要（区分“无文本”与“不是文本”）；WASM 保持 `None` |
| `ime_bridge` | objc2、Wayland、HarmonyOS、iOS、Redox、WASM | objc2/iOS/Harmony 需要（B28.12）；其余可 `None` |
| `accessibility_bridge` | objc2、Wayland、Android、iOS、Redox、WASM | 需要（B28.13），按平台优先级 |
| `open_external` / `has_external_handler` | objc2、Wayland、Android、HarmonyOS、iOS、WASM | 需要（B28.08）；Redox 诚实 `Unsupported` |
| `choose_path` | objc2、Wayland、Android、HarmonyOS、iOS、Redox、WASM | 需要（B28.08，先做异步契约决策） |
| `get_native_handle` | objc2、GTK、Wayland、Android、HarmonyOS、iOS、Redox、WASM | 需要（B28.19）；它是宿主集成/测试的逃生口 |
| `window_client_size` | macOS objc2 | 需要（B28.19） |
| `queue_resize_trigger` | macOS objc2 | 需要（B28.19） |
| `set_window_state` / `is_window_in_state` | GTK 默认 `false`；cocoa/objc2/Android/Harmony/iOS/Wayland/Redox/WASM 仅记录 | 需要（B28.19）：要么真正应用，要么显式 `Unsupported` |
| `set_window_min_size` / `window_min_size` | GTK 默认 `false`；cocoa/objc2/Android/Harmony/iOS/Wayland/Redox/WASM 仅记录 | 需要（B28.19） |
| `set_window_icon` / `window_icon` | 仅 Windows 真正应用；其余仅记录或默认 | 需要（B28.19）：GTK 可应用；AppKit/mobile 的图标是 bundle/manifest 级，应显式说明 |
| `menu_item_shortcut` | 仅 cocoa | **不是生产缺口**：公开菜单 API 走 `ControlBackend`；若恢复 native-menu 模式再补 |
| `invalidate_surface_rect` | 多数后端 | **不是**：文档明确默认 `false` 表示“不能收窄”，调用方回退到整控件重绘 |
| `take_pending_repaint` | Windows/cocoa/GTK/Redox | **不是**：这些后端直接失效真实窗口；只有表面表后端需要队列 |
| `supports_pen_input` / `route_pointer_event` | 全部（默认） | **不是**：默认即“无笔设备”/库内路由；笔数据由宿主构造指针事件 |
| `supports_web_engine` | 全部 | **不是**：Web 引擎已被 BLUE20 删除，默认 `false` 是设计 |
| `attach_menu_bar_to_window` | 全部 | **不是生产缺口**：公开 API 走 `ControlBackend`（`lib.rs:2284`） |
| `total_memory_mb` / `is_on_battery` | 无 | 已覆盖；不可读时返回 `None`/`false` |
| `process_memory_utilization` | Redox | 可保持 `None`（无 `/proc`） |
| `process_cpu_utilization` | Redox、WASM | 可保持 `None` |
| `has_print_support` | Android、iOS、Redox | 需要（B28.27）：Android `PrintManager`、iOS `UIPrintInteractionController`；Redox 诚实 `false` |

### 1.4 trait 之外的平台能力

以下是平台层常见、但当前 `Platform` **没有接缝**的事实。每条都给出“库内是否已有消费者”，
避免把“winit 有”当成“本库必须现在有”。

| 能力组 | 具体项 | 库内消费者 | 判定 |
|---|---|---|---|
| 窗口属性 | 装饰/可调整大小/置顶/透明度/焦点与激活请求/任务栏闪烁/窗口位置查询 | `app::WindowHandle` 只覆盖 state/min-size/icon；其余无 API | 需要决策（B28.20） |
| 窗口标题更新 | `WindowHandle::set_title` 只写控件属性，不更新 OS 标题 | 有消费者（`app/handle.rs:1356`） | 需要（B28.19） |
| 窗口生命周期/移动 | 关闭事件、焦点获得/丢失、移动、激活 | `Closed`/`Resized` 已有，覆盖不全 | 需要（B28.32） |
| 显示器元数据 | 刷新率、方向、色彩空间/HDR、DPI 变化通知、每窗口 scale、窗口位置 | `Display` 只有 id/矩形/scale/primary；`dpi_scale_factor` 是全局值 | 需要（B28.21） |
| 光标 | 形状、位置、捕获、隐藏 | 无控件意图模型 | 需要（B28.09） |
| 剪贴板 | 图片、文件列表、变更通知 | 只有文本/HTML/自定义 MIME | 需要（B28.22） |
| 拖放 | 文件拖放、MIME 协商、源/目标角色 | `begin_drag`/`poll_drop_event` 只有字节载荷 | 需要（B28.23） |
| 移动设备服务 | 方向锁、生命周期、死键/重复、软键盘、安全区、文字缩放；应用服务见 B28.44 | 核心 GUI 子集无接缝；应用服务非核心 | 需要（B28.24）；应用服务 P3（B28.44） |
| 桌面集成 | 托盘、系统通知、全局快捷键、单实例、Dock/任务栏徽标与进度 | 无 | **非核心**（B28.25，P3 可选） |
| Web 集成 | 页面可见性、全屏、指针锁；权限/存储/PWA 属页面宿主 | 无 | 可见性必需（B28.26）；其余可选/非核心 |
| 电源/热/网络 | 低电量、热状态、网络可达性 | 只有 `is_on_battery` | **非核心**（B28.28，P3 提示）；网络不做 |
| 打印 | 平台打印对话框/任务 | 只有 `spawn_print_job`/`has_print_support` | 条件必需（B28.27） |
| 环境事实 | 高对比、减少动效、语言、文本方向、强调色 | `EnvironmentProvider` 已有前四项，但 `PlatformEnvironment` 只转发 2/8；强调色在主题里已有 token，无系统来源 | 需要（B28.06） |
| 输入细节 | 原始输入、键盘布局、死键、重复、触摸/笔压、手势、指针锁 | 指针/键盘事件已有；其余无 | 需要决策（B28.20/B28.24） |
| 系统字体枚举/加载 | 系统字体列表 + 字体字节 | `runtime-fonts` 已支持 `register_face`，但没有任何平台枚举入口；`face_for_family("Arial")` 默认返回 `None` | **需要**（B28.39）；这是本项目特有的缺口，不是 winit 能力 |
| 屏幕捕获/录制 | — | 无消费者 | **不是目标** |
| 原生句柄（raw-window-handle） | rwh 0.6 window/display handle | 只有不透明的 `get_native_handle` | **需要**（B28.40，只读适配层） |

### 1.5 winit 能力清单对照（尽量有）

对照 `/home/mikeli/workspace/winit/winit-core/src/window.rs` 的公开方法。原则：**winit 有、且与本项目的窗口/控件模型
相关 → 进入计划；与“库拥有窗口并绘制”的架构直接冲突 → 只做只读适配或明确不做**。

| winit 能力 | winit API（代表） | 本项目现状 | 计划项 |
|---|---|---|---|
| 标题（读写） | `set_title` / `title` | 创建时设置；无 setter/getter，更新不推 OS | B28.19 |
| 表面尺寸/外框尺寸 | `surface_size` / `outer_size` | `window_client_size`（objc2 缺）；外框尺寸无 | B28.19/B28.21 |
| 位置（读写） | `outer_position` / `set_outer_position` | 创建时传 x/y；无读写 | B28.21 |
| 最小/最大尺寸 | `set_min/max_surface_size` | 有最小（GTK/cocoa 未应用）；无最大 | B28.19/B28.20 |
| 尺寸增量 | `surface_resize_increments` | 无 | B28.20 |
| 可调整大小 | `set_resizable` / `is_resizable` | 无 | B28.20 |
| 装饰 | `set_decorations` / `is_decorated` | 无 | B28.20 |
| 窗口按钮集合 | `set_enabled_buttons` | 无 | B28.20 |
| 最小化/最大化/全屏 | `set_minimized` / `set_maximized` / `set_fullscreen` | 只有 `WindowStateFlag`，且多后端仅记录 | B28.19 |
| 可见性 | `set_visible` / `is_visible` | 已应用（含窗口特例） | ✅ |
| 透明/模糊 | `set_transparent` / `set_blur` | 无 | B28.20 |
| 窗口层级 | `set_window_level` | 无 | B28.20 |
| 窗口类型/父子/弹出 | `WindowType`、`with_parent_window`、`WindowPositioner` | 无 | B28.41 |
| 多窗口 | 多次 `create_window` | 桌面/Redox 可；Android/Harmony/WASM 受单 Activity/canvas 限制，未声明 | B28.43 |
| 窗口图标 | `set_window_icon` | 仅 Windows | B28.19 |
| 主题（每窗口） | `set_theme` / `theme` | 只有全局 `system_theme()` | B28.06/B28.20 |
| 内容保护 | `set_content_protected` | 无 | B28.20 |
| 焦点/激活 | `focus_window` / `has_focus` | 只有控件焦点 | B28.20 |
| 用户注意请求 | `request_user_attention` | 无 | B28.20 |
| 光标形状/位置/捕获/可见/命中测试 | `set_cursor` / `set_cursor_position` / `set_cursor_grab` / `set_cursor_visible` / `set_cursor_hittest` | 无 | B28.09 |
| 拖动窗口/缩放窗口/系统窗口菜单 | `drag_window` / `drag_resize_window` / `show_window_menu` | 无（无自定义标题栏 API） | B28.20 |
| IME 允许/光标区/用途/请求/能力 | `set_ime_allowed` / `set_ime_cursor_area` / `set_ime_purpose` / `request_ime_update` / `ime_capabilities` | 有 focus/commit/composition/candidate position；缺 allowed/purpose/capabilities | B28.12 |
| 显示器 | `current_monitor` / `available_monitors` / `primary_monitor` | `displays` / `display_of_window` / `display_generation` | ✅（Wayland/WASM 见 B28.04/05） |
| 每窗口 scale | `scale_factor` | 全局 `dpi_scale_factor`；可从 display 推导 | B28.21 |
| 安全区 | `safe_area` | 已实现（见 §0.2） | ✅（Harmony/WASM 见 B28.07/05） |
| 原生句柄 | rwh_06 `window_handle` / `display_handle` | 只有不透明 handle | B28.40 |
| 请求重绘/呈现前通知 | `request_redraw` / `pre_present_notify` | 有 invalidate/repaint 队列；无 pre-present | B28.42 |
| 设备事件（电源/媒体键等） | `listen_device_events` | 无 | B28.28 |
| 生命周期（挂起/恢复） | Android `onPause/onResume`、iOS 生命周期 | 无 | B28.24 |
| 死键重置 | `reset_dead_keys` | 无 | B28.24 |
| 事件循环代理/唤醒 | `EventLoopProxy` | `platform::main_thread::schedule_in` + `stop` | ✅ |
| 剪贴板 | **winit 不提供** | 内建（见 §0.2） | ✅ + B28.22 |
| 字体枚举 | **winit 不提供** | `runtime-fonts` 可注册，但无枚举 | B28.39（本项目特有） |

> 说明：winit 的 `rwh_06` 是宿主做 GPU 互操作的标准入口；本项目的 `get_native_handle` 是不透明整数，
> 两者不是同一事实。B28.40 做**只读**适配，不改变“库拥有窗口并绘制”的契约。

### 1.6 跨平台 GUI 库的职责边界（本轮评估）

判定标准只有一条：**控件库必须让“窗口里的控件正常工作”这件事在平台间一致**；
应用外壳/业务服务不属于控件库。下表逐类给出结论，“必需”的条目已进入计划，“非核心”的
保留为可选扩展或明确不做，不再占用 P0/P1。

| 类别 | 是否需要 | 结论 | 计划项 |
|---|---|---|---|
| 窗口属性/生命周期 | **必需（核心）** | 标题、尺寸/位置、最小/最大、可调整、装饰、全屏/最大化/最小化、可见性、图标、关闭/焦点/移动事件，是任何 GUI 库的基础；winit 的对应 API 全在核心面 | B28.19/20/32/41/43 |
| 显示器元数据 | **必需（核心）+ 可选（进阶）** | 枚举、work area、scale、窗口归属、变更信号、每窗口 scale 必需；刷新率/方向在动画与移动端布局中必需；HDR/色彩空间对当前 RGBA8 自绘渲染没有消费者，列为可选 | B28.21（HDR 标 P3） |
| 光标 | **必需（形状/可见）+ 可选（其余）** | I-beam/pointer/resize/隐藏是控件可用性的一部分；位置/捕获/命中测试/自定义图像是进阶能力 | B28.09 |
| 剪贴板富内容 | **必需（文本/HTML/MIME/图片/文件）** | 控件库的复制/粘贴要对常见内容成立；变更通知是可选（剪贴板管理器类应用才需要） | B28.22（通知 P2） |
| 拖放文件 | **桌面必需；移动可选** | 文件拖入是桌面 GUI 的标准期望；移动端的系统拖放优先级低 | B28.23（移动部分 P3） |
| 移动设备服务 | **核心子集必需；应用服务非核心** | 方向、生命周期、软键盘、安全区、文字缩放、焦点属于 GUI；触觉/权限/通知/分享/深链/传感器/定位属于应用 | B28.24（核心）+ B28.44（可选扩展） |
| 桌面集成 | **非核心** | 托盘/系统通知/全局快捷键/单实例/Dock 或任务栏徽标属于应用外壳；winit 也不提供 | B28.25 改为 P3 可选 |
| Web 集成 | **核心子集必需；应用层非核心** | 页面可见性、DPR、安全区、输入转发属于 GUI；全屏/指针锁可选；权限/存储/PWA 属于页面宿主 | B28.26（核心）+ P3 可选 |
| 电源/热/网络 | **非核心** | `is_on_battery` 已存在；低电量/热状态可作为动画策略输入（可选）；网络可达性完全是应用层 | B28.28 改为 P3 可选 |
| 打印 | **条件必需** | 库已有 `print`/`PrintContext`/`has_print_support`，既然对外提供打印能力，各平台就应补齐或诚实 `false` | B28.27 |
| 输入细节 | **必需（死键/重复/触摸/笔压）+ 可选（布局/指针锁）；原始输入非目标** | 文本输入正确性依赖死键与重复；触摸/笔压是已有控件（SignaturePad 等）的真实需求；原始输入会引入权限与安全面 | B28.24 + B28.09；B28.02 |
| 系统字体枚举 | **必需（本项目特有）** | `runtime-fonts` 已能注册字体，但 FontComboBox/FontDialog 没有字体来源；见 B28.39 的证据 | B28.39 |
| 屏幕捕获 | **非核心；可选“捕获自身窗口”** | 系统级屏幕捕获需要平台权限（TCC/Graphics Capture/Wayland portal），是应用功能；库只需要已有的 `render_frame`/SVG 快照用于测试。若做，只做“捕获本库窗口” | §4 非目标 + 可选说明 |

**结论**：核心补齐的范围是 B28.19–B28.24（窗口/显示器/光标/剪贴板/拖放/移动核心）、
B28.27（打印，条件）、B28.32（生命周期事件）、B28.39（字体）、B28.40（rwh 互操作）。
B28.25/26/28/44 属于应用外壳，保留为可选 P3，不阻塞“全平台 GUI 完整”。

---

## 2. P0 — 正确性与可验证性阻塞项

### B28.01 统一的平台诊断探针（先做）

**问题**：只有 Android 有 `nativePlatformProbe`；其它平台的能力只能靠交叉检查或肉眼。
没有统一探针，“全平台完整”不可判定。

**交付**：

1. 在库内定义稳定的诊断位（与平台无关），例如
   `platform::diagnostics() -> PlatformDiagnostics`，位序固定：
   `clipboard/custom MIME`、`displays`、`display_of_window`、`display_generation`、`dpi`、
   `system_theme`、`text_scale`、`safe_area`、`ime`、`a11y`、`file_dialog`、`open_external`。
2. 导出 C ABI `rw_platform_diagnostics()` 与 JSON 形式（供不能调用 C 的宿主读取原因）。
3. 每平台提供薄 harness：Android（复用现有 Java）、iOS ObjC、Harmony NAPI、桌面示例、
   Redox 示例、WASM JS。harness 只翻译，不重复断言逻辑。
4. 门禁 `tools/check_platform_probe.sh`：源码里每个 bit 的文档、C 导出与各宿主声明必须一致；
   与 `check_jni_signatures.sh` 同样的“声明 ↔ 导出”思路。

**验收**：Android 模拟器/真机、GTK/X11、Wayland（weston）、macOS（CI）、iOS Simulator（CI）、
Redox/BSD（人工）各自输出同一份 `RESULT: PASS`；不支持的事实以“SKIP + 原因”出现，不计失败。

**证据**：`src/platform/android_jni.rs::Java_rust_1widgets_RustWidgets_nativePlatformProbe`。

### B28.02 Android 与 iOS 的宿主输入转发

**问题**：库侧 `widget::runtime::dispatch_pointer_event` / `dispatch_event` 已有并有测试，但两个
移动宿主的触摸/按键回调没有接进去，导致自绘控件“看得见、点不动”。

**交付**：

- Android：`MainActivity.onTouchEvent` / `onKeyDown`（或等价的 View 回调）经 JNI 调用
  `dispatch_pointer_event`/`dispatch_event`；处理按下/移动/抬起、坐标换算（含窗口原点）、
  返回键、IME commit。
- iOS/tvOS/visionOS：`UIView` 的 `touchesBegan/Moved/Ended/Cancelled`、`pressesBegan/Ended`、
  `UIKeyInput` 转发到同一入口；坐标从 UIKit 点空间换算。
- 两端各加一个可复跑的输入探针（点击 LineEdit → 键入 → `ValueChanged`），结果写入 logcat /
  Simulator 日志。

**验收**：Android 模拟器与 iOS Simulator 上 `RESULT: PASS`；GTK 的 `probe_keyboard_focus` 作为
桌面参照；无输入时不得产生点击回调。

**证据**：`src/platform/android/status.md`、`src/platform/ios/status.md` 的
“Input delivery into widgets”。

### B28.03 macOS 两个后端的 DPI

**问题**：`capabilities().dpi_scaling` 在 cocoa 与 objc2 上都是 `false`，`dpi_scale_factor()`
继承 `1.0`。Retina 上布局与文字会按 1x 处理。winit 的 AppKit 后端直接给出 `scale_factor`。

**交付**：

- cocoa：`NSScreen.backingScaleFactor`（枚举时逐屏）与 `NSWindow.backingScaleFactor`；
  `dpi_scale_factor()` 返回主屏/窗口所在屏的值；`displays().scale` 同步。
- objc2：同一来源，经 `objc2-app-kit`；主线程限制与其它 AppKit 方法一致。
- 把两者的 `dpi_scaling` 改为 `true`，并让 `docs_paths_tests.rs` 与三语 cookbook 矩阵同步。

**验收**：GitHub `macos-latest`（Retina）上 `dpi_scale_factor() == 2.0`；非 Retina 主机为
`1.0`；`check_capability_flags_match_their_methods.py` 通过。

### B28.04 Wayland 平台事实

**问题**：Wayland 是唯一没有 `displays` 的桌面后端；`dpi_scale_factor` 只看环境变量；
剪贴板只有进程内记录；`open_external` 未实现。

**交付**：

1. `displays()`：绑定 `wl_output`，用 `zxdg_output_manager_v1` 取逻辑位置/尺寸；id 用 output
   name 的稳定散列或注册序号；`scale` 用 `wl_output.scale`（有 `wp_fractional_scale_v1` 时优先）。
2. `display_of_window()`：从 `wl_surface` 的 `wl_surface.enter` 记录；`display_generation()`
   由 registry 的 output 增删/几何/scale 变化派生。
3. `dpi_scale_factor()`：当前窗口所在 output 的 scale；环境变量仅作无窗口时的回退。
4. `system_theme()`：XDG portal `org.freedesktop.portal.Settings.Read("org.freedesktop.appearance",
   "color-scheme")`；portal 不存在时 `None`。
5. `open_external()`：复用 `platform::external` 的 `xdg-open` 与 `has_external_handler`。
6. 剪贴板：`wl_data_device` + `wl_data_source`，写出与读回都走事件循环；只有 seat/data-device
   实际存在时才把 `clipboard` 置真。

**验收**：`tools/run_wayland_compositor_tests.sh`（CI 的 weston 环境）断言至少 1 个 output、
scale 与几何、generation 随 output 变化、`open_external` 可派发；剪贴板在同一会话内往返。
无 portal 的无头 weston 允许 theme 为 `None`。

### B28.05 Web 平台事实

**问题**：`WasmPlatform` 的宿主动作只到“设置 canvas 尺寸 + 表面表”，
`dpi_scale_factor`/`displays`/`system_theme`/`safe_area_insets`/`open_external` 都缺。

**交付**：

- DPI：`window.devicePixelRatio`。
- 显示器：`window.screen`（`width/height/availWidth/availHeight`，`scale = devicePixelRatio`，
  `is_primary = true`）；`display_of_window` 返回该屏；`display_generation` 由这些字段派生。
- 主题：`matchMedia('(prefers-color-scheme: dark)')`。
- 安全区：宿主提供一个带 `env(safe-area-inset-*)` 的隐藏探针元素，后端用 `getComputedStyle`
  读取；没有 DOM/样式时 `None`。
- `open_external`：`window.open(target, "_blank")`（仅 http/https，拒绝命令形状的输入）。
- 事件：为 canvas 增加 pointer/key 监听，转发到 `dispatch_pointer_event`/`dispatch_event`。

**验收**：浏览器测试页（Playwright 或人工）在 1x/2x DPR、浅/深色、iOS Safari 刘海样式下
分别断言；`wasm-pack test` 或独立页面脚本可复跑。

### B28.06 环境事实转发（高对比 / 减少动效 / 语言）

**问题**：`EnvironmentProvider` 有 8 个事实，`PlatformEnvironment` 只转发 2 个。控件读到的
`high_contrast`/`motion_preference`/`locale`/`text_direction` 永远是中性默认。

**交付**：

- 在 `Platform` 上增加语义方法（不是 `ThemeMode` 之类的策略类型）：
  `high_contrast() -> Option<bool>`、`reduce_motion() -> Option<bool>`、
  `system_locale() -> Option<&'static str>`（或一个固定容量 `LocaleTag`）。
- 各平台来源：
  - Windows：`SystemParametersInfoW(SPI_GETHIGHCONTRAST)`、`SPI_GETCLIENTAREAANIMATION`、
    `GetUserDefaultLocaleName`。
  - macOS：`NSWorkspace.accessibilityDisplayShouldIncreaseContrast` /
    `accessibilityDisplayShouldReduceMotion`、`NSLocale.currentLocale`。
  - iOS 家族：`UIAccessibility.isDarkerSystemColorsEnabled` / `isReduceMotionEnabled`、
    `NSLocale.currentLocale`。
  - Android：`AccessibilityManager.isHighTextContrastEnabled`、
    `Settings.Global.ANIMATOR_DURATION_SCALE == 0`、`Locale.getDefault()`。
  - GTK/BSD：`gtk-enable-animations`（减少动效）、主题名的高对比启发式（明确标注为启发式）、
    `glib::get_language_names`。
  - Wayland：portal `org.freedesktop.a11y`/`appearance`（有则用，无则 `None`）。
  - 强调色（可选，但在主题层已有 token）：Windows `HKCU…\DWM\AccentColor` 或
    `GetSysColor(COLOR_HIGHLIGHT)`；macOS `NSColor.controlAccentColor`；iOS
    `UIColor.tintColor`；Android `Resources` 的 `colorAccent`/Material You 动态色；GTK
    `@theme_selected_bg_color`。若不加，文档写明“主题强调色由应用提供”，不是“系统强调色已读”。
- 变化通知：主题/高对比/减少动效/语言都只有轮询，没有 generation。若宿主需要“变了就重绘”，
  新增 `environment_generation()`（同 `display_generation` 契约）。
- `PlatformEnvironment` 逐项转发；`EnvironmentSnapshot` 的“每帧一致”性质不变。

**验收**：每个后端一个可复跑探针；`EnvironmentSnapshot` 的 8 个字段有正/反例；
`text_direction` 由 locale 推导的规则在 bidi 测试中固定。

### B28.07 HarmonyOS 主题、安全区、文字缩放

**问题**：SDK 有 API，但当前桥没有输入：`OH_WindowManager_GetWindowAvoidArea` 需要 ArkTS 侧
window id；`OH_ArkUI_RegisterSystemColorModeChangeEvent` 是回调；`fontSizeScale` 在 ArkTS
`Configuration` 里。后端目前三者都 `None`/`1.0`。

**交付**：

1. 宿主契约：新增 `rw_harmony_set_window_id(i32)`（与 `nativeAttachContext` 同级的显式绑定）。
2. 安全区：对 `WINDOW_MANAGER_AVOID_AREA_TYPE_CUTOUT`、`SYSTEM_GESTURE`、
   `NAVIGATION_INDICATOR` 分别取 `top/left/right/bottom` 矩形并合并，按 vp 换算后返回。
3. 主题：在 XComponent 的 `ArkUI_NodeHandle` 上注册 color-mode 回调，缓存最后一次值；
   `system_theme()` 返回缓存，未收到回调前 `None`。
4. 文字缩放：宿主通过 `rw_harmony_set_font_scale(f32)` 上报 ArkTS `fontSizeScale`；
   未上报时保持 `1.0`。
5. 所有入口在 `ohos` target 外编译为空实现，保证宿主构建不受影响。

**验收**：`cargo ohos check/build` 通过；设备可用时，统一探针的 theme/safe-area/text-scale 位
PASS；没有设备时在日志中明确写为“链接级验证”。

### B28.08 文件选择与外部打开的契约统一

**问题**：`choose_path` 是同步阻塞契约，桌面三后端已实现；Android 有一个**独立** JNI picker，
objc2/Wayland/iOS/Harmony/WASM 没有。两种形态让宿主无法用一套代码。

**交付（先决策，再实现）**：

- 决策 A（推荐）：保留 `choose_path` 给“能同步弹出模态框”的桌面；新增异步
  `Platform::request_path(purpose, title) -> bool` + `take_path_choice() -> Option<PathChoice>`
  轮询，供 Android/iOS/Harmony/WASM 使用。`PathChoice = Chosen(PathBuf) | Cancelled | Failed`。
- 决策 B：不改公开契约，Android 的 picker 通过 `inject_path_choice` 注入，宿主自行承接
  Activity result。文档必须写明它不是 `choose_path`。
- 实现：Android 把现有 `ACTION_OPEN_DOCUMENT` 接到选定契约；iOS
  `UIDocumentPickerViewController`；Harmony ArkTS `DocumentViewPicker`；objc2 `NSOpenPanel`
  （主线程、阻塞）；Wayland 用 portal `org.freedesktop.portal.FileChooser`（异步）；
  WASM 用 `showOpenFilePicker`（异步）。
- `open_external`：Wayland/objc2 补 `xdg-open`/`open`；Android 用 `ACTION_VIEW`；iOS 用
  `UIApplication.openURL`；Harmony 用 `startAbility`；WASM 用 `window.open`。
  Redox 保持 `Unsupported`（没有处理器协议）。

**验收**：每个后端“选择/取消/不支持”三条路径都有测试；`capabilities().file_dialog` 与
`open_external` 与实际方法一致；统一探针覆盖。

### B28.19 窗口属性：从“记录”变成“应用到 OS”

**问题**：`app::WindowHandle` 的文档写的是“推送到 OS”，但多个后端只把值写进
`BackendState`；GTK 甚至没有覆盖，返回默认 `false`。窗口标题的更新路径则完全绕过了平台。

| 方法 | Windows | cocoa | objc2 | GTK | Wayland | Android/Harmony/iOS/Redox/WASM |
|---|---|---|---|---|---|---|
| `set_window_state` | ✅ 应用 | 🟡 仅记录 | 🟡 宏记录 | ❌ 默认 `false` | 🟡 宏记录 | 🟡 宏记录 |
| `set_window_min_size` | ✅ `WM_GETMINMAXINFO` | 🟡 仅记录 | 🟡 宏记录 | ❌ 默认 `false` | 🟡 宏记录 | 🟡 宏记录 |
| `set_window_icon` | ✅ `WM_SETICON` | ❌ 未覆盖 | 🟡 宏记录 | ❌ 未覆盖 | 🟡 宏记录 | 🟡 宏记录 |
| 窗口标题（`set_title`） | ❌ 模型 | 🟡 仅当走 `Platform` | ❌ 模型 | ❌ 模型 | ❌ 模型 | ❌ 模型 |
| `get_native_handle` | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| `window_client_size` | ✅ | ✅ | ❌ | ✅ | ✅ | ✅ |
| `queue_resize_trigger` | ✅ | ✅ | ❌ | ✅ | ✅ | ✅ |

证据：`app/handle.rs::set_icon`/`set_min_size`/`set_maximized` 的文档（约 3581–3690 行）写明
“AppKit `setContentMinSize:` / GTK `set_geometry_hints`/`maximize()` / GTK `set_icon_from_file`”，
但 `src/platform/linux/platform_impl.rs` 里没有这三个覆盖；
`src/platform/macos/platform_impl.rs::set_window_state`（约 867 行）只调用
`self.state.set_window_state`。窗口标题：`WindowHandle::set_title`（`app/handle.rs:1356`）→
`crate::set_widget_text` → 自定义后端只写控件属性
（`control_backend/custom/create_widgets_helpers.in.rs:126`），从未对
`Platform::create_window` 返回的 host id 调 `set_widget_text`。

**交付**：

1. 窗口标题：自定义后端在 `WidgetKind::Window` 的文本变化后，把新标题转发给
   `Platform::set_widget_text(host_id, title)`；同时更新 accessible name。
2. GTK：`set_window_state` 用 `gtk::Window::maximize/unmaximize/minimize/fullscreen`；
   `set_window_min_size` 用 `set_geometry_hints`；`set_window_icon` 用 `set_icon_from_file`。
3. cocoa/objc2：`set_window_state` 用 `zoom:`/`performMiniaturize:`/`toggleFullScreen:`；
   `set_window_min_size` 用 `setContentMinSize:`；图标是 bundle 级，`set_window_icon` 明确
   `Unsupported` 并改文档。
4. Wayland：`xdg_toplevel.set_maximized/unset_maximized/set_minimized/set_fullscreen`，
   `set_min_size` 用 `set_min_size`；图标有 `xdg-toplevel-icon-v1` 时用，否则 `false`。
5. Android：全屏/沉浸式与 `setDecorFitsSystemWindows`；Harmony：窗口模式 API；iOS：
   maximize/minimize 不适用，明确 `Unsupported`；Redox/WASM：分别用 Orbital/Fullscreen API，
   没有就明确 `false`。
6. `get_native_handle`：objc2（NSView）、GTK（`gtk::Widget`）、Android（View id/指针）、
   Harmony（XComponent）、iOS（UIView）、Wayland（wl_surface）、WASM（canvas）——有真实对象就返回，
   否则 `None`。
7. objc2 补 `window_client_size`（content rect）与 `queue_resize_trigger`（`crate::queue_resize_trigger`）。

**验收**：`WindowHandle::set_maximized/is_maximized` 在 Windows/GTK/cocoa/objc2/Wayland 上
真实生效；`set_min_size`/`set_icon` 返回 `true` 后对应 OS 状态可观察；标题修改在真实窗口标题栏
可见；每个“不适用”的格子改文档而不是继续写“推送”。

---

## 3. P1 — 体验与覆盖度

### B28.09 光标意图与形状

**为什么仍要列**：上一轮判定“暂不需要新增孤立 API”，但没有光标让文本输入/链接/拖拽的交互
不完整。正确的接缝是**控件意图 + 平台执行**两层，而不是只加一个后端方法。

**交付**：

1. `CursorIcon` 枚举（Default/Pointer/Text/Crosshair/Move/NotAllowed/Resize*/Grab/Grabbing/
   Wait/Progress/Help/ZoomIn/ZoomOut），并支持 winit 的 **自定义光标图像**
   （RGBA + hotspot；`Cursor::Custom`）。
2. `Widget::cursor_icon()`（默认 `Default`）与悬停分发：`dispatch_pointer_event` 在进入/离开
   控件时计算一次的意图，经 `Platform::set_cursor(window, icon)` 下发。
3. 后端：GTK `gdk::Window::set_cursor`；Windows `LoadCursorW` + `WM_SETCURSOR` 每窗口缓存；
   macOS `NSCursor` + cursor rect；Android `PointerIcon` + `View.setPointerIcon`；iOS
   `UIPointerInteraction`（若不可用则诚实 `false`）；Wayland `wl_pointer.set_cursor` +
   cursor theme；WASM canvas `style.cursor`。
4. 同时补 winit 的 cursor position / grab / visible / hit-test 四件事（无消费者时可先只做
   `set_cursor` + `set_cursor_visible`，其余明确 `Unsupported`）。

**验收**：LineEdit 悬停为 I-beam、Button 为 pointer、分隔条为 resize；X11 下用
`XFixesGetCursorImage` 或截图断言；其它平台用统一探针的 `cursor` 位。

### B28.10 iOS 文字缩放

`UIContentSizeCategory` 是类别名而不是乘数。用固定的类别→倍率表（`extraSmall`…`accessibilityXXXL`）
映射到 `1.0..=3.0`，`text_scale()` 返回；未注册类别返回 `None`。Simulator 上切换字号验证。

### B28.11 剪贴板剩余维度

- Wayland：`wl_data_device` 文本 + `text/html` + 自定义 MIME（与 §B28.04 一并）。
- WASM：`navigator.clipboard` 是 Promise，与同步契约不兼容；**保持 `false`**，并在
  `clipboard_backend()` 的文档里写明“Web 由宿主页面负责”，不伪造成功。
- Redox：文本已通；HTML/图片若 orbclient 无剪贴板协议则保持仅文本，能力标志按实际。

### B28.12 IME 组合

- objc2：`NSTextInputClient`（与 cocoa 的 `ime_macos` 状态机共用）。
- iOS：`UITextInput`/`UIKeyInput` 的 marked text 与 commit。
- Harmony：XComponent 已有关键事件与软键盘请求，补 composition 协议后再把 `ime` 置真。
- Wayland：`text-input-v3`（有协议时）。
- 其它保持 `ime: false`。

### B28.13 无障碍桥

objc2 `NSAccessibility`、Android `AccessibilityNodeProvider`、iOS `UIAccessibility`、
Wayland AT-SPI（经 zbus，GTK 之外的路径）。每项都必须有“读屏器可读到什么”的验收，
否则保持 `accessibility: false`。

### B28.14 tvOS / visionOS / Mac Catalyst 覆盖

- CI 增加 `cargo check --target aarch64-apple-{tvos,visionos}`（本机已完成，CI 未接）。
- tvOS/visionOS Simulator 运行 job（GitHub macOS runner 可用时）；不可用时明确标 SKIP。
- Mac Catalyst：`aarch64-apple-ios-macabi` 需要 nightly/特定 SDK；先做类型检查与文档，
  不宣称运行。watchOS 继续排除（`objc2-ui-kit` 无 `UIWindow`）。

### B28.15 Linux 非 GTK 路径的决策（FUTURE ITEM 1）

`LinuxPlatform` 在无 `gtk-native` 时是状态预览；`desktop` 默认并不启用 GTK。需要二选一：

- **A**：把 `gtk-native` 定为 Linux 桌面默认并写进 `desktop` 特征；无 GTK 的构建明确降级为
  “预览”，`runtime_gui_mode_for` 如实报告。
- **B**：实现原生 X11 后端（xcb/x11rb），不依赖 GTK。

在决策落地前，不在文档里把无 GTK 的 Linux 构建称作“完整桌面后端”。

### B28.16 宿主不可见测试（FUTURE ITEM 7 剩余）

把纯逻辑从 `cfg(target_os)` 模块中移出，使测试在宿主可编译：`ime_macos`、
`macos_objc2/tests.rs`、`ios/platform_impl.rs`、`android/platform_impl.rs`/`types.rs`、
`macos/tests.rs`、`accessibility/windows.rs`、`routing.rs` 的两条。目标：宿主套件覆盖这些
纯逻辑；OS 调用仍按目标门控。

### B28.17 更多目标三元组

- `aarch64-pc-windows-msvc`（Windows on ARM）
- `x86_64-unknown-linux-musl` / `aarch64-unknown-linux-musl`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-illumos`（`unix_desktop` 已覆盖谓词，未验证）

每项加交叉检查；GTK/Wayland 的 sysroot 缺失时如实 SKIP。

### B28.18 真机/会话证据矩阵

| 平台 | 目标证据 | 当前 |
|---|---|---|
| Windows GUI | 真实会话 + 统一探针 | ❌ 只有交叉检查 |
| macOS | CI `macos-latest` 的 AppKit/objc2 探针 + Retina | 🟡 CI 已有测试，无平台事实探针 |
| Linux X11 | 真实会话 | ✅ |
| Wayland | weston CI | ✅（后端绑定）；平台事实待 B28.04 |
| BSD | FreeBSD/NetBSD 真实会话 | ❌ 只有类型检查 |
| Android | 模拟器 + 真机 | ✅（x86_64 模拟器 12/12；arm64 真机历史证据） |
| HarmonyOS | 真机/模拟器 | ❌ 链接级 |
| iOS/tvOS/visionOS | Simulator/真机 | 🟡 iOS CI Simulator；tvOS/visionOS 无 |
| Redox | Orbital 会话 | ❌ 类型检查 |
| WASM | 浏览器 | ❌ 无浏览器运行门禁 |

### B28.20 窗口语义属性（trait 之外）

**现状**：`Platform` 只有“状态/最小尺寸/图标”三个窗口属性；没有装饰、可调整大小、置顶、
透明度、焦点/激活请求、任务栏闪烁。`app::WindowHandle` 也没有对应方法。

**需要决策**：先确认是否有真实宿主消费者（设计器/演示需要置顶或透明窗口吗？）。

- 若需要：新增一个**值类型** `WindowAttributes`（decorations、resizable、always_on_top、
  opacity、focus_on_map、attention）与 `Platform::apply_window_attributes(window, attrs)`，
  而不是每次加一个 trait 方法（principle #54 的单定义策略）。
- 各后端：Windows `SetWindowLongW`/`SetLayeredWindowAttributes`/`FlashWindowEx`；
  macOS `NSWindow` style mask/level/alpha/`requestUserAttention`；GTK/`gdk` 对应属性；
  Wayland `xdg_toplevel`（部分属性无协议，诚实 `Unsupported`）；移动端大多 `N/A`。

**验收**：属性值可通过 `window_attributes()` 读回；不支持的属性返回 `Unsupported` 并附原因；
文档不再用“应该能”描述。

### B28.21 显示器元数据与变更通知

**现状**：`Display` 只有 `id/bounds/work_area/scale/is_primary`；`display_generation` 是对现有字段
的散列。刷新率、物理方向、HDR/色彩空间、每显示器文字缩放都没有。

**交付**：

- 用**非破坏**方式扩展：新增 `Platform::display_info(id) -> Option<DisplayInfo>`，其中
  `DisplayInfo { refresh_rate_mhz: Option<u32>, orientation: Option<Orientation>,
  color_space: Option<ColorSpace>, hdr: Option<bool> }`；`Display` 保持现有字段不变（避免破坏
  结构体字面量）。
- 来源：Windows `EnumDisplaySettingsW`/`GetDisplayConfigBufferSizes`；macOS
  `NSScreen.deviceDescription`/`CGDisplayMode`；GTK `gdk::Monitor::refresh_rate`/`scale_factor`；
  Wayland `wl_output`（刷新率）/`wp-color-management`（色彩，若有）；Android
  `Display.getRefreshRate`/`getRotation`/`Display.HdrCapabilities`；iOS `UIScreen`（HDR）。
- 变更：`display_generation` 把新字段纳入散列；Wayland/Web 见 B28.04/B28.05。

**优先级拆分**：枚举/scale/work area/窗口归属/变更/每窗口 scale/刷新率/方向为 **P1**；
`ColorSpace`/HDR 为 **P3**——当前渲染路径是 RGBA8，没有颜色管理消费者，先把字段与来源留好，
不宣称 HDR 渲染。

**验收**：每个后端至少返回刷新率与方向（不可读则 `None` 并说明）；拔插/旋转后 generation 变化。

### B28.22 剪贴板富内容与变更通知

**现状**：`ClipboardContent` 只覆盖文本/HTML/自定义 MIME；没有图片、文件列表，也没有
“剪贴板变了”的信号。

**交付**：

- 新增 `ClipboardContent::Image { width, height, rgba }` 与 `Files(Vec<PathBuf>)`（或平台无关的
  URI 列表），并在 `has_format`/`get_contents_for_type` 中按类型协商。
- 新增 `Platform::clipboard_generation() -> u64`（同 `display_generation` 的轮询契约；`0` 表示
  不报告）。
- 后端：Windows `CF_DIBV5`/`CF_HDROP` + `AddClipboardFormatListener`；macOS
  `NSPasteboard` 的 image/file URL 类型 + `changeCount`；GTK `gdk::Clipboard` 的 image/URI +
  `changed` 信号；Android `ClipData` image URI/`ContentResolver` + `OnPrimaryClipChangedListener`；
  iOS `UIPasteboard.image`/`URLs` + `changeCount`；Wayland 见 B28.11（`wl_data_device`）；
  WASM 保持不支持（异步边界）。

**验收**：图片/多文件往返；写入后 changeCount/generation 变化；不支持的后端诚实 `None`/`0`。

**优先级拆分**：文本/HTML/自定义 MIME/图片/文件 + 有方法的后端 = **P1**；`clipboard_generation`
变更通知 = **P2**（只有剪贴板管理器类宿主需要）。

### B28.23 拖放：文件与角色

**现状**：`begin_drag` 只有 `mime + &[u8]`；`DropEvent` 没有文件列表、源/目标角色、拖拽位置。

**交付**：

- `DragPayload { mime, bytes }` 扩展为可携带 `files: Vec<PathBuf>` 或 URI 列表；
  `DropEvent` 增加 `action`（Copy/Move/Link）与 `position`。
- 后端：Windows `DoDragDrop`/`IDropTarget`（`CF_HDROP`）；macOS `NSDraggingSession`/`NSFilePromise`；
  GTK `gdk::DragContext` + `text/uri-list`；Wayland `wl_data_offer`（`text/uri-list`）；
  Android `ACTION_DRAG_*`/`ClipData`；iOS `UIDropInteraction`/`NSItemProvider`；Harmony
  `OH_Drag`（如 SDK 提供）。

**验收**：同进程与跨进程文件拖动都可表达；不接受的文件类型被拒绝而不是静默拷贝字节。

**优先级拆分**：桌面文件拖放 = **P1**；Android/iOS/Harmony 的系统拖放 = **P3**（移动端
拖放不是控件可用性的必要条件）。

### B28.24 移动 GUI 核心生命周期与输入

**职责判定**：控件库只负责“控件能正常显示、布局、输入、暂停/恢复”。触觉/权限/通知/分享/
深链/传感器/定位/常亮属于应用外壳，已移入 B28.44（可选 P3）。

**现状**：Android 有软键盘桥；屏幕方向、生命周期、死键/重复没有接缝。

**交付**：

- **方向**：读取当前方向（portrait/landscape + 四种倒置），并在方向变化时重跑布局；
  `MobilePlatformExtension` 提供 `orientation()` 与 `orientation_generation()`。
- **生命周期**：`suspend()`/`resume()`（或 `lifecycle_state()` + generation），宿主在
  Android `onPause/onResume`、iOS `sceneWillResignActive/didBecomeActive`、Harmony 对应回调里调用；
  库在挂起时停止动画时钟、恢复时重置 delta，不丢状态。
- **输入细节**：死键重置（winit `reset_dead_keys`）、按键重复速率、键盘布局查询；与 B28.02 的
  宿主转发配套，不另建事件模型。
- **软键盘**：Android 已有；iOS/Harmony 在 B28.12 补。
- 平台来源：Android `Display.getRotation`/`onConfigurationChanged`、`Activity.onPause/onResume`；
  iOS `UIDevice.orientationDidChangeNotification`、`UIWindowScene` 激活状态；Harmony
  `window.getWindowProperties().orientation` 与生命周期回调。

**验收**：模拟器/真机旋转后布局重算且 `orientation_generation` 变化；挂起后不再推进动画，
恢复后从正确 delta 继续；死键组合（如 `´` + `e`）产生正确字符；无方向 API 的后端返回 `None`
而不是假装 portrait。

### B28.25 桌面应用外壳集成（可选，P3）

**职责判定：非核心。** 托盘、系统通知、全局快捷键、单实例、Dock/任务栏徽标与进度属于应用
外壳；winit 也不提供。控件库不应为了“对齐 winit”而引入这些依赖。

**若要做**，作为独立的 `DesktopIntegration` 可选扩展（不进 `Platform` 通用面）：

- Windows：`Shell_NotifyIconW`、`Toast`、`RegisterHotKey`、`CreateMutex`、`ITaskbarList3`。
- macOS：`NSStatusItem`、`UNUserNotificationCenter`、`NSGlobalHotKey`（需辅助功能权限）、
  `NSApplication.dockTile`。
- GTK/BSD：AppIndicator、`org.freedesktop.Notifications`（zbus）、
  `org.freedesktop.Application` 单实例。
- Wayland：`org.kde.StatusNotifierItem`（有则用）、portal 通知。

**验收**（只在实现时）：每项有“权限/协议是否可用”的运行时查询；没有时 `Unsupported`。

### B28.26 Web GUI 核心集成

**职责判定**：可见性、DPR、安全区、输入转发是 GUI 必须；全屏/指针锁是可选交互；
权限、存储、PWA、Service Worker 是页面宿主职责。

**交付**：核心——`document.visibilitychange` → 挂起/恢复动画（与 B28.24 同一语义）、
canvas 的 pointer/key 转发、DPR/安全区（B28.05）；可选——fullscreen 与 pointer-lock 的封装。

**验收**：浏览器测试页在切 tab 后动画停止、回来后恢复；全屏/指针锁在不支持的浏览器返回
`Unsupported`；worker/wasi 环境全部 `Unsupported`。

### B28.27 打印（条件必需）

**职责判定**：库已有 `print`/`PrintContext`/`has_print_support`/`spawn_print_job`，既然对外
提供打印能力，各平台就应补齐或诚实 `false`；若未来移除 `print` 特性，本条随之取消。

**现状**：`has_print_support`/`spawn_print_job` 只在桌面三后端 + Harmony/WASM 有覆盖；
Android/iOS 默认 `false`。

**交付**：Android `PrintManager.print` + `PrintDocumentAdapter`；iOS
`UIPrintInteractionController`；Harmony `PrintKit`（若 SDK 提供）。没有 API 的平台保持 `false`。

**验收**：`has_print_support` 为真时 `spawn_print_job` 真的提交到系统打印队列（设备/模拟器探针）。

### B28.28 电源/热提示（可选，P3）

**职责判定：非核心。** 网络可达性属于应用层；低电量/热状态只作为“降级动画/减少重绘”的
可选策略输入，不影响控件正确性。

**若要做**：`power_state()`（battery level/charging/low-power）与 `thermal_state()`；来源：
Windows `GetSystemPowerStatus`/`PowerSettingRegisterNotification`、macOS
`IOPSCopyPowerSourcesInfo`/`ProcessInfo.thermalState`、iOS/Android 对应 API、GTK/BSD 的
`/sys`/`sysctl`；WASM 只提供 `navigator.onLine`（且仅作提示）。

**验收**（只在实现时）：不可读时 `None`；不把“插着电”当成“满电”；变化可通过轮询值观察。
网络可达性不进入控件库。

### B28.29 旧平台控件契约的归档

**现状**：`Platform` 仍有 50 个 `create_*`、14 个 combo/list、7 个 menu 方法，但生产路径是
`CustomPaintControlBackend`（`control_backend/dispatcher.rs::get_control_backend`）。它们不是缺口，
但读者会误以为“平台应创建原生控件”。

**交付**：

- 给这些方法加统一的文档说明：“生产路径不经过这里；保留给 native-mode/测试/历史契约”。
- 或者拆分出一个 `NativeControlPlatform` 子 trait，从 `Platform` 移走；后者是破坏性变更，
  需版本策略。
- 加一个门禁：公开 `lib.rs`/`bindings` 不得直接调用这些 `Platform::create_*`（只允许
  `control_backend/native.rs`）。

**验收**：读者从 trait 文档能分辨“自绘默认”与“真实缺口”；门禁可复跑。

### B28.30 生成式“平台事实矩阵”与门禁

**现状**：`docs/plans/platform_capability_matrix.md` 只覆盖 9 个能力标志；39 个事实/服务/窗口方法
没有生成矩阵，只有本文的手工表。

**交付**：扩展生成器（或新增 `tools/generate_platform_facts_matrix.py`），从各后端源码提取：
每个方法的覆盖状态（真实应用/仅记录/默认）、`status.md` 的边界说明、运行证据级别；
生成 `docs/plans/platform_facts_matrix.md`。门禁：源码新增/删除一个 `Platform` 方法时，
矩阵必须同步（`tools/check_platform_facts_matrix.sh`）。

**验收**：矩阵与源码一致；新增方法未登记时门禁失败；三语 cookbook 只引用生成物，不手抄。

### B28.31 文档真实性清算

**现状**：至少两处文档承诺了代码没有的行为：

- `app/handle.rs` 的 `set_icon`/`set_min_size`/`set_maximized` 文档写“推送到 OS”并列举
  AppKit/GTK 的具体 API，但 macOS 只记录、GTK 未实现。
- `Platform` 的 `text_scale` 文档曾把未实现的平台列为已实现（已在本轮修正），说明同类句子
  需要一次系统清扫。

**交付**：逐条核对 `src/platform/*/status.md`、`app/handle.rs` 的方法文档、三语 cookbook 的
“已支持”表述；每条要么改代码，要么改成“记录/不适用/未验证”。加一个纯文本门禁：
`grep` 出 `set_icon_from_file`、`setContentMinSize`、`maximize()` 等 API 名称时，必须同时有
对应源码符号或 `Unsupported` 说明。

**验收**：文档中每个“推送到 OS/平台 API”的句子都能在源码找到调用；找不到的已改为边界说明。

### B28.32 窗口关闭/焦点/移动生命周期事件

**现状**：`WidgetTriggerKind::Closed` 存在，`WindowHandle::on_close` 有真实消费者；但只有
Windows（`WM_CLOSE`）、objc2（`windowShouldClose:`）与 Wayland（close 请求）会入队。GTK、
cocoa、Android、Harmony、iOS、Redox、WASM 不会产生 `Closed`。窗口移动也没有事件
（`Resized` 只有尺寸）。

**交付**：

- GTK：`delete-event`/`destroy` → `inject_widget_trigger_event(window, Closed)`；
  `configure-event` 的 position 变化 → 新增 `WidgetTriggerKind::Moved { x, y }`（或单独事件）。
- cocoa：`windowShouldClose:`/`windowWillClose:`；objc2 已有 `windowShouldClose:`，补
  `windowDidMove:`。
- Android：`Activity.onBackPressed`/`onDestroy`；窗口尺寸/位置用已有的
  `nativeNotifyResize` 扩展为 `nativeNotifyGeometry`。
- Harmony：窗口 destroy/move 的 ArkUI 回调；iOS：`UIWindowScene` 生命周期与
  `viewWillDisappear`（按自绘窗口语义）。
- Redox/WASM：Orbital close 事件 / `beforeunload`、`resize`、`scroll`。

**验收**：`on_close` 在每个后端都能被用户关闭窗口触发一次；重复关闭不重复入队；移动事件坐标
与 `display_of_window` 同空间。

### B28.33 音频输出后端

**现状**：`audio-output` 直接依赖 `cpal`（`Cargo.toml:610`），没有任何平台层封装；cpal 的
Android/iOS/WASM/Harmony/Redox 支持情况未被本库验证。

**交付**：确认 cpal 对各目标的实际后端（WASAPI/CoreAudio/ALSA/PulseAudio/AAudio/WebAudio），
为不支持的目标提供明确 `Unsupported` 而不是链接失败；在 Harmony/Redox 上要么实现后端，要么
从 profile 中移除该能力并写明。

**验收**：`cargo check` 全部目标 + 至少一个真实输出会话；`supports_audio()` 之类的运行时探针
（若已有 API）与实现一致。

### B28.34 资源监视后端

**现状**：`i18n` 使用 `notify`（`Cargo.toml:713`）；`notify` 支持桌面与 BSD，但 Android/iOS/
Harmony/Redox/WASM 的文件监视语义不同（或不存在）。`AssetWatcher` 没有平台后端说明。

**交付**：逐平台确认 `notify` 后端可用性；不可用的目标让 `AssetWatcher` 返回明确的
“不支持/不可观察”，而不是启动一个永远不发事件的线程。

**验收**：删除/重命名/修改三类事件在桌面与 BSD 实测；移动端/WASM 有明确的
`Unsupported` 与文档。

### B28.35 GPU/wgpu 后端覆盖

**现状**：`gpu`/`wgpu` 依赖 wgpu 30（`Cargo.toml:343/753`）；WASM 有专门适配
（`src/gpu/adapter.rs`），但 Harmony/Redox/BSD 的 wgpu 后端未被本库验证。

**交付**：逐目标确认 `wgpu` 可编译并能创建 adapter（Harmony 的 Vulkan/GLES、Redox 的
软件/GL、BSD 的 Vulkan）；不可用的目标回退到软件后端并在能力查询中如实报告。

**验收**：每个目标至少 `cargo check`；可运行目标创建一个真实 surface 并清屏。

### B28.36 视频编解码后端

**现状**：`video-codecs` 依赖系统 `ffmpeg-next`（`Cargo.toml:612`）；移动端/Harmony/Redox/WASM
没有随包 ffmpeg，交叉编译会失败或静默关闭。

**交付**：明确“需要系统 ffmpeg”的构建边界；为不能提供 ffmpeg 的目标把视频能力报告为
`Unsupported`，而不是让整个 profile 编译失败；文档列出每个平台的安装/交叉编译路径。

**验收**：有 ffmpeg 的目标可播放；没有的目标构建成功且能力查询为 false。

### B28.37 profile × target 编译矩阵

**现状**：`desktop`/`tablet`/`mobile`/`mini`/`embedded` 与各目标的组合没有被系统检查；
例如 `mobile` + desktop 目标、`mini` + Apple 目标、`embedded` + Windows 是否都能编译，
没有一张表。

**交付**：在 CI 跑一个矩阵（可裁剪到代表性组合）：5 个 profile × {Windows, macOS, Linux,
Wayland, Android, Harmony, iOS, Redox, WASM}；每个组合 `cargo check`，记录
expectation（compiles / expected-to-fail-with-reason）。

**验收**：矩阵全绿或每个失败都有具名原因；新增 profile/target 时矩阵必须更新。

### B28.38 target → backend 选择门禁

**现状**：tvOS/visionOS 曾经“模块编译、选择不到、静默落到 stub”而无人发现；当前只靠
FUTURE 记录，没有自动门禁。

**交付**：一个脚本对每个受支持 triple 断言选中的后端：
`cargo rustc --target <t> -- --print cfg` 不足以看到 `create_native_platform` 的选择，
因此用一个编译期探针（`platform::backend_name()` 的 const 或测试）在目标上构建/运行，
或者在源码层解析 `runtime.rs` 的 `cfg` 分支并与目标列表对照。

**验收**：新增一个目标但忘记加选择臂时门禁失败；`apple_uikit`/`unix_desktop` 等别名
与 `Cargo.toml` 的依赖表保持一致。

### B28.39 系统字体枚举与加载（本项目特有）

**为什么需要**：winit **不提供**字体枚举，所以这不是“winit 有”。但它是本项目的真实缺口：

- `runtime-fonts` 已提供 `text::register_face(name, &'static [u8])`，宿主可以注册系统字体；
- `face_for_family` 先查运行时注册，再查编译进库的 face
  （`src/render/text/shaping.rs::face_for_family`）；
- 但没有任何平台枚举入口，且所有主题写的是 `"Arial"`，而库内只带 `"Open Sans"` 等，
  `face_for_family("Arial")` 默认返回 `None`（该模块文档已用实测数字记录了这个回退）；
- `FontComboBox` / `FontDialog` 的字体列表完全由调用方提供（
  `src/widget/input_widgets/font_combo_box.rs`、`src/widget/dialog/font_dialog.rs`），没有来源。

**交付**：

1. `Platform::system_fonts() -> Vec<SystemFont>`，`SystemFont { family, style, source }`；
   `source` 优先是文件路径（调用方读取后 `register_face`），没有可读路径的平台只返回 family。
2. `Platform::bundled_font_families()`（或 `render::text` 的公开访问器）列出**实际能渲染**的
   bundled 家族，供 `FontComboBox`/`FontDialog` 在无系统字体时使用。
3. 平台来源：Windows DirectWrite `IDWriteFontCollection` 或 GDI `EnumFontFamiliesExW`；
   macOS/iOS CoreText `CTFontManagerCopyAvailablePostScriptNames` + `CTFontDescriptor`；
   GTK/BSD fontconfig（`FcFontList`）或 `fontdb`；Android `/system/fonts` + `Typeface`；
   Harmony `OH_Drawing` 字体管理器；WASM `queryLocalFonts()`（异步 + 权限，默认返回空）；
   Redox 诚实返回空。
4. 选中系统字体时：读取文件 → `register_face`（需要 `runtime-fonts`）→ 重新布局/重绘；
   未启用 `runtime-fonts` 时只列出 bundled 家族并在文档中说明。
5. 新增能力标志 `system_fonts`（或在 `PlatformCapabilities` 中增加同义字段），与实际方法一致。

**验收**：列表与 `fc-list` / Font Book / 系统字体目录抽样一致；选中后 `face_for_family` 命中
新家族且文本真的变化；重名/多 style 去重；无系统字体时回退到 bundled 列表；不存在的字体名不
造成 panic 或静默成功。

### B28.40 raw-window-handle 只读适配（winit 有，尽量有）

**现状**：只有不透明的 `Platform::get_native_handle`；宿主想用自己的 GPU 设备/第三方渲染器时，
无法取得 `raw-window-handle` 0.6 的标准句柄。

**交付**：

- 新增可选特性 `raw-window-handle`，依赖 `raw-window-handle = "0.6"`。
- `Platform::raw_window_handle(window_id) -> Option<RawWindowHandle>` 与
  `raw_display_handle() -> Option<RawDisplayHandle>`（0.6 的枚举是 `Copy`，无需生命周期）。
- 后端：Win32 `HWND`/`HINSTANCE`；AppKit `NSView`；X11 `Window`/`Display`；Wayland
  `wl_surface`/`wl_display`；Android `ANativeWindow`；iOS UIKit `UIView`（若 rwh 提供 UiKit 变体）；
  WASM `WebWindowHandle`（canvas）；Redox/无窗口后端返回 `None`。
- 只读：不转移所有权、不改变 `mount_surface` 契约；文档写清“取得句柄不等于可以绕过库绘制”。

**验收**：一个 wgpu 示例能用该句柄创建 surface 并在 Windows/X11/Wayland/macOS 上清屏；
其余目标至少 `cargo check`；`None` 的后端有测试。

### B28.41 创建时的窗口属性

**现状**：`create_window(title, x, y, w, h)` 只有五个参数；winit 的 `WindowAttributes` 能在创建时
指定装饰、透明、可调整大小、可见性、位置器、主题等。

**交付**：

- 新增 `WindowAttributes`（默认值 = 当前行为）与 `Platform::create_window_with(attrs)`；
  默认实现转发到 `create_window` 并返回“哪些属性未被应用”的诊断，而不是静默丢弃。
  字段对齐 winit 的创建属性：`surface_size/min/max/resize_increments`、`position`、
  `resizable`、`enabled_buttons`、`title`、`maximized`、`visible`、`transparent`、`blur`、
  `decorations`、`window_icon`、`preferred_theme`、`content_protected`、`window_level`、
  `active`、`cursor`、`fullscreen`、`window_type`、`positioner`、`parent`。
- `app` 层提供 `WindowOptions` 构建器；已有调用保持不变。
- 后端：Windows style/ex-style；AppKit style mask/alpha/collection behavior；GTK 属性；
  Wayland `xdg_toplevel`（部分属性无协议，明确 `Unsupported`）；移动端只应用相关项
  （全屏/方向/状态栏）。

**验收**：无边框/透明/不可调整大小的窗口在桌面三后端真实创建；未应用属性被报告；
默认 `WindowAttributes` 与旧 `create_window` 行为逐项一致。

### B28.43 多窗口与窗口归属

**现状**：`create_window` 可以被多次调用，表面/触发器按 id 存储；但没有一个“这个后端能开几个窗口”的
契约。Android/Harmony/WASM 只有一个 Activity/canvas，iOS 可以创建多个 `UIWindow`，Redox 有
窗口表。宿主无法在创建前知道第二个窗口会成功还是静默成为一个普通 surface。

**交付**：`Platform::supports_multiple_windows() -> bool`（默认 `true`，单窗口后端覆盖为 `false`）；
`create_window` 在不支持时返回 `0` 并记录原因；每个窗口的 `mount_surface`/`display_of_window`/
`queue_resize_trigger`/close 事件都按窗口隔离；文档列出每个后端的上限（1 或 N）。

**验收**：桌面上两个窗口各自有独立表面与关闭事件；单窗口后端第二次 `create_window` 返回 `0`，
不是静默复用第一个 id；统一探针覆盖。

### B28.42 呈现前通知与帧节拍

**现状**：有 `invalidate_surface`/`take_pending_repaint`，但没有 winit 的
`pre_present_notify` 语义；Wayland 的 `wl_surface.frame` 回调没有接入。

**交付**：`Platform::pre_present_notify(window)` 默认 no-op；Wayland 用它请求 frame callback，
Windows/macOS/GTK 可映射到各自的 compositor 提交/刷新点；文档说明它不改变由宿主驱动的帧循环。

**验收**：Wayland 合成器下帧回调与呈现对齐；其余后端调用为 no-op 且不产生警告；
不引入库自己的循环。

### B28.44 移动应用服务（可选，P3）

**职责判定：非核心。** 以下能力属于应用外壳，不是“控件正常显示/交互”的必要条件；
如果库要提供，也只作为独立的可选扩展，不进入 `Platform` 通用面。

| 服务 | 放置位置 | 平台来源（若实现） |
|---|---|---|
| 保持常亮 | 应用（或控件库的可选扩展） | Android `FLAG_KEEP_SCREEN_ON`；iOS `idleTimerDisabled`；Harmony `KeepScreenOn` |
| 触觉/震动 | 应用 | Android `Vibrator`；iOS `UIImpactFeedbackGenerator`；Harmony `vibrator` |
| 权限查询/请求 | 应用 | Android `ActivityCompat`；iOS `AVCaptureDevice`/`PHPhotoLibrary` 等；Harmony `abilityAccessCtrl` |
| 本地通知 | 应用 | Android `NotificationManager`；iOS `UNUserNotificationCenter`；Harmony `notificationManager` |
| 分享面板 | 应用 | Android `Intent.createChooser`；iOS `UIActivityViewController`；Harmony `ShareKit` |
| 深链/URL scheme | 应用 | Android `Intent`；iOS `openURL`；Harmony `startAbility` |
| 传感器/地理 | 应用 | 各平台传感器 API；控件库不持有定位权限 |

**验收**（只在实现时）：每个服务有“可用/拒绝/不支持”三态；不引入权限请求的隐式副作用。

---

## 4. 明确不做（附理由，避免反复登记）

| 项 | 理由 |
|---|---|
| watchOS | `objc2-ui-kit` 不提供 `UIWindow`；没有可绘制窗口的后端不选。这与 winit 的 `target_vendor = "apple" && !macos` 字面条件不同，是有意的差异，已在 §1.5 标注。 |
| Web 同步剪贴板 | `navigator.clipboard` 是 Promise；同步 trait 不能 await。保持 `clipboard: false`，由页面宿主负责。 |
| 原生菜单 | 全平台菜单保持进程内树 + 注入触发；winit 也不提供；这是一致的设计裁决，不是缺口。 |
| 屏幕捕获/屏幕录制 | 系统级捕获需要平台权限（TCC/Graphics Capture/Wayland portal），属于应用功能；winit 不提供。库只需要已有的 `render_frame`/SVG 快照做测试。若将来要“捕获本库窗口”，另立可选项，不做系统屏幕捕获。 |
| 原始输入（raw input / evdev / IOHID） | 会引入权限、独占设备与安全面；控件只需要已转译的指针/键盘/触摸事件。 |
| 网络可达性 | 应用层职责，不是 GUI 平台事实；控件库不持有网络栈。 |
| 桌面应用外壳（托盘/系统通知/全局热键/单实例/徽标） | 归 B28.44 同类的可选扩展（B28.25，P3）；不是“控件正常显示”的必要条件，winit 也不提供。 |
| 移动应用服务（触觉/权限/通知/分享/深链/传感器/定位/常亮） | 见 B28.44（可选 P3）；控件库只保留方向/生命周期/软键盘/安全区等 GUI 必需项。 |
| 控制层路由/原生控件覆盖 | 属 `FUTURE.md` ITEM 0/6 与 `platform_capability_matrix.md` 的范围，不并入平台层计划。 |

> 上一版把 **raw-window-handle** 与 **挂起/恢复** 列为“不做”；按“winit 有、且与本项目相关”
> 的准则，它们已分别改为 B28.40（只读适配）与 B28.24（移动生命周期）。

---

## 5. 顺序与依赖

```text
B28.01 统一探针
   |
   +--> B28.02 移动输入转发（Android/iOS）
   |
   +--> B28.19 窗口属性真正应用到 OS（含标题/原生句柄）
   |
   +--> B28.03 macOS DPI
   |
   +--> B28.04 Wayland 事实 ---> B28.11 Wayland 剪贴板
   |
   +--> B28.05 Web 事实 ---> B28.26 Web 集成
   |
   +--> B28.06 环境事实
   |
   +--> B28.07 Harmony 事实（需要宿主 window id 契约）
   |
   +--> B28.08 文件/外部打开契约（先决策）
        |
        +--> B28.09 光标意图（依赖输入/悬停分发）
        +--> B28.12 IME / B28.13 无障碍（可并行，按平台）

B28.14–B28.18、B28.20–B28.25、B28.27–B28.31、B28.33–B28.44 为独立支线：
覆盖度/证据/集成/文档，不阻塞上条主链。B28.39（字体）与 B28.40（rwh）可先于 Wayland/Web 完成。
B28.25/28/44 是应用外壳（P3），不参与“全平台 GUI 完整”的硬验收。
```

先做 B28.01 的原因：没有统一探针，后面每一条都只能靠“代码看起来对了”来收口，
违反 principle #1。

---

## 6. 验收命令（新增/复跑）

```bash
# 现有平台门禁
bash tools/check_bsd_cross.sh
bash tools/check_harmony_cross.sh
bash tools/check_android_cross.sh
bash tools/check_ios_cross.sh
bash tools/check_macos_cross.sh
bash tools/check_redox_cross.sh
bash tools/check_jni_signatures.sh
python3 tools/check_capability_flags_match_their_methods.py
bash tools/check_changelog_sync.sh

# 新增（随 B28.01/各条落地）
tools/check_platform_probe.sh                 # 位定义 ↔ C 导出 ↔ 各宿主声明
cargo check --target aarch64-apple-tvos       # 已可跑，待接入 CI
cargo check --target aarch64-apple-visionos
cargo check --target aarch64-pc-windows-msvc
cargo check --target x86_64-unknown-linux-musl
tools/run_wayland_compositor_tests.sh         # 扩展 display/theme/clipboard 断言
tools/run_ios_testapp.sh                      # 扩展输入转发探针
tools/check_android_runtime.sh                # 扩展统一探针位
```

---

## 7. 状态

| ID | 条目 | 优先级 | 状态 |
|---|---|---|---|
| B28.01 | 统一平台诊断探针 | P0 | `[ ]` |
| B28.02 | Android/iOS 输入转发 | P0 | `[ ]` |
| B28.03 | macOS 双后端 DPI | P0 | `[ ]` |
| B28.04 | Wayland 显示器/DPI/主题/外部打开 | P0 | `[ ]` |
| B28.05 | Web 平台事实 | P0 | `[ ]` |
| B28.06 | 环境事实转发 | P0 | `[ ]` |
| B28.07 | HarmonyOS 主题/安全区/文字缩放 | P0 | `[ ]` |
| B28.08 | 文件/外部打开契约统一 | P0 | `[ ]` |
| B28.19 | 窗口属性从记录变为应用到 OS | P0 | `[ ]` |
| B28.09 | 光标意图与形状 | P1 | `[ ]` |
| B28.10 | iOS 文字缩放 | P1 | `[ ]` |
| B28.11 | 剪贴板剩余维度 | P1 | `[ ]` |
| B28.12 | IME 组合 | P1 | `[ ]` |
| B28.13 | 无障碍桥 | P1 | `[ ]` |
| B28.14 | tvOS/visionOS/Catalyst 覆盖 | P1 | `[ ]` |
| B28.15 | Linux 非 GTK 路径决策 | P1 | `[ ]` |
| B28.16 | 宿主不可见测试 | P1 | `[ ]` |
| B28.17 | 更多目标三元组 | P1 | `[ ]` |
| B28.18 | 真机/会话证据矩阵 | P1 | `[ ]` |
| B28.20 | 窗口语义属性（trait 之外） | P2（需决策） | `[ ]` |
| B28.21 | 显示器元数据与变更通知 | P1（HDR/色彩 P3） | `[ ]` |
| B28.22 | 剪贴板富内容与变更通知 | P1（变更通知 P2） | `[ ]` |
| B28.23 | 拖放：文件与角色 | P1（移动 P3） | `[ ]` |
| B28.24 | 移动 GUI 核心生命周期与输入 | P1 | `[ ]` |
| B28.25 | 桌面应用外壳集成 | P3（可选，非核心） | `[ ]` |
| B28.26 | Web GUI 核心集成 | P2（全屏/指针锁可选） | `[ ]` |
| B28.27 | 打印（Android/iOS/Harmony） | P2（条件必需） | `[ ]` |
| B28.28 | 电源/热提示 | P3（可选，非核心） | `[ ]` |
| B28.29 | 旧平台控件契约归档/门禁 | P1（文档） | `[ ]` |
| B28.30 | 生成式平台事实矩阵与门禁 | P1（工具） | `[ ]` |
| B28.31 | 文档真实性清算 | P0（文档） | `[ ]` |
| B28.32 | 窗口关闭/焦点/移动生命周期事件 | P1 | `[ ]` |
| B28.33 | 音频输出后端 | P2（条件） | `[ ]` |
| B28.34 | 资源监视后端 | P2（条件） | `[ ]` |
| B28.35 | GPU/wgpu 后端覆盖 | P1（GPU 特性） | `[ ]` |
| B28.36 | 视频编解码后端 | P2（条件） | `[ ]` |
| B28.37 | profile × target 编译矩阵 | P1（工具） | `[ ]` |
| B28.38 | target → backend 选择门禁 | P1（工具） | `[ ]` |
| B28.39 | 系统字体枚举与加载 | P1（本项目特有） | `[ ]` |
| B28.40 | raw-window-handle 只读适配 | P1（winit 对齐） | `[ ]` |
| B28.41 | 创建时的窗口属性 | P1（winit 对齐） | `[ ]` |
| B28.42 | 呈现前通知与帧节拍 | P2 | `[ ]` |
| B28.43 | 多窗口与窗口归属 | P1（winit 对齐） | `[ ]` |
| B28.44 | 移动应用服务（触觉/权限/通知/分享/深链/传感器/定位/常亮） | P3（可选，非核心） | `[ ]` |

---

## 8. “全平台完整”的判定标准

对每个后端，下列问题都必须有答案，且答案可被统一探针复跑：

1. `Platform` 的每个方法：真实来源 / 明确的 `None`/`Unsupported`，二选一。
2. `capabilities()` 的每个 `true` 都有对应方法（由现有门禁保证），每个 `false` 都不是
   “忘了实现”而是记录在 `status.md` 的边界。
3. 至少有一个该平台上的运行证据（CI 或人工），且证据是**统一探针**的输出，而不是只跑集成测试。
4. 文档（`status.md`、三语 cookbook、CHANGELOG、日志）与代码一致；`check_changelog_sync.sh`
   与 `check_capability_flags_match_their_methods.py` 通过。
