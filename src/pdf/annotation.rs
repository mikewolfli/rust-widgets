// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::core::{Color, Point, Rect};
use std::collections::HashMap;
/// The `/Subtype` of a PDF annotation, listing the standard types defined by
/// the PDF specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnotationType {
    /// A sticky note attached to a point on the page.
    Text,
    /// A highlight spanning the text being marked up.
    Highlight,
    /// An underline spanning the text being marked up.
    Underline,
    /// A strike-through line drawn across the text being marked up.
    StrikeOut,
    /// A jagged line drawn under the text being marked up.
    Squiggly,
    /// A hyperlink to another location in the document or to an external target.
    Link,
    /// A popup window used to display or edit the contents of a parent markup
    /// annotation.
    Popup,
    /// A straight line, optionally with leaders and line-ending decorations.
    Line,
    /// A rectangle.
    Square,
    /// An ellipse.
    Circle,
    /// A closed polygon.
    Polygon,
    /// An open series of connected vertices.
    PolyLine,
    /// Freehand scribble strokes.
    Ink,
    /// A rubber stamp applied to the page.
    Stamp,
    /// A caret marking an insertion point in the text.
    Caret,
    /// An embedded file attached at a location on the page.
    FileAttachment,
    /// An embedded sound clip.
    Sound,
    /// An embedded movie clip.
    Movie,
    /// A form field widget, which is drawn by the annotation itself and is
    /// represented by [`crate::pdf::form::FormField`] in this crate.
    Widget,
    /// A screen annotation defining a region for use by media.
    Screen,
    /// A printer's mark such as a registration target or colour bar.
    PrinterMark,
    /// A trap network used by prepress workflows to detect misregistration.
    TrapNet,
    /// A watermark image or text to be drawn at a fixed size and position on
    /// the page, independent of the page's zoom or rotation.
    Watermark,
    /// Three-dimensional artwork embedded in the page.
    ThreeD,
}
/// A PDF annotation together with the metadata common to every annotation type.
///
/// Type-specific payloads (quad points, ink strokes, line endings) live on the
/// dedicated structs that embed this one as their `base` field.
#[derive(Debug, Clone)]
pub struct Annotation {
    /// Caller-assigned identifier, unique within a document. Used as the key when
    /// an annotation is stored in an [`AnnotationManager`].
    pub id: String,
    /// Zero-based index of the page this annotation is attached to.
    pub page: u32,
    /// The `/Subtype` of the annotation.
    pub annotation_type: AnnotationType,
    /// The annotation's bounding box, in page coordinates.
    pub rect: Rect,
    /// The annotation's text content, shown in the popup for markup types and
    /// used as the control's value for widget annotations.
    pub contents: String,
    /// Name of the person who created the annotation.
    pub author: String,
    /// Creation timestamp as a raw PDF date string (for example
    /// `D:20260101090000Z`). Stored verbatim rather than parsed.
    pub creation_date: String,
    /// Last-modified timestamp as a raw PDF date string. Stored verbatim rather
    /// than parsed.
    pub modification_date: String,
    /// Colour used to draw the annotation, or `None` to let the viewer choose a
    /// default (usually transparent for non-markup annotations).
    pub color: Option<Color>,
    /// Opacity in the range `0.0` (fully transparent) to `1.0` (fully opaque),
    /// stored as the PDF constant alpha. Clamped by [`Annotation::with_opacity`],
    /// but a value assigned directly is not.
    pub opacity: f32,
    /// Rendering and interaction flags.
    pub flags: AnnotationFlags,
    /// Arbitrary application-specific key/value pairs, serialized as the
    /// annotation's custom data rather than as standard PDF keys.
    pub custom_data: HashMap<String, String>,
}
impl Annotation {
    /// Creates an annotation of the given type on `page`, positioned at `rect`.
    ///
    /// The text fields are empty, `color` is `None`, `opacity` is `1.0`, no flags
    /// are set, and `custom_data` is empty.
    pub fn new(id: String, page: u32, annotation_type: AnnotationType, rect: Rect) -> Self {
        Self {
            id,
            page,
            annotation_type,
            rect,
            contents: String::new(),
            author: String::new(),
            creation_date: String::new(),
            modification_date: String::new(),
            color: None,
            opacity: 1.0,
            flags: AnnotationFlags::default(),
            custom_data: HashMap::new(),
        }
    }
    /// Builder-style setter for the annotation's text content.
    pub fn with_contents(mut self, contents: String) -> Self {
        self.contents = contents;
        self
    }
    /// Builder-style setter for the author name.
    pub fn with_author(mut self, author: String) -> Self {
        self.author = author;
        self
    }
    /// Builder-style setter marking the annotation to be drawn in `color`.
    ///
    /// Only one colour can be set; a later call replaces an earlier one.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
    /// Builder-style setter for the opacity, clamped to `0.0..=1.0`.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
    /// Whether the annotation is drawn at all: true unless the `hidden` or
    /// `invisible` flag is set.
    ///
    /// The `no_view` flag is deliberately not consulted, since it only means the
    /// annotation should not be shown on screen in contexts that suppress
    /// annotations entirely.
    pub fn is_visible(&self) -> bool {
        !self.flags.hidden && !self.flags.invisible
    }
    /// Whether the user may modify the annotation: true unless the `locked` or
    /// `read_only` flag is set.
    pub fn is_editable(&self) -> bool {
        !self.flags.locked && !self.flags.read_only
    }
}
/// The annotation `/F` flag bits, describing how an annotation is rendered and
/// whether the user may interact with it.
///
/// The flags are independent; the defaults deserialize every flag as clear,
/// except that [`Annotation::new`] itself starts with an all-clear flag set too.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnnotationFlags {
    /// Do not render the annotation, and do not allow it to be edited, even
    /// though its rectangle still occupies space on the page.
    pub invisible: bool,
    /// Do not render the annotation, and do not allow interaction with it, as if
    /// the annotation were not present at all.
    pub hidden: bool,
    /// Render the annotation when the page is printed. Without this flag the
    /// annotation appears on screen only.
    pub print: bool,
    /// Scale the annotation's appearance with the page rather than magnifying it
    /// with the page's zoom factor, so that the drawing keeps a constant size on
    /// screen.
    pub no_zoom: bool,
    /// Do not rotate the annotation's appearance with the page. Annotations that
    /// themselves rotate their contents (such as stamps with a `/Rotate` entry)
    /// are unaffected.
    pub no_rotate: bool,
    /// Hide the annotation unless the document is being printed. The user cannot
    /// interact with it while it is hidden in this way.
    pub no_view: bool,
    /// Do not allow the annotation's contents or appearance to be edited.
    pub read_only: bool,
    /// Do not allow the annotation to be deleted or moved.
    pub locked: bool,
    /// Toggle the [`AnnotationFlags::no_view`] flag instead of leaving it set,
    /// so that viewing the document flips the annotation between visible and
    /// hidden.
    pub toggle_no_view: bool,
    /// Do not allow the annotation's contents (`/Contents`) to be modified, even
    /// if the annotation is otherwise editable.
    pub locked_contents: bool,
}
/// A sticky-note annotation, drawn at the top-left corner of its base rectangle.
#[derive(Debug, Clone)]
pub struct TextAnnotation {
    /// Metadata and geometry shared with every annotation type.
    pub base: Annotation,
    /// The name of the icon the viewer draws for the note, from the standard
    /// PDF icon set.
    pub icon: TextIcon,
    /// Whether the note's popup window is open when the document is opened.
    pub open: bool,
}
/// The standard icon names a PDF viewer may use to draw a text annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextIcon {
    /// A note icon. The PDF default when no icon name is given.
    #[default]
    Note,
    /// A comment icon.
    Comment,
    /// A key icon, conventionally meaning a key point or an important point.
    Key,
    /// A question-mark icon, conventionally meaning help text.
    Help,
    /// A new-paragraph icon.
    NewParagraph,
    /// A paragraph icon (a pilcrow).
    Paragraph,
    /// An insertion-caret icon.
    Insert,
    /// A cross icon.
    Cross,
    /// A circle icon.
    Circle,
    /// A star icon.
    Star,
    /// A check-mark icon, conventionally meaning approved or done.
    Check,
    /// A right-pointing arrow icon.
    RightArrow,
    /// A right-pointing pointer (hand) icon.
    RightPointer,
    /// An up-pointing arrow icon.
    UpArrow,
    /// An up-and-left-pointing arrow icon.
    UpLeftArrow,
    /// A cross-hairs targeting icon.
    CrossHairs,
}
/// A text markup annotation — highlight, underline, strikeout or squiggly —
/// covering one or more quadrilaterals of text.
#[derive(Debug, Clone)]
pub struct HighlightAnnotation {
    /// Metadata and geometry shared with every annotation type.
    pub base: Annotation,
    /// The quadrilaterals, one per marked-up line fragment, in page coordinates.
    pub quad_points: Vec<QuadPoint>,
    /// Which markup effect the quadrilaterals represent.
    pub highlight_type: HighlightType,
}
/// The kind of text markup represented by a [`HighlightAnnotation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightType {
    /// The text is shown over a translucent colour.
    Highlight,
    /// A line is drawn under the text.
    Underline,
    /// A wavy line is drawn under the text.
    Squiggly,
    /// A line is drawn through the middle of the text.
    StrikeOut,
}
/// One quadrilateral of a [`HighlightAnnotation`], expressed as four corner
/// points so that a marked-up line fragment can be rotated or slanted.
///
/// Coordinates are in page space. The points are conventionally numbered
/// counter-clockwise starting from the top-left of the quadrilateral, but
/// nothing in this crate enforces that order.
#[derive(Debug, Clone, Copy)]
pub struct QuadPoint {
    /// X of the first corner.
    pub x1: f32,
    /// Y of the first corner.
    pub y1: f32,
    /// X of the second corner.
    pub x2: f32,
    /// Y of the second corner.
    pub y2: f32,
    /// X of the third corner.
    pub x3: f32,
    /// Y of the third corner.
    pub y3: f32,
    /// X of the fourth corner.
    pub x4: f32,
    /// Y of the fourth corner.
    pub y4: f32,
}
/// A freehand scribble annotation.
#[derive(Debug, Clone)]
pub struct InkAnnotation {
    /// Metadata and geometry shared with every annotation type.
    pub base: Annotation,
    /// The strokes making up the drawing: one entry per stroke, each entry a
    /// series of points in page coordinates. Points within a stroke are joined
    /// by straight line segments.
    pub ink_list: Vec<Vec<Point>>,
}
/// A line annotation with optional leader lines and decorative line endings.
#[derive(Debug, Clone)]
pub struct LineAnnotation {
    /// Metadata and geometry shared with every annotation type.
    pub base: Annotation,
    /// The start point of the line, in page coordinates.
    pub start: Point,
    /// The end point of the line, in page coordinates.
    pub end: Point,
    /// Decoration drawn at the start point.
    pub start_style: LineEndingStyle,
    /// Decoration drawn at the end point.
    pub end_style: LineEndingStyle,
    /// Fill colour for line endings that enclose an area, such as
    /// [`LineEndingStyle::Circle`]. `None` leaves them unfilled.
    pub interior_color: Option<Color>,
    /// Length of the leader line in page units. Only drawn when the annotation
    /// is used as a callout with [`LineAnnotation::caption`] set.
    pub leader_line_length: f32,
    /// Amount by which the leader line is extended beyond the end point, in page
    /// units. Used to stop short of, or overshoot, the text being called out.
    pub leader_line_extension: f32,
    /// Whether the annotation is drawn as a callout with a leader line.
    pub caption: bool,
    /// Offset of the caption text from the line's end point, in page units, as
    /// `(x, y)`.
    pub caption_offset: (f32, f32),
}
/// The decoration drawn at one end of a [`LineAnnotation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEndingStyle {
    /// No decoration; the line simply ends.
    #[default]
    None,
    /// A filled square centred on the endpoint.
    Square,
    /// A filled circle centred on the endpoint.
    Circle,
    /// A filled diamond centred on the endpoint.
    Diamond,
    /// A V-shaped arrowhead drawn as two open strokes.
    OpenArrow,
    /// A filled triangular arrowhead pointing along the line.
    ClosedArrow,
    /// A short perpendicular bar across the line, as used to mark a dimension.
    Butt,
    /// An open arrowhead pointing back towards the line's interior, so that the
    /// decoration sits on the outside of the endpoint.
    ReverseOpenArrow,
    /// A filled arrowhead pointing back towards the line's interior, so that the
    /// decoration sits on the outside of the endpoint.
    ReverseClosedArrow,
    /// A single short diagonal stroke across the endpoint.
    Slash,
}
/// An in-memory store of the annotations of a document, indexed both by
/// annotation id and by page.
///
/// The page index keeps the ids in insertion order, so the annotations returned
/// for a page preserve the order in which they were added.
pub struct AnnotationManager {
    /// All annotations, keyed by [`Annotation::id`].
    annotations: HashMap<String, Annotation>,
    /// Ids of the annotations on each page, in insertion order.
    page_annotations: HashMap<u32, Vec<String>>,
}
impl AnnotationManager {
    /// Creates an empty manager with no annotations and no pages.
    pub fn new() -> Self {
        Self { annotations: HashMap::new(), page_annotations: HashMap::new() }
    }
    /// Stores `annotation` under its id and appends it to its page's index.
    ///
    /// Adding a second annotation with an id that is already stored replaces the
    /// existing entry, but the page index then holds that id twice, so the
    /// replaced annotation still counts towards [`AnnotationManager::annotation_count`]
    /// only once while [`AnnotationManager::get_page_annotations`] returns it
    /// twice. Callers are expected to use unique ids.
    pub fn add_annotation(&mut self, annotation: Annotation) {
        let id = annotation.id.clone();
        let page = annotation.page;
        self.annotations.insert(id.clone(), annotation);
        self.page_annotations.entry(page).or_default().push(id);
    }
    /// Removes and returns the annotation with the given id, also dropping it
    /// from its page's index. Returns `None` if no such annotation is stored.
    pub fn remove_annotation(&mut self, id: &str) -> Option<Annotation> {
        if let Some(annotation) = self.annotations.remove(id) {
            if let Some(page_annotations) = self.page_annotations.get_mut(&annotation.page) {
                page_annotations.retain(|a| a != id);
            }
            Some(annotation)
        } else {
            None
        }
    }
    /// Looks up the annotation with the given id.
    pub fn get_annotation(&self, id: &str) -> Option<&Annotation> {
        self.annotations.get(id)
    }
    /// Looks up the annotation with the given id for modification.
    pub fn get_annotation_mut(&mut self, id: &str) -> Option<&mut Annotation> {
        self.annotations.get_mut(id)
    }
    /// Returns the page's annotations in the order they were added, or an empty
    /// vector if the page has none.
    pub fn get_page_annotations(&self, page: u32) -> Vec<&Annotation> {
        self.page_annotations
            .get(&page)
            .map(|ids| ids.iter().filter_map(|id| self.annotations.get(id)).collect())
            .unwrap_or_default()
    }
    /// Returns the page's annotations whose bounding box intersects `rect`, in
    /// insertion order. Hit testing uses the annotation's full rectangle, so a
    /// markup annotation spanning several quads is matched by any overlap.
    pub fn get_annotations_in_rect(&self, page: u32, rect: &Rect) -> Vec<&Annotation> {
        self.get_page_annotations(page).into_iter().filter(|a| a.rect.intersects(rect)).collect()
    }
    /// Removes every annotation and every page index.
    pub fn clear(&mut self) {
        self.annotations.clear();
        self.page_annotations.clear();
    }
    /// Total number of stored annotations.
    pub fn annotation_count(&self) -> usize {
        self.annotations.len()
    }
    /// Number of distinct pages that have at least one annotation. Pages that
    /// have been emptied by [`AnnotationManager::remove_annotation`] still count,
    /// because the page index keeps its now-empty entry.
    pub fn page_count(&self) -> usize {
        self.page_annotations.len()
    }
}
crate::impl_default_via_new!(AnnotationManager);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_annotation_creation() {
        let annotation = Annotation::new(
            "test-1".to_string(),
            1,
            AnnotationType::Text,
            Rect::new(100, 100, 200, 50),
        )
        .with_contents("Test annotation".to_string())
        .with_author("Test User".to_string())
        .with_color(Color::YELLOW);
        assert_eq!(annotation.id, "test-1");
        assert_eq!(annotation.page, 1);
        assert_eq!(annotation.annotation_type, AnnotationType::Text);
        assert_eq!(annotation.contents, "Test annotation");
        assert!(annotation.color.is_some());
    }
    #[test]
    fn test_annotation_manager() {
        let mut manager = AnnotationManager::new();
        let annotation = Annotation::new(
            "test-1".to_string(),
            1,
            AnnotationType::Highlight,
            Rect::new(100, 100, 200, 50),
        );
        manager.add_annotation(annotation);
        assert_eq!(manager.annotation_count(), 1);
        assert!(manager.get_annotation("test-1").is_some());
        let page_annotations = manager.get_page_annotations(1);
        assert_eq!(page_annotations.len(), 1);
    }
}
