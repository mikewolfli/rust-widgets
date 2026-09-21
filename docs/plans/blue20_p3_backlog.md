# BLUE20 P3 存量 theme-blind 控件的逐条归因（第 59 轮）

> 本文件是**分析产物**，不是门禁输入。门禁仍以
> `tools/control_color_exemptions.txt`（数据色豁免）与 `tests/control_rendering_census_test.rs`
> 的 `KNOWN_THEME_BLIND`（待修清单）为准。
>
> 它的存在理由：35 个 theme-blind 控件里，哪些是**设计如此**（数据色）、哪些是**真缺陷**、
> 每个真缺陷**具体错在哪一处**，此前只散落在各控件的注释里。归因一次、写下来，
> 修的时候才不会把数据色也"顺手主题化"（规则 #108 明确禁止），也不会漏掉某个分支。

## 判定方法（可复跑）

```bash
# 1. 当前 theme-blind 全集（按 canonical name）
cargo run --no-default-features --features desktop --example control_rendering_census \
  | awk 'NF>0 && $1!="name" && $1!="checked" {p3=$(NF-1); if (p3=="NO") print $1}' | sort

# 2. 与豁免表求交/求差
comm -12 theme_blind.txt <(awk '!/^#/ && NF>0 {print $1}' tools/control_color_exemptions.txt | sort)
comm -23 theme_blind.txt <(awk '!/^#/ && NF>0 {print $1}' tools/control_color_exemptions.txt | sort)
```

## 总数分解（实跑）

| 类 | 数量 | 处置 |
|---|---|---|
| `data` — 数据色，外观无关是**设计如此** | **18** | ❌ 不改。已在豁免表 |
| `chrome` — 自身外观硬编码，**真缺陷** | **17** | ✅ 修 |
| 合计 | **35** | — |

> **`data` 类不改的理由**（规则 #108 ③）：这些控件的「主色」是**内容**——
> 图表系列、K 线涨跌、色板光谱、地图瓦片、视频帧、emoji 字形。
> 把它们主题化会**抹掉信息本身**。它们**另有** chrome（边框/标签），那些是主题化的，
> 但被内容色盖住了不成为"主色"，所以 P3 判据看不见——这正是豁免表要表达的事。

---

## `data` 类（18 个，不改）

| 控件 | 主色是什么内容 | 为什么不能主题化 |
|---|---|---|
| `line_chart` / `bar_chart` / `pie_chart` / `radar_chart` / `chart` | 系列标识色 | 系列 1..N 靠颜色区分；换成主题色则无法辨认哪条是哪条 |
| `candlestick_chart` / `volume_chart` / `depth_chart` / `indicator_chart` | 涨跌红绿 / 买卖盘 / 指标线 | **编码方向**；统一成主题色会让"涨"和"跌"不可区分 |
| `color_picker` | 色相环本身 | 环上的颜色**就是**被选取的值；按主题重映射等于改变值 |
| `meter` | 阈值色带 | 色带是调用方设定的阈值区间，是数据 |
| `emoji_picker` | emoji 字形 | 字形是被挑选的内容 |
| `font_preview` | 字形样例 | 样例展示字体**本身**的样子 |
| `map_view` | 地图瓦片 | 地理内容，配色属于地图而非主题 |
| `video_player` / `media_player` / `camera_preview` | 解码后的画面 | 画面是内容 |
| `image_gallery` | 位图 | 图片颜色是内容 |
| `audio_visualizer` | 信号渐变 | 颜色编码被播放的信号 |

---

## `chrome` 类（17 个，真缺陷）——逐条归因

每个控件的**具体病处**与**修法边界**。修法边界一栏是重点：
「只改外观，不动内容」——否则会把数据色也一起主题化。

