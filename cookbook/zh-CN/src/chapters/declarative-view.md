# 声明式视图层

> **平台可用性。** `rust_widgets::view` 在 `desktop`、`tablet`、`mobile` 三档**默认编译**。
> 用 `no-declarative-view` 可将其从设备构建中排除：
>
> ```bash
> cargo build --no-default-features --features desktop,no-declarative-view
> ```
>
> `mini` 与 `embedded` 下该模块**不存在** —— 这两档用 `add_child` 与 `create_*`。
> 见[平台支持](platform-support.md)。

## 它做什么

把控件树描述为**状态的函数**，由库算出变了什么：

```rust
impl View for Counter {
    fn build(&self) -> Node {
        // 「给定 self，屏幕就是这个样子」
    }
}
```

```rust
engine.mount(&state, &create);                 // 建树
state.count += 1;
let report = engine.update(&state, &create);   // 一个 SetProperty，别无其它
```

## 为什么用它

没有它时，每个改状态的地方还得同时够到正确的控件、知道该设哪个属性。这份知识会散落到每个
修改点，而「`count == 3` 时屏幕该是什么样」在任何地方都得不到回答。有了它，这个问题只有
**一个答案**：`build`。

更新也随之变便宜、**变局部**：引擎把新树与旧树求差，只碰不同的部分，因此某个兄弟节点被编辑时，
控件的焦点、滚动偏移与内部状态依然存活 —— 这正是它与「每帧拆掉整棵树重建」的区别。

**保留式模型不变。** 控件仍是带 `ObjectId` 的长期对象，`add_child` 仍然可用，两种写法可在
同一应用里混用。本层增加的是对**结构**的描述，不取代任何东西。
（React、Flutter、SwiftUI 同样既是声明式又是保留式，原因一致：两者正交。）

### 何时值得用

| 用它的场景 | 不用的场景 |
|---|---|
| 树随状态变化，且你有一棵「上一次」的树可比 | 只建一次、永不改动 —— `JsonLoader::load` 已经做了这件事，对静态树求差纯属额外开销 |
| 你想要一个地方回答「UI 应该是什么样」 | 只设一个属性一次 —— `btn.set_text("x")` 比为产生一个 patch 而描述整棵树更省 |
| 多个控件由共享状态派生 | 逐帧动画 —— 那是 `PropertyAnimation` 的职责 |

## 怎么用

### 1. 实现 `View::build`

从状态返回一棵 `Node` 树：

```rust
use rust_widgets::view::{Node, View};
use rust_widgets::widget::capability::CapabilityValue;

struct Counter {
    count: i64,
    items: Vec<String>,
}

impl View for Counter {
    fn build(&self) -> Node {
        Node::new("group_box")
            .key("root")
            .child(
                Node::new("label")
                    .key("count")
                    .prop("text", CapabilityValue::String(format!("Count: {}", self.count))),
            )
            .children_of(
                self.items.iter().map(|item| {
                    Node::new("label")
                        .key(item.clone())          // 每个条目一个稳定身份
                        .prop("text", CapabilityValue::String(item.clone()))
                }),
            )
    }
}
```

`Node::new(name)` 接收**工厂名** —— 与 `JsonLoader` 相同的拼写
（`"group_box"`、`"label"`、`"listview"` …）。`prop` 接收控件公开的属性名，
所以拼错是**被拒绝的写入**，而不是静默无操作。

`build` 必须是**纯函数**：不读时钟、不用随机数。它可能被调用任意多次，
一个在调用之间会变化的值会让 diff 看到并非来自状态的变化。

### 2. 提供构造器

引擎不知道怎么造控件，由你告诉它：

```rust
use rust_widgets::core::{ObjectId, Rect};
use rust_widgets::view::Node;
use rust_widgets::widget::{runtime, Widget, WidgetFactory};

let factory = WidgetFactory::new_with_defaults();
let create = move |node: &Node| -> Option<ObjectId> {
    let widget: Box<dyn Widget> = factory.create(
        &node.widget,
        Rect::new(0, 0, 200, 28),
        node.key_str().unwrap_or("anon"),
    )?;
    runtime::register(widget)
};
```

它是**注入**的而非硬连，因此本层可以**无窗口**测试（它自己的测试就是这么跑的），
且有自己构造策略的宿主 —— 复用控件池的设计工具、必须打桩的测试 —— 不会被加载器的实现选择绑架。

### 3. 先 mount 一次，之后按变化 update

```rust
use rust_widgets::view::ViewEngine;

let mut state = Counter { count: 0, items: vec!["alpha".into(), "beta".into()] };
let mut engine = ViewEngine::new();

engine.mount(&state, &create);        // 建树

state.count = 1;
let report = engine.update(&state, &create);

assert_eq!(report.patches.len(), 1);  // 一个 SetProperty，别无其它
```

在 `mount` 之前调用 `update` 会改为 mount，所以不想区分首次与后续调用的调用方不必区分。

### 4. 给列表项加 `key`

`key` 是 diff 在重建之间认出**同一个控件**的依据。没有 key 时匹配退化为按位置 ——
此时在头部插入一项会移动其后每个节点的身份，把焦点与滚动状态转移到错误的控件上。

```rust
// 正确：头部插入不影响其余行。
.children_of(rows.iter().map(|r| Node::new("label").key(r.id)))

// 降级：按位置匹配，头部插入会重排其后全部身份。
.children_of(rows.iter().map(|r| Node::new("label")))
```

key 必须在兄弟间唯一。`Node::duplicate_sibling_keys()` 上报冲突，
`tools/check_view_keys_are_unique.sh` 遇到字面量重复直接失败。

