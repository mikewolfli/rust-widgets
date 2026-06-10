# Styling & Theming

The `rust-widgets` styling system resolves widget appearance through a cascading resolution chain: **Theme → ThemeOverrides → StatefulTheme → Inline CSS**. A CSS parser with selector engine and specificity, hot-reloadable stylesheets, an animation framework with 10 easing functions plus physics-based springs, and built-in Material Light/Dark themes provide comprehensive visual customization.

---

## Cascading Resolution Chain

Widget styles are resolved through four levels, each falling through to the next when a property is unset:

```
1. Global Theme defaults (ThemeManager → Theme)
2. ThemeOverrides per widget class (e.g., "Button", "Label")
3. Widget instance state (StatefulTheme → WidgetState)
4. Inline style overrides (CSS from StyleSheetManager)
```

---

## `WidgetStyle` — Resolved Visual Properties

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

Builder methods provide fluent construction:

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

**Style inheritance** — child falls back to parent for unset properties:

```rust
let parent = WidgetStyle::default()
    .with_background(Color::RED)
    .with_text_color(Color::BLUE);

let child = WidgetStyle::default();  // all None

let inherited = child.inherit_from(&parent);
// inherited.background_color == Some(RED)
// inherited.text_color == Some(BLUE)

// Child properties override parent
let child = WidgetStyle::default().with_background(Color::GREEN);
let inherited = child.inherit_from(&parent);
// inherited.background_color == Some(GREEN)  — child wins
```

**Style merging** — fill in missing properties from another style:

```rust
let mut base = WidgetStyle::default().with_background(Color::RED);
let overlay = WidgetStyle::default().with_text_color(Color::BLUE);
base.merge(&overlay);
// base.background_color == Some(RED)   — unchanged
// base.text_color == Some(BLUE)         — filled from overlay
```

---

## Padding, Margin, and Shadow

### `Padding`

Per-side inner spacing between widget border and content:

```rust
let padding = Padding::new(8, 12, 8, 12);      // top, right, bottom, left
let uniform = Padding::all(4);                   // all sides = 4
let symmetric = Padding::symmetric(6, 12);        // vertical=6, horizontal=12
let safe = Padding::normalized(-1, 4, -99, 8);   // clamps negatives → (0, 4, 0, 8)
```

### `Margin`

Per-side outer spacing outside the widget border:

```rust
let margin = Margin::new(8, 4, 8, 4);
let uniform = Margin::all(8);
let symmetric = Margin::symmetric(4, 8);
let safe = Margin::normalized(-5, 0, 3, 2);  // → (0, 0, 3, 2)
```

### `Shadow`

Drop-shadow effect on the widget:

```rust
let shadow = Shadow::new()
    .with_offset(0, 4)                   // shadow 4px below
    .with_blur(8)                        // 8px blur radius
    .with_color(Color::rgba(0, 0, 0, 60)); // semi-transparent black
```

---

## CSS System

### CSS Parser

The `CssParser` converts CSS text into a `StyleSheet` with selector matching and property application:

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

// Apply to a widget
let mut style = WidgetStyle::default();
CssParser::parse_and_apply(css, "Button", Some("primary"), Some("submit-btn"), Some(PseudoState::Hover), &mut style)?;
```

### `CssSelector` — Selector Engine

The selector engine supports kind, class, ID, state pseudo-classes, and AND-combinations:

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

**Specificity rules:**
- ID selector (`#id`) — highest specificity
- Class selector (`.class`) — medium
- Kind selector (`Widget`) — low
- Universal (`*`) — lowest
- Pseudo-state (`:hover`) — additive

When multiple rules match, the one with highest specificity wins for each property.

---

## `StyleSheetManager` — Global Registry

Global, thread-safe stylesheet registry with priority layering:

```rust
use rust_widgets::style::stylesheet::{global_stylesheet_manager, StyleSheetManager};

// Register named stylesheets with priority
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
    "#, 0);  // priority 0 (base)

    mgr.register("brand-override", r#"
        .accent {
            background-color: #ff6f00;
        }
    "#, 100);  // priority 100 (overrides)
}

// Apply all matching rules to a widget style
{
    let mgr = global_stylesheet_manager();
    let mut style = WidgetStyle::default();
    mgr.apply_to("Button", Some("accent"), None, Some(PseudoState::Normal), &mut style)?;
}

// Unregister a sheet
{
    let mut mgr = global_stylesheet_manager();
    mgr.unregister("brand-override");
}
```

