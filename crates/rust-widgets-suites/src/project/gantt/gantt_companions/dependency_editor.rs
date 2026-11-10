// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//!
//! Dependency edges and the `DependencyEditor` control.

use rust_widgets::core::{Color, HorizontalAlignment, Point, Rect};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::RenderContext;
use rust_widgets::signal::Signal1;
use rust_widgets::widget::capability::coercion::expect_bool;
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::effective_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

use super::super::gantt_widget::DependencyKind;
use super::common::chart_chrome_palette;
use rust_widgets::impl_draw_bridge;

/// One dependency edge: `predecessor` must come before `successor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// Id of the task that comes first.
    pub predecessor: String,
    /// Id of the task that waits.
    pub successor: String,
    /// The relationship kind. [`Dependency::new`] leaves it finish-to-start, which is what the
    /// untyped `GanttTask::depends_on` spelling means.
    pub kind: DependencyKind,
}

impl Dependency {
    /// Creates a finish-to-start edge from `predecessor` to `successor`.
    pub fn new(predecessor: impl Into<String>, successor: impl Into<String>) -> Self {
        Self {
            predecessor: predecessor.into(),
            successor: successor.into(),
            kind: DependencyKind::default(),
        }
    }

    /// Creates an edge with an explicit relationship kind.
    pub fn with_kind(
        predecessor: impl Into<String>,
        successor: impl Into<String>,
        kind: DependencyKind,
    ) -> Self {
        Self { predecessor: predecessor.into(), successor: successor.into(), kind }
    }
}

/// Height of one dependency row.
const DEPENDENCY_ROW_HEIGHT: i32 = 24;
/// Padding inside the editor.
const DEPENDENCY_PADDING: i32 = 8;
/// Height reserved for the editor's caption.
const DEPENDENCY_HEADER: i32 = 18;

/// The row shape of the `dependency_edges` property.
///
/// An edge is one flat record. `kind` carries the same `fts`/`sts`/`ftf` token a gantt task's
/// `dependencies` field uses, so the two list-shaped contracts in this family describe an edge
/// identically. The identity field is the successor: it is present on every edge and gives a
/// designer a stable handle to list against, while duplicate edges (which `set_dependencies`
/// already refuses) are left to that method rather than refused twice with two different errors.
pub const DEPENDENCY_SCHEMA: rust_widgets::widget::capability::RecordSchema =
    rust_widgets::widget::capability::RecordSchema {
        fields: &[
            rust_widgets::widget::capability::RecordField::required(
                "predecessor",
                rust_widgets::widget::capability::PropertyValueKind::String,
            ),
            rust_widgets::widget::capability::RecordField::required(
                "successor",
                rust_widgets::widget::capability::PropertyValueKind::String,
            ),
            rust_widgets::widget::capability::RecordField::optional(
                "kind",
                rust_widgets::widget::capability::PropertyValueKind::String,
            ),
        ],
        id_field: "successor",
        ids_unique: false,
        ids_required: false,
    };

/// One row of [`DEPENDENCY_SCHEMA`].
fn dependency_record(dependency: &Dependency) -> rust_widgets::widget::capability::RecordValue {
    rust_widgets::widget::capability::RecordValue::new()
        .with("predecessor", CapabilityValue::String(dependency.predecessor.clone()))
        .with("successor", CapabilityValue::String(dependency.successor.clone()))
        .with("kind", CapabilityValue::String(dependency.kind.token().to_string()))
}

