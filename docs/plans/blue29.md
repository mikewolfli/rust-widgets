# BLUE29 — wgpu 路径选择：速度与美观的兼顾方案

> **来源**：用户要求「确认兼顾性能和效果、速度和美观的方案」（2026-10-30）。
>
> **证据基线**：`docs/log/log-20261030-1.md` §39（同一命令流 22 行、两轮实跑）+ 当前
> `src/wgpu_backend/` 源码事实。
>
> **本文是提案**：条目全部 `[ ]`，未改任何生产代码；文件名 `blue29.md`，与
> `archive/blue22.md`（历史 BLUE22）无主题关系。
>
> **判定口径**：
> **速度** = `benches/wgpu_bench.rs` 的实跑中位数（跨机器不可比，只比同一张表的行间比值）；
> **效果** = 三档之一，写死在用例里：**A 逐字节相同** · **B ≤1 LSB** · **C 允许边缘 AA 差异、
> 但形状/覆盖必须一致**。没有「看起来差不多」。

---

## 0. 结论（一页）

不存在「一个开关同时最优」。能兼顾的结构是**四层决策**，顺序固定、每层只有一个职责：

```
L0 质量意图（宿主声明）    ByteExact → 只允许效果 A（含「透明目标 + 无整帧 clear」的流也不上 GPU）；
                          Antialiased → 允许 B/C
L1 交付面（宿主能力）      能消费纹理 → with_draw_commands_texture（零回读）
                          必须拿像素 → 进入 L2
L2 成本门控（同一流的账）  argmin(CPU 成本, GPU 成本)，接近时保持上次选择（迟滞）
L3 回退（已是现状）        不可覆盖的流 → CPU 光栅器
```

一句话：**能不上 CPU 就不上（L1）；上不上 GPU 看两路成本、不看分辨率（L2）；要逐字节还是要
更好看，由 L0 明说，不由「碰巧走了哪条路」决定。**

---

## 1. 事实基线（取证，不是主张）

### 1.1 性能：分界线是成本，不是分辨率

| 量 | 实测（本机） | 含义 |
|---|---|---|
| GPU 回读交付下限 | **389.7 µs**（320×200）、**541.8 µs**（720p）（两轮中的最小行） | 与帧大小几乎无关，主要是同步等待 |
| GPU 纹理交付下限 | **24–64 µs**（简单图元），但**按对象收费** | `text-40` 228/682 µs、`images-8` 158/143 µs、`shadows-blur-8` 339/411 µs（320/720） |
| 回读路径胜负 | 320×200 **11/11 CPU 胜**；720p **9/11 CPU 胜** | GPU 只赢 `gradient-full`（2.27–2.36×）与 `blend-full`（3.02–3.10×） |
| 纹理交付 vs CPU | 720p **4.50–4.61×** 便宜；320×200 **CPU 反而快约 1.5×** | 小帧连交付都轮不到 GPU 赢 |
| borrowed vs owned | 0.97–1.42（320 持平，720 更快） | 像素宿主应优先 borrowed |

**能复现全部 22 行的保守规则**（L2 的第一版就照这个写，不拟合模型）：

> 默认 CPU；仅当 **面积 ≥ ~1 MP 且流属于 {大面积渐变, 非 Normal 混合模式}** 时走 GPU。
> 它不是普适真理，是「本机 + 两类已实测负载」的最小可行近似；`mixed`（CPU 883 / GPU 1698）
> 说明「面积大」单独不构成理由——GPU 计算本身贵时仍必须留在 CPU。

### 1.2 效果：GPU 与 CPU 的差异清单（都有出处）

| 场景 | 差异 | 档位 |
|---|---|---|
| 不透明矩形/图像/渐变/路径/阴影/文本 | 逐字节相同（有用例钉住） | A |
| 半透明合成 | 固定功能混合在目标里乘完即舍入，差 1 LSB（文本实测 6 字节） | B |
| 透明目标且流内无 clear | GPU 存「预乘 × 覆盖度」，CPU 存直通 alpha → 差值可**远超 1 LSB** | 不在 A/B 内（`Antialiased` 也需显式知道） |
| 圆/弧/圆角描边的 SDF 管线 | GPU **抗锯齿**，CPU 硬边 | C（视觉更好，但不可复现） |
| 非 Normal 混合模式（`set_blend_modes_on_gpu`，默认关） | 每条命令一次图层 + 快照合成 | A（不透明）/ B（半透明） |

出处：`src/wgpu_backend/mod.rs` 模块文档（#A known divergence）、
`src/wgpu_backend/renderer/frame.rs::set_blend_modes_on_gpu`、日志 §16.2。

