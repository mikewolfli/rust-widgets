<!-- SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com) -->
<!-- SPDX-License-Identifier: MIT -->

# OS 真机 / 真实运行时验证台账

> **用途**：把「某个操作系统 / 后端到底在**真实运行时**上被验证到什么程度」集中成一张可查的账。
> 上游反馈（如 `docs/issues/feedback-*.md`）与下游 re-pin 都依赖这张账：**它只写跑过的，不写推断的**。
>
> **记账口径**见 [`README.md`](README.md) 的「记账约定」一节：真机结论必须有命令 + 原始输出；
> 仅编译/链接必须显式写出；模拟器与真机分开记；过期结论写「需复跑」。
>
> **本文件时间基线**：`HEAD = bc51a9e1`（2026-10-15）。每个结论各自标注它自己的日期/修订，
> 「已确认」只对该行所写的日期/宿主成立。

## 图例

| 标记 | 含义 |
|---|---|
| ✅ **已确认** | 在**真机**或该平台的**原生运行时**上实跑，且有命令 + 原始输出 |
| 🔶 **仅编译/链接** | 只做了 `cargo check` / cross-compile / `build`+`link`，**未运行** |
| ⚠️ **已过期** | 曾实跑，但在更早修订上；当前 `HEAD` 未复跑 |
| ❓ **记录冲突** | 仓库内有不一致记录（来源已列出），未单方面采信 |
| ❌ **未验证** | 没有任何运行时证据（真机/模拟器都没有） |

## 一览（按宿主可及性排序）

| OS / 后端 | 真实运行时 | 结论 | 关键日期 / 修订 | 证据（命令 / 结果） |
|---|---|---|---|---|
| **Linux / X11 + GTK3** | 真 X11 桌面会话（本机 `DISPLAY=:0`，GTK 3.24.41） | ✅ **已确认** | 2026-10-14/15，`bc51a9e1` | `probe_window_on_screen`（X 服务器报 `IsViewable`、480x320、62 色）；`probe_file_picker`（真对话框 + XTEST 真实键入 → `Ok(Some("/tmp/rw_picked_by_probe.txt"))`）；`probe_cross_thread_quit`（`run()` 415.75ms 返回）；`GDK_SCALE=2` → 库报 2。见 `docs/log/log-20261014-1.md` 第 4 轮 §4/§7 |
| **Windows（Win32）** | 真 Windows 10.0.22621（MINGW64/MSVC，rustc 1.95.0） | ✅ **已确认（旧修订，需复跑）** | 2026-09-15 | `cargo test --no-default-features --features desktop --lib -q` → `4026 passed; 0 failed`；example + integration test 同机跑过。见 `docs/log/archive/log-20260915-1.md` |
| **macOS / `cocoa`** | 真 Mac（macOS 15.7.3，Apple M4 / arm64） | ✅ **已确认** | 2026-09-11 与 2026-10-14（`c957fdec`，2.8.6） | AppKit 探针 12 项 PASS（2026-09-11）；契约探针 `backend_name()=cocoa`、`NativeInteractive`、真的进 `[NSApplication run]`（2026-10-14）。见 `docs/log/archive/log-20260911-2.md`、`docs/issues/feedback-20261013.md` |
| **macOS / `macos-objc2-preview`** | 真 Mac | ⚠️ **部分（原生探针已删；post-BLUE15 仅剪贴板测试）** | 2026-09-11（原生，已过期）/ 2026-10-14 | `NSPasteboard` 往返：`cargo test … platform::macos_objc2` → 10 passed（`docs/log/log-20261014-1.md` 第 3 轮 §5）；`run()` 仍是轮询 loop（`src/platform/macos/status.md:127-129`） |
| **iOS** | **iOS Simulator**（iPhone 17 Pro，iOS 26.2 / 23C54）——**非真机** | ✅ **已确认（模拟器；真机 ❌ 从未验证）** | 2026-10-15，`1f3a6fa8` | `bash tools/run_ios_testapp.sh` → 12 项全 `[PASS]`，`RESULT: PASS`（原始输出见本节 §4，宿主 macOS 15.7.3 / Apple M4 / Xcode 26.2） |
| **Android** | **真机 Xiaomi M2102J2SC**（Android 13/API 33，arm64-v8a）**+ 模拟器**（API 34 x86_64 KVM） | ✅ **已确认（旧修订，需复跑）** | 2026-09-11 | `ANDROID_ABI=arm64-v8a ANDROID_SERIAL=65441025 bash tools/run_android_testapp.sh` → JNI 双向 `RESULT: PASS`，真 `ACTION_OPEN_DOCUMENT` 选择器弹出。见 `docs/log/archive/log-20260911-1.md`、`src/platform/android/status.md` |
| **HarmonyOS / OpenHarmony** | 无设备、无模拟器（`hdc list targets` → `[Empty]`；SDK 不含 emulator） | ❌ **未验证（仅交叉编译 + 链接）** | 2026-09-16 → 2026-10-14 | `cargo ohos build -t {aarch64,armv7,x86_64}` 真产出 `librust_widgets.so`（ELF machine 校验）；`loongarch64` 为 Tier 3、无法构建。见 `tools/check_harmony_cross.sh`、`src/platform/harmony/status.md` |
| **Wayland** | 真 **headless weston** 合成器（`wl_compositor`/`xdg_wm_base` 真绑定） | ✅ **已确认（协议绑定；非屏幕合成/输入）** | 2026-09-11 | `tools/run_wayland_compositor_tests.sh` → `bound live compositor … 14 passed`。见 `docs/log/archive/log-20260911-1.md` |
| **WASM / Web** | **WASI 运行时（wasmtime）** 跑库代码；**浏览器 ❌ 未验证** | ✅ **已确认（仅库代码）** | 2026-09-02 | `cargo test --target wasm32-wasip1 --no-default-features --features wasm` → 2155 passed；浏览器 `wasm32-unknown-unknown` 只 `check`/`build`。见 `docs/log/archive/log-20260902-1.md` |
| **Embedded / mini** | **宿主模拟**（内存 framebuffer / 内嵌引擎）——**无真实硬件** | ⚠️ **宿主模拟** | 2026-10-15 | `cargo test --lib --no-default-features --features mini -- render_engine::embedded` → 4 passed；生成器产物真编译（`tools/check_generator_output_compiles.sh`）。见 `docs/log/log-20261014-1.md` 第 4 轮 |

