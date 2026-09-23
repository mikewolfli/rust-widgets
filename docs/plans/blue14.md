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
| F. **Apple 主机可见的真实缺陷**（2026-09-11 第 9 轮，含补审） | **6**（**全部已闭环**） | 见 §三之四。均因「从未在 macOS 主机上执行」而长期不可见；F-5/F-6 为探针加强后暴露的「静默无操作」 |
| G. **本轮同源发现的非 Apple 缺陷**（第 9 轮补审） | **1**（**已闭环**） | F-7：`libvorbis` 硬编码，原被误判为「环境性失败」。取证 `brew deps`/`otool -L` 后推翻归因 |
| H. **第 9 轮换方向：FFI 健全性审计** | **2**（**全部已闭环**） | 见 §三之五。G-1：`ime_macos` 的 `ImeCtx` 四份局部定义 ⇒ 3 条原生路径静默失效（已闭环）；G-2：`unsafe impl Sync` 为多余不安全承诺（初记「2 已移除 + 其余待平台验证」，**第 9 轮收尾用删除-编译法全部判定**：另 3 处亦为多余并已删，1 处必要保留 ⇒ 见 §三之十二） |
| I. **第 9 轮换方向：错误路径/资源泄漏** | **2**（**全部已闭环**） | 见 §三之六。I-1：`ffmpeg_encoder` 失败时泄漏临时文件；I-2：`ffmpeg_decoder` 写入失败时泄漏（同类）。均以 RAII guard 闭环并附回归测试 |
| J. **第 9 轮换方向：并发 / panic 安全 / i18n 抖动** | **3**（**全部已闭环**） | 见 §三之七。J-1：i18n 仅靠 mtime 判定变更，粗粒度文件系统会漏检（已改指纹）；J-2：`undo/stack` 测试夹具 `static mut` 数据竞争（改原子）；J-3：双向绑定 `syncing` 标志被 panic 永久卡住（改 RAII guard） |
| K. **第 9 轮换方向：内存增长 / 长期运行** | **1**（**已闭环**） | 见 §三之八。K-1：平台层**无 widget 销毁 API**，动态 UI 会无界积累注册表条目（已为全 11 个后端 + C ABI 补齐）。**同时纠正一个错误结论**：RSS 增长经 `leaks` / ObjC 引用计数 / 纯 AppKit 对照三重取证后确认**来自 AppKit 自身**，非本库泄漏 |
| M. **第 9 轮换方向之五：跨平台 API 契约一致性** | **2**（**全部已闭环**）+ **1 有意不修** | 见 §三之九。M-1：`StubPlatform` 内部自相矛盾（21 个 `create_*` 忽略 `parent`，19 个校验）；M-2：cocoa 后端 3 个控件同类问题。M-3：dialog 不校验父级是 **7/11 后端多数派约定**（顶层模态框语义），**有意不修**并落测试固定该契约 |
| N. **第 9 轮换方向之六：文档与代码机械一致性** | **2**（**全部已闭环**） | 见 §三之十。N-1：`codemap.md` 声称 166 个变体，实际 167；N-2：README 双语文档声称 80+ 控件，实际 167。N-3：新增防脱节门禁 `tools/check_widget_kind_count.sh` 并接入 CI `validation-gates` |
| O. **第 9 轮收尾：全量验证暴露的自引入回归** | **3**（**全部已闭环**） | 见 §三之十一。O-1：`full` profile 下两个 Apple 探针编译失败（`full` 同时启用 `desktop`+`mini`，而探针未排除 `mini`）；O-2：`encode_temp_files()` 废弃方案残留成为死代码（违反规则 #4）；O-3：**自我纠正**——mobile/tablet/wasm/harmony 的测试计数文档记录错误。**根因：此前验证未带 `--all-targets`** |
| P. **H-2 闭环：逐个验证残留的 `unsafe impl Sync`** | **3 多余（已删）+ 1 必要（保留）** | 见 §三之十二。**方法：删除-编译法**（非读代码猜）。`EventHandlerContext` / `LinuxPlatform` / `TsfThreadMgr` 删除后 **0 error** ⇒ 多余承诺，已删；`AndroidPlatform` 删除后 **55 error** ⇒ 必要，保留。原记述「其余待平台验证」**不准确**：本机用 `x86_64-pc-windows-gnu` 交叉编译即可判定 |

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
| F-4 | **`--features full` / `--all-features` 在 macOS 上重复定义 `create_native_platform`（E0428）** | 本机可做（真实编译缺陷） | `full` 同时启用 `harmony` 与 `macos`。Linux 分支已有 `not(feature = "harmony")`，但 macOS/iOS 分支没有 ⇒ macOS 宿主上两个 `create_native_platform` 同时激活，**编译失败**。该组合只在 Linux CI 上编译过（macOS 分支被 cfg 掉），故不可见 | macOS/iOS 分支补 `not(feature = "harmony")`，与 Linux 分支对齐；`cargo test --lib --all-features` 现可编译并运行 **1858 passed / 0 failed**（见 F-7） |
| F-7 | **`libvorbis` 硬编码，被误判为「环境性失败」** | 本机可做（真实缺陷） | `src/audio/ffmpeg_encoder.rs` 把 Ogg 编码器硬编码为外部库 `libvorbis`。取证：`brew deps ffmpeg` 无 vorbis、`brew cat ffmpeg` 无引用、`otool -L` 无链接 ⇒ **Homebrew ffmpeg 从不编译 libvorbis**，装独立库无效。任何「无 libvorbis 的 ffmpeg」上该路径均失败 ⇒ 是**代码硬编码可选依赖**，非环境问题。修复后又暴露两个前置条件：内建 `vorbis` 需显式 `Compliance::Experimental`（CLI `-strict -2`），且仅支持 `fltp`（planar）而非代码固定传入的 `flt`（packed） | 改为运行时探活 `libvorbis` → 内建 `vorbis` 回退；`formats()` 自适应采样格式；内建 `vorbis`/`opus` 自动 opt-in。产出 Ogg 字节经 magic 校验为真实容器（`4f 67 67 53` = `OggS`）。**`--all-features` 1853/1 failed → 1858 passed / 0 failed**；`full` 3913 → **3918 passed / 0 failed**；新增 4 个回归测试 |

