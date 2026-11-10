//! Focused tests for the companion controls.
//!
//! Gated as a **file** (`#![cfg(test)]`), not only by the `#[cfg(test)] mod tests;` in
//! `companions.rs`: lexical tools that scan one file at a time (`check_locking.sh`) cannot see
//! the attribute on the `mod` item, so without this its `Arc<Mutex<Vec<T>>>` recording helpers
//! read as production locking.
#![cfg(test)]

use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::types::CapabilityValue;
use crate::widget::capability::WidgetProperties;
use crate::widget::special_widgets::toast::Toast;
use crate::widget::view_widgets::filter_expr::FilterOperator;
use crate::widget::{BaseWidget, Widget, WidgetKind};

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Records a payload signal into a shared vector.
fn record<T: Clone + Send + 'static>(signal: &Signal1<T>) -> Arc<Mutex<Vec<T>>> {
    let sink = Arc::new(Mutex::new(Vec::new()));
    let handle = Arc::clone(&sink);
    signal.connect(move |value: Arc<T>| {
        if let Ok(mut guard) = handle.lock() {
            guard.push((*value).clone());
        }
    });
    sink
}

/// Reads the recorded values.
fn values<T: Clone>(sink: &Arc<Mutex<Vec<T>>>) -> Vec<T> {
    sink.lock().map(|guard| guard.clone()).unwrap_or_default()
}

/// Counts payload-free emissions.
fn count_units(signal: &GenericSignal) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    let handle = Arc::clone(&count);
    signal.connect(move || {
        handle.fetch_add(1, Ordering::SeqCst);
    });
    count
}

fn unit_count(count: &Arc<AtomicUsize>) -> usize {
    count.load(Ordering::SeqCst)
}

#[test]
fn swatch_click_selects_and_double_click_activates() {
    let mut swatch = Swatch::new(Rect::new(0, 0, 200, 100));
    swatch.set_colors(vec![Color::RED, Color::GREEN, Color::BLUE]);
    let selected = record(&swatch.swatch_selected);
    let activated = record(&swatch.swatch_activated);

    // Cell 1 starts at x = inset 4 + pitch 28.
    swatch.handle_event(&Event::MousePress { pos: Point::new(40, 10), button: 1, modifiers: 0 });
    assert_eq!(swatch.selected_index(), 1);
    assert_eq!(values(&selected), vec![1]);

    swatch.handle_event(&Event::MouseDoubleClick { pos: Point::new(40, 10), button: 1 });
    assert_eq!(values(&activated), vec![Color::GREEN]);
}

#[test]
fn swatch_properties_round_trip() {
    let mut swatch = Swatch::new(Rect::new(0, 0, 200, 100));
    swatch
        .set("colors", CapabilityValue::Tuple(vec![CapabilityValue::Color(Color::BLUE)]))
        .expect("a colour list is writable");
    swatch.set("selected_index", CapabilityValue::UInt(0)).expect("writable");
    swatch.set("cell_size", CapabilityValue::UInt(30)).expect("writable");
    swatch.set("columns", CapabilityValue::UInt(2)).expect("writable");
    assert_eq!(swatch.colors(), &[Color::BLUE]);
    assert_eq!(swatch.cell_size(), 30);
    assert_eq!(swatch.columns(), 2);
    assert!(swatch.property_names().contains(&"colors"));
}

#[test]
fn filter_row_live_edit_publishes_and_clear_notifies() {
    let mut row = FilterRow::new(Rect::new(0, 0, 300, 36));
    row.set_columns(vec!["Name".to_string(), "Status".to_string()]);
    let changed = record(&row.filter_changed);
    let cleared = count_units(&row.filter_cleared);

    row.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1, modifiers: 0 });
    row.handle_event(&Event::TextInput { text: "open".to_string() });
    assert_eq!(row.conditions().len(), 1);
    assert_eq!(row.conditions()[0].column, 0);
    assert_eq!(row.conditions()[0].operand, "open");
    assert_eq!(values(&changed).len(), 1);

    row.clear_filters();
    assert!(row.conditions().is_empty());
    assert_eq!(unit_count(&cleared), 1);
}

