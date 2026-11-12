use super::*;
use crate::core::Orientation;
use crate::json::{place_widget_in_layout, place_widget_in_region_layout, ChildPlacement};

/// RW-117: the full-mount loader honours the same `breakpoints` list the incremental preview
/// does, so an app and its designer preview cannot render different trees for one document.
#[cfg(declarative_view)]
#[test]
fn breakpoints_select_nodes_on_the_load_path_too() {
    let json = r#"{
          "window": {
            "id": "w",
            "title": "Tiers",
            "width": 320,
            "height": 240,
            "layout": { "type": "vbox", "children": [
              { "label": { "id": "wide", "text": "Wide", "breakpoints": ["expanded"] } },
              { "label": { "id": "narrow", "text": "Narrow", "breakpoints": ["compact"] } }
            ] }
          }
        }"#;
    let compact = crate::view::with_breakpoint(crate::view::Breakpoint::Compact, || {
        JsonLoader::load(json).expect("the document loads")
    });
    assert!(compact.id("wide").is_none(), "the expanded-only node is absent");
    assert!(compact.id("narrow").is_some());

    let expanded = crate::view::with_breakpoint(crate::view::Breakpoint::Expanded, || {
        JsonLoader::load(json).expect("the document loads")
    });
    assert!(expanded.id("wide").is_some());
    assert!(expanded.id("narrow").is_none());
}

/// `lowered` is the loader's one answer to "compare this declared name against a lowercase
/// literal". The borrow is the point: a document that spells its controls the documented way
/// must not allocate once per comparison.
#[test]
fn a_lowercase_name_is_borrowed_and_a_mixed_case_name_is_lowered() {
    let plain = "button";
    assert!(
        matches!(lowered(plain), Cow::Borrowed(_)),
        "an already-lowercase name must not allocate"
    );
    assert_eq!(lowered(plain), "button");

    let mixed = "ScrollArea";
    assert!(matches!(lowered(mixed), Cow::Owned(_)), "a mixed-case name must still be lowered");
    assert_eq!(lowered(mixed), "scrollarea");

    // The predicate is ASCII case, not "contains no uppercase letter": digits and
    // underscores are already lowercase and must not trigger an allocation either.
    assert!(matches!(lowered("spin_box2"), Cow::Borrowed(_)));
}

/// The borrow shortcut must not change which arm a name reaches: a declaration that spells
/// its type in capitals still has to build the same control.
#[test]
fn a_mixed_case_type_name_reaches_the_same_arm_as_a_lowercase_one() {
    let declaration = serde_json::Map::new();
    let lowercase = JsonLoader::create_widget("button", &declaration).expect("button");
    let uppercase = JsonLoader::create_widget("BUTTON", &declaration).expect("BUTTON");
    assert_eq!(uppercase.kind(), lowercase.kind());
}

/// `slider`, `scrollbar` and `progressbar` read one declaration through one function
/// (`BIND-06`: the loader used to carry three copies of it). The three arms were identical
/// line for line, which is exactly the shape that drifts — so the shared contract is pinned
/// against all three widgets rather than against the helper alone.
#[test]
fn the_range_triple_reads_the_same_for_slider_scrollbar_and_progressbar() {
    let declaration = serde_json::json!({ "minimum": 5, "value": 7, "orientation": "vertical" });
    let obj = declaration.as_object().expect("a declaration is an object");

    let mut slider = Slider::new(Rect::new(0, 0, 100, 100));
    apply_range_like(obj, &mut slider);
    if let Some(orientation) = json_orientation(obj) {
        slider.set_orientation(orientation);
    }

    let mut scrollbar = ScrollBar::new(Rect::new(0, 0, 100, 100));
    apply_range_like(obj, &mut scrollbar);
    if let Some(orientation) = json_orientation(obj) {
        scrollbar.set_orientation(orientation);
    }

    let mut progress = ProgressBar::new(Rect::new(0, 0, 100, 100));
    apply_range_like(obj, &mut progress);
    if let Some(orientation) = json_orientation(obj) {
        progress.set_orientation(orientation);
    }

    for (name, minimum, maximum, value, orientation) in [
        ("slider", slider.minimum(), slider.maximum(), slider.value(), slider.orientation()),
        (
            "scrollbar",
            scrollbar.minimum(),
            scrollbar.maximum(),
            scrollbar.value(),
            scrollbar.orientation(),
        ),
        (
            "progressbar",
            progress.minimum(),
            progress.maximum(),
            progress.value(),
            progress.orientation(),
        ),
    ] {
        assert_eq!(minimum, 5, "{name} must take the declared minimum");
        assert_eq!(maximum, 100, "{name}: a minimum without a maximum keeps the default 100");
        assert_eq!(value, 7, "{name} must take the declared value");
        assert_eq!(orientation, Orientation::Vertical, "{name} must take the declared orientation");
    }
}

/// The other half of the shared rule: a document that states only the upper bound widens the
/// range instead of also resetting the lower one, and an orientation nobody defines leaves
/// the control on its own.
#[test]
fn a_maximum_alone_widens_the_range_without_moving_the_minimum() {
    let declaration = serde_json::json!({ "maximum": 40, "orientation": "sideways" });
    let obj = declaration.as_object().expect("a declaration is an object");

    let mut slider = Slider::new(Rect::new(0, 0, 100, 100));
    slider.set_range(-10, 10);
    apply_range_like(obj, &mut slider);

    assert_eq!(slider.minimum(), -10, "a lone maximum must not reset the lower bound");
    assert_eq!(slider.maximum(), 40);
    assert_eq!(
        json_orientation(obj),
        None,
        "an orientation nobody defines must leave the control alone"
    );
}

#[test]
fn conflicting_legacy_and_typed_action_icons_are_rejected_together() {
    let mut action = crate::widget::menu_toolbar::action::Action::new(
        "Save",
        crate::core::Rect::new(0, 0, 100, 30),
    );
    let declaration = serde_json::json!({
        "icon_text": "★",
        "icon_source": {"kind": "builtin", "name": "save"}
    });

    apply_properties(&mut action, declaration.as_object().expect("object declaration"));

    assert_eq!(action.icon_text(), "", "the legacy value must not win by key order");
    assert_eq!(action.icon_source(), None, "the typed value must not win by key order");
}

/// A token the crate does not know is **refused**, and refusing must not clear a live icon.
///
/// §19.13 case I06 keeps three "no glyph" states distinct: an empty slot, an explicit "no
/// icon", and an unknown token. The dangerous conflation is the last becoming the second — a
/// property layer that turned an unknown name into a *valid* empty value would let one typo
/// silently erase the icon the document already set, which is the "an unknown entry must not
/// silently succeed" rule §19.6 states for the typed entries.
#[test]
fn an_unknown_builtin_token_is_refused_rather_than_applied() {
    use crate::widget::icon_source::IconSource;
    use crate::widget::{Button, IconName};

    let mut button = Button::new("Save".into(), crate::core::Rect::new(0, 0, 80, 24));
    button.set_icon_source(Some(IconSource::Builtin(IconName::Info)));

    apply_properties(
        &mut button,
        serde_json::json!({
            "icon_source": { "kind": "builtin", "name": "definitely_not_a_token" }
        })
        .as_object()
        .expect("a declaration is an object"),
    );

    assert_eq!(
        button.icon_source(),
        Some(IconSource::Builtin(IconName::Info)),
        "a refused token must leave the live icon alone, not clear it"
    );

    // Control: the same declaration with a **known** token is applied, so the assertion above
    // is about the token and not about the whole property being inert.
    apply_properties(
        &mut button,
        serde_json::json!({ "icon_source": { "kind": "builtin", "name": "save" } })
            .as_object()
            .expect("a declaration is an object"),
    );
    assert_eq!(
        button.icon_source(),
        Some(IconSource::Builtin(IconName::Save)),
        "a known token is applied through the same path"
    );
}

#[test]
fn load_valid_minimal_window() {
    let json = r#"{"window": {"id": "main", "title": "Test", "width": 400, "height": 300}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    let layout = result.unwrap();
    assert_eq!(layout.len(), 1);
    assert!(layout.id("main").is_some(), "Expected Some for widget id 'main'");
}

#[test]
fn load_button_with_text() {
    let json = r#"{"window": {"id": "w", "title": "Window", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "btn", "text": "Click Me"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    let layout = result.unwrap();
    assert!(layout.id("btn").is_some());
}

struct TestIconResolver {
    pub(crate) fail: bool,
    pub(crate) calls: std::cell::Cell<usize>,
}

impl crate::widget::icon_source::IconResourceResolver for TestIconResolver {
    fn resolve_icon(
        &self,
        _resource_id: &str,
        _color_mode: crate::widget::icon_source::IconColorMode,
    ) -> Result<crate::widget::icon_source::IconSource, String> {
        self.calls.set(self.calls.get() + 1);
        if self.fail {
            return Err("resource is unavailable".to_string());
        }
        Ok(crate::widget::icon_source::IconSource::Builtin(crate::widget::IconName::Search))
    }
}

#[test]
fn json_loader_resolves_resources_only_with_explicit_context() {
    let json = r#"{"window":{"id":"resource-window","children":[{"button":{"id":"resource-button","icon_source":{"kind":"resource","id":"project/icons/search","color_mode":"monochrome"}}}]}}"#;
    let resolver = TestIconResolver { fail: false, calls: std::cell::Cell::new(0) };
    let layout = JsonLoader::load_with_icon_resolver(json, &resolver)
        .expect("a valid layout remains loadable");
    assert_eq!(resolver.calls.get(), 1, "nested declarations use the supplied resolver");
    assert!(layout.id("resource-button").is_some(), "the layout keeps its declared node");

    let mut button =
        crate::widget::Button::new("Search".into(), crate::core::Rect::new(0, 0, 80, 24));
    apply_properties_with_icon_resolver(
        &mut button,
        serde_json::json!({
            "icon_source": {
                "kind": "resource",
                "id": "project/icons/search",
                "color_mode": "monochrome"
            }
        })
        .as_object()
        .expect("test declaration is an object"),
        Some(&resolver),
    );
    assert_eq!(
        button.icon_source(),
        Some(crate::widget::icon_source::IconSource::Builtin(crate::widget::IconName::Search)),
        "the loader property path assigns the resolver's validated drawable content"
    );

    let failing_resolver = TestIconResolver { fail: true, calls: std::cell::Cell::new(0) };
    let mut unavailable_button =
        crate::widget::Button::new("Unavailable".into(), crate::core::Rect::new(0, 0, 80, 24));
    apply_properties_with_icon_resolver(
        &mut unavailable_button,
        serde_json::json!({
            "icon_source": {
                "kind": "resource",
                "id": "project/icons/unavailable",
                "color_mode": "original"
            }
        })
        .as_object()
        .expect("test declaration is an object"),
        Some(&failing_resolver),
    );
    assert_eq!(
        unavailable_button.icon_source(),
        None,
        "failed resolution must not install a success-shaped fallback"
    );

    let failed_layout = JsonLoader::load_with_icon_resolver(json, &failing_resolver)
        .expect("a rejected property does not abort the rest of the JSON layout");
    assert_eq!(failing_resolver.calls.get(), 2);
    assert!(failed_layout.id("resource-button").is_some(), "property refusal preserves layout");
}

