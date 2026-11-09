// Temporary probe: does the sample file dialog actually hold entries?
use rust_widgets::core::Rect;
use rust_widgets::widget::dialog::file_dialog::FileDialog;

fn main() {
    let mut dialog = FileDialog::open_file(Rect::new(0, 0, 240, 120));
    let filled = rust_widgets::sample_fill::apply("file_dialog", &mut dialog);
    println!("filled={filled} entries={} mode={:?}", dialog.entries().len(), dialog.mode());
    for entry in dialog.entries() {
        println!("  {} dir={}", entry.name, entry.is_directory);
    }
}