#[test]
fn filter_row_conditions_property_round_trips() {
    let mut row = FilterRow::new(Rect::new(0, 0, 300, 36));
    row.set_columns(vec!["Name".to_string()]);
    let value = CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
        &crate::widget::capability::FILTER_CONDITION_SCHEMA,
        vec![crate::widget::capability::RecordValue::new()
            .with("column", CapabilityValue::UInt(0))
            .with("operator", CapabilityValue::String("greater_than".to_string()))
            .with("operand", CapabilityValue::String("10".to_string()))],
    ));
    row.set("conditions", value).expect("a condition list is writable");
    assert_eq!(row.conditions().len(), 1);
    assert_eq!(row.conditions()[0].operator, FilterOperator::GreaterThan);
    assert_eq!(
        row.get("operator").expect("readable"),
        CapabilityValue::String("contains".to_string())
    );
}

/// CAP-1: the published `filter_changed` payload is the structured condition list, not the
/// Rust `Debug` spelling the arm used to emit.
#[test]
fn filter_row_event_payload_is_a_structured_condition_list() {
    use crate::widget::Widget;

    let mut row = FilterRow::new(Rect::new(0, 0, 300, 36));
    row.set_columns(vec!["Name".to_string()]);
    let event = row.event_signal_dyn("filter_changed").expect("the event is published");
    let delivered = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&delivered);
    let handle = event.subscribe(Box::new(move |value| sink.lock().unwrap().push(value.clone())));

    row.set_condition(
        0,
        crate::widget::view_widgets::filter_expr::FilterCondition::new(
            0,
            FilterOperator::Equals,
            "Ada",
        ),
    );
    let payloads = delivered.lock().unwrap().clone();
    event.disconnect(handle);

    assert_eq!(
        payloads,
        vec![CapabilityValue::Tuple(vec![CapabilityValue::Tuple(vec![
            CapabilityValue::UInt(0),
            CapabilityValue::String("equals".to_string()),
            CapabilityValue::String("Ada".to_string()),
        ])])],
        "each condition must arrive as [column, operator, operand] rather than a debug string"
    );
}

/// CP2-1: verbs the capability does not publish must answer `UnknownCommand`.
///
/// `OutOfRange` means "the command exists but needs a payload", so intercepting an
/// unpublished name as `OutOfRange` told a caller the opposite of the truth. The names below
/// are deliberately absent from `layer_panel`'s commands table (the selection / visibility /
/// scroll verbs are Rust-side) and `track_selected` is an event, not a command.
#[test]
fn unpublished_panel_verbs_answer_unknown_command() {
    use crate::widget::capability::types::CapabilityAccessError;

    let mut layers = LayerPanel::new(Rect::new(0, 0, 200, 120));
    for name in ["select_layer", "set_entry_visible", "scroll_to_layer"] {
        assert_eq!(
            layers.command(name),
            Err(CapabilityAccessError::UnknownCommand),
            "`{name}` is not a published command, so it must not claim it needs a payload"
        );
    }

    let mut playlist = PlaylistPanel::new(Rect::new(0, 0, 200, 120));
    assert_eq!(
        playlist.command("track_selected"),
        Err(CapabilityAccessError::UnknownCommand),
        "`track_selected` is an event name, not a command"
    );
    // The two published `set_*` names still answer `OutOfRange`, because their payload is the
    // property write.
    assert_eq!(playlist.command("set_tracks"), Err(CapabilityAccessError::OutOfRange),);
    assert_eq!(playlist.command("set_filter"), Err(CapabilityAccessError::OutOfRange),);
}

#[test]
fn legend_click_reports_and_toggles_a_checkable_series() {
    let mut legend = Legend::new(Rect::new(0, 0, 200, 60));
    legend.set_entries(vec![
        LegendEntry::new("Revenue", Color::GREEN),
        LegendEntry::new("Cost", Color::RED),
    ]);
    legend.set_checkable(true);
    legend.set_orientation(LegendOrientation::Horizontal);
    let clicked = record(&legend.entry_clicked);
    let toggled = record(&legend.series_toggled);

    legend.handle_event(&Event::MousePress { pos: Point::new(10, 20), button: 1, modifiers: 0 });
    assert_eq!(values(&clicked), vec![0]);
    assert_eq!(values(&toggled), vec![(0, false)]);
    assert!(!legend.entries()[0].visible);
}