/// Rebuilds one [`Dependency`] from a row of [`DEPENDENCY_SCHEMA`].
fn dependency_from_record(
    record: &rust_widgets::widget::capability::RecordValue,
) -> Result<Dependency, CapabilityAccessError> {
    let text = |name: &str| match record.get(name) {
        Some(CapabilityValue::String(value)) => Ok(value.clone()),
        _ => Err(CapabilityAccessError::TypeMismatch),
    };
    let kind = match record.get("kind") {
        None | Some(CapabilityValue::Null) => DependencyKind::default(),
        Some(CapabilityValue::String(token)) => {
            DependencyKind::from_token(token).ok_or_else(|| {
                CapabilityAccessError::ConstraintViolation {
                    property: rust_widgets::compat::String::from("dependency_edges"),
                    reason: rust_widgets::compat::format!("unknown dependency kind: {token}"),
                }
            })?
        }
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    Ok(Dependency { predecessor: text("predecessor")?, successor: text("successor")?, kind })
}

/// A list of dependency edges that can be selected, added and removed.
///
/// Each edge occupies one row. Pressing a row reports it through
/// [`Self::link_selected`] when the editor is `editable`; the whole row is the hit
/// area, because a dependency drawn only as a one-pixel elbow is untappable on a
/// touch screen. A self-edge (`a -> a`) is refused: it can never be scheduled.
pub struct DependencyEditor {
    base: BaseWidget,
    dependencies: Vec<Dependency>,
    /// The row captions, rebuilt with the list.
    ///
    /// A caption is a formatting of an edge that cannot change while the edge cannot change, so
    /// it is built once instead of once per row per frame.
    labels: Vec<String>,
    editable: bool,
    selected: Option<usize>,
    /// Emitted when an edge is added.
    pub dependency_added: Signal1<Dependency>,
    /// Emitted when an edge is removed.
    pub dependency_removed: Signal1<Dependency>,
    /// Emitted with the edge a press selected.
    pub link_selected: Signal1<Dependency>,
}

impl DependencyEditor {
    /// Creates an empty, editable editor.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DependencyEditor, geometry, "DependencyEditor"),
            dependencies: Vec::new(),
            labels: Vec::new(),
            editable: true,
            selected: None,
            dependency_added: Signal1::new(),
            dependency_removed: Signal1::new(),
            link_selected: Signal1::new(),
        }
    }

    /// Returns the edges in display order.
    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }

    /// Replaces the whole list, dropping self-edges and duplicates.
    ///
    /// A bulk assignment is not an edit, so no `dependency_added` fires; the
    /// selection is cleared because its index may now name a different edge.
    pub fn set_dependencies(&mut self, dependencies: Vec<Dependency>) {
        let mut unique: Vec<Dependency> = Vec::new();
        for dependency in dependencies {
            if dependency.predecessor == dependency.successor {
                continue;
            }
            if unique.iter().any(|existing| {
                existing.predecessor == dependency.predecessor
                    && existing.successor == dependency.successor
            }) {
                continue;
            }
            unique.push(dependency);
        }
        self.dependencies = unique;
        self.selected = None;
        self.rebuild_labels();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Adds an edge. Returns `false` for a self-edge, or when an edge between the same two tasks
    /// already exists.
    ///
    /// The pair is the editor's edge identity, the same rule [`Self::set_dependencies`] and
    /// [`Self::remove_dependency`] use: the arrows the chart draws are pair-shaped, so a second edge
    /// for one pair would paint two arrows on top of each other. The existing edge keeps its kind.
    pub fn add_dependency(&mut self, dependency: Dependency) -> bool {
        if dependency.predecessor == dependency.successor {
            return false;
        }
        if self.dependencies.iter().any(|edge| {
            edge.predecessor == dependency.predecessor && edge.successor == dependency.successor
        }) {
            return false;
        }
        self.dependencies.push(dependency.clone());
        self.rebuild_labels();
        self.dependency_added.emit(dependency);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Removes the first edge whose endpoints match `dependency`, whatever its kind.
    ///
    /// Returns whether such an edge existed. The pair is the identity (see
    /// [`Self::add_dependency`]); the payload names the edge that was actually removed, so a caller
    /// asking to remove a start-to-start edge between two tasks is told when the finish-to-start one
    /// was the edge that existed.
    pub fn remove_dependency(&mut self, dependency: &Dependency) -> bool {
        let Some(index) = self.dependencies.iter().position(|edge| {
            edge.predecessor == dependency.predecessor && edge.successor == dependency.successor
        }) else {
            return false;
        };
        let removed = self.dependencies.remove(index);
        self.rebuild_labels();
        self.selected = match self.selected {
            Some(selected) if selected == index => None,
            Some(selected) if selected > index => Some(selected - 1),
            other => other,
        };
        self.dependency_removed.emit(removed);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Rebuilds the row captions from the current edge list.
    fn rebuild_labels(&mut self) {
        self.labels = self
            .dependencies
            .iter()
            .map(|dependency| {
                format!("{}  \u{2192}  {}", dependency.predecessor, dependency.successor)
            })
            .collect();
    }

    /// Whether presses select an edge.
    pub fn editable(&self) -> bool {
        self.editable
    }

    /// Enables or disables selection by press.
    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        self.base.request_redraw();
    }

    /// The row index under `point`, if any.
    pub fn hit_link(&self, point: Point) -> Option<usize> {
        (0..self.dependencies.len()).find(|index| {
            self.row_rect(*index).is_some_and(|row| {
                point.x >= row.x
                    && point.x < row.x + row.width as i32
                    && point.y >= row.y
                    && point.y < row.y + row.height as i32
            })
        })
    }

    /// The line an edge's arrow glyph is drawn along.
    ///
    /// Exposed so a host (and the hit test's tests) can place an overlay on the
    /// same geometry the control paints; `hit_link` deliberately uses the whole row.
    pub fn link_geometry(&self, index: usize) -> Option<(Point, Point)> {
        let row = self.row_rect(index)?;
        let y = row.y + row.height as i32 / 2;
        Some((
            Point::new(row.x + row.width as i32 - 44, y),
            Point::new(row.x + row.width as i32 - 10, y),
        ))
    }

    /// The rectangle one row occupies, or `None` when it does not fit.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.dependencies.len() {
            return None;
        }
        let rect = self.geometry();
        let top =
            rect.y + DEPENDENCY_PADDING + DEPENDENCY_HEADER + index as i32 * DEPENDENCY_ROW_HEIGHT;
        if top + DEPENDENCY_ROW_HEIGHT > rect.y + rect.height as i32 {
            return None;
        }
        Some(Rect::new(
            rect.x + DEPENDENCY_PADDING,
            top,
            rect.width.saturating_sub(2 * DEPENDENCY_PADDING as u32),
            DEPENDENCY_ROW_HEIGHT as u32,
        ))
    }
}

