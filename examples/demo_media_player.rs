#[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
use rust_widgets::core::Rect;

fn main() {
    #[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
    {
        use rust_widgets::widget::special_widgets::media_player::MediaPlayer;
        let mut player = MediaPlayer::new(Rect::new(0, 0, 640, 360));
        player.set_source("file:///tmp/demo.mp4", 30_000);
        player.play();
        let svg = rust_widgets::widget::svg::render_to_svg(&mut player);
        println!("demo_media_player: rendered svg bytes={}", svg.len());
    }
}