#[test]
fn legend_properties_round_trip() {
    let mut legend = Legend::new(Rect::new(0, 0, 200, 60));
    legend
        .set(
            "entries",
            CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                &crate::widget::capability::LEGEND_ENTRY_SCHEMA,
                vec![crate::widget::capability::RecordValue::new()
                    .with("label", CapabilityValue::String("A".to_string()))
                    .with("color", CapabilityValue::Color(Color::BLUE))
                    .with("visible", CapabilityValue::Bool(true))],
            )),
        )
        .expect("entry list is writable");
    legend.set("checkable", CapabilityValue::Bool(true)).expect("writable");
    legend.set("orientation", CapabilityValue::String("horizontal".to_string())).expect("writable");
    legend.set("position", CapabilityValue::String("right".to_string())).expect("writable");
    assert_eq!(legend.entries()[0].label, "A");
    assert!(legend.is_checkable());
    assert_eq!(legend.orientation(), LegendOrientation::Horizontal);
    assert_eq!(legend.position(), LegendPosition::Right);
}

#[test]
fn chart_tooltip_shows_follows_and_hides() {
    let mut tooltip = ChartTooltip::new(Rect::new(0, 0, 300, 200));
    let shown = count_units(&tooltip.shown);
    let hidden = count_units(&tooltip.hidden);

    tooltip.show_at(Point::new(20, 20), vec![TooltipLine::new("Series A: 42")]);
    assert!(tooltip.is_visible());
    assert_eq!(unit_count(&shown), 1);

    tooltip.handle_event(&Event::MouseMove { pos: Point::new(60, 70) });
    assert_eq!(tooltip.anchor(), Point::new(60, 70));

    tooltip.hide();
    assert!(!tooltip.is_visible());
    assert_eq!(unit_count(&hidden), 1);
}

#[test]
fn tooltip_manager_honours_delay_and_duration() {
    let mut manager = TooltipManager::new(Rect::new(0, 0, 300, 200));
    manager.set_delay_ms(500);
    manager.set_duration_ms(1000);
    manager.register(7, "Saved");
    let shown = record(&manager.tooltip_shown);
    let hidden = record(&manager.tooltip_hidden);

    manager.note_hover(7);
    assert!(!manager.tick(400, 400));
    assert!(manager.tooltip().is_none());

    assert!(manager.tick(200, 600));
    assert_eq!(manager.tooltip(), Some("Saved"));
    assert_eq!(values(&shown), vec![7]);

    assert!(manager.tick(0, 1601));
    assert!(manager.tooltip().is_none());
    assert_eq!(values(&hidden), vec![7]);
}

#[test]
fn zoom_pan_controller_zooms_fits_and_marks_roi() {
    let mut controller = ZoomPanController::new(Rect::new(0, 0, 200, 100));
    controller.set_content_size(Size::new(400, 200));
    let zoom = record(&controller.zoom_changed);
    let roi = record(&controller.roi_changed);

    controller.zoom_in();
    assert!((controller.zoom_level() - 1.25).abs() < f32::EPSILON);
    assert_eq!(values(&zoom).len(), 1);

    controller.fit_to_width();
    assert!((controller.zoom_level() - 0.5).abs() < f32::EPSILON);
    assert_eq!(controller.pan_offset(), Point::new(0, 0));

    controller.set_roi(Rect::new(10, 10, 40, 20));
    assert_eq!(values(&roi).last(), Some(&Some(Rect::new(10, 10, 40, 20))));
    controller.clear_roi();
    assert_eq!(values(&roi).last(), Some(&None));
}