---

## 1. Linux / X11 + GTK3 — ✅ 已确认

**宿主**：`Linux 7.0.0 x86_64`，X11 真桌面会话（`DISPLAY=:0`，`XDG_SESSION_TYPE=x11`），GTK **3.24.41**。

| 验证项 | 命令 | 结果 | 证据 |
|---|---|---|---|
| 窗口**真的在屏幕上**（问 X 服务器，不是问库） | `./target/debug/examples/probe_window_on_screen` | `server map state = Some("IsViewable")`；`server geometry = 480x320`；`xwd capture = 480x320 at 32 bpp, 62 distinct colours` | `docs/log/log-20261014-1.md` 第 4 轮 §7.2 |
| 真系统选择器打开并被应答 | `./target/debug/examples/probe_file_picker` | `choose_path answered Ok(None) after 406.195251ms`；worker 线程 `Err(Unavailable)` | 同上 §2.4 |
| **真实键入**进真对话框（XTEST X 键事件） | `RW_GUI_PROBE_OK=1 ./target/debug/examples/probe_file_picker` | `choose_path answered Ok(Some("/tmp/rw_picked_by_probe.txt"))` | 同上 §7.1 |
| 跨线程 `quit()` 干净结束 `run()` | `./target/debug/examples/probe_cross_thread_quit` | `run() returned after 415.753571ms`；窗口 `mapped=true visible=true realized=true` | 同上 §1.6 |
| 真实 DPI 切换端到端 | `GDK_SCALE=2 ./target/debug/examples/probe_cross_thread_quit` | 库报 `dpi scale factor: 2`；窗口 `scale=2` | 同上 §4 |

**边界（原样保留）**：
- 窗口结论是**机械判据**（尺寸正确、可查看、像素非平涂），**不是**人眼排版判断，也**不是**合成器截图（同上 §7.2）。
- **指针半**（真实鼠标点击落到控件）工具已就绪（`tools/x11_inject_keys.py --click X Y`）但**刻意未在活动桌面上跑**（同上 §7.3）。
- GDK 剪贴板的 `text/html` **标记通道**在本机不可验证（第 3 轮 §6.5）；第 3 轮里 GDK 代码是在 **macOS 宿主的 GTK** 上编译/运行的，**不得**当作 Linux-X11 证据。

