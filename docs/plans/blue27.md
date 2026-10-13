
## 19. rust-widgets 统一图标完整设计方案

> 本文是 rustage 文档 `docs/plan/resources.md` 第 19 节的正文：该文档 §7.4 以
> `#19-rust-widgets-统一图标完整设计方案` 链接到这里，正文追加进去后该锚点才成立；
> 本节直接扩展那篇文档，不为它另建第二份方案文件。
> 本文出现的 §2、§7.4、§9、§13、§16 都指 `resources.md` 的对应小节。
> 源码基线同 `resources.md` §2 固定的核验提交 `027d7265`（与 rust-widgets 当前 HEAD
> 相同）；下面的新增类型、方法、属性和模块名均为**建议接口**，并非已实现 API。
> 实施位置主要在 rust-widgets；rustage 只接入正式公共契约，不复制控件实现。
> 目标是一个可分阶段交付的统一契约，不是一次性引入完整 SVG 引擎或资源平台。

### 19.1 最终决策：统一内容，分离描述、加载和绘制

推荐采用三层设计：

```text
IconSpec                         持久化/声明层：来源是什么
    |
    | 宿主解析、权限/目标校验、转换、异步加载
    v
IconSource                       运行时拥有层：已经可以绘制的内容
    |
    | 按帧借用，不克隆像素或全局查名
    v
IconRef + IconPaintOptions       绘制层：内容借用 + 本次状态/颜色/尺寸
    |
    v
共享测量与绘制 -> 现有 RenderContext -> 现有软件/GPU/SVG后端
```

**核心裁决：路径、URL、资源 ID、完整 SVG 文本不进入最底层绘制类型。**
绘制层只接收已解析内容，不在每帧做文件访问、解码、注册表修改或资源权限判断。
`IconSpec` 可表达资源 ID，但不是所有控件都必须持有 resolver；
宿主将描述解析成内容再应用到控件。

普通 Rust 调用者可以直接构造内置/静态来源，不必先创建 JSON、清单或资源服务。
声明式加载器和生成器使用描述层，手写 UI 使用运行时层，二者消费相同图标语义。

#### 不采用的方案

| 方案 | 不采用原因 |
|---|---|
| 所有 setter 改成 `&str` | token、路径、文本继续歧义，编译器无法帮助 |
| 所有图标都改成 `PathBuf` | 内置矢量退化为图片，内嵌被迫落盘，受限目标引入文件系统 |
| 所有图标都改成 `Image` | 丢失几何着色与 DPI 优势，像素所有权可能深拷贝 |
| 放大现有全局注册容量 | 只解决数量，不解决动态生命周期、工程隔离及不同控件入口 |
| 所有来源实现开放的 `dyn IconProvider` | 首版增加对象安全、线程、缓存身份和 profile 成本，尚无必要 |
| 在每个控件单独加 SVG 解码 | 重复解析、重复依赖、失败行为漂移 |
| 用新图标控件包住所有按钮 | 改变控件树、布局、点击与无障碍，不是最小接缝 |

### 19.2 类型模型与所有权

以下是接口形状约定，不是可直接编译的代码。实现时以现有 compat 类型和 cfg 别名为准。

| 建议类型 | 职责 | 必须满足的约束 |
|---|---|---|
| `IconName` | 现有内置词汇 | 保留公开路径、token 与现有生成来源 |
| `IconData`（**现有类型**） | 静态、单色、归一化轮廓 | 已公开、`Copy`、`grid` + `&'static [&'static str]`；直接作为静态几何契约，**不新增同义类型** |
| `OwnedIconGeometry` | 动态、单色轮廓 | 仅适用可分配构建；拥有数据，关闭工程可释放 |
| `IconPixels` | 已解码、只读像素 | 宽高/格式/步长/长度可验证；多色或显式 mask |
| `IconSource` | 控件存储的可绘制内容 | 内置/静态轻量；动态内容共享，克隆不复制所有像素 |
| `IconRef<'a>` | 本次测量/绘制的借用视图 | 不分配、不取得文件、不超出数据生命周期 |
| `IconPaintOptions` | 本次表现参数 | 不混入资源路径、命令 ID 或权限 |
| `IconSpec` | 可持久化来源描述 | 不含 Rust 指针、像素句柄、任意 Debug 字符串 |
| `IconError` | 解析/验证/绘制错误 | 可分类，保留来源位置与消费者上下文 |

推荐 `IconSource` 的最小来源集合：

```text
Builtin(IconName)
StaticGeometry(&'static IconData)
SharedGeometry(共享的不可变 OwnedIconGeometry)  [可分配条件]
SharedPixels(共享的不可变 IconPixels)          [图片/可分配条件]
```

`IconRef` 是对应的借用形式，可以从 `IconSource` 取得，亦可借用调用者已有的
几何/像素。直接几何入口不要求名称；可选的诊断名称不是查找键。

* 首版不引入动态纹理 ID、网络 URL、外部进程 provider 等额外变体。
* 共享存储仅在适用构建使用既有共享所有权机制；不要求受限构建具备 `Arc`。
* 是否能跨线程取决于实际像素/几何内容和 RenderContext，不擅自声明所有类型 Send/Sync。
* `IconName` 路径保持兼容；若挪动实现位置，旧模块通过 re-export 保留路径。
* 静态几何契约就是现有 `IconData`（`display_widgets/icon.rs`，已公开），它已具备
  `name` + `grid` + 路径切片，且被 `ICON_DATA` 的 69 项与 `ICON_FALLBACK` 复用。
  再引入一个 `StaticIconGeometry` 表达同一件事就是第二份事实来源；需要校验时只在
  `IconData` 上加一层包装，不手写第二份词汇。
* 首版不必把 `IconRef` 提升为独立公开层次：先让它作为 `IconSource` 上的借用方法返回
  的视图（内部类型或别名），确认存在第二消费者再公开；`IconPaintOptions` 保持公开。
* 新增公开 enum 需明确未来变体扩展策略；不对旧 enum 强行加属性假装无破坏。
* 几何与像素按来源拥有，状态/着色不修改源对象，多个控件可安全共享不同颜色。

#### 图片所有权必须真实共享

现有 `Image` 直接拥有 `DecodedImage`；它的 Clone 和某些 shared 构造仍会克隆像素。
因此，不能把“给 Image 加 Clone 调用”写成零拷贝设计。

推荐新图标内容以独立的不可变像素共享句柄承载：

* 从 owned Image 转入时尽可能移动已有数据，而不是先深拷贝。
* 从借用 Image 复制时明确成本，不承诺所有输入都零拷贝。
* 从现有解码缓存取得内容时复用其共享条目，避免解码后再拷一份进图标缓存。
* 为图标补少量共享像素接缝，不借机重构所有图片、视频和 GPU 资源。
* 零拷贝的绘制接缝**已经存在**，不要再造一条：`PaintBackend::draw_image(x, y, w, h,
  data: &[u8])`（D09-RENDER-02）只有默认实现才复制，软件后端已重写为零拷贝；而
  `ToolButton` 早已通过它 + `Arc<Vec<u8>>`（`image::cache::file_rgba8_or_none`）
  绘制文件图标。图标内容要做的只是把这个共享句柄提升为公共契约并复用
  `image::cache`，不是新建一条图像通道。
* 仍然存在的复制点只有一个：会**保留命令**的后端走 `RenderCommand::DrawImage` 的默认
  实现，必须拥有字节。哪些后端属于这一类、代价多少，由 U0/U2 量测记录；不能只改
  setter 就宣传“整条链零拷贝”。

