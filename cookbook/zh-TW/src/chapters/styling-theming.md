# 樣式與主題

`rust-widgets` 的樣式系統透過階層解析鏈來解析控制項外觀：**Theme → ThemeOverrides → StatefulTheme → Inline CSS**。系統包含具有選擇器引擎與特定性的 CSS 解析器、可熱重新載入的樣式表、具備 10 種緩動函數與物理彈簧的動畫框架，以及內建的 Material Light/Dark 主題，提供全面的視覺自訂能力。

---

## 階層解析鏈

控制項樣式透過四個層級解析，當屬性未設定時會依序向下遞補：

```
1. 全域主題預設值（ThemeManager → Theme）
2. 每個控制項類別的 ThemeOverrides（例如「Button」、「Label」）
3. 控制項實例狀態（StatefulTheme → WidgetState）
4. 行內樣式覆蓋（來自 StyleSheetManager 的 CSS）
```

---

## `WidgetStyle` — 已解析的視覺屬性

```rust
pub struct WidgetStyle {
    pub background_color: Option<Color>,
    pub background_gradient: Option<Gradient>,
    pub text_color: Option<Color>,
    pub font: Option<Font>,
    pub border_color: Option<Color>,
    pub border_width: u32,
    pub border_radius: u32,
    pub padding: Padding,
    pub margin: Margin,
    pub shadow: Option<Shadow>,
    pub touch_target: Option<Size>,
    pub opacity: Option<f32>,
}
```

Builder 方法提供流暢的建構方式：

```rust
let style = WidgetStyle::default()
    .with_background(Color::from_rgb(33, 150, 243))
    .with_text_color(Color::WHITE)
    .with_font(Font::bold("Arial", 14.0))
    .with_border(Color::from_rgb(25, 118, 210), 2, 8)
    .with_padding(Padding::symmetric(6, 12))
    .with_margin(Margin::all(4))
    .with_shadow(Shadow::new().with_offset(0, 2).with_blur(4).with_color(Color::rgba(0, 0, 0, 60)))
    .with_opacity(0.95)
    .with_touch_target(Size::new(44, 44));
```

**樣式繼承** — 子層級對未設定的屬性回退到父層級：

```rust
let parent = WidgetStyle::default()
    .with_background(Color::RED)
    .with_text_color(Color::BLUE);

let child = WidgetStyle::default();  // 全部為 None

let inherited = child.inherit_from(&parent);
// inherited.background_color == Some(RED)
// inherited.text_color == Some(BLUE)

// 子層級屬性覆蓋父層級
let child = WidgetStyle::default().with_background(Color::GREEN);
let inherited = child.inherit_from(&parent);
// inherited.background_color == Some(GREEN)  — 子層級勝出
```

**樣式合併** — 從另一個樣式填入缺失的屬性：

```rust
let mut base = WidgetStyle::default().with_background(Color::RED);
let overlay = WidgetStyle::default().with_text_color(Color::BLUE);
base.merge(&overlay);
// base.background_color == Some(RED)   — 未變更
// base.text_color == Some(BLUE)         — 從 overlay 填入
```

---

## Padding、Margin 與 Shadow

### `Padding`

控制項邊框與內容之間的各邊內距：

```rust
let padding = Padding::new(8, 12, 8, 12);      // 上、右、下、左
let uniform = Padding::all(4);                   // 所有邊 = 4
let symmetric = Padding::symmetric(6, 12);        // 垂直 = 6，水平 = 12
let safe = Padding::normalized(-1, 4, -99, 8);   // 將負值限制為 → (0, 4, 0, 8)
```

### `Margin`

控制項邊框外部的各邊外距：

```rust
let margin = Margin::new(8, 4, 8, 4);
let uniform = Margin::all(8);
let symmetric = Margin::symmetric(4, 8);
let safe = Margin::normalized(-5, 0, 3, 2);  // → (0, 0, 3, 2)
```

### `Shadow`

控制項上的投影效果：

