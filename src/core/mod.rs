//! Core primitives and library-wide contracts.

use std::fmt::Debug;

pub type ObjectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfile {
    Full,
    Embedded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformFamily {
    Desktop,
    Embedded,
    Mobile,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Font {
    pub family: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Top,
    Bottom,
}

pub trait CoreObject: Debug + Send + Sync {
    fn id(&self) -> ObjectId;
    fn set_id(&mut self, id: ObjectId);
}
