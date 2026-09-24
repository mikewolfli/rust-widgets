# docs/plans — 计划索引

> **本文件是入口。** 目录里有 12 份 `.md`（4 份是**工具/门禁依赖**，不可移动），
> 35 份历史计划已归档到 [`archive/`](archive/)。

---

## 1. 活跃计划（正在推进）

| 文件 | 内容 | 状态 |
|---|---|---|
| [`blue23.md`](blue23.md) | 状态层 / 动效总线 / 层级层 / 声明式原语 | **进行中**（另一个进程处理） |
| [`blue24.md`](blue24.md) | 帧循环 / 属性动画 / 正交状态 / 环境事实 / a11y 落地 | **下一步**（待 BLUE23 收口后开工） |

> **两份可并行**：`blue24.md` §12 写明了三条边界——BLUE24 不登记、不依赖、不阻塞
> BLUE23 的余项。BLUE24 的批 1（帧循环）在 BLUE23 收口前就能开工。

### 计划分级

- **blue1 ~ blue22**：✅ 已完成 → 见 [`archive/`](archive/)
- **blue23**：进行中
- **blue24**：下一步，且是 BLUE24 自身后续条目的唯一入口

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

## 4. 归档（35 份）

[`archive/`](archive/) 里的计划**全部已完成**或**已被取代**，仅供追溯。
**不要再从里面取「当前要求」**——那是 BLUE24 §9.1 要消除的失败形态
（同一件事在两处各写一半）。

| 归档文件 | 说明 |
|---|---|
| `blue1.md` … `blue22.md`（22 份） | 历次计划主体，逐次收口 |
| `blue7_verification.md` | BLUE7 验证记录 |
| `blue10_degraded_mappings_audit.md` | 降级映射审计（BLUE10 附录） |
| `blue10_wgpu_upgrade_evaluation.md` | wgpu 升级评估（BLUE10 附录） |
| `blue20_p3_backlog.md` | BLUE20 P3 遗留（已并入后续计划） |
| `cocoa_to_objc2_migration.md` | cocoa → objc2 迁移方案（已实施） |
| `miri_audit.md` | Miri 审计（已实施） |
| `plan.md` | 早期总计划（已被 BLUE 系列取代） |
| `REFACTOR_PLAN.md` / `REFACTOR_EXECUTION_GUIDE.md` | 重构计划与执行指南（已完成） |
| `scan_round1.md` / `scan_round2.md` / `scan_round3.md` | 早期扫描轮次（已被 BLUE 系列取代） |

---

## 5. 维护规则

1. **新计划 = 在 `blue24.md` 里加一节。** 不新建 `blue25.md`/`blue26.md`
   （`blue24.md` §9.1 的门禁 `check_single_open_plan` 会拦）。
2. **一份计划主体全部完成后**才移入 `archive/`，并**必须**在本文件 §4 补一行。
3. **第 12 节的四份文件不可移动**：`codemap.md` / `platform_capability_matrix.md` /
   `TODO.md` / `principle.md` 被门禁或源码引用，移动会让门禁报红。
4. **同一件事不两处各写一半**。这是本目录收敛的唯一目标——
   「多份计划并行」是允许的（见 §1），「同一主题两份计划」不是。

### 可复跑的检查

```text
$ ls docs/plans/*.md | wc -l          # 12
$ ls docs/plans/archive/*.md | wc -l  # 35
$ grep -rn "docs/plans/" --include=*.rs --include=*.sh --include=*.py . | grep -v archive
  # 零命中指向 archive/ 的路径（除刻意保留的历史引注）
```
