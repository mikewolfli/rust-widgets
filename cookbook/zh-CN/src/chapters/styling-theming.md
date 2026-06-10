# 样式与主题

`rust-widgets` 样式系统通过级联解析链解析窗口部件的外观：**主题（Theme）→ ThemeOverrides → StatefulTheme → 内联 CSS**。系统包含一个带有选择器引擎和特异性的 CSS 解析器、可热重载的样式表、一个具有 10 种缓动函数和基于物理的弹簧的动画框架，以及内置的 Material 亮色/暗色主题，提供全面的视觉定制能力。

---

## 级联解析链

窗口部件样式通过四个层级解析，当某一属性未设置时会依次回退到下一层：

```
1. 全局主题默认值 (ThemeManager → Theme)
2. 按窗口部件类别的 ThemeOverrides（例如 "Button"、"Label"）
3. 窗口部件实例状态 (StatefulTheme → WidgetState)
4. 内联样式覆盖 (StyleSheetManager 中的 CSS)
```

---

## `WidgetStyle` — 已解析的视觉属性

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

构建器方法提供流畅的构造方式：

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

**样式继承** — 子级对未设置的属性回退到父级：

```rust
let parent = WidgetStyle::default()
    .with_background(Color::RED)
    .with_text_color(Color::BLUE);

let child = WidgetStyle::default();  // 全部为 None

let inherited = child.inherit_from(&parent);
// inherited.background_color == Some(RED)
// inherited.text_color == Some(BLUE)

// 子级属性覆盖父级
let child = WidgetStyle::default().with_background(Color::GREEN);
let inherited = child.inherit_from(&parent);
// inherited.background_color == Some(GREEN)  — 子级优先
```

**样式合并** — 从另一个样式中填补缺失的属性：

```rust
let mut base = WidgetStyle::default().with_background(Color::RED);
let overlay = WidgetStyle::default().with_text_color(Color::BLUE);
base.merge(&overlay);
// base.background_color == Some(RED)   — 保持不变
// base.text_color == Some(BLUE)         — 从 overlay 填补
```

---

## Padding、Margin 和 Shadow

### `Padding`

每边独立的内边距，位于窗口部件边框和内容之间：

```rust
let padding = Padding::new(8, 12, 8, 12);      // 上, 右, 下, 左
let uniform = Padding::all(4);                   // 所有边 = 4
let symmetric = Padding::symmetric(6, 12);        // 垂直=6, 水平=12
let safe = Padding::normalized(-1, 4, -99, 8);   // 负值裁剪为 0 → (0, 4, 0, 8)
```

### `Margin`

每边独立的外边距，位于窗口部件边框外部：

```rust
let margin = Margin::new(8, 4, 8, 4);
let uniform = Margin::all(8);
let symmetric = Margin::symmetric(4, 8);
let safe = Margin::normalized(-5, 0, 3, 2);  // → (0, 0, 3, 2)
```

### `Shadow`

窗口部件的投影阴影效果：

```rust
let shadow = Shadow::new()
    .with_offset(0, 4)                   // 阴影在下方 4px
    .with_blur(8)                        // 8px 模糊半径
    .with_color(Color::rgba(0, 0, 0, 60)); // 半透明黑色
```

---

## CSS 系统

### CSS 解析器

`CssParser` 将 CSS 文本转换为包含选择器匹配和属性应用的 `StyleSheet`：

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

// 应用到窗口部件
let mut style = WidgetStyle::default();
CssParser::parse_and_apply(css, "Button", Some("primary"), Some("submit-btn"), Some(PseudoState::Hover), &mut style)?;
```

### `CssSelector` — 选择器引擎

选择器引擎支持种类、类、ID、状态伪类以及 AND 组合：

```rust
pub enum CssSelector {
    Universal,                              // *
    Kind(String),                           // "Button"
    Class(String),                          // ".primary"
    Id(String),                             // "#submit"
    State(PseudoState),                     // :hover, :pressed, :disabled
    And(Vec<CssSelector>),                  // Button.primary:hover
}
```

**特异性规则：**
- ID 选择器（`#id`）— 最高特异性
- 类选择器（`.class`）— 中等
- 种类选择器（`Widget`）— 较低
- 通用选择器（`*`）— 最低
- 伪状态（`:hover`）— 附加

当多个规则匹配时，每个属性使用特异性最高的规则。

---

## `StyleSheetManager` — 全局注册表

全局线程安全的样式表注册表，支持优先级分层：