#[test]
fn zoom_pan_panning_wheel_and_properties() {
    let mut controller = ZoomPanController::new(Rect::new(0, 0, 200, 100));
    controller.handle_event(&Event::MousePress {
        pos: Point::new(10, 10),
        button: 1,
        modifiers: 0,
    });
    controller.handle_event(&Event::MouseMove { pos: Point::new(25, 20) });
    assert_eq!(controller.pan_offset(), Point::new(15, 10));

    controller.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
    assert!(controller.zoom_level() > 1.0);
    controller
        .set("zoom_mode", CapabilityValue::String("fit_width".to_string()))
        .expect("writable");
    assert_eq!(controller.zoom_mode(), ZoomMode::FitWidth);
    controller.set("roi", CapabilityValue::Null).expect("writable");
    assert!(controller.roi().is_none());
}

#[test]
fn toast_host_expires_toasts_and_reports_overflow() {
    let mut host = ToastHost::new(Rect::new(0, 0, 320, 200));
    let expired = record(&host.toast_expired);
    let mut toast = Toast::new(Rect::new(0, 0, 10, 10), "Saved");
    toast.set_ttl_ms(1000);
    let id = host.push_at(toast, 0).expect("first toast is visible");
    assert_eq!(host.visible_toasts().len(), 1);

    assert_eq!(host.expire_toasts(1001), 1);
    assert_eq!(values(&expired), vec![id]);
    assert!(host.visible_toasts().is_empty());

    host.set_max_visible(1);
    host.set_max_queued(1);
    host.set_overflow_policy(ToastOverflowPolicy::Queue);
    let overflow = record(&host.queue_overflow);
    host.push_at(Toast::new(Rect::new(0, 0, 10, 10), "A"), 2000).expect("visible");
    host.push_at(Toast::new(Rect::new(0, 0, 10, 10), "B"), 2000).expect("queued");
    assert!(host.push_at(Toast::new(Rect::new(0, 0, 10, 10), "C"), 2000).is_none());
    assert_eq!(values(&overflow).len(), 1);
}

#[test]
fn toast_host_dismiss_and_properties() {
    let mut host = ToastHost::new(Rect::new(0, 0, 320, 200));
    let dismissed = record(&host.toast_dismissed);
    let id = host.push_at(Toast::new(Rect::new(0, 0, 10, 10), "A"), 0).expect("visible");
    assert!(host.dismiss(id));
    assert_eq!(values(&dismissed), vec![id]);
    assert!(!host.dismiss(id));

    host.set("max_visible", CapabilityValue::UInt(2)).expect("writable");
    host.set("max_queued", CapabilityValue::UInt(3)).expect("writable");
    host.set("default_duration_ms", CapabilityValue::UInt(1500)).expect("writable");
    host.set("anchor", CapabilityValue::String("top_right".to_string())).expect("writable");
    host.set("stacking_direction", CapabilityValue::String("newest_at_bottom".to_string()))
        .expect("writable");
    host.set("overflow_policy", CapabilityValue::String("drop_newest".to_string()))
        .expect("writable");
    assert_eq!(host.max_visible(), 2);
    assert_eq!(host.max_queued(), 3);
    assert_eq!(host.default_duration_ms(), 1500);
    assert_eq!(host.anchor(), ToastAnchor::TopRight);
    assert_eq!(host.stacking_direction(), ToastStackingDirection::NewestAtBottom);
    assert_eq!(host.overflow_policy(), ToastOverflowPolicy::DropNewest);
}

// ── Closed/focus/wheel/popup invariants ─────────────────────────────────────

/// A container that records every wheel the router offers it after the child declines.
struct WheelProbe {
    base: BaseWidget,
    wheels: Arc<AtomicUsize>,
}

impl WheelProbe {
    fn new(geometry: Rect) -> (Self, Arc<AtomicUsize>) {
        let wheels = Arc::new(AtomicUsize::new(0));
        (
            Self {
                base: BaseWidget::new(WidgetKind::Panel, geometry, "WheelProbe"),
                wheels: Arc::clone(&wheels),
            },
            wheels,
        )
    }
}

impl Widget for WheelProbe {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn accepts_wheel(&self) -> bool {
        true
    }
}