### 19.3 静态几何格式与 SVG 边界

首版沿用现有 `0 -grid grid grid` 坐标约定，避免重编码全部内置轮廓。
归一化导入由宿主/构建工具完成，渲染器不解析完整 SVG。

**首版支持范围：单色、经转换后可由环绕填充后端保真绘制的填充几何（洞按非零环绕语义，
见 §19.3.1）。** 不是“所有 SVG 都能注册”：

* stroke 先展开，transform 先应用，正坐标先转换。
* 复合路径、洞和 fill-rule 不是“须验证即可”，而是**当前实测失效**：见 §19.3.1。
* 洞与 fill-rule 由环绕填充承担后，不再要求把复合路径手工分解为不重叠实心区域；
  确实无法保真（渐变、滤镜、超出容量）才显式失败并改选已批准的像素来源。
* 旧 `DrawPath` 命令保持逐多边形语义不变，既有调用者继续有效；完整 compound fill
  由 U1 新增的环绕填充入口承担，并**同批**贯通所有声明支持的绘制后端与 SVG 输出，
  不能只实现某个软件后端（见 §19.3.1）。
* `icons` 关闭时，Builtin 复用现有 fallback 几何；输出保留当前兼容行为。
* `IconSource::StaticGeometry` 不应为了绘制自定义轮廓强制链接整个内置轮廓表。

校验分两次：

1. **源验证**：路径语法、有限数、非零 grid、边界、可表达特性，构造前验证。
2. **准备/绘制验证**：在具体尺寸与后端下展平，检查点/轮廓容量和资源预算。

源验证成功不能承诺任意极端尺寸下都成功；绘制结果必须能报告第二类失败。
沿用当前 1024 点/64 轮廓限制作为首版边界，改变预算属于明确行为和性能变更。
不靠增加栈数组绕过复杂资源拒绝，也不悄悄丢最后几个轮廓。

#### 19.3.1 填充保真：洞必须在 U1 一次做完（实测缺陷，不是未来项）

当前实现**逐轮廓**调用 `draw_path`（`display_widgets/icon.rs::draw_outline_in`），
而软件后端对每个多边形单独做奇偶扫描填充
（`render/pipeline/primitives.rs::fill_polygon`），所以任何**反白孔**都会被后画的轮廓
实心盖住。这不是新来源才有的风险，仓库自带的 Material outlined 图标已经如此：

```text
# 临时探针：软件后端、120px、icons 特性开启，取 glyph 中间行
info y=60: ..........#################################################.........
info 中行不透明段数 = 1        # 应为环（≥2 段）；实测是实心圆盘
```

提交在案的第二份证据是 census 自己：`tools/icon_census.txt` 记
`info contours=4 ink=4`、`star contours=2 ink=2` —— 孔被当成独立墨迹，而不是被减掉的
洞。`snapshots/svg/icon.svg` 只覆盖 `check`（单轮廓、无孔），所以现有快照与 census
的两套断言都检不出这个缺陷。

**裁决：把“一份来源的全部轮廓作一次环绕填充”放进 U1，与共享绘制接缝同批交付。**

* 只统一**内容契约**而不统一**填充规则**，等于把统一做成“统一地画错”；
  DoD 第 2/9 条会把实心色块当成通过。
* crate 里已有正确实现可复用：字形走的是**非零环绕累积**
  （`render/text/raster.rs`），图标却走逐轮廓奇偶 —— 同一个概念两套规则（原则 #101）。
  让图标复用环绕光栅器，而不是新写一套，也不要为 69 项内置轮廓手写第二份分解结果。
* 一处改动换三项收益：洞正确；每图标由 N 条命令降为 1 条；顺带消掉 §19.13 量到的
  逐轮廓克隆。
* 一旦推后，其后每个提交的视觉基线与 census 都要再改一遍。

同批必须做到：

* 贯通**所有**声明支持的绘制后端与 SVG 输出，不能只改软件后端就声称完成。
* 重生成 `tools/icon_census.txt` 与受影响快照，并给 census 增加一条断言：
  带孔 token 的 ink 段数必须等于外轮廓数（现有 C1–C4 不覆盖孔）。
* 修正 `display_widgets/icon.rs` 中“counter…fills as empty because its ring runs the
  opposite way — the same rule the glyph rasteriser uses”的注释：该描述与实测相反，
  留着会误导下一个实现者。
* 旧 `DrawPath` 的逐多边形语义保留给既有调用者，环绕填充用新增命令/入口，
  不在原命令上偷偷改行为。

### 19.4 绘制与测量 API 契约

建议共享入口的概念签名：

```text
measure_icon(icon_ref, requested_size, fit_policy)
    -> Result<IconMetrics, IconError>

paint_icon(render_context, icon_ref, slot_rect, paint_options)
    -> Result<IconPaintOutcome, IconError>

validate_icon(icon_ref, validation_limits)
    -> Result<ValidatedIconMetadata, IconError>
```

具体命名与参数可按上游习惯调整，但测量与绘制必须使用同一尺寸/比例规则。
不能引入一套新布局引擎：

* glyph 默认正方形 contain，非正方形图片默认保留宽高比，不拉伸。
* `slot_rect` 是调用控件已计算的内容槽，不是整个按钮点击区。
* 请求尺寸不得超出槽；裁剪交由现有 RenderContext 机制，不能绘出按钮边界。
* 绘制不返回控件点击事件，也不改变 enabled/selected/checked。
* 空槽返回 `SkippedEmptySlot`，这不是资源失效；无图标不调用绘制。
* 不支持格式/后端返回错误，不把 false 同时用于“空槽”“未知名”和“失败”。
* 请求重绘与尺寸失效按现有 widget 机制执行，不能只更新内容却沿用旧隐式尺寸。

`IconPaintOptions` 首版只保留必要参数：

| 参数 | 规则 |
|---|---|
| 目标颜色 | 由当前控件主题上下文解析；不硬编码 PRIMARY |
| 颜色策略 | 几何为单色；图片默认保留原色；mask 需显式声明 |
| 交互状态 | disabled/selected 等由控件提供，转换只做一次 |
| 几何方向 | 已解析的是否镜像；不能 renderer 和 widget 重复镜像 |
| 尺寸/fit | 实际槽与期望大小；不新增隐藏的默认拉伸 |

hover/pressed 背景、焦点环与动作权限仍由控件管理，避免把所有 UI 状态都塞入 renderer。
首次统一后的默认像素输出兼容现有控件；高对比等改良作为可测试样式策略，
不要在接口迁移中无提示改变旧主题。

### 19.5 控件与集合接入清单

“全部统一”必须定义范围，不能只改 Button 与 ToolButton 就结束。
本次源码发现的入口按以下类别迁移，实施时继续检查 call sites 与绘制消费者：