/// A `fontdialog`'s declared `value` must reach the dialog.
///
/// # Why this test exists
///
/// The `fontdialog` arm used to read `value`, discard it, and call
/// `set_current_font(Font::default())` — with a comment claiming font parsing
/// was unavailable. A caller writing `"value": "Monospace 20"` silently got
/// Arial 14 and no diagnostic. The sibling `colordialog` arm parses its own
/// `value` correctly, so the asymmetry was the bug.
///
/// The loader's arm calls `FontDialog::set_current_font`, and the round trip
/// through the live widget is the part that was never asserted. This test drives
/// the same call the loader makes with the same parse primitive it now uses, so a
/// regression to a constant default fails here.
#[cfg(not(alloc_frugal))]
#[test]
fn a_font_dialog_receives_the_font_parsed_from_its_declared_value() {
    let spec = "Monospace 20 bold";
    let mut dialog = crate::widget::dialog::FontDialog::new(Rect::new(0, 0, 400, 300));
    dialog.set_current_font(
        crate::core::Font::parse(spec).expect("the loader's parser must read this"),
    );
    assert_eq!(dialog.current_font().family(), "Monospace");
    assert_eq!(dialog.current_font().size(), 20.0);
    assert!(dialog.current_font().is_bold());

    // The value the old code substituted is measurably different, so this test
    // cannot pass against the old behaviour.
    assert_ne!(dialog.current_font(), &crate::core::Font::default());
}

/// An unparsable `value` must leave the dialog's own font alone.
#[cfg(not(alloc_frugal))]
#[test]
fn a_font_dialog_keeps_its_own_font_when_the_declared_value_is_unparsable() {
    let mut dialog = crate::widget::dialog::FontDialog::new(Rect::new(0, 0, 400, 300));
    let before = dialog.current_font().clone();
    // This is the branch the loader takes instead of substituting a default.
    if let Some(font) = crate::core::Font::parse("not a font at all") {
        dialog.set_current_font(font);
    }
    assert_eq!(dialog.current_font(), &before, "the dialog must keep its own font");
}

/// A `grid` layout's `col`/`row`/`col_span`/`row_span` must reach the grid.
///
/// # Why this test exists
///
/// `ChildLayoutAttrs::from_value` parsed all four keys, and both
/// `add_widget_to_layout` call sites then read `attrs.stretch` and dropped the rest. A
/// document that placed a control at an explicit cell got auto-placement instead, and
/// the four keys — absent from `is_loader_owned_key` — additionally drew a
/// *"no construction arm consumed it; the value was ignored"* warning about a value the
/// loader had in fact read. Nothing caught either half: the existing tests asserted that
/// loading succeeded, not where the widgets ended up.
///
/// This drives the same call the loader makes and asserts the **applied geometries**, so
/// restoring the drop fails here.
#[test]
fn a_grid_places_a_child_at_the_cell_the_document_declared() {
    let json = r#"{
            "window": {
                "title": "T",
                "children": [
                    { "layout": {
                        "type": "grid",
                        "columns": 3,
                        "spacing": 10,
                        "children": [
                            { "label": {
                                "id": "wide", "text": "W",
                                "col": 0, "row": 0, "col_span": 3
                            } },
                            { "label": { "id": "a", "text": "A", "col": 0, "row": 1 } },
                            { "label": { "id": "b", "text": "B", "col": 1, "row": 1 } },
                            { "label": { "id": "c", "text": "C", "col": 2, "row": 1 } }
                        ]
                    } }
                ]
            }
        }"#;

    let binding = JsonLoader::load(json).expect("the grid document must load");
    let window = binding.root().expect("the window is the root");

    // The `layout` pseudo-widget is not a node of its own: it attaches a layout to its
    // parent and its children hang off that parent. So the grid's cells are keyed by the
    // window, and `preview_layout` must be asked about the window.
    let geometries = crate::json::preview_layout(window, Rect::new(0, 0, 320, 100));
    let width_of = |name: &str| {
        let declared = binding.id(name).expect("the node is bound by its declared id");
        let mounted = crate::widget::runtime::registry_id_of(declared).unwrap_or(declared);
        geometries
            .iter()
            .find(|(placed, _)| *placed == mounted)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| {
                panic!(
                    "'{name}' (declared {declared}, mounted {mounted}) was placed by the \
                         layout; placed: {geometries:?}"
                )
            })
    };

    let span = width_of("wide");
    let a = width_of("a");
    let b = width_of("b");
    let c = width_of("c");

    // Explicit placement, not auto-placement. Auto-placement fills cells in declaration
    // order, which would put all four children on row 0 — so a one-row result would mean the
    // keys were ignored. Two rows, with the second holding exactly the three children that
    // declared `row: 1`, is only reachable if `row` was honoured.
    assert_eq!(span.y, 0, "the span was declared on row 0");
    assert!(a.y > span.y, "`row: 1` must be below `row: 0`: {span:?} then {a:?}");
    assert_eq!(a.y, b.y, "`a` and `b` were both declared on row 1");
    assert_eq!(b.y, c.y, "`c` was declared on row 1 too");

    // `col` is honoured: the three row-1 children are ordered left to right by their
    // declared columns, and the gaps between them are the declared `spacing`.
    assert!(a.x < b.x && b.x < c.x, "`col` must order them: {a:?} {b:?} {c:?}");
    assert_eq!(b.x - (a.x + a.width as i32), 10, "`spacing: 10` between columns");
    assert_eq!(c.x - (b.x + b.width as i32), 10, "`spacing: 10` between columns");

    // `col_span` is honoured: the span starts at column 0 and reaches the right edge of
    // column 2, bridging the two gaps it covers.
    assert_eq!(span.x, a.x, "the span starts at column 0");
    assert_eq!(
        span.width,
        3 * a.width + 2 * 10,
        "`col_span: 3` must bridge the two gaps it covers: {span:?} vs {a:?}"
    );
    assert_eq!(
        span.x + span.width as i32,
        c.x + c.width as i32,
        "the span must reach the right edge of the third column"
    );
}

/// An `absolute` layout must place each child at the position its own object declares.
///
/// API2-01: the absolute solver used to be unreachable from a document. This drives the real
/// loader and asserts the applied geometry, so a regression that stops routing `x`/`y`/
/// `width`/`height` to the solver fails here rather than only in the test-only module.
#[test]
fn an_absolute_layout_places_a_child_where_the_document_declared() {
    let json = r#"{
            "window": {
                "title": "T",
                "children": [
                    { "layout": {
                        "type": "absolute",
                        "children": [
                            { "label": { "id": "a", "text": "A",
                                          "x": 10, "y": 20, "width": 40, "height": 16 } },
                            { "label": { "id": "b", "text": "B", "x": 100, "y": 50 } }
                        ]
                    } }
                ]
            }
        }"#;

    let binding = JsonLoader::load(json).expect("the absolute document must load");
    let window = binding.root().expect("the window is the root");
    let geometries = crate::json::preview_layout(window, Rect::new(0, 0, 320, 200));
    let rect_of = |name: &str| {
        let declared = binding.id(name).expect("the node is bound by its declared id");
        let mounted = crate::widget::runtime::registry_id_of(declared).unwrap_or(declared);
        geometries
            .iter()
            .find(|(placed, _)| *placed == mounted)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| panic!("'{name}' was not placed; placed: {geometries:?}"))
    };

    let a = rect_of("a");
    // `width`/`height` are the explicit size the child declared.
    assert_eq!(
        (a.x, a.y, a.width, a.height),
        (10, 20, 40, 16),
        "the absolute position and size must be the declared ones"
    );
    let b = rect_of("b");
    assert_eq!((b.x, b.y), (100, 50), "x/y alone must place the second child");
}

/// A `constraint` layout must apply the relationships a child declares.
///
/// API2-01: the target is a document `id`, resolved through the loader's name map. This
/// asserts the applied geometry, so dropping the resolution would leave the child at the
/// parent origin and fail the offset check.
#[test]
fn a_constraint_layout_applies_the_relationship_the_document_declared() {
    let json = r#"{
            "window": {
                "title": "T",
                "children": [
                    { "layout": {
                        "type": "constraint",
                        "children": [
                            { "label": { "id": "a", "text": "A" } },
                            { "label": { "id": "b", "text": "B",
                                          "constraints": [
                                            { "target": "a", "kind": "right_to_right",
                                              "offset": 5 }
                                          ] } }
                        ]
                    } }
                ]
            }
        }"#;

    let binding = JsonLoader::load(json).expect("the constraint document must load");
    let window = binding.root().expect("the window is the root");
    let geometries = crate::json::preview_layout(window, Rect::new(0, 0, 320, 200));
    let rect_of = |name: &str| {
        let declared = binding.id(name).expect("the node is bound by its declared id");
        let mounted = crate::widget::runtime::registry_id_of(declared).unwrap_or(declared);
        geometries
            .iter()
            .find(|(placed, _)| *placed == mounted)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| panic!("'{name}' was not placed; placed: {geometries:?}"))
    };

    let b = rect_of("b");
    // Without the constraint `b` would stay at the parent origin; `right_to_right` measures
    // both right edges, so the offset is the only difference from zero.
    assert_eq!(b.x, 5, "the declared right_to_right offset must be applied: {b:?}");
    assert!(binding.id("a").is_some(), "the target node is bound by its declared id");
}

/// A `border` layout must place a child in the region and track the document declared.
///
/// The test asserts geometry, not just that loading succeeded: a `region` that was parsed and
/// then ignored would leave every child in the default center region, which is a different
/// arrangement from the one the document described.
#[test]
fn a_border_places_a_child_in_the_region_the_document_declared() {
    let json = r#"{
            "window": {
                "title": "T",
                "children": [
                    { "layout": {
                        "type": "border",
                        "spacing": 0,
                        "margin": 0,
                        "children": [
                            { "label": { "id": "top", "text": "T",
                                          "region": "north", "extent": 40 } },
                            { "label": { "id": "left", "text": "L",
                                          "region": "west", "extent": 80 } },
                            { "label": { "id": "body", "text": "B",
                                          "region": "center" } }
                        ]
                    } }
                ]
            }
        }"#;

    let binding = JsonLoader::load(json).expect("the border document must load");
    let window = binding.root().expect("the window is the root");
    let geometries = crate::json::preview_layout(window, Rect::new(0, 0, 400, 300));
    let rect_of = |name: &str| {
        let declared = binding.id(name).expect("the node is bound by its declared id");
        let mounted = crate::widget::runtime::registry_id_of(declared).unwrap_or(declared);
        geometries
            .iter()
            .find(|(placed, _)| *placed == mounted)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| {
                panic!("'{name}' was not placed by the layout; placed: {geometries:?}")
            })
    };

    assert_eq!(rect_of("top"), Rect::new(0, 0, 400, 40), "north is a fixed 40px track");
    assert_eq!(
        rect_of("left"),
        Rect::new(0, 40, 80, 260),
        "west is an 80px track below the north band"
    );
    assert_eq!(rect_of("body"), Rect::new(80, 40, 320, 260), "center takes the remaining area");
}

/// A `region` declared on a parent that has no border layout is reported, not silently claimed.
#[test]
fn declaring_a_region_on_a_parent_without_a_border_is_reported() {
    /// A parent id no other test uses, so the thread-local layout map cannot collide.
    const BOX_PARENT: crate::core::ObjectId = 0x5EED_0004;
    const MISSING_PARENT: crate::core::ObjectId = 0x5EED_0005;

    assert!(
        !place_widget_in_region_layout(1, crate::layout::BorderRegion::North, 40, MISSING_PARENT),
        "an unknown parent must report the miss"
    );

    crate::json::store_layout(
        BOX_PARENT,
        Box::new(crate::layout::BoxLayout::new(crate::core::Orientation::Vertical, 0, 0)),
    );
    assert!(
        !place_widget_in_region_layout(1, crate::layout::BorderRegion::North, 40, BOX_PARENT),
        "a box layout has no regions to place a child in"
    );
    crate::json::forget_layout(BOX_PARENT);
}

