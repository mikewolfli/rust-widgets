| R130 | Keyboard ↔ 参考工具包 Virtual Keyboard | [组成] 仅 Qwerty/Numeric 双布局（:40-45）；无 CapsLock、无死键、无 emoji/多语言布局。 | 343 | ⬜ |
| R131 | Keyboard ↔ 参考工具包 Virtual Keyboard | [操作] 无长按变音弹层、无按键自动重复（:452-471 按下即触发无 repeat）、无滑行输入、无联想候选条。 | 343 | ⬜ |
| R132 | Keyboard ↔ 参考工具包 Virtual Keyboard | [动画] 仅按压高亮闩锁，无按键缩放/涟漪。 | 343 | ⬜ |
| R133 | ListBox ↔ 参考工具包 `list widget` | [组成] 条目纯字符串（:39）：无图标/checkState/角色/排序/多列/setItemWidget/editTriggers 重命名 → Item 结构化。 | 348 | ⬜ |
| R134 | ListBox ↔ 参考工具包 `list widget` | [事件] 键盘移动要求 `modifiers == 0`（:810/:820）——**Shift/Ctrl+方向键键盘扩展选区缺失**（Extended 仅鼠标 :328-365）→ 键盘路径并入 select_with_modifiers。 | 348 | ⬜ |
| R135 | ListBox ↔ 参考工具包 `list widget` | [事件] 无 PageUp/Down、无首字母 type-ahead、无 Ctrl+Home/End；DnD 重排缺失。 | 348 | ⬜ |
| R136 | MaskedEdit ↔ 参考工具包 inputMask | [属性] 掩码字母表仅 0/9/A/a/X/x（:1292-1307）：缺 D/d、#、H/h、B/b、大小写转换器 `>` `<` `!`、转义 `\`、`;c` 自定义占位符（恒 `_` :1272-1278）。 | 353 | ⬜ |
| R137 | MaskedEdit ↔ 参考工具包 inputMask | [操作] 无粘贴进掩码路径；无双击选词。 | 353 | ⬜ |
| R138 | Mention | [事件] **`ImeCommit` 无分支**（:864-922 仅 TextInput/KeyDown/KeyPress）——**IME 提交的中文/日文文本被整体丢弃（bug 级）** → 补 `Event::ImeCommit` 臂。 | 357 | ⬜ |
| R139 | Mention | [操作] 点击仅落 caret 至**文末**（:872-879 自认 "no glyph metrics"）：无任意落点、无选区、无拖选 → 复用 LineEdit `byte_index_at_x`。 | 357 | ⬜ |
| R140 | Mention | [属性] MAX_VISIBLE/行高为常量非属性（:107-110）。 | 357 | ⬜ |
| R141 | MultiSelectComboBox | [事件] **完全无键盘**（:670-695 仅鼠标）：无 Escape 收起、无上下键、无 Space 切换 → 键盘臂补齐。 | 362 | ⬜ |
| R142 | MultiSelectComboBox | [属性] 弹层高=28×items 全量（:683）无上限、无 Wheel；无全选/清空弹层动作。 | 362 | ⬜ |
| R143 | NumberPicker ↔ 参考工具包 `spin box`/NSPicker | [属性] 无 prefix、无小数支持（i64）、无 special_value_text、无数值键入。 | 366 | ⬜ |
| R144 | NumberPicker ↔ 参考工具包 `spin box`/NSPicker | [事件] Wheel/drag/箭头/PageUp/Down/Home/End/双击半区 ≈齐备（:418-506，全目录键盘/滚轮最完整控件）。 | 366 | ⬜ |
| R145 | NumberPicker ↔ 参考工具包 `spin box`/NSPicker | [动画] 松手直接吸附（:241-251），无惯性减速/弹性吸附 → AnimationDriver ease-out 收尾。 | 366 | ⬜ |
| R146 | OtpInput | [事件] **无 `TextInput`/`ImeCommit` 臂**（:496-527 仅 KeyPress ASCII）→ IME 与非 ASCII 键盘无法输入。 | 371 | ⬜ |
| R147 | OtpInput | [事件] 无 Primary+V 键绑定（paste() 仅为公有 API :279-295）；无 Tab 逐框焦点；Enter/Escape 空操作（:509）。 | 371 | ⬜ |
| R148 | RangeSlider | [事件] **无键盘**（:885-935 仅鼠标；slider 有箭头/PageUp/Down/Home/End）；无 Wheel。 | 375 | ⬜ |
| R149 | RangeSlider | [操作] 点击轨道不跳最近柄（无 track-jump）；无焦点柄可视化。 | 375 | ⬜ |
| R150 | RichEdit ↔ 参考工具包 `text edit` | [组成] **名实不符**：纯 String+选区（:33-73），无格式 span/图片/HTML-Markdown 导入导出 → 引入 span 或对齐 plain text edit 更名。 | 379 | ⬜ |
| R151 | RichEdit ↔ 参考工具包 `text edit` | [事件] **无拖选**（:774-784 落 caret 无 MouseMove 扩展臂）；无剪贴板三键、无 Wheel、无滚动（无 first_visible 字段，超出可视高度即裁掉）。 | 379 | ⬜ |
| R152 | SearchBar ↔ `NSSearchField` | [事件] **无焦点门控**：KeyPress 不查 focused（:537-564），任何键入即改写文本（对照 search_box.rs:567 有门控）。 | 383 | ⬜ |
| R153 | SearchBar ↔ `NSSearchField` | [操作] 追加式编辑无光标；无 Escape 取消；取消按钮宽度硬编码 60px（:506-508）；无最近搜索菜单、无 token 化。 | 383 | ⬜ |
| R154 | SearchBox ↔ line edit(clearButton) | [组成] **无光标绘制**（:29-47 无 caret 字段）：聚焦后无插入点可视 → 复用 CursorBlink。 | 387 | ⬜ |
| R155 | SearchBox ↔ line edit(clearButton) | [操作] 追加式无光标模型（Delete 判定 no-op 自证 :587-594）；无 Left/Right/选区。 | 387 | ⬜ |
| R156 | ShortcutEditor ↔ 参考工具包 `key sequence edit` | [组成] 无逐行 capture 模式（录制态提示/Escape 取消/Backspace 清空）；无多键序列（一次 KeyPress 即定值 :766-772）；无冲突检测；无单独清除/恢复单项。 | 391 | ⬜ |
| R157 | ShortcutEditor ↔ 参考工具包 `key sequence edit` | [风格] 裸数字比较 `*modifiers == 2`（:756/:760）绕过 `Modifiers` 抽象；无 ⌘⌥⇧ 符号渲染（:784-836 纯文本名）。 | 391 | ⬜ |
| R158 | SpinBox ↔ 参考工具包 `spin box`/`double spin box` | [组成] **数值不可键入**：无文本编辑面（:986-1031 仅按钮+上下箭头）→ 内嵌数值文本编辑+提交解析。 | 395 | ⬜ |
| R159 | SpinBox ↔ 参考工具包 `spin box`/`double spin box` | [事件] **无 Wheel 步进**；无 PageUp/Down；按住按钮无自动重复（abstract spin box autoRepeat）。 | 395 | ⬜ |
| R160 | SpinBox ↔ 参考工具包 `spin box`/`double spin box` | [属性] prefix/suffix/decimals/specialValueText/wrapping ≈高对齐（:524-570）；无 stepType；到界时按钮无禁用视觉态。 | 395 | ⬜ |
| R161 | TagInput ↔ `NSTokenField` | [操作] 输入缓冲无光标（恒追加 :291-299）；无 Left/Right、中段编辑、选区。 | 400 | ⬜ |
| R162 | TagInput ↔ `NSTokenField` | [事件] 无剪贴板（**粘贴逗号分隔批量建标签**缺失）；无 tag 拖拽重排；无建议补全；无双击编辑已有标签；无 max tags 上限。 | 400 | ⬜ |
