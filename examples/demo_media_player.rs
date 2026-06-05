use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::media_player::MediaPlayer;

fn main() {
    let mut player = MediaPlayer::new(Rect::new(0, 0, 640, 360));
    player.set_source("file:///tmp/demo.mp4", 30_000);
    player.play();
    let svg = rust_widgets::widget::svg::render_to_svg(&mut player);
    println!("demo_media_player: rendered svg bytes={}", svg.len());
}