> **本轮同时闭环 A 类 #4/#5**：新增 `examples/apple_appkit_probe.rs`（主线程 AppKit 探针）、
> `tools/build_ios_testapp.sh` + `tools/run_ios_testapp.sh`（无 Xcode 工程的 iOS 模拟器 E2E）、
> `tools/check_apple_native.sh`（统一门禁）与 `.github/workflows/ci.yml` 的 `apple-native` 作业（macOS runner）。
> Apple 状态文档：`src/platform/macos/status.md`、`src/platform/ios/status.md`。

### 第 9 轮补审（同日续作）— 探针加强后暴露的 2 个「静默无操作」缺陷

> 首轮探针只断言了「对象已创建」与「state 往返」，**不足以区分「真实 FFI 接线」与「state-only 桩」**。
> 补上「变更是否真到达原生对象」的断言后，又暴露 2 个真实缺陷（均属 F-3 同类）。

| # | 项目 | 卡点类型 | 修复前的事实（可验证） | 闭环证据 |
|---|---|---|---|---|
| F-5 | **objc2 `set_native_frame`/`set_native_hidden` 把视图 selector 发给 NSWindow** | 本机可做（真实缺陷） | `set_native_frame` 无条件 `setFrame:`，但 `NSWindow` **不响应** `setFrame:`（窗口用 `setFrame:display:`）⇒ objc2 报 `invalid message send to -[NSWindow setFrame:]: method not found`，**进程中止**（worker 线程）或静默失败。`set_native_hidden` 同理（窗口用 `orderOut:`/`makeKeyAndOrderFront:`，非 `setHidden:`）。修复前探针不覆盖「窗口几何」，故不可见 | 四个 setter 均加主线程守卫 + 按 `isKindOfClass:NSWindow` 分派窗口/视图路径；`set_native_enabled`/`set_native_text` 用 `respondsToSelector:` 守卫。新增探针断言 `native_window_frame_applied`（`NSWindow.frame` 真变为 400×300）与 `native_window_visibility`（`isVisible` 随 hide/show 变化），两个后端均 PASS |
| F-6 | **iOS `set_native_text` 对 `UIButton` 是静默无操作** | 本机可做（真实缺陷，F-3 孪生） | 原循环用 `respondsToSelector:` 探 `setTitle:forState:`——但该 selector 是**双参数**，单参数探针永远返回 false ⇒ 落到同义可响应的 `setAccessibilityLabel:`，**可见的按钮标题从未更新**。`rw_set_widget_text` 只改了 Rust state。实测：`rw_set_widget_text(button, "Tapped")` 后原生 `UIButton.titleForState` 仍为 `"Tap"` | 改为类型化分派（`setTitle:forState:` → `setText:` → `setAccessibilityLabel:`）；探针新增 `native_text_applied` 断言原生 `UIButton` 标题，实测 `UIButton title = Tapped`（修复前 FAIL） |

> 方法论教训（规则 #12/#19）：**「状态往返」不等于「原生写入」**。
> 首轮探针若只停在 `rw_get_widget_text` 的 state 往返，F-6 会被当成已完成。
> 因此探针现已对窗口/控件的**原生属性**（`NSWindow.frame`/`isVisible`、`UIButton.title`）直接断言。

---

## 三之五、G 类 — 第 9 轮换方向：FFI 健全性审计（2026-09-11）

前八轮都在查「功能有没有接线」。本轮换一个从未系统审过的维度：**`unsafe` 边界的正确性**
（平台 FFI 最容易「测试全绿但运行时崩」）。方法为工具 + 可复现，不采信印象。

### 基线与排除项

| 检查 | 结果 |
|---|---|
| `unsafe` 块总数 | 238 |
| `unsafe_op_in_unsafe_fn`（Rust 2024 默认 lint）告警 | **0**（未在 `unsafe fn` 里裸做 unsafe 操作） |
| `unsafe impl Send/Sync` | 9 处 |

### G-1 `ime_macos` 的 `ImeCtx` 被写成四个不同的局部类型 ⇒ 3 条原生路径静默失效

**卡点类型**：本机可做（真实缺陷）

`ImeCtx` 在四个不同函数作用域内各声明一次（`try_activate_nstextinputcontext`、
`sync_nstextinputcontext`、`focus_in`、`focus_out`）。同名、同布局，
但 `Any::downcast_ref` 按 **`TypeId`** 匹配，**不看内存布局**。
故 `token.downcast_ref::<ImeCtx>()` **永远返回 `None`**，原注释
「the repr(C) layout guarantees downcast_ref works」为错误认知。

