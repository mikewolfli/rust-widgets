//! XML layout loading demo.

use rust_widgets::xml::XmlLayoutLoader;

fn main() {
    let xml = r#"
    <Window id="main" title="Demo">
        <Button id="ok_btn" text="OK" />
        <Button id="cancel_btn" text="Cancel" />
    </Window>
    "#;

    let mut loader = XmlLayoutLoader::new();
    loader
        .load_layout_from_xml_str("main", xml)
        .expect("load xml layout");

    let ok_btn = loader.find_element_by_id("main", "ok_btn");
    println!("found ok button: {}", ok_btn.is_some());
}