| # | 控件 | 文件 | 病处（实测） | 修法边界 |
|---|---|---|---|---|
| 1 | `animated_image` | `media_widgets/animated_image.rs` | 空态占位面板 `rgba(230,230,230,200)`、就绪背景 `Color::WHITE`、禁用背景 `rgba(200,200,200,100)` 全硬编码 | 只改**面板/背景/边框**；`draw_image` 的帧数据、播放三角、进度条填充不动 |
| 2 | `lottie_widget` | `media_widgets/lottie_widget.rs` | 空态 `rgba(230,230,230,200)`、就绪 `rgba(240,240,250,255)`、边框 `rgba(100,100,180,150)`、进度条底 `rgba(200,200,200,150)` | 只改面板/边框/进度条**底槽**；播放状态点（绿/橙）、进度填充色是状态编码，保留 |
| 3 | `rive_widget` | `media_widgets/rive_widget.rs` | 同 lottie（空态、就绪面板、紫色边框） | 同上 |
| 4 | `hero_animation` | `media_widgets/hero_animation.rs` | 外壳 `rgba(240,240,240,100/255)`、文字 `rgba(160,160,160,220)` | 只改外壳与文字；源/目标色插值 `33,118,210 → 76,175,80` 是**动画语义**（从哪到哪），保留 |
| 5 | `carousel` | `container_widgets/carousel.rs` | 空态 `fill_rounded_rect(rect, 8, rgba(230,230,230,200))`；`draw_page`/`draw_indicator` 亦无 style 读取 | 改空态面板 + 页容器背景 + 指示器；页内容由子控件自绘，不动 |
| 6 | `masonry_layout` | `container_widgets/masonry_layout.rs` | 空态提示 `rgba(180,180,180,200)`；布局容器的 `fill_rounded_rect(*item_rect, .., _item.color)` | 改空态与容器底；`_item.color` 是**调用方给每个卡片的色**，是数据，保留 |
| 7 | `sparkline` | `chart_widgets/sparkline.rs` | 无 `style` 读取；`line_color` / `last_point_color` 是**字段**，由调用方设定 | ✅ **判定为数据色（不改）**：它**不绘制任何背景**，唯一的颜色是调用方给的「系列身份」。按规则 #51 的判定（无重复即不接入），强行接入主题只会引入无收益的间接层。→ 移入豁免表 |
| 8 | `menu_bar` | `menu_toolbar/menu_bar.rs` | 条底与项底硬编码 | 改条底/项底/分隔线/文字；`draw_text` 颜色同样 |
| 9 | `menu_button` | `menu_toolbar/menu_button.rs` | `bg_color` 三档硬编码（`220,220,220,180` / `200,200,220,200` / `235,235,240,200`）、边框 `180,180,190,200`、弹出菜单 `Color::WHITE`、选中项 `220,235,255,200` | 改按钮三态底、边框、弹出菜单底；**选中项高亮**改读 style 的 accent（它是 chrome 的选中态，不是数据） |
| 10 | `tool_button` | `menu_toolbar/tool_button.rs` | 四态背景硬编码（`180,210,255` / `200,225,255` / `220,238,255` / `240,240,240`）、边框 `0,120,215` | 改四态底与边框；图标/文字颜色一并 |
| 11 | `progress_circle` | `display_widgets/progress_circle.rs` | `track_color` 为字段默认硬编码；禁用态 `rgba(200,200,200,100)` | 改**轨道**与禁用态；**进度弧**是值编码（可能带阈值色），按数据色处理，保留 |
| 12 | `skeleton_loader` | `display_widgets/skeleton_loader.rs` | `base_color = rgba(200,200,200, opacity*255)` 完全硬编码 | 改占位底色；opacity 脉动是动画参数，保留 |
| 13 | `masked_edit` | `input_widgets/masked_edit.rs` | `bg_color` 三档硬编码；光标/分隔符色亦硬编码 | 改底/边框/文字；**掩码占位符字形**保留 |
| 14 | `search_box` | `input_widgets/search_box.rs` | `bg_color` 三档（`240,240,240,160` / `245,245,255,220` / `235,235,235,200`）、边框 `200,200,200,160`、聚焦环 `60,140,255,200` | 改底/边框；**聚焦环**改读 style 的 accent（chrome 的聚焦态） |
| 15 | `search_bar` | `input_widgets/search_bar.rs` | `field_color` 硬编码（`212,157,216` 混合值→实为半透明灰叠在探针上的结果） | 同上 |
| 16 | `tag_input` | `input_widgets/tag_input.rs` | `bg_color`、`chip_bg`、`input_bg` 三处硬编码 | 改容器底/输入框底；**chip 底**是 chip 的 chrome，也改；chip 文字保留 |
| 17 | `swipe_to_dismiss` | `overlay_widgets/swipe_to_dismiss.rs` | 唯一颜色是**破坏性动作红** `rgba(255,59,48,255)`（iOS red）+ 白字 | ✅ **判定为语义色（规则 #108 ②）**：这个红不是外观色也不是数据色，它**编码「这一步会删除」**，因此必须读 `theme.colors.error`。→ 接语义 token，**不**入豁免表 |

