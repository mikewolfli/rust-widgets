//! Evidence for BLUE23 §93: the M3 animation family.
//!
//! Each control here gained a `PropertyDriver`, and each claim below is on the **painted**
//! picture rather than on the driver — because this crate has already shipped an animation whose
//! model moved while the pixels teleported (`segmented_control`'s `let t = 1.0`), and a test that
//! samples the model passes with that defect present.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test m3_animation_probe -- --nocapture

use rust_widgets::core::{Color, Rect};
use rust_widgets::theme::AppearanceMode;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::svg::render_widget_to_svg_on;
use rust_widgets::widget::WidgetFactory;

use rust_widgets::widget::Widget;

fn render(widget: &mut dyn Widget, geometry: Rect) -> String {
    let drawable = draw_of(widget).expect("draw bridge");
    render_widget_to_svg_on(drawable, geometry, Color::BLACK)
}

/// Opens a control, samples the picture at three points, and asserts it moved.
///
/// The three frames are the shape §0.3 asks for: the middle sample must differ from both ends,
/// which a control that paints its final geometry immediately cannot satisfy.
fn assert_reveals_in_frames(
    label: &str,
    mut widget: Box<dyn Widget>,
    geometry: Rect,
    open: impl Fn(&mut dyn Widget),
) {
    let _guard = rust_widgets::theme::theme_test_guard();
    let closed = render(widget.as_mut(), geometry);

    open(widget.as_mut());
    assert!(widget.is_animating(), "{label}: opening must owe frames");

    // One millisecond in: the first frame of the movement.
    let _ = widget.tick(1);
    let first = render(widget.as_mut(), geometry);

    // Part-way.
    let _ = widget.tick(60);
    let middle = render(widget.as_mut(), geometry);

    while widget.is_animating() {
        let _ = widget.tick(16);
    }
    let settled = render(widget.as_mut(), geometry);

    println!(
        "{label}: closed={} first={} middle={} settled={}",
        closed.len(),
        first.len(),
        middle.len(),
        settled.len()
    );

    assert_ne!(closed, settled, "{label}: an open popup must differ from a closed one");
    assert_ne!(
        first, middle,
        "{label}: the panel must be painted differently 1 ms and 60 ms into the open; identical \
         pictures mean the reveal never reached the draw path"
    );
    assert!(!widget.is_animating(), "{label}: a settled control owes no more frames");
}

#[test]
fn probe_the_menu_family_reveals_in_frames() {
    rust_widgets::theme::global_theme_manager().set_appearance(AppearanceMode::Dark);
    let geometry = Rect::new(0, 0, 160, 28);
    let factory = WidgetFactory::new_with_defaults();

    {
        // `dropdown_menu` ships an **empty** list, so a probe that did not fill it would measure a
        // control whose list never draws — which reads as "the reveal is broken".
        let mut widget = factory.create("dropdown_menu", geometry, "x").expect("dropdown_menu");
        {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::menu_toolbar::dropdown_menu::{DropdownItem, DropdownMenu};
            let menu = widget_as_mut::<DropdownMenu>(widget.as_mut()).expect("dropdown menu");
            for name in ["Alpha", "Beta", "Gamma", "Delta"] {
                menu.add_item(DropdownItem::new(name, name));
            }
        }
        assert_reveals_in_frames("dropdown_menu", widget, geometry, |w| {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::menu_toolbar::dropdown_menu::DropdownMenu;
            widget_as_mut::<DropdownMenu>(w).expect("dropdown menu").expand();
        });
    }

    {
        let mut widget = factory.create("menu_button", geometry, "x").expect("menu_button");
        {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::menu_toolbar::menu_button::{MenuButton, MenuItem};
            let button = widget_as_mut::<MenuButton>(widget.as_mut()).expect("menu button");
            for (index, name) in ["Alpha", "Beta", "Gamma"].iter().enumerate() {
                button.add_item(MenuItem::new(index as u64, name));
            }
        }
        assert_reveals_in_frames("menu_button", widget, geometry, |w| {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::menu_toolbar::menu_button::MenuButton;
            widget_as_mut::<MenuButton>(w).expect("menu button").open_menu();
        });
    }

    {
        let mut widget = factory.create("menu", geometry, "x").expect("menu");
        {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::menu_toolbar::menu::{Menu, MenuEntry};
            let menu = widget_as_mut::<Menu>(widget.as_mut()).expect("menu");
            for name in ["Alpha", "Beta", "Gamma"] {
                menu.add_item(MenuEntry::new(name));
            }
        }
        assert_reveals_in_frames("menu", widget, geometry, |w| {
            w.show();
        });
    }
}