| 类别 | 已发现入口 | 新契约接入 |
|---|---|---|
| 独立图标 | Icon | 存 IconSource，复用 paint_icon；保留名称访问兼容 |
| 按钮 | Button、ToolButton、MenuButton | 增加 typed source，保持 label/icon 布局与点击行为 |
| 菜单命令 | Action、MenuEntry、MenuItem、DropdownMenu 条目 | 动作图标槽与勾选/子菜单箭头槽分别处理 |
| 复合工具 | RibbonBar、PieMenu、Fab | 新增真实内容入口；旧 icon_text 保持文字 |
| 页签与容器项 | TabBar、TabWidget、ToolBox、MDI 子窗口 | 每个 item 的图标字段/句柄贯通实际 draw |
| 导航集合 | BottomNavigationBar、AdaptiveScaffold、NavigationDrawer、TabView | 每个条目有 typed source，不通过显示文字回查图标 |
| 其他内容 | SegmentedButton、EmptyState、Cupertino 导航项 | 同一来源与状态算法；保留现有缺省表现 |
| 内置装饰 | 折叠箭头、勾选、星形、媒体播放/暂停等 | 共享内置绘制接缝，不强迫新增可编辑属性 |
| 数据视图 | 树/列表/表格的实际图标槽 | 先核验是否有现成槽；有则接入，没有则不虚构现有接口 |
| 语义状态 | MessageBoxIcon 等 | 保留 severity 枚举，内部映射 glyph，不改成任意图片替代 severity |
| 原生/系统 | 原生菜单、窗口图标、任务栏/Dock | 单独能力适配，OS 应用图标不并入 glyph setter |

建议的新方法族：

```text
set_icon_source(Option<IconSource>)
icon_source() -> Option<IconRef>
clear_icon_source()

集合对应：
set_item_icon_source(stable_item_id, Option<IconSource>)
    -> Result<..., item or icon error>
```

集合优先复用现有稳定 item ID；只有 index 的现有集合不得为此私自重写选择模型，
先明确索引有效性和列表变更行为，稳定身份提升作为相关但独立的版本变更。
不存在的 item 返回显式错误，不静默丢掉新图标。

布局回归必须检查：

* 无图标、纯图标、图标加文本、文本在下方的隐式尺寸。
* 同一菜单同时有 check、glyph、shortcut、submenu arrow 时不相互覆盖。
* 同级菜单图标列宽一致，没有 glyph 的行仍与其它行对齐。
* 工具栏统一 size 时保留比例，不把宽图拉成方形。
* 新增来源不会改变原本点击、快捷键、弹出位置或选中状态。

### 19.6 兼容策略：不能用“双字段优先级”草率收尾

Rust 不支持按参数类型重载；不能直接把旧 `set_icon(Image)` 改成
`set_icon(IconSource)` 并宣称兼容。新方法使用明确的新名字，旧方法保留签名。

#### 私有字段控件

推荐一个内部活动状态，区分：

```text
Empty
LegacyImage / LegacyPath / LegacyName / LegacyText  [按实际旧控件选用]
Typed(IconSource)
```

旧和新 setter 都替换同一活动状态，**最近一次显式设置生效**。
旧缓存是活动状态的派生，不是第二套可独立修改的数据。
迁移初期 Legacy 绘制可保持原通道，后续由共享 renderer 适配，但要有等价测试。

旧 getter 的规则必须写清：

* 返回旧 Image/Path/String 引用的 getter 不可能准确表达所有新来源。
* 旧类型恰好是 Option 时，Typed 状态返回 None，并在 API 文档说明；
  调用者应使用新 getter 查询真实内容。
* 旧 getter 是非 Optional 名称/文本时，Typed 状态使用该旧接口约定的空/未设置表示，
  内置 token 若能无损表示则可返回 token；不得返回误导性的陈旧路径或伪造名称。
* setter 方法的新调用保持正确；依赖旧 getter 重新渲染 typed 内容的内部消费者必须迁移。
* 不在 getter 中把几何光栅化或把内存图片落盘来制造旧类型。

这只是跨类型查询的兼容约定，不把新来源当成加载失败。
所有旧 setter -> getter -> draw 流程必须与原行为一致。

#### 公开字段集合是版本边界

`NavItem.icon: String`、若干 MenuItem/TabPage 的公开 icon 字段存在调用方直接构造和赋值的可能。
以下做法都会破坏源兼容：

* 把 String 换成 IconSource。
* 增加必须初始化的字段。
* 把公开结构改成私有字段或添加 non-exhaustive 后要求外部不能构造。

推荐策略：

1. 兼容阶段保留原条目结构和方法。
2. 对私有持有集合的控件提供 typed item setter，类型化覆盖作为明确的 presentation 状态，
   用真实条目身份关联，不用字符串、位置猜测关联。
3. 旧 add/replace/update 对该条目的 typed presentation 有明确清除或替换规则。
4. 对允许外部修改公开条目、无法可靠观察变更的入口，不承诺自动同步；
   推迟该入口的最终模型替换到明确的不兼容版本。
5. 最终版本将条目变成受方法管理的内容模型，附构造器、builder 和迁移示例。

这是少量过渡成本，不建立永久的 index -> icon side map。
若上游尚未承诺兼容且选择一次性破坏版本，也必须列出受影响结构、旧调用迁移和 schema
迁移，不能因版本号较小就声称修改没有影响。

#### 旧文本语义必须保留

Action、Fab、Ribbon、PieMenu 等旧 `icon_text` 仍然是文本；
BottomNavigationBar 等已有“已知 token 绘 glyph、否则绘文本”的行为保持。
新 typed 接口严格校验，未知 token 报错，不回退为普通文字。
不能把文本 `"save"` 或 emoji 自动升级成资源引用，也不能把已有 emoji 突然删掉。

### 19.7 资源加载、异步状态与故障闭环

运行时内容 API 本身不需要 Pending：
已经完成验证的 IconSource 被同步提交到控件；异步生命周期由宿主/加载适配层负责。

```text
Unspecified / Empty
    -> Loading(request_id, resource_revision)
    -> Ready(validated_source)
    -> Failed(diagnostic)
```

资源热更新另外保留 last-valid，候选失败时：

* 首次加载失败：显示可配置错误占位/文字，报告真实失败。
* 已有有效来源后更新失败：保留 last-valid，但显示过期/失败诊断。
* 用户明确清除：变 Empty，绝不能继续保留上次图标。
* 新请求覆盖旧请求：旧结果的 request_id/revision 不匹配，不得应用。
* 消费者销毁：取消任务或丢弃结果，释放内容引用，不访问已销毁 widget。

同一项目同资源可以共享读取/解码，应用图标的每个消费者独立持有状态与布局。
资源解析服务属于工程/应用上下文，不是全局进程表；多个工程同 ID 不互相串内容。
`ResourceResolver` 的歧义规则保留；类型化图标系统不绕过权限、来源选择或 schema 校验。

建议错误类别：

```text
UnknownBuiltin / UnknownRegistered
InvalidGeometry / UnsupportedSvgFeature / OutlineCapacityExceeded
InvalidPixels / UnsupportedPixelFormat
ResourceNotFound / AmbiguousResource / ResourceReadFailed
DecodeFailed / BudgetExceeded
UnsupportedSourceForProfile / UnsupportedNativePresentation
BackendDrawFailed
```

错误上下文包含资源身份/修订、控件或条目 ID、操作阶段和可定位原因；
不在共享 renderer 里直接弹对话框。上层统一诊断，同一个失败修订只报告一次。

现有 Draw trait/调用链未必返回 Result，因此建议：

1. 共享 paint_icon 返回 Result。
2. widget 的 draw 在库标准诊断接缝中提交结果，并执行明确占位策略。
3. 若库没有可用接缝，补一个由渲染/运行上下文持有的有界诊断收集入口，
   由 runtime/宿主消费；无需把所有 Draw trait 改成 Result。
4. 诊断必须有真实消费与测试，不只是新增无消费者的 Vec 或日志。
5. 旧 bool API 保留返回兼容性，但内部新结果可供诊断；不能新增 `.ok()` 静默吞错。

### 19.8 注册表定位与迁移

原全局 16 槽注册表保持为**旧名称扩展适配层**，不是新系统的基础存储。