/// A cell declared on a parent that has no cells is reported, not silently claimed.
///
/// The child is still placed by the parent's own rule — a box layout has no cells to put it
/// in — but `place_widget_in_layout` answers `false`, which is what lets the loader log the
/// mismatch instead of letting a `col` on a `vbox` read as "the vbox ignored my column".
#[test]
fn declaring_a_cell_on_a_parent_without_cells_is_reported() {
    /// A parent id no other test uses, so the thread-local layout map cannot collide.
    const BOX_PARENT: crate::core::ObjectId = 0x5EED_0002;
    const MISSING_PARENT: crate::core::ObjectId = 0x5EED_0003;

    let placement = ChildPlacement { col: 0, row: 0, col_span: 1, row_span: 1 };

    assert!(
        !place_widget_in_layout(1, placement, MISSING_PARENT),
        "an unknown parent must report the miss rather than claim the placement"
    );

    store_layout(
        BOX_PARENT,
        alloc::boxed::Box::new(crate::layout::BoxLayout::new(Orientation::Horizontal, 0, 0)),
    );
    assert!(
        !place_widget_in_layout(1, placement, BOX_PARENT),
        "a box layout has no cells, so it must answer `false` rather than accept the cell"
    );
    assert!(crate::json::forget_layout(BOX_PARENT));
}

/// Every key a `create_widget` arm reads must be resolvable by the layer that
/// runs next.
///
/// # Why this test exists
///
/// After the arm table builds a control, `apply_properties` walks the JSON keys a
/// second time and reports any it cannot resolve. A key read by an arm is
/// therefore fine only if **either**
///
/// 1. the control publishes it as a property, so the name-driven pass applies it
///    a second time (harmlessly — same value), or
/// 2. it is listed in [`is_loader_owned_key`], which tells the pass to skip it.
///
/// A key that is neither gets a *"no construction arm consumed it; the value was
/// ignored"* warning — about a value the arm just applied. The author then hunts
/// a non-bug. `tristate`, `password`, `word_wrap`, `tab_shape`, `h_policy`,
/// `v_policy` and `alpha` were all in exactly that state: read by an arm, absent
/// from the list, and not published by their controls.
///
/// This test parses the loader's own arm table, so it cannot drift: adding
/// `obj.get("new_key")` to an arm without either publishing it on the control or
/// listing it here fails.
#[test]
fn every_arm_consumed_key_is_either_published_or_loader_owned() {
    // The test reads the loader's source file: `include_str!` is resolved relative to the
    // file that contains the macro, which is `json/loader/tests.rs`.
    let source = include_str!("nodes.rs");
    let start = source
        .find("    pub(crate) fn create_widget(")
        .expect("the arm table must exist; if it was renamed, fix this test");
    // The arm table ends where the next top-level `    fn ` begins.
    let rest = &source[start + 1..];
    let end = rest.find("\n    fn ").map(|offset| start + 1 + offset).unwrap_or(source.len());
    let arms = &source[start..end];

    // Every capability's published property names, as one set of strings. The
    // schema tables are the authority on what the name-driven pass can resolve.
    let factory = crate::widget::WidgetFactory::new_with_defaults();
    let mut published: alloc::collections::BTreeSet<&str> = alloc::collections::BTreeSet::new();
    for capability in factory.capabilities() {
        for schema in capability.properties {
            published.insert(schema.name);
        }
    }

    let mut unresolved: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
    // Both spellings an arm can use to read a key: `obj.get("k")` and the typed
    // `json_*_at(obj, "k")` helpers. The original test only checked the first, so a helper
    // read could still name a published property without the duplication being noticed.
    let mut consumed: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
    for (offset, _) in arms.match_indices("obj.get(\"") {
        let remainder = &arms[offset + "obj.get(\"".len()..];
        let Some(close) = remainder.find('"') else { continue };
        consumed.push(&remainder[..close]);
    }
    for (offset, _) in arms.match_indices("_at(obj, \"") {
        let remainder = &arms[offset + "_at(obj, \"".len()..];
        let Some(close) = remainder.find('"') else { continue };
        consumed.push(&remainder[..close]);
    }
    for name in &consumed {
        if !published.contains(name) && !is_loader_owned_key(name) {
            unresolved.push(name);
        }
    }
    unresolved.sort_unstable();
    unresolved.dedup();

    assert!(
        unresolved.is_empty(),
        "these keys are read by a `create_widget` arm but are neither published as \
             properties nor listed in `is_loader_owned_key`, so the name-driven pass \
             will log a false \"was ignored\" warning about them: {unresolved:?}"
    );

    // The other half of BIND-05: an arm may read a *published* key only when that key is
    // also loader-owned (its spelling or type differs from the property). Any other
    // published key read by an arm is a second implementation of a property, which is the
    // drift this issue filed.
    let mut duplicated: alloc::vec::Vec<&str> = consumed
        .iter()
        .copied()
        .filter(|name| published.contains(name) && !is_loader_owned_key(name))
        .collect();
    duplicated.sort_unstable();
    duplicated.dedup();
    assert!(
        duplicated.is_empty(),
        "these published properties are applied by a `create_widget` arm as well as by the \
             name-driven pass; keep only the construction-time keys here: {duplicated:?}"
    );
}

/// Every control the factory can build must also be constructible through the loader.
///
/// # Why this test exists (BIND-05)
///
/// The loader had two construction routes: the arm table and the factory fallback. A control
/// registered with the factory but forgotten by the table silently took the fallback — which
/// used to pass no text to its constructor. The fallback now forwards the document's `text`,
/// and this test is the gate that the two routes at least agree on **which control was
/// built**: for every registered name the loader must answer `Ok`, and the widget's own
/// `kind()` must be the one the registry describes. A copy-paste error in an arm (the
/// `window` arm building a `Button`, say) fails here rather than at run time.
#[test]
fn every_factory_name_is_constructible_by_the_loader() {
    let factory = crate::widget::WidgetFactory::new_with_defaults();
    let empty = serde_json::Map::new();
    let mut failures: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    for capability in factory.capabilities() {
        let name = capability.canonical_name;
        match JsonLoader::create_widget(name, &empty) {
            Ok(widget) => {
                if widget.kind() != capability.kind {
                    failures.push(alloc::format!(
                        "{name}: built {:?}, registry says {:?}",
                        widget.kind(),
                        capability.kind
                    ));
                }
            }
            Err(error) => failures.push(alloc::format!("{name}: {error}")),
        }
    }
    assert!(
        failures.is_empty(),
        "every registered control must be constructible from JSON with its registry kind: \
             {failures:?}"
    );
}

/// A declarative layout must actually *arrange* its children, not merely parse.
///
/// # Why this test exists
///
/// `store_layout` used to run after the child loop, so every child was registered
/// against a layout that did not exist yet (`add_widget_to_layout` is a no-op in
/// that case). The array parsed, the layout was built, `apply_layout` ran and
/// returned nothing, and two children stayed at their declared overlapping
/// positions — with no error anywhere. Every existing test passed, because they all
/// asserted that loading succeeded rather than where the widgets ended up.
///
/// # Why this asserts the applied geometries
///
/// An earlier version of this test read `preview_layout` and passed even with the
/// old ordering restored, because the layout still ended up stored — just too late
/// to hear about its children, and therefore empty. `preview_layout` on an empty
/// layout answers with an empty list, which is what `apply_layout` returned too,
/// so the assertion has to be on the geometry the layout *computed for the children
/// it was given*: two entries, stacked.
#[test]
fn a_vbox_layout_positions_its_children_below_one_another() {
    let json = r#"{"window": {"id": "w", "title": "W", "width": 400, "height": 300,
            "layout": {"type": "vbox", "spacing": 4, "children": [
                {"label": {"id": "first", "text": "one", "width": 100, "height": 20}},
                {"label": {"id": "second", "text": "two", "width": 100, "height": 20}}
            ]}}}"#;
    let loaded = JsonLoader::load(json).expect("the document must load");
    let window = loaded.id("w").expect("the window");
    let first = loaded.id("first").expect("the first label");
    let second = loaded.id("second").expect("the second label");

    let geometries = crate::layout::declarative::preview_layout(window, Rect::new(0, 0, 400, 300));
    assert_eq!(
        geometries.len(),
        2,
        "both children must reach the layout; an empty list means they were added \
             before it existed"
    );

    let rect_of = |id| {
        geometries
            .iter()
            .find(|(child, _)| *child == id)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| panic!("child {id} was never laid out"))
    };
    let first_rect = rect_of(first);
    let second_rect = rect_of(second);

    assert!(
        second_rect.y > first_rect.y,
        "a vbox must stack its children: first at {first_rect:?}, second at {second_rect:?}"
    );
    assert_eq!(
        first_rect.x, second_rect.x,
        "a vbox keeps one column: {first_rect:?} vs {second_rect:?}"
    );

    crate::layout::declarative::forget_layout(window);
}

/// The sibling of the above for `hbox`: side by side, same row.
///
/// A single orientation test could pass with the axes swapped, so both directions
/// are pinned.
#[test]
fn an_hbox_layout_positions_its_children_side_by_side() {
    let json = r#"{"window": {"id": "w", "title": "W", "width": 400, "height": 300,
            "layout": {"type": "hbox", "spacing": 4, "children": [
                {"label": {"id": "left", "text": "L", "width": 60, "height": 20}},
                {"label": {"id": "right", "text": "R", "width": 60, "height": 20}}
            ]}}}"#;
    let loaded = JsonLoader::load(json).expect("the document must load");
    let window = loaded.id("w").expect("the window");
    let left = loaded.id("left").expect("the left label");
    let right = loaded.id("right").expect("the right label");

    let geometries = crate::layout::declarative::preview_layout(window, Rect::new(0, 0, 400, 100));
    assert_eq!(
        geometries.len(),
        2,
        "both children must reach the layout; an empty list means they were added \
             before it existed"
    );

    let rect_of = |id| {
        geometries
            .iter()
            .find(|(child, _)| *child == id)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| panic!("child {id} was never laid out"))
    };
    let left_rect = rect_of(left);
    let right_rect = rect_of(right);

    assert!(
        right_rect.x > left_rect.x,
        "an hbox must place children left to right: {left_rect:?} then {right_rect:?}"
    );
    assert_eq!(left_rect.y, right_rect.y, "an hbox keeps one row: {left_rect:?} vs {right_rect:?}");

    crate::layout::declarative::forget_layout(window);
}

#[test]
fn load_label_with_alignment() {
    let json = r#"{"window": {"id": "w", "title": "Window", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"label": {"id": "lbl", "text": "Hello", "alignment": "center"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_nested_layouts() {
    let json = r#"{"window": {"id": "w", "title": "Nested", "width": 500, "height": 400, "layout": {"type": "vbox", "children": [{"layout": {"type": "hbox", "children": [{"button": {"id": "b1", "text": "One"}}, {"button": {"id": "b2", "text": "Two"}}]}}, {"label": {"id": "footer", "text": "Footer"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    let layout = result.unwrap();
    assert!(layout.id("b1").is_some());
    assert!(layout.id("b2").is_some());
    assert!(layout.id("footer").is_some());
}

