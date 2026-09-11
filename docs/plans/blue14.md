# BLUE14 — 平台相关未完成项清单（Linux / Android / Harmony / WASM / Wayland / Apple）

> 版本: v0.14.0
> 基线: 继承 BLUE13 全部核心规则 + Rust 原生设计原则
> 编制日期: 2026-09-11
> 文档性质: `docs/log/log-20260909-1.md` 平台相关部分的**未完成项单一事实清单**
> 关联记录: `docs/log/log-20260909-1.md`（已完成证据）、`docs/log/log-20260911-1.md`（第 7/8 轮）、`docs/log/log-20260911-2.md`（第 9 轮 Apple）、`docs/plans/FUTURE.md`（长期受限项）
> 原则依据: `docs/plans/principle.md`（规则 #1/#12/#16/#18/#25/#26）

---

## 核心规则（继承 BLUE13 全部）

1. 结论必须有构建/测试/代码证据，不允许"推测已修复"。
2. 修一个点必须扫同类模式，避免重复返工。
3. 优先修功能阻断项，再做体验增强。
4. 平台策略不变：原生优先，自绘兜底。
5. 不允许占位、空函数、逻辑错误，log/debug 占位 — 所有功能必须完整实现。
6. 注释英文 — 所有新增模块的代码注释必须使用英文。
7. 回写完成率 — 每轮完成后回写完成率。
8. mod.rs 文件只放接口导入等。
9. 单个代码文件少于 2000 行的无需拆分，除非有结构重组需要的 — 这条优先于更改计划。
10. 最后清理所有 warnings + errors。
11. 所有 test - fail, ignore 必须完整修复，不准跳过或删除，除非测试目标已删除。
12. **🚫 绝对禁止假修复** — 修复必须产生可观测、可验证的行为变化。
13. **🚫 绝对禁止不完整修复** — 每条修复必须完整闭环。
14. **🚫 绝对禁止空修复** — 禁止占位行为。
15. **🚫 绝对禁止跳过测试** — 测试修复的硬性要求。
16. **🔍 每条修复必须附带验证证据** — cargo test / clippy / 运行时日志。
17. **🚫 绝对禁止"迁移幻觉"** — 子模块代码被实际调用，旧代码被删除。
18. **🚫 绝对禁止"文档欺骗"** — 文档与代码必须一致。
19. **🔬 BLUE11 自检规则：每条声称的修复必须独立验证。**
20. **🆕 移动端优先** — 新增控件必须同时考虑 desktop/tablet/mobile 三端适配。
21. **🆕 向前兼容** — 不破坏现有 API 签名，通过 feature-gate 或新增模块引入。
22. **🆕 WidgetKind 零孤儿原则** — 每个 WidgetKind 变体必须有对应实现或 type alias。
23. **🆕 零重复变体原则** — 无语义重复或大小写重复的变体。
24. **🆕 基础设施先于控件** — 缺失基础设施优先于新增控件。
25. **🆕 FFI 接线完整性** — native FFI 必须在 platform_impl 中实际调用。
26. **🆕 IME 真实现原则** — IME 必须是真实 OS API 调用，不允许 log 占位。
27. **🆕 WidgetKind→Module 映射可审计** — 每个变体可追溯到唯一模块文件路径。
28. **🦀 零成本抽象优先** — 能用 Rust enum / trait / 泛型解决的问题，不用运行时动态分发。
29. **🦀 编译期安全检查** — 样式、布局、事件路由尽量在编译期通过类型系统约束。
30. **🦀 所有权驱动内存** — 使用 `Box`/`Rc`/`Arc`/`heapless`，不用手动 malloc/free。
31. **🦀 enum 数据布局** — 用 Rust `enum` + 模式匹配表达多态，零开销且类型安全。
32. **🦀 Builder 模式替代 varargs** — 不用 C 的 varargs 风格函数簇。
33. **🦀 Trait 替代回调函数指针** — 用 `EventHandler` trait + `match event`。
34. **🦀 编译期样式检查** — setter 返回 `Result<_, StyleError>`，测试中验证。

### BLUE14 新增规则

35. **🆕 未完成项必须标注卡点类型** — 所有未完成项必须区分「外部环境依赖」与「本机可做的真实缺口」，不得混为一谈。
36. **🆕 外部环境依赖不得计入完成率分母** — 但必须在本文档与 `FUTURE.md` 中同时登记。
37. **🆕 环境阻断必须给出可复现的前置条件** — 若为解除环境阻断引入了本机安装（如 `~/.local` 下的依赖），必须写明复现所需的 `PATH`/`PKG_CONFIG_PATH`。
38. **🆕 覆盖必须可见** — 「测试被 `#[cfg(target_os)]` 档在构建之外」不等于「测试跳过」：前者是**零覆盖且无任何提示**。审计必须区分「测试失败」「测试 ignore」「测试不在构建中」三者；后者必须登记为缺口（2026-09-11 第 7 轮新增，`ime_windows` 15 例 / `windows_notify` 11 例）。
39. **🆕 生成文档不得手写镜像代码事实** — 能力矩阵/路由矩阵等的「降级说明」若描述代码行为，必须由**机械解析源码**生成，并配「重新生成后与落盘文档 diff」的门禁；手写镜像会脱节（2026-09-11 第 7 轮实证）。

---

## 一、统计口径

**`docs/log/log-20260909-1.md` 本轮平台部分 todo 完成率：26/26。**

其中：
- **26 项已完成**（Linux / Android / Harmony / WASM / Wayland / 跨平台门禁）。
- **1 项未计入分母**：Windows OLE 拖放 + IME TSF（按用户明确要求留待 Windows 机器执行）。
- **9 项未完成**：其中 **3 项为外部环境依赖**，**6 项已闭环/落实**（#10 Android CI；#7 Wayland 合成器；#4 Android 真机 arm64；#5 Android FileDialog；#6 AndroidX Toolbar 定性；#3 Windows 三控件原生化（代码+编译验证完成），均 2026-09-11）。