**影响**（与 F-6 同类「假接线」）：

| 位置 | 应当调用 | 修复前实际 |
|---|---|---|
| `sync_nstextinputcontext` | `invalidateCharacterCoordinates` | 永远 early-return |
| `focus_in` | `activate` | 永远不执行 |
| `focus_out` | `deactivate` | 永远不执行 |

**闭环证据**：提升为模块级唯一定义 + `ime_ctx_from_token()`；新增 2 个回归测试。
**负向验证**：改回旧写法后 `ime_token_downcasts_back_to_ime_ctx` **如期 FAILED**，恢复后全绿。

### G-2 Apple 的 `unsafe impl Sync for NativePtr` 是多余的不安全承诺

**卡点类型**：本机可做（可验证的冗余 unsafe）

`macos_objc2/native.rs` 与 `ios/native.rs` 的 `NativePtr` 同时 `unsafe impl Send + Sync`。
实测（逐个注释后重编）：

| 组合 | 编译 |
|---|---|
| Send + Sync（原样） | ✅ |
| 仅 Send（去 Sync） | ✅ **仍通过** |
| 去 Send | ❌ `E0277: *mut c_void cannot be sent between threads safely` |

即 `Send` 必需（static `Mutex<HashMap>`），**`Sync` 完全多余**。
而 `Sync` 恰好放宽「`&NativePtr` 可跨线程共享」——正是 F-1/F-5 能发生的结构性前提。
**已移除 2 处 `Sync`**（保留 `Send`）。

> **同类待处理（需对应平台验证，本轮未动）**：`ime_windows.rs` 的 `TsfThreadMgr`、
> `android/types.rs`、`linux/types.rs`、`json/events.rs`、`widget/registry.rs`、
> `memory/allocators.rs`、`video/ffmpeg_decoder.rs`。
> 这些与 Apple 无关，且部分需要真实设备/系统验证，不伪装为已闭环。

### 冰山扫描
### 冰山之扇（同类模式）

全项目 `downcast_ref` 扫描：其余全部使用**模块级类型**，无同类问题。
函数内 `struct` 仅剩 `ios/native.rs` 的 `ButtonTarget`（`define_class!` 宏类型，
走 ObjC 运行时而非 `Any::downcast`），不适用。

---

## 三之六、I 类 — 第 9 轮换方向之二：错误路径与资源泄漏审计（2026-09-11）

与 §三之五 同属本轮换方向审计，主题为**错误处理路径的资源释放**。

### I-1 `ffmpeg_encoder` 编码失败时泄漏临时文件

**卡点类型**：本机可做（真实资源泄漏）

`ffmpeg_next::format::output_as` 会**立即在磁盘上创建文件**，而该函数后续有大量 `?` 提前返回
（resampler 构造、编码器打开、帧发送、包写入……）。原实现只在成功路径末尾调用 `remove_file`
⇒ **任何失败都泄漏文件**。

**实测复现**（修复前）：

```
before: []
encode result: Some("Failed to create resampler: Invalid argument")
after : ["rust_widgets_audio_enc_80794_0.mp3"]
LEAKED_COUNT=1
```

**修复**：新增 `TempFileGuard`（RAII），`Drop` 时删除文件，覆盖所有退出路径（含 `?` 与 panic 展开）。

**验证**（同一探针，修复后）：

```
after : []
LEAKED_COUNT=0
```

**新增回归测试**（`ffmpeg_encoder::tests`）：

- `temp_file_guard_removes_file_on_drop` —— 确定性验证 guard 契约（无共享计数器、无并发）；
- `test_failed_encode_leaves_no_temp_file` —— 失败路径不泄漏；
- `test_successful_encode_leaves_no_temp_file` —— 成功路径也不泄漏。

### I-2 `ffmpeg_decoder` 写入失败时泄漏临时文件（同一缺陷类）

`FfmpegDecoder::new` 先 `File::create`，再 `write_all`/`flush`；这两步的 `?` 会在文件已创建后
提前返回。原清理逻辑只覆盖 `from_path` 失败（且注释声称「If construction failed, clean up」，
实际漏了写入失败）⇒ 写入失败时**文件不会被删**。

**修复**：同样用 RAII guard；成功时 `disarm()`（所有权交给解码器的 `_temp_path`，其 `Drop` 负责清理）。
新增 `test_failed_decoder_leaves_no_temp_file` 与 `test_leaves_no_temp_file`。

### I-3 测试隔离问题（过程中发现并修正）

为验证 I-1/I-2 写的「临时目录前后快照」断言在 `--all-features` 下**间歇失败**：
同一测试二进制内多个模块（audio/video）并发编码，共享 `TEMP_COUNTER` 与系统 temp 目录，
互相把对方的 in-flight 文件当成泄漏。

处置（不掩盖、不跳过）：

1. 将模组内所有会调用编码的测试串行化（`ENCODE_TEST_LOCK` / `DECODER_TEST_LOCK`）；
2. 断言改为**确定性契约**（guard 的 `Drop` 行为）+ 对具体路径的检查（不对全局计数做断言）；
3. 新增 `temp_path_in(dir, ext, count)` seam，使路径可推导且不依赖并发时机。

连续 10 次 `--all-features` 全绿（修复前为间歇失败）。

### 验证