```rust
use rust_widgets::style::stylesheet::{global_stylesheet_manager, StyleSheetManager};

// 注册带优先级的命名样式表
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
    "#, 0);  // 优先级 0（基础）

    mgr.register("brand-override", r#"
        .accent {
            background-color: #ff6f00;
        }
    "#, 100);  // 优先级 100（覆盖）
}

// 将所有匹配规则应用到窗口部件样式
{
    let mgr = global_stylesheet_manager();
    let mut style = WidgetStyle::default();
    mgr.apply_to("Button", Some("accent"), None, Some(PseudoState::Normal), &mut style)?;
}

// 注销样式表
{
    let mut mgr = global_stylesheet_manager();
    mgr.unregister("brand-override");
}
```

**优先级分层：** 样式表在应用前按 `priority`（升序）排序。低优先级值先应用，高优先级值覆盖它们。

---

## `CssWatcher` — CSS 文件热重载

基于轮询的文件监视器，当文件变化时自动重新加载 CSS 到全局 `StyleSheetManager`：

```rust
use rust_widgets::style::css_watcher::CssWatcher;

let mut watcher = CssWatcher::new("theme.css", "main-theme");
watcher.set_poll_interval(500);  // 每 500ms 检查一次（默认）

// 在主循环中轮询
loop {
    match watcher.poll() {
        Ok(true)  => println!("CSS 已重新加载 — 样式已更新！"),
        Ok(false) => { /* 无变化 */ }
        Err(e)    => eprintln!("CSS 监视错误: {}", e),
    }

    // ... 渲染帧 ...

    std::thread::sleep(std::time::Duration::from_millis(16));
}

// 强制重新加载（忽略修改时间）
watcher.reload()?;
```

---

## 动画系统

### 缓动函数 — 10 种预设

```rust
use rust_widgets::style::animation::EasingFunction;

let t = 0.5;  // 进度 [0.0, 1.0]

let linear    = EasingFunction::Linear.apply(t);      // t
let ease_in   = EasingFunction::EaseIn.apply(t);      // t²
let ease_out  = EasingFunction::EaseOut.apply(t);     // 1 - (1-t)²
let ease_inout= EasingFunction::EaseInOut.apply(t);   // 平滑 S 曲线

// 弹跳
let bounce_in = EasingFunction::BounceIn.apply(t);    // 过冲 + 稳定
let bounce_out= EasingFunction::BounceOut.apply(t);

// 弹性
let elastic_in  = EasingFunction::ElasticIn.apply(t); // 类似弹簧
let elastic_out = EasingFunction::ElasticOut.apply(t);

// 回退（过冲）
let back_in  = EasingFunction::BackIn.apply(t);       // 先过冲再进入
let back_out = EasingFunction::BackOut.apply(t);      // 先进入再过冲
```

### `AnimationConfig`

控制动画的时序和行为：

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

核心动画状态机：

```rust
let mut anim = Animation::new(config);
anim.on_complete(Box::new(|| println!("动画完成！")));

anim.start();

// 每帧：
let progress = anim.current_progress();  // 0.0..=1.0
let eased = config.easing.apply(progress);

// 检查状态
if anim.is_running() { /* 更新视觉效果 */ }
if anim.is_completed() { /* 清理 */ }

anim.pause();
anim.resume();
anim.stop();
anim.reset();
```

### `ColorAnimation`

在两个颜色之间动画过渡：

```rust
use rust_widgets::style::animation::ColorAnimation;

let mut color_anim = ColorAnimation::new(
    Color::RED,
    Color::BLUE,
    AnimationConfig::new(Duration::from_millis(500)).with_easing(EasingFunction::EaseInOut),
);

color_anim.start();

// 每帧：
let current_color = color_anim.current_value();
widget.set_background(current_color);
```

### `FloatAnimation`

动画标量值（不透明度、位置、大小）：

```rust
use rust_widgets::style::animation::FloatAnimation;

let mut fade = FloatAnimation::new(
    0.0,   // 起始值
    1.0,   // 目标值
    AnimationConfig::new(Duration::from_millis(300)),
);

fade.start();

// 每帧：
let current_opacity = fade.current_value();
widget.set_opacity(current_opacity);
```

### `KeyframeAnimation`

多点动画，支持百分比定义的关键帧：

```rust
use rust_widgets::style::animation::KeyframeAnimation;

let mut keyframes = KeyframeAnimation::new(
    AnimationConfig::new(Duration::from_millis(1000)).infinite(),
);

keyframes.add_keyframe(0.0, 0.0);              // 开始: translateX(0)
keyframes.add_keyframe(0.25, 50.0);            // 25%: translateX(50)
keyframes.add_keyframe(0.5, 100.0);            // 50%: translateX(100)
keyframes.add_keyframe(0.75, 50.0);            // 75%: translateX(50)
keyframes.add_keyframe(1.0, 0.0);              // 100%: 回到 0

keyframes.start();

// 每帧：
let offset = keyframes.current_value();        // 插值后的值
widget.set_x(offset);
```