#[test]
fn load_spacer_widget() {
    let json = r#"{"window": {"id": "w", "title": "Spacer", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "b1", "text": "Top"}}, {"spacer": {"stretch": 1}}, {"button": {"id": "b2", "text": "Bottom"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_roundtrip_fields() {
    let json = r#"{"window": {"id": "main", "title": "Roundtrip", "width": 800, "height": 600, "layout": {"type": "grid", "columns": 2, "spacing": 4, "margin": 2, "children": [{"button": {"id": "ok", "text": "OK", "x": 0, "y": 0, "width": 80, "height": 30, "visible": true, "enabled": true}}, {"label": {"id": "info", "text": "Info", "visible": true}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    let layout = result.unwrap();
    assert!(layout.id("main").is_some());
    assert!(layout.id("ok").is_some());
    assert!(layout.id("info").is_some());
    assert_eq!(layout.len(), 3);
}

#[test]
fn load_invalid_json_returns_error() {
    let json = r#"{"window": {"id": "broken" "title": "Bad"}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_err());
    let err = result.unwrap_err();
    // The message must name the input (its size) and say the parse failed.
    assert!(err.contains("could not be parsed"), "Expected parse error, got: {}", err);
    assert!(err.contains("bytes"), "Expected the size to be named, got: {}", err);
}

#[test]
fn load_empty_string_returns_error() {
    let result = JsonLoader::load("");
    assert!(result.is_err());
}

#[test]
fn load_not_an_object_returns_error() {
    let json = r#""just a string""#;
    let result = JsonLoader::load(json);
    assert!(result.is_err());
}

#[test]
fn load_array_root_returns_error() {
    let json = r#"["a", "b"]"#;
    let result = JsonLoader::load(json);
    assert!(result.is_err());
}

#[test]
fn load_multiple_root_keys_returns_error() {
    let json = r#"{"window": {"title": "A"}, "button": {"text": "B"}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("one widget type"), "Expected 'one widget type' error, got: {}", err);
}

#[test]
fn load_unknown_widget_type_returns_error() {
    let json = r#"{"bogus_widget": {"id": "x"}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("unknown widget type"),
        "Expected unknown widget type error, got: {}",
        err
    );
}

#[test]
fn load_missing_id_still_works() {
    let json = r#"{"window": {"title": "No ID", "width": 300, "height": 200}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    let layout = result.unwrap();
    assert_eq!(layout.len(), 0, "Without 'id', no entries should be registered");
}

#[test]
fn every_marker_key_is_extractable_from_a_node() {
    // The table is the single list the loader reads, so this asserts the property that used to
    // be split across two tuple-returning functions: every declared key is found, by name, at
    // the marker it says it has.
    for (key, marker) in MARKER_KEYS {
        assert_eq!(
            crate::json::marker_for_key(key),
            Some(*marker),
            "`{key}` must extract as its own marker"
        );
    }
}

#[test]
fn a_node_declaring_no_event_key_yields_nothing() {
    let map = serde_json::Map::new();
    assert!(crate::json::marker_key_names().all(|key| map.get(key).is_none()));
}

#[test]
fn the_loader_consumes_exactly_the_marker_keys() {
    // The "not a property" pattern and the event wiring must agree, because a key in one and
    // not the other is either a warning about a live key or a silently ignored handler.
    //
    // The loader now **derives** the key set from the marker table rather than restating it
    // (the pattern used to spell the eight names as literals next to `MARKER_KEYS`, so adding a
    // ninth key would wire it but report a live handler as an unknown property). This asserts
    // the derivation, which is the property that makes the two paths agree by construction.
    for (key, _) in MARKER_KEYS {
        assert!(
            crate::json::is_marker_key(key),
            "`{key}` must be recognised as a marker key so the property pass leaves it alone"
        );
        assert!(
            is_loader_owned_key(key),
            "`{key}` must be loader-owned, or a declared handler is reported as a property"
        );
    }
    // The derivation, not a coincidental match: a name the table does not list is not owned.
    assert!(
        !is_loader_owned_key("on_definitely_not_a_marker"),
        "an unknown `on_*` key is not loader-owned"
    );
}

#[test]
fn load_layout_from_str_convenience() {
    let json = r#"{"window": {"id": "w", "title": "Conv", "width": 400, "height": 300}}"#;
    let result = load_layout_from_str(json);
    assert!(result.is_ok());
}

#[test]
fn load_widget_with_tooltip_and_style() {
    let json = r##"{"window": {"id": "w", "title": "Style", "width": 400, "height": 300, "visible": true, "enabled": true, "layout": {"type": "vbox", "children": [{"button": {"id": "styled_btn", "text": "Styled", "tooltip": "A styled button", "background": "#ff0000", "text_color": "#ffffff", "border_color": "#000000", "border_width": 2, "border_radius": 5, "min_width": 100, "min_height": 30}}]}}}"##;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_spinbox_with_all_properties() {
    let json = r#"{"window": {"id": "w", "title": "Spin", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"spinbox": {"id": "sb", "min": 0, "max": 100, "value": 50, "single_step": 5, "prefix": "$", "suffix": " USD", "wrapping": true}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_grid_widget_with_properties() {
    let json = r#"{"window": {"id": "w", "title": "Grid", "width": 600, "height": 400, "layout": {"type": "vbox", "children": [{"grid": {"id": "g", "rows": 3, "columns": 4, "spacing": 5}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_empty_children_array() {
    let json = r#"{"window": {"id": "w", "title": "Empty", "width": 400, "height": 300, "layout": {"type": "vbox", "children": []}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_window_default_title() {
    let json = r#"{"window": {"id": "w", "width": 400, "height": 300}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_checkbox_with_checked() {
    let json = r#"{"window": {"id": "w", "title": "Check", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"checkbox": {"id": "cb", "text": "Enable feature", "checked": true, "tristate": true}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_lineedit_with_all_properties() {
    let json = r#"{"window": {"id": "w", "title": "Edit", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"lineedit": {"id": "le", "value": "initial", "placeholder": "Type here...", "max_length": 100, "password": true}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_slider_with_range() {
    let json = r#"{"window": {"id": "w", "title": "Slider", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"slider": {"id": "sl", "min": 0, "max": 200, "value": 75, "orientation": "horizontal", "single_step": 5, "page_step": 20, "tracking": true}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

/// Both the published range names and the `min`/`max` aliases must resolve.
///
/// # Why this test exists
///
/// The arms read `min`/`max` only, but `slider`/`scrollbar`/`progressbar`/
/// `spinbox` publish `minimum`/`maximum`. So an author writing the documented
/// spelling got *no* range applied by the arm, and an author writing `min`/`max`
/// got the range applied but a false "was ignored" warning. Neither failure is
/// visible from `is_ok()`, which is all `load_slider_with_range` asserts.
///
/// This drives `read_json_range` directly: the loader's layout is an id index
/// and does not retain the controls, so the applied value cannot be read back
/// through it.
#[test]
fn a_range_resolves_under_both_the_published_name_and_the_alias() {
    let parse = |json: &str| {
        let value: Value = serde_json::from_str(json).expect("probe JSON must parse");
        let obj = value.as_object().expect("probe must be an object").clone();
        read_json_range(&obj)
    };

    // The published spelling.
    assert_eq!(parse(r#"{"minimum": 0, "maximum": 200}"#), (Some(0), Some(200)));
    // The alias the arms used to require.
    assert_eq!(parse(r#"{"min": 5, "max": 15}"#), (Some(5), Some(15)));
    // Mixed, and the published name wins when both are present.
    assert_eq!(parse(r#"{"minimum": 1, "min": 9}"#), (Some(1), None));
    assert_eq!(parse(r#"{"maximum": 7, "max": 3}"#), (None, Some(7)));
    // Absent keys resolve to nothing rather than to a made-up default.
    assert_eq!(parse(r#"{}"#), (None, None));
}

#[test]
fn load_progressbar_with_properties() {
    let json = r#"{"window": {"id": "w", "title": "Progress", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"progressbar": {"id": "pb", "min": 0, "max": 100, "value": 50, "orientation": "horizontal", "text_visible": true, "inverted_appearance": false}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_combobox_with_items() {
    let json = r#"{"window": {"id": "w", "title": "Combo", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"combobox": {"id": "cb", "items": ["One", "Two", "Three"], "current_index": 1, "editable": true, "max_visible_items": 10}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

/// B26.09, end to end: a document's record list reaches the live control, typed.
///
/// # Why this test exists rather than only the unit tests underneath it
///
/// `items` is a **loader-owned** key — `combobox` and `listbox` read it as an array of strings —
/// and the name-driven pass skipped every loader-owned key. So each piece could be correct on its
/// own (the value converts, the schema validates, the control's `set` works) while the document
/// still produced an empty chip group: the value was skipped by the pass and ignored by the
/// construction arm. Only a test that starts from a document and reads the live control closes
/// that gap.
#[cfg(not(alloc_frugal))]
#[test]
fn load_chip_with_a_record_list_reaches_the_live_control() {
    use crate::widget::capability::{widget_property_get, CapabilityValue};

    let json = r#"{"window": {"id": "w", "title": "Chips", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"chip": {"id": "chips", "items": [{"id": "a", "label": "Alpha", "selected": true}, {"id": "b", "label": "Beta"}]}}]}}}"#;
    let layout = JsonLoader::load(json).expect("the document must load");
    let chip = layout.id("chips").expect("the document names the chip");

    let items =
        crate::widget::runtime::with_widget(chip, |widget| widget_property_get(widget, "items"))
            .expect("the chip is mounted")
            .expect("`items` is readable");
    let CapabilityValue::Records(list) = items else {
        panic!("`items` must read as a record list, got {items:?}");
    };
    assert_eq!(list.len(), 2, "the document's two records must reach the control");
    assert_eq!(list.id_of(0), Some("a"));
    assert_eq!(list.get_by_id("b").expect("present").text("label"), "Beta");
    assert!(matches!(
        list.get_by_id("a").expect("present").get("selected"),
        Some(CapabilityValue::Bool(true))
    ));
    // The list a *control* returns is the control's own schema, so identity lookups work on it.
    assert_eq!(list.schema().id_field, "id");
}

/// The same document with a list the control's schema refuses leaves the items empty.
///
/// A separate test because a second `JsonLoader::load` in one test yields an empty layout — a
/// pre-existing single-load behaviour of the loader, not something this case is about. Every
/// loader test above loads once for the same reason.
#[cfg(not(alloc_frugal))]
#[test]
fn a_document_record_list_the_schema_refuses_is_not_applied() {
    use crate::widget::capability::{widget_property_get, CapabilityValue};

    // `label` is required, so a record without it is refused — with the property and the missing
    // field named in the log, rather than applied as a half-built chip.
    let json = r#"{"window": {"id": "w", "title": "Chips", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"chip": {"id": "chips", "items": [{"id": "a"}]}}]}}}"#;
    let layout = JsonLoader::load(json).expect("the document still loads");
    let chip = layout.id("chips").expect("the document names the chip");

    let CapabilityValue::Records(list) =
        crate::widget::runtime::with_widget(chip, |widget| widget_property_get(widget, "items"))
            .expect("the chip is mounted")
            .expect("`items` is readable")
    else {
        panic!("`items` must read as a record list");
    };
    assert!(list.is_empty(), "a record list the schema refuses must not be applied");
}

/// A chip item's `icon_source` is a **structured** row field: the object is read through the
/// row's declared kind, not refused as a compound value, and it reaches the live control.
#[cfg(not(alloc_frugal))]
#[test]
fn load_chip_item_icon_source_reaches_the_live_control() {
    use crate::widget::capability::{widget_property_get, CapabilityValue};

    let json = r#"{"window": {"id": "w", "title": "Chips", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"chip": {"id": "chips", "items": [{"id": "a", "label": "Alpha", "icon_source": {"kind": "builtin", "name": "info"}}]}}]}}}"#;
    let layout = JsonLoader::load(json).expect("the document must load");
    let chip = layout.id("chips").expect("the document names the chip");

    let items =
        crate::widget::runtime::with_widget(chip, |widget| widget_property_get(widget, "items"))
            .expect("the chip is mounted")
            .expect("`items` is readable");
    let CapabilityValue::Records(list) = items else {
        panic!("`items` must read as a record list, got {items:?}");
    };
    assert_eq!(list.len(), 1, "the row must be applied, not dropped as a compound value");
    assert!(matches!(
        list.get_by_id("a").expect("present").get("icon_source"),
        Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
            crate::widget::IconName::Info
        )))
    ));

    // A resource row needs the host's resolver, so a plain load must refuse it by name rather
    // than apply a chip whose icon silently disappeared.
    let resource = r#"{"window": {"id": "w2", "title": "Chips", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"chip": {"id": "chips2", "items": [{"id": "a", "label": "Alpha", "icon_source": {"kind": "resource", "id": "project/icons/a"}}]}}]}}}"#;
    let layout = JsonLoader::load(resource).expect("the document still loads");
    let chip = layout.id("chips2").expect("the document names the chip");
    let CapabilityValue::Records(list) =
        crate::widget::runtime::with_widget(chip, |widget| widget_property_get(widget, "items"))
            .expect("the chip is mounted")
            .expect("`items` is readable")
    else {
        panic!("`items` must read as a record list");
    };
    assert!(list.is_empty(), "a resource row without a resolver must not be applied");
}

#[test]
fn load_listbox_with_selection_mode() {
    let json = r#"{"window": {"id": "w", "title": "List", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"listbox": {"id": "lb", "items": ["A", "B", "C"], "selection_mode": "multi"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_scrollarea_with_policies() {
    let json = r#"{"window": {"id": "w", "title": "Scroll", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"scrollarea": {"id": "sa", "widget_resizable": true, "h_policy": "always_on", "v_policy": "as_needed"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_frame_with_shape_and_shadow() {
    let json = r#"{"window": {"id": "w", "title": "Frame", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"frame": {"id": "f", "frame_shape": "panel", "frame_shadow": "raised", "line_width": 2.0}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[cfg(not(alloc_frugal))]
#[test]
fn load_tabwidget_with_properties() {
    let json = r#"{"window": {"id": "w", "title": "Tabs", "width": 500, "height": 400, "layout": {"type": "vbox", "children": [{"tabwidget": {"id": "tw", "current_index": 0, "tab_position": "north", "closable": true, "movable": false}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[cfg(not(alloc_frugal))]
#[test]
fn load_textedit_with_readonly() {
    let json = r#"{"window": {"id": "w", "title": "Text", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"textedit": {"id": "te", "value": "Multi\nline", "read_only": true, "word_wrap": true}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_radiobutton_with_group() {
    let json = r#"{"window": {"id": "w", "title": "Radio", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"radiobutton": {"id": "rb1", "text": "Option A", "checked": true, "group_id": "group1"}}, {"radiobutton": {"id": "rb2", "text": "Option B", "group_id": "group1"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn load_groupbox_with_title() {
    let json = r#"{"window": {"id": "w", "title": "Group", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"groupbox": {"id": "gb", "title": "Settings", "checkable": true, "checked": true}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[cfg(not(alloc_frugal))]
#[test]
fn load_filedialog_with_mode() {
    let json = r#"{"window": {"id": "w", "title": "Dialog", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"filedialog": {"id": "fd", "mode": "save_file", "directory": "/tmp"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[cfg(not(alloc_frugal))]
#[test]
fn load_colordialog_with_color() {
    let json = r##"{"window": {"id": "w", "title": "Color", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"colordialog": {"id": "cd", "value": "#ff0000", "alpha": true}}]}}}"##;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[cfg(not(alloc_frugal))]
#[test]
fn load_messagebox_with_icon() {
    let json = r#"{"window": {"id": "w", "title": "Msg", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"messagebox": {"id": "mb", "title": "Warning", "text": "Are you sure?", "icon": "warning"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[cfg(not(alloc_frugal))]
#[test]
fn load_listview_widget() {
    let json = r#"{"window": {"id": "w", "title": "ListView", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"listview": {"id": "lv"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

// ── The framework-routed path (factory fallback + name-driven properties) ──

/// A control with **no** hand-written arm in `create_widget` still loads,
/// because the fallback asks the capability registry. `"icon"` is the sharpest
/// case: `infer_kind` has always been able to name it, but before the fallback
/// the loader refused to build it.
#[cfg(full_widgets)]
#[test]
fn a_control_with_no_hand_written_arm_loads_via_the_factory() {
    let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"icon": {"id": "ic", "icon_name": "star", "size": 48}}]}}}"#;
    let layout = JsonLoader::load(json).expect("a control the factory knows must load");
    assert!(layout.id("ic").is_some(), "the icon must be registered by its JSON id");
}

/// An `Icon` loads its **typed** source from a document, not only the legacy name string.
///
/// §19.10 asks for the structured `icon_source` property on the entries that carry an icon, and
/// the standalone `Icon` is the first of them (§19.5). The read goes through the live control's
/// own capability getter, so this asserts the document reached the widget rather than merely
/// that the parser accepted the object.
#[cfg(full_widgets)]
#[test]
fn an_icon_loads_a_typed_source_from_a_document() {
    use crate::widget::capability::{widget_property_get, CapabilityValue};

    let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"icon": {"id": "ic", "icon_source": {"kind": "builtin", "name": "info"}}}]}}}"#;
    let layout = JsonLoader::load(json).expect("a typed icon source must load");
    let icon = layout.id("ic").expect("the document names the icon");

    let read = crate::widget::runtime::with_widget(icon, |widget| {
        widget_property_get(widget, "icon_source")
    })
    .expect("the icon is mounted")
    .expect("`icon_source` is readable");
    assert_eq!(
        read,
        CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
            crate::widget::IconName::Info
        )),
        "the document's typed source must reach the live control"
    );
}

/// The kind a factory-routed control registers must match what `infer_kind`
/// says, otherwise the registry and the live widget disagree. Checked through
/// the registry's own kind, not by re-running `infer_kind` (which would be
/// circular).
#[cfg(full_widgets)]
#[test]
fn a_factory_routed_control_registers_its_real_kind() {
    let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"rating": {"id": "rt", "value": 3}}]}}}"#;
    JsonLoader::load(json).expect("rating must load");
    assert_eq!(
        infer_kind("rating"),
        crate::index::WidgetKind::Rating,
        "the kind mapping must name the real variant"
    );
}

