use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::terminal_view::TerminalView;

fn main() {
    let mut terminal = TerminalView::new(Rect::new(0, 0, 640, 360));
    terminal.append_output("Terminal ready");
    terminal.set_input_line("help");
    let _ = terminal.submit();
    let svg = rust_widgets::widget::svg::render_to_svg(&mut terminal);
    println!("demo_terminal: rendered svg bytes={}", svg.len());
}
