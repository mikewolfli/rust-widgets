use rust_widgets::core::Font;
use rust_widgets::render::{RenderContext, SvgPaintBackend};

/// What the shared `text_line` says the line box is, versus what a consumer that draws its own
/// label would compute by hand, for the *same* context and font.
fn main() {
    let mut backend = SvgPaintBackend::new(rust_widgets::core::Size::new(300, 400));
    let ctx = RenderContext::new(&mut backend);
    let font = Font::default();

    // The shared derivation, on a band that is neither at 0 nor the same height as the font.
    let band = rust_widgets::core::Rect::new(17, 41, 120, 40);
    let line = ctx.text_line(band, &font);

    // A consumer doing what the crate says not to do: `band.y + band.height / 2`.
    let hand = (band.x, band.y + band.height as i32 / 2);

    println!("text_line(band) = {line:?}");
    println!("hand-rolled top-left = ({}, {})", hand.0, hand.1);
    println!("ctx.dpi_scale() = {}", ctx.dpi_scale());
    println!("measure_text(\"M\").height = {}", ctx.measure_text("M", &font).height);
}