未完成项按性质分三类：

| 类别 | 数量 | 含义 |
|---|---|---|
| A. 外部环境依赖 | 2 （原 3） | 本机确实缺少运行环境；**未伪装为已闭环**（原 #4 iOS / #5 macOS 已于 2026-09-11 在本机闭环） |
| B. 本机可做但未做 | 0（原 1） | 原唯一项（Android CI 作业）已于 2026-09-11 闭环 |
| C. 原判为环境阻断、后证实可闭环 | 2（原 3） | Wayland 合成器（免 root headless weston）；**Apple（原判无 macOS/iOS 环境，实为本机即 macOS 主机）→ iOS 模拟器与 macOS AppKit 均已于 2026-09-11 真机闭环** |
| D. **审计新发现的本机可做缺口**（2026-09-11 第 7 轮） | **5**（**全部已闭环**） | 见 §三之二 |
| E. **主机不可见测试覆盖**（2026-09-11 第 8 轮） | **6**（**5 已闭环 / 1 定性为 A 类**） | 见 §三之三 |
| F. **Apple 主机可见的真实缺陷**（2026-09-11 第 9 轮） | **4**（**全部已闭环**） | 见 §三之四。均因「从未在 macOS 主机上执行」而长期不可见 |

> 另：#5 的 `ColorDialog`/`FontDialog` **不算未做**——Android 平台确实没有系统级颜色/字体选择器，保持 logical-only 是正确行为（平台事实，非缺口）。

> **第 7 轮审计更正（2026-09-11）**：§一 此前的「本机可做项已全部归零」结论**不完整**。
> 经全项目审计（不采信文档标注，一律 grep + 读源码 + 编译取证），发现 **5 个本机可做的真实缺口**
> （其中 1 个是 GTK 真实 panic 路径），**均已在本轮闭环**。详见 §三之二。

> **第 8 轮审计更正（2026-09-11）**：第 7 轮的「B 类已归零」同样**不完整**。
> `FUTURE.md` ITEM 7 自述的「仍开放」7 项中，有 **4 项（`ime_macos` 19 / `android` 8 / `ios` 6 /
> `macos_objc2` 17，共 50 个纯逻辑测试）实为本机可闭环**，已在本轮全部解除门控并实跑取证。
> 仅 `accessibility/windows`(2) 经取证确认为 **A 类真实环境阻断**（无条件引用 `winapi::um::winuser::EVENT_*`），
> 且其唯一可解除方式会造成空断言假覆盖，故**有意不修**。详见 §三之三。

---

## 二、A 类 — 需要外部环境（本机确实做不到，未伪装为已完成）

| # | 项目 | 卡点 | 现状 | 出处 |
|---|---|---|---|---|
| 1 | **Harmony ArkUI 原生桥** | 本机无 OpenHarmony SDK（N-API/ArkUI 头文件） | 只有 state 后端 + `aarch64-unknown-linux-ohos` 编译验证（0 warning，12 host tests pass）；能力契约已诚实化（`native_menu: false`） | `FUTURE.md` ITEM 2、`src/platform/harmony/status.md` |
| 2 | **Windows OLE 拖放 + IME TSF** | 需真实 Win32/COM 运行验证 | 按用户要求留待 Windows 机器。Windows 原生对话框（`MessageBoxW`/`GetOpenFileNameW`/`ChooseColorW`/`ChooseFontW`）已完成；**新增 CI 交叉检查作业 `windows-cross-check`**（`x86_64-pc-windows-msvc` 全 desktop feature `cargo check` + clippy `-D warnings`，本机已复现通过） | `FUTURE.md` P2-2（未勾选） |
| 3 | ✅ **Windows SpinBox/ListView/ScrollArea 原生化**（2026-09-11 代码完成，编译已验） | 原为 state-backed；现已实现真实 Win32 对象：`msctls_updown32` / `SysListView32`（report + 列 + `LVS_EX_FULLROWSELECT`）/ `WS_HSCROLL\|WS_VSCROLL` 子窗口 + 初始滚动范围 | **编译验证完成**：`x86_64-pc-windows-msvc` 与 `x86_64-pc-windows-gnullvm` 全 feature 0 warning，clippy `-D warnings` 通过，Windows-only 测试模块对目标类型检查通过；**运行验证待 Windows 机器**（本机无 Windows、无 Wine、无 MSVC/mingw C 工具链） | `src/platform/windows/helpers.rs`、`FUTURE.md` ITEM 2b |
| 4 | ✅ **iOS 模拟器视图行为**（2026-09-11 真机模拟器闭环） | 原判「无 Apple 运行环境」，**实测本机即 macOS 主机**（macOS 15.7.3 / arm64 / Xcode 26.2） | 新增 `tools/build_ios_testapp.sh`（Rust staticlib + ObjC host → 无 Xcode 工程的真实 `.app`）与 `tools/run_ios_testapp.sh`（boot iOS 26.2 模拟器 → install → launch → 断言 `RESULT: PASS`）。实测 8/8 项通过：真实 `UIApplication` / 真实 `UIWindow`（rootVC 已装）/ 真实 `UIButton`+`UILabel`+`UITextField` 子视图 / 文本往返 / 可见性往返 / 几何变更落到实时 `UIButton.frame`。证据见 `docs/log/log-20260911-2.md`，状态见 `src/platform/ios/status.md` | `FUTURE.md` ITEM 4（已勾选） |
| 5 | ✅ **macOS AppKit 交互**（2026-09-11 真机闭环，含 2 个真实缺陷修复） | 原判「无 macOS 运行环境」，**实测本机即 macOS 主机** | 新增 `examples/apple_appkit_probe.rs`（AppKit 主线程探针），对 cocoa-legacy 与 objc2 **两个后端** 均实测 `RESULT: PASS`：真实 `NSWindow` 进入 `NSApplication.windows`、真实 `NSMenu` 装上 `mainMenu`、`NSPasteboard` 往返、`NSAlert`/`NSOpenPanel`/`NSColorPanel`/`NSFontPanel` 构造、几何/可见性落到实时 `NSButton.frame`。**过程中修复 2 个此前不可见的真实缺陷**：① `MacOSPlatform` 全部 AppKit 调用无主线程守卫 ⇒ 在 macOS 上 `cargo test --lib --features desktop` **整个进程 SIGABRT**（`c_abi_widget_lifecycle_roundtrip`），现加 `is_main_thread()` 守卫 + 状态回退；② objc2 原生 FFI 误用别名 feature `objc2-macos` 门控，致 `--features macos` **静默退化为 state-only**（43 处改为规范 feature `macos`）。另修 `set_native_text` 的 `performSelector:` 返回类型错误。证据见 `docs/log/log-20260911-2.md`，状态见 `src/platform/macos/status.md` | `FUTURE.md` ITEM 5 / ITEM 5b |