| 检查 | 结果 |
|---|---|
| `ffmpeg_encoder` 测试 | 9 passed |
| `video::ffmpeg_decoder` 测试 | 9 passed |
| `cargo test --lib --features desktop` | 3836 passed / 0 failed |
| `cargo test --lib --no-default-features --features "macos,serde,serde_json"` | 2257 passed / 0 failed |
| `cargo test --lib --all-features`（连续 10 次） | 1865 passed / 0 failed，**无抖动** |
| `cargo test --lib --features full` | 3925 passed / 0 failed |
| clippy（desktop+video-codecs / all-features） | 0 warnings |
| `tools/check_profiles.sh` | PASS |

---

## 三之七、J 类 — 第 9 轮换方向之三：并发安全 / panic 安全 / i18n 时序抖动（2026-09-11）

### J-1 i18n 热重载仅依赖 mtime，粗粒度文件系统会静默漏检

**卡点类型**：本机可做（真实健壮性缺陷）

**先取证**：本机（macOS/APFS）mtime 为纳秒级，连续写入可区分（实测写入间隔 36µs 仍 `mtime2 > mtime1`），
故上轮 `tablet` 的间歇失败**不是**本机精度问题。

**但代码确实有同类缺陷**：`check_and_reload` 仅靠 `modified > last_modified` 判定。
在 mtime 粒度粗的文件系统（ext3 / 部分网络挂载 / 某些容器 FS）上，同一 tick 内的两次写入
mtime 相同 ⇒ **变更被静默漏检**，热重载不触发。
**佐证**：测试里原本有一处 `sleep(100ms)`，注释直言
「Give the file system time to register a different modification time」——正是为规避该缺陷而加的补丁。

**修复**：改为 `FileFingerprint { modified, len, hash }`，按 mtime → 长度 → 内容哈希（FNV-1a）
依次比较，不依赖文件系统时钟推进。同时删除两处多余 `sleep(100ms)`（不参与任何同步）。

**闭环证据**：新增 `test_i18n_reload_detects_change_with_identical_mtime`（写入等长不同内容并把 mtime
回拨到加载时的值，使 mtime 与长度均不变，仍应检测到变更）。**负向验证**：改回 mtime-only 后该测试如期 FAILED。
i18n 测试耗时 **0.11s → 0.01s**。

### J-2 `undo/stack` 测试夹具的 `static mut` 数据竞争

**卡点类型**：本机可做（真实 UB）

`src/undo/stack.rs` 的测试夹具用 `static mut NEXT_ID: u64` 在 `unsafe` 块内 `+= 1`。
Rust 测试**多线程并行**，这是真实数据竞争（且 `static mut` 在 Rust 2024 为硬错误）。

**修复**：改为 `AtomicU64` + `fetch_add`，消除 `unsafe`。

### J-3 双向绑定的 `syncing` 标志会被 panic 永久卡住

**卡点类型**：本机可做（真实 panic 安全缺陷）

`TwoWayListener::on_value_changed` 先 `syncing.swap(true)` 防重入，函数末尾 `store(false)` 复位。
中间任何 panic 都会**跳过复位** ⇒ `syncing` 永久为 `true` ⇒ 该双向绑定**从此静默失效**。

**修复**：改为 RAII guard（`SyncingGuard`），`Drop` 复位，覆盖包括 panic 在内的所有退出路径。

**闭环证据**：新增 3 个测试（含真实 unwinding 路径）。
**负向验证**：禁用 guard 后，两个 guard 测试**如期 FAILED**
（失败信息："the guard must clear `syncing` while unwinding, otherwise the binding is dead forever"）。

### 已核验无缺陷的方向（取证后排除，不伪造工作）

| 检查 | 结果 |
|---|---|
| `catch_unwind` 在 C ABI 边界 | 105 个 `extern "C" fn` 的实际逻辑均经 `c_try!`/`c_try_void!`；`rw_free_rust_string` 为转调，继承保护——**无缺口** |
| `rw_free_string` 指针契约 | null 有保护，`CString::from_raw` 立即 drop，契约已文档化——**健全** |
| 锁顺序反转 | `TwoWayListener` 已明确「先释放 source 再锁 target」并注释说明；其余多锁获取绝大多数在测试代码内 |
| 全局可变 static | 除已修的 `static mut` 外均为 `OnceLock`/`LazyLock`/`Mutex`/`Atomic*` |

### 已知限制（登记，非缺陷）

`panic = "abort"` 出现在 `release-embedded` 与 `release-mini` profile。
**后果**：这两个 profile 下 `catch_unwind` 不生效，C ABI 的 panic 保护退化为进程 abort。
属嵌入式设计取舍；使用这两个 profile 的调用方应知悉。

---

## 三之八、K 类 — 第 9 轮换方向之四：内存增长 / 长期运行（2026-09-11）

### K-1 平台层没有 widget 销毁 API（真实架构缺口，已闭环）

**卡点类型**：本机可做（真实缺口）

**取证**：`Platform` trait 共 95 个方法，`grep` 确认**无任何销毁入口**
（只有 `list_box_remove_item`，那是列表项而非 widget）。所有后端注册表（`NATIVE_VIEWS`、`handles`、
`list_data`、`menus`……）**只增不减**；布局层的 `remove_widget` 只改布局内部向量，不触达平台层。
⇒ 动态 UI（创建/丢弃循环）会**无界积累注册表条目**。