**Priority layering:** sheets are sorted by `priority` (ascending) before application. Lower-priority values are applied first, higher-priority values override them.

---

## `CssWatcher` — Hot-Reload CSS Files

Poll-based file watcher that automatically reloads CSS into the global `StyleSheetManager` when the file changes:

```rust
use rust_widgets::style::css_watcher::CssWatcher;

let mut watcher = CssWatcher::new("theme.css", "main-theme");
watcher.set_poll_interval(500);  // check every 500ms (default)

// Poll in your main loop
loop {
    match watcher.poll() {
        Ok(true)  => println!("CSS reloaded — styles updated!"),
        Ok(false) => { /* no change */ }
        Err(e)    => eprintln!("CSS watch error: {}", e),
    }

    // ... render frame ...

    std::thread::sleep(std::time::Duration::from_millis(16));
}

// Force reload regardless of modification time
watcher.reload()?;
```

---

## Animation System

### Easing Functions — 10 Presets

```rust
use rust_widgets::style::animation::EasingFunction;

let t = 0.5;  // progress [0.0, 1.0]

let linear    = EasingFunction::Linear.apply(t);      // t
let ease_in   = EasingFunction::EaseIn.apply(t);      // t²
let ease_out  = EasingFunction::EaseOut.apply(t);     // 1 - (1-t)²
let ease_inout= EasingFunction::EaseInOut.apply(t);   // smooth S-curve

// Bounce
let bounce_in = EasingFunction::BounceIn.apply(t);    // overshoot + settle
let bounce_out= EasingFunction::BounceOut.apply(t);

// Elastic
let elastic_in  = EasingFunction::ElasticIn.apply(t); // spring-like
let elastic_out = EasingFunction::ElasticOut.apply(t);

// Back (overshoot)
let back_in  = EasingFunction::BackIn.apply(t);       // overshoot then in
let back_out = EasingFunction::BackOut.apply(t);      // in then overshoot
```

### `AnimationConfig`

Controls animation timing and behavior:

```rust
use rust_widgets::style::animation::{AnimationConfig, AnimationDirection, AnimationFillMode};

let config = AnimationConfig::new(Duration::from_millis(300))
    .with_delay(Duration::from_millis(100))
    .with_easing(EasingFunction::EaseInOut)
    .with_direction(AnimationDirection::Alternate)
    .with_fill_mode(AnimationFillMode::Forwards)
    .with_iterations(3);  // play 3 times

let infinite = AnimationConfig::new(Duration::from_millis(2000)).infinite();
```

### `AnimationDriver`

The core animation state machine:

```rust
let mut anim = Animation::new(config);
anim.on_complete(Box::new(|| println!("Animation finished!")));

anim.start();

// Each frame:
let progress = anim.current_progress();  // 0.0..=1.0
let eased = config.easing.apply(progress);

// Check state
if anim.is_running() { /* update visuals */ }
if anim.is_completed() { /* cleanup */ }

anim.pause();
anim.resume();
anim.stop();
anim.reset();
```

### `ColorAnimation`

Animate between two colors:

```rust
use rust_widgets::style::animation::ColorAnimation;

let mut color_anim = ColorAnimation::new(
    Color::RED,
    Color::BLUE,
    AnimationConfig::new(Duration::from_millis(500)).with_easing(EasingFunction::EaseInOut),
);

color_anim.start();

// Each frame:
let current_color = color_anim.current_value();
widget.set_background(current_color);
```

### `FloatAnimation`

Animate scalar values (opacity, position, size):

```rust
use rust_widgets::style::animation::FloatAnimation;

let mut fade = FloatAnimation::new(
    0.0,   // from
    1.0,   // to
    AnimationConfig::new(Duration::from_millis(300)),
);

fade.start();

// Each frame:
let current_opacity = fade.current_value();
widget.set_opacity(current_opacity);
```

### `KeyframeAnimation`

Multi-stop animation with percentage stops:

```rust
use rust_widgets::style::animation::KeyframeAnimation;

let mut keyframes = KeyframeAnimation::new(
    AnimationConfig::new(Duration::from_millis(1000)).infinite(),
);

keyframes.add_keyframe(0.0, 0.0);              // start: translateX(0)
keyframes.add_keyframe(0.25, 50.0);            // 25%: translateX(50)
keyframes.add_keyframe(0.5, 100.0);            // 50%: translateX(100)
keyframes.add_keyframe(0.75, 50.0);            // 75%: translateX(50)
keyframes.add_keyframe(1.0, 0.0);              // 100%: back to 0

keyframes.start();

// Each frame:
let offset = keyframes.current_value();        // interpolated value
widget.set_x(offset);
```