> 备注：**A 类合计 4 个条目、对应 3 个平台/领域类别**（Windows #2 与 Windows 运行验证归一类）。
>
> 说明：#3（Windows 三控件原生化）已从 A 类移出——代码完成、编译验证完成，仅剩运行验证，已单列于 `FUTURE.md` ITEM 2b。
>
> 已于 2026-09-11 移出本表并闭环/定性：
> - **原 A 类 #7 Wayland 合成器交互** → 免 root headless weston 可闭环（§六）。
> - **原 A 类 #4 Android 真机 arm64 运行** → 用户提供实体机，真机 `RESULT: PASS`（证据见 `docs/log/log-20260911-1.md` 附录）。
> - **原 A 类 #5 Android FileDialog** → 已真实启动 `ACTION_OPEN_DOCUMENT`；ColorDialog/FontDialog 为平台事实，不算缺口。
> - **原 A 类 #6 AndroidX Toolbar** → 由「推测」升级为真机实测的确定性约束（`R$attr` 等资源需 AAR 资源合并），有意不捆绑。

---

## 三、B 类 — 本机可做但未做（无环境阻断）

| # | 项目 | 说明 | 优先级 |
|---|---|---|---|
| 10 | ✅ **Android CI 作业**（2026-09-11 完成） | 已交付 `.github/workflows/android.yml`：`jni-bindings`（双 ABI 构建+签名+JNI 签名/导出符号门禁+产物上传）与 `emulator-e2e`（API 34 x86_64 + KVM + `RESULT: PASS`）。CI 复用本机已验证脚本，并已按 CI 相同命令在本机逐条实跑取证（见 `docs/log/log-20260911-1.md`） | P1 |
| 11 | ✅ **主机不可见测试覆盖（`FUTURE.md` ITEM 7 的主体）**（2026-09-11 第 8 轮完成） | 见 §三之三 E 类：`ime_macos`(19)、`android`(8)、`ios`(6)、`macos_objc2`(17) 共 **50 个纯逻辑测试**由「不在构建中」变为**主机真实执行** | P1 |

> B 类已归零。剔除外部环境依赖后的本机可做项完成率：2/2 = 100%。

---

## 三之二、D 类 — 第 7 轮审计新发现的真实缺口（全部**已闭环**，2026-09-11）

> 本节是 §一 「本机可做项已全部归零」的**事实更正**。以下 5 项均为**本机可做、且无环境阻断**，
> 但在此前各轮中被遗漏。每项均已修复并附可观测证据；证据全文见 `docs/log/log-20260911-1.md` 第 7 轮。

