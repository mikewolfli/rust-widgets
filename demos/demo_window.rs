//! Window demo

use rust_widgets::core::Rect;
use rust_widgets::widget::{Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the GUI library
    init();
    
    // Create a window
    let mut window = Window::new(
        "Hello Window".to_string(),
        Rect { x: 100, y: 100, width: 800, height: 600 }
    );
    
    let _handle = window.closed.connect(|| {
        println!("Window closed");
    });

    window.show();

    run();
}