**修复（全后端）**：

| 层 | 变更 |
|---|---|
| `BackendState` | 新增 `destroy_widget()` / `widget_count()` |
| `Platform` trait | 新增 `destroy_widget()`，**默认实现返回 `false`**（向前兼容，规则 #21） |
| `ControlBackend` trait | 新增并转发（native / custom 后端） |
| 11 个后端 | 均实现**真实清理**（非表面） |
| C ABI | 新增 `rw_destroy_widget`（ABI 门禁通过，头文件重生成 106 个声明） |

**清理深度（非表面清理）**：

- objc2：释放 `Menu` 的**派生 submenu id**（否则 `NSMenu` 泄漏）；`Menu` 销毁**级联**其子项；
 清理 `attached_menu_bar` / `menu_children` / `menu_item_shortcuts` / 待处理事件队列
- cocoa：额外调用既有的 `a11y_bridge.unregister_handle`（**此前从未被调用**，每 widget 泄漏一个 a11y 条目）
- iOS：按钮额外释放保留的 `ButtonTarget`
- Windows：`control_command_to_widget` 按**值**扫描（键是命令 id 而非 widget id）
- 全部实现：每把 `Mutex` 守卫限定在自身语句/块内，**不同时持两把锁**

**闭环证据**：新增 `platform::teardown_tests`（3 个测试，断言**库自身注册表**而非 RSS）。
**负向验证**：禁用 state 层的真实删除后，3 个测试**全部 FAILED**（报错精确）。

### K-2 ⚠️ 纠正一个错误结论：RSS 增长不是本库泄漏

初稿曾以「创建 8000 控件 RSS +49MB」为由声称内存泄漏。**该结论错误**，用三个独立方法推翻：

1. **macOS 官方 `leaks`**：`0 leaks for 0 total leaked bytes`；两次运行的 malloc 节点数
 748391 / 748382，**不随 churn 增长**。
2. **ObjC 视图树直测**：销毁前 `contentView subviews = 1`，销毁后 `= 0`（对象确实释放）。
3. **纯 AppKit 对照**（不含本库任何代码）：RSS 曲线与本库探针**几乎重合**
 （第 6 轮：纯 AppKit 108704 KB vs 本库 109376 KB）。

⇒ RSS 增长 100% 来自 **AppKit 自身**（`NSButton` 创建触发类/图形缓存与内存池）。

> **方法论教训**：**RSS 不是衡量库内存行为的有效指标**（受分配器、运行时缓存、页对齐干扰）。
> 正确指标是 `leaks` 的节点计数与库自身注册表尺寸。本轮将回归测试改为断言**注册表状态**。
> 另：这也说明「用 RSS 涨了」就断言泄漏是**未完成求证**的归因（与 F-7 同类错误）。

### K-3 保留的 `removeFromSuperview`（有独立依据，非钼制为修泄漏）

查错过程中 `remove_native_view` 新增了 `removeFromSuperview`。它的保留理由是**语义正确性**，
而非修一个不存在的泄漏：不加它时，被销毁的控件会**留在父视图树中**
（仍会被 AppKit 绘制与命中测试）——即「逻辑上已销毁但视觉/交互仍存在」的不一致。
注释已按真实理由重写。

### 验证

| 检查 | 结果 |
|---|---|
| 新增 `platform::teardown_tests` | 3 passed |
| 负向验证（禁用 state 真实删除） | 3 个测试全部 FAILED |
| `cargo test --lib --features desktop` | 3854 passed / 0 failed |
| `cargo test --lib --all-features` | 1869 passed / 0 failed |
| `cargo test --lib --features full` | 3946 passed / 0 failed |
| `cargo test --lib --no-default-features --features "macos,serde,serde_json"` | 2275 passed / 0 failed |
| 其他 profile（mobile / tablet / wasm / harmony） | 3662 / 3653 / 2263 / 2268，全 0 failed |
| `tools/check_widget_kind_count.sh`（新增：文档计数一致性） | PASS（167 variants） |
| clippy（`--all-features --all-targets -D warnings`） | 0 warnings |
| `cargo check --all-targets`（desktop / full / mini / embedded） | 全 PASS |
| `tools/check_abi.sh` | PASS（106 个声明） |
| `tools/check_profiles.sh` / capability / route matrix | 全 PASS |



## 三之九、M 类 — 第 9 轮换方向之五：跨平台 API 契约一致性（2026-09-11）

### M-0 方法：静态扫描会骗人，改用行为测试

本轮先尝试用正则统计各后端 `create_*` 中 `parent` 的校验情况。**该静态方法给出错误结论**：
它统计出「linux 校验数 = 0」「mobile 校验数 = 0」，而实际情况是 Linux 的校验位于
`src/platform/linux/widget_creation.rs`（44 处）、mobile 位于各自 `platform_impl.rs`。
⇒ 本轮全部结论改由**行为测试**产生：直接调用各后端 `create_*` 并断言其返回 `false`。
这也解释了为何 M-1/M-2 长期不可见——没有任何测试检查过这条契约。

### M-1 `StubPlatform` 内部自相矛盾（真实缺陷，已闭环）

**卡点类型**：本机可做（真实契约不一致）

