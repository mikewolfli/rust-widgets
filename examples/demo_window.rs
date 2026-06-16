use rust_widgets::core::Rect;
use rust_widgets::widget::window::Window;

fn main() {
    #[cfg(not(feature = "mini"))]
    {
        let mut window = Window::new("Demo Window".to_string(), Rect::new(0, 0, 640, 360));
        window.set_title("Window demo".to_string());
        let svg = rust_widgets::widget::svg::render_to_svg(&mut window);
        println!("demo_window: rendered svg bytes={}", svg.len());
    }
}