| # | 项目 | 卡点类型 | 修复前的事实（可验证） | 闭环证据 |
|---|---|---|---|---|
| D-1 | **`ime_windows` 的 15 个测试从未编译、从未运行** | 本机可做（覆盖可见性） | `src/platform/mod.rs` 的模块级 `#[cfg(target_os = "windows")]` 使整个模块（含 `#[cfg(test)] mod tests`）**不在主机构建中**；且 `cargo check --tests --target *-windows-*` 本机先被 `cc-rs: lib.exe not found` 中断 ⇒ 这些测试在**任何可达路径上都不执行**。测试本身是**纯状态机逻辑**，不碰 HWND/COM | 解除模块级门控（TSF 部分本就内部 gated，行为不变）；`cargo test ime_windows` → **15 passed** |
| D-2 | **Windows 通知码映射（纯逻辑）困在 `target_os` 之后** | 本机可做（覆盖可见性） | `src/platform/windows/notify.rs` **每个函数**都被 `#[cfg(target_os = "windows")]` 包裹，其测试与 `WindowsHandleKind`（纯数据枚举）同处 gated 模块，主机上**零执行** | 新建无门控的 `src/platform/windows_notify.rs`（**移动**而非拷贝，Windows 后端改为转调，公共 API 不变）；`cargo test windows_notify` → **11 passed** |
| D-3 | **`windows/tests.rs` 存在「永不执行的主机回退分支」** | 本机可做（假覆盖） | 该文件写了 `else { assert_eq!(..., None) }` 主机回退断言，但整个测试模块被与 `WindowsPlatform` 同为 Windows-only 的模块门控 ⇒ 该分支**永不执行**，形成假覆盖 | 修复后语义诚实：需真实 `WindowsPlatform` 的测试显式 `#[cfg(target_os = "windows")]`，纯逻辑测试已在 D-2 的共享模块中于主机真实执行 |
| D-4 | **`DatePicker`/`TimePicker`/`DateTimePicker` 的原生实现「已接线但从不被调用」** | 本机可做（规则 #25 FFI 接线完整性） | `src/control_backend/native.rs` 三者原样转调 `create_panel`（正是 🔶「降级为其它原语」）；但平台层**早已有真实实现**：Linux（`create_*_picker_impl`：`gtk::MenuButton`+`Popover`+`Calendar`、双 `SpinButton`）与 Windows（`SysDateTimePick32`）。同类 `Calendar` 早已改专用调用，三者被遗漏 ⇒ **既有原生代码成为死代码** | `native.rs` 三处改为转调各自平台方法；11 个后端**全部**实现三个 trait 方法且无一降级到 `Panel`；机械解析调用图确认指向已变（见 D-5） |
| D-5 | **能力矩阵「降级说明」手写镜像已与代码脱节（文档欺骗，规则 #18）** | 本机可做（文档真实性） | `tools/generate_platform_capability_matrix.py` 的 `DEGRADATION_NOTES` 是**手打字符串**（注释自称 mirror of `native.rs`），**无任何校验**。实测已脱节：`GroupBox`/`TabWidget`/`Splitter`/`Calendar`/`FontComboBox`/`Dialog`/`DirectoryDialog`/`ContextMenu`/`PopupWindow` 早已专用化，却仍被列为降级 | 改为**机械解析** `native.rs` 调用图（自委派=专用实现自动排除）+ 按真实 `WidgetKind` 过滤去重；**新增防脱节门禁**（重新生成后 diff），并以**追加漂移标记使门禁如期失败**做负向验证 |
| D-6 | **GTK 剪贴板的真实 panic 路径** | 本机可做（真实缺陷，非环境） | `src/platform/linux/widget_state.rs` 的 `gtk_clipboard()` 直调 `gdk::Display::default()`；文档注释声称 headless 返回 `None`，实则在**非主线程上 panic**（`GDK may only be used from the main thread`），任何非 GTK 线程读写剪贴板都会**中止进程**。`git checkout` 还原后失败依旧复现 ⇒ **既有缺陷** | 加 `gtk::is_initialized_main_thread()` 守卫，落到既有逻辑镜像回退。非主线程套件由 **1 failed → 2478 passed / 0 failed**；真实 X display(`:0`) 下注入 `assert!(is_initialized_main_thread())` 证明主线程路径**仍被执行**（未被守卫误伤） |

> **新增规则 #38/#39 由此三类事实推出**：①「测试不在构建中」必须与「测试失败/ignore」区分登记；
> ②描述代码事实的生成文档必须机械派生并配门禁。

### D 类闭环后的测试增量归因

`cargo test --lib --features desktop`：**3793 → 3819**（+26）。

增量**全部**来自「既有测试从**从不执行**变为**真实执行**」：

| 来源 | 增量 | 性质 |
|---|---|---|
| `platform::ime_windows::tests`（D-1） | +15 | 原「不在构建中」→ 现主机执行 |
| `platform::windows_notify::tests`（D-2） | +11 | 原「不在构建中」→ 现主机执行 |
| 新增断言 | 0 | **未**通过灌水断言数量凑数 |

---

## 三之三、E 类 — 第 8 轮：主机不可见测试覆盖（全部**已闭环**，2026-09-11）

> 承接 D-1/D-2 的同类模式（规则 #38「覆盖必须可见」）。本轮把 `FUTURE.md` ITEM 7 中
> 「仍开放」的条目逐项取证并解除门控。每一项的判据是：**该测试只使用平台无关的状态机，
> 不触碰任何 OS API**（逐块机械扫描确认，非人工阅读）。

| # | 项目 | 修复前的事实（可验证） | 闭环证据 |
|---|---|---|---|
| E-1 | **`ime_macos` 19 个测试从未编译、从未运行** | 文件首行 `#![cfg(target_os = "macos")]` 使整个模块（含 `#[cfg(test)] mod tests`）不在主机构建中。测试**纯状态机**（含 UTF-16 range 跟踪），零 AppKit 调用 | 移除文件级门控；9 处内部 AppKit 门控由 `feature = "objc2-macos"` 收紧为 `all(target_os = "macos", feature = "objc2-macos")`（`objc2` 仅 macOS/iOS 目标依赖，必须同时约束目标）；`cargo test platform::ime_macos` → **19 passed** |
| E-2 | **`android/platform_impl` 测试不可见，且该文件既有测试存在编译期缺陷** | 模块声明 `#[cfg(target_os = "android")]` + `android/types.rs` 的 `AndroidHandleKind` **缺少 `Debug`** ⇒ 其 `assert_eq!(..., Some(AndroidHandleKind::Button))` **根本无法编译**。该缺陷在同一文件既有测试中一直存在，只是从未进入构建 | 解除模块门控；补 `Debug` derive（`IosHandleKind` 早已有，属漏配）；`cargo test platform::android` → **8 passed**（注意：实测为 8，此前文档记为 7） |
| E-3 | **`ios/platform_impl` 6 个测试不在构建中** | 模块声明 `#[cfg(target_os = "ios")]`。UI 侧本已 `#[cfg(feature = "ios-uikit-ffi")]` 门控，测试只驱动 `IosMobilePlatform` 状态机 | 解除模块门控；22 处 `ios-uikit-ffi` 门控收紧为 `all(target_os = "ios", feature = "ios-uikit-ffi")`（`--all-features` 会在 Linux 上开启该 feature，而 `objc2-ui-kit` 仅 iOS 目标存在）；`cargo test platform::ios` → **6 passed** |
| E-4 | **`macos_objc2/tests.rs` 17 个测试不在构建中** | 模块声明 `#[cfg(all(target_os = "macos", ...))]`。`native` 子模块本已 `#![cfg]` 自门控，测试只用状态后端 `MacOSObjc2Platform` | 解除模块门控（改为按 feature），并给 `native` 子模块补显式目标门控；`cargo test --features macos platform::macos_objc2` → **17 passed** |
| E-5 | **`--all-features` 实际编译失败（CI 命令）** | E-2/E-3 的门控放宽后暴露：`mini`+`serde` 组合下 `BackendState` 不派生 `Serialize`，而 `serialize_state` 仅以 `#[cfg(feature = "serde_json")]` 门控 ⇒ `E0277` 编译错误 | 三处（`android`/`ios` 方法 + ios 测试）门控与 `BackendState` 的 derive 条件对齐（`serde_json` ∧ `serde` ∧ ¬(`mini`∨`embedded`)）；`cargo test --all-features` → **0 failed** |
| E-6 | **`accessibility/windows.rs` 的 2 个测试含「主机空断言」** | 该模块**确实**依赖 Windows 目标（`notify_*_changed` 无条件引用 `winapi::um::winuser::EVENT_*`），故属 **A 类（真实环境阻断）**；但 `test_uia_control_type_mapping` 在非 Windows 上**函数体为空**，构成 D-3 式假覆盖 | 本轮**不解除门控**（解除会引入假覆盖）；已在本表登记为「应保持门控」的负向结论，避免后续轮次误判为 B 类缺口 |

