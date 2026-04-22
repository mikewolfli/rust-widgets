use crate::core::{Color, Point, Rect};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnotationType {
    Text,
    Highlight,
    Underline,
    StrikeOut,
    Squiggly,
    Link,
    Popup,
    Line,
    Square,
    Circle,
    Polygon,
    PolyLine,
    Ink,
    Stamp,
    Caret,
    FileAttachment,
    Sound,
    Movie,
    Widget,
    Screen,
    PrinterMark,
    TrapNet,
    Watermark,
    ThreeD,
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub id: String,
    pub page: u32,
    pub annotation_type: AnnotationType,
    pub rect: Rect,
    pub contents: String,
    pub author: String,
    pub creation_date: String,
    pub modification_date: String,
    pub color: Option<Color>,
    pub opacity: f32,
    pub flags: AnnotationFlags,
    pub custom_data: HashMap<String, String>,
}

impl Annotation {
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

    pub fn with_contents(mut self, contents: String) -> Self {
        self.contents = contents;
        self
    }

    pub fn with_author(mut self, author: String) -> Self {
        self.author = author;
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn is_visible(&self) -> bool {
        !self.flags.hidden && !self.flags.invisible
    }

    pub fn is_editable(&self) -> bool {
        !self.flags.locked && !self.flags.read_only
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AnnotationFlags {
    pub invisible: bool,
    pub hidden: bool,
    pub print: bool,
    pub no_zoom: bool,
    pub no_rotate: bool,
    pub no_view: bool,
    pub read_only: bool,
    pub locked: bool,
    pub toggle_no_view: bool,
    pub locked_contents: bool,
}

#[derive(Debug, Clone)]
pub struct TextAnnotation {
    pub base: Annotation,
    pub icon: TextIcon,
    pub open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextIcon {
    Note,
    Comment,
    Key,
    Help,
    NewParagraph,
    Paragraph,
    Insert,
    Cross,
    Circle,
    Star,
    Check,
    RightArrow,
    RightPointer,
    UpArrow,
    UpLeftArrow,
    CrossHairs,
}

impl Default for TextIcon {
    fn default() -> Self {
        Self::Note
    }
}

#[derive(Debug, Clone)]
pub struct HighlightAnnotation {
    pub base: Annotation,
    pub quad_points: Vec<QuadPoint>,
    pub highlight_type: HighlightType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightType {
    Highlight,
    Underline,
    Squiggly,
    StrikeOut,
}

#[derive(Debug, Clone, Copy)]
pub struct QuadPoint {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub x3: f32,
    pub y3: f32,
    pub x4: f32,
    pub y4: f32,
}

#[derive(Debug, Clone)]
pub struct InkAnnotation {
    pub base: Annotation,
    pub ink_list: Vec<Vec<Point>>,
}

#[derive(Debug, Clone)]
pub struct LineAnnotation {
    pub base: Annotation,
    pub start: Point,
    pub end: Point,
    pub start_style: LineEndingStyle,
    pub end_style: LineEndingStyle,
    pub interior_color: Option<Color>,
    pub leader_line_length: f32,
    pub leader_line_extension: f32,
    pub caption: bool,
    pub caption_offset: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEndingStyle {
    None,
    Square,
    Circle,
    Diamond,
    OpenArrow,
    ClosedArrow,
    Butt,
    ReverseOpenArrow,
    ReverseClosedArrow,
    Slash,
}

impl Default for LineEndingStyle {
    fn default() -> Self {
        Self::None
    }
}

pub struct AnnotationManager {
    annotations: HashMap<String, Annotation>,
    page_annotations: HashMap<u32, Vec<String>>,
}

impl AnnotationManager {
    pub fn new() -> Self {
        Self {
            annotations: HashMap::new(),
            page_annotations: HashMap::new(),
        }
    }

    pub fn add_annotation(&mut self, annotation: Annotation) {
        let id = annotation.id.clone();
        let page = annotation.page;

        self.annotations.insert(id.clone(), annotation);
        self.page_annotations.entry(page).or_default().push(id);
    }

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

    pub fn get_annotation(&self, id: &str) -> Option<&Annotation> {
        self.annotations.get(id)
    }

    pub fn get_annotation_mut(&mut self, id: &str) -> Option<&mut Annotation> {
        self.annotations.get_mut(id)
    }

    pub fn get_page_annotations(&self, page: u32) -> Vec<&Annotation> {
        self.page_annotations
            .get(&page)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.annotations.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_annotations_in_rect(&self, page: u32, rect: &Rect) -> Vec<&Annotation> {
        self.get_page_annotations(page)
            .into_iter()
            .filter(|a| a.rect.intersects(rect))
            .collect()
    }

    pub fn clear(&mut self) {
        self.annotations.clear();
        self.page_annotations.clear();
    }

    pub fn annotation_count(&self) -> usize {
        self.annotations.len()
    }

    pub fn page_count(&self) -> usize {
        self.page_annotations.len()
    }
}

impl Default for AnnotationManager {
    fn default() -> Self {
        Self::new()
    }
}

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