---

## 弹簧动画 — 基于物理

`SpringAnimation` 模拟质量-弹簧-阻尼器系统，实现自然感觉的运动：

```rust
use rust_widgets::style::animation::SpringAnimation;

let mut spring = SpringAnimation::new(
    0.0,      // 起始值
    100.0,    // 目标值
    0.5,      // 阻尼比（0.0 = 无阻尼，1.0 = 临界阻尼）
    20.0,     // 刚度（越高越快）
);

spring.start();

// 每帧：
let position = spring.current_value();
let velocity = spring.velocity();

// 自然稳定 — 无固定时长
if spring.is_settled() {
    // 动画完成
}
```

**调优指南：**

| 阻尼 | 刚度 | 行为 |
|:---:|:---:|------|
| 0.3 | 15 | 有弹性，缓慢稳定 |
| 0.5 | 20 | 有弹性，中等稳定 |
| 0.7 | 25 | 轻微弹性，快速稳定 |
| 1.0 | 20 | 临界阻尼（无弹性，最快稳定） |

---

## 动画组

### `ParallelAnimation` — 并行播放

同时播放多个动画：

```rust
use rust_widgets::style::animation_group::ParallelAnimation;

let mut group = ParallelAnimation::new();
group.add(fade_in);
group.add(slide_up);
group.add(scale_in);

group.start();

// 所有动画同时运行
if group.is_completed() {
    // 所有动画已完成
}
```

### `SequentialAnimation` — 链式播放

一个接一个地播放动画：

```rust
use rust_widgets::style::animation_group::SequentialAnimation;

let mut sequence = SequentialAnimation::new();
sequence.add(fade_out);
sequence.add(swap_content);
sequence.add(fade_in);

sequence.start();

// 动画按顺序播放
// 只有当最后一个动画完成时，is_completed() 才返回 true
```

### `AnimationGroup` — 混合组合

组合并行和串行动画组：

```rust
use rust_widgets::style::animation_group::AnimationGroup;

let mut enter_animation = AnimationGroup::new();

// 淡入 + 上滑同时进行
let mut enter_parallel = ParallelAnimation::new();
enter_parallel.add(fade_in);
enter_parallel.add(slide_up);

// 然后等待
let delay = Animation::new(
    AnimationConfig::new(Duration::from_millis(500))
);

// 然后弹跳
let bounce = FloatAnimation::new(100.0, 120.0,
    AnimationConfig::new(Duration::from_millis(300)).with_easing(EasingFunction::BounceOut));

enter_animation.add(Box::new(enter_parallel));
enter_animation.add(Box::new(delay));
enter_animation.add(Box::new(bounce));

enter_animation.start();
```

---

## 主题系统

### `Theme` — 高级定义

一个完整的主题定义了颜色、字体、间距、边框和按类别的覆盖：

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

### `Colors` — 语义调色板

```rust
let colors = Colors {
    background: Color::from_rgb(240, 240, 240),
    foreground: Color::BLACK,
    primary:    Color::from_rgb(33, 150, 243),    // 品牌蓝色
    secondary:  Color::from_rgb(158, 158, 158),    // 中性灰色
    accent:     Color::from_rgb(255, 152, 0),      // 橙色强调
    error:      Color::from_rgb(244, 67, 54),      // 红色
    warning:    Color::from_rgb(255, 193, 7),      // 琥珀色
    success:    Color::from_rgb(76, 175, 80),      // 绿色
    disabled:   Color::from_rgb(200, 200, 200),
    info:       Color::INFO,
};
```

### `Fonts` — 排版令牌

```rust
let fonts = Fonts {
    regular:    Font::simple("Arial", 14.0),
    bold:       Font::bold("Arial", 14.0),
    italic:     Font { family: "Arial".into(), size: 14.0, italic: true, .. },
    monospace:  Font::simple("Courier New", 12.0),
    caption:    Font::simple("Arial", 11.0),    // 脚注、次要文本
    body:       Font::simple("Arial", 14.0),    // 默认段落
    title:      Font::bold("Arial", 16.0),      // 章节标题
    headline:   Font::bold("Arial", 20.0),      // 突出标题
    display:    Font::bold("Arial", 28.0),      // 大型装饰文字
};
```

### `Spacing` — 缩放令牌