impl EventHandler for WheelProbe {
    fn handle_event(&mut self, event: &Event) {
        // The containers this stands in for consult `enabled` before consuming a wheel, so a
        // disabled probe must not count one either — otherwise the fixture would model a widget
        // the crate's enabled contract forbids.
        if !self.base.is_enabled() {
            return;
        }
        if matches!(event, Event::Wheel { .. }) {
            self.wheels.fetch_add(1, Ordering::SeqCst);
        }
    }
}

/// Mounts `child` under a [`WheelProbe`] and links the parent half through the registry.
fn mount_under_wheel_probe(
    child: Box<dyn Widget>,
) -> (crate::core::ObjectId, crate::core::ObjectId, Arc<AtomicUsize>) {
    let (probe, wheels) = WheelProbe::new(Rect::new(0, 0, 400, 300));
    let parent = crate::widget::runtime::register(Box::new(probe)).expect("mount the wheel probe");
    let child = crate::widget::runtime::register(child).expect("mount the companion");
    crate::widget::runtime::with_widget_mut(parent, |widget| {
        widget.base_mut().add_child_linked(child);
    });
    (parent, child, wheels)
}

/// Mounts `popup` under a window so `top_level_window_of` can resolve the grab owner.
fn mount_popup_in_window(popup: Box<dyn Widget>) -> (crate::core::ObjectId, crate::core::ObjectId) {
    let popup = crate::widget::runtime::register(popup).expect("mount the popup");
    let window = crate::widget::runtime::register(Box::new(crate::widget::Window::new(
        "popup host".to_string(),
        Rect::new(0, 0, 800, 600),
    )))
    .expect("mount the window");
    crate::widget::runtime::with_widget_mut(window, |widget| {
        widget.base_mut().add_child_linked(popup);
    });
    (window, popup)
}

/// Opens a popup, asserts it took the grab, then presses far outside and asserts it closed.
fn assert_popup_owns_outside_press(
    popup: Box<dyn Widget>,
    open: impl FnOnce(&mut dyn Widget),
    still_open: impl Fn(&mut dyn Widget) -> bool,
) {
    use crate::widget::runtime::dispatch_pointer_event;

    let (window, id) = mount_popup_in_window(popup);
    crate::widget::runtime::with_widget_mut(id, open);
    assert_eq!(
        crate::widget::runtime::popup_grab(),
        Some(id),
        "an open popup must arm the one-shot grab"
    );

    let point = Point::new(700, 550);
    assert!(
        dispatch_pointer_event(window, &Event::mouse_press(point.x, point.y, 1), point),
        "the outside press must reach the popup instead of falling through the window"
    );
    assert!(
        !crate::widget::runtime::with_widget_mut(id, |widget| still_open(widget)).unwrap_or(true),
        "the outside press must dismiss the popup"
    );
    assert_eq!(crate::widget::runtime::popup_grab(), None, "the dismissal must release the grab");

    crate::widget::runtime::unregister(id);
    crate::widget::runtime::unregister(window);
}