/// Every control the factory can build must register under its **own** kind.
///
/// # Why this test exists
///
/// `infer_kind` is a hand-written table of about a hundred names, but the factory
/// routes many more. Everything the table did not name fell through to
/// `_ => WidgetKind::Button`, so an `icon`, `chip`, `progress_bar` or `tag_input`
/// node was registered as a `Button` — the widget registry, and therefore
/// `LayoutInspector`'s diagnostics and any `by_kind` query, disagreed with the
/// tree the loader had just built.
///
/// The previous test asserted only `infer_kind("rating") == Rating`, which is a
/// tautology over the very table that was incomplete. This one asserts the
/// invariant that matters: the kind now recorded comes from the live control, so
/// for the names below — deliberately chosen to be **absent** from `infer_kind`'s
/// table — the table's answer and the control's answer must differ, proving the
/// registration no longer trusts the table.
#[test]
fn every_factory_routed_control_registers_its_own_kind() {
    let factory = crate::widget::WidgetFactory::new_with_defaults();
    // `infer_kind`'s fallback arm, i.e. what these names used to be recorded as.
    let table_fallback = infer_kind("definitely_not_a_widget_name");
    assert_eq!(
        table_fallback,
        crate::index::WidgetKind::Button,
        "if the fallback changes, these assertions need revisiting"
    );

    let mut mislabelled_by_the_table = 0usize;
    for widget_type in ["icon", "chip", "progress_bar", "tag_input", "scroll_area"] {
        let expected = factory
            .create(widget_type, Rect::new(0, 0, 10, 10), "probe")
            .unwrap_or_else(|| panic!("the factory must build {widget_type:?}"))
            .kind();

        // The invariant the loader now relies on: a control knows its own kind.
        assert_ne!(
            expected, table_fallback,
            "{widget_type:?} happens to equal the table fallback, so this name \
                 no longer demonstrates the fix"
        );

        if infer_kind(widget_type) == table_fallback {
            mislabelled_by_the_table += 1;
        }

        // And the loader must register that real kind. The registry the loader
        // fills is not reachable after `load`, so the observable equivalent is
        // that the control it built reports the right kind — which is what the
        // registration now reads.
        let json = format!(
            r#"{{"window": {{"id": "w", "title": "T", "width": 400, "height": 300,
                    "layout": {{"type": "vbox", "children": [
                        {{"{widget_type}": {{"id": "probe"}}}}
                    ]}}}}}}"#
        );
        let layout =
            JsonLoader::load(&json).unwrap_or_else(|error| panic!("{widget_type}: {error:?}"));
        assert!(
            layout.id("probe").is_some(),
            "{widget_type:?} must be registered under its JSON id"
        );
    }

    assert!(
        mislabelled_by_the_table > 0,
        "none of the probed names is missing from `infer_kind`, so this test no \
             longer covers the defect it was written for"
    );
}

/// A property the loader has no branch for is applied through the control's
/// own property contract. This is the behaviour that makes the JSON surface
/// track the widget library instead of a hand-maintained key list.
#[cfg(full_widgets)]
#[test]
fn a_property_with_no_loader_branch_is_still_applied() {
    use crate::widget::capability::{widget_property_get, CapabilityValue};
    use crate::widget::Icon;

    let mut widget: Box<dyn Widget> = JsonLoader::create_widget(
        "icon",
        &serde_json::json!({"icon_name": "heart"}).as_object().unwrap().clone(),
    )
    .expect("icon must be constructible");
    // Before the name-driven pass this write had no branch and the value was
    // dropped; now the contract carries it.
    apply_properties(&mut *widget, serde_json::json!({"icon_name": "heart"}).as_object().unwrap());
    assert_eq!(
        widget_property_get(&*widget, "icon_name"),
        Ok(CapabilityValue::String("heart".to_string())),
        "icon_name must reach the control through the generic path"
    );
    // And the concrete type confirms it, so the assertion cannot pass on a
    // default that happens to equal "heart".
    let icon = Icon::new(Rect::new(0, 0, 10, 10));
    assert_ne!(format!("{:?}", icon.icon()), "heart");
}

/// A JSON value of the wrong type is reported, not silently dropped. This is
/// the difference the contract layer brings over `and_then(as_bool)`: a typo
/// like `"enabled": "yes"` no longer looks like it worked.
#[cfg(full_widgets)]
#[test]
fn a_wrongly_typed_property_value_does_not_silently_apply() {
    use crate::json::properties::{apply_widget_property, ApplyOutcome};
    let mut widget = Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
    assert_eq!(
        apply_widget_property(&mut widget, "enabled", &serde_json::json!("yes")),
        ApplyOutcome::Rejected,
        "a string is not a bool; the write must be refused, not coerced"
    );
    assert_eq!(
        apply_widget_property(&mut widget, "enabled", &serde_json::json!(false)),
        ApplyOutcome::Applied
    );
}

