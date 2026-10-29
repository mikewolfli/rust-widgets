// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The reusable column-header strip (`GridViewHeader`) and its paint/input.

use super::*;

/// A column header strip shared by the grid-family controls.
///
/// `DataGrid`, `GridTable` and `Pagination` each used to draw their own header, which is why
/// one showed sort arrows without filter affordances and another the reverse. This control is
/// the single implementation: it owns the section geometry, paints the arrows and filter glyphs,
/// and reports the three gestures a header can receive (click, resize, filter).
///
/// It is **not** a data control: `set_sort_indicator` records what the table asked to show, and
/// `section_clicked` says what the reader did. Sorting the rows remains the table's job.
pub struct GridViewHeader {
    pub(crate) base: BaseWidget,
    pub(crate) columns: Vec<ColumnSpec>,
    /// The order shown per section, `None` when unsorted.
    pub(crate) sorts: Vec<Option<SortOrder>>,
    /// Whether sort arrows are painted.
    pub(crate) sort_indicator_visible: bool,
    /// How section widths are decided.
    pub(crate) resize_mode: ColumnWidthMode,
    /// Whether filter glyphs are painted on filterable sections.
    pub(crate) filter_icon_visible: bool,
    /// The section the pointer is over.
    pub(crate) hovered: Option<usize>,
    /// The drag in progress: `(section, press x, width at the press)`.
    pub(crate) resizing: Option<(usize, i32, u32)>,
    /// Emitted with the section index when its body is clicked.
    pub section_clicked: Signal1<usize>,
    /// Emitted with `(section, width)` after a drag that changed a width.
    pub section_resized: Signal1<(usize, u32)>,
    /// Emitted with the section index when its filter glyph is clicked.
    pub filter_clicked: Signal1<usize>,
}