#[test]
fn probe_the_toast_family_and_map_animate_in_frames() {
    rust_widgets::theme::global_theme_manager().set_appearance(AppearanceMode::Dark);
    let geometry = Rect::new(0, 0, 320, 200);
    let factory = WidgetFactory::new_with_defaults();

    {
        // A snackbar rises from its own bottom edge.
        //
        // Sampled at **16 ms** intervals rather than 1 ms then 60 ms: this control animates on
        // `MotionSlot::Fast` (100 ms), so a 60 ms second step lands at the very end of the
        // transition and the "middle" frame is the settled one. The first version of this probe
        // did exactly that and read as "the reveal never reached the draw path" — a measurement
        // artefact, not a defect.
        let mut widget = factory.create("snackbar", geometry, "Saved").expect("snackbar");
        let hidden = render(widget.as_mut(), geometry);
        {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::special_widgets::Snackbar;
            widget_as_mut::<Snackbar>(widget.as_mut()).expect("snackbar").show("Saved");
        }
        assert!(widget.is_animating(), "showing a snackbar must owe frames");
        let _ = widget.tick(16);
        let first = render(widget.as_mut(), geometry);
        let _ = widget.tick(16);
        let middle = render(widget.as_mut(), geometry);
        while widget.is_animating() {
            let _ = widget.tick(16);
        }
        let settled = render(widget.as_mut(), geometry);

        // The revealed height is read from the clip the control pushes, which is the geometry the
        // reveal decides — asserting on it names the quantity under test rather than on a byte count.
        fn clip_height(svg: &str) -> u32 {
            svg.lines()
                .find(|line| line.contains("clipPath"))
                .and_then(|line| line.split("height=\"").nth(1))
                .and_then(|rest| rest.split('"').next())
                .and_then(|value| value.parse().ok())
                .expect("a clipPath height")
        }

        println!(
            "snackbar: hidden={} first={} middle={} settled={}",
            clip_height(&hidden),
            clip_height(&first),
            clip_height(&middle),
            clip_height(&settled)
        );
        assert!(
            clip_height(&first) < clip_height(&settled),
            "the bar must be revealed shorter on its first frame ({}) than when settled ({}); \
             equal heights mean the reveal never reached the draw path",
            clip_height(&first),
            clip_height(&settled)
        );
        assert!(
            clip_height(&middle) > clip_height(&first),
            "and it must keep growing between frames ({} -> {})",
            clip_height(&first),
            clip_height(&middle)
        );
        assert_ne!(hidden, settled, "a shown bar must differ from a dismissed one");
    }

    {
        // A toast rises into its seat.
        let mut widget = factory.create("toast_stack", geometry, "x").expect("toast_stack");
        {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::special_widgets::{ToastItem, ToastLevel, ToastStack};
            let stack = widget_as_mut::<ToastStack>(widget.as_mut()).expect("stack");
            stack.push(ToastItem::new("one", "first", ToastLevel::Info, 3000));
            stack.push(ToastItem::new("two", "second", ToastLevel::Success, 3000));
        }
        assert!(widget.is_animating(), "a push must owe frames");
        let _ = widget.tick(16);
        let first = render(widget.as_mut(), geometry);
        let _ = widget.tick(16);
        let middle = render(widget.as_mut(), geometry);
        // The offset is read out of the drawn row's y, because the byte *length* of two frames can
        // coincide while their content differs — printing lengths alone reported "4159 / 4159" for
        // frames that were in fact different, which reads as "nothing moved".
        fn newest_row_y(svg: &str) -> Option<i32> {
            svg.lines()
                .filter(|line| line.contains("<rect"))
                .filter_map(|line| {
                    let y = line.split("y=\"").nth(1)?.split('"').next()?.parse::<i32>().ok()?;
                    let w =
                        line.split("width=\"").nth(1)?.split('"').next()?.parse::<i32>().ok()?;
                    // The toast rows are the inset ones; the band spans the control.
                    if w < 900 {
                        Some(y)
                    } else {
                        None
                    }
                })
                .max()
        }
        println!(
            "toast_stack: first_y={:?} middle_y={:?}",
            newest_row_y(&first),
            newest_row_y(&middle)
        );
        assert_ne!(
            newest_row_y(&first),
            newest_row_y(&middle),
            "the newest toast must be mid-rise: the row is offset by how far the reveal has run, \
             so its y cannot be the same on two frames of the rise"
        );
    }

    {
        // A map's grid follows the painted zoom.
        //
        // # Why the pane is 900 px wide
        //
        // The grid step is `40 * zoom`, and the lines are placed from the pane's own origin, so at a
        // 320 px pane a step of 40 and a step of 80 put lines at the *same* x positions (0, 80,
        // 160, 240) and the two frames are identical even though the reveal is working. The first
        // version of this probe used the 320 px census geometry and reported a defect that was its
        // own measurement — the control was correct. A wider pane makes the step audible in the
        // drawing, which is what the assertion needs.
        let geometry = Rect::new(0, 0, 900, 200);
        let mut widget = factory.create("map_view", geometry, "x").expect("map_view");
        {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::special_widgets::MapView;
            widget_as_mut::<MapView>(widget.as_mut()).expect("map").set_zoom(4.0);
        }
        assert!(widget.is_animating(), "changing zoom must owe frames");
        let _ = widget.tick(16);
        let first = render(widget.as_mut(), geometry);
        let _ = widget.tick(16);
        let middle = render(widget.as_mut(), geometry);
        while widget.is_animating() {
            let _ = widget.tick(16);
        }
        let settled = render(widget.as_mut(), geometry);
        println!(
            "map_view: first={} middle={} settled={}",
            first.len(),
            middle.len(),
            settled.len()
        );
        assert_ne!(
            first, middle,
            "the painted projection must differ between frames of a zoom; identical pictures \
             mean the stored target is being painted directly"
        );
        assert_ne!(first, settled, "and it must arrive somewhere different from where it started");
    }
}