降级是**被上报的，不是静默的** —— 读报告即可发现：

```rust
let report = engine.update(&state, &create);
if report.positional_matches > 0 {
    log::warn!("{} 个节点按位置匹配；请补 Node::key(..)", report.positional_matches);
}
```

| `DiffReport` 字段 | 含义 | 该怎么办 |
|---|---|---|
| `patches` | 要施加的变更 | — |
| `positional_matches` | 因**没有 `key`** 而按**位置**匹配的节点数 | 非零即「该补 key」 |
| `replaced_subtrees` | 因类型或 key 变化而整棵重建的子树 | 当某节点**确实**变成不同控件时，这是预期行为 |

## 由响应式状态驱动

`Binding<T>` 可以被任意线程 set，所以 `BindingListener` 要求 `Send`。
而 `ViewEngine` 及其控件是 `!Send`，因为控件注册表是 thread-local。
**因此监听器不能持有引擎** —— 这就是契约本身，它点名了唯一正确的设计：

```text
  工作线程                            UI 线程
  ─────────────                      ─────────
  binding.set(v)
    └─ 监听器触发   ──队列──▶  host.pump()
                                  └─ view.build()
                                     └─ diff → apply   （碰控件）
```

```rust
use rust_widgets::data_binding::Binding;
use rust_widgets::view::ReactiveHost;
use std::sync::Arc;

let text = Arc::new(Binding::new(String::from("first")));

// 视图借用 binding，因此每次 build 都会重读当前值。
struct Greeting<'a> { text: &'a Binding<String> }
// impl View for Greeting<'_> { .. }

let mut host = ReactiveHost::new(Greeting { text: &text }, Box::new(create));
host.mount();
host.subscribe(&text);

// 任意线程：
text.set(String::from("second"));

// UI 线程，通常每帧一次。返回本次重建次数；常见情况是 0，代价为一次无竞争加锁。
let rebuilds = host.pump();
```

用队列而不是原子标志，是为了让更新次数**等于** set 次数。标志会丢失中间值，
使「这次改动产生几个 patch」依赖时序 —— 测试与推理都不可靠。

## 把 patch 送到屏幕

`apply` 通过每个控件自身的属性契约写入。之后发生什么取决于你的宿主：

```rust
host.pump();
// 然后按你的表面策略重绘
```

> **关于局部重绘 —— 你什么都不用做。** `apply` 会写入每个控件自身的属性契约，
> 而这次写入最终落到该控件自己的 `request_redraw`，并在此记录 damage。
> 因此一次声明式更新产生的 damage 与手写更新完全相同：本层不需要标记任何矩形，
> 一个 view 也不需要特殊的重绘策略。
>
> 具体重绘多少，在你**挂载时**就替你决定好了。一个面积足够大、
> 承载不止一个控件、且确实请求过重绘的表面会被启用为 `RepaintMode::Adaptive`；
> 其余一律保持 `RepaintMode::Full`。`Adaptive` 会自我修正 ——
> 某一帧的 damage 覆盖整个表面时，该帧回退为整幅重绘，待 damage 缩小后自动恢复区域重绘 ——
> 所以这个决定不可能产出错误的一帧，最多只是一点有界的簿记开销。
> 你也可以用 `enable_damage_tracking_if_useful(window)` 显式询问，
> 或用 `set_repaint_mode` 强制指定策略。完整链路详见[性能与质量](performance-quality.md)。

## 无窗口测试

因为构造器是注入的，整层都可以在无显示环境下测试：

```rust
let mut engine = ViewEngine::new();
let ids = StubIds::new(10);          // 依次给出 10, 11, 12, …
engine.mount(&state, &ids.creator());

assert_eq!(engine.id_at(&[]), Some(10));   // 根
assert_eq!(engine.id_at(&[0]), Some(11));  // 其第一个子节点
```

`id_at(&[0, 2])` 回答「这还是我原本聚焦的那个控件吗？」，无需遍历树 ——
这正是断言「一次更新保住了身份」的方法。

## 组成

| 类型 | 是什么 |
|---|---|
| `Node` | **作为值来描述**的树：控件名、可选 `key`、属性、子节点。不含 id、不含活控件、不调用平台 API —— 这正是 `diff` 能是纯函数的原因 |
| `View` | 只有一个方法 `build(&self) -> Node` 的 trait |
| `ViewEngine` | 持有上一棵树，调用 `diff`，并应用结果 |
| `Patch` | 一个变更：`SetProperty` / `Insert` / `Remove` / `Move` / `Replace` |
| `ReactiveHost` | 把 `Binding` 接到引擎，使任意线程的 `set` 都能驱动 `update` |

`Patch::SetProperty` 落在每个控件**自身**公开的属性契约上 —— 与 JSON 加载器写入的是同一套 ——
因此声明式层不可能凭空发明一个控件并不具备的属性。`apply` 是本层中唯一会 mutate 的函数。

## 它不做什么

- **没有组件模型** —— 没有 `context`、没有 hooks、没有组件边界。控件是扁平的
  `WidgetKind`；引入那些抽象等于为一个本不存在的层级造抽象。
- **没有异步/并发 diff** —— 控件是 `!Send`，diff 必须在 UI 线程。
- **没有动画插值** —— 动画是 `PropertyAnimation` 的职责。本层表达**目标**状态；
  混入时间会让 diff 不确定。
- **diff 不处理样式级联** —— `Patch::SetProperty` 覆盖属性；样式表由 CSS 层独立处理。
- **不在 `mini` / `embedded` 上**，也**不经 C ABI 暴露** —— 仅 Rust。