**取证**：`src/platform/stub.rs` 中 **21 个** `create_*` 把参数写作 `_parent` 并**直接忽略**，
而另外 **19 个**同名同形的方法会校验父级句柄是否存在、类型是否匹配。
同一个 trait 的同一族方法对「非法父级」给出两种相反行为 ⇒ 使用 `StubPlatform` 的测试
无法作为其它后端的契约基准，且掩盖了真实的父级校验缺失。

**修复**：21 处全部补齐父级校验（`parent` 语义一致化）。两个特例按真实语义处理：
`create_menu_bar` 要求父级是 `Window`；`create_menu` 要求 `MenuBar | Menu`。

### M-2 同类模式：`macos`（cocoa）的 3 个控件（真实缺陷，已闭环）

**取证**：`src/platform/macos/platform_impl.rs` 中 `create_spin_box` / `create_list_view` /
`create_scroll_area` 忽略父级，而**紧邻其下的** `create_group_box` 却正确校验——
说明这是遗漏而非设计。

**修复**：三者补齐与 `create_group_box` 同形态的父级校验。

### M-3 ◉ 有意不修：dialog 不校验父级是**多数派约定**

`create_message_box` / `create_file_dialog` / `create_color_dialog` / `create_font_dialog`
在 **7 个后端忽略**父级、**4 个后端校验**父级。经分析，**忽略是多数派且语义正确**：
对话框是**顶层模态窗口**，不属于父级控件的子视图树，父级句柄仅作为「归属提示」。
若强行统一为「必须校验」，会破坏 7 个后端的正确行为。

**处理**：登记为**有意不修**（规则 #22：区分「真实缺口」与「平台事实」），
并在 `src/platform/contract_tests.rs` 中用测试**固定该契约**，避免后续轮次误判为缺口。

### M-4 闭环证据

新增 `src/platform/contract_tests.rs`（**11 个测试**）：

- 6 个契约点测试（`assert_contract`）分别对 Stub + 选定后端 + feature 门控后端执行；
- 1 个 **17 方法子控件父级拒绝矩阵**（逐一调用并断言拒绝非法父级）。

**负向验证（双向）**：

| 操作 | 预期 | 实测 |
|---|---|---|
| 还原 stub 的父级校验 | 测试 FAILED | ✅ FAILED |
| 还原 cocoa `create_spin_box` 校验 | 测试 FAILED | ✅ FAILED，报错精确：`cocoa: spin_box must reject an unknown parent` |

---

## 三之十、N 类 — 第 9 轮换方向之六：文档与代码的机械一致性（2026-09-11）

### N-1 真实脱节：`codemap.md` 声称 166 个变体，实际 167

**卡点类型**：本机可做（文档欺骗，规则 #18）

**取证**：`docs/plans/codemap.md` **两处**写 `166 variants`；而项目自身的路由矩阵门禁
`check_control_route_matrix` 打印 `Total WidgetKind variants: 167`。
⇒ 文档与**机械事实**不符（不是与人的记忆不符）。

**修复**：两处改为 167。

### N-2 README 严重低估控件数

`README.md` 写 `80+ widgets`，`README.zh-CN.md` 写 `80+ 控件`；实际 `WidgetKind` 变体数为 **167**。
两个 README 均已按实际数量更正（`167 widget kinds` / `167 种控件`）。

> 注：`80+` 在数量上「没有说谎」（167 ≥ 80），但它会让读者对库的规模产生**数量级误判**，
> 且与其它文档的精确计数不一致，故按机械一致性要求一并修正。

### N-3 新增防脱节门禁 `tools/check_widget_kind_count.sh`

**动机**：N-1/N-2 这类脱节**会随代码演进而复发**（每次新增控件都需人工同步 4 个文档）。
仅修数字是「治标」；按冰山法则必须加**机械门禁**。

**实现**：机械解析 `src/widget/kind.rs` 得到变体数（剥离注释与 `#[cfg]`），
校验以下文档中的计数声明与之一致：

| 文件 | 校验的模式 |
|---|---|
| `docs/plans/codemap.md` | `<n> variants` |
| `docs/plans/platform_capability_matrix.md` | `<n> variants` |
| `README.md` | `<n> widget kinds` |
| `README.zh-CN.md` | `<n> 种控件` |

已接入 `.github/workflows/ci.yml` 的 `validation-gates` 作业。
**负向验证**：写入 `999 widget kinds` → 门禁如期 FAILED。

### 已核验一致（不伪造工作，规则 #20）

`README` 声称的「11 个手势识别器」经机械统计确为 **11 个** `*Gesture` 结构体，**一致，未改动**。

### 验证

| 检查 | 结果 |
|---|---|
| `tools/check_widget_kind_count.sh` | PASS（167 variants） |
| 负向验证（写入 999 widget kinds） | ✅ FAILED（门禁有效） |
| `cargo test --lib --features desktop` | 3854 passed / 0 failed（含新增 11 个 contract 测试） |
| `cargo test --lib --all-features` | 1869 passed / 0 failed |
| `cargo test --lib --features full` | 3946 passed / 0 failed |
| `cargo test --lib --no-default-features --features "macos,serde,serde_json"` | 2275 passed / 0 failed |
| 其他 profile（mobile / tablet / wasm / harmony） | 3662 / 3653 / 2263 / 2268，全 0 failed |
| clippy（`--all-features -D warnings`） | 0 warnings |
| `cargo fmt --check` | PASS |
| 8 个门禁（profiles / abi / widget_kind_count / capability_matrix / capability_truthfulness / route_matrix / impl_matrix / apple_thread_safety） | 全 PASS |

