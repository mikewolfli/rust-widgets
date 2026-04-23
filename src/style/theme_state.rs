use crate::core::Color;
use std::collections::HashMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetState {
    Normal,
    Hover,
    Pressed,
    Focused,
    Disabled,
    Checked,
    Selected,
    Active,
    Inactive,
    Error,
    Warning,
    Success,
}
impl Default for WidgetState {
    fn default() -> Self {
        Self::Normal
    }
}
#[derive(Debug, Clone)]
pub struct StateTheme {
    pub background_color: Color,
    pub foreground_color: Color,
    pub border_color: Color,
    pub border_width: u32,
    pub text_color: Color,
    pub shadow_color: Option<Color>,
    pub shadow_offset: (i32, i32),
    pub shadow_blur: u32,
    pub opacity: f32,
    pub custom_properties: HashMap<String, String>,
}
impl StateTheme {
    pub fn new(background: Color, foreground: Color, text: Color) -> Self {
        Self {
            background_color: background,
            foreground_color: foreground,
            border_color: Color::TRANSPARENT,
            border_width: 0,
            text_color: text,
            shadow_color: None,
            shadow_offset: (0, 0),
            shadow_blur: 0,
            opacity: 1.0,
            custom_properties: HashMap::new(),
        }
    }
    pub fn with_border(mut self, color: Color, width: u32) -> Self {
        self.border_color = color;
        self.border_width = width;
        self
    }
    pub fn with_shadow(mut self, color: Color, offset: (i32, i32), blur: u32) -> Self {
        self.shadow_color = Some(color);
        self.shadow_offset = offset;
        self.shadow_blur = blur;
        self
    }
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
    pub fn with_property(mut self, key: &str, value: &str) -> Self {
        self.custom_properties
            .insert(key.to_string(), value.to_string());
        self
    }
}
impl Default for StateTheme {
    fn default() -> Self {
        Self::new(Color::WHITE, Color::BLACK, Color::BLACK)
    }
}
#[derive(Debug, Clone)]
pub struct StatefulTheme {
    name: String,
    states: HashMap<WidgetState, StateTheme>,
    default_state: StateTheme,
    transitions: HashMap<(WidgetState, WidgetState), u32>,
}
impl StatefulTheme {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            states: HashMap::new(),
            default_state: StateTheme::default(),
            transitions: HashMap::new(),
        }
    }
    pub fn add_state(&mut self, state: WidgetState, theme: StateTheme) {
        self.states.insert(state, theme);
    }
    pub fn get_state(&self, state: &WidgetState) -> &StateTheme {
        self.states.get(state).unwrap_or(&self.default_state)
    }
    pub fn set_default_state(&mut self, theme: StateTheme) {
        self.default_state = theme;
    }
    pub fn set_transition(&mut self, from: WidgetState, to: WidgetState, duration_ms: u32) {
        self.transitions.insert((from, to), duration_ms);
    }
    pub fn get_transition(&self, from: &WidgetState, to: &WidgetState) -> Option<u32> {
        self.transitions.get(&(*from, *to)).copied()
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    Auto,
}
impl Default for ThemeMode {
    fn default() -> Self {
        Self::Light
    }
}
pub struct ThemeStateManager {
    light_theme: StatefulTheme,
    dark_theme: StatefulTheme,
    current_mode: ThemeMode,
    auto_switch_threshold: Option<(u8, u8)>,
}
impl ThemeStateManager {
    pub fn new(light: StatefulTheme, dark: StatefulTheme) -> Self {
        Self {
            light_theme: light,
            dark_theme: dark,
            current_mode: ThemeMode::Light,
            auto_switch_threshold: None,
        }
    }
    pub fn set_mode(&mut self, mode: ThemeMode) {
        self.current_mode = mode;
    }
    pub fn current_mode(&self) -> ThemeMode {
        self.current_mode
    }
    pub fn current_theme(&self) -> &StatefulTheme {
        match self.current_mode {
            ThemeMode::Light => &self.light_theme,
            ThemeMode::Dark => &self.dark_theme,
            ThemeMode::Auto => {
                if self.should_use_dark() {
                    &self.dark_theme
                } else {
                    &self.light_theme
                }
            }
        }
    }
    pub fn toggle_mode(&mut self) {
        self.current_mode = match self.current_mode {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Auto => ThemeMode::Light,
        };
    }
    pub fn set_auto_switch(&mut self, hour_start: u8, hour_end: u8) {
        self.auto_switch_threshold = Some((hour_start, hour_end));
    }
    fn should_use_dark(&self) -> bool {
        if let Some((start, end)) = self.auto_switch_threshold {
            let hour = 12;
            hour >= start && hour < end
        } else {
            false
        }
    }
    pub fn get_state_theme(&self, state: &WidgetState) -> &StateTheme {
        self.current_theme().get_state(state)
    }
}
impl Default for ThemeStateManager {
    fn default() -> Self {
        let light = StatefulTheme::new("light");
        let dark = StatefulTheme::new("dark");
        Self::new(light, dark)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_state_theme() {
        let theme = StateTheme::new(Color::WHITE, Color::BLACK, Color::BLACK)
            .with_border(Color::GRAY, 1)
            .with_opacity(0.8);
        assert_eq!(theme.background_color, Color::WHITE);
        assert_eq!(theme.border_width, 1);
        assert_eq!(theme.opacity, 0.8);
    }
    #[test]
    fn test_stateful_theme() {
        let mut theme = StatefulTheme::new("test");
        let normal = StateTheme::new(Color::WHITE, Color::BLACK, Color::BLACK);
        let hover = StateTheme::new(Color::LIGHT_GRAY, Color::BLACK, Color::BLACK);
        theme.add_state(WidgetState::Normal, normal);
        theme.add_state(WidgetState::Hover, hover);
        assert!(theme.get_state(&WidgetState::Normal).background_color == Color::WHITE);
        assert!(theme.get_state(&WidgetState::Hover).background_color == Color::LIGHT_GRAY);
    }
    #[test]
    fn test_theme_manager() {
        let light = StatefulTheme::new("light");
        let dark = StatefulTheme::new("dark");
        let mut manager = ThemeStateManager::new(light, dark);
        assert_eq!(manager.current_mode(), ThemeMode::Light);
        manager.set_mode(ThemeMode::Dark);
        assert_eq!(manager.current_mode(), ThemeMode::Dark);
        manager.toggle_mode();
        assert_eq!(manager.current_mode(), ThemeMode::Light);
    }
}
