# 宣告式檢視層

> **平台可用性。** `rust_widgets::view` 在 `desktop`、`tablet`、`mobile` 三檔**預設編譯**。
> 用 `no-declarative-view` 可將其從裝置建置中排除：
>
> ```bash
> cargo build --no-default-features --features desktop,no-declarative-view
> ```
>
> `mini` 與 `embedded` 下該模組**不存在** —— 這兩檔用 `add_child` 與 `create_*`。
> 見[平台支援](platform-support.md)。

## 它做什麼

把控制項樹描述為**狀態的函式**，由函式庫算出變了什麼：

```rust
impl View for Counter {
    fn build(&self) -> Node {
        // 「給定 self，畫面就是這個樣子」
    }
}
```

```rust
engine.mount(&state, &create);                 // 建樹
state.count += 1;
let report = engine.update(&state, &create);   // 一個 SetProperty，別無其它
```

## 為什麼用它

沒有它時，每個改狀態的地方還得同時夠到正確的控制項、知道該設哪個屬性。這份知識會散落到每個
修改點，而「`count == 3` 時畫面該是什麼樣」在任何地方都得不到回答。有了它，這個問題只有
**一個答案**：`build`。

更新也隨之變便宜、**變局部**：引擎把新樹與舊樹求差，只碰不同的部分，因此某個兄弟節點被編輯時，
控制項的焦點、捲動偏移與內部狀態依然存活 —— 這正是它與「每幀拆掉整棵樹重建」的區別。

**保留式模型不變。** 控制項仍是帶 `ObjectId` 的長期物件，`add_child` 仍然可用，兩種寫法可在
同一應用裡混用。本層增加的是對**結構**的描述，不取代任何東西。
（宣告式與保留式是兩條正交的軸：宣告式層描述結構，保留樹持有活控件，
應用可以同時用兩者。）

### 何時值得用

| 用它的場景 | 不用的場景 |
|---|---|
| 樹隨狀態變化，且你有一棵「上一次」的樹可比 | 只建一次、永不改動 —— `JsonLoader::load` 已經做了這件事，對靜態樹求差純屬額外開銷 |
| 你想要一個地方回答「UI 應該是什麼樣」 | 只設一個屬性一次 —— `btn.set_text("x")` 比為產生一個 patch 而描述整棵樹更省 |
| 多個控制項由共享狀態派生 | 逐幀動畫 —— 那是 `PropertyAnimation` 的職責 |

## 怎麼用

### 1. 實作 `View::build`

從狀態回傳一棵 `Node` 樹：

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
                        .key(item.clone())          // 每個條目一個穩定身分
                        .prop("text", CapabilityValue::String(item.clone()))
                }),
            )
    }
}
```

`Node::new(name)` 接收**工廠名** —— 與 `JsonLoader` 相同的拼寫
（`"group_box"`、`"label"`、`"listview"` …）。`prop` 接收控制項公開的屬性名，
所以拼錯是**被拒絕的寫入**，而不是靜默無操作。

`build` 必須是**純函式**：不讀時鐘、不用隨機數。它可能被呼叫任意多次，
一個在呼叫之間會變化的值會讓 diff 看到並非來自狀態的變化。

### 2. 提供建構器

引擎不知道怎麼造控制項，由你告訴它：

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

它是**注入**的而非硬連，因此本層可以**無視窗**測試（它自己的測試就是這麼跑的），
且有自己建構策略的宿主 —— 重用控制項池的設計工具、必須打樁的測試 —— 不會被載入器的實作選擇綁架。

### 3. 先 mount 一次，之後按變化 update

```rust
use rust_widgets::view::ViewEngine;

let mut state = Counter { count: 0, items: vec!["alpha".into(), "beta".into()] };
let mut engine = ViewEngine::new();

engine.mount(&state, &create);        // 建樹

state.count = 1;
let report = engine.update(&state, &create);

assert_eq!(report.patches.len(), 1);  // 一個 SetProperty，別無其它
```

在 `mount` 之前呼叫 `update` 會改為 mount，所以不想區分首次與後續呼叫的呼叫方不必區分。

### 4. 給列表項加 `key`

`key` 是 diff 在重建之間認出**同一個控制項**的依據。沒有 key 時匹配退化為按位置 ——
此時在頭部插入一項會移動其後每個節點的身分，把焦點與捲動狀態轉移到錯誤的控制項上。

```rust
// 正確：頭部插入不影響其餘列。
.children_of(rows.iter().map(|r| Node::new("label").key(r.id)))

// 降級：按位置匹配，頭部插入會重排其後全部身分。
.children_of(rows.iter().map(|r| Node::new("label")))
```

key 必須在兄弟間唯一。`Node::duplicate_sibling_keys()` 上報衝突，
`tools/check_view_keys_are_unique.sh` 遇到字面量重複直接失敗。

降級是**被上報的，不是靜默的** —— 讀報告即可發現：

```rust
let report = engine.update(&state, &create);
if report.positional_matches > 0 {
    log::warn!("{} 個節點按位置匹配；請補 Node::key(..)", report.positional_matches);
}
```

| `DiffReport` 欄位 | 含義 | 該怎麼辦 |
|---|---|---|
| `patches` | 要套用的變更 | — |
| `positional_matches` | 因**沒有 `key`** 而按**位置**匹配的節點數 | 非零即「該補 key」 |
| `replaced_subtrees` | 因型別或 key 變化而整棵重建的子樹 | 當某節點**確實**變成不同控制項時，這是預期行為 |

## 由狀態驅動

`Binding<T>` 可以被任意執行緒 set，所以 `BindingListener` 要求 `Send`。
而 `ViewEngine` 及其控制項是 `!Send`，因為控制項註冊表是 thread-local。
**因此監聽器不能持有引擎** —— 這就是契約本身，它點名了唯一正確的設計：

```text
  工作執行緒                          UI 執行緒
  ─────────────                      ─────────
  binding.set(v)
    └─ 監聽器觸發   ──佇列──▶  host.pump()
                                  └─ view.build()
                                     └─ diff → apply   （碰控制項）
