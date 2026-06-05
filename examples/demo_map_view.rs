use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::map_view::{MapMarker, MapView};

fn main() {
    let mut map = MapView::new(Rect::new(0, 0, 640, 360));
    map.set_markers(vec![MapMarker::new("center", "Center", 0.0, 0.0)]);
    map.set_zoom(1.5);
    let svg = rust_widgets::widget::svg::render_to_svg(&mut map);
    println!("demo_map_view: rendered svg bytes={}", svg.len());
}