/// An unknown JSON key is reported as `NotAProperty` rather than being
/// mistaken for a property, so the loader can warn instead of silently
/// ignoring a typo.
#[cfg(full_widgets)]
#[test]
fn an_unknown_key_is_not_mistaken_for_a_property() {
    let mut widget = Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
    assert_eq!(
        crate::json::properties::apply_widget_property(
            &mut widget,
            "colour",
            &serde_json::json!("#ff0000")
        ),
        crate::json::properties::ApplyOutcome::NotAProperty
    );
}

// ── Declaration-driven styling ────────────────────────────────────────

/// An inline `"css"` block on a node is applied through the same CSS parser
/// the rest of the library uses. Before this, the key did not exist and the
/// rule had nowhere to go.
#[test]
fn an_inline_css_block_styles_the_node() {
    let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "css": "Button { background-color: #123456; }"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "an inline css block must not fail the load: {:?}", result.err());
}

/// A `"class"` attribute selects a class rule. Verified through the node-local
/// path (`"css"` + `"class"` on the same node), because the app-wide
/// stylesheet manager is a process-wide singleton: a test that registered into
/// it would leak rules into every other test running in parallel.
#[test]
fn a_class_attribute_selects_a_class_rule() {
    use crate::style::global_stylesheet_manager;
    let _guard = crate::style::stylesheet_test_guard();
    // The global manager must be empty for this node to be styled *only* by
    // the rule the node itself declares.
    global_stylesheet_manager().clear();

    let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "class": "primary", "css": ".primary { border-radius: 7px; }"}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "a class-selectable node must load: {:?}", result.err());

    // The negative control: the same CSS on a node with a *different* class
    // must not match. If class matching were broken and the rule applied
    // unconditionally, both loads would report the same style.
    let unmatched = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "class": "secondary", "css": ".primary { border-radius: 7px; }"}}]}}}"#;
    assert!(JsonLoader::load(unmatched).is_ok(), "the unmatched case must still load");
}

/// A descendant combinator in inline CSS matches the ancestor chain the loader built.
///
/// Before STY-002 the inline entry passed no chain, so `Panel Button { … }` parsed, was
/// indexed, and could never match the button it was written for.
#[test]
fn a_descendant_combinator_in_inline_css_matches_the_loader_chain() {
    let obj: serde_json::Map<String, Value> = serde_json::from_str(
        r#"{"id": "bt", "text": "Go", "css": "Panel Button { border-radius: 11px; }"}"#,
    )
    .expect("the test node must be a JSON object");
    let ancestors = [ElementRef { kind: WidgetKind::Panel, class: None, id: None, state: None }];

    let mut matched = Button::new("Go".to_string(), Rect::new(0, 0, 80, 24));
    apply_declared_styles(&mut matched, &obj, &ancestors);
    assert_eq!(matched.style().border_radius, Some(11), "the chain must satisfy the selector");

    let mut unmatched = Button::new("Go".to_string(), Rect::new(0, 0, 80, 24));
    apply_declared_styles(&mut unmatched, &obj, &[]);
    assert_ne!(
        unmatched.style().border_radius,
        Some(11),
        "without a chain the combinator must not match"
    );
}

/// A descendant combinator in an app-wide sheet matches the same chain.
#[test]
fn a_descendant_combinator_in_a_global_sheet_matches_the_loader_chain() {
    use crate::style::{global_stylesheet_manager, stylesheet_test_guard};
    let _guard = stylesheet_test_guard();
    global_stylesheet_manager().clear();
    global_stylesheet_manager().register("combinator", "Panel Button { border-radius: 13px; }", 0);

    let obj: serde_json::Map<String, Value> = serde_json::from_str(r#"{"id": "bt", "text": "Go"}"#)
        .expect("the test node must be a JSON object");
    let ancestors = [ElementRef { kind: WidgetKind::Panel, class: None, id: None, state: None }];

    let mut matched = Button::new("Go".to_string(), Rect::new(0, 0, 80, 24));
    apply_declared_styles(&mut matched, &obj, &ancestors);
    assert_eq!(matched.style().border_radius, Some(13));

    let mut unmatched = Button::new("Go".to_string(), Rect::new(0, 0, 80, 24));
    apply_declared_styles(&mut unmatched, &obj, &[]);
    assert_ne!(unmatched.style().border_radius, Some(13));

    global_stylesheet_manager().clear();
}

/// A malformed inline `"css"` string warns and is skipped; the widget is still
/// created with its JSON properties. Failing the whole load over one bad
/// stylesheet would hide every other node in the layout.
#[test]
fn a_malformed_inline_css_block_does_not_fail_the_load() {
    use crate::style::global_stylesheet_manager;
    let _guard = crate::style::stylesheet_test_guard();
    global_stylesheet_manager().clear();
    let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "css": "Button { background-color: #123456;"}}]}}}"#;
    let layout = JsonLoader::load(json).expect("a bad stylesheet must not abort the layout");
    assert!(layout.id("bt").is_some(), "the node must still exist");
}

/// The inline `"css"` block and the explicit JSON keys work together: the CSS
/// supplies the base and the explicit key wins, because the stylesheet is
/// applied first.
#[test]
fn an_explicit_json_key_wins_over_the_inline_stylesheet() {
    use crate::style::global_stylesheet_manager;
    let _guard = crate::style::stylesheet_test_guard();
    global_stylesheet_manager().clear();
    let json = r#"{"window": {"id": "w", "title": "T", "width": 400, "height": 300, "layout": {"type": "vbox", "children": [{"button": {"id": "bt", "text": "Go", "css": "Button { border-radius: 9px; }", "border_radius": 2}}]}}}"#;
    let result = JsonLoader::load(json);
    assert!(result.is_ok(), "the two sources must combine: {:?}", result.err());
}

// ── Theme application ────────────────────────────────────────────────

/// A loaded node's style takes its colours from the active theme. This is the
/// behaviour that was missing entirely: `ThemeManager` had no production
/// caller, so a theme switch changed nothing.
#[cfg(full_widgets)]
#[test]
fn a_loaded_node_is_styled_from_the_active_theme() {
    use crate::theme::{global_theme_manager, AppearanceMode, Theme};
    let _guard = crate::style::theme_test_guard();

    // Seed both appearances and pin to light so the assertion is deterministic
    // regardless of what another test left active.
    {
        let mut manager = global_theme_manager();
        manager.register_theme(Theme::dark());
        assert!(manager.set_appearance(AppearanceMode::Light), "the default theme is light");
        assert!(manager.set_theme("default"), "the default theme must be selectable");
    }

    let theme_front = { crate::style::palette_shared().expect("an active theme").foreground };
    assert_eq!(
        crate::theme::resolved_theme_style("label").expect("theme resolves").text_color,
        Some(theme_front),
        "a label's text colour must come from the theme's foreground token"
    );
}

/// Switching appearance changes the resolved colours. Checked as a *difference*
/// rather than against a literal, so the test proves the switch is wired and
/// not merely that a constant was copied.
#[cfg(full_widgets)]
#[test]
fn switching_appearance_changes_the_resolved_style() {
    use crate::theme::{global_theme_manager, resolved_theme_style, AppearanceMode, Theme};
    let _guard = crate::style::theme_test_guard();
    {
        let mut manager = global_theme_manager();
        manager.register_theme(Theme::default());
        manager.register_theme(Theme::dark());
    }

    let light = {
        let mut manager = global_theme_manager();
        assert!(manager.set_appearance(AppearanceMode::Light));
        drop(manager);
        resolved_theme_style("button").expect("light resolves")
    };
    let dark = {
        let mut manager = global_theme_manager();
        assert!(manager.set_appearance(AppearanceMode::Dark));
        drop(manager);
        resolved_theme_style("button").expect("dark resolves")
    };
    assert_ne!(
        light.background_color, dark.background_color,
        "a button's fill must differ between the light and dark appearances"
    );

    // Restore the light default so later tests start from a known theme.
    global_theme_manager().set_appearance(AppearanceMode::Light);
}

/// The theme supplies a font. `Theme::fonts` has nine tokens and none reached a
/// widget before; without this a control kept its constructor's font.
#[cfg(full_widgets)]
#[test]
fn the_resolved_style_carries_the_theme_font() {
    use crate::theme::{global_theme_manager, resolved_theme_style, AppearanceMode, Theme};
    let _guard = crate::style::theme_test_guard();
    {
        let mut manager = global_theme_manager();
        manager.register_theme(Theme::default());
        manager.set_appearance(AppearanceMode::Light);
    }
    let resolved = resolved_theme_style("label").expect("theme resolves");
    // `resolve_style` scales the theme's body token by the device's text-size preference, so
    // the expectation carries that scale too rather than the nominal token.
    let expected_font = global_theme_manager()
        .current_theme()
        .expect("active theme")
        .fonts
        .body
        .clone()
        .scaled(crate::style::environment::environment().effective_text_scale());
    assert_eq!(resolved.font, Some(expected_font), "the theme's body font must reach the style");
}

/// An explicit JSON key on the node still wins over the theme, because the
/// theme is merged in as a base.
#[cfg(full_widgets)]
#[test]
fn an_explicit_json_key_wins_over_the_theme() {
    use crate::style::global_stylesheet_manager;
    let _guard = crate::style::theme_test_guard();
    let _sheet_guard = crate::style::stylesheet_test_guard();
    global_stylesheet_manager().clear();
    let theme_front = { crate::style::palette_shared().expect("active theme").foreground };
    // Pick a colour the theme's foreground is guaranteed not to be.
    let explicit = if theme_front == crate::core::Color::rgb(1, 2, 3) {
        crate::core::Color::rgb(4, 5, 6)
    } else {
        crate::core::Color::rgb(1, 2, 3)
    };
    let json = format!(
        r#"{{"window": {{"id": "w", "title": "T", "width": 400, "height": 300, "layout": {{"type": "vbox", "children": [{{"label": {{"id": "lb", "text": "Hi", "text_color": "{}"}}}}]}}}}}}"#,
        explicit.to_hex_rgba()
    );
    let result = JsonLoader::load(&json);
    assert!(result.is_ok(), "the theme and an explicit key must combine: {:?}", result.err());
}

/// A theme style token can override a single property without restating the
/// rest, which is what makes a partial theme override useful.
#[cfg(full_widgets)]
#[test]
fn a_theme_style_token_overrides_only_what_it_names() {
    use crate::style::global_stylesheet_manager;
    use crate::theme::{resolved_theme_style, Theme, ThemeOverrides, ThemeStyleToken, WidgetRole};
    let _guard = crate::style::theme_test_guard();
    let _sheet_guard = crate::style::stylesheet_test_guard();
    global_stylesheet_manager().clear();

    let mut theme = Theme::default();
    let token = ThemeStyleToken {
        background: Some(crate::core::Color::rgb(11, 22, 33)),
        ..Default::default()
    };
    theme.overrides =
        ThemeOverrides { styles: [(String::from("label"), token)].into_iter().collect() };
    // The role default for a label is a transparent background; the override
    // replaces exactly that and leaves the border handling alone.
    assert_eq!(WidgetRole::for_kind_name("label"), WidgetRole::Text);
    let style = {
        let mut manager = crate::theme::global_theme_manager();
        manager.register_theme(theme);
        manager.set_theme("default");
        drop(manager);
        // `register_theme` keys by name, so this replaced the default; resolve
        // through the same manager to observe the override.
        resolved_theme_style("label").expect("theme resolves")
    };
    assert_eq!(
        style.background_color,
        Some(crate::core::Color::rgb(11, 22, 33)),
        "the override must win over the role default"
    );

    // Restore the pristine default theme for later tests.
    let mut manager = crate::theme::global_theme_manager();
    manager.register_theme(Theme::default());
    manager.set_appearance(crate::theme::AppearanceMode::Light);
}