/// FINDING 1: a host that paints nothing while closed must not be hit-testable.
///
/// The capability table declares these hosts `visible` by default, so a freshly constructed
/// closed host starts with the base flag set; its own `contains_point` is what keeps the closed
/// rectangle from swallowing presses. Once toggled, `open`/`close` also keep the base flag in
/// step, so a closed host is invisible to both halves of the hit rule.
#[test]
fn a_closed_host_keeps_its_base_flag_in_step_with_its_open_state() {
    use crate::widget::Widget;

    let mut palette = CommandPaletteHost::new(Rect::new(240, 120, 480, 320));
    assert!(palette.base().is_visible(), "the declared default is visible");
    assert!(!palette.contains_point(Point::new(250, 130)), "a closed palette takes no press");
    palette.open();
    assert!(palette.contains_point(Point::new(250, 130)));
    palette.close();
    assert!(!palette.base().is_visible());
    assert!(!palette.contains_point(Point::new(250, 130)));

    let mut menu = MenuPanel::new(Rect::new(0, 0, 180, 100));
    assert!(!menu.contains_point(Point::new(10, 10)));
    menu.open();
    assert!(menu.contains_point(Point::new(10, 10)));
    menu.close();
    assert!(!menu.base().is_visible());
    assert!(!menu.contains_point(Point::new(10, 10)));

    let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
    assert!(!bar.contains_point(Point::new(10, 10)));
    bar.open();
    assert!(bar.contains_point(Point::new(10, 10)));
    bar.close();
    assert!(!bar.base().is_visible());
    assert!(!bar.contains_point(Point::new(10, 10)));

    let mut tree_bar = TreeFindBar::new(Rect::new(0, 0, 260, 28));
    assert!(!tree_bar.contains_point(Point::new(10, 10)));
    tree_bar.open();
    assert!(tree_bar.contains_point(Point::new(10, 10)));
    tree_bar.close();
    assert!(!tree_bar.base().is_visible());
    assert!(!tree_bar.contains_point(Point::new(10, 10)));

    let mut popup = DropDownPopupHost::new(Rect::new(0, 0, 200, 200));
    assert!(!popup.contains_point(Point::new(10, 10)), "a closed host must not take the press");
    popup.open_at(Rect::new(0, 0, 80, 24));
    assert!(popup.contains_point(Point::new(10, 10)));
    popup.close();
    assert!(!popup.contains_point(Point::new(10, 10)));
    assert!(!popup.base().is_visible(), "close still keeps the base flag in step");
}

/// FINDING 3: one hover produces one bubble; expiry must not restart the delay forever.
#[test]
fn an_expired_tooltip_stays_hidden_until_the_pointer_returns() {
    use crate::widget::Widget;

    let mut manager = TooltipManager::new(Rect::new(0, 0, 300, 200));
    manager.set_delay_ms(0);
    manager.set_duration_ms(100);
    manager.register(7, "Saved");
    manager.note_hover(7);

    assert!(manager.tick(0, 0), "a zero delay shows the bubble on the first tick");
    assert_eq!(manager.tooltip(), Some("Saved"));
    assert!(manager.tick(200, 200), "the duration expires the bubble");
    assert!(manager.tooltip().is_none());
    assert!(!manager.is_animating(), "an expired hover must not keep the frame bus awake");
    assert!(!manager.tick(16, 216), "and it must not re-show by itself");
    assert!(manager.tooltip().is_none());
}

/// FINDING 4: the documented per-frame `expire_toasts` call must not repaint an empty queue.
#[test]
fn an_empty_toast_queue_does_not_request_a_redraw_every_frame() {
    use crate::widget::runtime::structure_epoch;

    let mut host = ToastHost::new(Rect::new(0, 0, 320, 200));
    let before = structure_epoch();
    assert_eq!(host.expire_toasts(0), 0);
    assert_eq!(structure_epoch(), before, "an empty queue must not request a frame");

    host.push_at(Toast::new(Rect::new(0, 0, 10, 10), "Saved"), 0);
    let after_push = structure_epoch();
    assert_eq!(host.expire_toasts(100_000), 1);
    assert_ne!(structure_epoch(), after_push, "an actual expiry still repaints");
}

/// FINDING 5: opening a find bar moves the keyboard onto it.
#[test]
fn opening_a_find_bar_focuses_the_query() {
    use crate::widget::capability::coercion::widget_as_mut;
    use crate::widget::runtime::{
        clear_focus, focused_widget, register, unregister, with_widget_mut,
    };

    let id = register(Box::new(FindReplaceBar::new(Rect::new(0, 0, 400, 32))))
        .expect("mount the find bar");
    assert!(with_widget_mut(id, |widget| {
        widget_as_mut::<FindReplaceBar>(widget).is_some_and(|bar| {
            bar.open();
            bar.is_open()
        })
    })
    .unwrap_or(false));
    assert_eq!(focused_widget(), Some(id), "open must focus the query field");
    clear_focus();
    unregister(id);

    let id = register(Box::new(TreeFindBar::new(Rect::new(0, 0, 260, 28))))
        .expect("mount the tree find bar");
    assert!(
        with_widget_mut(id, |widget| widget.is_focusable()).unwrap_or(false),
        "the tree find bar must be reachable by Tab"
    );
    assert!(with_widget_mut(id, |widget| {
        widget_as_mut::<TreeFindBar>(widget).is_some_and(|bar| {
            bar.open();
            bar.is_visible()
        })
    })
    .unwrap_or(false));
    assert_eq!(focused_widget(), Some(id), "open must focus the tree bar's query");
    clear_focus();
    unregister(id);
}

