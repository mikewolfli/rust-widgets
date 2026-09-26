"""Append the a11y regression tests to the two controls, before each file's final brace."""

RANGE_TEST = '''
    /// A screen reader is told the range, not silence and not half of it.
    ///
    /// # The defect this pins
    ///
    /// `Widget::accessible_value`'s default reads a value property from a fixed list -- `value` /
    /// `progress` / `rating` / `level` -- and `range_slider` publishes `lower` / `upper` instead, so
    /// it announced **nothing** about where the range sat. Renaming a property is not the fix: two
    /// handles are the contract, and announcing one would report half a range as if it were the
    /// value.
    ///
    /// The assertion names both ends, so an implementation that silently announced only `lower`
    /// (the tempting shortcut) fails.
    #[test]
    fn the_range_is_announced_with_both_handles() {
        let mut rs = RangeSlider::new(Rect::new(0, 0, 200, 28));
        rs.set_range(0.0, 100.0);
        rs.set_lower_value(20.0);
        rs.set_upper_value(70.0);

        let announced = rs.accessible_value();
        assert!(
            announced.contains("20"),
            "the lower handle must be announced, got {announced:?}"
        );
        assert!(
            announced.contains("70"),
            "and the upper handle too, got {announced:?}"
        );
        assert!(
            announced.contains('–'),
            "the two ends must read as one range rather than two sentences, got {announced:?}"
        );

        // An integral float is announced as its integer, which is the one formatting rule the
        // capability layer already owns -- asserted here so this control cannot grow a second one.
        assert_eq!(announced, "20\u{2013}70");
    }
}
'''

CALENDAR_TEST = '''
    /// A screen reader is told which day is selected.
    ///
    /// # The defect this pins
    ///
    /// The trait default looks the value up under `value` / `progress` / `rating` / `level`, and a
    /// calendar's value is called `selected_date` -- so the one control whose entire state is the
    /// chosen day announced nothing about it.
    ///
    /// The assertion reads the *date the control reports* and requires the announcement to contain
    /// it, so the two cannot be told two different days.
    #[test]
    fn the_selected_day_is_announced() {
        use crate::widget::Widget;
        use crate::widget::capability::WidgetProperties;

        let mut calendar = Calendar::new(Rect::new(0, 0, 260, 240));
        let day = match chrono::NaiveDate::from_ymd_opt(2026, 3, 9) {
            Some(date) => date,
            None => panic!("a valid literal date"),
        };
        calendar.set_selected_date(day);

        let announced = calendar.accessible_value();
        let reported = match calendar.get("selected_date") {
            Ok(value) => value.to_announcement_string(),
            Err(e) => panic!("the calendar publishes selected_date: {e:?}"),
        };
        assert_eq!(
            announced, reported,
            "the announcement and the published property must be the same date"
        );
        assert!(
            announced.contains("2026-03-09"),
            "and it must be the date that was set, got {announced:?}"
        );
    }
}
'''


def append_before_last_brace(path: str, block: str) -> None:
    src = open(path, encoding="utf-8").read()
    idx = src.rstrip().rfind("}")
    if idx < 0:
        raise SystemExit(f"no closing brace in {path}")
    head = src[:idx].rstrip("\n")
    open(path, "w", encoding="utf-8").write(head + "\n" + block)


append_before_last_brace("src/widget/input_widgets/range_slider.rs", RANGE_TEST)
append_before_last_brace("src/widget/advanced_widgets/calendar.rs", CALENDAR_TEST)
print("appended")