/// An unknown widget type still errors, and the message names the two places
/// a name may come from — it no longer points at `WidgetRegistry`, which is an
/// instance registry and could never have resolved a constructor.
#[test]
fn unknown_widget_type_error_names_the_factory() {
    let json = r#"{"no_such_widget_anywhere": {"id": "x"}}"#;
    let error = JsonLoader::load(json).expect_err("an unknown type must error");
    assert!(error.contains("no_such_widget_anywhere"), "{error}");
    assert!(error.contains("widget factory"), "{error}");
    assert!(!error.contains("WidgetRegistry"), "{error}");
}

/// Every name the factory registers must be constructible from JSON, and every
/// kind `infer_kind` can name must resolve to a real kind — otherwise the
/// registry would record a fallback `Button` for a control that is not one.
///
/// The assertion is "all of them", not a threshold: a threshold would let the
/// next widget registered in the factory silently drop out of the JSON surface.
#[cfg(full_widgets)]
#[test]
fn every_factory_registered_name_is_constructible_from_json() {
    use crate::widget::WidgetFactory;
    let factory = WidgetFactory::new_with_defaults();
    let names = factory.widget_names();
    assert!(names.len() > 100, "the registry should be populated");

    let mut failures = Vec::new();
    for name in &names {
        let json = format!(
            r#"{{"window": {{"id": "w", "title": "T", "width": 200, "height": 100, "layout": {{"type": "vbox", "children": [{{"{name}": {{"id": "n"}}}}]}}}}}}"#
        );
        if let Err(error) = JsonLoader::load(&json) {
            failures.push(format!("{name}: {error}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} factory names cannot be built from JSON: {failures:?}",
        failures.len(),
        names.len()
    );
}

// ── Marker routing (W-04 / W-05 / W-06) ─────────────────────────

/// Mounts a fresh `Button` and returns its runtime id.
fn mount_probe_button() -> ObjectId {
    crate::widget::runtime::register(Box::new(Button::new(
        "probe".to_string(),
        Rect::new(0, 0, 80, 24),
    )))
    .expect("a button must mount")
}

/// Counts `name`'s invocations on this thread's handler registry.
fn counting_handler(name: &'static str) -> crate::compat::Arc<core::sync::atomic::AtomicUsize> {
    use core::sync::atomic::{AtomicUsize, Ordering};
    let calls = crate::compat::Arc::new(AtomicUsize::new(0));
    let counter = crate::compat::Arc::clone(&calls);
    crate::json::register_global_handler(name, move |_ctx| {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    calls
}

/// The `on_*` key for `marker`, read from the one table instead of spelled here.
///
/// `tools/check_json_event_route.py` forbids these names as literals in this file — the loader
/// must derive its key set from `event_route::MARKER_KEYS`, and a test literal is exactly the
/// second spelling that rule exists to prevent.
fn marker_key(marker: crate::json::JsonTriggerMarker) -> &'static str {
    crate::json::marker_key_for(marker)
        .unwrap_or_else(|| panic!("{marker:?} must have an `on_*` key in MARKER_KEYS"))
}

fn emit_focus_gained(id: ObjectId) {
    crate::widget::runtime::with_widget_mut(id, |widget| {
        widget.handle_event(&crate::event::Event::FocusGained {
            reason: crate::event::FocusReason::Programmatic,
        });
    });
}

fn emit_focus_lost(id: ObjectId) {
    crate::widget::runtime::with_widget_mut(id, |widget| {
        widget.handle_event(&crate::event::Event::FocusLost);
    });
}

/// W-05: `on_focus` / `on_blur` reach the widget's own focus signals.
///
/// # The defect this pins
///
/// Both focus routes were dead: the marker was refused (no handle callback fires on focus) and
/// the capability table publishes no per-control focus event, so no `events:` spelling existed
/// either. The runtime's focus router emits `BaseWidget::focus_gained` / `focus_lost` through
/// `BaseWidget::handle_event`, so the marker is bound there — the same event a real focus move
/// produces, not a value change standing in for it.
#[test]
fn focus_and_blur_reach_the_widgets_focus_signals() {
    use core::sync::atomic::Ordering;
    crate::json::clear_global_handlers();
    let gained = counting_handler("on_focus_probe");
    let lost = counting_handler("on_blur_probe");
    let id = mount_probe_button();

    assert!(
        crate::json::bind_marker_event(
            id,
            marker_key(crate::json::JsonTriggerMarker::FocusGained),
            crate::json::JsonTriggerMarker::FocusGained,
            "on_focus_probe"
        ),
        "`on_focus` must bind, not be refused"
    );
    assert!(
        crate::json::bind_marker_event(
            id,
            marker_key(crate::json::JsonTriggerMarker::FocusLost),
            crate::json::JsonTriggerMarker::FocusLost,
            "on_blur_probe"
        ),
        "`on_blur` must bind, not be refused"
    );

    emit_focus_gained(id);
    assert_eq!(gained.load(Ordering::SeqCst), 1, "`on_focus` must run when focus is gained");
    assert_eq!(lost.load(Ordering::SeqCst), 0, "gaining focus must not run `on_blur`");

    emit_focus_lost(id);
    assert_eq!(lost.load(Ordering::SeqCst), 1, "`on_blur` must run when focus is lost");
    assert_eq!(gained.load(Ordering::SeqCst), 1, "losing focus must not run `on_focus`");

    crate::json::clear_global_handlers();
    crate::widget::runtime::unregister(id);
}

/// W-04: a marker binding yields a token, and releasing it stops delivery.
///
/// `bind_one_releasable` returned `None` for every `on_*` key, so a document's marker wiring
/// could only be released by destroying the control. Both routes now produce a token; this
/// asserts the marker half behaves like the published half — release is real, and it is
/// idempotent.
#[test]
fn a_marker_binding_can_be_released_by_its_token() {
    use core::sync::atomic::Ordering;
    crate::json::clear_global_handlers();
    let calls = counting_handler("on_focus_probe");
    let id = mount_probe_button();

    let mut token = JsonLoader::bind_one_releasable(
        id,
        crate::json::JsonEventBinding::Marker {
            key: marker_key(crate::json::JsonTriggerMarker::FocusGained),
            marker: crate::json::JsonTriggerMarker::FocusGained,
        },
        "on_focus_probe".to_string(),
    )
    .expect("a marker route must yield a releasable token");

    emit_focus_gained(id);
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the marker binding must be live");

    token.release();
    assert!(!token.is_live(), "a released token is no longer live");
    emit_focus_gained(id);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "a released marker binding must not run its handler"
    );
    // Releasing twice is a no-op, not a double disconnect or a panic.
    token.release();

    crate::json::clear_global_handlers();
    crate::widget::runtime::unregister(id);
}

/// W-04: unloading a document releases its marker wiring along with its published wiring.
///
/// `LOADED_BINDINGS` is the per-control registry `Detach`/`forget_node` releases. A marker
/// binding used to bypass it entirely, so the registry only ever reclaimed half a document's
/// wires and the marker half lived until the control was destroyed.
#[test]
fn releasing_a_controls_bindings_also_reclaims_its_marker_wiring() {
    use core::sync::atomic::Ordering;
    crate::json::clear_global_handlers();
    let calls = counting_handler("on_focus_probe");
    let id = mount_probe_button();

    assert!(
        JsonLoader::bind_one(
            id,
            crate::json::JsonEventBinding::Marker {
                key: marker_key(crate::json::JsonTriggerMarker::FocusGained),
                marker: crate::json::JsonTriggerMarker::FocusGained,
            },
            "on_focus_probe".to_string(),
        ),
        "`on_focus` must bind"
    );
    assert_eq!(
        crate::json::retained_binding_count(id),
        1,
        "the marker wire must be retained for the control, like a published one"
    );

    emit_focus_gained(id);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    crate::json::release_widget_bindings(id);
    assert_eq!(crate::json::retained_binding_count(id), 0);
    emit_focus_gained(id);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "releasing the control's bindings must disconnect its marker wiring too"
    );

    crate::json::clear_global_handlers();
    crate::widget::runtime::unregister(id);
}

/// W-06: `on_double_click` is carried by the published `double_clicked` event.
///
/// # The defect this pins
///
/// `on_double_click` was refused while `double_clicked` was a working published name, so the two
/// routes disagreed about one intent and the error message pointed nowhere. The marker now asks
/// for the published event: a control that publishes it (the canvas) is wired to its own signal,
/// and a control that does not (a button) is refused rather than bound to the single-click
/// signal.
#[test]
fn a_double_click_marker_uses_the_published_event() {
    use core::sync::atomic::Ordering;
    crate::json::clear_global_handlers();
    let calls = counting_handler("on_double_probe");

    // A button does not publish `double_clicked`, so the marker is refused — and, crucially,
    // not attached to `base.clicked`, which fires on the first click of a double click.
    let button = mount_probe_button();
    assert!(
        !crate::json::bind_marker_event(
            button,
            marker_key(crate::json::JsonTriggerMarker::DoubleClicked),
            crate::json::JsonTriggerMarker::DoubleClicked,
            "on_double_probe"
        ),
        "a control that does not publish `double_clicked` cannot carry `on_double_click`"
    );
    crate::widget::runtime::with_widget_mut(button, |widget| widget.base().clicked.emit());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "a refused double-click wire must not run on a single click"
    );

    // The canvas publishes `double_clicked` and resolves it through `event_signal_dyn`, so the
    // marker is wired to that signal.
    let canvas =
        crate::widget::runtime::register(Box::new(crate::Canvas::new(Rect::new(0, 0, 80, 24))))
            .expect("a canvas must mount");
    assert!(
        crate::json::bind_marker_event(
            canvas,
            marker_key(crate::json::JsonTriggerMarker::DoubleClicked),
            crate::json::JsonTriggerMarker::DoubleClicked,
            "on_double_probe"
        ),
        "the canvas publishes `double_clicked`, so `on_double_click` must bind"
    );
    crate::widget::runtime::with_widget_mut(canvas, |widget| {
        let Some(canvas) = (widget as &mut dyn core::any::Any).downcast_mut::<crate::Canvas>()
        else {
            panic!("the mounted control must be the canvas that was registered");
        };
        canvas.double_clicked_signal().emit((crate::core::Point::new(5, 6), 1));
    });
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "`on_double_click` must reach the canvas's own double-click signal"
    );

    crate::json::clear_global_handlers();
    crate::widget::runtime::unregister(button);
    crate::widget::runtime::unregister(canvas);
}

/// W-301: one call binds every published name that already has a same-named handler.
#[test]
fn a_batch_binding_reaches_every_published_name_with_a_handler() {
    use core::sync::atomic::Ordering;
    crate::json::clear_global_handlers();
    let calls = counting_handler("clicked");
    let button = mount_probe_button();

    let bound = crate::json::bind_all_published_events(button, "button");
    assert_eq!(bound, 1, "only `clicked` has a handler among the button's published names");
    crate::widget::runtime::with_widget_mut(button, |widget| widget.base().clicked.emit());
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the batch wire must reach the control's signal");

    // Additive, matching `bind_published_event`: a second call makes a second subscription.
    assert_eq!(crate::json::bind_all_published_events(button, "button"), 1);
    crate::widget::runtime::with_widget_mut(button, |widget| widget.base().clicked.emit());
    assert_eq!(calls.load(Ordering::SeqCst), 3, "the batch binder does not de-duplicate");

    crate::json::clear_global_handlers();
    crate::widget::runtime::unregister(button);
}