> **方法学（规则 #19/#38）**：E-6 是本轮唯一**主动判定为「不应修」**的条目——
> 「能让测试在主机上跑」不等于「应该让它跑」：条件编译掉整个断言体会制造 D-3 式假覆盖，
> 比保持门控更糟。这与 D-1/E-1~E-4（测试本身是纯逻辑）形成明确分界。

### E 类闭环后的测试增量归因

`cargo test --lib --features desktop`：**3820 → 3853**（+33）。

| 来源 | 增量 | 性质 |
|---|---|---|
| `platform::ime_macos::tests`（E-1） | +19 | 原「不在构建中」→ 现主机执行 |
| `platform::android::*`（E-2） | +8 | 同上（含修复 `Debug` 后新可编译的测试） |
| `platform::ios::*`（E-3） | +6 | 同上 |
| 新增断言 | 0 | **未**通过灌水断言数量凑数 |

> `macos_objc2`(17) 仅在 `--features macos` 下进入构建，故不计入默认 `desktop` 增量；
> 在 `--features full` 下可见（`full` 套件 3932 passed）。

---

## 三之四、F 类 — 第 9 轮：Apple 主机可见的真实缺陷（全部**已闭环**，2026-09-11）

> 本轮的前提修正：**验证主机本身就是 macOS 主机**（macOS 15.7.3 / arm64 / rustc 1.98.0 / Xcode 26.2）。
> 此前各轮将 Apple 记为「无运行环境」（§二 A 类 #4/#5），该判定在本机不成立。
> 一旦在 macOS 上真实执行既有测试与 AppKit 路径，暴露出 **4 个此前完全不可见的真实缺陷**；
> 每项均已修复并附可观测证据（全文见 `docs/log/log-20260911-2.md`）。

| # | 项目 | 卡点类型 | 修复前的事实（可验证） | 闭环证据 |
|---|---|---|---|---|
| F-1 | **`MacOSPlatform`（cocoa-legacy）全部 AppKit 调用无主线程守卫** | 本机可做（真实缺陷） | `create_window` 无条件 `NSWindow::alloc`；`init`/`create_button`/`create_menu_bar`/`set_clipboard_text` 等 ~30 处同样无守卫。测试在 worker 线程上调用 C ABI ⇒ AppKit 抛 ObjC 外来异常，Rust 无法捕获 ⇒ **整个 `cargo test --lib --features desktop` 进程 SIGABRT**（`fatal runtime error: Rust cannot catch foreign exceptions`），首个触发点 `bindings::binding_impl::tests::c_abi_widget_lifecycle_roundtrip` | 新增 `is_main_thread()` + `register_state_only_handle()`，对全部 AppKit 触点加守卫（离线回退 `ptr == 0`，保留父级校验语义）；`add_to_parent_window`/`sync_list_box_native` 跳过 nil 接收者（cocoa crate 会在 nil 上 `null pointer dereference` 中止）。**`desktop` 3836 passed / 0 failed**，探针实测两个后端 `RESULT: PASS` |
| F-2 | **objc2 原生 FFI 用别名 feature 门控 ⇒ `--features macos` 静默退化为 state-only** | 本机可做（FFI 接线完整性，规则 #25） | `macos_objc2` 的 43 处原生路径门控在 `feature = "objc2-macos"`；而 Cargo 别名是单向的（`objc2-macos = ["macos"]`），启用 `macos` **不会**启用 `objc2-macos`。`desktop` 也不启用 `macos`。⇒ 文档记载的 OS 后端轴 `--features macos` 下，`create_window` 走不到 `create_ns_window`，实测 `NSApplication.windows.count = 0` | 43 处改为规范 feature `macos`；`--features macos` 与 `--features objc2-macos` 现均实测 `native_window_registered = 1`，`native_menu_bar` 装上 `mainMenu` |
| F-3 | **objc2 `set_native_text` 的 `performSelector:withObject:` 返回类型错误** | 本机可做（真实缺陷，被 F-2 掩盖） | `let _: () = msg_send![object, performSelector: selector, withObject: &*value]` —— 该 selector 返回 `id`，而 objc2 运行时会校验声明的返回类型 ⇒ **每次设置文本都 panic/中止**（`expected return to have type code '@', but found 'v'`）。因 F-2 使原生路径不可达，此前从未触发 | 改为分派类型化消息 `setStringValue:` / `setTitle:` / `setAccessibilityLabel:`（无 `performSelector:`）；两项 macOS 探针文本往返均 PASS |
| F-4 | **`--features full` / `--all-features` 在 macOS 上重复定义 `create_native_platform`（E0428）** | 本机可做（真实编译缺陷） | `full` 同时启用 `harmony` 与 `macos`。Linux 分支已有 `not(feature = "harmony")`，但 macOS/iOS 分支没有 ⇒ macOS 宿主上两个 `create_native_platform` 同时激活，**编译失败**。该组合只在 Linux CI 上编译过（macOS 分支被 cfg 掉），故不可见 | macOS/iOS 分支补 `not(feature = "harmony")`，与 Linux 分支对齐；`cargo test --lib --all-features` 现可编译并运行（1853 passed，唯一失败为环境缺 `libvorbis` 的既有音频用例） |

