| R087 | LineEdit ↔ 参考工具包 `line edit` | [属性] 缺 `PasswordEchoOnEdit`（第四变体）；缺 `modified` 脏标记；缺 `clearButtonEnabled` → EchoMode 加变体 + 清除按钮装饰位。 | 274 | ⬜ |
| R088 | LineEdit ↔ 参考工具包 `line edit` | [事件] 无 `MouseDoubleClick`（双击选词/三击全选）；无 selectionChanged/cursorPositionChanged 信号。 | 274 | ⬜ |
| R089 | LineEdit ↔ 参考工具包 `line edit` | [操作] **选区模型存在但绘制缺失**（draw :1707-1962 无 selection 填充；TextArea/TextEdit 均画选带）→ 补 per-line fill_rect（复用 textarea.rs:1300-1336 模式）。 | 274 | ⬜ |
| R090 | LineEdit ↔ 参考工具包 `line edit` | [操作] 长文本溢出走省略号而非水平滚动（surface.rs:1048；caret 被夹在 :1909-1914）→ 引入 scroll_x + caret 驱动 reveal 滚动。 | 274 | ⬜ |
| R091 | LineEdit ↔ 参考工具包 `line edit` | [操作] 无 Ctrl+Left/Right 词移动、无文本 DnD。 | 274 | ⬜ |
| R092 | LineEdit ↔ 参考工具包 `line edit` | [macOS] 无焦点光环（:1741-1754 仅语义描边）→ 主题级 focus-ring token。 | 274 | ⬜ |
| R093 | TextArea ↔ 参考工具包 `plain text edit` | [组成] 纯 `String` 模型，无 maximumBlockCount 环形缓冲、无内容边距；**无软换行**（:1268-1274 自认）、无水平滚动。 | 282 | ⬜ |
| R094 | TextArea ↔ 参考工具包 `plain text edit` | [事件] **无 Wheel 臂**（:1037-1193）；**无剪贴板快捷键**（仅 undo/redo/select-all :1062-1075，无 Primary+C/X/V；LineEdit 有 :1473-1523）；无双击选词、无 PageUp/Down、Tab 被当控制符丢弃（:1159）。 | 282 | ⬜ |
| R095 | TextArea ↔ 参考工具包 `plain text edit` | [丝滑] 选带绘制每行每帧重新 `text_advance` 测量（:1321-1322）→ 缓存行前缀宽度表。 | 282 | ⬜ |
| R096 | TextArea ↔ 参考工具包 `plain text edit` | [组成] 撤销为整文快照（:80-82），大文档每键 O(n)。 | 282 | ⬜ |
| R097 | TextEdit ↔ 参考工具包 `text edit`/`plain text edit` | [组成] **名为 Rich 实为纯文本+行区间**（:38-97）：无格式 span/图片/列表/HTML 导入导出 → 引入 `Vec<(Range, CharFormat)>` 或按 plain text edit 定位更名。 | 288 | ⬜ |
| R098 | TextEdit ↔ 参考工具包 `text edit`/`plain text edit` | [操作] 折行**逐字符断行**（rows_for :594 任意字符处断），参考工具包 默认词边界 → 断行点回退空格/连字符。 | 288 | ⬜ |
| R099 | TextEdit ↔ 参考工具包 `text edit`/`plain text edit` | [事件] 无 Wheel、无剪贴板三键、无双击选词、无 PageUp/Down。 | 288 | ⬜ |
| R100 | TextEdit ↔ 参考工具包 `text edit`/`plain text edit` | [丝滑] 行缓存失效用整串 `text ==` 比较（:497-501）每键 O(n) → 代际计数器。 | 288 | ⬜ |
| R101 | ComboBox ↔ 参考工具包 `combo box` | [组成] 条目纯 `Vec<String>`（:229）：无 per-item 图标/数据角色/分隔符、无 model/view 分离；editable 仅布尔旗标无内嵌编辑器 → 引入 `ComboBoxItem { text, icon, data, enabled }`。 | 294 | ⬜ |
| R102 | ComboBox ↔ 参考工具包 `combo box` | [属性] 无 insertPolicy、无 sizeAdjustPolicy。 | 294 | ⬜ |
| R103 | ComboBox ↔ 参考工具包 `combo box` | [事件] 无 Wheel 步进；弹层无 PageUp/Down；无首字母 type-ahead；无 completer 挂点。 | 294 | ⬜ |
| R104 | ComboBox ↔ 参考工具包 `combo box` | [动画] 弹层瞬开无渐显 → AnimationDriver 150ms fade。 | 294 | ⬜ |
| R105 | EditableComboBox ↔ 参考工具包 `combo box`(editable) | [组成] **无光标模型：输入恒追加**（:204-210），无 Left/Right/Home/End/选区（:654-719）→ 复用 LineEdit 作内嵌编辑器（参考工具包 即此架构）。 | 300 | ⬜ |
| R106 | EditableComboBox ↔ 参考工具包 `combo box`(editable) | [属性] 无 max_visible_items：弹层高=items×28 全量展开（:753-757）→ 限高+滚动。 | 300 | ⬜ |
| R107 | EditableComboBox ↔ 参考工具包 `combo box`(editable) | [事件] 无 Wheel、无实时补全过滤。 | 300 | ⬜ |
| R108 | FontComboBox ↔ 参考工具包 `font combo box` | [组成] 弹层各行统一 `Font::default_ui()`（:783）**未以各自字族渲染**（参考工具包 核心体验）→ 每行按 family 绘制；无字样预览列、无 sampleText。 | 305 | ⬜ |
| R109 | FontComboBox ↔ 参考工具包 `font combo box` | [属性] 无 fontFilters（Monospaced/Scalable/Proportional）、无 writingSystemFilter。 | 305 | ⬜ |
| R110 | FontComboBox ↔ 参考工具包 `font combo box` | [事件] 释放时**循环到下一个字体而非命中行**（:627-642，弹层无行命中测试）；弹层不随外部点击关闭（:381 自认）；无 Wheel、无 type-ahead；无弹层滚动。 | 305 | ⬜ |
| R111 | AutoCompleteEdit ↔ 参考工具包 `completer` | [属性] 匹配模式硬编码 contains-忽略大小写（:259-282）：无 MatchStartsWith/EndsWith/RegExp/FixedString、无 caseSensitivity、无 completionPrefix 词级定位。 | 310 | ⬜ |
| R112 | AutoCompleteEdit ↔ 参考工具包 `completer` | [操作] 输入恒追加（:129-135）：无光标、无中段插入、无 Tab 补全、无 InlineCompletion 模式。 | 310 | ⬜ |
| R113 | AutoCompleteEdit ↔ 参考工具包 `completer` | [事件] 无鼠标光标定位；补全后弹层关闭策略不可配。 | 310 | ⬜ |
| R114 | Cascader ↔ 参考工具包 `combo box`+级联 | [属性] 选项无 `enabled` 禁用位（:126-135）；异步 Loading/Failed 状态设计 ≈领先（:104-116）。 | 315 | ⬜ |
| R115 | Cascader ↔ 参考工具包 `combo box`+级联 | [事件] filterable 过滤吃 `TextInput` 但**不吃 `ImeCommit`**（:1054-1059）——**CJK 过滤断裂** → 补 ImeCommit 分支。 | 315 | ⬜ |
| R116 | Cascader ↔ 参考工具包 `combo box`+级联 | [操作] 键盘仅 Up/Down/Escape（:1045-1050）：无 Right/Left 跨级、无 Enter 提交叶、无 Home/End。 | 315 | ⬜ |
| R117 | Cascader ↔ 参考工具包 `combo box`+级联 | [丝滑] 级列无 Wheel、无行数上限（draw_level :899-1001 全量画）；弹层瞬开。 | 315 | ⬜ |
| R118 | CommandLink ↔ 参考工具包 `command link` | [组成] 无盾形图标槽位。 | 321 | ⬜ |
| R119 | CommandLink ↔ 参考工具包 `command link` | [事件] 无键盘激活（:202-218 仅鼠标）；clicked 在**按下**即发（:206-208），参考工具包 语义为释放确认可滑出取消 → 改 release-commit。 | 321 | ⬜ |
| R120 | Dropdown ↔ 参考工具包 `combo box`/`NSPopUpButton` | [属性] 无 max_visible/滚动偏移（:98-115），弹层全量绘制（文档自述 "scrollable" :7 但实现无滚动）。 | 325 | ⬜ |
| R121 | Dropdown ↔ 参考工具包 `combo box`/`NSPopUpButton` | [事件] 无 Wheel（:415-476）、无 Home/End/首字母、无 PageUp/Down。 | 325 | ⬜ |
| R122 | ImePreedit ↔ 参考工具包 `input-method event` | [组成] **只消费 KeyPress 自造 preedit（:324-345），不处理真实 `Event::ImePreedit`/`ImeCommit`** → 订阅真实事件变体。 | 329 | ⬜ |
| R123 | ImePreedit ↔ 参考工具包 `input-method event` | [组成] 无分段属性（参考工具包 每段可带 TextFormat/Cursor attribute）：整串同色同下划线（:228-316）→ 分段 span。 | 329 | ⬜ |
| R124 | ImePreedit ↔ 参考工具包 `input-method event` | [生态] 无 caret 屏幕坐标上报（micro-focus anchoring）——平台候选窗无法锚定（见共性）。 | 329 | ⬜ |
| R125 | InplaceEditor ↔ 参考工具包 `editTriggers`/the reference text field 双击编辑 | [属性] 编辑触发硬编码双击（:581-593），无 editTriggers 配置 → 触发位掩码。 | 334 | ⬜ |
| R126 | InplaceEditor ↔ 参考工具包 `editTriggers`/the reference text field 双击编辑 | [操作] 无选区（:28-49）；点击不落光标；无 Home/End、无 Up/Down。 | 334 | ⬜ |
| R127 | InplaceEditor ↔ 参考工具包 `editTriggers`/the reference text field 双击编辑 | [组成] Delete 借 `insert_char('\u{ffff}')` 语义混入（:618-621）→ 独立 delete_forward API。 | 334 | ⬜ |
| R128 | input_validator ↔ 参考工具包 `validator` 家族 | [组成] 仅 Phone/Email 两个硬编码枚举（:24-32）：无 trait 扩展点、无 Int/Double 范围校验、无正则校验、无 `fixup()`。 | 339 | ⬜ |
| R129 | input_validator ↔ 参考工具包 `validator` 家族 | [语义] `check` 二态 bool+message（:60-88），参考工具包 为 Invalid/Intermediate/Acceptable 三态 → `ValidationState` 三态 + `Validator` trait，LineEdit 持 `Box<dyn Validator>`。 | 339 | ⬜ |
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