---

## Spring Animation — Physics-Based

`SpringAnimation` simulates a mass-spring-damper system for natural-feeling motion:

```rust
use rust_widgets::style::animation::SpringAnimation;

let mut spring = SpringAnimation::new(
    0.0,      // from value
    100.0,    // to value
    0.5,      // damping ratio (0.0 = no damping, 1.0 = critically damped)
    20.0,     // stiffness (higher = faster)
);

spring.start();

// Each frame:
let position = spring.current_value();
let velocity = spring.velocity();

// Settles naturally — no fixed duration
if spring.is_settled() {
    // Animation complete
}
```

**Tuning guide:**

| Damping | Stiffness | Behavior |
|---|---|---|
| 0.3 | 15 | Bouncy, slow settling |
| 0.5 | 20 | Bouncy with moderate settle |
| 0.7 | 25 | Slight bounce, fast settle |
| 1.0 | 20 | Critically damped (no bounce, fastest settle) |

---

## Animation Groups

### `ParallelAnimation` — Simultaneous

Play multiple animations at the same time:

```rust
use rust_widgets::style::animation_group::ParallelAnimation;

let mut group = ParallelAnimation::new();
group.add(fade_in);
group.add(slide_up);
group.add(scale_in);

group.start();

// All animations run simultaneously
if group.is_completed() {
    // All animations finished
}
```

### `SequentialAnimation` — Chained

Play animations one after another:

```rust
use rust_widgets::style::animation_group::SequentialAnimation;

let mut sequence = SequentialAnimation::new();
sequence.add(fade_out);
sequence.add(swap_content);
sequence.add(fade_in);

sequence.start();

// Animations play in order
// is_completed() returns true only when the last one finishes
```

### `AnimationGroup` — Mixed

Combine parallel and sequential groups:

```rust
use rust_widgets::style::animation_group::AnimationGroup;

let mut enter_animation = AnimationGroup::new();

// Fade in + slide up simultaneously
let mut enter_parallel = ParallelAnimation::new();
enter_parallel.add(fade_in);
enter_parallel.add(slide_up);

// Then wait
let delay = Animation::new(
    AnimationConfig::new(Duration::from_millis(500))
);

// Then bounce
let bounce = FloatAnimation::new(100.0, 120.0,
    AnimationConfig::new(Duration::from_millis(300)).with_easing(EasingFunction::BounceOut));

enter_animation.add(Box::new(enter_parallel));
enter_animation.add(Box::new(delay));
enter_animation.add(Box::new(bounce));

enter_animation.start();
```

---

## Theme System

### `Theme` — High-Level Definition

A complete theme defines colors, fonts, spacing, borders, and per-class overrides:

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

### `Colors` — Semantic Palette

```rust
let colors = Colors {
    background: Color::from_rgb(240, 240, 240),
    foreground: Color::BLACK,
    primary:    Color::from_rgb(33, 150, 243),    // brand blue
    secondary:  Color::from_rgb(158, 158, 158),    // neutral grey
    accent:     Color::from_rgb(255, 152, 0),      // orange accent
    error:      Color::from_rgb(244, 67, 54),      // red
    warning:    Color::from_rgb(255, 193, 7),      // amber
    success:    Color::from_rgb(76, 175, 80),      // green
    disabled:   Color::from_rgb(200, 200, 200),
    info:       Color::INFO,
};
```

### `Fonts` — Typography Tokens

```rust
let fonts = Fonts {
    regular:    Font::simple("Arial", 14.0),
    bold:       Font::bold("Arial", 14.0),
    italic:     Font { family: "Arial".into(), size: 14.0, italic: true, .. },
    monospace:  Font::simple("Courier New", 12.0),
    caption:    Font::simple("Arial", 11.0),    // footnotes, secondary
    body:       Font::simple("Arial", 14.0),    // default paragraph
    title:      Font::bold("Arial", 16.0),      // section titles
    headline:   Font::bold("Arial", 20.0),      // prominent headings
    display:    Font::bold("Arial", 28.0),      // large decorative
};
```

### `Spacing` — Scale Tokens