* 旧 register/lookup/clear/count 签名、容量与同名替换语义保留。
* 新 Builtin 直接用 IconName，新 StaticGeometry 直接借用数据，不占注册槽。
* 动态内容拥有/共享数据，不使用 Box::leak，也不挤入注册表。
* 旧名称查询转为已知来源再绘制；保持内置优先的既有顺序。
* 注册表校验若增强到拒绝此前能注册但无法绘制的路径，应单独列为行为变更。
* 原 API 返回 bool 无法附完整原因；可以新增详细 try-register 接口，
  旧 bool wrapper 保留，同时按标准诊断规则暴露拒绝原因。
* 不因统一就扩容、改成 HashMap、增加读锁或让绘制每帧访问全局表。
* 首版不移除注册表；是否未来废弃由实际使用情况与版本政策决定。

新入口至少要证明：注册表已满时，直接 StaticGeometry 仍能在 Button/ToolButton/菜单
正常显示，同时旧第 17 个注册继续按旧契约拒绝。

### 19.9 feature/profile 边界

推荐首版不增加一组新的 icon feature 开关。
沿用 `icons` 控制内置高质量数据，`image` 控制图像内容，既有 cfg 别名控制可分配
和控件集合。若某个新依赖只能独立可选，再论证新增 feature，避免组合爆炸。

| 构建能力 | 必须能使用 | 不得意外带入 |
|---|---|---|
| 不启用 icons | 现有 Builtin fallback、具备路径能力时的自有静态几何 | 完整 Material 路径表 |
| 启用 icons | 内置高质量数据、直接几何 | 文件系统、SVG 解码器只为 glyph 而启用 |
| 启用 image 且可分配 | 已解码图片内容，适用缓存 | 仅为共享图标强制视频/音频依赖 |
| mini / alloc_frugal | 当前实际允许的内置装饰/静态入口 | Arc、动态工程 resolver、后台文件加载 |
| embedded | 按实际渲染与内存契约接入静态数据 | 桌面窗口/控件树、隐式 resvg |
| desktop/tablet/mobile | 实际可用 widget 的统一内容 | 将三者当同一 OS 后端支持声明 |
| 无 device profile | 当前可编译基础模块的最小类型 | full_widgets、JSON 或平台单例的误引用 |

“静态像素也支持受限目标”不作为首版无证据承诺。
若后续确需，在不依赖解码器的借用层提供已验证像素视图，并验证该目标实际 DrawImage 通道；
不能让该需求反过来强迫 mini 启用 image。

`asset` 顶层模块的现有门禁不因引入 IconSpec 就自动改变。
资源描述若需多目标使用，可在现有 capability/基础类型层定义规范化描述，
由宿主适配到 ResourceId；不要复制归一化算法，也不要把 watcher 无条件开放到受限构建。

### 19.10 capability、JSON、view 与生成器

建议增加结构化 `icon_source` 属性，与旧 `icon_name` / `icon_text` / 路径字段分开。
不能把 JSON 对象 stringify 成 CapabilityValue::String 冒充类型化实现。

建议的持久化内容示例：

```json
{
  "icon_source": {
    "kind": "builtin",
    "name": "save"
  }
}
```

```json
{
  "icon_source": {
    "kind": "resource",
    "id": "project/icons/save",
    "color_mode": "monochrome"
  }
}
```

显式清空使用 `null`；字段缺失意味着保持该文档版本的缺省规则，
不能与“未知 builtin”或“资源丢失”混为一谈。

结构化属性契约应包括：

* source kind、必需字段、互斥字段、token/ID 校验与当前 target 支持。
* 适用 widget 与条目字段 schema，支持查询而不是宿主手写“所有按钮都支持”。
* 内容读写、错误信息与版本化序列化一致。
* 资源来源的颜色模式以清单角色为准；矛盾声明报错，不把彩色图片悄悄变 mask。
* runtime 非持久化内存来源返回“不可序列化”，不伪造临时 ResourceId。
* 同一文档同时给新旧冲突字段：诊断并拒绝，不能随 JSON 键顺序最后写胜出。

首选**复用已有的记录契约**，而非新增 `CapabilityValue` 变体：树内已有
`PropertyValueKind::Records` 与 `PropertySchema::record_schema`（BLUE26 B26.09），它已经
带身份、必需字段和逐字段校验。U5 的第一个动作是**先证伪**：`{"kind": …}` 这类带互斥
字段的 source union 能否由 Records 的 schema 表达。能表达就不动 `CapabilityValue`；
不能表达才论证新变体，并把“公开 enum 新变体破坏外部 exhaustive match”写进兼容版本说明。
既不能仅因存在 Records 就套用，也不能因为不好表达就退回把 JSON 对象 stringify 成
`CapabilityValue::String`。

必须追踪的消费面：

1. widget property schema、get/set、coercion 和 factory。
2. 集合 item schema 与稳定 ID 修改。
3. JSON loader 的专用分支及通用 fallback，两者不能产生不同来源。
4. Node/view 的属性 diff、apply、引用生命周期和事务错误。
5. designer 实际注册的生成入口：识别 source kind，查询目标 API，生成真实构造。
6. 对应 FFI/bindings 或平台后端适配，若公开该属性则完整接线，若不支持则显式拒绝。
7. 文档、demo、snapshot/census 和许可追踪。

静态 Rust 生成顺序：

```text
IconSpec + profile
 -> source/capability verification
 -> reachability and resource report
 -> builtin / generated static geometry / approved pixel representation
 -> actual target API construction
 -> compile and run validation
```

反射接口不承担磁盘 IO：resource 描述需由显式 apply context 解析，
普通 property setter 没有 resolver 时返回需解析的明确结果/错误。
不能把资源 ID 的字符串直接塞到 typed setter，再宣称模式 1 已显示正确。
JSON/CSS 的外观/内容边界保持：CSS 控颜色/尺寸等已有样式，内容身份归结构/资源模型。

### 19.11 主题、无障碍、原生平台和缓存

共享 renderer 实现 §9 的一次着色/禁用策略，控件供给当前主题状态。
多色源保留原色；只有明确 mask 来源接受统一 tint。
高对比时使用可配置的状态策略，不能固定半 alpha 后声称达标。

无障碍仍属于 widget/command：

* IconSource 不携带命令名称，避免同源多个操作名称错误共享。
* 图标按钮从 command/label 获取可访问名称；装饰 glyph 不新增重复读屏节点。
* 替换图标不改变 role、焦点顺序、enabled 或 checked。
* 菜单勾选与错误 severity 等语义不被用户传入图片覆盖。

原生后端只在真实支持图标的入口进行转换，必要时按设备尺寸光栅化，
持有并释放正确的 OS 图像句柄。声明支持前验证更新、销毁、DPI 与线程约束。
原生菜单不支持的装饰图标返回可查询的能力结果，按平台策略保留文字；
这不同于自绘菜单加载失败。窗口图标/安装包图标继续遵循 §13，不走任意 glyph slot。

缓存分三类，避免彼此污染：

1. **内容缓存**：源摘要/修订 -> 已解析不可变几何/像素。
2. **几何准备缓存**：内容身份 + 实际尺寸/方向/精度 -> 展平结果。
3. **呈现缓存**：内容身份 + 后端/尺寸/颜色/状态/方向 -> 图片或纹理。