---

## 三之十一、O 类 — 第 9 轮收尾：全量验证暴露的自引入回归（2026-09-11）

### O-0 为什么此前没发现：验证命令缺了 `--all-targets`

M/N 两轮的验证用的是 `cargo check --features desktop` 与 `cargo clippy --all-features`，
**均不带 `--all-targets`**。不带它的 `clippy`/`check` **根本不看 `examples/` 与 `benches/`**，
而本轮新增的两个 Apple 探针恰好就在 `examples/`。补上 `--all-targets` 后三个问题全部暴露。

### O-1 `full` profile 下两个 Apple 探针编译失败（真实回归，已闭环）

**取证**：

```
error[E0432]: unresolved import `rust_widgets::platform::get_platform`
 --> examples/apple_appkit_probe.rs:46:34
error[E0432]: unresolved import `rust_widgets::platform::get_platform`
 --> examples/apple_appkit_probe_objc2.rs:36:34
note: found an item that was configured out --> src/platform/mod.rs:108
```

**根因**：`Cargo.toml` 中 `full = ["desktop", …, "mini", …]` —— **`full` 同时启用 `desktop` 与 `mini`**，
而 `get_platform` 的导出条件是 `#[cfg(not(feature = "mini"))]`。两个探针只门控了
「macOS ∧ 对应后端 feature」，**未排除 `mini`** ⇒ 在 `full` 下探针主体进入编译但 `get_platform` 不存在。

**修复**：门控补 `not(feature = "mini")`（`main`/`run` 各 2 处），并保留显式「skipped」分支。

**闭环证据**：`cargo check --no-default-features --features {full,mini,desktop,embedded} --all-targets` → **全 Finished**（原 `full` FAILED）。

> 这是**第二次**踩到同一类陷阱（规矩 #5：`examples/` 会被每个 profile 的 `--all-targets` 编译）。
> 第一次是探针未门控 `cocoa-legacy`，这次是未门控 `mini`。

### O-2 死代码 `encode_temp_files()`（真实回归，已闭环）

**取证**：`grep -n "encode_temp_files" src/audio/ffmpeg_encoder.rs` **仅 1 处命中**（只有定义）；
`clippy --all-targets -D warnings` 报 `function is never used`。

**来历**：这是 J-3 测试隔离问题的**废弃方案残留**——最初的泄漏测试用「前后对比 temp 目录文件计数」，
该函数是其实现；后发现与并发测试**必然竞争**，改为「记录计数器 → 断言**精确路径**」，辅助函数忘了删。

**修复**：删除该死函数（违反规则 #4，且是误导性残留）。
**闭环证据**：clippy **0 warnings**；`cargo test --lib --all-features` 仍 **1869 passed / 0 failed**（未丢测试）。

### O-3 ⚠️ 自我纠正：四个 profile 的测试计数文档记录错误

K 轮的验证表曾记录 mobile / tablet / wasm / harmony = `3860 / 3854 / 3864 / 3869`。
实测（两次运行结果稳定）：

| profile | 文档原记 | 实测 | 差异 |
|---|---|---|---|
| mobile | 3860 | **3662** | -198 |
| tablet | 3854 | **3653** | -201 |
| wasm | 3864 | **2263** | -1601 |
| harmony | 3869 | **2268** | -1601 |

**归因**：wasm / harmony 的偏差量完全一致（-1601），说明它们缺了同一块不参与构建的测试模块
（这两个 profile 不启用 `desktop-runtime` 等，模块级 `#[cfg]` 使测试不计入）；
desktop / full 的计数（3854 / 3946）与文档**逐字相符**，说明偏差**局限于这四个 profile**，非全表失效。

> **性质**：这是**记录错误**，不是代码缺陷（无测试丢失、无失败）。但错误的数字比没有数字更糟，
> 因为它会被后续轮次当作基线。已按规则 #18 更正两处计划文档与本日志。

### O-4 全量验证（含 `--all-targets`）

