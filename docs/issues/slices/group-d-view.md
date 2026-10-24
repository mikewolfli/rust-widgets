| R192 | DataSource ↔ 参考工具包 `abstract item model` | [组成] 仅 `IncrementalTableDataSource` 单一 pull 窗口 trait（:17-96）：无 model index/parent/role 寻址、无 proxy 基类 → 引入 role 参数与 (parent,column) 寻址。 | 441 | ⬜ |
| R193 | DataSource ↔ 参考工具包 `abstract item model` | [方法/事件] 仅 `revision()` + 单一 `data_changed_signal`（:60-66）粗粒度通知；无 beginInsertRows/dataChanged(row,col) 细粒度信号族 → 订阅方只能整窗重取。 | 441 | ⬜ |
| R194 | DataSource ↔ 参考工具包 `abstract item model` | [操作] 无 `setData`/flags/mime —— **模型只读单向，是全项目编辑生态缺失的根因**。 | 441 | ⬜ |
| R195 | FilterExpr ↔ `sort filter proxy model` 谓词 | 10 算子+组合树 ≈；无 caseSensitivity 等开关属性；无 changed 信号；每行全表达式求值无缓存 → 10 万行按列建位图索引。 | 446 | ⬜ |
| R196 | DataGrid ↔ table view+Proxy | [方法] **排序/过滤仅作用于可见窗口而非全模型**（:488-518）——违反代理模型"全模型排序视图"语义 → 下沉数据源层或独立排序投影。 | 449 | ⬜ |
| R197 | DataGrid ↔ table view+Proxy | [属性] 单元格单选；无列宽模式/行高/交替行色/editTriggers。 | 449 | ⬜ |
| R198 | DataGrid ↔ table view+Proxy | [组成] 预留表头但**不绘制列标签、不可点击**（:35, 864-916）；**冻结列仅画线装饰（:941-950），cell_rect 未锁定——伪功能** → 实做或删除。 | 449 | ⬜ |
| R199 | DataGrid ↔ table view+Proxy | [操作] handle_event（:954-1010）无键盘、无右键、无编辑；仅垂直滚轮，无水平滚动、无滚动条。 | 449 | ⬜ |
| R200 | GridTableWidget ↔ table view/header view | [事件] cell_selected/cell_double_clicked/sort_changed/header_clicked 信号齐 ≈全项目最接近 table widget。 | 455 | ⬜ |
| R201 | GridTableWidget ↔ table view/header view | [属性] 列宽拖拽调整仅 Interactive 一档；缺三态 setSectionResizeMode；选择模式仅单选；无 Stretch/Fixed/交替行色。 | 455 | ⬜ |
| R202 | GridTableWidget ↔ table view/header view | [操作] 仅方向键滚动（:1180-1187）；无 Home/End/PageUp/首字跳转/多选/编辑/右键。 | 455 | ⬜ |
| R203 | GridTableWidget ↔ table view/header view | [丝滑] 表头扁平无 the reference table view 厚表头/压纹；无滚动条；重绘全量。 | 455 | ⬜ |
| R204 | TableWidget ↔ table view+styled item delegate | [组成] **ItemDelegate 仅 create/set/get 三方法（:73-85），无 paint() 钩子、无 sizeHint/updateEditorGeometry/closeEditor** → checkbox 列/进度条单元格/富文本等 delegate 生态核心价值不可实现。 | 461 | ⬜ |
| R205 | TableWidget ↔ table view+styled item delegate | [属性] 无 editTriggers；编辑靠手动 begin/commit/cancel（:440-481）程序化驱动。 | 461 | ⬜ |
| R206 | TableWidget ↔ table view+styled item delegate | [事件] handle_event（:799-836）**无滚轮、无键盘、无双击触发编辑** → 超一屏内容不可达。 | 461 | ⬜ |
| R207 | VirtualTable | [组成] 无选择/表头行/委托——定位非交互；滚轮=1 行/格，无 PageUp/PageDown 倍率；无滚动条使 10 万行只能逐格轮滚。 | 466 | ⬜ |
| R208 | ListView ↔ list view | [组成] **全项目唯一 SelectionModel**（:186-334：Single/Multi/Extended/None、anchor、Shift 范围、Ctrl 切换）但仅行索引、未接入其他控件。 | 469 | ⬜ |
| R209 | ListView ↔ list view | [丝滑] **完全无滚动**：row_rect 越界返回 None（:712-726）、handle_event 无滚轮分支（:1084-1122）→ 内容超出视口即不可达（本文件最大缺口）。 | 469 | ⬜ |
| R210 | ListView ↔ list view | [操作] 无键盘；Icon 模式只画文本块无图标内容；无拖拽重排/右键/双击信号。 | 469 | ⬜ |
| R211 | VirtualList | 窗口化 fetch+overscan+虚拟化测试 ≈正确（:263-274, 734-767）；选择只改字段无 selectionChanged 信号；无键盘；无滚动条。 | 474 | ⬜ |
| R212 | TreeView ↔ tree view | [组成] TreeModel=扁平 DFS+NodeView 投影（:122-161, 347-413）——非 参考工具包 canFetchMore 惰性树 → 大树支持惰性拉取。 | 477 | ⬜ |
| R213 | TreeView ↔ tree view | [事件] **无 expanded/collapsed/activated/doubleClicked 信号**（参考工具包 核心信号）。 | 477 | ⬜ |
| R214 | TreeView ↔ tree view | [操作] 单选；键盘仅 Right/Left（:1121-1128），无 Up/Down 移焦/Home/End；无双击展开。 | 477 | ⬜ |
| R215 | TreeView ↔ tree view | [丝滑] **无滚动**（:575-601）——树超出视口不可达；无展开动画（tree view::setAnimated 默认开）；无 source-list 侧栏样式。 | 477 | ⬜ |
| R216 | TreeTable ↔ tree widget 多列 | [组成] expanded HashSet 全量投影（:103, 328-348）——10 万节点展开 O(全树) → 窗口化投影。 | 483 | ⬜ |
| R217 | TreeTable ↔ tree widget 多列 | [操作] **无键盘、无滚轮**（:386-394）、无右键；无表头（列名无处显示）、无排序、无缩进/行高属性。 | 483 | ⬜ |
| R218 | TreeTable ↔ tree widget 多列 | [事件] selection_changed 折叠清选广播正确 ≈；缺 expansion_changed。 | 483 | ⬜ |
| R219 | ImageGallery ↔ list view(IconMode) | [动画] PropertyDriver/MotionSlot 淡入淡出——**全项目唯一控件级动画**，机制应推广。 | 488 | ⬜ |
| R220 | ImageGallery ↔ list view(IconMode) | [操作] 方向键翻页成立；无滚轮翻页、无双击全屏、无多选、无 zoom。 | 488 | ⬜ |
| R221 | ImageGallery ↔ list view(IconMode) | [组成] 缩略图为**占位图形、无真实图片解码**（:851-861）→ 对比 image reader 异步管线。 | 488 | ⬜ |
| R222 | PropertiesPanel / PropertyGrid / QueryBuilder | PropertiesPanel：仅 Bool 支持点击切换（:390-400, :796）；Text/Number/Choice 无编辑器；content_box+clip+first_visible_row 窗口化 ≈质量最高之一；滚轮 delta*3 无行数语义；无键盘。 | 493 | ⬜ |
| R223 | PropertiesPanel / PropertyGrid / QueryBuilder | PropertyGrid：**斑马纹行是全项目唯一交替行色**（:371, 467-471）→ 应抽共享绘制层推广；键盘含 **Home/End（:644-700，唯一实现）**；无 PageUp/双击编辑/右键/cellChanged。 | 493 | ⬜ |
| R224 | PropertiesPanel / PropertyGrid / QueryBuilder | QueryBuilder：一层深度输出（:176-193, 402-462），`(a AND b) OR c` 嵌套无 UI；**无滚动**（:699-703 行超出底边直接不画）；swap_rows 仅程序化无拖拽；无 rowsChanged 信号。 | 493 | ⬜ |