impl GridViewHeader {
    /// Creates an empty header.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::GridViewHeader, geometry, "GridViewHeader"),
            columns: Vec::new(),
            sorts: Vec::new(),
            sort_indicator_visible: true,
            resize_mode: ColumnWidthMode::Fixed,
            filter_icon_visible: true,
            hovered: None,
            resizing: None,
            section_clicked: Signal1::new(),
            section_resized: Signal1::new(),
            filter_clicked: Signal1::new(),
        }
    }

    /// The configured columns.
    pub fn columns(&self) -> &[ColumnSpec] {
        &self.columns
    }

    /// Replaces the columns.
    ///
    /// Sort indicators survive for the sections that still exist; a section that is gone cannot
    /// keep an arrow nothing paints.
    pub fn set_columns(&mut self, columns: Vec<ColumnSpec>) {
        let mut sorts = core::mem::take(&mut self.sorts);
        sorts.resize(columns.len(), None);
        self.sorts = sorts;
        self.columns = columns;
        self.hovered = None;
        self.resizing = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether sort arrows are painted.
    pub fn sort_indicator_visible(&self) -> bool {
        self.sort_indicator_visible
    }

    /// Shows or hides sort arrows.
    pub fn set_sort_indicator_visible(&mut self, visible: bool) {
        if self.sort_indicator_visible == visible {
            return;
        }
        self.sort_indicator_visible = visible;
        self.base.request_redraw();
    }

    /// Whether filter glyphs are painted.
    pub fn filter_icon_visible(&self) -> bool {
        self.filter_icon_visible
    }

    /// Shows or hides the filter glyphs of filterable sections.
    pub fn set_filter_icon_visible(&mut self, visible: bool) {
        if self.filter_icon_visible == visible {
            return;
        }
        self.filter_icon_visible = visible;
        self.base.request_redraw();
    }

    /// How section widths are decided.
    pub fn resize_mode(&self) -> ColumnWidthMode {
        self.resize_mode
    }

    /// Sets how section widths are decided.
    pub fn set_resize_mode(&mut self, mode: ColumnWidthMode) {
        if self.resize_mode == mode {
            return;
        }
        self.resize_mode = mode;
        self.base.request_redraw();
    }

    /// The width painted for section `column`.
    pub fn section_width(&self, column: usize) -> u32 {
        let count = self.columns.len().max(1) as u32;
        let fixed = |column: usize| self.columns.get(column).map_or(1, |spec| spec.width.max(1));
        match self.resize_mode {
            ColumnWidthMode::Fixed | ColumnWidthMode::Interactive => fixed(column),
            ColumnWidthMode::Stretch => (self.geometry().width / count).max(1),
            ColumnWidthMode::StretchLast => {
                if column + 1 != count as usize {
                    return fixed(column);
                }
                let used: u32 = (0..self.columns.len().saturating_sub(1)).map(fixed).sum();
                self.geometry().width.saturating_sub(used).max(1)
            }
        }
    }

    /// The painted box of section `column`.
    ///
    /// Paint, hit-testing and the resize handle all read this one derivation, so a press can
    /// never land in a section other than the one it is drawn in.
    pub fn section_rect(&self, column: usize) -> Option<Rect> {
        if column >= self.columns.len() {
            return None;
        }
        let rect = self.geometry();
        let x: i32 = (0..column).map(|index| self.section_width(index) as i32).sum();
        let width = self.section_width(column);
        let trimmed = width.min((rect.width as i32 - x).max(0) as u32);
        (trimmed > 0).then(|| Rect::new(rect.x + x, rect.y, trimmed, rect.height))
    }

    /// The painted box of section `column`'s filter glyph, when it has one.
    pub fn filter_rect(&self, column: usize) -> Option<Rect> {
        if !self.filter_icon_visible {
            return None;
        }
        if !self.columns.get(column).is_some_and(|spec| spec.filterable) {
            return None;
        }
        let section = self.section_rect(column)?;
        if section.width < HEADER_ICON_SIDE + 4 {
            return None;
        }
        Some(Rect::new(
            section.x + section.width as i32 - HEADER_ICON_SIDE as i32 - 3,
            section.y + (section.height as i32 - HEADER_ICON_SIDE as i32) / 2,
            HEADER_ICON_SIDE,
            HEADER_ICON_SIDE,
        ))
    }

    /// The order shown for section `column`.
    pub fn sort_indicator(&self, column: usize) -> Option<SortOrder> {
        self.sorts.get(column).copied().flatten()
    }

    /// Records the sort order to paint on section `column`.
    ///
    /// `None` clears the arrow. Returns `false` for a section that does not exist: a caller
    /// asking to mark a column the header does not have has made a mistake worth reporting.
    pub fn set_sort_indicator(&mut self, column: usize, order: Option<SortOrder>) -> bool {
        if column >= self.columns.len() {
            return false;
        }
        if self.sorts[column] == order {
            return true;
        }
        self.sorts[column] = order;
        self.base.request_redraw();
        true
    }

    /// Cycles section `column` between ascending, descending and unsorted.
    ///
    /// Returns the order it landed on, or `None` when the section is not sortable or does not
    /// exist — a header that reported a sort it never painted would be lying to its table.
    pub fn toggle_sort(&mut self, column: usize) -> Option<SortOrder> {
        let spec = self.columns.get(column)?;
        if !spec.sortable {
            return None;
        }
        let next = match self.sorts.get(column).copied().flatten() {
            None => Some(SortOrder::Ascending),
            Some(SortOrder::Ascending) => Some(SortOrder::Descending),
            Some(SortOrder::Descending) => None,
        };
        let _ = self.set_sort_indicator(column, next);
        next
    }

    /// The section under `pos`.
    pub(crate) fn section_at(&self, pos: Point) -> Option<usize> {
        (0..self.columns.len())
            .find(|column| self.section_rect(*column).is_some_and(|rect| rect.contains_point(pos)))
    }

    /// The section whose right edge `pos` is on, when that section may be resized.
    pub(crate) fn resize_handle_at(&self, pos: Point) -> Option<usize> {
        let column = self.section_at(pos)?;
        let spec = self.columns.get(column)?;
        if !spec.resizable || !matches!(self.resize_mode, ColumnWidthMode::Interactive) {
            return None;
        }
        let section = self.section_rect(column)?;
        let right = section.x + section.width as i32;
        ((pos.x - right).abs() <= HEADER_RESIZE_GRAB).then_some(column)
    }

    /// Sets one section's width, switching to interactive sizing.
    pub fn set_section_width(&mut self, column: usize, width: u32) -> bool {
        let Some(spec) = self.columns.get_mut(column) else {
            return false;
        };
        let width = width.max(8);
        if spec.width == width {
            return true;
        }
        spec.width = width;
        self.resize_mode = ColumnWidthMode::Interactive;
        self.base.request_layout();
        self.base.request_redraw();
        true
    }
}