**由此得到的关键事实**：「效果更好」（AA）与「同输入同像素」（可复现）互斥。当前实现把选择权
交给了「哪条路径接了这条流」——同一个含圆的流，走 GPU 抗锯齿、走 CPU 硬边，这是隐式质量切换。

### 1.3 三处结构性缺口（方案必须各自处置）

1. **调度按「可覆盖」而不是「划算」**：`renderer/dispatch.rs::commands_gpu_dispatchable_with`
   只回答「管线能不能画」，不回答「该不该画」。后果：720p `clear` 走 GPU+回读 0.54 ms，
   而 CPU 只要 0.23 ms。
2. **质量由路径隐式决定**：没有质量档位 API，要「逐字节一致」的宿主对圆/弧流拿不到 CPU 分支。
3. **测量与节流缺基础设施**：公开 API 无 `wait`/`idle`，`render_frame` 无 present 路径
   （直接返回 `Err`），因此端到端 GPU 帧时间现在测不到、也无法限流。

---

## 2. 方案

### 2.1 决策顺序（L0 → L3，固定）

```text
quality = host.quality()             // ByteExact | Antialiased，默认待定（见 §5 D-1）
if quality == ByteExact:
    refuse streams containing AA-affected shapes (circle fill / ring / arc / rounded outline)
    refuse streams drawn onto a transparent target with no whole-frame opaque clear
if host.consumes_texture():          // L1
    encode on GPU into texture; f(&view); return        // 零回读
else:                                // L2
    if stream refused by dispatcher → CPU                // L3
    else if cost_gate.choose(stream) == Gpu → GPU + readback
    else → CPU
```

`cost_gate.choose` 的**唯一**输入是「这条流的 CPU 估计成本」与「GPU 交付下限 + 该流 GPU 计算成本」；
比较结果落在 ±10% 内时**沿用上次选择**（迟滞），避免同一画面逐帧抖动。

### 2.2 质量档位（L0）

- `ByteExact`：像素必须与 CPU 参考逐字节相同 → 两类流强制 CPU：
  ① 含 AA 形状（圆填充/环/弧/圆角描边）；② 画在透明目标上且流内没有整帧不透明 clear
  （预乘存储差异与 AA 无关，不靠 AA 开关解决）。其余负载不受影响。
- `Antialiased`：允许 B/C 两档；SDF 形状留在 GPU。
- 语义只描述意图，不出现 `gpu`/`cpu`/`sdf` 等机制词汇（原则 #52）。

### 2.3 交付面（L1）

三条入口已在 `renderer/stream.rs` 中收口，本方案**不新增入口**，只把「哪类宿主用哪条」写进文档：

| 宿主 | 入口 | 依据 |
|---|---|---|
| GPU 上消费（自采样/present） | `with_draw_commands_texture` | 720p 比 CPU 便宜 4.5×，比回读便宜 5.5× |
| 拿 CPU 像素 | `render_draw_commands_cached_rgba8` | borrowed 从不更差（0.97–1.42） |
| 需要独占 `Vec<u8>` | `render_draw_commands_rgba8` | 允许每帧一次分配时用 |

### 2.4 成本门控（L2）

- 第一版 = §1.1 的白名单（面积 + 负载类别），**两个常数**（面积阈值、迟滞带）写在代码里并注明出处。
- 门控只影响「像素回读」路径。**纹理入口第一版不做门控**：它的替代形态是「CPU 光栅 + 上传」，
  而这份成本还没测（B29.04），在拿到数之前不拿「不做门控」冒充已证明的选择。
- 门控不改变像素：两条分支都是既有实现，逐字节保证不变。

### 2.5 非目标（具名排除，不计入完成）

1. **不做通用成本模型拟合**：样本量（22 行 × 2 轮）不足以支撑系数回归；白名单更稳。
2. **不引入 swapchain/present**：本计划只解决「选哪条路」，不解决「怎么上屏」；那需要另一份计划
   （前置项：`render_frame` 仍返回 `Err`）。
3. **不改 CPU 光栅器输出**：它是效果 A 的参考实现。
4. **不承诺跨机器固定阈值**：阈值随 adapter 变化，门控必须可被 bench 重新标定。

---

## 3. 条目（B29.01–B29.06）

### B29.01 质量档位类型与行为

