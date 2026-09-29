# docs/plans — 计划索引

> **本文件是入口。** 目录里有 14 份 `.md`（4 份是**工具/门禁依赖**，不可移动），
> 38 份历史计划已归档到 [`archive/`](archive/)。

---

## 1. 活跃计划（正在推进）

| 文件 | 内容 | 状态 |
|---|---|---|
| [`blue24.md`](blue24.md) | 帧循环 / 属性动画 / 正交状态 / 环境事实 / a11y 落地 | **主体已完成**（§10 批 0–8 与 §9 均 ✅）；**本文件仍是 BLUE24 自身后续条目的唯一入口**（§12 的 U-1…U-14），未完成项登记在那里 |

> **BLUE23 已归档** → [`archive/blue23.md`](archive/blue23.md)（本轮收口：0 个 `[ ]`，
> 全部批次 ✅，5 个跟踪扫描 0 失败）。

> **两份可并行**：`blue24.md` §12 写明了三条边界——BLUE24 不登记、不依赖、不阻塞
> BLUE23 的余项。BLUE24 的批 1（帧循环）在 BLUE23 收口前就能开工。

### 计划分级

- **blue1 ~ blue23**：✅ 已完成 → 见 [`archive/`](archive/)
- **blue24**：**主体已完成**，且仍是 BLUE24 自身后续条目（§12）的唯一入口

---

## 2. 参考文档（**不是**待办清单）

| 文件 | 是什么 | 谁在读 |
|---|---|---|
| [`principle.md`](principle.md) | 全仓工程原则（#1–#101），所有 BLUE 计划的依据 | 所有计划、`src/widget/special_widgets/code_editor/mod.rs` |
| [`codemap.md`](codemap.md) | 模块地图 | **门禁** `check_widget_kind_count.sh` |
| [`platform_capability_matrix.md`](platform_capability_matrix.md) | 平台能力矩阵（**生成物**，不手改） | **三个门禁** + `tests/blue9_r6_platform_capability_test.rs` |
| [`platform_differences.md`](platform_differences.md) | 平台差异说明 | 人读 |
| [`whitepaper.md`](whitepaper.md) | 架构白皮书 | 人读 |
| [`steps.md`](steps.md) | 阶段进度 | 人读 |
| [`custom_widget_mounting.md`](custom_widget_mounting.md) | 自绘控件挂载契约 | `src/platform/{linux,macos,windows}/canvas.rs` |
| [`harmony_integration.md`](harmony_integration.md) | Harmony 集成说明 | 人读 |

---

## 3. 待办与受限项

| 文件 | 是什么 |
|---|---|
| [`TODO.md`](TODO.md) | 路线图待办。**当前 `[x]` 127 / `[ ]` 0 / `[~]` 0**（即无待办） |
| [`FUTURE.md`](FUTURE.md) | 环境受限项（需特定宿主才能验证）。**8 项中有 2 项已关闭** |

> `TODO.md` 与 `FUTURE.md` 都不在 `docs/plans/` 的历史归档范围内：
> 前者被 `tools/check_locking.sh` / `tools/check_error_messages.py` / CI 引用，
> 后者是「受限项」的正式载体。**不要归档它们。**

---

## 4. 归档（38 份）

[`archive/`](archive/) 里的计划**全部已完成**或**已被取代**，仅供追溯。
**不要再从里面取「当前要求」**——那是 BLUE24 §9.1 要消除的失败形态
（同一件事在两处各写一半）。

> **本表逐份列名**（`tools/check_plan_archive_has_index.sh` 断言每一份都在此有一行）：
> 一个「`blue1.md` … `blue22.md`」的范围写法看起来更短，但它会让 `blue11.md` 与
> `blue13.md` 之间的文件既不在表里也不在视线里，而“中间那些是什么”恰恰是归档最该回答的。