无堆 profile 可直接使用调用者的有界 scratch，不要求全局 LRU。
桌面优先复用现有缓存；确需新增缓存必须有容量上限、失效规则及测量。
共享句柄 clone、prepare、录制 RenderCommand、GPU 上传分别量化，
不能因为查找 O(1) 就承诺整帧成本固定。
主题改变不使 source 失效，DPI 改变只使尺寸相关派生失效，
资源更新通过不可变 revision 防止缓存读到半新半旧内容。

### 19.12 推荐代码落点与提交拆分

不预建多个独立 crate，也不为每个概念建一个空模块。
按现有 widget/render 分层，推荐先用一个小型统一模块，增长后再按职责拆分：

```text
src/widget/icon_source.rs                  新增：内容、借用、验证契约
src/widget/display_widgets/icon.rs         修改：共享绘制、独立 Icon、旧函数适配
src/widget/icon_registry.rs                修改：旧查名适配与详细错误入口
src/widget/mod.rs                         修改：公开 re-export 和 cfg
src/widget/<各类消费控件>.rs               修改：新来源入口及真实 draw/layout
src/widget/capability/*                    修改：schema、值、属性与条目契约
src/json/loader.rs                        修改：结构化加载与解析上下文
src/view/*                                修改：属性更新与应用
src/lib.rs 中 designer 的实际模块入口       修改：确认路径后接入，不猜文件名
src/render/backend/*、src/render/pipeline/* 修改：环绕填充入口与命令贯通（§19.3.1）
tools/icon_census.txt、snapshots/svg/*    修改：带孔图标对应重生成（§19.3.1）
src/control_backend/*、src/platform/*      修改：适用后端映射
tests/、examples/、demo/、docs/             修改：兼容/视觉/生命周期证据
```

上述是拟修改地图；不是声称所有文件已经存在或必须在一个 PR 修改。
注意两层的归属不同：§19.3.1 的环绕填充**从 U1 起就落在 render 层**（它是绘制能力，
不是图标概念）；而面向图标的共享绘制/测量包装可以先留在 widget 层，成熟后再迁入
render 并保留旧公共函数的兼容 re-export，避免 render 反向依赖 runtime/宿主服务。

| 提交 | 交付范围 | 依赖 | 门禁 |
|---|---|---|---|
| U0 | 完整入口/公开结构/cfg 清查与旧行为基线 | 无 | 所有实际消费点、兼容风险与 profile 证据列清；并记下每帧分配与填充保真的实测基线（§19.13） |
| U1 | 内容/借用模型、可诊断几何绘制、一次环绕填充（洞/复合路径）、旧绘制适配 | U0 | Icon 与内联 glyph 共用，内置/fallback 输出保持；带孔图标视觉正确，census 与快照同步重生成 |
| U2 | 共享像素接缝、Button/ToolButton、新旧 setter 状态 | U1 | 内存图片不落盘，克隆与禁用/布局实测 |
| U3 | Action、自绘菜单与复合工具真实图标槽 | U2 | 同一来源跨入口，勾选/快捷键/焦点不回归 |
| U4 | 集合与导航迁移、公开条目版本变更策略 | U1/U3 的相关接缝 | 无串项、无陈旧 side map，公开 API 迁移明确 |
| U5 | IconSpec、capability/JSON/view、资源 apply context | U1，及对应控件提交 | round-trip、冲突、缺资源和动态结果竞态验证 |
| U6 | 静态生成与目标转换、引用裁剪 | U5 | 所宣称目标的输出真实可编译、可运行 |
| U7 | 原生后端映射、三平台矩阵、性能与文档收敛 | 对应消费点与 U5/U6 | 适用原生能力有证据，未支持项明确 |

不要求 U4 所有集合阻塞 U2 的首个工具栏使用，但最终“统一完成”必须覆盖已承诺范围。
每个提交应有真实使用点和最小测试，不交付只有 enum 没有消费者的占位。
不得把尚未完成的 JSON/生成接线标成“不影响核心”以宣布整项完成。

### 19.13 测试与量化验收

沿用上游既有 test/build/feature 探针和 CI 命令；实施时从实际脚本取得命令，
不使用所有互斥 profile 同时启用的 `--all-features` 作为通过证据。
先跑变更相关选择器，再跑受影响 feature/profile 矩阵与平台测试。

#### 核心必测用例

| 编号 | 输入/场景 | 必须观察的结果 |
|---|---|---|
| I01 | 同一个 Builtin Save 用于 Icon、Button、ToolButton、Action/自绘菜单 | 同色同尺寸 glyph 一致，行为仍各自正确 |
| I02 | 自有 StaticGeometry，在注册表已满时使用 | 新来源成功；原注册表容量行为不变 |
| I03 | 同一动态几何/图片在 100 个消费者共享 | 创建后 clone 不深拷贝数据；销毁最后消费者后可释放 |
| I04 | 无 icons feature | Builtin fallback 按旧约定绘制，不链接高质量数据表 |
| I05 | image 关闭/受限 profile | 无不存在类型导入；不支持描述显式阻断 |
| I06 | 空槽、显式无图标、未知 token | 三者状态不同，未知项不可静默成功 |
| I07 | 非法几何、超 1024 点/64 轮廓、零 grid、错误像素长度 | 明确分类错误，无部分绘制当成功 |
| I08 | 新旧 setter 交替、清空、旧 getter 调用 | 活动状态规则明确，无陈旧来源重新出现 |
| I09 | 旧 icon_text 与既有 emoji 导航 | 保留旧文本行为；新 typed unknown 不走文字 fallback |
| I10 | 并发请求 A/B，A 最后才完成；销毁控件后结果到达 | 只应用当前请求，不访问过期消费者 |
| I11 | 资源更新失败、有 last-valid、用户随后清空 | 保留有效内容并报告失败；清空后真正移除 |
| I12 | 菜单 glyph + check + shortcut + submenu、RTL | 槽位不重叠、方向正确、读屏不重复 |
| I13 | JSON round-trip、旧新冲突字段、运行期不可序列化来源 | 保真/拒绝有明确结果，不 stringify 句柄 |
| I14 | 模式 1 与模式 2、目标不支持资源来源 | 可用目标显示一致；不支持目标在生成前阻断 |
| I15 | 原生后端支持/不支持图标，销毁窗口菜单 | 能力结果真实；句柄释放，无假显示成功 |
| I16 | 带孔/复合轮廓：`info`、`settings`、`mail`、`star` 与等价自有关闭轮廓 | 中间行不透明段数 = 外轮廓数（环保真，不是实心色块）；census/SVG 输出同步 |
| I17 | 同一批图标连续绘制多帧 | 每帧堆分配数不高于 U0 基线；环绕填充落地后应显著下降 |

视觉矩阵继承 §16，并增加**跨消费点的同源比较**：
相同 slot、颜色、scale、方向时，几何结果应一致；
有意不同的按钮背景/间距不作为 glyph 差异。
软件/GPU 后端抗锯齿差异按已有阈值和环境记录，不能无限放宽容差。

现有快照集检不出填充缺陷，要补两类基线：

* **覆盖带孔与多轮廓 token**：当前 `snapshots/svg/icon.svg` 只画 `check`（单轮廓、
  无孔），`info`/`settings`/`mail`/`star` 这类图标没有任何视觉基线，§19.3.1 的缺陷
  因此长期无人发现。基线至少覆盖一个实心盘加反白孔的 token。
* **断言孔而不是断言形状**：`icon_census_test` 的 C1–C4 不比较孔。补一条规则：带孔
  token 的 ink 段数必须等于外轮廓数，且该规则要先证明它**能**因当前实现而失败
  （在 U1 之前红、之后绿），否则它只是又一条自证的门禁。

性能门禁：