- [x] `RenderQuality { ByteExact, Antialiased }` + `set_render_quality` / `render_quality`；
      `ByteExact` 时调度器拒绝含 `circle fill / ring / arc / rounded outline` 的流。
      **落点**：`src/wgpu_backend/renderer/policy.rs`（类型 + 两个拒绝规则）、
      `stream.rs`（三条入口先问意图）、`frame.rs`（setter/getter）。透明目标规则见 §2.2 ②，
      已按验收 5 一并实现。
- 验收（`cargo test --lib --features desktop,gpu-wgpu wgpu_backend`）：
  1. `ByteExact` + 含圆流 == `rasterize_draw_commands_rgba8` 逐字节；
  2. `ByteExact` + 纯矩形流 == CPU 逐字节（档位不误伤效果 A 的负载）；
  3. `Antialiased` + 含圆流：边缘至少 1 像素与 CPU 不同，且形状断言仍通过（档位真的生效）；
  4. 两档对纯矩形流的输出**相同**（证明只影响 AA 类）；
  5. `ByteExact` + 透明目标且无整帧 clear 的半透明流 == CPU 逐字节（预乘差异不在 AA 之外被遗漏）。
- 反向注入：删掉 `ByteExact` 分支 ⇒ 用例 1 变红。

### B29.02 像素回读路径的成本门控

- [x] 在 `stream.rs` 的两条像素入口前接入 `cost_gate`（§2.4 白名单 + 迟滞）。
      **落点**：`renderer/policy.rs::CostGate`；`COST_GATE_AREA_THRESHOLD = 900_000`
      （= 720p 的 921 600 下方，理由见常量文档）、`COST_GATE_HYSTERESIS_PERCENT = 10`。
      门控只对“像素回读”路径生效；纹理入口不门控（§2.4）。
      **实测（本机，2026-10-30）**：320×200 winner 全 `cpu`；720p 仅 `gradient-full` /
      `blend-full` 为 `gpu`；320×200 选中路径总耗时 942.0 µs vs 强制 CPU 941.0 µs（+0.1%，
      在 5% 预算内）。
- 验收：
  1. `cargo bench --features desktop,gpu-wgpu --bench wgpu_bench` 的 winner 列：
     320×200 全 `cpu`；720p 仅 `gradient-full` / `blend-full` 为 `gpu`（与 §1.1 一致）；
  2. `ByteExact` 下同一批流的像素与改动前逐字节相同；
  3. 320×200 CPU 汇总耗时不劣于改动前 5%（门控不能引入额外开销）。
- 风险：行为变化（同一流可能从 GPU 换到 CPU）→ 按原则 #21 需写明边界与迁移说明。

### B29.03 公开的「等待设备」入口

- [x] `pub fn wait_idle(&self) -> Result<(), String>`（名字描述意图，不暴露队列/栅栏机制）。
      **落点**：`renderer/frame.rs`，超时沿用 `GPU_READBACK_TIMEOUT`（D-3 取值）。
- 验收：bench 新增 `texture+wait` 列，用来量化「纹理交付」当前低估了多少 GPU 完成时间；
      用例证明 `wait_idle` 之后立即读回与之前读回相同（没有新帧、也没有丢帧）。

### B29.04 纹理入口的「被拒流」分支补测

- [x] bench 增加一列：`with_draw_commands_texture` 在 CPU 光栅分支（流被拒）的耗时。
      **实测**：`cpu-fallback` 列 320×200 为 34.9–323.8 µs，720p 为 359.0–4493.2 µs，均非零。
- 验收：该列存在且非零；日志写明「纹理宿主对不可覆盖流的真实代价 = CPU 光栅 + 上传」。

### B29.05 文档与口径同步

- [x] `src/wgpu_backend/mod.rs` 的 dispatch 段加入「质量档位 + 成本门控」两段事实；
      `benches/wgpu_bench.rs` 表头补 `ByteExact/Antialiased` 两档的运行方式。
      **实测**：`cargo doc --no-deps --features desktop,gpu-wgpu` 无**新增**警告（仅存量
      `set_blend_modes_on_gpu` / `render_draw_commands_rgba8` 两处私有链接警告）；
      `tools/check_source_structure.sh` 通过；`rustfmt --check` 干净。
- 验收：`cargo doc --no-deps` 无**新增**警告；`tools/check_source_structure.sh` 通过；`rustfmt --check` 干净。

### B29.06 端到端上屏（**前置，不在本计划内实现**）

- [x] 仅登记依赖：present/swapchain 缺失使「纹理交付」的收益只能在宿主自采样场景成立。
      **登记位置**：`mod.rs` 的 Reachability 段 + `frame.rs::render_frame` 的 `Err` 说明；
      本节仍不实现上屏。验收口径不变：`texture` 数字只代表 **CPU 侧交付成本**。