```rust
let spacing = Spacing {
    small: 4,
    medium: 8,
    large: 16,
    extra_large: 24,
};
```

### `Borders` — 边框/阴影默认值

```rust
let borders = Borders {
    width: 1,         // 默认边框宽度
    radius: 4,        // 默认圆角半径
    shadow: true,     // 启用投影
};
```

### `ThemeOverrides` — 按类别的令牌

在不创建完整主题的情况下微调特定窗口部件类别：

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

## `ThemeManager` — 运行时主题切换

```rust
use rust_widgets::theme::ThemeManager;

let mut manager = ThemeManager::new();

// 从 JSON 加载
manager.load_theme("custom_theme.json")?;

// 运行时切换主题
manager.set_theme("dark");

// 通过信号监听主题变化
manager.on_theme_changed().connect(|| {
    println!("主题已更改！请求重绘...");
});

// 从当前主题解析窗口部件样式
let button_style = manager.resolve_style("button");
// 返回一个已预填充颜色、字体、内边距和阴影的 WidgetStyle

// 通过编程方式注册主题
let mut custom = Theme::default();
custom.name = "corporate".into();
custom.colors.primary = Color::from_rgb(0, 121, 107);
manager.register_theme(custom);

// 将当前主题保存为 JSON
manager.save_theme("exported_theme.json")?;
```

**内置主题：**

