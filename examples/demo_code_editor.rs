use rust_widgets::core::Rect;
use rust_widgets::widget::special_widgets::code_editor::CodeEditor;

fn main() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 640, 400));
    editor.set_text("fn main() {\n    println!(\"hello\");\n}");
    let svg = rust_widgets::widget::svg::render_to_svg(&mut editor);
    println!("demo_code_editor: rendered svg bytes={}", svg.len());
}