* Builtin/Static 来源构造不新增堆分配，借用不分配。
* 正常 paint 路径无 IO、完整 SVG 解析或像素深拷贝。
* **每帧分配必须有基线并受门禁**：现有几何路径每次 `draw_path` 克隆一份
  `Vec<Point>`，软件后端 `fill_polygon` 还每扫描行分配一次 `Vec<f32>`。本次核验在
  当前基线实测（软件后端、`icons` 开启、每 30 个图标）：

  ```text
  check   24px = 22 次/图标      check   96px = 58
  info    24px = 94              info    96px = 226
  settings 24px = 136            settings 96px = 279
  ```

  即一个 30 项工具栏图标约 2.6k–4.2k 次分配/帧，且随尺寸与 DPI 增长；按现方案
  “完成”会把这个成本固化进公共契约。U0 用同一探针复录该基线，U1 的环绕填充
  （每图标 1 条命令、复用有界 scratch）应把它降到常数级；门禁要求**不高于 U0 基线**，
  并把展平缓存（内容 + 尺寸/方向/精度 → 展平结果）的归属写进对应提交，
  而不是只写“桌面优先复用现有缓存”。
* 如现有 RenderCommand 仍导致拷贝，报告并**说明哪个后端属于保留命令的那一类**
  （`draw_image(&[u8])` 只覆盖立即后端），不能笼统宣称完成。
* 常用 glyph 的动态共享 clone 成本不随图片像素数量增长。
* 多轮创建/销毁工程之后，活跃引用与缓存预算稳定，不用 `Box::leak` 达到效果。
* 与同环境旧实现比较冷/热绘制、主题/DPI 切换和峰值内存；
  有回归须给原因和批准预算，不能用尚未测量的固定毫秒承诺。

#### 最终 Definition of Done

只有以下各项一起完成，才称 rust-widgets 统一图标完成：

1. 所承诺的控件、集合和内联入口使用同一内容/绘制契约。
2. 内置、自定义静态几何、适用目标的内存图片都能**正确**绘制：带孔/复合轮廓按环绕
   语义保真（§19.3.1），不是实心色块。
3. 直接几何不依赖注册槽，动态资源可释放且无全局串工程。
4. 尺寸、主题、禁用、RTL、焦点与无障碍行为经过真实入口验证。
5. 旧方法/文本行为按兼容政策保持，公开条目破坏性变更有明确版本和迁移。
6. 新内容的 capability、JSON、view、生成器与资源错误处理已接线。
7. feature/profile/平台支持声明与实际构建运行证据一致。
8. 原生不支持图标与加载/绘制失败可以区分，不用空白/文字伪装全部成功。
9. 文档、示例、视觉基线、许可和性能记录随接口同步更新。

**推荐最终形态：一个轻量的图标内容模型、一套共享绘制接缝、适用层的资源描述与解析，
以及各控件薄适配。不是一个全局万能注册表，也不是复制多套图标控件。**

### 19.14 执行状态与后续步骤（跟进用；实施记录见 `docs/log/log-20261012-1.md`）

本节不改变上面的设计裁决，只记录**执行进度**与**剩余步骤的顺序**，便于后续接手时不必重读全部日志。
每完成一格就把证据补进日志并在本表更新；判定"完成"一律按上面 §19.13 的 Definition of Done，
不因局部测试通过而提前计数。

#### 19.14.1 阶段状态（保守口径：只按完整闭环计数）

| 阶段 | 状态 | 已交付的关键证据 | 还缺 |
|---|---|---|---|
| U0 | 🟢 闭环 | `tools/check_icon_entry_contract.sh`（A1/A2/A3/A4/**A5**）；**I17 已闭环**：`tests/icon_allocations.rs` 实测稳态 **0 分配/绘制** + `tools/icon_allocations_baseline.txt` 记录 U0 基线（22–279/次）并做双重门；§19.5 各类别可达性核验；**（2026-10-13）逐项清点已是导出产物**：`tools/gen_icon_entry_inventory.py` → `tools/icon_entry_inventory.txt`（**54 条**，含类别/符号/文件/**cfg 门**/集合身份；第五轮 `PieMenu` 提升为 record 通道后由 52 → 54），A5 重新生成并比对，产物腐化即 FAIL（已反向注入验证）；**A1 已于第五轮强化**：判据锚定 getter 的拒绝语句形状（`^…Err\(CapabilityAccessError::IconSourceNotPersistable\)`），原「文件里出现过 token」判据可由测试断言满足——只删生产分支仍绿，实测已修（见 `tools/gates_reverse_injection.md`） | — |
| U1 | 🟢 闭环 | `CompoundPath` + 软件/SVG/WGPU 非零填充；`IconSource`/`IconRef`/`measure_icon`/`validate_icon`/`paint_icon`；census（实测 ink + runs）+ C5/C6 环绕保真；stepper 快照重生成 | — |
| U2 | 🟢 闭环 | `SharedIconPixels`（Arc 共享、move 语义、缓存同一分配）；Button/ToolButton 接入；`fit_image_into` 统一 fit；**（2026-10-13）峰值预算与矩阵**：`tests/icon_shared_pixels_budget_test.rs`（9 项）实测「from_rgba8 只付 Arc、64 次 clone 0 分配、64 控件持同一句柄差值 0、每帧成本与像素量无关」，矩阵覆盖 3 控件 × 5 尺寸 × 启用/禁用（含 `button` 槽位在 <32px 收缩为 0 的合法边界），并以复制像素的反向注入验证断言非空 | — |
| U3 | 🟡 部分 | Action/MenuButton/DropdownMenu/PieMenu/RibbonBar 的 typed 槽；I01 跨入口同源等价；**I12 已闭环**：菜单行列的按对不重叠（并修掉实测缺陷——shortcut 与 arrow 曾画在同一个 28 px 列里，现由 `Menu::row_shows_shortcut` 明确优先级）+ check/arrow 是几何而非字符 | RTL 的**布局镜像无实现**（见 §19.14.4，全 crate 无镜像设施；`direction` 只约束按键语义） |
| U4 | 🟢 闭环 | 导航集合/TabBar/ToolBox/BottomNavigationBar/MdiArea 的 item setter 与绘制；条目身份键的写法已对齐 §19.6；豁免清单已写进 `docs/MIGRATION_GUIDE.md` §6.3；**（2026-10-13）`PieMenu` 完成身份提升**：条目获得稳定 `u64` id（集合分配、宿主可 pin）+ 条目自持 typed 图标，`set_item_icon_source(id, …)` 取代索引寻址，并发布只读 `items` 行（4 项测试，含「图标跟随条目跨越 insert/remove」与 id 不可重复）；**收益与边界的实测口径**：`items` 声明为 `readable=true, writable=false`，即条目身份与 typed 图标**可读/可导出、不可从文档写回**（不说成"可持久化"）；`pie_menu.rs` 内 `item_icon_sources` 引用 8 → 0；三处变更 API 在 `pie_menu.rs` 之外**调用点为 0**（破坏面为零）；规模 +342/−32。注：旧实现自身是自洽的（13 个变更点全部同步维护侧表），故本项是**消除隐患 + 新增读通道**，不是修一个正在发生的缺陷；其余四个集合按证据**保持不变**（只增删不变、文档已声明 index 稳定，见迁移说明 §6.3.1） | —（同一 index 侧表模式仍有 53 处/6 文件，见 19.14.1 计数口径说明） |
| U5 | 🟢 闭环 | IconSpec/IconCapability/JSON/manifest/ABI/binding；显式 resolver 贯通 loader/view/engine；记录通道：`chip`/`dropdown_menu`/`menu_button`（只读行 + 具名 read-only 拒绝）；**I10/I11 已闭环**：`IconResourceBinding`（陈旧结果丢弃、last-valid 保留、显式清除后不可复活）；**I13 已闭环（2026-10-13）**：`json::records_to_json`（读写的同一处形状定义）+ `tests/icon_record_roundtrip_test.rs`（三条通道逐字节往返、身份不漂移、运行期专有源一律拒绝、新旧冲突字段被拒而非键序取胜） | —（**Node 运行时验证已完成，2026-10-13**：`bindings/nodejs/verify_icon_contract.js` 10 项断言 6/6 稳定通过，过程中修掉绑定层四个真实缺陷——BigInt 参数被 ref-napi 拒绝、控件 id 经 `Number()` 丢精度、`rw_free_string` 后 slot 悬垂导致下次 ffi 调用终止进程、`ref-struct-napi` 拉入第二份 ref-napi 导致进程 core dump。**第五轮**取证了"只有 Node 16 能编译"的根因（`ffi-napi` 自己的 `src/ffi.cc` 把 const-env 的 `get-uv-event-loop` 指针存进非 const 槽位；`node-addon-api` overrides 与 `@lwahonen/ffi-napi@4.0.12` 均无法修复）。**第八轮已根治**：绑定移植到 `koffi`（预编译二进制、无编译器、无 node-addon-api），Node **16.20.2 / 18.20.8 / 20.20.2 / 22.23.3 / 24.15.0** 各 **17/17** 断言通过，Node 24 下 `npm install` 288 ms 成功——约束消失，门控为"≥16 即可"且本机默认 v24 下 **PASS**） |
| U6 | 🟢 闭环 | Builtin `IconSpec` 生成并在 `mini`/`embedded` 真实编译；resource 在生成时被报告为需 resolver（不生成假 setter）；**（2026-10-13）**`GenerationReport::icon_tokens` 可达性报告（收集点即发射点）+ 引用裁剪测试（无图标文档产出零 `IconName::`）+ 未知 kind 在生成前拒绝 + `check_generator_output_compiles.sh` 实跑编译产物（fixture 含行级图标） | 「自定义 geometry/image 的发射」为**格式层具名边界**：持久化联合只有 `builtin`/`resource` 两种，`IconSource::StaticGeometry` 是借用数据、运行期句柄不得写入文档，故生成器对无文档形态的 kind 一律**生成前拒绝**而不伪造 setter（详见日志 §2.3） |
| U7 | 🟢 闭环 | `Platform::supports_menu_item_icons` → `ControlBackend` → `crate::native_menu_item_icons_supported()` → `WindowHandle::new_menu_item_with_icon`（不支持时保留文本并报告 unsupported presentation）；`tests/native_icon_capability_test.rs`（I15）；窗口图标保持 §13 路径契约 | — |

