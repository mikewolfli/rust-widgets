import pathlib

p = pathlib.Path("src/widget/runtime.rs")
t = p.read_text(encoding="utf-8")
diag = '''    #[test]
    fn diag_children() {
        let id = register(sample_editor("fn main() {}")).expect("r");
        assert!(set_geometry(id, Rect::new(0, 0, 800, 600)));
        let g = geometry_of(id).unwrap();
        let kids = with_widget(id, |w| w.base().children().len()).unwrap_or(999);
        println!("DIAG geometry={:?} children={} -> should={}", g, kids, should_track_damage(id));
        unregister(id);
    }

'''
anchor = "    /// The `Adaptive` run counter for a widget"
assert anchor in t
p.write_text(t.replace(anchor, diag + anchor, 1), encoding="utf-8")
print("diag added")
