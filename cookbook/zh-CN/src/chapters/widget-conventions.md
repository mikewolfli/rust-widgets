# 控件惯例

本章是新增控件要对齐的清单：每一行都写明**姊妹控件已经在用的机制**、照抄哪一家、以及是否有门禁钉住。写成文字的原因是同一类缺口总按家族逐个补齐——撤销信号补了三家漏第四家、disabled 遮罩画了七家漏第八家——一份清单是最便宜的「防第九家」手段。

---

## 1. 姊妹对齐清单

| 关注点 | 机制 | 照抄哪家 | 由什么保证 |
|---|---|---|---|
| hover 反馈 | `draw` 末尾调用 `BaseWidget::paint_hover_wash(context, rect)` | `Button` | *无门禁*——靠与姊妹比对 |
| 禁用外观 | `BaseWidget::paint_disabled_veil(context, rect)`；处理输入前先看 `self.base.is_enabled()` | `Button` | `tools/check_disabled_state_covers_kinds.sh` |
| 焦点环 | 焦点可达名册上调用 `BaseWidget::draw_focus_ring_if_needed(...)` | `Button` | `tools/check_focus_ring_respects_reason.sh`、`tools/check_focus_reachable_key_arms.sh` |
| `enabled` 契约 | `handle_event` 里 `!self.base.is_enabled()` 早退 | `Button` | `tools/check_enabled_is_honoured.sh` |
| 撤销可用性 | `undo_available` / `redo_available`（`Signal1<bool>`，只在变化时发）——或一个在 undo/redo 时也会发的变更事件 | `SearchBar`、`Canvas` | `tools/check_capability_events_are_emitted.sh`、`tools/check_event_payload_types.py` |
| 双向光标 | 用 `text_utils::visual_caret_advance` / `text_utils::visual_byte_at`，不要自己算字节 | `TagInput`、`InplaceEditor` | `text_utils` 的单元测试 |
| 无障碍状态 | 在改变状态的那个 setter 里调用 `widget::a11y_submit::submit_state_changed(id)` | `Switch`、`Rating` | `tools/check_a11y_has_a_producer.sh` |
| 已发布事件 | `event_signal_dyn` 一个臂 + `tools/event_published_census.txt` 一行 | 任何带信号的控件 | `tools/check_event_signal_dyn.sh` |
| 尺寸 hint | `ControlMetrics::implicit_size` 配 `estimate_line_height` / `estimate_text_width` | `ZoomStrip` | `tools/check_implicit_size_uses_metrics.sh` |
| 隐式重绘 | 每个结构性修改器都调 `BaseWidget::request_redraw` | 任意 | `tools/check_no_anonymous_repaint.sh` |

### 没有门禁的几行才需要争论

上表有三项**没有**机械检查：hover 反馈、bidi 光标算法的「形状」，以及控件是否真的在输入路径上尊重 disabled（`check_disabled_state_covers_kinds.sh` 证明遮罩调用存在，`check_enabled_is_honoured.sh` 证明处理输入的控件会查 `enabled`，但两者合起来仍不等于「这个控件禁用时什么都不做」）。新增控件时，读两家姊妹实现仍然是工作的一部分。

---

## 2. 撤销历史：**程序化写入**该做什么

撤销的用户可见面各处一致（一个栈、`undo()`/`redo()`、`can_undo()` 查询）。真正按控件分化的是**程序化写入**——宿主调 `set_value`、绑定推一次模型变更——对历史意味着什么。仓里有两种被文档化的策略，新增控件必须在 setter 的文档里写明用哪一种：

| 策略 | 发生什么 | 仓内已文档化的例子 |
|---|---|---|
| **记一步** | 这次写入和用户编辑一样可撤销 | `DateEdit::set_date`——setter 里 push 一条 `ValueCommand`（正在恢复历史时除外） |
| **清历史** | 这次写入是新的基线：不记条目，且描述旧值的历史作废 | `LineEdit` 的 `EditOrigin::Programmatic`——「不记 undo 条目，且描述前一值的历史被丢弃」；`Canvas::clear()` 同时清空两个栈 |

第三种安排——只有一个 "modified" 标志、完全没有历史——在当前注册控件里不存在（`grep 'fn is_modified'` 找不到任何没有 `undo_stack` 的控件），所以不要悄悄引入。

### 可用性必须被宣告，不能靠轮询

宿主画一个撤销按钮，需要知道答案何时变化：

- **文本输入族**发布 `undo_available` / `redo_available`，且只在值变化时发；`undo()`、`redo()` 与每个修改点都调用同一个 `refresh_edit_capabilities()`。
- **Canvas 与两个编辑器**发布的文档变更事件在恢复路径上也会发（`content_changed`、`text_changed`），宿主监听同一信号再读 `can_undo()`。

无论选哪种形态，规则一样：撤销/重做必须能通过信号被看见，而不只是一个 getter。

---

## 3. 为什么这是惯例而不是框架

上面每个机制都长在 `BaseWidget` 或共享 helper 上，控件一次调用就能接上，不必重新推导规则。惯例留给不能共享的那部分：控件**有没有**这个状态、程序化写入对它意味着什么、它在跟谁对齐。把那个决定写进 setter 的文档里——下一个读者就长在那里。