| 检查 | 结果 |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo clippy --all-features --all-targets -- -D warnings` | **0 warnings** |
| `cargo test --lib --features desktop` | 3854 passed / 0 failed |
| `cargo test --lib --all-features` | 1869 passed / 0 failed |
| `cargo test --lib --no-default-features --features full` | 3946 passed / 0 failed |
| `cargo test --lib --no-default-features --features "macos,serde,serde_json"` | 2275 passed / 0 failed |
| `cargo test --lib`（mobile / tablet / wasm / harmony） | 3662 / 3653 / 2263 / 2268，全 0 failed |
| `cargo check --all-targets`（desktop / full / mini / embedded） | 全 Finished |
| 8 道门禁 | 全 PASS |

**新增规则（已并入本项目规矩 #37 的延伸）**：
**验证必须带 `--all-targets`。** 新增/修改任何 `examples/*.rs` 后，必须跑
`cargo check --all-targets` 至少覆盖 `desktop` 与 `full` 两个 profile。

---

### H-2 部分 · 已于第 9 轮收尾（§三之十二 P 类）**全部判定完毕**

原记「其余待平台验证」**不准确**：残留的 3 个 `unsafe impl Sync` 中，
`LinuxPlatform` / `TsfThreadMgr` / `EventHandlerContext` 均可**在本机用删除-编译法判定**，
且删除后均为 **0 error** ⇒ 均为多余不安全承诺，已删。
唯一必要的 `AndroidPlatform`（删除后 55 error）保留。**H-2 至此完全闭环。**

## 三之十二、P 类 — H-2 闭环：逐个验证 `unsafe impl Sync` 是否真的必要（2026-09-11）

### P-0 方法：删除-编译法（不是读代码猜）

对每个残留的 `unsafe impl Sync`，**注释掉该行后编译**：编译失败 ⇒ 必要（保留）；
编译通过 ⇒ **多余的不安全承诺（真实缺陷，删除）**。这比读注释判断可靠——注释会过期，编译器不会。

### P-1 验证结果矩阵

| # | 类型 | 文件 | 删除后编译 | 结论 |
|---|---|---|---|---|
| 1 | `EventHandlerContext` | `src/json/events.rs` | **0 errors** | 多余（已删） |
| 2 | `LinuxPlatform` | `src/platform/linux/types.rs` | **0 errors**（desktop / full / all-features 均通过） | 多余（已删） |
| 3 | `TsfThreadMgr` | `src/platform/ime_windows.rs` | **0 errors**（含 `x86_64-pc-windows-gnu` 交叉编译） | 多余（已删） |
| 4 | `AndroidPlatform` | `src/platform/android/types.rs` | **55 errors** | 必要（保留） |
| 5 | `NativePtr`（ios / macos_objc2） | — | 本就**只有 `Send`** | 早已正确 |

### P-2 为什么危险（不是纯洁癖）

`unsafe impl Sync` 的语义是「**允许 `&T` 跨线程共享**」：

- `EventHandlerContext::user_data<T>() -> Option<&T>` 与 `user_data_mut<T>() -> Option<&mut T>`
 从**同一无主指针**返回 `&T` / `&mut T`。若 `&EventHandlerContext` 可跨线程共享，
 两线程即可分别拿到指向同一对象的 `&T` 与 `&mut T` ⇒ **数据竞争（UB）**。
- `LinuxPlatform` 内含 `gtk::*`（`!Sync`），GTK 要求所有调用发生在调用 `gtk::init` 的线程上。
- `TsfThreadMgr` 持有 TSF COM 对象，COM 是**单元线程**模型。

⇒ 这三个 impl **不提供任何收益**，只扩大不安全面。删除是**净安全收益**。

### P-3 `AndroidPlatform` 为何保留

删除后 **55 个 E0277**（`*mut c_void cannot be shared between threads safely`），
证明该 `Sync` 被**真实代码路径需要**（`jvm` 字段使类型含裸指针，且平台实例需跨线程共享）。
按规则 #22（区分真实需求与虚假承诺）保留，并在注释中保留原有 SAFETY 依据。

### P-4 闭环证据

| 检查 | 结果 |
|---|---|
| `cargo check --all-targets`（desktop / all-features / full / mini / embedded / mobile / tablet / wasm / harmony） | 全 0 errors |
| `cargo check --target x86_64-pc-windows-gnu --lib` | 0 errors |
| `cargo clippy --all-features --all-targets -- -D warnings` | **0 warnings** |
| `cargo test --lib`（desktop / all-features / full） | 3854 / 1869 / 3946，全 0 failed |
| 8 道门禁 | 全 PASS |

> **注**：过程中自身出错 2 次（编造不存在的类型名；一次编辑误删结构体声明行致结构断裂），
> 均已立即 `git checkout` 还原并重做。教训：**编辑后必须读回改动处**（符号配对完整性是项目强制规矩）。

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
| **iOS（第 9 轮，2026-09-11）** | 新增无 Xcode 工程的 `.app` 构建与模拟器 E2E：`bindings/ios/main.m` + `Info.plist`、`tools/build_ios_testapp.sh`、`tools/run_ios_testapp.sh`；在 **iOS 26.2 模拟器**上 **9/9** 断言 `RESULT: PASS`（真实 `UIWindow`/`UIButton`/`UILabel`/`UITextField`/文本与几何往返，含 F-6 的原生标题写入）；修复 5 处 iOS 专属 clippy 告警 + 1 处未用 import；新增 `src/platform/ios/status.md` |
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
| `cargo test --lib --all-features`（CI 实际命令） | **1858 passed / 0 failed**（修复前 E0428 编译失败；修 F-4 后为 1853/1 failed） |
| `cargo test --lib --features full` | **3918 passed / 0 failed** |
| Apple AppKit 探针 — cocoa-legacy（`--features desktop`） | **RESULT: PASS**（真实 `NSWindow`/`NSMenu`/`NSPasteboard`/dialogs） |
| Apple AppKit 探针 — objc2（`--features macos`） | **RESULT: PASS**（真实 `NSWindow` 入 `windows`、真实 `mainMenu`） |
| iOS 模拟器探针（iOS 26.2 / arm64） | **8/8 PASS**（`RESULT: PASS`） |
| `platform::ios::*` — 目标编译（device / simulator × state / FFI） | 全 **0 error / 0 warning** |
| `cargo clippy`（desktop / macos / iOS 目标，`-D warnings`） | 均 **0 warnings** |
| `cargo fmt --check` | 通过 |
| 其他 profile（mobile / tablet / mini / wasm / harmony） | 3841 / 3836 / 1766 / 3845 / 3850，**全 0 failed** |
| `tools/check_profiles.sh`（官方 8 步 profile 门禁） | **`All profile checks passed.`**（含 `embedded`） |

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
