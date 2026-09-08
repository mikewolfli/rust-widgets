#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]
use rust_widgets::core::Rect;

fn main() {
    #[cfg(all(
        any(feature = "desktop", feature = "tablet", feature = "mobile"),
        not(any(feature = "mini", feature = "embedded"))
    ))]
    {
        use rust_widgets::widget::view_widgets::list_view::ListView;
        let mut list = ListView::new(Rect::new(0, 0, 320, 220));
        let svg = rust_widgets::widget::svg::render_to_svg(&mut list);
        println!("demo_list_view: rendered svg bytes={}", svg.len());
    }
}
