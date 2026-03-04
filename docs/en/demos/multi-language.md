# Multi-language Demo

The multi-language demo showcases the internationalization capabilities of the rust_widgets library, supporting multiple languages including English, Simplified Chinese, Traditional Chinese, and French.

## Features

- **Language Selection**: Ability to switch between different languages
- **Translated UI**: All UI elements are translated based on the selected language
- **Dynamic Language Switching**: Languages can be changed at runtime without restarting the application
- **Comprehensive Translation**: All UI elements, including menus, buttons, labels, and error messages, are properly translated

## Supported Languages

| Language | Code | Status |
|----------|------|--------|
| English | en | ✅ |
| Simplified Chinese | zh-cn | ✅ |
| Traditional Chinese | zh-tw | ✅ |
| French | fr | ✅ |

## Translation Files

Translation files are located in the `i18n` directory:

- `i18n/en.json` - English translations
- `i18n/zh-cn.json` - Simplified Chinese translations
- `i18n/zh-tw.json` - Traditional Chinese translations
- `i18n/fr.json` - French translations

## Usage

1. **Build the demo**: `cargo build --example demo_multi_language`
2. **Run the demo**: `cargo run --example demo_multi_language`
3. **Select a language**: Use the language menu to switch between different languages

## Code Example

```rust
use rust_widgets::{create_window, create_button, create_label, create_menu_bar, attach_menu_bar_to_window, create_menu, menu_add_item, show_widget, run, init, poll_widget_trigger_event, WidgetTriggerKind, set_widget_text, i18n::translate, i18n::init_with_options};

fn main() {
    // Initialize the library
    init();
    
    // Initialize i18n with options
    init_with_options(Some("en"));
    
    // Create window
    let window = create_window(&translate("window_title"), 100, 100, 800, 600);
    
    // Create menu bar
    let menu_bar = create_menu_bar(window, 0, 0, 800, 30);
    attach_menu_bar_to_window(window, menu_bar);
    
    // Create language menu
    let lang_menu = create_menu(menu_bar, &translate("language"), 0, 0, 100, 30);
    let en_item = menu_add_item(lang_menu, "English", None);
    let zh_cn_item = menu_add_item(lang_menu, "简体中文", None);
    let zh_tw_item = menu_add_item(lang_menu, "繁體中文", None);
    let fr_item = menu_add_item(lang_menu, "Français", None);
    
    // Create widgets
    let button = create_button(window, &translate("button_text"), 350, 250, 100, 30);
    let label = create_label(window, &translate("label_text"), 350, 200, 200, 30);
    
    // Show window
    show_widget(window);
    
    // Main loop
    loop {
        // Poll for events
        if let Some(event) = poll_widget_trigger_event() {
            match event.kind {
                WidgetTriggerKind::Clicked if event.widget_id == button => {
                    set_widget_text(label, &translate("button_clicked"));
                }
                WidgetTriggerKind::Clicked if event.widget_id == en_item => {
                    // Switch to English
                    init_with_options(Some("en"));
                    update_ui(window, button, label);
                }
                WidgetTriggerKind::Clicked if event.widget_id == zh_cn_item => {
                    // Switch to Simplified Chinese
                    init_with_options(Some("zh-cn"));
                    update_ui(window, button, label);
                }
                WidgetTriggerKind::Clicked if event.widget_id == zh_tw_item => {
                    // Switch to Traditional Chinese
                    init_with_options(Some("zh-tw"));
                    update_ui(window, button, label);
                }
                WidgetTriggerKind::Clicked if event.widget_id == fr_item => {
                    // Switch to French
                    init_with_options(Some("fr"));
                    update_ui(window, button, label);
                }
                _ => {}
            }
        }
        
        // Run event loop
        run();
    }
}

fn update_ui(window: crate::core::ObjectId, button: crate::core::ObjectId, label: crate::core::ObjectId) {
    use rust_widgets::{set_widget_text, set_window_title, translate};
    set_window_title(window, &translate("window_title"));
    set_widget_text(button, &translate("button_text"));
    set_widget_text(label, &translate("label_text"));
}
```

## Translation File Example

### English (`i18n/en.json`)

```json
{
  "window_title": "Multi-language Demo",
  "language": "Language",
  "button_text": "Click Me",
  "label_text": "Hello, World!",
  "button_clicked": "Button clicked!"
}
```

### Simplified Chinese (`i18n/zh-cn.json`)

```json
{
  "window_title": "多语言演示",
  "language": "语言",
  "button_text": "点击我",
  "label_text": "你好，世界！",
  "button_clicked": "按钮被点击了！"
}
```

### Traditional Chinese (`i18n/zh-tw.json`)

```json
{
  "window_title": "多語言演示",
  "language": "語言",
  "button_text": "點擊我",
  "label_text": "你好，世界！",
  "button_clicked": "按鈕被點擊了！"
}
```

### French (`i18n/fr.json`)

```json
{
  "window_title": "Démonstration multilingue",
  "language": "Langue",
  "button_text": "Cliquez-moi",
  "label_text": "Bonjour le monde !",
  "button_clicked": "Bouton cliqué !"
}
```

## Screenshot

![Multi-language Demo](https://trae-api-cn.mchost.guru/api/ide/v1/text_to_image?prompt=A%20screenshot%20of%20a%20multi-language%20demo%20window%20with%20language%20selection%20menu%20and%20translated%20UI%20elements%20in%20a%20clean%20modern%20GUI%20layout&image_size=landscape_16_9)