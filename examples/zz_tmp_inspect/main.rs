//! Temporary inspection harness (deleted after debugging).

use rust_widgets::core::{Point, Rect};
use rust_widgets::widget::svg::{render_to_svg, text_ink_boxes};

fn main() {
    // --- menu ---
    {
        use rust_widgets::widget::menu_toolbar::menu::Menu;
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
        menu.add_action("Open");
        menu.add_action("Save");
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 1000, 800));
        let svg = render_to_svg(&mut menu);
        println!("=== MENU ink boxes: {:?}", text_ink_boxes(&svg));
        println!("=== MENU heading_height {:?} item_height {:?}", 0, 0);
    }

    // --- line edit ---
    {
        use rust_widgets::widget::input_widgets::lineedit::LineEdit;
        let mut plain = LineEdit::new(Rect::new(0, 0, 240, 120));
        plain.set_text("12");
        let svg = render_to_svg(&mut plain);
        println!("=== LINEEDIT ink boxes: {:?}", text_ink_boxes(&svg));

        let mut prefixed = LineEdit::new(Rect::new(0, 0, 240, 120));
        prefixed.set_text("12");
        prefixed.set_prefix("$");
        let svg = render_to_svg(&mut prefixed);
        println!("=== LINEEDIT prefix ink boxes: {:?}", text_ink_boxes(&svg));
    }

    // --- textarea ---
    {
        use rust_widgets::widget::input_widgets::textarea::TextArea;
        let mut ta = TextArea::new("ab".to_string(), Rect::new(0, 0, 240, 120));
        ta.set_text("ab".to_string());
        let svg = render_to_svg(&mut ta);
        println!("=== TEXTAREA ink boxes: {:?}", text_ink_boxes(&svg));

        use rust_widgets::widget::input_widgets::lineedit::LineEdit;
        let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
        le.set_text("ab");
        let svg = render_to_svg(&mut le);
        println!("=== LINEEDIT ab ink boxes: {:?}", text_ink_boxes(&svg));
    }

    // --- spinbox ---
    {
        use rust_widgets::widget::input_widgets::spinbox::SpinBox;
        let mut sb = SpinBox::new(Rect::new(0, 0, 200, 24));
        sb.set_value(7);
        let svg = render_to_svg(&mut sb);
        println!("=== SPINBOX bare ink boxes: {:?}", text_ink_boxes(&svg));

        let mut sb2 = SpinBox::new(Rect::new(0, 0, 200, 24));
        sb2.set_value(7);
        sb2.set_prefix("a-very-long-prefix".to_string());
        let svg2 = render_to_svg(&mut sb2);
        println!("=== SPINBOX long-prefix ink boxes: {:?}", text_ink_boxes(&svg2));
    }
}