```rust
let shadow = Shadow::new()
    .with_offset(0, 4)                   // 陰影在下方 4px
    .with_blur(8)                        // 8px 模糊半徑
    .with_color(Color::rgba(0, 0, 0, 60)); // 半透明黑色
```

---

## CSS 系統

### CSS 解析器

`CssParser` 將 CSS 文字轉換為 `StyleSheet`，支援選擇器比對與屬性套用：

```rust
use rust_widgets::style::css::CssParser;

let css = r#"
    Button {
        background-color: #2196f3;
        text-color: #ffffff;
        border-radius: 8;
        border-width: 0;
    }

    .primary {
        background-color: #0066cc;
        text-color: #ffffff;
    }

    #submit-btn {
        background-color: #4caf50;
        font-size: 16;
    }

    Button:hover {
        background-color: #1976d2;
    }

    Button:pressed {
        background-color: #1565c0;
    }
"#;

let sheet = CssParser::parse(css).unwrap();

// 套用至控制項
let mut style = WidgetStyle::default();
CssParser::parse_and_apply(css, "Button", Some("primary"), Some("submit-btn"), Some(PseudoState::Hover), &mut style)?;
```

### `CssSelector` — 選擇器引擎

選擇器引擎支援型別、類別、ID、狀態偽類別以及 AND 組合：

```rust
pub enum CssSelector {
    Universal,                              // *
    Kind(String),                           // "Button"
    Class(String),                          // ".primary"
    Id(String),                             // "#submit"
    State(PseudoState),                     // :hover、:pressed、:disabled
    And(Vec<CssSelector>),                  // Button.primary:hover
}
```

**特定性規則：**
- ID 選擇器（`#id`）— 最高特定性
- 類別選擇器（`.class`）— 中等
- 型別選擇器（`Widget`）— 低
- 通用選擇器（`*`）— 最低
- 偽狀態（`:hover`）— 附加計算

當多條規則同時符合時，每個屬性由特定性最高者勝出。

---

## `StyleSheetManager` — 全域註冊表

具備優先級分層的全域執行緒安全樣式表註冊表：

```rust
use rust_widgets::style::stylesheet::{global_stylesheet_manager, StyleSheetManager};

// 使用優先級註冊具名樣式表
{
    let mut mgr = global_stylesheet_manager();
    mgr.register("material-base", r#"
        Button {
            background-color: #2196f3;
            border-radius: 4;
        }
        Label {
            text-color: #212121;
        }
    "#, 0);  // 優先級 0（基底）

    mgr.register("brand-override", r#"
        .accent {
            background-color: #ff6f00;
        }
    "#, 100);  // 優先級 100（覆蓋）
}

// 將所有符合規則套用至控制項樣式
{
    let mgr = global_stylesheet_manager();
    let mut style = WidgetStyle::default();
    mgr.apply_to("Button", Some("accent"), None, Some(PseudoState::Normal), &mut style)?;
}

// 取消註冊樣式表
{
    let mut mgr = global_stylesheet_manager();
    mgr.unregister("brand-override");
}
```

**優先級分層：** 樣式表在套用前會依 `priority`（升序）排序。較低優先級的值會先被套用，較高優先級的值則會覆蓋它們。

---

## `CssWatcher` — 熱重新載入 CSS 檔案

基於輪詢的檔案監看器，可在 CSS 檔案變更時自動重新載入至全域 `StyleSheetManager`：

```rust
use rust_widgets::style::css_watcher::CssWatcher;

let mut watcher = CssWatcher::new("theme.css", "main-theme");
watcher.set_poll_interval(500);  // 每 500ms 檢查一次（預設值）

// 在主迴圈中輪詢
loop {
    match watcher.poll() {
        Ok(true)  => println!("CSS 已重新載入 — 樣式已更新！"),
        Ok(false) => { /* 無變更 */ }
        Err(e)    => eprintln!("CSS 監看錯誤：{}", e),
    }

    // ... 渲染影格 ...

    std::thread::sleep(std::time::Duration::from_millis(16));
}

// 忽略修改時間強制重新載入
watcher.reload()?;
```

---

## 動畫系統

### 緩動函數 — 10 種預設