/// FINDINGS 7 + 10: the scrollable companions move toward the start for a positive notch,
/// and only the notch that actually moved them claims the wheel.
#[test]
fn a_scrolling_companion_claims_only_the_wheel_that_moved_it() {
    use crate::widget::capability::coercion::widget_as_mut;
    use crate::widget::runtime::{dispatch_wheel_event, unregister, with_widget_mut};

    let wheel = |delta_y: i32| Event::Wheel { delta: Point::new(0, delta_y), modifiers: 0 };

    // LayerPanel: at its first row a positive notch has nowhere to go.
    let mut layers = LayerPanel::new(Rect::new(0, 0, 200, 100));
    layers.set_entries((0..10).map(|index| LayerEntry::new(index, format!("L{index}"))).collect());
    let (parent, id, wheels) = mount_under_wheel_probe(Box::new(layers));
    assert!(dispatch_wheel_event(id, &wheel(1)));
    assert_eq!(wheels.load(Ordering::SeqCst), 1, "a clamped list must leave the notch");
    assert!(dispatch_wheel_event(id, &wheel(-1)));
    assert_eq!(wheels.load(Ordering::SeqCst), 1, "a moved list claims the notch");
    unregister(id);
    unregister(parent);

    // PlaylistPanel behaves the same way.
    let mut playlist = PlaylistPanel::new(Rect::new(0, 0, 240, 100));
    playlist.set_tracks((0..10).map(|index| format!("/music/{index}.mp3")).collect());
    let (parent, id, wheels) = mount_under_wheel_probe(Box::new(playlist));
    assert!(dispatch_wheel_event(id, &wheel(1)));
    assert_eq!(wheels.load(Ordering::SeqCst), 1);
    assert!(dispatch_wheel_event(id, &wheel(-1)));
    assert_eq!(wheels.load(Ordering::SeqCst), 1);
    unregister(id);
    unregister(parent);

    // MenuPanel.
    let (parent, id, wheels) =
        mount_under_wheel_probe(Box::new(MenuPanel::new(Rect::new(0, 0, 200, 100))));
    with_widget_mut(id, |widget| {
        let menu = widget_as_mut::<MenuPanel>(widget).expect("menu panel");
        menu.set_items((0..10).map(|index| MenuPanelItem::new(format!("Row {index}"))).collect());
        menu.set_max_visible_rows(2);
        menu.open();
    });
    assert!(dispatch_wheel_event(id, &wheel(1)));
    assert_eq!(wheels.load(Ordering::SeqCst), 1);
    assert!(dispatch_wheel_event(id, &wheel(-1)));
    assert_eq!(wheels.load(Ordering::SeqCst), 1);
    with_widget_mut(id, |widget| widget_as_mut::<MenuPanel>(widget).expect("menu").close());
    unregister(id);
    unregister(parent);

    // DropDownPopupHost.
    let (parent, id, wheels) =
        mount_under_wheel_probe(Box::new(DropDownPopupHost::new(Rect::new(0, 0, 200, 200))));
    with_widget_mut(id, |widget| {
        let popup = widget_as_mut::<DropDownPopupHost>(widget).expect("popup");
        popup.set_items(
            (0..10)
                .map(|index| PopupItem::new(format!("id{index}"), format!("Row {index}")))
                .collect(),
        );
        popup.set_max_visible_rows(2);
        popup.open_at(Rect::new(0, 0, 80, 24));
    });
    assert!(dispatch_wheel_event(id, &wheel(1)));
    assert_eq!(wheels.load(Ordering::SeqCst), 1);
    assert!(dispatch_wheel_event(id, &wheel(-1)));
    assert_eq!(wheels.load(Ordering::SeqCst), 1);
    with_widget_mut(id, |widget| {
        widget_as_mut::<DropDownPopupHost>(widget).expect("popup").close()
    });
    unregister(id);
    unregister(parent);

    // CommandPaletteHost.
    let (parent, id, wheels) =
        mount_under_wheel_probe(Box::new(CommandPaletteHost::new(Rect::new(0, 0, 480, 120))));
    with_widget_mut(id, |widget| {
        let palette = widget_as_mut::<CommandPaletteHost>(widget).expect("palette");
        palette.set_entries(
            (0..10)
                .map(|index| PaletteEntryRef::new(format!("id{index}"), format!("Row {index}")))
                .collect(),
        );
        palette.open();
    });
    assert!(dispatch_wheel_event(id, &wheel(1)));
    assert_eq!(wheels.load(Ordering::SeqCst), 1);
    assert!(dispatch_wheel_event(id, &wheel(-1)));
    assert_eq!(wheels.load(Ordering::SeqCst), 1);
    with_widget_mut(id, |widget| {
        widget_as_mut::<CommandPaletteHost>(widget).expect("palette").close()
    });
    unregister(id);
    unregister(parent);
}

