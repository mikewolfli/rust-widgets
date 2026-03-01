//! XML layout loading demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::Button;
use rust_widgets::xml::XmlLayoutLoader;

fn main() {
    // Define a declarative layout string with two buttons.
    let xml = r#"
    <Window id="main" title="Demo">
        <Button id="ok_btn" text="OK" />
        <Button id="cancel_btn" text="Cancel" />
    </Window>
    "#;

    // Parse and register the XML layout model.
    let mut loader = XmlLayoutLoader::new();
    loader
        .load_layout_from_xml_str("main", xml)
        .expect("load xml layout");

    // Query a node by id from the declarative model.
    let ok_btn = loader.find_element_by_id("main", "ok_btn");
    println!("found ok button in xml model: {}", ok_btn.is_some());

    // Instantiate runtime widgets and keep a bound id registry.
    let mut bound = loader
        .instantiate_bound_layout("main")
        .expect("instantiate bound layout");

    let ok_id = bound.id("ok_btn").expect("ok button id should exist");
    println!("bound ok button id: {ok_id}");

    bound
        .set_tooltip_by_name("ok_btn", "Submit current dialog")
        .expect("set tooltip by name");

    // Create and append an imperative widget into the bound tree.
    let dynamic_btn = Box::new(Button::new(
        "Extra".to_string(),
        Rect {
            x: 12,
            y: 64,
            width: 88,
            height: 28,
        },
    ));

    let dynamic_id = bound
        .add_imperative_widget("main", Some("extra_btn"), dynamic_btn)
        .expect("append imperative widget");

    println!("imperative widget appended: id={dynamic_id}");

    // Remove one declarative widget by its symbolic name.
    let _ = bound.remove_widget_by_name("cancel_btn");
    println!("declarative widget removed: cancel_btn");
}