```rust
let spacing = Spacing {
    small: 4,
    medium: 8,
    large: 16,
    extra_large: 24,
};
```

### `Borders` — Border/Shadow Defaults

```rust
let borders = Borders {
    width: 1,         // default border width
    radius: 4,        // default corner radius
    shadow: true,     // enable drop shadows
};
```

### `ThemeOverrides` — Per-Class Tokens

Fine-tune specific widget classes without creating a full theme:

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

## `ThemeManager` — Runtime Theme Switching

```rust
use rust_widgets::theme::ThemeManager;

let mut manager = ThemeManager::new();

// Load from JSON
manager.load_theme("custom_theme.json")?;

// Switch themes at runtime
manager.set_theme("dark");

// Listen for theme changes via signal
manager.on_theme_changed().connect(|| {
    println!("Theme changed! Requesting repaint...");
});

// Resolve a widget style from the current theme
let button_style = manager.resolve_style("button");
// Returns a WidgetStyle with colors, fonts, padding, and shadow pre-populated

// Register a theme programmatically
let mut custom = Theme::default();
custom.name = "corporate".into();
custom.colors.primary = Color::from_rgb(0, 121, 107);
manager.register_theme(custom);

// Save current theme to JSON
manager.save_theme("exported_theme.json")?;
```

**Built-in themes:**