/// FINDING 7 for the zoom controller: a notch clamped at the zoom limit is not a claim.
#[test]
fn a_wheel_that_cannot_zoom_does_not_claim_the_notch() {
    use crate::widget::capability::coercion::widget_as_mut;
    use crate::widget::runtime::{dispatch_wheel_event, unregister, with_widget_mut};

    let (parent, id, wheels) =
        mount_under_wheel_probe(Box::new(ZoomPanController::new(Rect::new(0, 0, 200, 100))));
    with_widget_mut(id, |widget| {
        let controller = widget_as_mut::<ZoomPanController>(widget).expect("zoom controller");
        let max = controller.max_zoom();
        controller.set_zoom_level(max);
    });

    // A notch in cannot change a zoom already at its maximum.
    assert!(dispatch_wheel_event(id, &Event::Wheel { delta: Point::new(0, -1), modifiers: 0 },));
    assert_eq!(wheels.load(Ordering::SeqCst), 1, "a clamped zoom must leave the notch");

    // A notch out can.
    assert!(dispatch_wheel_event(id, &Event::Wheel { delta: Point::new(0, 1), modifiers: 0 },));
    assert_eq!(wheels.load(Ordering::SeqCst), 1, "the zoom that moved claims it");

    unregister(id);
    unregister(parent);
}

/// FINDING 13: header menus, drop-down hosts and the palette own the outside press.
#[test]
fn every_popup_companion_owns_the_press_that_dismisses_it() {
    use crate::widget::capability::coercion::widget_as_mut;

    assert_popup_owns_outside_press(
        Box::new(HeaderMenu::new(Rect::new(0, 0, 200, 120))),
        |widget| {
            widget_as_mut::<HeaderMenu>(widget)
                .expect("header menu")
                .open_for("name", Rect::new(0, 0, 80, 24));
        },
        |widget| widget_as_mut::<HeaderMenu>(widget).expect("header menu").is_open(),
    );

    assert_popup_owns_outside_press(
        Box::new(DropDownPopupHost::new(Rect::new(0, 0, 200, 200))),
        |widget| {
            widget_as_mut::<DropDownPopupHost>(widget)
                .expect("popup")
                .open_at(Rect::new(0, 0, 80, 24));
        },
        |widget| widget_as_mut::<DropDownPopupHost>(widget).expect("popup").is_open(),
    );

    assert_popup_owns_outside_press(
        Box::new(CommandPaletteHost::new(Rect::new(0, 0, 480, 320))),
        |widget| widget_as_mut::<CommandPaletteHost>(widget).expect("palette").open(),
        |widget| widget_as_mut::<CommandPaletteHost>(widget).expect("palette").is_open(),
    );
}