## 2. Windows（Win32）— ✅ 已确认（旧修订，需复跑）

**宿主**：真 Windows **10.0.22621**（MINGW64_NT / MSVC，rustc 1.95.0），2026-09-15。

```text
$ cargo test --no-default-features --features desktop --lib -q
test result: ok. 4026 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

同机另跑：example `control_creation_is_single_mechanism`（`backend = WindowsPlatform … RESULT: PASS`）、
integration `--test widget_accessor_routing_test`（24 passed）、profile 门控与 clippy。见 `docs/log/archive/log-20260915-1.md`。

**边界**：
- 上述是**旧修订**（2026-09-15）上的真机结果；**当前 `HEAD` 未在 Windows 上复跑**（本机是 Linux，无 Windows 宿主）。
- **Windows 文件选择器（`GetOpenFileNameW` / `SHBrowseForFolderW`）仅编译**：逐字 `include!` + `cargo check --target x86_64-pc-windows-msvc` → Finished（`docs/log/log-20261014-1.md` 第 4 轮 §2.3/§6.3）。
- **整机 Windows 交叉检查在本 Linux 宿主上不可跑**：build script 需要 `lib.exe`（本机无 MSVC/mingw）。解除条件：`sudo apt install mingw-w64`，或 `cargo install xwin` + `xwin … splat`（`tools/check_profiles.sh`）。
- **Wine 下的 Windows PE 运行**被 `docs/issues/feedback-20261015.md:81-83` 引用过，但它引的证据日志 `docs/log/log-20261015-1.md` **不在本仓库**（属下游）——**本仓库内不可引用**。

## 3. macOS — ✅ 已确认（见各自边界）

**宿主**：真 Mac，macOS **15.7.3**，Apple M4 / arm64（2026-09-11 报 arm64 + rustc 1.98.0）。

### 3.1 `cocoa` 后端 — ✅

| 日期 / 修订 | 验证 | 结果 |
|---|---|---|
| 2026-09-11 | `examples/apple_appkit_probe`（`--features desktop`） | `RESULT: PASS (12 named checks)`：主线程、`NSApplication`、真 `NSWindow` 注册、`NSMenu`、`NSPasteboard` 往返、四种对话框、几何/可见性从**原生对象**读回（`docs/log/archive/log-20260911-2.md`） |
| 2026-10-14 / `c957fdec` | 契约探针（T01.01） | `backend_name()=cocoa`、`runtime_gui_mode()=NativeInteractive`、真的进 `[NSApplication run]`；**跨线程 `quit()` 不结束 loop**（即 feedback §6 的缺口，见 `docs/issues/feedback-20261013.md`） |

> 上表 2026-09-11 行的 AppKit 探针（`examples/apple_appkit_probe*`）随 BLUE15 自绘改造**已删除**（`tools/check_apple_native.sh` 记录移除），该行是**历史记录**；当前修订的 cocoa 真机证据以 2026-10-14 契约探针与宿主测试套件（`cargo test --no-default-features --features desktop --lib`）为准。

### 3.2 `macos-objc2-preview` 后端 — ⚠️ 部分

- 2026-09-11：AppKit 探针 `12 checks PASS`（**但该探针面向 BLUE15 之前的原生控件架构，现已删除**）。
- post-BLUE15 的真机客观证据只有 `NSPasteboard` 剪贴板：`cargo test --no-default-features --features "macos,desktop" --lib platform::macos_objc2` → **10 passed / 0 failed**（`docs/log/log-20261014-1.md` 第 3 轮 §5）。
- `run()` 仍是轮询 loop（`src/platform/macos/status.md:127-129`）。

### 3.3 边界

- `tools/check_apple_native.sh` 在非 macOS 宿主上**按设计** `exit 2`（`unsupported host`），故 Apple 运行时**不在** Linux 宿主覆盖范围内。
- `docs/log/log-20261005-1.md:214` 有一行「真实 macOS 原生控件 ✅ 真实创建按钮 + 标签」，与 BLUE15 后「无 per-kind 原生控件」的现状**不符**，标为**可疑/需复核**。

## 4. iOS — ✅ 模拟器已确认 / 真机 ❌ 未验证

**宿主**：Mac（macOS 15.7.3，Apple M4）+ Xcode **26.2**；运行时是 **iOS Simulator**（iOS 26.2 / 23C54，`iPhone 17 Pro`）。**非真机**。

```text
$ bash tools/run_ios_testapp.sh          # 2026-10-15, HEAD 1f3a6fa8
    runtime=com.apple.CoreSimulator.SimRuntime.iOS-26-2 device='iPhone 17 Pro'
    [PASS] ui_application: UIApplication.sharedApplication = 0x10ca05270
    [PASS] create_window_id: rw_create_window = 6000285942072475648
    [PASS] backend_window_is_a_painting_surface: before=1 after=2 library=0x10b7077b0 rootVC=0x10cc07340
    [PASS] control_handles: button=6000285942072475649 label=6000285942072475650 line_edit=6000285942072475651
    [PASS] self_painted_no_native_views: UIButton=0x0 UILabel=0x0 UITextField=0x0
    [PASS] text_roundtrip: rw_get_widget_text = Tapped
    [PASS] visibility_geometry: hidden_reported=1 shown_reported=1
    [PASS] destroy_unmounts: rw_destroy_widget=1 text_after=(empty)
    [PASS] backend_identified: rw_backend_name = custom-paint-control-backend
    [PASS] dpi_claim_matches_the_measurement: caps=0x30 dpi_scaling=0 scale=1.000
    [PASS] no_native_menu_claim: caps=0x30 native_menu=0
    [PASS] quit_is_clean: rw_quit returned without aborting
    RESULT: PASS