```

```rust
use rust_widgets::data_binding::Binding;
use rust_widgets::view::ReactiveHost;
use std::sync::Arc;

let text = Arc::new(Binding::new(String::from("first")));

// 檢視借用 binding，因此每次 build 都會重讀當前值。
struct Greeting<'a> { text: &'a Binding<String> }
// impl View for Greeting<'_> { .. }

let mut host = ReactiveHost::new(Greeting { text: &text }, Box::new(create));
host.mount();
host.subscribe(&text);

// 任意執行緒：
text.set(String::from("second"));

// UI 執行緒，通常每幀一次。回傳本次重建次數；常見情況是 0，代價為一次無競爭加鎖。
let rebuilds = host.pump();
```

用佇列而不是原子旗標，是為了讓更新次數**等於** set 次數。旗標會丟失中間值，
使「這次改動產生幾個 patch」依賴時序 —— 測試與推理都不可靠。

## 把 patch 送到畫面

`apply` 透過每個控制項自身的屬性契約寫入。之後發生什麼取決於你的宿主：

```rust
host.pump();
// 然後按你的表面策略重繪
```

> **關於局部重繪 —— 你什麼都不用做。** `apply` 會寫入每個控件自身的屬性契約，
> 而這次寫入最終落到該控件自己的 `request_redraw`，並在此記錄 damage。
> 因此一次宣告式更新產生的 damage 與手寫更新完全相同：本層不需要標記任何矩形，
> 一個 view 也不需要特殊的重繪策略。
>
> 具體重繪多少，在你**掛載時**就替你決定好了。一個面積足夠大、
> 承載不止一個控件、且確實請求過重繪的表面會被啟用為 `RepaintMode::Adaptive`；
> 其餘一律保持 `RepaintMode::Full`。`Adaptive` 會自我修正 ——
> 某一幀的 damage 覆蓋整個表面時，該幀回退為整幅重繪，待 damage 縮小後自動恢復區域重繪 ——
> 所以這個決定不可能產出錯誤的一幀，最多只是一點有界的簿記開銷。
> 你也可以用 `enable_damage_tracking_if_useful(window)` 顯式詢問，
> 或用 `set_repaint_mode` 強制指定策略。完整鏈路詳見[效能與品質](performance-quality.md)。

## 無視窗測試

因為建構器是注入的，整層都可以在無顯示環境下測試：

```rust
let mut engine = ViewEngine::new();
let ids = StubIds::new(10);          // 依次給出 10, 11, 12, …
engine.mount(&state, &ids.creator());

assert_eq!(engine.id_at(&[]), Some(10));   // 根
assert_eq!(engine.id_at(&[0]), Some(11));  // 其第一個子節點
```

`id_at(&[0, 2])` 回答「這還是我原本聚焦的那個控制項嗎？」，無需遍歷樹 ——
這正是斷言「一次更新保住了身分」的方法。

## 組成

| 型別 | 是什麼 |
|---|---|
| `Node` | **作為值來描述**的樹：控制項名、可選 `key`、屬性、子節點。不含 id、不含活控制項、不呼叫平台 API —— 這正是 `diff` 能是純函式的原因 |
| `View` | 只有一個方法 `build(&self) -> Node` 的 trait |
| `ViewEngine` | 持有上一棵樹，呼叫 `diff`，並套用結果 |
| `Patch` | 一個變更：`SetProperty` / `Insert` / `Remove` / `Move` / `Replace` |
| `ReactiveHost` | 把 `Binding` 接到引擎，使任意執行緒的 `set` 都能驅動 `update` |

`Patch::SetProperty` 落在每個控制項**自身**公開的屬性契約上 —— 與 JSON 載入器寫入的是同一套 ——
因此宣告式層不可能憑空發明一個控制項並不具備的屬性。`apply` 是本層中唯一會 mutate 的函式。

## 它不做什麼

- **沒有元件模型** —— 沒有 `context`、沒有 hooks、沒有元件邊界。控制項是扁平的
  `WidgetKind`；引入那些抽象等於為一個本不存在的層級造抽象。
- **沒有非同步/並行 diff** —— 控制項是 `!Send`，diff 必須在 UI 執行緒。
- **沒有動畫插值** —— 動畫是 `PropertyAnimation` 的職責。本層表達**目標**狀態；
  混入時間會讓 diff 不確定。
- **diff 不處理樣式串聯** —— `Patch::SetProperty` 覆蓋屬性；樣式表由 CSS 層獨立處理。
- **不在 `mini` / `embedded` 上**，也**不經 C ABI 暴露** —— 僅 Rust。