- 验收：另立计划后回填本行；在此之前 §1.1 的 texture 数字只代表 **CPU 侧交付成本**。

---

## 4. 总验收（全部条目完成后一次跑）

```text
cargo test --lib                                           # 全量
cargo test --lib --features desktop,gpu-wgpu wgpu_backend  # 后端用例
cargo clippy --all-targets --no-default-features --features desktop -- -D warnings
cargo clippy --lib --tests --features desktop,gpu-wgpu -- -D warnings
cargo bench --features desktop,gpu-wgpu --bench wgpu_bench # 两档质量 × 三条交付
bash tools/check_source_structure.sh
```

通过判据：**效果 A 的负载在 `ByteExact` 下逐字节不变**；**§1.1 的 winner 分布不变差**；
`B29.03`/`B29.04` 新增列有数、且日志写明其口径与边界（原则 #105）。

---

## 5. 需要决策（⚠️ 未定，不自行假设）

| 编号 | 问题 | 两个选项的后果 |
|---|---|---|
| D-1 | `RenderQuality` 默认值 | 默认 `ByteExact`：圆的边缘从抗锯齿变硬边（行为变化，可复现优先）；默认 `Antialiased`：行为不变，但「逐字节」要显式申请 |
| D-2 | 是否允许 B29.02 改变既有调度行为 | 允许：性能按实测最优；不允许：只能做 L0/L1（速度收益仅限纹理宿主） |
| D-3 | `wait_idle` 的超时策略 | 沿用 `GPU_READBACK_TIMEOUT`（与回读一致）；或独立常量（需要新理由） |
| D-4 | 阈值归属 | 白名单常数（稳、可解释）；或 bench 标定后写入（随 adapter 变化、需要重跑机制） |

---

## 6. 决策记录（本轮落定，可回滚）

§5 的四项在实施时按「不改变既有画面、可被实测推翻」取值。这里记录取值、理由与回滚方式；
不是新增约束，§5 仍是这些问题的原始登记处。

| 编号 | 本轮取值 | 理由 | 回滚 |
|---|---|---|---|
| D-1 | **默认 `Antialiased`** | 与实施前画面一致；`ByteExact` 由宿主显式申请，避免静默把圆边从抗锯齿换成硬边 | 改 `RenderQuality` 的 `#[default]` |
| D-2 | **允许 B29.02 改变调度** | 计划的核心收益就是同一流的成本门控；不允许则 §1.1 的实测无法转成默认行为 | `set_cost_gate_enabled(false)` 恢复逐流 GPU |
| D-3 | **沿用 `GPU_READBACK_TIMEOUT`（10 s）** | 与回读同一条等待预算，少一个需要解释的常量 | 改 `wait_idle` 的超时实参 |
| D-4 | **白名单常数 + 文档出处** | 22 行 × 2 轮不足以支撑系数回归；阈值随 adapter 变化，故用可覆盖的常数 | 改 `COST_GATE_AREA_THRESHOLD` 或按 bench 重新标定 |

**一个只读查询**：`WgpuRenderer::selects_accelerated_pixels` 回答门控的选择本身，供 bench 的
winner 列使用——门控把流留在参考光栅器时，两个计时列测量的是同一实现，只差噪声，用计时判
胜负会报出假的 GPU 胜场。它不是第四条交付入口，不改变任何像素。

**未覆盖（诚实声明）**：门控阈值只在「本机 + benchmark 的 11 类流」上验证；另一张 GPU
（例如集显或软件回退）上的最优点可能落在阈值另一侧。B29.02 的验收是「winner 分布与本机
§1.1 一致」，不是跨机器承诺（§2.5.4）。

---

## 7. 端到端上屏（B29.07–B29.16）

> **状态：计划，条目全部 `[ ]`，未改任何生产代码。**
> **来源**：用户要求把 B29.06 的前置项（present/swapchain）做成「完美闭环」，并给出本机
> 可运行的操作系统：macOS（真机）、Windows（`xwin` 交叉）、OpenHarmony（SDK 已装）、
> Android（SDK + 模拟器）；其余平台「后期运行」。

### 7.0 「完美闭环」的定义（先定口径，#105）

本节的「闭环」不是“代码写了”，而是**每个平台逐项可判定**：