| 主题 | 描述 |
|------|------|
| `"default"` | Material Light：灰色背景，蓝色主色调，Arial 字体 |
| `"dark"` | Material Dark：近黑色背景 (#121212)，浅色文字，柔和主色调 |

```rust
// 暗色主题预设
let dark = Theme::dark();
manager.register_theme(dark);
manager.set_theme("dark");
```

---

## `ThemeStateManager` — 亮色/暗色/自动模式

管理按窗口部件状态的主题，支持亮色/暗色/自动切换：

```rust
use rust_widgets::style::theme_state::{ThemeStateManager, ThemeMode, StatefulTheme, WidgetState, StateTheme};

let mut light_theme = StatefulTheme::new("light");

// 为按钮定义各状态
light_theme.add_state(WidgetState::Normal, StateTheme::new(
    Color::from_rgb(33, 150, 243),   // 背景
    Color::WHITE,                     // 前景
    Color::WHITE,                     // 文本
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

// 暗色主题变体
let mut dark_theme = StatefulTheme::new("dark");
// ... 定义暗色状态 ...

// 创建管理器
let mut state_manager = ThemeStateManager::new(light_theme, dark_theme);

// 切换模式
state_manager.set_mode(ThemeMode::Dark);
state_manager.toggle_mode();  // 暗色 → 亮色 → 自动模式循环

// 基于时间的自动切换
state_manager.set_auto_switch(18, 6);  // UTC 时间 18:00 到 6:00 为暗色模式

// 响应模式变化
state_manager.on_mode_changed(|mode| {
    match mode {
        ThemeMode::Light => println!("切换到亮色主题"),
        ThemeMode::Dark => println!("切换到暗色主题"),
        ThemeMode::Auto => println("自动模式已激活"),
    }
});

// 解析状态特定的外观
let hover_style: &StateTheme = state_manager.get_state_theme(&WidgetState::Hover);
widget.set_background(hover_style.background_color);
widget.set_foreground(hover_style.foreground_color);
widget.set_text_color(hover_style.text_color);
```

### `WidgetState` — 12 种交互状态

```rust
pub enum WidgetState {
    Normal,     // 默认静止状态
    Hover,      // 鼠标悬停在窗口部件上
    Pressed,    // 正在被点击/按下
    Focused,    // 拥有键盘焦点
    Disabled,   // 灰色显示，无法交互
    Checked,    // 复选框/单选按钮：已选中
    Selected,   // 列表项：当前已选
    Active,     // 窗口/对话框：最前端
    Inactive,   // 窗口/对话框：后台
    Error,      // 验证错误
    Warning,    // 验证警告
    Success,    // 验证成功
}
```

### `StateTheme` — 每状态的视觉属性

```rust
let mut state_theme = StateTheme::new(
    Color::WHITE,                   // 背景
    Color::BLACK,                   // 前景
    Color::from_rgb(33, 33, 33),   // 文本
)
    .with_border(Color::GRAY, 1)
    .with_shadow(Color::rgba(0, 0, 0, 30), (0, 2), 4)
    .with_opacity(0.95)
    .with_property("border-radius", "8");
```

**状态过渡：** 定义状态对之间的动画持续时间：

```rust
stateful_theme.set_transition(WidgetState::Normal, WidgetState::Hover, 200);   // 200ms 悬停进入
stateful_theme.set_transition(WidgetState::Hover, WidgetState::Normal, 150);   // 150ms 悬停退出
stateful_theme.set_transition(WidgetState::Normal, WidgetState::Pressed, 50);  // 瞬时按下
```

---

## `HighContrastMode`

覆盖所有颜色以实现无障碍访问：

```rust
pub enum HighContrastMode {
    None,
    BlackOnWhite,     // 强制黑色文字在白底上
    WhiteOnBlack,     // 强制白色文字在黑底上
    Custom { fg: Color, bg: Color },  // 任意自定义配对
}
```

当 `HighContrastMode` 为 `None` 以外的值时，主题颜色解析会忽略主题调色板，改用强制的前景/背景色。

---

## `TouchTargetSize` — 按设备类别

按设备类别划分的最小可交互区域：

```rust
pub enum TouchTargetSize {
    Desktop,      // 32×32 pt, 8px 间距
    Tablet,       // 44×44 pt, 12px 间距
    Phone,        // 48×48 pt, 16px 间距
    Embedded,     // 40×40 pt, 10px 间距
    Projection,   // 24×24 pt, 6px 间距  (feature = "projection")
}

let size = TouchTargetSize::Phone.dimensions();    // Size(48, 48)
let spacing = TouchTargetSize::Phone.spacing();    // 16
```

对 `WidgetStyle` 应用按设备类别的覆盖：

```rust
let phone_style = WidgetStyle::default()
    .with_touch_target(TouchTargetSize::Phone.dimensions());
```

---

## 常见模式

### 带样式过渡的悬停动画

```rust
impl MyButton {
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseEnter { .. } => {
                self.state = WidgetState::Hover;
                self.hover_animation.start();  // 动画过渡到悬停颜色
                true
            }
            Event::MouseLeave { .. } => {
                self.state = WidgetState::Normal;
                self.hover_animation.reverse(); // 动画恢复
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

        // 使用插值后的颜色渲染...
    }
}
```

### 使用关键帧动画的加载旋转器

```rust
let mut spinner = FloatAnimation::new(
    0.0, 360.0,
    AnimationConfig::new(Duration::from_millis(1000)).infinite(),
);
spinner.start();

// 在渲染循环中：
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

### 完整主题定义（JSON）

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

加载它：

```rust
manager.load_theme("midnight.json")?;
manager.set_theme("midnight");
```

### 自定义 CSS 与热重载

```rust
// theme.css:
//   Button { background-color: #2196f3; border-radius: 8; }
//   Button:hover { background-color: #1976d2; }

let mut watcher = CssWatcher::new("theme.css", "main-theme");

// 在主循环中：
app.on_frame_update(move || {
    if watcher.poll() == Ok(true) {
        app.request_repaint_all();
    }
});
```

### 基于物理的下拉刷新

```rust
let mut spring = SpringAnimation::new(
    0.0,       // 起始值（静止位置）
    -80.0,     // 目标值（下拉偏移）
    0.6,       // 阻尼
    15.0,      // 刚度
);

spring.start();

// 拖拽手势更新目标值
match event {
    Event::Drag { delta, .. } => {
        spring.set_target(spring.target() + delta.y);
    }
    Event::TouchEnd { .. } => {
        spring.set_target(0.0); // 弹回
    }
    _ => {}
}

// 每帧：
let offset = spring.current_value();
refresh_indicator.set_position(offset);
```

### 完整样式解析管道

```rust
fn resolve_widget_style(
    widget_id: ObjectId,
    theme_manager: &ThemeManager,
    state_manager: &ThemeStateManager,
    class: &str,
    state: &WidgetState,
) -> WidgetStyle {
    // 1. 基于主题的基础样式
    let mut style = theme_manager.resolve_style(class);

    // 2. 状态主题覆盖
    let state_theme = state_manager.get_state_theme(state);
    style.background_color = style.background_color.or(Some(state_theme.background_color));
    style.text_color = style.text_color.or(Some(state_theme.text_color));
    style.border_color = style.border_color.or(Some(state_theme.border_color));
    style.border_width = if style.border_width == 0 { state_theme.border_width } else { style.border_width };
    style.opacity = style.opacity.map(|o| o * state_theme.opacity);

    // 3. 全局样式表管理器中的 CSS 覆盖
    let mgr = global_stylesheet_manager();
    let _ = mgr.apply_to(class, None, None, Some(to_pseudo_state(state)), &mut style);

    // 4. 内联覆盖（应用特定）
    // ... 合并每个窗口部件的内联覆盖 ...

    style
}
```
