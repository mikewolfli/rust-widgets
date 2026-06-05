use rust_widgets::core::Rect;
use rust_widgets::widget::view_widgets::list_view::ListView;

fn main() {
    let mut list = ListView::new(Rect::new(0, 0, 320, 220));
    let svg = rust_widgets::widget::svg::render_to_svg(&mut list);
    println!("demo_list_view: rendered svg bytes={}", svg.len());
}