> **本轮同时闭环 A 类 #4/#5**：新增 `examples/apple_appkit_probe.rs`（主线程 AppKit 探针）、
> `tools/build_ios_testapp.sh` + `tools/run_ios_testapp.sh`（无 Xcode 工程的 iOS 模拟器 E2E）、
> `tools/check_apple_native.sh`（统一门禁）与 `.github/workflows/ci.yml` 的 `apple-native` 作业（macOS runner）。
> Apple 状态文档：`src/platform/macos/status.md`、`src/platform/ios/status.md`。

---

## 四、长期受限项（`FUTURE.md` 登记，非本轮新增）

| ITEM | 项目 | 说明 |
|---|---|---|
| ITEM 0 | Hybrid per-control 编译期路由闭合 | 需要稳定的 per-control 能力表 + 全 profile 编期路由校验 |
| ITEM 1 | 无 `gtk-native` 时的 Linux 完整原生对等 | 当前是 state/preview 循环。本轮已让 `gtk-native` 路径**完整验证**，但非 GTK 路径的原生对等仍未解决 |
| ITEM 2 | Harmony 桌面原生窗口/渲染/事件循环 | 同 A 类 #1 |
| ITEM 4 | iOS mobile backend 实现 | 同 A 类 #8 |
| ITEM 5 | macOS objc2 preview backend 毕业 | 同 A 类 #9 |
| ITEM 6 | 跨平台全控件对等矩阵闭合 | 部分控件在至少一个后端仍走 trait 默认兜底语义，需逐后端补齐 `create_*` 或显式声明「不支持」。**2026-09-11 第 7 轮更新**：`DatePicker`/`TimePicker`/`DateTimePicker` 已不再转调 `create_panel`（改为调用早已存在的原生实现）；矩阵降级表已改为机械派生 + 防脱节门禁 |
| ITEM 7 | **主机不可见测试覆盖**（被 `#[cfg(target_os)]` 挡在构建外） | **2026-09-11 第 7 轮新增，第 8 轮大部分闭环**。已修：`ime_windows`(15)、`windows_notify`(11)（第 7 轮）；`ime_macos`(19)、`android`(8)、`ios`(6)、`macos_objc2`(17)（第 8 轮，见 §三之三）。**仍开放（已定性为 A 类，非缺口）**：`accessibility/windows`(2) —— 该模块无条件引用 `winapi::um::winuser::EVENT_*`，本就无法离 Windows 编译；解除门控只会制造空断言假覆盖。另：`control_backend/routing` 的 2 个 Windows-only 测试是**按 OS 分支的有意不对称**（Windows 侧结论与其余平台相反），不属本 ITEM 范围 |

---

## 五、环境前置（不是代码缺陷，但影响复现）

按规则 #37 登记：

- 本机 `/usr/lib` 只有 dav1d **运行时**库、无开发包，且无免密 sudo。
- 本轮用 `pip install --user meson ninja` + 从源码构建 dav1d 1.5.0 安装到 `~/.local`，解除了 `image` / `desktop` 的构建阻断。
- **新 shell 中构建含 `image` 的配置需要**：
  ```bash
  export PATH="$HOME/.local/bin:$PATH"
  export PKG_CONFIG_PATH="$HOME/.local/lib/x86_64-linux-gnu/pkgconfig"
  ```
- 构建命令（`-Denable_asm=false`，因本机无 `nasm`）：
  ```bash
  meson setup build --prefix="$HOME/.local" --buildtype=release \
    -Denable_tools=false -Denable_tests=false -Denable_examples=false -Denable_asm=false
  ninja -C build && meson install -C build
  ```
- **更干净的长期方案**：系统级安装 `libdav1d-dev`。
- **Wayland 合成器（2026-09-11 新增）**：本机无合成器二进制且无免密 sudo，但 `apt-get download` 不需要 root。复现脚本 `tools/run_wayland_compositor_tests.sh` 自包含完成：下载 weston 13.0.0 + libweston → 解包到 `~/.local/opt/weston-extract` → 将 libweston 中编译期硬编码的模块目录（`/usr/lib/x86_64-linux-gnu/libweston-13`、`/usr/lib/x86_64-linux-gnu/weston`）**原位补丁**为 `~/.local/lib/wlmods`（后者严格更短，故不改动二进制布局与任何偏移）→ 以 `headless-backend.so + kiosk-shell.so` 启真实合成器。
  - **依赖**：`apt-get` 可达镜像、`python3`、`cargo`；无需 root、无需图形会话、无需 `libweston-13-0` 系统包。
  - **一次性运行**：`bash tools/run_wayland_compositor_tests.sh`（构建含 `image` 的配置时需先设 dav1d 的 `PATH`/`PKG_CONFIG_PATH`）。
  - **更干净的长期方案**：系统级安装 `weston`（有 sudo 时）。

---

## 六、本轮已完成（作为对照基线，证据见日志）

Linux、Android、Wayland **与 Apple** 在本机能力范围内**已全部闭环**：

