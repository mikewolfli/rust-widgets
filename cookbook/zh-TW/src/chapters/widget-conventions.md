# 控制項慣例

本章是新增控制項要對齊的清單：每一列都寫明**姊妹控制項已在用的機制**、照抄哪一家、以及是否有閘門釘住。寫成文字的原因是同一類缺口總按家族逐個補齊——撤銷訊號補了三家漏第四家、disabled 遮罩畫了七家漏第八家——一份清單是最便宜的「防第九家」手段。

---

## 1. 姊妹對齊清單

| 關注點 | 機制 | 照抄哪家 | 由什麼保證 |
|---|---|---|---|
| hover 回饋 | `draw` 末尾呼叫 `BaseWidget::paint_hover_wash(context, rect)` | `Button` | *無閘門*——靠與姊妹比對 |
| 停用外觀 | `BaseWidget::paint_disabled_veil(context, rect)`；處理輸入前先看 `self.base.is_enabled()` | `Button` | `tools/check_disabled_state_covers_kinds.sh` |
| 焦點環 | 焦點可達名冊上呼叫 `BaseWidget::draw_focus_ring_if_needed(...)` | `Button` | `tools/check_focus_ring_respects_reason.sh`、`tools/check_focus_reachable_key_arms.sh` |
| `enabled` 契約 | `handle_event` 裡 `!self.base.is_enabled()` 早退 | `Button` | `tools/check_enabled_is_honoured.sh` |
| 撤銷可用性 | `undo_available` / `redo_available`（`Signal1<bool>`，只在變化時發）——或一個在 undo/redo 時也會發的變更事件 | `SearchBar`、`Canvas` | `tools/check_capability_events_are_emitted.sh`、`tools/check_event_payload_types.py` |
| 雙向游標 | 用 `text_utils::visual_caret_advance` / `text_utils::visual_byte_at`，不要自己算位元組 | `TagInput`、`InplaceEditor` | `text_utils` 的單元測試 |
| 無障礙狀態 | 在改變狀態的那個 setter 裡呼叫 `widget::a11y_submit::submit_state_changed(id)` | `Switch`、`Rating` | `tools/check_a11y_has_a_producer.sh` |
| 已發佈事件 | `event_signal_dyn` 一個臂 + `tools/event_published_census.txt` 一行 | 任何帶訊號的控制項 | `tools/check_event_signal_dyn.sh` |
| 尺寸 hint | `ControlMetrics::implicit_size` 配 `estimate_line_height` / `estimate_text_width` | `ZoomStrip` | `tools/check_implicit_size_uses_metrics.sh` |
| 隱式重繪 | 每個結構性修改器都呼叫 `BaseWidget::request_redraw` | 任意 | `tools/check_no_anonymous_repaint.sh` |

### 沒有閘門的幾列才需要爭論

上表有三項**沒有**機械檢查：hover 回饋、bidi 游標演算法的「形狀」，以及控制項是否真的在輸入路徑上尊重 disabled（`check_disabled_state_covers_kinds.sh` 證明遮罩呼叫存在，`check_enabled_is_honoured.sh` 證明處理輸入的控制項會查 `enabled`，但兩者合起來仍不等於「這個控制項停用時什麼都不做」）。新增控制項時，讀兩家姊妹實作仍然是工作的一部分。

---

## 2. 撤銷歷史：**程式化寫入**該做什麼

撤銷的使用者可見面各處一致（一個堆疊、`undo()`/`redo()`、`can_undo()` 查詢）。真正按控制項分化的是**程式化寫入**——宿主呼叫 `set_value`、綁定推一次模型變更——對歷史意味著什麼。儲存庫裡有兩種被文件化的策略，新增控制項必須在 setter 的文件裡寫明用哪一種：

| 策略 | 發生什麼 | 儲存庫內已文件化的例子 |
|---|---|---|
| **記一步** | 這次寫入和使用者編輯一樣可撤銷 | `DateEdit::set_date`——setter 裡 push 一條 `ValueCommand`（正在還原歷史時除外） |
| **清歷史** | 這次寫入是新的基線：不記條目，且描述舊值的歷史作廢 | `LineEdit` 的 `EditOrigin::Programmatic`——「不記 undo 條目，且描述前一值的歷史被丟棄」；`Canvas::clear()` 同時清空兩個堆疊 |

第三種安排——只有一個 "modified" 旗標、完全沒有歷史——在目前註冊控制項裡不存在（`grep 'fn is_modified'` 找不到任何沒有 `undo_stack` 的控制項），所以不要悄悄引入。

### 可用性必須被宣告，不能靠輪詢

宿主畫一個撤銷按鈕，需要知道答案何時變化：

- **文字輸入族**發佈 `undo_available` / `redo_available`，且只在值變化時發；`undo()`、`redo()` 與每個修改點都呼叫同一個 `refresh_edit_capabilities()`。
- **Canvas 與兩個編輯器**發佈的文件變更事件在還原路徑上也會發（`content_changed`、`text_changed`），宿主監聽同一訊號再讀 `can_undo()`。

無論選哪種形態，規則一樣：撤銷/重做必須能透過訊號被看見，而不只是一個 getter。

---

## 3. 為什麼這是慣例而不是框架

上面每個機制都長在 `BaseWidget` 或共享 helper 上，控制項一次呼叫就能接上，不必重新推導規則。慣例留給不能共享的那部分：控制項**有沒有**這個狀態、程式化寫入對它意味著什麼、它在跟誰對齊。把那個決定寫進 setter 的文件裡——下一個讀者就長在那裡。