```rust
use rust_widgets::style::animation::EasingFunction;

let t = 0.5;  // 進度 [0.0, 1.0]

let linear    = EasingFunction::Linear.apply(t);      // t
let ease_in   = EasingFunction::EaseIn.apply(t);      // t²
let ease_out  = EasingFunction::EaseOut.apply(t);     // 1 - (1-t)²
let ease_inout= EasingFunction::EaseInOut.apply(t);   // 平滑 S 曲線

// 彈跳
let bounce_in = EasingFunction::BounceIn.apply(t);    // 超調 + 穩定
let bounce_out= EasingFunction::BounceOut.apply(t);

// 彈性
let elastic_in  = EasingFunction::ElasticIn.apply(t); // 彈簧般
let elastic_out = EasingFunction::ElasticOut.apply(t);

// 回退（超調）
let back_in  = EasingFunction::BackIn.apply(t);       // 超調後進入
let back_out = EasingFunction::BackOut.apply(t);      // 進入後超調
```

### `AnimationConfig`

控制動畫的 timing 與行為：

```rust
use rust_widgets::style::animation::{AnimationConfig, AnimationDirection, AnimationFillMode};

let config = AnimationConfig::new(Duration::from_millis(300))
    .with_delay(Duration::from_millis(100))
    .with_easing(EasingFunction::EaseInOut)
    .with_direction(AnimationDirection::Alternate)
    .with_fill_mode(AnimationFillMode::Forwards)
    .with_iterations(3);  // 播放 3 次

let infinite = AnimationConfig::new(Duration::from_millis(2000)).infinite();
```

### `AnimationDriver`

核心動畫狀態機：

```rust
let mut anim = Animation::new(config);
anim.on_complete(Box::new(|| println!("動畫已完成！")));

anim.start();

// 每影格：
let progress = anim.current_progress();  // 0.0..=1.0
let eased = config.easing.apply(progress);

// 檢查狀態
if anim.is_running() { /* 更新視覺效果 */ }
if anim.is_completed() { /* 清理 */ }

anim.pause();
anim.resume();
anim.stop();
anim.reset();
```

### `ColorAnimation`

在兩種顏色之間動畫：

```rust
use rust_widgets::style::animation::ColorAnimation;

let mut color_anim = ColorAnimation::new(
    Color::RED,
    Color::BLUE,
    AnimationConfig::new(Duration::from_millis(500)).with_easing(EasingFunction::EaseInOut),
);

color_anim.start();

// 每影格：
let current_color = color_anim.current_value();
widget.set_background(current_color);
```

### `FloatAnimation`

動畫化純量數值（不透明度、位置、尺寸）：

```rust
use rust_widgets::style::animation::FloatAnimation;

let mut fade = FloatAnimation::new(
    0.0,   // 起始值
    1.0,   // 結束值
    AnimationConfig::new(Duration::from_millis(300)),
);

fade.start();

// 每影格：
let current_opacity = fade.current_value();
widget.set_opacity(current_opacity);
```

### `KeyframeAnimation`

具有百分比停止點的多段動畫：

```rust
use rust_widgets::style::animation::KeyframeAnimation;

let mut keyframes = KeyframeAnimation::new(
    AnimationConfig::new(Duration::from_millis(1000)).infinite(),
);

keyframes.add_keyframe(0.0, 0.0);              // 起始：translateX(0)
keyframes.add_keyframe(0.25, 50.0);            // 25%：translateX(50)
keyframes.add_keyframe(0.5, 100.0);            // 50%：translateX(100)
keyframes.add_keyframe(0.75, 50.0);            // 75%：translateX(50)
keyframes.add_keyframe(1.0, 0.0);              // 100%：回到 0

keyframes.start();

// 每影格：
let offset = keyframes.current_value();        // 插值後的值
widget.set_x(offset);
```

---

## Spring Animation — 物理基礎

`SpringAnimation` 模擬質量-彈簧-阻尼系統，產生自然感的運動：