| 归档文件 | 说明 |
|---|---|
| [`blue1.md`](archive/blue1.md) … [`blue22.md`](archive/blue22.md)、[`blue23.md`](archive/blue23.md)（逐份见下）| 历次计划主体，逐次收口 |
| `blue1.md` | BLUE1 — 控件库奠基 |
| `blue2.md` | BLUE2 — 控件扩充 |
| [`BLUE3.md`](archive/BLUE3.md) | BLUE3 — 计划主体（大写拼写，与 `blue3` 不共存） |
| [`BLUE3_R3_COMPLETION.md`](archive/BLUE3_R3_COMPLETION.md) | BLUE3 第 3 轮完成记录 |
| [`BLUE4.md`](archive/BLUE4.md) | BLUE4 — 计划主体 |
| [`BLUE5.md`](archive/BLUE5.md) | BLUE5 — 计划主体 |
| [`BLUE6.md`](archive/BLUE6.md) | BLUE6 — 计划主体 |
| [`BLUE7.md`](archive/BLUE7.md) | BLUE7 — 计划主体 |
| [`blue7_verification.md`](archive/blue7_verification.md) | BLUE7 验证记录 |
| [`blue8.md`](archive/blue8.md) | BLUE8 — 计划主体 |
| [`blue9.md`](archive/blue9.md) | BLUE9 — 计划主体 |
| [`blue10.md`](archive/blue10.md) | BLUE10 — 计划主体 |
| [`blue10_degraded_mappings_audit.md`](archive/blue10_degraded_mappings_audit.md) | 降级映射审计（BLUE10 附录） |
| [`blue10_wgpu_upgrade_evaluation.md`](archive/blue10_wgpu_upgrade_evaluation.md) | wgpu 升级评估（BLUE10 附录） |
| [`blue11.md`](archive/blue11.md) | BLUE11 — 计划主体 |
| [`blue12.md`](archive/blue12.md) | BLUE12 — 计划主体 |
| [`blue13.md`](archive/blue13.md) | BLUE13 — 计划主体 |
| [`blue14.md`](archive/blue14.md) | BLUE14 — 计划主体 |
| [`blue15.md`](archive/blue15.md) | BLUE15 — 计划主体 |
| [`blue16.md`](archive/blue16.md) | BLUE16 — 计划主体 |
| [`blue17.md`](archive/blue17.md) | BLUE17 — 计划主体 |
| [`blue18.md`](archive/blue18.md) | BLUE18 — 计划主体 |
| [`blue19.md`](archive/blue19.md) | BLUE19 — 计划主体 |
| [`blue20.md`](archive/blue20.md) | BLUE20 — 计划主体 |
| [`blue20_p3_backlog.md`](archive/blue20_p3_backlog.md) | BLUE20 P3 遗留（已并入后续计划） |
| [`blue21.md`](archive/blue21.md) | BLUE21 — 计划主体 |
| [`blue22.md`](archive/blue22.md) | BLUE22 — 计划主体 |
| [`blue23.md`](archive/blue23.md) | BLUE23 — 状态层 / 动效总线 / 层级层 / 声明式原语（附录 A 逐条收口）|
| [`cocoa_to_objc2_migration.md`](archive/cocoa_to_objc2_migration.md) | cocoa → objc2 迁移方案（已实施） |
| [`miri_audit.md`](archive/miri_audit.md) | Miri 审计（已实施） |
| [`plan.md`](archive/plan.md) | 早期总计划（已被 BLUE 系列取代） |
| [`plan copy.md`](archive/plan%20copy.md) | 早期总计划的重复副本（已废） |
| [`REFACTOR_PLAN.md`](archive/REFACTOR_PLAN.md) | 重构计划（已完成） |
| [`REFACTOR_EXECUTION_GUIDE.md`](archive/REFACTOR_EXECUTION_GUIDE.md) | 重构执行指南（已完成） |
| [`scan_round1.md`](archive/scan_round1.md) | 早期扫描第 1 轮（已被 BLUE 系列取代） |
| [`scan_round2.md`](archive/scan_round2.md) | 早期扫描第 2 轮（已被 BLUE 系列取代） |
| [`scan_round3.md`](archive/scan_round3.md) | 早期扫描第 3 轮（已被 BLUE 系列取代） |
| [`TODO.md`](archive/TODO.md) | 路线图待办的**历史快照**；现行 TODO 在 [`../TODO.md`](TODO.md) |

---

## 5. 维护规则

1. **新计划可以新建文件。**`docs/plans/blue26.md`、`blue27.md` … 均可；
   一条新主题就开一份新计划，不再要求把新主题塞进 `blue24.md`。
2. **一份计划主体全部完成后**才移入 `archive/`，并**必须**在本文件 §4 补一行。
3. **第 12 节的四份文件不可移动**：`codemap.md` / `platform_capability_matrix.md` /
   `TODO.md` / `principle.md` 被门禁或源码引用，移动会让门禁报红。
4. **同一件事不两处各写一半**。同一主题的两份计划允许并行（见 §1），但**同一主题**
   不应在两处各写一半——这是本目录收敛的唯一目标。

### 可复跑的检查

```text
$ ls docs/plans/*.md | wc -l          # 13
$ ls docs/plans/archive/*.md | wc -l  # 38
$ grep -rn "docs/plans/" --include=*.rs --include=*.sh --include=*.py . | grep -v archive
  # 零命中指向 archive/ 的路径（除刻意保留的历史引注）
```

### 两条元门禁（BLUE24 §9）

> `check_single_open_plan.sh` 已于 2026-09-29 **删除**（用户指令）：它把「同一主题
> 至多一份活计划」变成硬门禁，连带禁止新建 `blue25/blue26`。现已改为**目录纪律**（见 §5.4），
> 门禁本身不再存在。

本目录的收敛由**两条可运行的门禁**钉住（而不是靠习惯）：

| 门禁 | 断言 | 反向注入 |
|---|---|---|
| `tools/check_plan_archive_has_index.sh` | `archive/` 每份文件在本 README 有行；README 不引用不存在的归档 | 向 `archive/` 放一份未入索引的文件 ⇒ 变红 |
| `tools/check_gates_are_worth_running.sh` | 每个 `check_*.sh` 被 `run_all_gates.sh` 的 glob 枚举；运行预算 ≤ 45 min；每个门禁在 `gates_reverse_injection.md` 有名 | 删一行记录 / 改预算为 9999 ⇒ 各自变红 |

> `gates_reverse_injection.md` 逐门禁记录「注入什么、报什么」。尚未注入的门禁以 `—`
> 如实列出（不计入通过），元门禁会把它作为一个**数字**打印出来。