| 行 | 判据 | 允许的诚实结论 |
|---|---|---|
| 编译 | 该平台目标 `cargo check` 通过 | `PASS` |
| 链接 | 目标产物真实链接（C/FFI 依赖可解析） | `PASS` |
| 真实上屏 | 真机/模拟器窗口出现、present 成功、无花屏 | `PASS` 或 `SKIP(host-limited)` |
| 像素 | `Exact` 档与现有 CPU blit 逐字节一致；`Smooth` 档差异只落在 §1.2 具名集合 | `PASS` / `NAMED-DIVERGENCE` |
| 尺寸 | resize / DPI / 最小化恢复 / 多屏移动 | `PASS` |
| 错误 | lost / outdated / timeout / OOM / device lost 均有明确返回值 | `PASS` |
| 拆除 | 关窗、切后端、退出无泄漏/无悬挂 | `PASS` |

**不允许把 `SKIP(host-limited)` 计入 `PASS`**（原则 #103/#105）。本机没有运行时的平台，只能
得到「编译/链接闭环 + 运行具名排除」，并附可复制的真机运行手册。

**不可承诺项（必须写进文档，不能靠“看起来一样”）：**
1. `Smooth` 档与 CPU 参考**不能**逐字节相同——圆/环/弧/圆角描边的 SDF 边缘见 §1.2；
2. 宿主合成器最终呈现的像素，测试只能验证到「present 成功 + 可读回帧 + 截图非空」，不能断言
   用户屏幕上的每个像素；
3. 模拟器/软件适配器的性能数字不代表真机（Android 尤其）。

### 7.1 本机取证（2026-10-30 实查，不是假设）

| 事实 | 证据 |
|---|---|
| macOS 真机可运行 | 宿主为 `aarch64-apple-darwin`，`cargo test --lib` 实跑 10305 passed |
| macOS 原生句柄已具备 | `Platform::raw_window_handle` 在 `macos`、`macos_objc2` 已实现（AppKit `NSView`） |
| Windows 句柄已具备，但只有交叉 | `windows/platform_impl.rs` 返回 `Win32`；本机有 `cargo-xwin` + `~/.xwin`，无 Windows 运行时 |
| Android 可跑模拟器 | `~/Library/Android/sdk/{adb,emulator}` 存在；`system-images/android-34/google_apis/arm64-v8a` 与 AVD `rw_arm64` 存在 |
| Android 句柄未实现 | `rust-widgets` 内 `raw_window_handle` 仅 macos/macos_objc2/windows 有实现；Android 后端只有 surface 记录 + repaint 队列 |
| OpenHarmony SDK 已装 | `~/Library/OpenHarmony/Sdk/18`（含 `native`、`toolchains/hdc`） |
| OpenHarmony 无运行设备 | `hdc list targets` → `[Empty]`；只能先编译/链接，运行时待设备 |
| OpenHarmony 句柄未实现，但上游有能力 | `raw-window-handle 0.6.2` 有 `OhosNdk`/`AndroidNdk`；`wgpu-hal 30.0.1` 的 `gles/egl.rs` 处理 `Rwh::OhosNdk`，`vulkan/instance.rs` 只处理 `AndroidNdk` |
| 绘制入口在平台画布 | macOS `blit_frame`/`draw_rect`、Windows `paint_canvas`、Linux `connect_draw`、Harmony XComponent 回调、Android repaint 队列 |
| 现有 GPU 桥很小 | `wgpu_backend/bridge.rs` 只映射 `FillPathNonZero/PushClip/PopClip/SetBlendMode`；`RenderCommand` 约 38 个变体 |
| 帧循环归宿主 | `Platform::pre_present_notify` 文档明写「library never starts a frame loop；host drives the loop」 |

### 7.2 架构（固定，避免两套并行机制，#101）

```text
宿主事件循环（平台已拥有）
  └─ 画布回调（现有 blit 点）
       └─ FramePresenter（新增，平台无关，位于 src/render/present/）
            ├─ Exact  ：widget::runtime 产出 RGBA → 上传当前呈现目标 → present
            └─ Smooth ：GpuPaintBackend 记录 RenderCommand → 桥接 WgpuDrawCommand
                        → 既有管线绘制 → present；桥不完整的帧整帧回落 Exact
```

- **平台层只提供两件事**：原生窗口句柄（`raw-window-handle`，已有 trait 位）与“何时该重绘”。
  原生类型不离开 `src/platform/`；`FramePresenter` 只吃 `RawWindowHandle`/`RawDisplayHandle`。
- **公有 API 不出现机制词**（原则 #52）：宿主用 `WindowHandle::set_frame_presentation(..)`
  选择意图，不出现 `swapchain`/`surface`/`gpu`。
- **只有一条呈现路径**：`FramePresenter` 内部 State 机；平台画布不再直接 `blit`（保留为
  `Default` 档的回退实现，不另起一套 present）。