```rust
use rust_widgets::style::animation::SpringAnimation;

let mut spring = SpringAnimation::new(
    0.0,      // 起始值
    100.0,    // 結束值
    0.5,      // 阻尼比（0.0 = 無阻尼，1.0 = 臨界阻尼）
    20.0,     // 剛度（越高越快）
);

spring.start();

// 每影格：
let position = spring.current_value();
let velocity = spring.velocity();

// 自然穩定 — 無固定持續時間
if spring.is_settled() {
    // 動畫完成
}
```

**調校指南：**

| 阻尼 | 剛度 | 行為 |
|---|---|---|
| 0.3 | 15 | 有彈性，穩定慢 |
| 0.5 | 20 | 有彈性，適中穩定 |
| 0.7 | 25 | 輕微彈跳，快速穩定 |
| 1.0 | 20 | 臨界阻尼（無彈跳，最快穩定） |

---

## 動畫群組

### `ParallelAnimation` — 同時播放

同時播放多個動畫：

```rust
use rust_widgets::style::animation_group::ParallelAnimation;

let mut group = ParallelAnimation::new();
group.add(fade_in);
group.add(slide_up);
group.add(scale_in);

group.start();

// 所有動畫同時執行
if group.is_completed() {
    // 所有動畫已完成
}
```

### `SequentialAnimation` — 鏈式播放

依序播放動畫：

```rust
use rust_widgets::style::animation_group::SequentialAnimation;

let mut sequence = SequentialAnimation::new();
sequence.add(fade_out);
sequence.add(swap_content);
sequence.add(fade_in);

sequence.start();

// 動畫依序播放
// is_completed() 僅在最後一個動畫完成時回傳 true
```

### `AnimationGroup` — 混合播放

結合平行與序列群組：

```rust
use rust_widgets::style::animation_group::AnimationGroup;

let mut enter_animation = AnimationGroup::new();

// 淡入 + 滑入同時進行
let mut enter_parallel = ParallelAnimation::new();
enter_parallel.add(fade_in);
enter_parallel.add(slide_up);

// 然後等待
let delay = Animation::new(
    AnimationConfig::new(Duration::from_millis(500))
);

// 然後彈跳
let bounce = FloatAnimation::new(100.0, 120.0,
    AnimationConfig::new(Duration::from_millis(300)).with_easing(EasingFunction::BounceOut));

enter_animation.add(Box::new(enter_parallel));
enter_animation.add(Box::new(delay));
enter_animation.add(Box::new(bounce));

enter_animation.start();
```

---

## 主題系統

### `Theme` — 高階定義

完整的主題定義了顏色、字型、間距、邊框以及各類別的覆蓋設定：

```rust
pub struct Theme {
    pub name: String,
    pub colors: Colors,
    pub fonts: Fonts,
    pub spacing: Spacing,
    pub borders: Borders,
    pub overrides: ThemeOverrides,
}
```

### `Colors` — 語義色板

```rust
let colors = Colors {
    background: Color::from_rgb(240, 240, 240),
    foreground: Color::BLACK,
    primary:    Color::from_rgb(33, 150, 243),    // 品牌藍色
    secondary:  Color::from_rgb(158, 158, 158),    // 中性灰色
    accent:     Color::from_rgb(255, 152, 0),      // 橙色強調
    error:      Color::from_rgb(244, 67, 54),      // 紅色
    warning:    Color::from_rgb(255, 193, 7),      // 琥珀色
    success:    Color::from_rgb(76, 175, 80),      // 綠色
    disabled:   Color::from_rgb(200, 200, 200),
    info:       Color::INFO,
};
```

### `Fonts` — 排版標記

```rust
let fonts = Fonts {
    regular:    Font::simple("Arial", 14.0),
    bold:       Font::bold("Arial", 14.0),
    italic:     Font { family: "Arial".into(), size: 14.0, italic: true, .. },
    monospace:  Font::simple("Courier New", 12.0),
    caption:    Font::simple("Arial", 11.0),    // 註腳、次要文字
    body:       Font::simple("Arial", 14.0),    // 預設段落
    title:      Font::bold("Arial", 16.0),      // 章節標題
    headline:   Font::bold("Arial", 20.0),      // 醒目標題
    display:    Font::bold("Arial", 28.0),      // 大型裝飾文字
};
```