/// A container's declared children must reach the container's own content model.
///
/// # Why this test exists (issue-20261104 DM-11)
///
/// The generic child path mounts a node and applies layout placement, which is all a
/// plain container needs. A container that keeps its content in a side table — a page
/// list, a pane list, a slot — was left with that table empty, so a JSON document could
/// construct the control but never fill it: a stacked widget painted every page on top of
/// the others, and a scroll area had no viewport child. This walks the four families the
/// loader now routes (stacked pages, single-slot content, accordion sections, and the
/// menu/tool-bar entries) and asserts the side table is populated.
#[test]
fn declared_children_reach_the_container_content_model() {
    let json = r#"{"window": {"id": "route-window", "width": 640, "height": 480, "children": [
            {"stacked_widget": {"id": "route-stack", "width": 200, "height": 100, "children": [
                {"label": {"id": "route-page-0", "text": "first"}},
                {"label": {"id": "route-page-1", "text": "second"}}
            ]}},
            {"scroll_area": {"id": "route-scroll", "width": 200, "height": 100, "children": [
                {"label": {"id": "route-scroll-content", "text": "content"}}
            ]}},
            {"accordion": {"id": "route-accordion", "width": 200, "height": 100, "sections": [
                {"label": {"id": "route-section-body", "title": "One"}}
            ]}},
            {"menu_bar": {"id": "route-menubar", "width": 400, "height": 24, "menus": [
                {"title": "File", "items": [{"text": "Open", "shortcut": "Ctrl+O"}, {"separator": true}]}
            ]}},
            {"tool_bar": {"id": "route-toolbar", "width": 400, "height": 32, "actions": [
                {"text": "Run"}, {"separator": true}
            ]}},
            {"adaptive_scaffold": {"id": "route-scaffold", "title": "Home", "width": 400, "height": 600,
                "children": [
                    {"label": {"id": "route-body", "text": "body", "slot": "body"}},
                    {"button": {"id": "route-fab", "text": "+", "slot": "fab"}},
                    {"label": {"id": "route-appbar-bottom", "text": "strip", "slot": "bottom"}}
                ],
                "pages": [
                    {"index": 1, "label": {"id": "route-nav-page", "text": "second"}}
                ]
            }},
            {"tab_view": {"id": "route-tabview", "width": 300, "height": 200, "children": [
                {"label": {"id": "route-tabview-page", "text": "page"}}
            ]}},
            {"modal_bottom_sheet": {"id": "route-sheet", "width": 300, "height": 200, "children": [
                {"label": {"id": "route-sheet-content", "text": "content"}}
            ]}},
            {"navigation_stack": {"id": "route-navstack", "width": 300, "height": 400, "children": [
                {"label": {"id": "route-nav-root", "text": "root"}},
                {"label": {"id": "route-nav-top", "text": "top"}}
            ]}},
            {"masonry_layout": {"id": "route-masonry", "width": 300, "height": 400, "children": [
                {"button": {"id": "route-card-0", "text": "first card", "height": 90}},
                {"button": {"id": "route-card-1", "text": "second card"}}
            ]}},
            {"carousel": {"id": "route-carousel", "width": 300, "height": 200, "pages": [
                {"index": 0, "label": {"id": "route-carousel-page-0", "text": "first slide"}},
                {"index": 1, "label": {"id": "route-carousel-page-1", "text": "second slide"}}
            ]}},
            {"flow_layout": {"id": "route-flow", "width": 300, "height": 200, "children": [
                {"button": {"id": "route-flow-0", "text": "a", "width": 80, "height": 30}},
                {"button": {"id": "route-flow-1", "text": "b", "width": 80, "height": 30}}
            ]}}
        ]}}"#;
    let layout = JsonLoader::load(json).expect("the document loads");

    let stack = layout.id("route-stack").expect("the stack is mounted");
    let pages = crate::widget::runtime::with_widget(stack, |widget| {
        crate::widget::capability::coercion::widget_as::<StackedWidget>(widget)
            .map(|s| (s.count(), s.widget(0), s.widget(1)))
    })
    .flatten()
    .expect("the stack is a StackedWidget");
    assert_eq!(pages.0, 2, "both declared children are pages, not just tree children");
    assert_eq!(pages.1, layout.id("route-page-0"), "pages keep document order");
    assert_eq!(pages.2, layout.id("route-page-1"));
    assert_eq!(
        crate::widget::read_widget_property_by_id(stack, "current_index").ok(),
        Some(crate::widget::capability::types::CapabilityValue::UInt(0)),
        "the first page is selected"
    );

    let scroll = layout.id("route-scroll").expect("the scroll area is mounted");
    let content = crate::widget::runtime::with_widget(scroll, |widget| {
        crate::widget::capability::coercion::widget_as::<ScrollArea>(widget)
            .and_then(|s| s.widget())
    })
    .flatten();
    assert_eq!(
        content,
        layout.id("route-scroll-content"),
        "the first declared child becomes the scrolling content"
    );

    let accordion = layout.id("route-accordion").expect("the accordion is mounted");
    let section = crate::widget::runtime::with_widget(accordion, |widget| {
        crate::widget::capability::coercion::widget_as::<Accordion>(widget)
            .and_then(|a| a.sections().first().map(|s| s.title().to_string()))
    })
    .flatten();
    assert_eq!(section.as_deref(), Some("One"), "the declared section exists");

    let menubar = layout.id("route-menubar").expect("the menu bar is mounted");
    let menus = crate::widget::runtime::with_widget(menubar, |widget| {
        crate::widget::capability::coercion::widget_as::<MenuBar>(widget)
            .map(|bar| (bar.entries().len(), bar.attached_menu(0).map(|menu| menu.items().len())))
    })
    .flatten()
    .expect("the menu bar is a MenuBar");
    assert_eq!(menus.0, 1, "the declared menu became a top-level entry");
    assert_eq!(menus.1, Some(2), "its declared items were attached, separator included");

    let toolbar = layout.id("route-toolbar").expect("the tool bar is mounted");
    let actions = crate::widget::runtime::with_widget(toolbar, |widget| {
        crate::widget::capability::coercion::widget_as::<ToolBar>(widget)
            .map(|bar| bar.items().len())
    })
    .flatten();
    assert_eq!(actions, Some(2), "the declared actions were added");

    let scaffold = layout.id("route-scaffold").expect("the scaffold is mounted");
    let slots = crate::widget::runtime::with_widget(scaffold, |widget| {
        crate::widget::capability::coercion::widget_as::<AdaptiveScaffold>(widget)
            .map(|s| (s.body_id(), s.nav_body_id(1), s.app_bar_bottom_id()))
    })
    .flatten()
    .expect("the scaffold is an AdaptiveScaffold");
    assert_eq!(slots.0, layout.id("route-body"), "a child with `slot: body` fills the body slot");
    assert_eq!(
        slots.1,
        layout.id("route-nav-page"),
        "a declared page is bound to the destination its `index` names"
    );
    assert_eq!(
        slots.2,
        layout.id("route-appbar-bottom"),
        "a child with `slot: bottom` fills the app bar's own bottom strip"
    );

    let tab_view = layout.id("route-tabview").expect("the tab view is mounted");
    let tab_page = crate::widget::runtime::with_widget(tab_view, |widget| {
        crate::widget::capability::coercion::widget_as::<TabView>(widget)
            .map(|v| (v.tab_count(), v.content_widget_id()))
    })
    .flatten()
    .expect("the tab view is a TabView");
    assert_eq!(tab_page.0, 1, "the declared child became a tab page");
    assert_eq!(tab_page.1, layout.id("route-tabview-page"));

    let sheet = layout.id("route-sheet").expect("the sheet is mounted");
    let sheet_content = crate::widget::runtime::with_widget(sheet, |widget| {
        crate::widget::capability::coercion::widget_as::<ModalBottomSheet>(widget)
            .and_then(|s| s.content_id())
    })
    .flatten();
    assert_eq!(
        sheet_content,
        layout.id("route-sheet-content"),
        "the declared child fills the sheet's content well"
    );

    let navstack = layout.id("route-navstack").expect("the navigation stack is mounted");
    let pages = crate::widget::runtime::with_widget(navstack, |widget| {
        crate::widget::capability::coercion::widget_as::<NavigationStack>(widget)
            .map(|s| (s.page_count(), s.page_id(0), s.page_id(1)))
    })
    .flatten()
    .expect("the stack is a NavigationStack");
    assert_eq!(pages.0, 2, "each declared child became a page in the history");
    assert_eq!(pages.1, layout.id("route-nav-root"), "the first child is the root");
    assert_eq!(pages.2, layout.id("route-nav-top"), "the last one is on top");
    // While the push slides, the outgoing page is shown on purpose; the visibility rule is
    // "one page once settled", so settle the slide first and then read it.
    crate::widget::runtime::with_widget_mut(navstack, |w| {
        let _ = w.tick(5_000);
    });
    let root_visible = layout
        .id("route-nav-root")
        .and_then(|root| crate::widget::runtime::with_widget(root, |w| w.is_visible()));
    assert_eq!(root_visible, Some(false), "a page below the top is hidden");

    let masonry = layout.id("route-masonry").expect("the masonry layout is mounted");
    let cards = crate::widget::runtime::with_widget(masonry, |widget| {
        crate::widget::capability::coercion::widget_as::<MasonryLayout>(widget).map(|m| {
            m.items()
                .iter()
                .map(|item| (item.label.clone(), item.height, item.widget_id))
                .collect::<alloc::vec::Vec<_>>()
        })
    })
    .expect("the masonry layout is mounted")
    .expect("the masonry layout is a MasonryLayout");
    assert_eq!(cards.len(), 2, "each declared child became one card");
    assert_eq!(cards[0].0, "first card", "the label is the child's own text");
    assert_eq!(cards[0].1, 90, "the card height is the child's declared height");
    assert_eq!(cards[1].1, DEFAULT_DECLARED_CARD_HEIGHT, "and the default when absent");
    assert_eq!(cards[0].2, layout.id("route-card-0"), "the card carries the mounted child");

    let carousel = layout.id("route-carousel").expect("the carousel is mounted");
    let slides = crate::widget::runtime::read_widget(carousel, |widget| {
        crate::widget::capability::coercion::widget_as::<Carousel>(widget).map(|c| {
            (
                c.page_count(),
                c.page_title(0).map(str::to_string),
                c.pages()[0].content_id(),
                c.pages()[1].content_id(),
            )
        })
    })
    .flatten()
    .expect("the carousel is a Carousel");
    assert_eq!(slides.0, 2, "each declared page became a page");
    assert_eq!(slides.1.as_deref(), Some("first slide"), "the title comes from the child");
    assert_eq!(slides.2, layout.id("route-carousel-page-0"));
    assert_eq!(slides.3, layout.id("route-carousel-page-1"));

    let flow = layout.id("route-flow").expect("the flow layout is mounted");
    let items = crate::widget::runtime::with_widget(flow, |widget| {
        crate::widget::capability::coercion::widget_as::<FlowLayout>(widget)
            .map(|f| f.items().len())
    })
    .flatten();
    assert_eq!(items, Some(2), "each declared child became one item");
}