- `unsafe` 边界：从 `RawWindowHandle` 到 `SurfaceTargetUnsafe` 的调用必须注释前置条件
  （句柄在窗口存活期内有效、主线程约束、调用方不并发销毁窗口）。

### 7.3 条目

#### B29.07 呈现能力契约与句柄覆盖（前置）

- [ ] `Platform::frame_presentation_supported() -> bool`（默认 `false`，各后端按事实覆盖）；
      不得以 `cfg(target_os)` 在调用点分支（原则 #41/#53）。
- [ ] 补齐 `raw_window_handle`：Android（`ANativeWindow*` → `AndroidNdk`）、
      OpenHarmony（XComponent `OHNativeWindow*` → `OhosNdk`）；均受 `raw-window-handle`
      feature 门控，未启用时返回 `None`（现状默认）。
- [ ] 句柄生命周期契约写成文档：只在窗口存活期有效；窗口销毁后 presenter 必须先失效；
      跨线程/主线程约束由各后端文档说明。
- 验收：四平台 `cargo check`；`raw-window-handle` 未开启时所有后端仍编译；
  `check_platform_impl_matrix.sh` / `check_platform_capability_matrix.sh` 通过。
- 反向注入：把某后端 `frame_presentation_supported` 改成无条件的 `true` 而句柄仍 `None`，
  能力矩阵门控必须变红。

#### B29.08 `FramePresenter` 与配置（平台无关）

- [ ] 新增 `src/render/present/`（`mod.rs` 只做声明/导出，原则 #8）：
      `FramePresenter::{new, configure, resize, present, status, wait_idle}`。
- [ ] 配置是纯函数：`choose_config(caps, size, scale, quality) -> SurfaceConfiguration`；
      格式优先级、alpha mode、present mode（`AutoVsync` 默认，`Fifo` 回退）都有单测。
- [ ] 颜色空间判定：现有 CPU 帧是 sRGB 字节；必须实测「写入 → 读回」是否逐值一致，
      不一致则改用 `*Unorm` + 视图/sRGB 映射，禁止用“看起来对”放过。
- 验收：无 GPU 的纯逻辑测试覆盖尺寸/缩放/格式；真实测试覆盖 configure 成功与非法尺寸报错。
- 公共 API：`FramePresentation { Default, Exact, Smooth }` + `WindowHandle::set_frame_presentation`
      返回 `bool`（不支持时 `false`，保持 `Default`）。

#### B29.09 `Exact` 档（先闭环像素，再谈加速）

- [ ] 帧来源沿用 `widget::runtime::render_frame_[tree]_cached_shared` 的 RGBA；
      上传到当前呈现目标后 present。
- [ ] `Exact` 的判定复用 B29.01 的 `quality_refuses_stream`（AA 形状 / 透明无 clear）——
      但 `Exact` 本身**永远**走 CPU 光栅 + 上传，不依赖 `Smooth` 的桥接完整度。
- [ ] 损伤：首帧/尺寸变化/恢复全帧；稳定帧只上传 `present_damage` 报告的行带；无损伤不 present。
- 验收：macOS 真机 present 成功；读回帧与 `render_frame_*` 逐字节一致（含 BGRA/RGBA 映射）；
      连续 300 帧无花屏、无泄漏；resize 后一帧内恢复。
- 反向注入：删掉 `Exact` 分支后 `Smooth` 对含圆流不再逐字节，像素用例变红。

#### B29.10 `Smooth` 档与 `RenderCommand` 桥（最大工作量）

- [ ] 新增 `GpuPaintBackend`（实现 `PaintBackend`），只**记录** `RenderCommand`，不逐条提交；
      帧末一次性桥接 + 绘制。
- [ ] 桥接普查：`RenderCommand` 每个变体必须归入且仅归入一类，门控打印计数（原则 #100）：
      `Mapped` / `MappedWithDivergence`（仅 §1.2 的 AA 集合）/ `ForcesCpu`（该帧整帧回落 `Exact`）。
- [ ] 未分类变体默认 `ForcesCpu`，不得静默丢弃或近似（`bridge.rs` 的既有纪律）。
- [ ] 仅当普查 `ForcesCpu = 0 或已具名` 且逐变体像素用例通过，才允许 `Smooth` 上线；
      `Smooth` 帧内出现任一 `ForcesCpu` ⇒ 整帧走 `Exact`（不混用两条路径）。
- 验收：`Mapped` 变体逐条 GPU vs CPU 比对；`MappedWithDivergence` 只允许 §1.2 集合；
      `ForcesCpu` 逐条列出原因。