### `Spacing` — 間距尺度標記

```rust
let spacing = Spacing {
    small: 4,
    medium: 8,
    large: 16,
    extra_large: 24,
};
```

### `Borders` — 邊框／陰影預設值

```rust
let borders = Borders {
    width: 1,         // 預設邊框寬度
    radius: 4,        // 預設圓角半徑
    shadow: true,     // 啟用投影
};
```

### `ThemeOverrides` — 各類別標記

無需建立完整主題即可微調特定控制項類別：

```rust
let mut overrides = ThemeOverrides {
    styles: HashMap::new(),
};

overrides.styles.insert("button".into(), ThemeStyleToken {
    background: Some(Color::from_rgb(33, 150, 243)),
    foreground: Some(Color::WHITE),
    border: Some(Color::from_rgb(25, 118, 210)),
    border_width: Some(2),
    radius: Some(8),
});
```

---

## `ThemeManager` — 執行時期主題切換

```rust
use rust_widgets::theme::ThemeManager;

let mut manager = ThemeManager::new();

// 從 JSON 載入
manager.load_theme("custom_theme.json")?;

// 在執行時期切換主題
manager.set_theme("dark");

// 透過信號監聽主題變更
manager.on_theme_changed().connect(|| {
    println!("主題已變更！請求重新繪製...");
});

// 從目前主題解析控制項樣式
let button_style = manager.resolve_style("button");
// 回傳一個已預填顏色、字型、內邊距與陰影的 WidgetStyle

// 以程式方式註冊主題
let mut custom = Theme::default();
custom.name = "corporate".into();
custom.colors.primary = Color::from_rgb(0, 121, 107);
manager.register_theme(custom);

// 將目前主題儲存為 JSON
manager.save_theme("exported_theme.json")?;
```

**內建主題：**

| 主題 | 說明 |
|---|---|
| `"default"` | Material Light：灰色背景、藍色主要色、Arial 字型 |
| `"dark"` | Material Dark：近似黑色背景（#121212）、淺色文字、柔和主要色 |

```rust
// 深色主題預設
let dark = Theme::dark();
manager.register_theme(dark);
manager.set_theme("dark");
```

---

## `ThemeStateManager` — 淺色／深色／自動模式

管理各控制項狀態的主題，支援淺色／深色／自動切換：

```rust
use rust_widgets::style::theme_state::{ThemeStateManager, ThemeMode, StatefulTheme, WidgetState, StateTheme};

let mut light_theme = StatefulTheme::new("light");

// 為按鈕定義狀態
light_theme.add_state(WidgetState::Normal, StateTheme::new(
    Color::from_rgb(33, 150, 243),   // 背景
    Color::WHITE,                     // 前景
    Color::WHITE,                     // 文字
));

light_theme.add_state(WidgetState::Hover, StateTheme::new(
    Color::from_rgb(66, 165, 245),
    Color::WHITE,
    Color::WHITE,
));

light_theme.add_state(WidgetState::Pressed, StateTheme::new(
    Color::from_rgb(25, 118, 210),
    Color::WHITE,
    Color::WHITE,
));

light_theme.add_state(WidgetState::Disabled, StateTheme::new(
    Color::from_rgb(189, 189, 189),
    Color::from_rgb(245, 245, 245),
    Color::from_rgb(158, 158, 158),
));

// 深色主題變體
let mut dark_theme = StatefulTheme::new("dark");
// ... 定義深色狀態 ...

// 建立管理器
let mut state_manager = ThemeStateManager::new(light_theme, dark_theme);

// 切換模式
state_manager.set_mode(ThemeMode::Dark);
state_manager.toggle_mode();  // Dark → Light → Auto 模式循環

// 自動模式搭配時間切換
state_manager.set_auto_switch(18, 6);  // 晚上 6 點到早上 6 點 UTC 為深色模式

// 回應模式變更
state_manager.on_mode_changed(|mode| {
    match mode {
        ThemeMode::Light => println!("切換至淺色主題"),
        ThemeMode::Dark => println!("切換至深色主題"),
        ThemeMode::Auto => println!("自動模式啟用中"),
    }
});

// 解析特定狀態的外觀
let hover_style: &StateTheme = state_manager.get_state_theme(&WidgetState::Hover);
widget.set_background(hover_style.background_color);
widget.set_foreground(hover_style.foreground_color);
widget.set_text_color(hover_style.text_color);
```

