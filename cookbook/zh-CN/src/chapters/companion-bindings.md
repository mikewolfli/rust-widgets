# 伴生控件绑定

**伴生控件**（companion）是编辑另一个控件状态、但不拥有文档的条带、菜单或栏。
`ColumnChooser`、`HeaderMenu`、`GridStatusBar`、`SummaryRow`、`TreeFindBar`、
`Breadcrumb` 与 `FindReplaceBar` 都是伴生控件。

所有配对的绑定形制相同；新增伴生控件时照此模板实现。

## 三步形制

1. **伴生控件暴露 source trait 或信号。** `TreeFindBar` 消费 `TreeSearchSource`；
   `FindReplaceBar` 通过信号报告意图；`HeaderMenu` 通过信号报告排序、过滤与隐藏请求。
   伴生控件从不持有文档。
2. **宿主增加一个 `attach_*` 方法。** 用 `ConnectionScope` 订阅，在附加时把伴生控件的
   token 解析到自己的列/行，并把请求放入共享队列。
3. **宿主每帧 poll 一次。** `poll_*` 排空队列并应用到自己的状态，因此伴生控件的一次点击
   无需应用层手工转发即可到达宿主。

## 宿主对应的附加方法

| 宿主 | 附加 | 轮询 |
|---|---|---|
| `TreeView` | `attach_tree_find_bar`、`attach_path_bar` | `poll_tree_find_bar`、`poll_path_bar` |
| `TreeTable` | `attach_tree_find_bar`、`attach_column_chooser`、`attach_header_menu`、`attach_grid_status_bar`、`attach_summary_row`、`attach_path_bar` | `poll_tree_find_bar`、`poll_column_chooser`、`poll_header_menu`、`poll_grid_status_bar`、`poll_summary_row`、`poll_path_bar` |
| `GridTable` | `attach_column_chooser`、`attach_header_menu`、`attach_grid_status_bar`、`attach_summary_row` | `poll_column_chooser`、`poll_header_menu`、`poll_grid_status_bar`、`poll_summary_row` |
| `DataGrid` | `attach_column_chooser`、`attach_header_menu`、`attach_grid_status_bar`、`attach_summary_row`、`attach_find_bar` | 对应的 `poll_*` / `poll_find_bar` |
| `RichEdit` | `attach_find_replace_bar` | `poll_find_replace_bar` |

## 保持接线诚实的规则

- **附加时解析 token。** 宿主不认识的 token 直接忽略，绝不猜测。
- **每个伴生控件只保留一个订阅。** 再次 `attach_*` 会替换旧绑定，不会留下两条链路。
- **只支持宿主真正能兑现的能力。** `TreeTable` 与 `GridTable` 忽略 `HeaderMenu` 的过滤请求：
  它们没有过滤模型，假装应用会报告画面并不存在的状态。
- **detach 保留最后状态。** `detach_*` 只丢弃订阅；伴生控件最后设置的排序、可见性或文本
  保留到宿主主动修改为止。

完整范例见 `TreeTable::attach_column_chooser` 与 `TreeTable::poll_column_chooser`；
header menu 链路使用同样的队列形制。