- 反向注入：把一个 `Mapped` 变体误标后，其像素用例必须变红；把 `ForcesCpu` 静默跳过，
      普查门控必须变红。

#### B29.11 尺寸 / 缩放 / 生命周期 / 错误

- [ ] resize、scale factor 变化、最小化（0 尺寸）、恢复、全屏、跨屏移动、关窗。
- [ ] `SurfaceError` 映射为语义状态：`Lost`（重建）、`Outdated`（重配）、`Timeout`（本帧跳过、
      下帧重试、连续 N 次上报）、`OutOfMemory`、`DeviceLost`（停止并回落 `Default`）。
- [ ] 禁止无限重试；每次状态变化有日志与计数。
- 验收：错误分支用可注入的假 surface 状态单测；macOS 真机覆盖 resize/最小化/恢复/关窗；
      Android 模拟器覆盖旋转与后台切换。

#### B29.12 帧循环与节流

- [ ] presenter 只从现有画布回调调用（7.1 列表），库不新建事件循环。
- [ ] 无损伤不 present；`wait_idle` 只用于测试/bench，不进入每帧路径。
- [ ] vsync 默认；掉帧策略（本帧丢弃、保持上一帧）明确；`pre_present_notify` 在需要的后端
      （Wayland 已实现）于 present 前调用。
- 验收：空闲 60 s 无 GPU 提交（计数为 0）；稳定帧的 present 次数 = 损伤帧数。

#### B29.13 平台接线（按本机可验证性排序）

**macOS（真机，先闭环）**
- [ ] `macos_objc2`（默认运行时）与 `macos`（`cocoa-legacy`）两个画布都接 `FramePresenter`；
      AppKit 主线程约束写成文档并由调试断言守护。
- [ ] 真机脚本：有界运行、窗口可见、present 成功、截图非空、退出码明确。

**Android（SDK + arm64 模拟器，可闭环）**
- [ ] JNI 新增入口接收 `Surface` → `ANativeWindow_fromSurface` → 句柄注册；
      `Platform::raw_window_handle` 返回 `AndroidNdk`；窗口销毁时注销。
- [ ] 适配器探测：Vulkan 优先、GLES 回退；都不可用 ⇒ `unsupported host (no Vulkan/GLES3)`。
- [ ] `tools/run_present_android.sh`：启动 `rw_arm64` AVD、安装、有界运行、`logcat` 判定；
      模拟器性能标注为非代表性。

**OpenHarmony（SDK 已装，无设备 ⇒ 运行具名排除）**
- [ ] XComponent `on_surface_created/changed/destroyed` → `OHNativeWindow*` → `OhosNdk`；
      回调线程与 UI 线程边界写入文档。
- [ ] **只能走 GLES/EGL 档**：`wgpu-hal 30.0.1` 的 Vulkan 后端没有 `OhosNdk` 分支
      （`vulkan/instance.rs` 只有 `AndroidNdk`），GLES 后端明确处理了 `Rwh::OhosNdk`。
      因此降级必须可见（原则 #45）：上报 `GpuBackendTier::Gl`，不得声称 Primary。
- [ ] `tools/run_present_harmony.sh`：`hdc list targets` 为空时打印
      `unsupported host (no hdc target)` 并 `exit 2`；有设备时执行同一脚本。

**Windows（xwin 交叉，无运行时 ⇒ 运行具名排除）**
- [ ] `Win32` 句柄接 presenter；`cargo xwin` 完整 desktop profile 的 check + link。
- [ ] Wine 冒烟为 best-effort：能跑则记录；不能跑则
      `unsupported host (no Windows/Wine GPU surface)`，**不计入 PASS**。

**其它平台（Linux/Wayland/X11、wasm、iOS）**
- [ ] 具名排除，写明进入条件：Linux 需 rwh + 现有 Wayland surface 接线；wasm 需 canvas
      目标与 `wasm32` surface；iOS 需 UIKit 句柄与移动端生命周期。后期运行。

#### B29.14 门控与真机脚本

- [ ] `tools/check_present_matrix.sh`：探测每平台能力 → 决定 `RUN / SKIP(host-limited)`；
      所有子脚本走 `rw_run_bounded`，预算写 `# rw-gate-budget:`（原则 #104）。
- [ ] 真机脚本必须**有界**且每次迭代可见进度（原则 #58/#59）。
- [ ] 截图冒烟只断言「非空 + 至少一个 clear 颜色区域」，不做像素 golden（合成器差异）。
- 验收：本机 macOS `RUN`；Android 模拟器 `RUN`（或明确的适配器缺失 SKIP）；
      HarmonyOS/Windows `SKIP(host-limited)` 且耗时秒级到分钟级。