---

## 归因收敛后的实际修法清单

上面 17 条里，有 **2 条经查证不是「外观色硬编码」**，不能按通用手法修：

| 控件 | 重新分类 | 处置 |
|---|---|---|
| `sparkline` | **③ 数据色**（不画背景，只有调用方给的系列色） | 移入 `tools/control_color_exemptions.txt`，附理由 |
| `swipe_to_dismiss` | **② 语义色**（破坏性动作红 → 编码「会删除」） | 改读 `theme.colors.error`（规则 #109） |

⇒ **实际需要按「外观色」修的只有 15 个**：
`animated_image`、`carousel`、`hero_animation`、`lottie_widget`、`masked_edit`、`masonry_layout`、
`menu_bar`、`menu_button`、`progress_circle`、`rive_widget`、`search_bar`、`search_box`、
`skeleton_loader`、`tag_input`、`tool_button`。

**这正是「逐条归因」的价值**：如果按「17 个都要主题化」一把梭，
`sparkline` 会被套上一个空的间接层（规则 #28/#51），而 `swipe_to_dismiss` 的删除红
会被当成外观色换成主题背景色——**那就把「危险」这个信息抹掉了**。

---

## 修复的通用手法（避免 15 次各写一遍）

每处的形态**高度一致**：`match 状态 { A => 字面量, B => 字面量, _ => 字面量 }`。
统一改为既有的三级优先链（第 58 轮已在 `tool_bar`/`status_bar` 上验证过）：

```rust
// 1) 调用方显式设置的颜色最优先
// 2) 主题为本控件解析出的样式
// 3) 原字面量作为最后回退（保留原外观，不引入回归）
let bg = style.background_color
    .or_else(|| crate::style::resolved_theme_style("<name>").and_then(|t| t.background_color))
    .unwrap_or(Color::rgba(230, 230, 230, 200));
```

**为什么保留字面量作回退**（而不是删掉）：
- 主题未激活时（`resolved_theme_style` 返回 `None`）控件仍有确定外观；
- **回退值是"旧行为"，所以任何既有像素测试都不回退**（规则 #21 向前兼容）；
- 第 3 层的 SVG 快照会把"主题生效后"的样子记下来，diff 可审。

**状态色的例外**：`focused` / `hovered` / `disabled` / `checked` 这类**交互态**各有语义，
不能都塌缩成同一个 `style.background_color`。做法是：
**用主题色作为基色，再用既有的 `blend`/`contrast` 工具派生状态色**（crate 里已有），
这样既随主题变化，又保留状态可区分性。

---

## 完成判据

- `KNOWN_THEME_BLIND` 缩短（每修好一个必须从清单删掉，门禁**双向断言**）；
- `check_control_rendering.sh` 的 `failed=0` 不变；
- 第 3 层 SVG 快照重新生成后，`<name>.svg` 与 `<name>.light.svg` 必须**不同**（新修的控件）；
- `snapshots/svg/` 的再生门禁逐字节通过；
- 五 profile 0 error、clippy `-D warnings` 干净。
