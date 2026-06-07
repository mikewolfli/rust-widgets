use rust_widgets::core::Rect;
use rust_widgets::widget::base_widgets::button::Button;
use rust_widgets::widget::window::Window;

fn main() {
    let mut window = Window::new("Rust Widgets Demo".to_string(), Rect::new(0, 0, 800, 480));
    let mut button = Button::new("Start".to_string(), Rect::new(20, 60, 120, 36));
    button.set_text("Start demo".to_string());

    let window_svg = rust_widgets::widget::svg::render_to_svg(&mut window);
    let button_svg = rust_widgets::widget::svg::render_to_svg(&mut button);

    println!("demo_main: window_svg={} button_svg={}", window_svg.len(), button_svg.len());
}