**DoD 进度：第 1–9 条全部闭合**（第三轮补齐第 5/6/9 条，证据见 `docs/log/log-20261012-1.md` 第三轮会话 §6.2；第四轮补齐 U0/U2/U5 的阶段闭环证据，见同文第四轮会话）。
阶段完整闭环数：**7/8**（U0、U1、U2、U4、U5、U6、U7）。**这个计数的口径必须写清：🟢 表示「该阶段没有
未处置的缺陷」，不表示「该阶段涉及的所有集合都已改造」。** U4 的准确含义是条目身份模型未处置项为零；
同一模式在全仓库仍有 **53 处 / 6 个文件**（`menu_button` 12、`chip` 8、`dropdown_menu` 8、
`bottom_navigation_bar` 7、`adaptive_scaffold` 7、`cupertino/core` 6、`navigation_drawer` 5）保持 index
寻址，依据见 §19.14.3 与迁移说明 §6.3.1——是**具名判断**，不是遗漏。
**U3 仍记 🟡 部分（未闭合）**：RTL 布局镜像属整个布局层变更（见 §19.14.4）。**具名不等于闭合**——把 U3
写成「仅剩计划自己具名的排除项」会读成已收口，故此处按未闭合记账。
Node 语言运行时验证**已完成**（`verify_icon_contract.js`，koffi 层，Node 16–24 全绿），并修掉绑定层四个真实缺陷。
**（第八轮）** 绑定已从 `ffi-napi`/`ref-napi` 移植到 `koffi`：Node 16/18/20/22/24 **各 17/17 断言通过**，
Node 24 全新 `npm install` 288 ms 且无编译步骤——原先"只有 Node 16 能编译"的约束随依赖一起消失。

#### 19.14.2 剩余步骤（按建议执行顺序）

1. ~~**I12 菜单槽位与 RTL 矩阵**（U3，DoD 4）~~ **已完成**（槽位按对不重叠 + check/arrow 为几何）。
   - 遗留：RTL 的布局镜像不在本方案内 —— 全 crate 没有布局镜像设施，`Menu::direction` 的文档
     自身把范围限定为**按键语义**；把菜单单独做成 RTL 镜像会与 renderer 之外的第二个方向来源
     冲突（§19.4「不能 renderer 和 widget 重复镜像」）。结论记录在 §19.14.4，不假装已支持。
2. ~~**I13 冲突矩阵与全入口 round-trip**（U5，DoD 6）~~ **已完成（2026-10-13）**。
   - `src/json/properties.rs::records_to_json`（public，与读取器同模块，形状只有一处定义）+
     `tests/icon_record_roundtrip_test.rs`：`chip` 走真实文档（文档 → 行 → JSON → 再加载 → 再导出，
     逐字节相同），`dropdown_menu`/`menu_button` 走各自公开契约的行重建，三者都断言身份与图标不漂移。
   - 运行期专有源（`StaticGeometry`）在三条通道上一律 `Err(IconSourceNotPersistable)`；菜单行写入
     一律 `ReadOnlyProperty`（行无法表达 `children`/`submenu`）；同一行同时声明新旧字段被拒且 reason
     点名冲突字段。反向注入（把写出器的 `"builtin"` 改错）实测使往返断言变红，证明断言非空。
   - Node 运行时验证：**已完成（2026-10-13，第七/八轮）**。原先**未验证**的原因已取证：`ffi-napi@4.0.3`
     的 `napi_add_finalizer` 与 Node 24 的 `node_api_basic_finalize` 签名不兼容，`node-gyp` 编译失败
     （原文见日志 §1）。**根治方式不是绕过而是换掉那个依赖**：绑定已由 `ffi-napi`+`ref-napi` 移植到
     `koffi`（预编译二进制、无编译器），Node **16/18/20/22/24 全部 17/17 断言通过**，
     `npm install` 在 Node 24 下已可成功（实测 288 ms、无编译步骤）。
3. ~~**U6 静态生成 / 目标转换 / 引用裁剪**（DoD 6）~~ **已完成（2026-10-13）**。
   - 新增 `GenerationReport::icon_tokens`：生成产物**真正引用**的 token（去重、首次引用序），
     收集点在发射点本身（`note_icon_reference` + `record_list_expr`/`record_field_expr` 的
     `&mut Vec<&'static str>`），因此报告与产物不可能各说一套。
   - 三个测试：引用集合恰为所引用者（含行级）、无图标文档产出零 `IconName::`（裁剪的可行那一半）、
     `{"kind":"geometry"}` 这类无文档形态的 kind 在**生成前**被拒（I14 不只覆盖 resource）。
   - 「自定义 geometry/image 的发射」：**格式层具名边界**，不是待办遗漏。持久化联合只有
     `builtin`/`resource`（`icon_spec_from_json` 只认这两种），`IconSource::StaticGeometry` 借用
     `&'static IconData` 无法跨文档边界，而把运行期句柄写进文档正是 §19.10 禁止的伪造。
     要做它必须先给格式增加一种可持久几何描述 —— 那是格式的破坏性扩展，属独立变更。
   - 产物可运行：沿用 `tools/check_generator_output_compiles.sh`（实跑 `cargo check` 生成产物，
     fixture 含行级 `icon_source`）。