### `WidgetState` — 12 種互動狀態

```rust
pub enum WidgetState {
    Normal,     // 預設靜止狀態
    Hover,      // 滑鼠游標懸停在控制項上
    Pressed,    // 正在被點擊／按壓
    Focused,    // 擁有鍵盤焦點
    Disabled,   // 灰階顯示，無法互動
    Checked,    // 核取方塊／選項按鈕：已切換開啟
    Selected,   // 列表項目：目前已選取
    Active,     // 視窗／對話框：最前方
    Inactive,   // 視窗／對話框：背景
    Error,      // 驗證錯誤
    Warning,    // 驗證警告
    Success,    // 驗證成功
}
```

### `StateTheme` — 各狀態的視覺屬性

```rust
let mut state_theme = StateTheme::new(
    Color::WHITE,                   // 背景
    Color::BLACK,                   // 前景
    Color::from_rgb(33, 33, 33),   // 文字
)
    .with_border(Color::GRAY, 1)
    .with_shadow(Color::rgba(0, 0, 0, 30), (0, 2), 4)
    .with_opacity(0.95)
    .with_property("border-radius", "8");
```

**狀態轉場：** 定義狀態配對之間的動畫持續時間：

```rust
stateful_theme.set_transition(WidgetState::Normal, WidgetState::Hover, 200);   // 200ms 懸停進入
stateful_theme.set_transition(WidgetState::Hover, WidgetState::Normal, 150);   // 150ms 懸停離開
stateful_theme.set_transition(WidgetState::Normal, WidgetState::Pressed, 50);  // 即時按壓
```

---

## `HighContrastMode`

為無障礙功能覆蓋所有顏色：

```rust
pub enum HighContrastMode {
    None,
    BlackOnWhite,     // 強制黑色文字在白色背景上
    WhiteOnBlack,     // 強制白色文字在黑色背景上
    Custom { fg: Color, bg: Color },  // 任意的自訂配對
}
```

當 `HighContrastMode` 設定為非 `None` 的值時，主題顏色解析會忽略主題色板，並使用強制的前景／背景色。

---

## `TouchTargetSize` — 依裝置類別

依據裝置類別的最小互動區域：

```rust
pub enum TouchTargetSize {
    Desktop,      // 32×32 pt，8px 間距
    Tablet,       // 44×44 pt，12px 間距
    Phone,        // 48×48 pt，16px 間距
    Embedded,     // 40×40 pt，10px 間距
    Projection,   // 24×24 pt，6px 間距（feature = "projection"）
}

let size = TouchTargetSize::Phone.dimensions();    // Size(48, 48)
let spacing = TouchTargetSize::Phone.spacing();    // 16
```

套用依裝置類別的覆蓋至 `WidgetStyle`：

```rust
let phone_style = WidgetStyle::default()
    .with_touch_target(TouchTargetSize::Phone.dimensions());
```

---

## 常用模式

### 使用樣式轉場的懸停動畫

```rust
impl MyButton {
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseEnter { .. } => {
                self.state = WidgetState::Hover;
                self.hover_animation.start();  // 動畫至懸停顏色
                true
            }
            Event::MouseLeave { .. } => {
                self.state = WidgetState::Normal;
                self.hover_animation.reverse(); // 動畫回原狀
                true
            }
            _ => false,
        }
    }

    fn render(&self, scene: &mut RenderScene) {
        let state_theme = theme_manager.get_state_theme(&self.state);
        let progress = self.hover_animation.current_progress();

        let bg = lerp_color(
            &self.normal_bg,
            &state_theme.background_color,
            progress,
        );

        // 使用插值後的顏色進行渲染...
    }
}
```