```

**边界与冲突**：
- **真机（物理 iPhone）从未验证**——本行是 `aarch64-apple-ios-sim`（模拟器），`docs/plans/FUTURE.md` 亦记「physical-device run not yet performed」。
- ✅ **原记录冲突已消解**：本节此前记「当前 `bindings/ios/main.m` 的完整检查集自 2026-09-30 起未复跑」，`src/platform/ios/status.md` 亦称 BLUE15 后的模拟器探针「未复跑、对本修订未验证」。本次（2026-10-15）在**真机宿主**的模拟器上复跑，当前探针全集 **12 项全 PASS**，含 `destroy_unmounts` / `dpi_claim_matches_the_measurement` / `quit_is_clean`。
- iOS `UIPasteboard` 剪贴板：**仅交叉编译**（`cargo check --target aarch64-apple-ios …`），无会话运行（`docs/log/log-20261014-1.md` 第 3 轮 §2/§10）。

## 5. Android — ✅ 已确认（旧修订，需复跑）

**运行时**：**真机 Xiaomi M2102J2SC（thyme），Android 13 / API 33，arm64-v8a** + **模拟器 API 34 x86_64（KVM）**。

```text
$ ANDROID_ABI=arm64-v8a ANDROID_SERIAL=65441025 bash tools/run_android_testapp.sh
I RustWidgetsTest: nativeInit ok
I RustWidgetsTest: nativeSelfTestKinds -> 7
I RustWidgetsTest: nativeSelfTestDialog -> 1
I RustWidgetsTest: nativeSelfTestFileDialog -> 1
I RustWidgetsTest: RESULT: PASS
✅ Android JNI integration test PASSED (arm64-v8a, serial=65441025, emulator=no)
```
`docs/log/archive/log-20260911-1.md`；摘要见 `src/platform/android/status.md:234-257`。

**边界**：
- 真选择器是原生 `ACTION_OPEN_DOCUMENT`（`com.android.documentsui/.picker.PickActivity`）——**真机实测**。AndroidX `Toolbar` 的约束已被**刻画**（非 pass/fail 功能）。
- **设备覆盖只有一台 arm64 真机**；`status.md` 明确把"超过一台设备"列为未验证。
- **JNI 剪贴板（`ClipboardManager`）仅编译验证**（2026-10-14），**未**在真机/模拟器运行（`src/platform/android/status.md:256`）。
- **MIUI/HyperOS 安装守卫**（真机发现）：普通 `adb install` 报 `INSTALL_FAILED_USER_RESTRICTED`，runner 自动回退到 `pm install-create/-write/-commit` 会话 API。
- 无 `ANDROID_SERIAL`/无 AVD 时，`tools/check_android_runtime.sh` 前置探测后 `exit 2`（`unsupported host`），**不**靠超时收场。

## 6. HarmonyOS / OpenHarmony — ❌ 未验证（仅交叉编译 + 链接）

**边界（不是遗漏）**：仓库里没有任何 HarmonyOS **运行时**验证工具（既无 `run_harmony_testapp.sh`，也无 `.hap` 构建/签名/安装流程），所以真机/模拟器**运行**从未被断言过。SDK 与模拟器本身是**装好的**（2026-10-15 在本机核对）：

```text
$ hdc -v
Ver: 3.1.0e                                    ← ~/Library/OpenHarmony/Sdk/18/toolchains
$ hdc list targets
[Empty]                                        ← 当前没有模拟器在运行
$ ls ~/Library/Huawei/Sdk/system-image
HarmonyOS-6.0.0                                ← 模拟器镜像（phone_all_arm）已安装
$ ls ~/.Huawei/Emulator/deployed
phone_config.json  versionInfo.txt  "Pura X View"  ...   ← 已部署一个模拟器实例
```

> ⚠️ **订正**：本节此前写「无设备、无模拟器」并列出「SDK 不含 emulator」。那只描述了 `~/Library/OpenHarmony/Sdk/<ver>/`（其中确实没有 `emulator/` 子目录）；模拟器实际由 DevEco Studio 提供——镜像在 `~/Library/Huawei/Sdk/system-image/`，二进制在 `DevEco-Studio.app/Contents/tools/emulator/Emulator`，且已有一个部署好的实例（`~/.Huawei/Emulator/deployed/`）。因此准确的表述是：**模拟器已安装、但未启动**（`hdc list targets` → `[Empty]`），而且**仓库缺少把它跑起来所需的 `.hap` 构建 + runner**——这仍是一个 ⬜ 工程，不是一条命令。

历史记录（当时确实无模拟器）：`docs/log/archive/log-20260930-4.md:595-609`；`docs/plans/harmony_integration.md` 亦把「真机/模拟器运行验证」标为 ⬜。

**已确认的只是编译 + 链接**：`aarch64` / `armv7` / `x86_64` 各**产出并链接**真 `librust_widgets.so`（ELF machine 与 triple 一致），XComponent 桥链接 `libace_ndk.z.so`；`tools/check_harmony_cross.sh` 正向断言该状态（`src/platform/harmony/status.md:255-259`、`:272-288`）。

**Tier 3 限制（如实记录）**：`loongarch64-unknown-linux-ohos` **无法构建**——rustup 无预编译 std（Tier 3）+ SDK 无 loongarch64 libc（`bits/alltypes.h` 缺失）。门禁把这一状态当作**正面断言**，而非假装通过。

## 7. Wayland — ✅ 已确认（协议绑定）

**运行时**：真 **headless weston** 合成器（rootless 模式）。

```text
[5/5] Running Wayland tests against the live compositor
bound live compositor: wl_compositor=yes xdg_wm_base=yes dpi_scale=1
test result: ok. 14 passed; 0 failed; 0 ignored
```
`docs/log/archive/log-20260911-1.md`；runner 为 `tools/run_wayland_compositor_tests.sh`。

**边界**：验证的是**真实协议绑定**，**不**覆盖屏幕合成与输入（headless 合成器）；另有 negative control（无 `WAYLAND_DISPLAY` → state-only）真实走一遍。

## 8. WASM / Web — ✅ 已确认（仅库代码）/ 浏览器 ❌ 未验证

| 验证项 | 命令 | 结果 |
|---|---|---|
| 库代码在 **WASI 运行时**真跑 | `cargo test --target wasm32-wasip1 --no-default-features --features wasm` | `2155 passed; 0 failed`（含一个**只在 32 位下暴露**的 `geometry::from_isize` 溢出 bug） |
| 浏览器目标 | `cargo check --target wasm32-unknown-unknown …` | 仅 `0 errors 0 warnings`（`check`/`build`，**无浏览器运行**） |
| 浏览器 `cargo test` | — | 明确记为"不可行"，改以 `wasip1 + wasmtime` 作为真实运行 |

见 `docs/log/archive/log-20260902-1.md`。**边界**：WASI 运行**不**经过 DOM/`web-sys` 路径；故"Web 渲染"**未被断言**（`tools/check_web_engine_honest.sh`）。cookbook 的 WASM "✅ Verified" 由「check + wasip1」支撑，**不是**浏览器。

## 9. Embedded / mini — ⚠️ 宿主模拟

- `cargo test --lib --no-default-features --features mini -- render_engine::embedded` → **4 passed**（内嵌引擎含跨线程停止）。
- 生成器产物真编译：`tools/check_generator_output_compiles.sh` → 5 passed（含 `mini`/`embedded` 模板）。
- **无真实硬件证据**（无 MCU / 板卡 / 目标上 QEMU）；`examples/embedded_host.rs` 是软件 framebuffer 的诚实示例。

---

## 10. 门禁在受限宿主上的 SKIP（原则 #103）

这些门禁在**能力不具备**的宿主上**前置探测**后 `exit 2` 并打印 `unsupported host (…)`，被
`tools/run_all_gates.sh` 记为 `SKIP(host-limited)`——**不是失败，也不是通过**：

| 门禁 | 触发条件 | 解除条件（哪个宿主） |
|---|---|---|
| `tools/check_apple_native.sh` | 非 macOS 宿主 | 一台 Mac |
| `tools/check_android_runtime.sh` | 无 `adb` / 无 emulator / 无 AVD 且无 `ANDROID_SERIAL` | 有 adb + 设备或 AVD 的宿主 |
| `tools/check_profiles.sh` `[6b/9]` | 装了 `x86_64-pc-windows-msvc` 但宿主无 C 工具链 | `sudo apt install mingw-w64` 或 `xwin … splat` |
| `tools/check_harmony_cross.sh` | 缺 `cargo-ohos` / 缺 OHOS SDK / 目标未装 | 装了 OpenHarmony SDK 的宿主 |
| `examples/probe_file_picker.rs` | GTK 构建下无显示会话 | 有显示会话的桌面 |
| `examples/probe_window_on_screen.rs` | 缺 `wmctrl`/`xwininfo`/`xwd` | 装有 X 工具的桌面 |
| 三个 GUI 探针 | 未设 `RW_GUI_PROBE_OK=1` | 显式同意在活动桌面上弹窗 |

> 记录口径：**超时 ≠ 失败 ≠ 通过**（原则 #59.4）；`SKIP(host-limited)` 表示"该门禁跑了，并诚实地回答了它跑不了"。

## 11. 记录冲突与待复核（不单方面采信）

| 项 | 冲突内容 | 处置 |
|---|---|---|
| iOS Simulator 是否复跑 | `src/platform/ios/status.md:120-126` 说"未复跑" | ✅ **已消解**：2026-10-15 在真机宿主的 iOS 26.2 模拟器上复跑，当前探针 12 项全 PASS（见 §4） |
| macOS「两个后端都已原生验证」 | `src/platform/macos/status.md:6,22` 的"Last verified 2026-09-11"面向**已删除的原生控件架构**；而 2026-10-05 日志又出现一条与 BLUE15 后现状不符的"原生控件 ✅" | 标 ⚠️/❓；以 2026-10-14 `cocoa` 契约探针为当前修订的真机证据 |
| 引用路径 | `src/platform/macos/status.md` 引用 `docs/log/log-20260911-2.md`，实际在 `docs/log/archive/` 下 | 待修正引用（文档漂移） |
| Apple 宿主存在性 | `docs/reports/CHANGELOG.md`（2.8.3，2026-09-30）称"开发机上没有 Apple 宿主"；同日的 iOS Simulator 日志却在跑 | 标 ❓；可能是**两台不同开发机**，未采信任一方 |
| Wine / Windows PE | `docs/issues/feedback-20261015.md` 引用本仓库不存在的 `docs/log/log-20261015-1.md` | 本仓库**不可引用**；不在本台账 |

## 12. 维护约定

- 新增一次真机验证时，**更新本文件对应行 + 该后端的 `status.md`**，并附命令与原始输出（或日志路径 + 段落）。
- 结论从「已确认」降级为「已过期」时，写清**复跑所需的宿主**。
- 本文件与 `docs/plans/platform_capability_matrix.md` 是两个口径：前者是**运行时可达性台账**，
  后者是**控件能力矩阵**（由脚本生成，其 ✅ 指"创建了平台原语"，**不等于**本文件的真机结论）。