impl Widget for DependencyEditor {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("dependency_editor")
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> rust_widgets::core::Size {
        rust_widgets::core::Size::new(240, 160)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "dependency_added" => Some(EventSignalRef::mapped(
                "dependency_added",
                &self.dependency_added,
                dependency_value,
            )),
            "dependency_removed" => Some(EventSignalRef::mapped(
                "dependency_removed",
                &self.dependency_removed,
                dependency_value,
            )),
            "link_selected" => {
                Some(EventSignalRef::mapped("link_selected", &self.link_selected, dependency_value))
            }
            _ => None,
        }
    }
}

/// The published payload shape of a dependency event: `(predecessor, successor)`.
fn dependency_value(dependency: &Dependency) -> CapabilityValue {
    CapabilityValue::Tuple(alloc::vec![
        CapabilityValue::String(dependency.predecessor.clone()),
        CapabilityValue::String(dependency.successor.clone()),
    ])
}

impl WidgetProperties for DependencyEditor {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "dependencies" => Ok(CapabilityValue::UInt(self.dependencies.len() as u64)),
            // The whole edge list as records (SP-123). `dependencies` stays the count it has always
            // been; this is the read/write face that can say *which* edges exist.
            "dependency_edges" => Ok(CapabilityValue::Records(
                rust_widgets::widget::capability::RecordList::from_items(
                    &DEPENDENCY_SCHEMA,
                    self.dependencies.iter().map(dependency_record).collect(),
                ),
            )),
            "editable" => Ok(CapabilityValue::Bool(self.editable)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            // A whole-list replacement through `set_dependencies`, so the record face and the
            // concrete API cannot disagree about duplicates or self-edges.
            "dependency_edges" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&DEPENDENCY_SCHEMA).map_err(|violation| {
                    CapabilityAccessError::ConstraintViolation {
                        property: rust_widgets::compat::String::from("dependency_edges"),
                        reason: rust_widgets::compat::format!("{violation}"),
                    }
                })?;
                let dependencies = list
                    .items()
                    .iter()
                    .map(dependency_from_record)
                    .collect::<Result<Vec<_>, CapabilityAccessError>>()?;
                self.set_dependencies(dependencies);
                Ok(())
            }
            // A count cannot say *which* edges to keep; the list is replaced through
            // `set_dependencies`, which carries the identities.
            "dependencies" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["dependencies", "dependency_edges", "editable", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for DependencyEditor {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if !self.editable {
                return;
            }
            if let Some(index) = self.hit_link(*pos) {
                self.selected = Some(index);
                if let Some(dependency) = self.dependencies.get(index) {
                    self.link_selected.emit(dependency.clone());
                }
                self.base.request_redraw();
            }
        }
    }
}

impl Draw for DependencyEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let (surface, border, ink) = chart_chrome_palette(style, "dependency_editor");
        context.face_with_gradient(rect, surface, None, Default::default(), 0, Color::BLACK);
        let font = effective_font(style);
        context.draw_text_fitted_ellipsis(
            Rect::new(rect.x + DEPENDENCY_PADDING, rect.y + 2, rect.width.saturating_sub(16), 14),
            "Dependencies",
            font,
            ink,
            HorizontalAlignment::Left,
        );
        if self.dependencies.is_empty() {
            context.draw_text_fitted_ellipsis(
                Rect::new(
                    rect.x + DEPENDENCY_PADDING,
                    rect.y + DEPENDENCY_PADDING + DEPENDENCY_HEADER,
                    rect.width.saturating_sub(16),
                    16,
                ),
                "No dependencies",
                font,
                ink,
                HorizontalAlignment::Left,
            );
            return;
        }
        for index in 0..self.dependencies.len() {
            let Some(row) = self.row_rect(index) else {
                break;
            };
            if self.selected == Some(index) {
                context.draw_rect(row, ink);
            }
            let label = self.labels.get(index).map(String::as_str).unwrap_or("");
            context.draw_text_fitted_ellipsis(
                Rect::new(row.x + 4, row.y + 4, row.width.saturating_sub(52), 14),
                label,
                font,
                ink,
                HorizontalAlignment::Left,
            );
            if let Some((start, end)) = self.link_geometry(index) {
                context.draw_line(start, end, border);
                // Arrow head: two short strokes back from the tip.
                context.draw_line(end, Point::new(end.x - 4, end.y - 3), border);
                context.draw_line(end, Point::new(end.x - 4, end.y + 3), border);
            }
        }
    }
}