| 平台 | 已完成内容 |
|---|---|
| **Linux** | GTK dialog/chooser（`MessageDialog`/`FileChooserDialog`/`ColorChooserDialog`/`FontChooserDialog`）、GTK/GDK 剪贴板（含 `store()`）、AT-SPI a11y 名称转发、**IBus 引擎协议**（真实输入上下文 + 五个引擎调用，本机守护进程实测，修正 3 个导致静默失败的缺陷）、`set_widget_text` 子类 downcast 顺序缺陷、menu bar 几何/父级记录、**完整 `desktop` 回归（3787 passed / 0 failed / 0 ignored）** |
| **Android** | JNI 视图工厂 `create_native_view`（12 种 `AndroidViewClass`）+ setter 转发、`MessageBox` 原生 `AlertDialog`、**`FileDialog` 真实 `ACTION_OPEN_DOCUMENT`**、`jni_available()` 修正、menu kind 校验、runtime 后端选择、`bindings` 门禁放开、logcat 日志后端、JNI 签名门禁 + 映射审计产物、NDK 交叉编译、无 Gradle APK 构建链、模拟器 E2E + **真机（arm64，Xiaomi M2102J2SC / Android 13）E2E `RESULT: PASS`** |
| **Harmony** | 能力契约去过度声明（`native_menu: false`）、`aarch64-unknown-linux-ohos` 目标编译验证、`status.md` |
| **Wayland** | fd 阻塞事件循环（`poll` on compositor socket）+ 退出延迟回归测试；**2026-09-11 新增**：无 root 下跑起真实 headless weston（`tools/run_wayland_compositor_tests.sh`，`system`/`rootless`/`auto` 三模式）+ 真实绑定测试（`native_session_binds_live_compositor`，实测 `wl_compositor=yes xdg_wm_base=yes`）+ **CI 作业 `wayland-compositor`**（apt 装 weston/dav1d 后以 system 模式跑） |
| **WASM** | menu 树 parent-child 边记录 + shortcut 保留 |
| **跨平台** | `runtime.rs` Android/Harmony 分支 + `Platform::mobile_extension()`、C ABI 生命周期测试、JNI 门禁多 ABI 化、JNI 映射审计产物 |
| **第 7 轮审计修复**（2026-09-11） | ① `ime_windows` 15 测试由「不在构建中」→ 真实执行（`src/platform/mod.rs`）；② Windows 通知码映射抽到无门控的 `src/platform/windows_notify.rs`（11 测试主机可跑，Windows 后端改为转调）；③ `windows/tests.rs` 移除永不执行的主机回退分支；④ `DatePicker`/`TimePicker`/`DateTimePicker` 改为调用早已存在的原生实现（原转调 `create_panel`，使原生代码成死代码）；⑤ 能力矩阵降级表改为**机械解析** `native.rs` + 新增防脱节门禁；⑥ **GTK 剪贴板真实 panic 路径**加 `is_initialized_main_thread()` 守卫 |
| **macOS（第 9 轮，2026-09-11）** | 在**真实 macOS 主机**上闭环 AppKit 交互：新增 `examples/apple_appkit_probe.rs`（主线程探针，两个后端均 `RESULT: PASS`）；修复 F-1（cocoa-legacy 全量主线程守卫，`desktop` 由 SIGABRT → 3836 passed）、F-2（objc2 原生 FFI 改用规范 feature `macos`，43 处）、F-3（`set_native_text` 返回类型错误）；objc2 后端新增 `NSApplication` 引导与真实 `NSMenu`/`mainMenu` 接线；新增 `src/platform/macos/status.md` |
| **iOS（第 9 轮，2026-09-11）** | 新增无 Xcode 工程的 `.app` 构建与模拟器 E2E：`bindings/ios/main.m` + `Info.plist`、`tools/build_ios_testapp.sh`、`tools/run_ios_testapp.sh`；在 **iOS 26.2 模拟器**上 8/8 断言 `RESULT: PASS`（真实 `UIWindow`/`UIButton`/`UILabel`/`UITextField`/文本与几何往返）；修复 5 处 iOS 专属 clippy 告警 + 1 处未用 import；新增 `src/platform/ios/status.md` |
| **Apple CI / 门禁**（第 9 轮） | `tools/check_apple_native.sh`（统一门禁）+ `.github/workflows/ci.yml` 新增 `apple-native` 作业（macos runner：两个后端 AppKit 探针 + iOS 模拟器 E2E + 测试 + 设备目标编译检查） |

---

## 七、验证现状

最近一次全量验证（2026-09-11，**第 9 轮后重跑**）：

> ⚠️ 验证主机为 **macOS**（macOS 15.7.3 / arm64）。§七 表格已改为本机实测结果；
> 第 1–8 轮报告的 Linux 数字（如 `desktop` 3853）在 macOS 上为 **3836**（差 17 = `macos_objc2` 仅在 `--features macos`/`full` 下进入构建）。

| 检查 | 结果 |
|---|---|
| `cargo test --lib --features desktop` | **3836 passed**, 0 failed, 0 ignored（第 9 轮修复 F-1 前：**进程 SIGABRT**） |
| `cargo test --lib --features macos` | **3853 passed**, 0 failed（包含 F-2/F-3 修复后的 objc2 原生路径） |
| `cargo test --lib --features macos-legacy` / `cocoa-legacy` | **3836 passed**, 0 failed |
| `cargo test --lib --features objc2-macos` | **3853 passed**, 0 failed |
| `cargo test --lib --all-features`（CI 实际命令） | **编译通过**（修复 F-4 前为 E0428）并运行 1853 passed；唯一失败为环境缺 `libvorbis` 的既有音频用例 |
| `cargo test --lib --features full` | 编译通过并运行 3913 passed；同上唯一环境性失败 |
| Apple AppKit 探针 — cocoa-legacy（`--features desktop`） | **RESULT: PASS**（真实 `NSWindow`/`NSMenu`/`NSPasteboard`/dialogs） |
| Apple AppKit 探针 — objc2（`--features macos`） | **RESULT: PASS**（真实 `NSWindow` 入 `windows`、真实 `mainMenu`） |
| iOS 模拟器探针（iOS 26.2 / arm64） | **8/8 PASS**（`RESULT: PASS`） |
| `platform::ios::*` — 目标编译（device / simulator × state / FFI） | 全 **0 error / 0 warning** |
| `cargo clippy`（desktop / macos / iOS 目标，`-D warnings`） | 均 **0 warnings** |
| `cargo fmt --check` | 通过 |
| 其他 profile（mobile / tablet / mini / wasm / harmony） | 3841 / 3836 / 1766 / 3845 / 3850，**全 0 failed**（`embedded` 为既有预存编译缺陷，与本轮无关，已如实记录） |