#### B29.15 性能与功耗（口径 #105）

- [ ] `benches/present_bench.rs`：`Exact` / `Smooth` / 现有 CPU blit 三路，分别测
      encode+upload+present+wait；60/120 Hz 两个节流档。
- [ ] 空闲无提交、稳定帧只 present 损伤区、GPU 内存不随帧数增长。
- 验收：macOS 真机给出三路数字；Android 模拟器数字标注非代表性；不承诺跨机器阈值。

#### B29.16 兼容与回退

- [ ] 默认 `FramePresentation::Default`（现有 CPU blit），行为与像素零变化；
      `Exact`/`Smooth` 由宿主显式开启（原则 #21 的向前兼容）。
- [ ] 不支持时 `set_frame_presentation` 返回 `false` 并保持 `Default`，不静默切换画面。
- [ ] 迁移说明：`Default → Exact` 像素不变、只是交付方式变化；`Exact → Smooth` 允许 §1.2
      的 AA 差异，逐字节需求留在 `Exact`。

### 7.4 实施顺序（每步独立可验证）

1. **B29.07** 句柄覆盖 + 能力契约（纯编译/单测，无运行时依赖）。
2. **B29.08 + B29.09（macOS）** `Exact` 真机闭环：present 成功、像素一致、resize/恢复。
3. **B29.11 + B29.12（macOS）** 错误与节流；空闲无提交。
4. **B29.10** 桥接普查 + `Smooth` 逐变体像素用例；先从矩形/文本/图像/渐变开始，其余
   `ForcesCpu` 整帧回落。
5. **B29.13 Android** JNI 句柄 + 模拟器闭环。
6. **B29.13 OpenHarmony** 编译/链接 + GLES 档接线；无设备则运行 SKIP。
7. **B29.13 Windows** xwin check+link；运行时 SKIP。
8. **B29.14–B29.16** 门控、bench、文档、兼容说明。

每一步都要求：代码 + 测试/脚本实跑输出；未跑到的平台只写 `SKIP(host-limited)`，不写“应该可以”。

### 7.5 完美闭环判定表（最终必须逐格填）

| 平台 | 编译 | 链接 | 真实上屏 | 像素 | 尺寸/生命周期 | 错误 | 拆除 | 结论 |
|---|---|---|---|---|---|---|---|
| macOS（真机） | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | 目标：全 PASS |
| Android（arm64 模拟器） | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | 目标：全 PASS（性能非代表性） |
| OpenHarmony（SDK，无设备） | ⬜ | ⬜ | `SKIP(host-limited)` | ⬜（离屏等价） | ⬜（单测） | ⬜（单测） | ⬜（单测） | 编译/链接闭环 + 运行具名排除 |
| Windows（xwin，无运行时） | ⬜ | ⬜ | `SKIP(host-limited)` | ⬜（离屏等价） | ⬜（单测） | ⬜（单测） | ⬜（单测） | 编译/链接闭环 + 运行具名排除 |
| Linux/Wayland、wasm、iOS | — | — | — | — | — | — | — | 具名排除，后期运行 |

**填表规则**：`SKIP` 不是 `PASS`；离屏等价不能替代真实上屏；模拟器不能替代真机性能。

### 7.6 风险与预先裁定的处理

| 风险 | 事实 | 处理 |
|---|---|---|
| OpenHarmony 只有 GLES 档 | `wgpu-hal 30` Vulkan 无 `OhosNdk` | 强制 GL 档并上报降级；无 GLES3 ⇒ SKIP + 手势 |
| `RenderCommand` 桥接量大 | 约 38 个变体，现有桥只 4 个 | 普查 + `ForcesCpu` 整帧回落；不追求一次全映射 |
| `Smooth` 与 CPU 不可能逐字节 | §1.2 AA 集合 | `Exact` 档保留逐字节；文档写死差异集合 |
| macOS 主线程 | AppKit 约束 | presenter 只在主线程创建/present；调试断言 |
| Android 句柄可被系统替换 | rwh 文档明示 Android native window 可被替换/移除 | 每次 `surfaceCreated/Destroyed` 重建 presenter；不使用过期指针 |
| 合成器不可测 | 无法断言用户屏幕每个像素 | present 成功 + 读回帧 + 截图非空；不承诺像素级合成器验证 |
| 模拟器/软件适配器 | SwiftShader 等 | 性能数字标注非代表性；功能仍须过 |

---