impl Widget for GridViewHeader {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(320, HEADER_STRIP_HEIGHT)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves the three header gestures; see `DiffViewer`'s note for why the arms are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "section_clicked" => {
                Some(EventSignalRef::mapped("section_clicked", &self.section_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "section_resized" => {
                Some(EventSignalRef::mapped("section_resized", &self.section_resized, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            "filter_clicked" => {
                Some(EventSignalRef::mapped("filter_clicked", &self.filter_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for GridViewHeader {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "columns" => Ok(CapabilityValue::Tuple(
                self.columns.iter().map(|spec| CapabilityValue::String(spec.to_token())).collect(),
            )),
            "column_count" => Ok(CapabilityValue::UInt(self.columns.len() as u64)),
            "sort_indicator_visible" => Ok(CapabilityValue::Bool(self.sort_indicator_visible)),
            "resize_mode" => {
                Ok(CapabilityValue::String(column_width_mode_to_str(self.resize_mode).to_string()))
            }
            "filter_icon_visible" => Ok(CapabilityValue::Bool(self.filter_icon_visible)),
            "sort_columns" => Ok(CapabilityValue::String(
                self.sorts
                    .iter()
                    .enumerate()
                    .filter_map(|(column, order)| {
                        order.map(|order| format!("{column}:{}", order.as_str()))
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "columns" => {
                let items = expect_string_list(value)?;
                let mut columns = Vec::with_capacity(items.len());
                for item in items {
                    columns.push(
                        ColumnSpec::from_token(&item).ok_or(CapabilityAccessError::TypeMismatch)?,
                    );
                }
                self.set_columns(columns);
                Ok(())
            }
            "column_count" | "sort_columns" => Err(CapabilityAccessError::ReadOnlyProperty),
            "sort_indicator_visible" => {
                self.set_sort_indicator_visible(expect_bool(value)?);
                Ok(())
            }
            "resize_mode" => {
                self.set_resize_mode(expect_column_width_mode(value)?);
                Ok(())
            }
            "filter_icon_visible" => {
                self.set_filter_icon_visible(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "columns",
            "column_count",
            "sort_indicator_visible",
            "resize_mode",
            "filter_icon_visible",
            "sort_columns",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl crate::event::EventHandler for GridViewHeader {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(column) = self.resize_handle_at(*pos) {
                    let width = self.section_width(column);
                    self.resizing = Some((column, pos.x, width));
                    return;
                }
                if let Some(column) = (0..self.columns.len()).rev().find(|column| {
                    self.filter_rect(*column).is_some_and(|rect| rect.contains_point(*pos))
                }) {
                    self.filter_clicked.emit(column);
                    return;
                }
                if let Some(column) = self.section_at(*pos) {
                    let _ = self.toggle_sort(column);
                    self.section_clicked.emit(column);
                }
            }
            Event::MouseMove { pos } => {
                if let Some((column, press_x, start_width)) = self.resizing {
                    let width = (start_width as i32 + (pos.x - press_x)).max(8) as u32;
                    if let Some(spec) = self.columns.get_mut(column) {
                        if spec.width != width {
                            spec.width = width;
                            self.base.request_redraw();
                        }
                    }
                    return;
                }
                let hovered = self.section_at(*pos);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                if let Some((column, _, start_width)) = self.resizing.take() {
                    let width = self.section_width(column);
                    if width != start_width {
                        self.section_resized.emit((column, width));
                    }
                    self.base.request_redraw();
                }
            }
            _ => {}
        }
    }
}

impl Draw for GridViewHeader {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("data_grid");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        let font = effective_font(style);

        context.face_with_gradient_corners(
            rect,
            surface.blend(&ink, 0.06),
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        for column in 0..self.columns.len() {
            let Some(section) = self.section_rect(column) else {
                continue;
            };
            if self.hovered == Some(column) {
                context.fill_rect(section, surface.blend(&ink, 0.03));
            }
            let spec = &self.columns[column];
            let filter_slot = if self.filter_rect(column).is_some() { 12i32 } else { 0 };
            let sort_slot = if self.sort_indicator_visible && self.sort_indicator(column).is_some()
            {
                12
            } else {
                0
            };
            let title_width =
                section.width.saturating_sub(8 + filter_slot.max(0) as u32 + sort_slot).max(8);
            let label = crate::widget::text_utils::elide_end(
                &spec.title,
                (title_width as usize / 7).max(1),
            );
            context.draw_text(
                Point::new(section.x + 4, section.y + (section.height as i32 - 12) / 2),
                &label,
                font,
                ink,
                HorizontalAlignment::Left,
            );
            if self.sort_indicator_visible {
                if let Some(order) = self.sort_indicator(column) {
                    let icon = if order.is_descending() {
                        crate::widget::IconName::ArrowDown
                    } else {
                        crate::widget::IconName::ArrowUp
                    };
                    let icon_rect = Rect::new(
                        section.x + section.width as i32
                            - filter_slot
                            - HEADER_ICON_SIDE as i32
                            - 3,
                        section.y + (section.height as i32 - HEADER_ICON_SIDE as i32) / 2,
                        HEADER_ICON_SIDE,
                        HEADER_ICON_SIDE,
                    );
                    crate::widget::draw_icon_at(context, icon_rect, ink, icon);
                }
            }
            if let Some(filter_rect) = self.filter_rect(column) {
                crate::widget::draw_icon_at(
                    context,
                    filter_rect,
                    ink.blend(&surface, 0.25),
                    crate::widget::IconName::Filter,
                );
            }
            context.draw_line(
                Point::new(section.x + section.width as i32 - 1, section.y),
                Point::new(section.x + section.width as i32 - 1, section.y + section.height as i32),
                surface.blend(&ink, 0.12),
            );
        }
    }
}
