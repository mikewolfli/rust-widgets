#[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
use rust_widgets::core::Rect;
#[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
use rust_widgets::widget::base_widgets::button::Button;

fn main() {
    #[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
    {
        let mut button = Button::new("Click me".to_string(), Rect::new(0, 0, 160, 36));
        button.set_text("Button demo".to_string());
        let svg = rust_widgets::widget::svg::render_to_svg(&mut button);
        println!("demo_button: rendered svg bytes={}", svg.len());
    }
}
