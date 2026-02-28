//! Style system primitives.

use crate::core::{Color, Font};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeInsets {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

impl EdgeInsets {
    pub fn all(value: u32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    pub x: i32,
    pub y: i32,
    pub blur: u32,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetStyle {
    pub background_color: Option<Color>,
    pub text_color: Option<Color>,
    pub font: Option<Font>,
    pub border_color: Option<Color>,
    pub border_width: u32,
    pub border_radius: u32,
    pub padding: EdgeInsets,
    pub margin: EdgeInsets,
    pub shadow: Option<Shadow>,
}

impl Default for WidgetStyle {
    fn default() -> Self {
        Self {
            background_color: None,
            text_color: None,
            font: None,
            border_color: None,
            border_width: 0,
            border_radius: 0,
            padding: EdgeInsets::all(0),
            margin: EdgeInsets::all(0),
            shadow: None,
        }
    }
}