#[test]
fn probe_the_navigation_controls_move_in_frames() {
    rust_widgets::theme::global_theme_manager().set_appearance(AppearanceMode::Dark);
    let factory = WidgetFactory::new_with_defaults();

    {
        // A bar's pill must slide between tab centres, so the picture differs while it travels.
        let geometry = Rect::new(0, 0, 320, 56);
        let mut widget = factory.create("bottom_navigation_bar", geometry, "x").expect("bar");
        {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::nav_widgets::bottom_navigation_bar::BottomNavigationBar;
            let bar = widget_as_mut::<BottomNavigationBar>(widget.as_mut()).expect("bar");
            for name in ["Home", "Search", "Files", "More"] {
                bar.add_item("★", name);
            }
        }
        let resting = render(widget.as_mut(), geometry);
        {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::nav_widgets::bottom_navigation_bar::BottomNavigationBar;
            widget_as_mut::<BottomNavigationBar>(widget.as_mut())
                .expect("bar")
                .set_selected_index(3);
        }
        assert!(widget.is_animating(), "changing tab must owe frames");
        let _ = widget.tick(1);
        let first = render(widget.as_mut(), geometry);
        let _ = widget.tick(60);
        let middle = render(widget.as_mut(), geometry);
        while widget.is_animating() {
            let _ = widget.tick(16);
        }
        let settled = render(widget.as_mut(), geometry);

        println!(
            "bottom_navigation_bar: rest={} first={} middle={} settled={}",
            resting.len(),
            first.len(),
            middle.len(),
            settled.len()
        );
        assert_ne!(
            first, middle,
            "the pill must be painted in a different place 1 ms and 60 ms into the slide; \
             identical pictures mean it teleported"
        );
        assert_ne!(resting, settled, "the pill must end on the tab that was chosen");
        assert!(settled.contains("rgba(0,0,0"), "the picture is still a drawing");
    }

    {
        // A push must move the content region; a pop must move it the other way.
        let geometry = Rect::new(0, 0, 200, 120);
        let mut widget = factory.create("navigation_stack", geometry, "x").expect("stack");
        {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::nav_widgets::navigation_stack::NavigationStack;
            let stack = widget_as_mut::<NavigationStack>(widget.as_mut()).expect("stack");
            stack.push(Box::new(rust_widgets::widget::base_widgets::label::Label::new(
                String::from("page 2"),
                Rect::new(0, 0, 100, 20),
            )));
        }
        assert!(widget.is_animating(), "a push must owe frames");
        let _ = widget.tick(1);
        let push_first = render(widget.as_mut(), geometry);
        let _ = widget.tick(60);
        let push_middle = render(widget.as_mut(), geometry);
        while widget.is_animating() {
            let _ = widget.tick(16);
        }
        let push_settled = render(widget.as_mut(), geometry);

        println!(
            "navigation_stack push: first={} middle={} settled={}",
            push_first.len(),
            push_middle.len(),
            push_settled.len()
        );
        assert_ne!(
            push_first, push_middle,
            "the content region must be offset differently 1 ms and 60 ms into a push"
        );
    }

    {
        // The Cupertino bar's large title collapses: the **painted bar height** must take
        // intermediate values, because iOS's signature interaction is a continuous resize.
        // Asserting on `is_animating` would pass with the old code, which flipped a bool.
        let geometry = Rect::new(0, 0, 375, 120);
        let mut widget = factory.create("cupertino_navigation_bar", geometry, "x").expect("bar");
        {
            use rust_widgets::widget::capability::coercion::widget_as_mut;
            use rust_widgets::widget::cupertino::nav_bar::CupertinoNavigationBar;
            let bar = widget_as_mut::<CupertinoNavigationBar>(widget.as_mut()).expect("bar");
            bar.set_title("Settings");
            bar.set_large_title(false);
        }
        assert!(widget.is_animating(), "collapsing must owe frames");

        // One millisecond in, and part-way.
        let _ = widget.tick(1);
        let first = render(widget.as_mut(), geometry);
        let _ = widget.tick(60);
        let middle = render(widget.as_mut(), geometry);
        while widget.is_animating() {
            let _ = widget.tick(16);
        }
        let settled = render(widget.as_mut(), geometry);

        // The bar is the rectangle that spans the control's width at the top. The canvas fill
        // is *also* a full-width rect at `y=0`, so the two are told apart by height: the bar is
        // the one bounded by the large-title maximum, while the canvas fill is the control's own
        // height. Reading the wrong one made hte first version of this probe report a constant
        // `120` for every frame -- the page, not the bar.
        let bar_height = |svg: &str| -> u32 {
            svg.lines()
                .filter(|l| l.contains("<rect ") && l.contains("x=\"0\"") && l.contains("y=\"0\""))
                .filter_map(|l| {
                    let w = l.split("width=\"").nth(1)?.split('"').next()?.parse::<u32>().ok()?;
                    let h = l.split("height=\"").nth(1)?.split('"').next()?.parse::<u32>().ok()?;
                    Some((w, h))
                })
                .filter(|(w, h)| *w == 375 && *h <= 96)
                .map(|(_, h)| h)
                .next()
                .unwrap_or(0)
        };
        let (h_first, h_middle, h_settled) =
            (bar_height(&first), bar_height(&middle), bar_height(&settled));
        println!(
            "cupertino_navigation_bar collapse: first={h_first} middle={h_middle} settled={h_settled}"
        );

        assert_eq!(h_settled, 44, "a settled collapse is the compact bar exactly");
        assert!(
            h_middle > 44 && h_middle < 96,
            "part-way through, the bar must be **between** the two ends, not snapped to one \
             ({h_middle} px); a bool flip paints 96 then 44 with nothing in between"
        );
        assert!(h_first > h_middle, "it collapses downward, not upward");
    }
}
