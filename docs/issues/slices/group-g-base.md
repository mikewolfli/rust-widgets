| R483 | base.rs (`src/widget/base.rs`) ↔ 参考工具包 widget | [方法] `set_geometry` 不 clamp 到 min/max、不触发重绘（:228-230）→ clamp+request_draw。 | 980 | ⬜ |
| R484 | base.rs (`src/widget/base.rs`) ↔ 参考工具包 widget | [属性] min/max size 仅 advisory，布局不消费（:239-248）→ 布局器强制下限；`minimum_size_hint` 全库缺失（对应 参考工具包 sizeHint/minimumSizeHint 体系）。 | 980 | ⬜ |
| R485 | base.rs (`src/widget/base.rs`) ↔ 参考工具包 widget | [操作] show/hide 不传播子树（:467-475）→ 递归可见性或渲染期跳过隐藏子树。 | 980 | ⬜ |
| R486 | base.rs (`src/widget/base.rs`) ↔ 参考工具包 widget | [方法] dpi setter 不拒 NaN/非正值（:629-633）→ Result 或 debug_assert。 | 980 | ⬜ |
| R487 | base.rs (`src/widget/base.rs`) ↔ 参考工具包 widget | [已达成] runtime.rs 健康：RepaintMode Full/Dirty/Adaptive（runtime.rs:3502+）、AUTO_REPAINT_MIN_PIXELS=250_000 脏区阈值、原生控件 report_state 同步（:3074-3136）✓。 | 980 | ⬜ |
| R488 | registry.rs / census.rs | registry 是闭包转发表（registry.rs:21-53），所有权实际在 runtime——命名易误导，建议文档化或更名 factory_table。 | 987 | ⬜ |
| R489 | registry.rs / census.rs | census.rs canonical-name 全量口径 **188 控件为权威**（census.rs:34-37），本清单所有控件计数以此为基准。 | 987 | ⬜ |
| R493 | 绕过检查（冰山法则） | 220 处 `impl Widget`（其中 4 处测试桩）；6 处私有 pressed 字段均为文档化不同语义——**无实质绕过注册表** ✓。 | 996 | ⬜ |
| R494 | icon 体系 / svg / draw_bridge | icon_source/icon_data/icon_registry/icon_fallback_data：缺 icon 的 State/Mode 变体（on/off/disabled 图标组）与 @2x 高分档。 | 999 | ⬜ |
| R495 | icon 体系 / svg / draw_bridge | svg.rs 是导出通道非渲染器（职责正确）✓；draw_bridge 宏+覆盖率测试健康 ✓。 | 999 | ⬜ |
| R496 | text_utils / metrics / numeric / sample | **text_utils 缺共享 elide/wrap 原语——22 控件各自为战（grep elide 44 处），全库最大冰山重复** → 下沉 text_utils 供 Label/Button/Tab 等复用。 | 1003 | ⬜ |
| R497 | text_utils / metrics / numeric / sample | metrics.rs 缺 参考工具包 subControlRect 级子控件度量；焦点环色缺 outline token（metrics.rs:620-632）→ 补 token 防主题漂移。 | 1003 | ⬜ |
| R498 | text_utils / metrics / numeric / sample | sample_fill 不泄漏生产（sample_fill.rs:18-29）✓；apply() downcast 建议加全量类型覆盖锁测试。 | 1003 | ⬜ |
