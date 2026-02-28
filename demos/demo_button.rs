//! Button demo

use rust_widgets::core::Rect;
use rust_widgets::widget::{Button, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the GUI library
    init();
    
    // Create a window
    let mut window = Window::new(
        "Button Demo".to_string(),
        Rect { x: 100, y: 100, width: 400, height: 300 }
    );
    
    let button = Button::new(
        "Click Me!".to_string(),
        Rect { x: 150, y: 120, width: 100, height: 40 }
    );

    let _handle = button.activated.connect(|| {
        println!("Button clicked!");
    });

    window.add_child(button.id());

    window.show();

    run();
}