### 使用關鍵幀動畫的載入旋轉器

```rust
let mut spinner = FloatAnimation::new(
    0.0, 360.0,
    AnimationConfig::new(Duration::from_millis(1000)).infinite(),
);
spinner.start();

// 在渲染迴圈中：
let rotation = spinner.current_value();
let arc_command = RenderCommand::DrawArc {
    center: Point::new(50, 50),
    radius: 20,
    start_angle: rotation.to_radians(),
    end_angle: (rotation + 270.0).to_radians(),
    color: theme.colors.primary,
    filled: false,
};
```

### 完整主題定義（JSON）

```json
{
    "name": "midnight",
    "colors": {
        "background": "#0d1117",
        "foreground": "#c9d1d9",
        "primary": "#58a6ff",
        "secondary": "#8b949e",
        "accent": "#f78166",
        "error": "#f85149",
        "warning": "#d29922",
        "success": "#3fb950",
        "disabled": "#484f58",
        "info": "#79c0ff"
    },
    "fonts": {
        "regular": { "family": "Inter", "size": 14.0 },
        "bold": { "family": "Inter", "size": 14.0, "bold": true },
        "monospace": { "family": "JetBrains Mono", "size": 13.0 },
        "title": { "family": "Inter", "size": 18.0, "bold": true },
        "headline": { "family": "Inter", "size": 24.0, "bold": true }
    },
    "spacing": { "small": 4, "medium": 8, "large": 16, "extra_large": 24 },
    "borders": { "width": 1, "radius": 6, "shadow": true },
    "overrides": { "styles": {} }
}
```

載入它：

```rust
manager.load_theme("midnight.json")?;
manager.set_theme("midnight");
```

### 自訂 CSS 搭配熱重新載入

```rust
// theme.css：
//   Button { background-color: #2196f3; border-radius: 8; }
//   Button:hover { background-color: #1976d2; }

let mut watcher = CssWatcher::new("theme.css", "main-theme");

// 在主迴圈中：
app.on_frame_update(move || {
    if watcher.poll() == Ok(true) {
        app.request_repaint_all();
    }
});
```

### 物理基礎的下拉重新整理

```rust
let mut spring = SpringAnimation::new(
    0.0,       // 起始值（靜止位置）
    -80.0,     // 結束值（下拉偏移）
    0.6,       // 阻尼
    15.0,      // 剛度
);

spring.start();

// 拖曳手勢更新目標值
match event {
    Event::Drag { delta, .. } => {
        spring.set_target(spring.target() + delta.y);
    }
    Event::TouchEnd { .. } => {
        spring.set_target(0.0); // 彈回
    }
    _ => {}
}

// 每影格：
let offset = spring.current_value();
refresh_indicator.set_position(offset);
```

### 完整樣式解析管線

```rust
fn resolve_widget_style(
    widget_id: ObjectId,
    theme_manager: &ThemeManager,
    state_manager: &ThemeStateManager,
    class: &str,
    state: &WidgetState,
) -> WidgetStyle {
    // 1. 從主題取得基底
    let mut style = theme_manager.resolve_style(class);

    // 2. 狀態主題覆蓋
    let state_theme = state_manager.get_state_theme(state);
    style.background_color = style.background_color.or(Some(state_theme.background_color));
    style.text_color = style.text_color.or(Some(state_theme.text_color));
    style.border_color = style.border_color.or(Some(state_theme.border_color));
    style.border_width = if style.border_width == 0 { state_theme.border_width } else { style.border_width };
    style.opacity = style.opacity.map(|o| o * state_theme.opacity);

    // 3. 來自全域樣式表管理器的 CSS 覆蓋
    let mgr = global_stylesheet_manager();
    let _ = mgr.apply_to(class, None, None, Some(to_pseudo_state(state)), &mut style);

    // 4. 行內覆蓋（應用程式特定）
    // ... 合併各控制項的行內覆蓋 ...

    style
}
```