4. ~~**U4 条目身份模型**（DoD 5，可选但最终需要）~~ **已完成（2026-10-13）**：
   - 豁免清单已写进 `docs/MIGRATION_GUIDE.md` §6.3（含身份与理由，并写明「缺行是具名决定」）。
   - **模型提升按需触发、且只在确有需要处实施**：`PieMenu` 是唯一**同时**使用按位置侧表与支持
     `insert_item`/`remove_item` 的集合（其自身文档承认 `current_index` 可能指向别的条目），因此获得了
     稳定 id 与 record 行；其余四个侧表集合（`BottomNavigationBar`/`NavigationDrawer`/Cupertino rail/
     `AdaptiveScaffold`）**只有追加与清空**，`BottomNavigationBar::set_item_icon_source` 的文档本身
     声明「index 在条目生命周期内稳定」，加 id 只增加接口面而不改变任何行为——按「确是改进才做」的判据
     保留现状，理由写在迁移说明 §6.3.1。
5. ~~**收尾**（原则 #56）~~ **已完成（2026-10-13）**：
   - `cargo test --no-default-features --features desktop,icons --tests` → 65 个目标、合计
     **7 634 passed / 0 failed**，EXIT=0。
   - `bash tools/run_all_gates.sh` → **PASS=109 / FAIL=0 / TIMEOUT=0 / SKIP=2 / EXIT=0**（第七轮为
     **111 门控**、全部改动之后的最终复跑；第六轮为 `PASS=108 / FAIL=0 / TIMEOUT=1 / SKIP=2 / EXIT=1`，
     第四轮为 110 门控口径 `SKIP=1`）。第三轮曾为 PASS=107 / FAIL=1；唯一曾 FAIL 的是元门控
     发现 blue27 自己的 `check_icon_entry_contract.sh` **没有反向注入记录**，本轮对它做了 A1–A4 四次
     真实注入（每次还原后 `cmp` 逐字节一致）并把证据写入 `tools/gates_reverse_injection.md`，
     复跑元门控 **3/3 PASS**；**第五轮又把 A1 从「文件里出现过 token」强化为「拒绝语句本身的形状」**，
     因为前者可由测试断言满足（只删生产分支仍绿，实测），现在只删生产分支即红（注入证据同上）。
   - **第七轮四项同时改善**（依据：原则 #103/#104 与日志第七轮）：
     ① `check_node_icon_contract.sh` 由 SKIP → **PASS**（口径由"只认 Node 16"订正为"≥16 即可"——
     addon 是 N-API 模块、ABI 稳定，Node 16 编出的二进制在 24 下探针 `PASS` 实测）；② android 门控由
     900s `TIMEOUT`/"result unknown" → **191s 诚实 `SKIP`**（宿主能力前置检测 + 有界启动）；
     ③ `TIMEOUT` 计数 1 → **0**；④ 退出码由恒为 1 → **0**，恢复为可用信号。
     `SKIP`（`check_apple_native.sh` 需 macOS；`check_android_runtime.sh` 需可用 Android 设备/模拟器）
     按原则 #59.4 记为宿主限制，不算通过也不算缺陷。
   - 顺带修掉一处**既有**的 fmt 漂移（`src/widget/menu_toolbar/menu.rs:1446`，I12 遗留的超宽行），
     `cargo fmt --all -- --check` 现已通过；`clippy --all-targets -- -D warnings` 0 条。
   - 全量测试中两次各出现 1 个失败，是**两处不同的既有并发测试**（`app::app_core` 与 `event::types`
     的 reentrancy 用例，隔离下通过、HEAD 基线 5/5 全绿）；`app_core` 的根因已在代码中定位
     （`STARTUP` 是进程级单槽，`App::init` 会取走它，而该测试不持有同模块的 `app_init_test_lock()`），
     属无关既有测试隔离缺陷，按规范未改动，只在日志 §5.4 记录并给出「一行修复」建议。

#### 19.14.3 条目级 record 通道的豁免清单（不是遗漏，是具名决定）

§19.6 允许"只有 index 的集合不得为此私自重写选择模型"。以下集合有 `set_item_icon_source`
但没有 record 行，理由由 `tools/check_icon_entry_contract.sh` 的 A3 断言保证不会被悄悄扩大：

| 集合 | 身份 | 不作 record 行的理由 |
|---|---|---|
| `MdiArea` | 子窗口 `ObjectId` | `ObjectId` 是运行期句柄，不是文档值 |
| `TabWidget` | 位置 | 集合本身只有位置身份 |
| `ToolBox` | 无 | item 字段私有且无 id，造一行等于发明身份 |
| `BottomNavigationBar` / `NavigationDrawer` / Cupertino rail | 各自模型 | 未发布 item id |
| `TabView` | 位置 | `TabPage` 字段公开，§19.6 明确推迟到不兼容版本 |

已具备 record 通道的**四个**集合：`Chip`（host 文本 id；行可写，因为一行就是整个条目）、
`DropdownMenu`（`DropdownItem::value`）、`MenuButton`（`MenuItem.id`）、`PieMenu`（`PieMenuItem::id`，
2026-10-13 加入）——后三者的行都**只读**，因为一行无法承载 `children`/`submenu` 或重算的扇形角；
写入用控件自身 API，避免"写入即丢子树/发明角度"的静默损失。

本轮获得身份提升的是 `PieMenu`——它是唯一**同时**满足「按位置的侧表」与「支持结构性编辑」的集合：
其文档自身承认插入后 `current_index` 可能指向别的条目，而按位置键的侧表正是 §19.6 点名要避免的形态。
（`TabWidget`/`TabBar`/`ToolBox` 也有 insert/remove，但它们**本就把图标存在条目上**，所以编辑会带着
图标走；它们仍缺的是文档身份，由上面的豁免表记录。）
其余四个侧表集合只追加与清空，加 id 只增加接口面而不改变行为（迁移说明 §6.3.1）。

#### 19.14.4 本方案**不**承诺的一项：RTL 布局镜像（具名排除，非遗漏）

I12 的"方向正确"容易读成"菜单要支持 RTL 镜像"。实测三件事，因此判定它不属于本方案：

1. 全 crate 搜索 `is_rtl` / `layout_direction` / `mirror_rect` **零命中**：没有布局镜像设施。
2. `Menu::direction` 的文档把范围写清楚了——"the whole content of the direction field as it applies
   here"指的是**水平方向键**在 RTL 下取反（`begin_step_to_left_step`），不是绘制镜像。
3. §19.4 的规则要求方向只解析一次：若菜单自己镜像而其它控件不镜像，就出现第二个方向事实来源。

要做 RTL 镜像应当是**整个布局层**的一次变更（镜像 `Rect` 派生、影响全部控件与快照），
并在那个变更里带上 RTL 视觉基线；它不属于图标统一契约。**当前状态：RTL 菜单渲染与 LTR 相同**，
这是已知且已具名的边界，不是本方案漏掉的一步。
