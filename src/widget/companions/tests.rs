//! Focused tests for the companion controls.

use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::types::CapabilityValue;
use crate::widget::capability::WidgetProperties;
use crate::widget::special_widgets::toast::Toast;
use crate::widget::view_widgets::filter_expr::FilterOperator;

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
    let value = CapabilityValue::Tuple(vec![CapabilityValue::Tuple(vec![
        CapabilityValue::UInt(0),
        CapabilityValue::String("greater_than".to_string()),
        CapabilityValue::String("10".to_string()),
    ])]);
    row.set("conditions", value).expect("a condition list is writable");
    assert_eq!(row.conditions().len(), 1);
    assert_eq!(row.conditions()[0].operator, FilterOperator::GreaterThan);
    assert_eq!(
        row.get("operator").expect("readable"),
        CapabilityValue::String("contains".to_string())
    );
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
            CapabilityValue::Tuple(vec![CapabilityValue::Tuple(vec![
                CapabilityValue::String("A".to_string()),
                CapabilityValue::Color(Color::BLUE),
                CapabilityValue::Bool(true),
            ])]),
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