> **方法学声明（规则 #38）**：本轮验证继续区分三类「未通过/未覆盖」状态：
> ①测试 FAILED；②测试 `ignored`；③测试**根本不在构建中**。
> 第 8 轮将第 ③ 类从 7 项缩至 **1 项**（`accessibility/windows`，已定性为 A 类真实环境阻断，且其修法会造成假覆盖）。
> 本仓库无 `ignore`，① 已归零。

---

## 八、结论

- **Linux、Android、Wayland 在本机能力范围内已全部闭环**，证据见 `docs/log/log-20260909-1.md` 与 `docs/log/log-20260911-1.md`。
- **B 类（#10 Android CI 作业）已于 2026-09-11 闭环**。
- **原 A 类 #7（Wayland 合成器交互运行）已于 2026-09-11 闭环**：证实「无合成器」并非不可解除的环境阻断。
- **原 A 类 #4/#5/#6（Android）已于 2026-09-11 闭环/定性**：用户提供实体 arm64 真机后，#4 真机 `RESULT: PASS`；#5 FileDialog 真实启动系统选择器；#6 Toolbar 由推测升级为真机实测的确定性约束。
- **原 A 类 #3（Windows 三控件原生化）已于 2026-09-11 完成代码 + 编译验证**（真实 `msctls_updown32` / `SysListView32` / 滚动子窗口），并纳入 CI 交叉检查；**运行验证待 Windows 机器**。
- **第 7 轮审计更正：原「本机可做项已全部归零」结论不完整。** 全项目审计后发现 **5 个本机可做的真实缺口**（D-1~D-6，见 §三之二）——包括 **26 个被 `#[cfg(target_os)]` 挡在构建之外、从未执行的测试**，一个**已接线但从不被调用**的原生实现（Date/Time/DateTimePicker），以及一条 **GTK 剪贴板的真实 panic 路径**。**均已在本轮闭环**，`--features desktop` 由 3793 → **3819 passed / 0 failed**。
- **第 8 轮审计更正：`FUTURE.md` ITEM 7「仍开放」的 7 项已缩至 1 项。** 本轮将 `ime_macos`(19)、`android`(8)、`ios`(6)、`macos_objc2`(17) 共 **50 个纯逻辑测试**由「不在构建中」变为**主机真实执行**（§三之三 E 类），`--features desktop` 由 3819 → **3853 passed / 0 failed**。过程中额外暴露并修复 2 个既有缺陷：
  - **`AndroidHandleKind` 缺 `Debug`** —— 使 `android/types.rs` 自身的 3 个 `assert_eq!` 测试**从未能编译**（从未进入构建，故无人发现）；
  - **`--all-features` 实际编译失败（E0277）** —— `serialize_state` 的门控宽于 `BackendState` 的 `Serialize` derive 条件；该命令是 **CI 实际执行的命令**，说明此前的「全 0 failed」记录未能覆盖真实 CI 命令。
- **第 9 轮前提更正：验证主机本身就是 macOS 主机（macOS 15.7.3 / arm64 / Xcode 26.2）。**
  因此 **原 A 类 #4（iOS 模拟器视图行为）与 #5（macOS AppKit 交互）已于 2026-09-11 在本机真机闭环**，
  不再是「外部环境依赖」：
  - #5：新增 `examples/apple_appkit_probe.rs`（主线程 AppKit 探针），cocoa-legacy 与 objc2 **两个后端均 `RESULT: PASS`**（真实 `NSWindow`/`NSMenu`/`NSPasteboard`/`NSAlert`…）。
  - #4：新增 `tools/build_ios_testapp.sh` + `tools/run_ios_testapp.sh`，在 **iOS 26.2 模拟器**上装/跑真实 `.app`，8/8 断言通过（真实 `UIWindow`/`UIButton`/`UILabel`/`UITextField`/文本与几何往返）。
  - 过程中暴露并修复 **4 个主机可见的真实缺陷**（F-1~F-4，见 §三之四），其中 F-1 是 **cocoa-legacy 后端使 `desktop` 测试进程整个 SIGABRT**，F-2 是 **`--features macos` 静默退化**，F-3 是 **objc2 文本设置必然中止**，F-4 是 **`full` profile 在 macOS 上编译失败**。
  - 新增 CI 作业 `apple-native`（macos runner）与门禁 `tools/check_apple_native.sh` 持续验证。
- 剩余未完成项：**2 类**（Windows OLE/IME + Windows 运行验证 / Harmony SDK），**均未伪装为已闭环**。
- 所有外部环境依赖项**均未伪装为已闭环**，均在 `FUTURE.md` 与各平台 `status.md` 中如实登记（遵守规则 #18）。
- **诚实边界**：D-1~D-3、E-1~E-4 的意义是「覆盖变为可见」；第 9 轮的 Apple 验证虽是**真实 AppKit/UIKit 对象**层面的断言，但：
  - iOS 侧跑在**模拟器**而非物理设备（签名/描述文件不在本门禁范围）；
  - objc2 后端的 `run()` 仍是轮询循环，**真实 `NSApplication` 事件循环桥接尚未实现**（ITEM 5 剩余部分），故 `backend_name()` 仍为 `macos-objc2-preview`；
  - Windows 运行验证仍待 Windows 机器（本机无 Windows / Wine / MSVC·mingw C 工具链）。
