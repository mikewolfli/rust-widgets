| R001 | Button (`src/widget/base_widgets/button.rs`) ↔ 参考工具包 `push button`/`abstract button` | [属性] 缺 `shortcut`、`menu`（下拉挂载）、`autoDefault`/`autoExclusive`、独立 `flat` 属性；`iconSize` 固定为 `dimensions::BUTTON_ICON_SIZE`（button.rs:214-229）不可调；文本仅单行 → 增 `set_icon_size`、`shortcut: Option<Vec<u32>>`、`menu` 属性与 hit-test 弹出。 | 138 | ⬜ |
| R002 | Button (`src/widget/base_widgets/button.rs`) ↔ 参考工具包 `push button`/`abstract button` | [方法] 缺 `set_menu/set_shortcut/set_auto_default` 等价 API。 | 138 | ⬜ |
| R003 | Button (`src/widget/base_widgets/button.rs`) ↔ 参考工具包 `push button`/`abstract button` | [操作] 缺右键菜单、DnD。 | 138 | ⬜ |
| R004 | Button (`src/widget/base_widgets/button.rs`) ↔ 参考工具包 `push button`/`abstract button` | [动画] PropertyDriver hover=0.5/press=1.0（button.rs:96, 377-388）优于 参考工具包 ≈持平偏优。 | 138 | ⬜ |
| R005 | Button (`src/widget/base_widgets/button.rs`) ↔ 参考工具包 `push button`/`abstract button` | [丝滑] 文本 fitted 绘制；缺 `参考工具包::Elide*` 等省略等级控制。 | 138 | ⬜ |
| R006 | Button (`src/widget/base_widgets/button.rs`) ↔ 参考工具包 `push button`/`abstract button` | [macOS] default 强调环（:1117）贴合 HIG；flat 形态缺失。 | 138 | ⬜ |
| R007 | Checkbox (`base_widgets/checkbox.rs`) ↔ 参考工具包 `check box` | [属性] 三态 CheckState（:53-60）覆盖 参考工具包 PartiallyChecked；缺 `autoExclusive`。 | 146 | ⬜ |
| R008 | Checkbox (`base_widgets/checkbox.rs`) ↔ 参考工具包 `check box` | [组成/操作] **draw 无焦点环分支**（:516-643 无 focus ring）→ Tab 后无可见键盘焦点即不可用键盘导航（HIG 硬伤）；参照 radiobutton.rs:617-630 已有实现补齐。 | 146 | ⬜ |
| R009 | Checkbox (`base_widgets/checkbox.rs`) ↔ 参考工具包 `check box` | [动画] 无按压/hover 过渡，库内 Button 有 → 库内不一致（见共性 1）。 | 146 | ⬜ |
| R010 | Frame (`base_widgets/frame.rs`) ↔ 参考工具包 `frame` | [属性] 6 shape+3 shadow（:31-107）≈参考工具包；Box raised/sunken 的 bevel 硬编码 1px 忽略 `line_width`（:293, 334）；Box/WinPanel 不读 `border_radius` → bevel 宽度=`line_width`，shape 绘制统一消费 border_radius。 | 151 | ⬜ |
| R011 | Frame (`base_widgets/frame.rs`) ↔ 参考工具包 `frame` | [方法] 缺 `contentsMargins`、独立 frameWidth 语义 → 增 margin 并在子内容布局中扣除。 | 151 | ⬜ |
| R012 | Frame (`base_widgets/frame.rs`) ↔ 参考工具包 `frame` | [macOS/3D] bevel 用无主题字面量色（:284-288）→ 改主题派生；Bevel 仿光影（:255-349）是库内唯一光影表达。 | 151 | ⬜ |
| R013 | Label (`base_widgets/label.rs`) ↔ 参考工具包 `label` | [组成] 仅单行文本；参考工具包 label 支持 pixmap/富文本 → 内容模型差距最大的控件之一。 | 156 | ⬜ |
| R014 | Label (`base_widgets/label.rs`) ↔ 参考工具包 `label` | [属性] 仅 text+alignment（:36-74）；缺 `wordWrap`、`textFormat`、`pixmap`/`scaledContents`、`margin`/`indent`、`openExternalLinks`、`selectedText`、`buddy` → 至少先加 `word_wrap` 与 `margin`（换行原语可复用 alert_dialogs.rs:207 `wrap_words`）。 | 156 | ⬜ |
| R015 | Label (`base_widgets/label.rs`) ↔ 参考工具包 `label` | [丝滑] `draw_text` 无省略无换行（:238-244），长文本溢出破坏布局 → 加 elide+wrap 路径。 | 156 | ⬜ |
| R016 | Radiobutton (`base_widgets/radiobutton.rs`) ↔ 参考工具包 `radio button`/`button group` | [属性] `group_id` 仅存储（:212-218）；`select_in_group` 需调用方传 peers（:242-254）→ 缺 参考工具包 `autoExclusive`（同父自动排他）与 button group 等价管理器 → 库内按 group_id 注册表，点击自动互斥。 | 161 | ⬜ |
| R017 | Radiobutton (`base_widgets/radiobutton.rs`) ↔ 参考工具包 `radio button`/`button group` | [操作] 缺 参考工具包 组内箭头键导航（↑↓←→ 在同组移动并选中）→ 有焦点环（:617-630）基础上增 ArrowKey 按 group_id 找 peers。 | 161 | ⬜ |
| R018 | Radiobutton (`base_widgets/radiobutton.rs`) ↔ 参考工具包 `radio button`/`button group` | [动画] 无 hover/press 过渡（共性 1）。 | 161 | ⬜ |
| R019 | ToggleButton (`base_widgets/toggle_button.rs`) ↔ 参考工具包 `tool button`(checkable) | [属性] `auto_exclusive` 仅意图标志不强制（:172-178）；缺 `icon`/`arrowType`/`menu`/`popupMode`/`autoRaise`。 | 166 | ⬜ |
| R020 | ToggleButton (`base_widgets/toggle_button.rs`) ↔ 参考工具包 `tool button`(checkable) | [事件] **handle_event 无 KeyPress 臂（:585-630）→ 无键盘激活**；tool button Space/Enter 可 toggle → 补 Space/Enter 臂（Button :909-911 有现成范式）。 | 166 | ⬜ |
| R021 | ToggleButton (`base_widgets/toggle_button.rs`) ↔ 参考工具包 `tool button`(checkable) | [操作] 键盘不可达 = a11y 硬伤。 | 166 | ⬜ |
| R022 | ToggleButton (`base_widgets/toggle_button.rs`) ↔ 参考工具包 `tool button`(checkable) | [动画] PropertyDriver（:92, 243-264）≈持平偏优。 | 166 | ⬜ |