| Theme | Description |
|---|---|
| `"default"` | Material Light: grey background, blue primary, Arial fonts |
| `"dark"` | Material Dark: near-black background (#121212), light text, muted primaries |

```rust
// Dark theme preset
let dark = Theme::dark();
manager.register_theme(dark);
manager.set_theme("dark");
```

---

## `ThemeStateManager` — Light/Dark/Auto Mode

Manages per-widget-state themes with light/dark/auto switching:

```rust
use rust_widgets::style::theme_state::{ThemeStateManager, ThemeMode, StatefulTheme, WidgetState, StateTheme};

let mut light_theme = StatefulTheme::new("light");

// Define states for a button
light_theme.add_state(WidgetState::Normal, StateTheme::new(
    Color::from_rgb(33, 150, 243),   // background
    Color::WHITE,                     // foreground
    Color::WHITE,                     // text
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

// Dark theme variant
let mut dark_theme = StatefulTheme::new("dark");
// ... define dark states ...

// Create the manager
let mut state_manager = ThemeStateManager::new(light_theme, dark_theme);

// Switch modes
state_manager.set_mode(ThemeMode::Dark);
state_manager.toggle_mode();  // Dark → Light → Auto mode cycle

// Auto mode with time-based switching
state_manager.set_auto_switch(18, 6);  // dark mode from 6 PM to 6 AM UTC

// React to mode changes
state_manager.on_mode_changed(|mode| {
    match mode {
        ThemeMode::Light => println!("Switched to light theme"),
        ThemeMode::Dark => println!("Switched to dark theme"),
        ThemeMode::Auto => println!("Auto mode active"),
    }
});

// Resolve state-specific appearance
let hover_style: &StateTheme = state_manager.get_state_theme(&WidgetState::Hover);
widget.set_background(hover_style.background_color);
widget.set_foreground(hover_style.foreground_color);
widget.set_text_color(hover_style.text_color);
```

### `WidgetState` — 12 Interactive States

```rust
pub enum WidgetState {
    Normal,     // default resting state
    Hover,      // mouse cursor over widget
    Pressed,    // actively being clicked/pressed
    Focused,    // has keyboard focus
    Disabled,   // greyed out, no interaction
    Checked,    // checkbox/radio: toggled on
    Selected,   // list item: currently selected
    Active,     // window/dialog: frontmost
    Inactive,   // window/dialog: background
    Error,      // validation error
    Warning,    // validation warning
    Success,    // validation success
}
```

### `StateTheme` — Per-State Visual Properties

```rust
let mut state_theme = StateTheme::new(
    Color::WHITE,                   // background
    Color::BLACK,                   // foreground
    Color::from_rgb(33, 33, 33),   // text
)
    .with_border(Color::GRAY, 1)
    .with_shadow(Color::rgba(0, 0, 0, 30), (0, 2), 4)
    .with_opacity(0.95)
    .with_property("border-radius", "8");
```

**State transitions:** define animation durations between state pairs:

```rust
stateful_theme.set_transition(WidgetState::Normal, WidgetState::Hover, 200);   // 200ms hover enter
stateful_theme.set_transition(WidgetState::Hover, WidgetState::Normal, 150);   // 150ms hover exit
stateful_theme.set_transition(WidgetState::Normal, WidgetState::Pressed, 50);  // instant press
```

---

## `HighContrastMode`

Overrides all colors for accessibility:

```rust
pub enum HighContrastMode {
    None,
    BlackOnWhite,     // forced black text on white background
    WhiteOnBlack,     // forced white text on black background
    Custom { fg: Color, bg: Color },  // arbitrary custom pair
}
```

When a `HighContrastMode` other than `None` is active, theme color resolution ignores the theme palette and uses the forced foreground/background.

---

## `TouchTargetSize` — Per Device Class

Minimum interactive area by device class:

```rust
pub enum TouchTargetSize {
    Desktop,      // 32×32 pt, 8px spacing
    Tablet,       // 44×44 pt, 12px spacing
    Phone,        // 48×48 pt, 16px spacing
    Embedded,     // 40×40 pt, 10px spacing
    Projection,   // 24×24 pt, 6px spacing  (feature = "projection")
}

let size = TouchTargetSize::Phone.dimensions();    // Size(48, 48)
let spacing = TouchTargetSize::Phone.spacing();    // 16
```

Apply a per-device-class override to `WidgetStyle`:

```rust
let phone_style = WidgetStyle::default()
    .with_touch_target(TouchTargetSize::Phone.dimensions());
```

---

## Common Patterns

### Hover Animation with Style Transition

```rust
impl MyButton {
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseEnter { .. } => {
                self.state = WidgetState::Hover;
                self.hover_animation.start();  // animate to hover colors
                true
            }
            Event::MouseLeave { .. } => {
                self.state = WidgetState::Normal;
                self.hover_animation.reverse(); // animate back
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

        // Render with interpolated color...
    }
}
```

### Loading Spinner with Keyframe Animation

```rust
let mut spinner = FloatAnimation::new(
    0.0, 360.0,
    AnimationConfig::new(Duration::from_millis(1000)).infinite(),
);
spinner.start();

// In render loop:
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

### Complete Theme Definition (JSON)

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

Load it:

```rust
manager.load_theme("midnight.json")?;
manager.set_theme("midnight");
```

### Custom CSS with Hot Reload

```rust
// theme.css:
//   Button { background-color: #2196f3; border-radius: 8; }
//   Button:hover { background-color: #1976d2; }

let mut watcher = CssWatcher::new("theme.css", "main-theme");

// In main loop:
app.on_frame_update(move || {
    if watcher.poll() == Ok(true) {
        app.request_repaint_all();
    }
});
```

### Physics-Based Pull-to-Refresh

```rust
let mut spring = SpringAnimation::new(
    0.0,       // from (resting position)
    -80.0,     // to (pull-down offset)
    0.6,       // damping
    15.0,      // stiffness
);

spring.start();

// Drag gesture updates the target
match event {
    Event::Drag { delta, .. } => {
        spring.set_target(spring.target() + delta.y);
    }
    Event::TouchEnd { .. } => {
        spring.set_target(0.0); // snap back
    }
    _ => {}
}

// Each frame:
let offset = spring.current_value();
refresh_indicator.set_position(offset);
```

### Full Style Resolution Pipeline

```rust
fn resolve_widget_style(
    widget_id: ObjectId,
    theme_manager: &ThemeManager,
    state_manager: &ThemeStateManager,
    class: &str,
    state: &WidgetState,
) -> WidgetStyle {
    // 1. Base from theme
    let mut style = theme_manager.resolve_style(class);

    // 2. Stateful theme overlay
    let state_theme = state_manager.get_state_theme(state);
    style.background_color = style.background_color.or(Some(state_theme.background_color));
    style.text_color = style.text_color.or(Some(state_theme.text_color));
    style.border_color = style.border_color.or(Some(state_theme.border_color));
    style.border_width = if style.border_width == 0 { state_theme.border_width } else { style.border_width };
    style.opacity = style.opacity.map(|o| o * state_theme.opacity);

    // 3. CSS overrides from global stylesheet manager
    let mgr = global_stylesheet_manager();
    let _ = mgr.apply_to(class, None, None, Some(to_pseudo_state(state)), &mut style);

    // 4. Inline overrides (app-specific)
    // ... merge in per-widget inline overrides ...

    style
}
```
