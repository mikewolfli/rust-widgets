// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! RiveWidget — a Rive animation runtime widget.
//!
//! # Capability boundary
//!
//! **This control decodes a documented subset of the binary `.riv` format** and renders a
//! host-provided shape document. [`RiveWidget::load_animation`] reads a `.riv` file path (or use
//! [`RiveWidget::load_animation_from_riv`] with bytes already in memory); the decoder parses the
//! Rive file header, property table and object stream, and exposes the artboard names, animation
//! names/durations and state-machine names. It does not build a full Rive scene graph: artboard
//! drawables are host-provided, so a host that wants pixels flattens the artboard to the
//! Rive-inspired JSON this widget renders through [`RiveWidget::load_animation_from_json`].
//!
//! The decoder is hand-rolled (no `riv` dependency and no Cargo feature): it validates the magic,
//! version, property table and each object chunk, and reports a specific error for a malformed or
//! truncated file instead of silently fabricating an animation.

//!
//! # State machine input contract
//!
//! The inputs are a small documented convention layered on the parsed JSON, not a real Rive state
//! machine:
//!
//! - a numeric input named `"progress"` pins the playhead to that value, clamped
//!   to `0.0..=1.0`, so a host can scrub the animation;
//! - a boolean input named `"playing"` set to `false` pauses advancement for as
//!   long as it stays `false`;
//! - a trigger input is fired with [`RiveWidget::fire_input`], which reports the edge through
//!   `input_fired` and maps the lifecycle names `play`, `pause`, `stop` and `reset` onto the
//!   corresponding control method.
//!
//! Other names are stored and reported but do not affect playback.

use crate::compat::HashMap;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};

/// The value type for a Rive state machine input.
#[derive(Debug, Clone, PartialEq)]
pub enum RiveInputValue {
    /// Boolean input.
    Bool(bool),
    /// Numeric (float) input.
    Number(f32),
    /// Trigger input (one-shot).
    Trigger,
}

/// A named input for a Rive state machine.
#[derive(Debug, Clone)]
pub struct RiveInput {
    /// The name of the input.
    pub name: String,
    /// The current value of the input.
    pub value: RiveInputValue,
}

/// The lifecycle state this control's lightweight state machine is in.
///
/// Rive's own state graphs are not decoded, so the state is the animation's own lifecycle. It is a
/// real, observable state rather than a copy of the inputs: every transition emits
/// [`RiveWidget::state_changed`] with the state name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RiveState {
    /// No animation is loaded.
    #[default]
    Idle,
    /// The animation is advancing.
    Playing,
    /// The animation is loaded and paused.
    Paused,
    /// A finite loop count was exhausted.
    Completed,
}

impl RiveState {
    /// The published spelling of this state.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Completed => "completed",
        }
    }
}

// ──────────────────────────────────────────────
// Rive animation shape model
// ──────────────────────────────────────────────

/// Type of a Rive animated shape.
#[derive(Debug, Clone, Copy, PartialEq)]
enum RiveShapeType {
    Rectangle,
    Circle,
    Ellipse,
    Triangle,
    Star,
}

/// An animated shape in a Rive animation.
/// Each shape has keyframed properties that interpolate based on progress.
#[derive(Debug, Clone)]
struct RiveAnimatedShape {
    /// The shape type.
    shape_type: RiveShapeType,
    /// Base x position (center).
    x: f64,
    /// Base y position (center).
    y: f64,
    /// Base width.
    width: f64,
    /// Base height.
    height: f64,
    /// Base color (rgba as u8 values).
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    /// Motion amplitude for animation — how far the shape moves from base.
    motion_x: f64,
    motion_y: f64,
    /// Scale animation amplitude (0 = no scaling).
    scale_amplitude: f64,
    /// Rotation amplitude in degrees (0 = no rotation).
    rotation_amplitude: f64,
    /// Color shift — whether color changes with progress.
    color_shift: bool,
}

impl RiveAnimatedShape {
    /// Compute the interpolated position for a given progress value (0.0 - 1.0).
    fn position_at(&self, progress: f64) -> (f64, f64) {
        let angle = progress * std::f64::consts::TAU;
        let nx = self.x + self.motion_x * angle.sin();
        let ny = self.y + self.motion_y * angle.cos();
        (nx, ny)
    }

    /// Compute the interpolated size (width, height) for a given progress.
    fn size_at(&self, progress: f64) -> (f64, f64) {
        if self.scale_amplitude > 0.0 {
            let scale = 1.0 + self.scale_amplitude * (progress * std::f64::consts::TAU).sin();
            (self.width * scale, self.height * scale)
        } else {
            (self.width, self.height)
        }
    }

    /// Compute the color for a given progress.
    fn color_at(&self, progress: f64) -> Color {
        if self.color_shift {
            let hue_shift = (progress * 360.0) as f32;
            let r = (self.r as f32 + hue_shift * 0.3).round().clamp(0.0, 255.0) as u8;
            let g = (self.g as f32 + hue_shift * 0.2).round().clamp(0.0, 255.0) as u8;
            let b = (self.b as f32 + hue_shift * 0.1).round().clamp(0.0, 255.0) as u8;
            Color::rgba(r, g, b, self.a)
        } else {
            Color::rgba(self.r, self.g, self.b, self.a)
        }
    }

    /// Compute rotation in degrees for a given progress.
    fn rotation_at(&self, progress: f64) -> f64 {
        if self.rotation_amplitude > 0.0 {
            self.rotation_amplitude * (progress * std::f64::consts::TAU).sin()
        } else {
            0.0
        }
    }
}

/// The animation data parsed from a Rive-like JSON structure.
#[derive(Debug, Clone)]
struct RiveAnimationData {
    /// Name of the animation.
    _name: String,
    /// Duration of the animation in frames.
    duration_frames: u32,
    /// Frame rate declared by the document.
    frame_rate: f32,
    /// Shapes in this animation.
    shapes: Vec<RiveAnimatedShape>,
}

impl RiveAnimationData {
    /// Parse from a JSON value that describes the animation.
    fn from_json(name: &str, val: &serde_json::Value) -> Self {
        let duration_frames = val.get("duration").and_then(|v| v.as_u64()).unwrap_or(60) as u32;
        let frame_rate = val.get("fr").and_then(|v| v.as_f64()).unwrap_or(60.0) as f32;

        let shapes = if let Some(shapes_arr) = val.get("shapes").and_then(|v| v.as_array()) {
            shapes_arr
                .iter()
                .map(|s| {
                    let shape_type_str = s.get("ty").and_then(|v| v.as_str()).unwrap_or("rect");
                    let shape_type = match shape_type_str {
                        "circle" => RiveShapeType::Circle,
                        "ellipse" => RiveShapeType::Ellipse,
                        "triangle" => RiveShapeType::Triangle,
                        "star" => RiveShapeType::Star,
                        _ => RiveShapeType::Rectangle,
                    };
                    let x = s.get("x").and_then(|v| v.as_f64()).unwrap_or(50.0);
                    let y = s.get("y").and_then(|v| v.as_f64()).unwrap_or(50.0);
                    let width = s.get("w").and_then(|v| v.as_f64()).unwrap_or(40.0);
                    let height = s.get("h").and_then(|v| v.as_f64()).unwrap_or(40.0);
                    let r = s.get("r").and_then(|v| v.as_u64()).unwrap_or(80) as u8;
                    let g = s.get("g").and_then(|v| v.as_u64()).unwrap_or(60) as u8;
                    let b = s.get("b").and_then(|v| v.as_u64()).unwrap_or(180) as u8;
                    let a = s.get("a").and_then(|v| v.as_u64()).unwrap_or(220) as u8;
                    let motion_x = s.get("mx").and_then(|v| v.as_f64()).unwrap_or(10.0);
                    let motion_y = s.get("my").and_then(|v| v.as_f64()).unwrap_or(5.0);
                    let scale_amp = s.get("sa").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let rot_amp = s.get("ra").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let color_shift = s.get("cs").and_then(|v| v.as_bool()).unwrap_or(false);
                    RiveAnimatedShape {
                        shape_type,
                        x,
                        y,
                        width,
                        height,
                        r,
                        g,
                        b,
                        a,
                        motion_x,
                        motion_y,
                        scale_amplitude: scale_amp,
                        rotation_amplitude: rot_amp,
                        color_shift,
                    }
                })
                .collect()
        } else {
            // Default shapes if none specified: a bouncing rect and a pulsing circle.
            vec![
                RiveAnimatedShape {
                    shape_type: RiveShapeType::Rectangle,
                    x: 50.0,
                    y: 50.0,
                    width: 60.0,
                    height: 60.0,
                    r: 60,
                    g: 100,
                    b: 200,
                    a: 220,
                    motion_x: 20.0,
                    motion_y: 15.0,
                    scale_amplitude: 0.2,
                    rotation_amplitude: 15.0,
                    color_shift: false,
                },
                RiveAnimatedShape {
                    shape_type: RiveShapeType::Circle,
                    x: 50.0,
                    y: 50.0,
                    width: 30.0,
                    height: 30.0,
                    r: 200,
                    g: 80,
                    b: 60,
                    a: 200,
                    motion_x: -15.0,
                    motion_y: 10.0,
                    scale_amplitude: 0.3,
                    rotation_amplitude: 0.0,
                    color_shift: true,
                },
            ]
        };

        Self { _name: name.to_string(), duration_frames, frame_rate, shapes }
    }
}

// ──────────────────────────────────────────────
// Binary `.riv` decoder (hand-rolled subset)
// ──────────────────────────────────────────────

/// One animation decoded from a Rive file's object stream.
#[derive(Debug, Clone, PartialEq)]
pub struct RiveAnimationInfo {
    /// Animation name as the file declares it.
    pub name: String,
    /// Duration in animation frames.
    pub duration_frames: u32,
    /// Authoring frame rate. Falls back to 60 when the file omits it.
    pub frame_rate: f32,
}

/// The names and durations decoded from a `.riv` file header and objects.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RiveFileInfo {
    /// Major format version from the file header.
    pub major_version: u32,
    /// Minor format version from the file header.
    pub minor_version: u32,
    /// Artboard names, in file order.
    pub artboards: Vec<String>,
    /// Animations with their declared duration and frame rate.
    pub animations: Vec<RiveAnimationInfo>,
    /// State-machine names, in file order.
    pub state_machines: Vec<String>,
}

/// A bounds-checked reader over a `.riv` byte slice.
struct RivReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> RivReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn eof(&self) -> bool {
        self.position >= self.bytes.len()
    }

    fn error(&self, message: impl core::fmt::Display) -> String {
        format!("malformed Rive file at byte {}: {message}", self.position)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self.position.checked_add(length).ok_or_else(|| self.error("length overflow"))?;
        if end > self.bytes.len() {
            return Err(self.error(format!(
                "wanted {length} byte(s), only {} remain",
                self.bytes.len() - self.position
            )));
        }
        let slice = &self.bytes[self.position..end];
        self.position = end;
        Ok(slice)
    }

    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    /// Unsigned LEB128, the encoding every Rive integer uses.
    fn var_uint(&mut self) -> Result<u64, String> {
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = self.byte()?;
            if shift >= 64 || (shift == 63 && (byte & 0x7e) != 0) {
                return Err(self.error("integer varint exceeds 64 bits"));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
    }

    fn var_u32(&mut self) -> Result<u32, String> {
        let value = self.var_uint()?;
        u32::try_from(value).map_err(|_| self.error(format!("value {value} exceeds u32")))
    }

    fn u32_le(&mut self) -> Result<u32, String> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// A length-prefixed byte run. String and bytes properties share this framing, so the decoder
    /// reads the bytes and defers the UTF-8 decision to the caller that actually wants text.
    fn bytes(&mut self) -> Result<&'a [u8], String> {
        let length = usize::try_from(self.var_uint()?)
            .map_err(|_| self.error("byte length does not fit in usize"))?;
        self.take(length)
    }
}

/// The value of one object property, kept only as far as the name extraction needs it.
///
/// `Float` and `Color` are read and skipped so the stream stays aligned; this subset extracts no
/// value of those kinds, hence the allow.
#[allow(dead_code)]
enum RivPropertyValue {
    Uint(u64),
    Bytes(Vec<u8>),
    Float(f32),
    Color(u32),
}

impl RivPropertyValue {
    fn as_string(&self) -> Option<&str> {
        match self {
            Self::Bytes(bytes) => core::str::from_utf8(bytes).ok(),
            _ => None,
        }
    }

    fn as_u32(&self) -> Option<u32> {
        match self {
            Self::Uint(value) => u32::try_from(*value).ok(),
            _ => None,
        }
    }
}

const RIV_MAGIC: &[u8; 4] = b"RIVE";
/// The newest format version this decoder understands. Rive 7.x is the current runtime format;
/// older files are accepted because the object/property framing is the same.
const RIV_MAX_MAJOR: u32 = 7;

// Object and property keys used to extract names and durations. These are the stable generated
// keys from the Rive runtime (Artboard = 1, Animation = 27, LinearAnimation = 31, StateMachine =
// 53; name = 4/55/138, fps = 56, duration = 57).
const RIV_ARTBOARD_TYPE: u32 = 1;
const RIV_ANIMATION_TYPE: u32 = 27;
const RIV_LINEAR_ANIMATION_TYPE: u32 = 31;
const RIV_STATE_MACHINE_TYPE: u32 = 53;
const RIV_COMPONENT_NAME_KEY: u32 = 4;
const RIV_ANIMATION_NAME_KEY: u32 = 55;
const RIV_ANIMATION_FPS_KEY: u32 = 56;
const RIV_ANIMATION_DURATION_KEY: u32 = 57;
const RIV_STATE_MACHINE_NAME_KEY: u32 = 138;

/// Physical encoding of every property key in the Rive runtime's generated core registry.
///
/// `0` = unsigned LEB128, `1` = length-prefixed bytes, `2` = little-endian `f32`, `3` =
/// little-endian packed colour, `4` = one bool byte. The file header's ToC covers property keys the
/// writing runtime considered non-core; the keys below are the ones the runtime resolves through
/// its own registry, which a file commonly omits from the ToC. Both are consulted, so skipping a
/// property never desynchronises the stream.
///
/// Generated from `rive-app/rive-cpp`'s `include/rive/generated/**/*_base.hpp` property keys and
/// `core_registry.hpp`'s `propertyFieldId` grouping.
const RIV_PROPERTY_KINDS: &[(u32, u8)] = &[
    (4, 1),
    (5, 0),
    (7, 2),
    (8, 2),
    (9, 2),
    (10, 2),
    (11, 2),
    (12, 2),
    (13, 2),
    (14, 2),
    (15, 2),
    (16, 2),
    (17, 2),
    (18, 2),
    (20, 2),
    (21, 2),
    (23, 0),
    (24, 2),
    (25, 2),
    (26, 2),
    (31, 2),
    (32, 4),
    (33, 2),
    (34, 2),
    (35, 2),
    (37, 3),
    (38, 3),
    (39, 2),
    (40, 0),
    (41, 4),
    (42, 2),
    (46, 2),
    (47, 2),
    (48, 0),
    (49, 0),
    (50, 4),
    (51, 0),
    (53, 0),
    (55, 1),
    (56, 0),
    (57, 0),
    (58, 2),
    (59, 0),
    (60, 0),
    (61, 0),
    (62, 4),
    (63, 2),
    (64, 2),
    (65, 2),
    (66, 2),
    (67, 0),
    (68, 0),
    (69, 0),
    (70, 2),
    (79, 2),
    (80, 2),
    (81, 2),
    (82, 2),
    (83, 2),
    (84, 2),
    (85, 2),
    (86, 2),
    (87, 2),
    (88, 3),
    (89, 2),
    (90, 2),
    (91, 2),
    (92, 0),
    (93, 0),
    (94, 4),
    (95, 0),
    (96, 2),
    (97, 2),
    (98, 2),
    (99, 2),
    (100, 2),
    (101, 2),
    (102, 0),
    (103, 0),
    (104, 2),
    (105, 2),
    (106, 2),
    (107, 2),
    (108, 2),
    (109, 2),
    (110, 0),
    (111, 0),
    (112, 0),
    (113, 0),
    (114, 2),
    (115, 2),
    (116, 2),
    (117, 0),
    (118, 0),
    (119, 0),
    (120, 0),
    (121, 0),
    (122, 0),
    (123, 2),
    (124, 2),
    (125, 0),
    (126, 2),
    (127, 2),
    (128, 0),
    (129, 0),
    (136, 0),
    (138, 1),
    (140, 2),
    (141, 4),
    (149, 0),
    (151, 0),
    (152, 0),
    (155, 0),
    (156, 0),
    (157, 2),
    (158, 0),
    (160, 0),
    (161, 2),
    (162, 2),
    (163, 2),
    (164, 4),
    (165, 0),
    (166, 2),
    (167, 0),
    (168, 0),
    (171, 0),
    (172, 2),
    (173, 0),
    (174, 4),
    (175, 0),
    (177, 2),
    (178, 0),
    (179, 0),
    (180, 0),
    (181, 4),
    (182, 2),
    (183, 2),
    (184, 2),
    (185, 2),
    (186, 2),
    (187, 2),
    (188, 4),
    (189, 4),
    (190, 4),
    (191, 4),
    (192, 4),
    (193, 4),
    (194, 4),
    (195, 0),
    (196, 4),
    (197, 0),
    (198, 0),
    (199, 2),
    (200, 2),
    (201, 4),
    (202, 2),
    (203, 1),
    (204, 0),
    (206, 0),
    (207, 2),
    (208, 2),
    (210, 0),
    (212, 1),
    (215, 2),
    (216, 2),
    (218, 0),
    (221, 0),
    (222, 0),
    (223, 1),
    (224, 0),
    (225, 0),
    (227, 0),
    (228, 0),
    (229, 2),
    (236, 0),
    (237, 0),
    (238, 4),
    (239, 2),
    (240, 0),
    (243, 2),
    (245, 4),
    (246, 1),
    (248, 1),
    (249, 0),
    (268, 1),
    (269, 0),
    (270, 0),
    (271, 0),
    (272, 0),
    (273, 0),
    (274, 2),
    (279, 0),
    (280, 1),
    (281, 0),
    (284, 0),
    (285, 2),
    (286, 2),
    (287, 0),
    (288, 2),
    (289, 0),
    (292, 2),
    (296, 0),
    (297, 2),
    (298, 0),
    (299, 2),
    (300, 2),
    (301, 0),
    (302, 0),
    (303, 2),
    (304, 2),
    (305, 2),
    (306, 2),
    (307, 2),
    (308, 2),
    (312, 0),
    (313, 0),
    (315, 4),
    (316, 0),
    (317, 2),
    (318, 2),
    (319, 2),
    (320, 0),
    (321, 2),
    (322, 2),
    (323, 2),
    (324, 2),
    (325, 0),
    (326, 0),
    (327, 2),
    (328, 2),
    (329, 2),
    (330, 2),
    (331, 2),
    (332, 2),
    (333, 4),
    (334, 2),
    (335, 0),
    (336, 2),
    (337, 2),
    (338, 2),
    (339, 2),
    (340, 2),
    (349, 0),
    (350, 0),
    (356, 0),
    (357, 0),
    (359, 1),
    (362, 1),
    (363, 2),
    (364, 4),
    (365, 4),
    (366, 2),
    (367, 2),
    (368, 2),
    (369, 2),
    (370, 2),
    (371, 2),
    (372, 2),
    (373, 2),
    (376, 4),
    (377, 0),
    (378, 0),
    (380, 2),
    (381, 2),
    (389, 0),
    (390, 2),
    (392, 0),
    (393, 0),
    (399, 0),
    (400, 0),
    (405, 0),
    (406, 2),
    (407, 2),
    (408, 0),
    (410, 2),
    (411, 2),
    (412, 0),
    (414, 0),
    (415, 0),
    (416, 2),
    (417, 2),
    (418, 0),
    (419, 4),
    (420, 4),
    (446, 0),
    (447, 3),
    (448, 2),
    (449, 0),
    (450, 0),
    (451, 4),
    (452, 0),
    (453, 0),
    (454, 0),
    (455, 0),
    (456, 4),
    (457, 4),
    (458, 2),
    (459, 0),
    (460, 0),
    (461, 4),
    (462, 2),
    (463, 2),
    (464, 2),
    (465, 2),
    (466, 0),
    (467, 4),
    (468, 0),
    (470, 0),
    (481, 1),
    (482, 1),
    (483, 2),
    (485, 1),
    (486, 1),
    (487, 0),
    (489, 0),
    (490, 0),
    (493, 0),
    (494, 0),
    (498, 2),
    (499, 2),
    (500, 2),
    (501, 2),
    (502, 2),
    (503, 2),
    (504, 2),
    (505, 2),
    (506, 2),
    (507, 2),
    (508, 2),
    (509, 2),
    (510, 2),
    (511, 2),
    (512, 2),
    (513, 2),
    (514, 2),
    (515, 2),
    (516, 2),
    (517, 2),
    (518, 2),
    (519, 2),
    (523, 2),
    (524, 2),
    (530, 2),
    (536, 0),
    (537, 0),
    (538, 0),
    (541, 4),
    (549, 0),
    (550, 0),
    (554, 0),
    (555, 3),
    (557, 1),
    (560, 0),
    (561, 1),
    (565, 0),
    (566, 0),
    (572, 1),
    (574, 0),
    (575, 2),
    (577, 0),
    (578, 1),
    (579, 1),
    (582, 1),
    (583, 0),
    (586, 0),
    (587, 0),
    (588, 1),
    (589, 0),
    (590, 0),
    (591, 0),
    (592, 2),
    (593, 4),
    (596, 0),
    (597, 0),
    (598, 0),
    (599, 0),
    (604, 0),
    (605, 0),
    (606, 4),
    (607, 0),
    (608, 0),
    (609, 0),
    (610, 0),
    (611, 0),
    (612, 0),
    (613, 0),
    (614, 0),
    (615, 0),
    (616, 0),
    (617, 0),
    (618, 0),
    (619, 0),
    (620, 0),
    (621, 0),
    (622, 0),
    (623, 0),
    (624, 0),
    (625, 0),
    (626, 0),
    (627, 0),
    (628, 0),
    (629, 0),
    (630, 0),
    (631, 0),
    (632, 0),
    (634, 4),
    (635, 1),
    (636, 2),
    (637, 0),
    (638, 3),
    (639, 4),
    (640, 2),
    (641, 2),
    (642, 2),
    (643, 2),
    (644, 2),
    (645, 2),
    (647, 4),
    (650, 0),
    (651, 3),
    (652, 2),
    (653, 0),
    (654, 1),
    (655, 0),
    (656, 0),
    (660, 0),
    (662, 1),
    (663, 2),
    (664, 2),
    (665, 0),
    (666, 0),
    (667, 0),
    (668, 0),
    (669, 0),
    (672, 0),
    (673, 0),
    (675, 2),
    (676, 4),
    (677, 0),
    (679, 0),
    (681, 2),
    (682, 0),
    (683, 0),
    (685, 0),
    (686, 0),
    (687, 0),
    (689, 0),
    (690, 2),
    (691, 4),
    (692, 2),
    (693, 4),
    (697, 2),
    (698, 2),
    (699, 2),
    (700, 2),
    (703, 4),
    (705, 0),
    (706, 2),
    (707, 2),
    (708, 0),
    (709, 0),
    (711, 1),
    (713, 0),
    (714, 0),
    (715, 0),
    (716, 2),
    (717, 2),
    (718, 2),
    (719, 2),
    (722, 0),
    (724, 4),
    (725, 0),
    (726, 0),
    (727, 0),
    (728, 2),
    (729, 2),
    (730, 2),
    (731, 0),
    (734, 4),
    (743, 0),
    (744, 1),
    (745, 0),
    (746, 0),
    (747, 0),
    (748, 0),
    (749, 2),
    (750, 2),
    (751, 2),
    (752, 4),
    (756, 2),
    (757, 0),
    (758, 0),
    (759, 2),
    (760, 2),
    (761, 2),
    (762, 2),
    (763, 2),
    (764, 0),
    (765, 0),
    (766, 1),
    (770, 4),
    (775, 0),
    (776, 0),
    (777, 2),
    (778, 0),
    (779, 4),
    (781, 2),
    (782, 4),
    (783, 2),
    (784, 2),
    (785, 2),
    (786, 2),
    (800, 0),
    (806, 2),
    (807, 2),
    (808, 2),
    (809, 2),
    (810, 2),
    (811, 2),
    (814, 0),
    (816, 0),
    (817, 1),
    (818, 2),
    (823, 0),
    (824, 0),
    (835, 0),
    (836, 3),
    (846, 0),
    (848, 0),
    (850, 4),
    (851, 4),
    (858, 0),
    (859, 2),
    (860, 2),
    (862, 0),
    (864, 2),
    (865, 2),
    (866, 1),
    (868, 1),
    (870, 0),
    (871, 1),
    (872, 0),
    (873, 0),
    (874, 0),
    (875, 0),
    (876, 0),
    (887, 0),
    (888, 2),
    (889, 2),
    (891, 4),
    (892, 0),
    (893, 0),
    (894, 2),
    (895, 4),
    (907, 2),
    (908, 2),
    (911, 1),
    (912, 0),
    (914, 4),
    (920, 1),
    (921, 4),
    (922, 0),
    (926, 1),
    (930, 0),
    (931, 0),
    (932, 0),
    (934, 0),
    (935, 0),
    (952, 0),
    (953, 4),
    (954, 4),
    (955, 4),
    (956, 0),
    (957, 0),
    (962, 0),
    (963, 1),
    (965, 0),
    (966, 0),
    (971, 0),
    (972, 0),
    (973, 0),
    (974, 0),
    (975, 2),
    (976, 2),
    (977, 0),
    (978, 0),
    (979, 4),
    (980, 0),
    (981, 0),
    (982, 0),
    (983, 1),
    (984, 1),
    (985, 1),
    (986, 0),
    (987, 0),
    (988, 0),
    (989, 4),
    (990, 4),
    (991, 4),
    (992, 4),
    (993, 4),
    (994, 4),
    (995, 4),
    (996, 4),
    (997, 4),
    (998, 0),
    (1000, 4),
    (1001, 4),
    (1002, 4),
    (1003, 4),
    (1004, 4),
    (1005, 4),
    (1006, 4),
    (1007, 4),
    (1008, 4),
    (1009, 4),
    (1010, 0),
    (1011, 0),
    (1014, 4),
    (1015, 0),
    (1018, 0),
    (1019, 0),
    (1020, 0),
    (1021, 0),
    (1022, 0),
    (1023, 2),
    (1024, 2),
    (1025, 4),
    (1026, 0),
    (1027, 0),
    (1028, 0),
    (1029, 2),
    (1033, 0),
    (1040, 2),
    (1041, 2),
    (1045, 0),
    (1046, 0),
    (1047, 0),
    (1048, 0),
    (1049, 0),
    (1050, 0),
    (1057, 2),
    (1058, 2),
    (1059, 0),
    (1061, 0),
    (1062, 0),
    (1063, 2),
    (1064, 0),
    (1065, 2),
    (1066, 2),
    (1067, 2),
    (1068, 0),
    (1069, 2),
    (1070, 2),
    (1071, 2),
    (1073, 0),
    (1074, 0),
    (1075, 0),
    (1076, 0),
    (1077, 0),
    (1078, 0),
    (1087, 0),
    (1094, 0),
    (1095, 4),
    (1098, 4),
    (1099, 4),
    (1103, 2),
    (1104, 2),
    (1105, 2),
    (1106, 2),
    (1107, 2),
];

/// The physical kind of `property_key`: the file's own ToC first, then the runtime registry.
fn riv_property_kind(property_key: u32, toc: &HashMap<u32, u8>) -> Option<u8> {
    toc.get(&property_key).copied().or_else(|| {
        RIV_PROPERTY_KINDS
            .binary_search_by_key(&property_key, |(key, _)| *key)
            .ok()
            .map(|index| RIV_PROPERTY_KINDS[index].1)
    })
}

/// Parses the Rive file header and object/property chunks.
///
/// The header is `"RIVE"`, major/minor/file-id varints, a zero-terminated list of property keys,
/// then the property **kind** table bit-packed two bits per key in little-endian `u32`s. Objects
/// follow: a type key varint, then `(property key, value)` pairs ended by key zero. Every property
/// is skipped by the kind the header declared, so the stream stays aligned even for objects and
/// properties this subset does not model.
fn decode_riv(bytes: &[u8]) -> Result<RiveFileInfo, String> {
    if bytes.len() < RIV_MAGIC.len() || &bytes[..RIV_MAGIC.len()] != RIV_MAGIC {
        return Err("not a Rive file: expected the 4-byte \"RIVE\" magic at the start".to_string());
    }
    let mut reader = RivReader::new(bytes);
    reader.position = RIV_MAGIC.len();
    let major_version = reader.var_u32()?;
    let minor_version = reader.var_u32()?;
    if major_version == 0 || major_version > RIV_MAX_MAJOR {
        return Err(format!(
            "unsupported Rive version {major_version}.{minor_version}; this decoder supports 1..={RIV_MAX_MAJOR}"
        ));
    }
    let _file_id = reader.var_uint()?;

    let mut property_keys = Vec::new();
    loop {
        let key = reader.var_u32()?;
        if key == 0 {
            break;
        }
        property_keys.push(key);
        if property_keys.len() > 1_000_000 {
            return Err(reader.error("property table is unreasonably large"));
        }
    }

    // Two bits per property key: 0 = uint, 1 = length-prefixed string, 2 = f32, 3 = packed colour.
    let mut property_kinds = HashMap::new();
    let mut current = 0u32;
    let mut bit = 8u32;
    for key in property_keys {
        if bit == 8 {
            current = reader.u32_le()?;
            bit = 0;
        }
        property_kinds.insert(key, ((current >> bit) & 3) as u8);
        bit += 2;
    }

    let mut info = RiveFileInfo {
        major_version,
        minor_version,
        artboards: Vec::new(),
        animations: Vec::new(),
        state_machines: Vec::new(),
    };

    while !reader.eof() {
        let type_key = reader.var_u32()?;
        let mut name: Option<String> = None;
        let mut fps: Option<u32> = None;
        let mut duration: Option<u32> = None;

        loop {
            let property_key = reader.var_u32()?;
            if property_key == 0 {
                break;
            }
            let kind = riv_property_kind(property_key, &property_kinds).ok_or_else(|| {
                reader
                    .error(format!("unknown property key {property_key} on object type {type_key}"))
            })?;
            let value = match kind {
                0 => RivPropertyValue::Uint(reader.var_uint()?),
                1 => RivPropertyValue::Bytes(reader.bytes()?.to_vec()),
                2 => {
                    let bytes = reader.take(4)?;
                    RivPropertyValue::Float(f32::from_le_bytes([
                        bytes[0], bytes[1], bytes[2], bytes[3],
                    ]))
                }
                3 => RivPropertyValue::Color(reader.u32_le()?),
                4 => {
                    // A bool occupies one byte; none of the names/durations extracted here use it.
                    let _ = reader.byte()?;
                    continue;
                }
                other => {
                    return Err(
                        reader.error(format!("property {property_key} has unknown kind {other}"))
                    );
                }
            };

            if (type_key == RIV_ARTBOARD_TYPE && property_key == RIV_COMPONENT_NAME_KEY)
                || ((type_key == RIV_ANIMATION_TYPE || type_key == RIV_LINEAR_ANIMATION_TYPE)
                    && property_key == RIV_ANIMATION_NAME_KEY)
                || (type_key == RIV_STATE_MACHINE_TYPE
                    && (property_key == RIV_STATE_MACHINE_NAME_KEY
                        || property_key == RIV_ANIMATION_NAME_KEY))
            {
                name = value.as_string().map(str::to_string);
            } else if type_key == RIV_LINEAR_ANIMATION_TYPE && property_key == RIV_ANIMATION_FPS_KEY
            {
                fps = value.as_u32();
            } else if type_key == RIV_LINEAR_ANIMATION_TYPE
                && property_key == RIV_ANIMATION_DURATION_KEY
            {
                duration = value.as_u32();
            }
        }

        match type_key {
            RIV_ARTBOARD_TYPE => {
                if let Some(name) = name {
                    info.artboards.push(name);
                }
            }
            RIV_ANIMATION_TYPE | RIV_LINEAR_ANIMATION_TYPE => {
                let name = name.unwrap_or_default();
                if !name.is_empty() {
                    info.animations.push(RiveAnimationInfo {
                        name,
                        duration_frames: duration.unwrap_or(0),
                        frame_rate: fps
                            .filter(|fps| *fps > 0)
                            .map(|fps| fps as f32)
                            .unwrap_or(60.0),
                    });
                }
            }
            RIV_STATE_MACHINE_TYPE => {
                if let Some(name) = name.filter(|name| !name.is_empty()) {
                    info.state_machines.push(name);
                }
            }
            _ => {}
        }
    }

    if info.artboards.is_empty() && info.animations.is_empty() {
        return Err(
            "malformed Rive file: the object stream contains no artboard or animation".to_string()
        );
    }
    Ok(info)
}

/// RiveWidget — a Rive animation runtime widget.
pub struct RiveWidget {
    base: BaseWidget,
    /// Name of the currently loaded animation.
    animation_name: String,
    /// Whether the animation is currently playing.
    is_playing: bool,
    /// State machine inputs.
    state_machine_inputs: Vec<RiveInput>,
    /// Loop count: 0 = infinite, >0 = number of repetitions.
    loop_count: i32,
    /// Internal timer accumulator in milliseconds.
    frame_timer: u64,
    /// Internal animation progress (0.0 – 1.0).
    animation_progress: f32,
    /// Emitted when the animation finishes (all loops completed).
    pub animation_finished: GenericSignal,
    /// Emitted with the new state name whenever the lifecycle state changes.
    pub state_changed: Signal1<String>,
    /// Emitted with the input name when a Trigger input is fired.
    pub input_fired: Signal1<String>,
    /// The lifecycle state of the lightweight state machine.
    state: RiveState,
    /// Completed repetitions, kept separately from the configured loop count.
    loops_completed: i32,
    /// Parsed animation data with shapes to render.
    ///
    /// The raw JSON source is deliberately **not** kept: parsing happens once, here, and
    /// everything the renderer needs survives in this value. An earlier revision also stored the
    /// original string in an `animation_json` field that nothing ever read — a second copy of the
    /// source whose only possible future was to drift from the parsed form (principle #99).
    animation_data: Option<RiveAnimationData>,
    /// Names and durations decoded from a `.riv` file, when the current animation came from one.
    file_info: Option<RiveFileInfo>,
    /// Composition extent (half-size in document units), cached until the animation changes.
    comp_extent: Option<f64>,
    /// The `Rive: <name>` caption, rebuilt when the animation changes.
    name_text: String,
    /// The animation name `name_text` was built from.
    name_source: String,
    /// The progress caption, rebuilt when the integer percentage changes.
    progress_text: String,
    /// The integer percentage `progress_text` was built from.
    progress_key: u32,
    /// The state-input count caption.
    input_text: String,
    /// The input count `input_text` was built from.
    input_text_key: usize,
}

impl RiveWidget {
    /// Creates a new RiveWidget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RiveWidget, geometry, "RiveWidget"),
            animation_name: String::new(),
            is_playing: false,
            state_machine_inputs: Vec::new(),
            loop_count: 0,
            frame_timer: 0,
            animation_progress: 0.0,
            animation_finished: GenericSignal::new(),
            state_changed: Signal1::new(),
            input_fired: Signal1::new(),
            state: RiveState::Idle,
            loops_completed: 0,
            animation_data: None,
            file_info: None,
            comp_extent: None,
            name_text: String::new(),
            name_source: String::new(),
            progress_text: "0%".to_string(),
            progress_key: u32::MAX,
            input_text: String::new(),
            input_text_key: usize::MAX,
        }
    }

    /// Loads a named animation from a `.riv` file path.
    ///
    /// The file is read from disk and decoded by [`Self::load_animation_from_riv`]. A path that
    /// cannot be read (or a caller with no filesystem, under the `mini` profile) is an explicit
    /// error rather than a fabricated animation.
    #[allow(clippy::needless_return)]
    pub fn load_animation(&mut self, name: &str) -> Result<(), String> {
        #[cfg(not(feature = "mini"))]
        {
            let bytes = std::fs::read(name).map_err(|error| {
                format!(
                    "Rive animation \"{name}\" could not be read as a .riv file: {error}; \
                     use load_animation_from_riv for an in-memory file"
                )
            })?;
            return self.load_animation_from_riv(name, &bytes);
        }
        #[cfg(feature = "mini")]
        {
            let _ = name;
            Err("this build has no filesystem; pass .riv bytes to load_animation_from_riv"
                .to_string())
        }
    }

    /// Loads an animation from the bytes of a `.riv` file, with no filesystem access.
    ///
    /// The decoder parses the Rive header, property table and object stream, and makes the
    /// artboard, animation and state-machine names available through [`Self::artboard_names`],
    /// [`Self::animation_names`] and [`Self::state_machine_names`]. Rendering stays host-provided:
    /// a `.riv` file's artboard is not converted to drawable shapes here.
    pub fn load_animation_from_riv(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let info = decode_riv(bytes)?;
        let display_name = if name.is_empty() {
            info.animations
                .first()
                .map(|animation| animation.name.clone())
                .or_else(|| info.artboards.first().cloned())
                .unwrap_or_default()
        } else {
            name.to_string()
        };
        let animation = info.animations.first();
        self.animation_name = display_name.clone();
        self.animation_progress = 0.0;
        self.frame_timer = 0;
        self.is_playing = false;
        self.loops_completed = 0;
        self.animation_data = Some(RiveAnimationData {
            _name: display_name,
            duration_frames: animation.map(|a| a.duration_frames).unwrap_or(60).max(1),
            frame_rate: animation.map(|a| a.frame_rate).unwrap_or(60.0),
            shapes: Vec::new(),
        });
        self.comp_extent = None;
        self.file_info = Some(info);
        self.set_state(RiveState::Paused);
        self.base.request_redraw();
        Ok(())
    }

    /// Alias for [`Self::load_animation_from_riv`], for callers that spell the input as bytes.
    pub fn load_animation_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.load_animation_from_riv(name, bytes)
    }

    /// The header and object names decoded from the current `.riv` file, if any.
    pub fn file_info(&self) -> Option<&RiveFileInfo> {
        self.file_info.as_ref()
    }

    /// The animation names in the current `.riv` file, or the JSON-loaded animation's name.
    pub fn animation_names(&self) -> Vec<String> {
        if let Some(info) = &self.file_info {
            if !info.animations.is_empty() {
                return info.animations.iter().map(|animation| animation.name.clone()).collect();
            }
        }
        if self.animation_name.is_empty() {
            Vec::new()
        } else {
            vec![self.animation_name.clone()]
        }
    }

    /// Selects one of the animations decoded from the current `.riv` file by name.
    ///
    /// Returns `false` when the name is not in the file (including when no file is loaded), so a
    /// caller can tell an accepted switch from a silent no-op. A successful switch restarts the
    /// playhead at 0 and pauses, which is what a host switching clips expects.
    pub fn set_animation(&mut self, name: &str) -> bool {
        let Some(animation) = self
            .file_info
            .as_ref()
            .and_then(|info| info.animations.iter().find(|animation| animation.name == name))
            .cloned()
        else {
            return false;
        };
        self.animation_name = animation.name.clone();
        self.animation_progress = 0.0;
        self.frame_timer = 0;
        self.is_playing = false;
        self.loops_completed = 0;
        self.comp_extent = None;
        self.animation_data = Some(RiveAnimationData {
            _name: self.animation_name.clone(),
            duration_frames: animation.duration_frames.max(1),
            frame_rate: animation.frame_rate,
            shapes: Vec::new(),
        });
        self.set_state(RiveState::Paused);
        self.base.request_redraw();
        true
    }

    /// The current animation's duration in milliseconds, from its frame count and rate.
    pub fn duration_ms(&self) -> u64 {
        let (frames, rate) = match self.animation_data.as_ref() {
            Some(data) => (data.duration_frames.max(1), data.frame_rate),
            None => return 0,
        };
        let rate = if rate.is_finite() && rate > 0.0 { rate as f64 } else { 60.0 };
        ((frames as f64 / rate) * 1000.0).round().max(0.0) as u64
    }

    /// The composition's half-extent in document units, cached until the shapes change.
    ///
    /// A default composition (or one with no shapes) is 100x100, so its half-extent is 50.
    /// The cache is invalidated by every loader and by [`Self::set_animation`].
    fn composition_extent(&mut self) -> f64 {
        if let Some(extent) = self.comp_extent {
            return extent;
        }
        let extent = match self.animation_data.as_ref() {
            Some(data) if !data.shapes.is_empty() => data
                .shapes
                .iter()
                .map(|shape| {
                    let mx = (shape.x + shape.motion_x).abs().max((shape.x - shape.motion_x).abs())
                        + shape.width.abs()
                        + 20.0;
                    let my = (shape.y + shape.motion_y).abs().max((shape.y - shape.motion_y).abs())
                        + shape.height.abs()
                        + 20.0;
                    mx.max(my)
                })
                .fold(100.0_f64, |a, b| a.max(b)),
            _ => 50.0,
        };
        self.comp_extent = Some(extent);
        extent
    }

    /// Refreshes the static captions when their inputs changed (L-P03).
    fn refresh_labels(&mut self) {
        if self.name_source != self.animation_name {
            self.name_source = self.animation_name.clone();
            self.name_text.clear();
            use core::fmt::Write as _;
            let _ = write!(self.name_text, "Rive: {}", self.animation_name);
        }
        let percent = (self.animation_progress * 100.0).round() as u32;
        if self.progress_key != percent {
            self.progress_key = percent;
            self.progress_text.clear();
            use core::fmt::Write as _;
            let _ = write!(self.progress_text, "{percent}%");
        }
        let inputs = self.state_machine_inputs.len();
        if self.input_text_key != inputs {
            self.input_text_key = inputs;
            self.input_text.clear();
            use core::fmt::Write as _;
            let _ = write!(self.input_text, "Inputs: {inputs}");
        }
    }

    /// The artboard names in the current `.riv` file.
    pub fn artboard_names(&self) -> Vec<String> {
        self.file_info.as_ref().map(|info| info.artboards.clone()).unwrap_or_default()
    }

    /// Loads a named animation from JSON data.
    /// The JSON should contain shape definitions and animation parameters.
    pub fn load_animation_from_json(&mut self, name: &str, json_data: &str) -> Result<(), String> {
        if json_data.is_empty() {
            return Err(format!(
                "Rive animation \"{name}\" has empty JSON data ({} bytes); pass the contents \
                 of a .json animation file",
                json_data.len()
            ));
        }
        let parsed: serde_json::Value = serde_json::from_str(json_data)
            .map_err(|e| format!("Invalid Rive animation JSON: {e}"))?;

        self.animation_name = name.to_string();
        self.animation_progress = 0.0;
        self.frame_timer = 0;
        self.is_playing = false;
        self.animation_data = Some(RiveAnimationData::from_json(name, &parsed));
        self.comp_extent = None;
        self.file_info = None;
        self.loops_completed = 0;
        self.set_state(RiveState::Paused);
        self.base.request_redraw();
        Ok(())
    }

    /// Returns the name of the currently loaded animation.
    pub fn animation_name(&self) -> &str {
        &self.animation_name
    }

    /// Starts playback of the animation.
    ///
    /// Does nothing until animation data has actually been loaded through
    /// [`Self::load_animation_from_json`]: a name alone is not a loaded
    /// animation.
    pub fn play(&mut self) {
        if self.animation_name.is_empty() || self.animation_data.is_none() {
            return;
        }
        self.is_playing = true;
        self.set_state(RiveState::Playing);
        self.base.request_redraw();
    }

    /// Pauses playback, keeping the current animation state.
    pub fn pause(&mut self) {
        self.is_playing = false;
        if self.animation_data.is_some() {
            self.set_state(RiveState::Paused);
        }
        self.base.request_redraw();
    }

    /// Stops playback and resets the animation to the beginning.
    pub fn stop(&mut self) {
        self.is_playing = false;
        self.animation_progress = 0.0;
        self.frame_timer = 0;
        self.set_state(if self.animation_data.is_some() {
            RiveState::Paused
        } else {
            RiveState::Idle
        });
        self.base.request_redraw();
    }

    /// The lifecycle state of this control's lightweight state machine.
    pub fn state(&self) -> RiveState {
        self.state
    }

    /// Changes the state, emitting `state_changed` only on a real transition.
    fn set_state(&mut self, state: RiveState) {
        if self.state == state {
            return;
        }
        self.state = state;
        self.state_changed.emit(state.as_str().to_string());
    }

    /// The state-machine names the current source exposes.
    ///
    /// A `.riv` file's state machines come from its decoded object stream. The Rive-inspired JSON
    /// path has no state-machine definitions, so its addressable input names are returned instead;
    /// [`Self::input_names`] is the input-only view for either source.
    pub fn state_machine_names(&self) -> Vec<String> {
        if let Some(info) = &self.file_info {
            if !info.state_machines.is_empty() {
                return info.state_machines.clone();
            }
        }
        self.input_names()
    }

    /// The names a host can address with [`Self::set_input`] / [`Self::fire_input`].
    pub fn input_names(&self) -> Vec<String> {
        self.state_machine_inputs.iter().map(|input| input.name.clone()).collect()
    }

    /// Fires the Trigger input named `name`.
    ///
    /// Returns `false` when the input does not exist or is not a Trigger: a boolean or number
    /// input has a value, not an edge, so "firing" it would have no defined meaning. A fired
    /// Trigger is announced through `input_fired`, and the lifecycle names `play`, `pause`,
    /// `stop` and `reset` also drive the matching control method.
    pub fn fire_input(&mut self, name: &str) -> bool {
        let Some(input) = self.state_machine_inputs.iter().find(|input| input.name == name) else {
            return false;
        };
        if !matches!(&input.value, RiveInputValue::Trigger) {
            return false;
        }
        self.input_fired.emit(name.to_string());
        match name {
            "play" => self.play(),
            "pause" => self.pause(),
            "stop" | "reset" => self.stop(),
            _ => {}
        }
        true
    }

    /// Returns whether the animation is currently playing.
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Sets the value of a named state machine input.
    /// If the input does not exist, it is created.
    ///
    /// The names `"progress"` (numeric) and `"playing"` (boolean) carry the
    /// documented conventions described on [`Self::tick`].
    pub fn set_input(&mut self, name: &str, value: RiveInputValue) {
        if let Some(input) = self.state_machine_inputs.iter_mut().find(|i| i.name == name) {
            input.value = value;
        } else {
            self.state_machine_inputs.push(RiveInput { name: name.to_string(), value });
        }
        // Keep the documented `"progress"` convention observable immediately: setting the
        // input scrubs the playhead even before the next tick.
        let pinned_progress =
            self.state_machine_inputs.iter().find(|input| input.name == "progress").and_then(
                |input| match &input.value {
                    RiveInputValue::Number(value) => Some(*value),
                    _ => None,
                },
            );
        if let Some(value) = pinned_progress {
            self.animation_progress = value.clamp(0.0, 1.0);
        }
        self.base.request_redraw();
    }

    /// Returns a reference to a named state machine input, if it exists.
    pub fn get_input(&self, name: &str) -> Option<&RiveInput> {
        self.state_machine_inputs.iter().find(|i| i.name == name)
    }

    /// Returns a mutable reference to a named state machine input, if it exists.
    pub fn get_input_mut(&mut self, name: &str) -> Option<&mut RiveInput> {
        self.state_machine_inputs.iter_mut().find(|i| i.name == name)
    }

    /// Removes a named state machine input.
    pub fn remove_input(&mut self, name: &str) {
        self.state_machine_inputs.retain(|i| i.name != name);
        self.base.request_redraw();
    }

    /// Returns a reference to all state machine inputs.
    pub fn inputs(&self) -> &[RiveInput] {
        &self.state_machine_inputs
    }

    /// Sets the loop count. 0 = infinite, >0 = number of repetitions.
    pub fn set_loop_count(&mut self, n: i32) {
        self.loop_count = n.max(0);
    }

    /// Returns the current loop count.
    pub fn loop_count(&self) -> i32 {
        self.loop_count
    }

    /// The number of finite loops completed so far.
    pub fn loops_completed(&self) -> i32 {
        self.loops_completed
    }

    /// Returns the current animation progress (0.0 – 1.0).
    pub fn animation_progress(&self) -> f32 {
        self.animation_progress
    }

    /// Advances the animation state by the given number of milliseconds using the frame rate and
    /// duration the document declared.
    ///
    /// A fixed 60 fps used to be assumed, so an animation authored at 24 or 30 fps played fast and
    /// one with a non-60-frame duration finished at the wrong time. The declared `fr` and
    /// `duration` drive the step now; a document with no usable frame rate falls back to 60.
    ///
    /// Returns true if the animation completed a finite loop.
    ///
    /// # State machine inputs
    ///
    /// Two input names are consumed, following the "Rive-inspired JSON" contract
    /// documented on the module:
    ///
    /// - a numeric `"progress"` input pins the playhead to that value (clamped to
    ///   `0.0..=1.0`) and suppresses advancement for the call;
    /// - a boolean `"playing"` input set to `false` gates advancement until it is
    ///   set back to `true` or removed.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        let pinned_progress =
            self.state_machine_inputs.iter().find(|input| input.name == "progress").and_then(
                |input| match &input.value {
                    RiveInputValue::Number(value) => Some(*value),
                    _ => None,
                },
            );
        if let Some(value) = pinned_progress {
            self.animation_progress = value.clamp(0.0, 1.0);
            self.base.request_redraw();
            return false;
        }

        let gated_playback = self
            .state_machine_inputs
            .iter()
            .find(|input| input.name == "playing")
            .is_some_and(|input| input.value == RiveInputValue::Bool(false));
        if gated_playback || !self.is_playing || self.animation_name.is_empty() {
            return false;
        }

        let (declared_rate, duration_frames) = match self.animation_data.as_ref() {
            Some(data) => (data.frame_rate, data.duration_frames),
            None => (60.0, 60),
        };
        let fps = if declared_rate.is_finite() && declared_rate > 0.0 {
            declared_rate as f64
        } else {
            60.0
        };
        // Microsecond period, like the sibling Lottie control. `(1000 / fps).floor()` lost the
        // fractional millisecond on every frame, so a 24 fps clip ran measurably slow; the
        // remainder is carried in `frame_timer` instead (L-P08). The period is floored rather
        // than rounded: rounding up made an exact one-second tick at 24 fps consume only 23
        // frames, which is the same under-advance in a different sign.
        let frame_delay_us = (1_000_000.0 / fps).floor().max(1.0) as u64;
        let step = 1.0 / duration_frames.max(1) as f32;
        // `saturating_add`: `frame_timer` is a `u64` fed by a public `tick(delta_ms)`. A plain `+=`
        // could overflow and wrap to a *small* value, and the `while` below would then run for the
        // wrapped remainder — potentially billions of iterations.
        self.frame_timer = self.frame_timer.saturating_add(delta_ms.saturating_mul(1000));

        // How many whole frames elapsed. Computed once rather than by decrementing in the loop, so
        // the work is proportional to the frames actually consumed (bounded below) instead of to
        // whatever magnitude the timer reached.
        let elapsed_frames = self.frame_timer / frame_delay_us;
        self.frame_timer %= frame_delay_us;

        let mut completed = false;
        // A single `tick` cannot meaningfully advance more than this many frames; a huge `delta_ms`
        // is clamped so one call cannot spin for an unbounded time. Any excess is simply dropped
        // (the animation is caught up, not replayed frame by frame).
        const MAX_FRAMES_PER_TICK: u64 = 600;
        let frames_to_advance = elapsed_frames.min(MAX_FRAMES_PER_TICK);
        for _ in 0..frames_to_advance {
            self.animation_progress += step;

            // The tolerance absorbs the rounding of repeated `1 / duration_frames` additions:
            // after exactly `duration_frames` steps the f32 sum can sit a few ulps below 1.0,
            // which made a one-second tick at 60 fps stop one frame short.
            if self.animation_progress >= 1.0 - 1.0e-4 {
                if self.loop_count == 0 {
                    // Infinite looping: wrap around.
                    self.animation_progress = 0.0;
                } else {
                    self.loops_completed += 1;
                    if self.loops_completed >= self.loop_count {
                        // All loops completed, stop. The configured `loop_count` is untouched.
                        self.is_playing = false;
                        self.animation_progress = 1.0;
                        self.set_state(RiveState::Completed);
                        self.animation_finished.emit();
                        completed = true;
                        break;
                    }
                    self.animation_progress = 0.0;
                }
            }
        }

        self.base.request_redraw();
        completed
    }
}

fn draw_shape(
    context: &mut RenderContext,
    shape_type: RiveShapeType,
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    color: Color,
    rotation_deg: f64,
) {
    if w <= 1.0 || h <= 1.0 {
        return;
    }
    let iw = w.round() as u32;
    let ih = h.round() as u32;
    let has_rotation = rotation_deg.abs() > 0.5;

    match shape_type {
        RiveShapeType::Rectangle => {
            let ex = (cx - w / 2.0).round() as i32;
            let ey = (cy - h / 2.0).round() as i32;
            let shape_rect = Rect::new(ex, ey, iw, ih);
            if has_rotation {
                // Draw with a subtle visual hint of rotation using corner radius.
                let corner = (iw.min(ih) / 6).max(1);
                context.fill_rounded_rect(shape_rect, corner, color);
            } else {
                context.fill_rect(shape_rect, color);
            }
        }
        RiveShapeType::Circle => {
            let radius = (iw.min(ih) / 2).max(1);
            context.fill_circle(Point::new(cx.round() as i32, cy.round() as i32), radius, color);
        }
        RiveShapeType::Ellipse => {
            let radius = (iw.min(ih) / 2).max(1);
            let ex = (cx - w / 2.0).round() as i32;
            let ey = (cy - h / 2.0).round() as i32;
            let shape_rect = Rect::new(ex, ey, iw, ih);
            context.fill_rounded_rect(shape_rect, radius, color);
        }
        RiveShapeType::Triangle => {
            // Draw triangle as a filled shape approximated via the rasterizer.
            // We use a centered rounded rect as a stand-in for triangle rendering
            // since the renderer doesn't have native triangle support.
            let ex = (cx - w / 2.0).round() as i32;
            let ey = (cy - h / 2.0).round() as i32;
            let shape_rect = Rect::new(ex, ey, iw, ih);
            context.fill_rect(shape_rect, color);
            // Draw diagonal hints to suggest a triangle shape.
            let p1 = Point::new(ex, ey + ih as i32);
            let p2 = Point::new(ex + iw as i32, ey);
            context.draw_line_stroke(p1, p2, color, 2);
        }
        RiveShapeType::Star => {
            // Draw a star approximated by overlapping circles and a central rect.
            let radius = (iw.min(ih) / 3).max(1);
            let cx_i32 = cx.round() as i32;
            let cy_i32 = cy.round() as i32;
            context.fill_circle(Point::new(cx_i32, cy_i32), radius, color);
            context.fill_circle(Point::new(cx_i32 - radius as i32, cy_i32), radius / 2, color);
            context.fill_circle(Point::new(cx_i32 + radius as i32, cy_i32), radius / 2, color);
            context.fill_circle(Point::new(cx_i32, cy_i32 - radius as i32), radius / 2, color);
            context.fill_circle(Point::new(cx_i32, cy_i32 + radius as i32), radius / 2, color);
        }
    }
}

impl Widget for RiveWidget {
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
        crate::core::Size::new(300, 300)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `RiveWidget` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    // Trait contract: `u32` ms. This control's own `tick` takes `u64`; widen at the boundary.
    fn tick(&mut self, delta_ms: u32) -> bool {
        RiveWidget::tick(self, u64::from(delta_ms))
    }

    fn is_animating(&self) -> bool {
        self.is_playing()
    }
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "animation_finished" => {
                Some(EventSignalRef::unit("animation_finished", &self.animation_finished))
            }
            "state_changed" => {
                Some(EventSignalRef::mapped("state_changed", &self.state_changed, |v| {
                    CapabilityValue::String((*v).clone())
                }))
            }
            "input_fired" => Some(EventSignalRef::mapped("input_fired", &self.input_fired, |v| {
                CapabilityValue::String((*v).clone())
            })),
            _ => None,
        }
    }
}

/// `RiveWidget`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_media.in.rs` / `access_write_media.in.rs` dispatch: `is_playing`
/// maps onto `play` / `pause`.
impl WidgetProperties for RiveWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "is_playing" => Ok(CapabilityValue::Bool(self.is_playing())),
            "loop_count" => Ok(CapabilityValue::Int(self.loop_count() as i64)),
            "loops_completed" => Ok(CapabilityValue::Int(self.loops_completed() as i64)),
            "state" => Ok(CapabilityValue::String(self.state().as_str().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "is_playing" => {
                if expect_bool(value)? {
                    self.play();
                } else {
                    self.pause();
                }
                Ok(())
            }
            "loop_count" => {
                let count = match value {
                    CapabilityValue::Int(value) => value as i32,
                    CapabilityValue::UInt(value) => value as i32,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_loop_count(count);
                Ok(())
            }
            "loops_completed" | "state" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "is_playing",
            "loop_count",
            "loops_completed",
            "state",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for RiveWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        self.refresh_labels();
        let comp_extent = self.composition_extent();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then fall back to the original literal. The literal step is
        // kept deliberately: an inactive theme must still give the widget a defined
        // appearance, and the value is the one this widget painted before, so an existing
        // pixel baseline cannot move. Resolved once per draw, because the empty state, the
        // background, the border, the chips and the progress trough all read from it and
        // re-resolving would take the theme lock several times inside one draw.
        let style = self.style();
        let themed = crate::style::resolved_theme_style_shared("rive_widget");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let themed_border = themed.as_ref().and_then(|resolved| resolved.border_color);
        let themed_text = themed.as_ref().and_then(|resolved| resolved.text_color);
        let base_bg =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(245, 240, 250, 255));
        // The page this shell sits in, read once. A disabled **fill** recedes toward the page it
        // sits on; it cannot recede toward its own contrast colour the way ink does, because that
        // would make it stand out *more* against the page -- the opposite of disabled.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let border_color =
            style.border_color.or(themed_border).unwrap_or(Color::rgba(140, 80, 180, 150));
        // The placeholder ink follows the theme's foreground so it stays legible on whichever
        // surface the theme painted; the literal is only the last resort.
        let placeholder_text =
            style.text_color.or(themed_text).unwrap_or(Color::rgba(160, 160, 160, 220));
        // The neutral fill used by the name/progress pills and the progress trough: this
        // control's chrome, derived from the resolved border so it tracks the theme instead
        // of staying one grey in every appearance.
        let trough_color = border_color.blend(&base_bg, 0.55);

        if self.animation_name.is_empty() || self.animation_data.is_none() {
            // Honest empty state: a neutral placeholder panel. A name without data (the
            // result of `load_animation`, which has no JSON to parse) and a fresh widget both
            // land here, so nothing may be drawn that looks like animation content. The panel
            // and the label are this widget's chrome, so both follow the theme instead of
            // pinning the control to one appearance.
            context.fill_rounded_rect(rect, 4, base_bg);
            let font = effective_font(style);
            let text = if self.animation_name.is_empty() {
                "No Rive animation loaded"
            } else {
                "Rive animation not loaded"
            };
            // Centred on the vertical midline and fitted to the control's width: the label
            // is 24 characters at 14 px, wider than most controls it is drawn into, and
            // without the fit it ran past the panel and out of the SVG snapshot.
            let metrics = context.measure_text(text, font);
            let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;
            let line = Rect::new(rect.x, text_y, rect.width, metrics.height);
            context.draw_text_fitted_cow(
                line,
                text,
                font,
                placeholder_text,
                HorizontalAlignment::Center,
            );
            return;
        }

        // Background — the surface the animation is rendered onto, i.e. chrome. The former
        // Disabled steps the resolved fill toward its own contrast colour rather than toward a
        // fixed white, which on a dark surface made the disabled state *more* prominent than the
        // enabled one.
        let bg = if !is_enabled {
            self.base.disabled_surface_near(base_bg, window_fill)
        } else {
            base_bg
        };
        context.face_with_gradient(
            rect,
            bg,
            style.background_gradient.as_ref(),
            style.surface.unwrap_or_default(),
            style.border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // Draw bounding box — a chrome edge around the animation, so it follows the theme's
        // border token rather than a fixed violet.
        context.draw_rect_stroke(rect, border_color, 1);

        // The composition's extent is cached by `composition_extent`; this pass only reads it.
        let (comp_w, comp_h) = (comp_extent * 2.0, comp_extent * 2.0);

        let scale_x = rect.width as f64 / comp_w.max(1.0);
        let scale_y = rect.height as f64 / comp_h.max(1.0);
        let scale = scale_x.min(scale_y);
        let offset_x = rect.x as f64 + (rect.width as f64 - comp_w * scale) / 2.0;
        let offset_y = rect.y as f64 + (rect.height as f64 - comp_h * scale) / 2.0;

        let progress = self.animation_progress as f64;

        // Render animated shapes.
        if let Some(ref data) = self.animation_data {
            for shape in &data.shapes {
                let (sx, sy) = shape.position_at(progress);
                let (sw, sh) = shape.size_at(progress);
                let color = shape.color_at(progress);
                let rot = shape.rotation_at(progress);

                let cx = offset_x + sx * scale;
                let cy = offset_y + sy * scale;
                let w = sw * scale;
                let h = sh * scale;

                draw_shape(context, shape.shape_type, cx, cy, w, h, color, rot);
            }
        }

        // Draw animation name label at top. The text is cached by `refresh_labels`.
        let font = effective_font(style);
        let name_text = self.name_text.as_str();
        let name_metrics = context.measure_text(name_text, font);
        let name_x = rect.x + 4;
        // Origin is the glyph box's top edge, so it is the same `+ 2` the backdrop uses;
        // the removed `+ ascent` had pushed the label half a line down inside its own pill.
        let name_y = rect.y + 2;
        let name_bg = Rect::new(
            name_x - 2,
            rect.y + 1,
            name_metrics.width + 8,
            name_metrics.height + 4,
        );
        context.fill_rounded_rect(name_bg, 3, trough_color);
        context.draw_text(
            Point::new(name_x, name_y),
            name_text,
            font,
            Color::WHITE,
            HorizontalAlignment::Left,
        );

        // Draw progress percentage at top-right. Rebuilt only when the integer percentage
        // changes, not on every frame.
        let progress_text = self.progress_text.as_str();
        let p_metrics = context.measure_text(progress_text, font);
        let px = rect.x + rect.width as i32 - p_metrics.width as i32 - 6;
        // Same top-edge origin as the name label: no ascent term, or the percentage sits
        // half a line low in its pill.
        let py = rect.y + 2;
        let p_bg =
            Rect::new(px - 2, rect.y + 1, p_metrics.width + 8, p_metrics.height + 4);
        context.fill_rounded_rect(p_bg, 3, trough_color);
        context.draw_text(
            Point::new(px, py),
            progress_text,
            font,
            Color::WHITE,
            HorizontalAlignment::Left,
        );

        // Play/pause icon: a `Play`/`Pause` **icon outline**, not the `▶`/`⏸` text glyphs
        // (covered by no bundled face). The two colours are deliberately not themed: green
        // versus amber *is* the state encoding — "playing" versus "paused" — and a theme would
        // recolour both to whatever roles they happened to match.
        let status_icon = if self.is_playing { IconName::Play } else { IconName::Pause };
        let status_color = if self.is_playing {
            Color::rgba(40, 160, 40, 200)
        } else {
            Color::rgba(180, 100, 40, 200)
        };
        crate::widget::draw_icon_at(
            context,
            Rect::new(rect.x + 4, rect.y + rect.height as i32 - 16, 12, 12),
            status_color,
            status_icon,
        );

        // State machine inputs count.
        if !self.state_machine_inputs.is_empty() {
            let input_text = self.input_text.as_str();
            let input_metrics = context.measure_text(input_text, font);
            let input_x = rect.x + rect.width as i32 - input_metrics.width as i32 - 6;
            // `input_y` is the intended *top* edge, not a baseline, so the ascent term is
            // dropped; subtracting it had pulled the label half a line up off that edge.
            let input_y = rect.y + rect.height as i32 - 6;
            context.draw_text(
                Point::new(input_x, input_y),
                input_text,
                font,
                Color::rgba(120, 80, 160, 180),
                HorizontalAlignment::Left,
            );
        }

        // Progress bar at bottom.
        let progress_bar_height = 6u32;
        let progress_bar_y = rect.y + rect.height as i32 - progress_bar_height as i32 - 4;
        let progress_bar_full = Rect::new(
            rect.x + 4,
            progress_bar_y,
            rect.width.saturating_sub(8),
            progress_bar_height,
        );
        // The trough is chrome — the empty part of an indicator — so it follows the theme;
        // the fill below is data and keeps its colour.
        context.fill_rounded_rect(progress_bar_full, 3, trough_color);

        let filled_width =
            ((progress_bar_full.width as f64) * self.animation_progress as f64) as u32;
        if filled_width > 0 {
            let progress_bar_fill = Rect::new(
                progress_bar_full.x,
                progress_bar_full.y,
                filled_width,
                progress_bar_full.height,
            );
            // Deliberately not themed: this violet *encodes progress*. Recolouring it from a
            // theme would make the bar indistinguishable from the trough it sits in.
            context.fill_rounded_rect(progress_bar_fill, 3, Color::rgba(140, 60, 200, 200));
        }
    }
}

impl EventHandler for RiveWidget {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            // Playback toggles on **release** only (L-P01).
            Event::MouseRelease { pos, button } => {
                if *button == 1 && self.geometry().contains_point(*pos) {
                    if self.is_playing {
                        self.pause();
                    } else {
                        self.play();
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_rive_json() -> String {
        r#"{
            "duration": 60,
            "fr": 60,
            "shapes": [
                {"ty":"rect","x":50,"y":50,"w":60,"h":60,"r":60,"g":100,"b":200,"a":220,"mx":20,"my":15,"sa":0.2,"ra":15},
                {"ty":"circle","x":50,"y":50,"w":30,"h":30,"r":200,"g":80,"b":60,"a":200,"mx":-15,"my":10,"sa":0.3,"cs":true}
            ]
        }"#
        .to_string()
    }

    #[test]
    fn rive_widget_creation_defaults() {
        let rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        assert_eq!(rive.animation_name(), "");
        assert!(!rive.is_playing());
        assert_eq!(rive.loop_count(), 0);
        assert_eq!(rive.animation_progress(), 0.0);
        assert!(rive.inputs().is_empty());
        assert_eq!(rive.kind(), WidgetKind::RiveWidget);
    }

    #[test]
    fn rive_widget_load_animation_and_play_pause_stop() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation_from_json("idle", &make_rive_json()).unwrap();
        assert_eq!(rive.animation_name(), "idle");

        assert!(!rive.is_playing());
        rive.play();
        assert!(rive.is_playing());
        rive.pause();
        assert!(!rive.is_playing());
        rive.play();
        assert!(rive.is_playing());
        rive.stop();
        assert!(!rive.is_playing());
        assert_eq!(rive.animation_progress(), 0.0);
    }

    #[test]
    fn rive_widget_set_input_and_get_input() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.set_input("enabled", RiveInputValue::Bool(true));
        rive.set_input("speed", RiveInputValue::Number(1.5));
        rive.set_input("trigger", RiveInputValue::Trigger);

        assert_eq!(rive.inputs().len(), 3);

        let enabled = rive.get_input("enabled").unwrap();
        assert_eq!(enabled.name, "enabled");
        assert_eq!(enabled.value, RiveInputValue::Bool(true));

        let speed = rive.get_input("speed").unwrap();
        assert_eq!(speed.name, "speed");
        assert_eq!(speed.value, RiveInputValue::Number(1.5));

        let trigger = rive.get_input("trigger").unwrap();
        assert_eq!(trigger.name, "trigger");
        assert_eq!(trigger.value, RiveInputValue::Trigger);
    }

    #[test]
    fn rive_widget_update_existing_input() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.set_input("volume", RiveInputValue::Number(0.5));
        rive.set_input("volume", RiveInputValue::Number(0.8));
        assert_eq!(rive.inputs().len(), 1);

        let volume = rive.get_input("volume").unwrap();
        assert_eq!(volume.value, RiveInputValue::Number(0.8));
    }

    #[test]
    fn rive_widget_tick_advances_progress() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation_from_json("run", &make_rive_json()).unwrap();
        rive.play();

        assert_eq!(rive.animation_progress(), 0.0);

        // Tick 17ms: a 60 fps frame is 16.667 ms, so 16 ms is *less* than one frame now that the
        // fractional period is carried instead of truncated to 16 ms.
        rive.tick(17);
        assert!(rive.animation_progress() > 0.0);
        assert!(rive.animation_progress() < 0.1);
    }

    #[test]
    fn rive_widget_tick_not_playing_does_nothing() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation_from_json("run", &make_rive_json()).unwrap();
        // Not playing.
        rive.tick(100);
        assert_eq!(rive.animation_progress(), 0.0);
    }

    #[test]
    fn rive_widget_animation_finished_signal() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation_from_json("walk", &make_rive_json()).unwrap();
        rive.set_loop_count(1);
        rive.play();

        let finished = Arc::new(Mutex::new(false));
        rive.animation_finished.connect({
            let finished = Arc::clone(&finished);
            move || {
                *finished.lock().unwrap() = true;
            }
        });

        // Tick enough to complete one loop (60 frames at 60fps ≈ 1000ms).
        rive.tick(1000);
        assert!(*finished.lock().unwrap());
        assert!(!rive.is_playing());
    }

    #[test]
    fn rive_widget_infinite_loop() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation_from_json("spin", &make_rive_json()).unwrap();
        rive.set_loop_count(0); // infinite
        rive.play();

        // Tick enough to loop multiple times.
        rive.tick(2000);
        assert!(rive.is_playing()); // still playing (infinite)
                                    // Progress should be somewhere in [0, 1.0)
        assert!(rive.animation_progress() >= 0.0);
        assert!(rive.animation_progress() < 1.0);
    }

    #[test]
    fn rive_widget_remove_input() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.set_input("foo", RiveInputValue::Bool(true));
        rive.set_input("bar", RiveInputValue::Number(42.0));
        assert_eq!(rive.inputs().len(), 2);

        rive.remove_input("foo");
        assert_eq!(rive.inputs().len(), 1);
        assert!(rive.get_input("foo").is_none());
        assert!(rive.get_input("bar").is_some());
    }

    #[test]
    fn rive_widget_load_from_json() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_rive_json();
        rive.load_animation_from_json("test", &json).unwrap();
        assert_eq!(rive.animation_name(), "test");
        // Should have parsed the 2 shapes from JSON.
        assert!(rive.animation_data.is_some());
    }

    #[test]
    fn rive_widget_load_from_empty_json_returns_error() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        let result = rive.load_animation_from_json("test", "");
        assert!(result.is_err());
    }

    #[test]
    fn rive_widget_load_from_invalid_json_returns_error() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        let result = rive.load_animation_from_json("test", "not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid Rive animation JSON"));
    }

    #[test]
    fn rive_widget_position_at_varies_with_progress() {
        let shape = RiveAnimatedShape {
            shape_type: RiveShapeType::Rectangle,
            x: 50.0,
            y: 50.0,
            width: 40.0,
            height: 40.0,
            r: 100,
            g: 100,
            b: 100,
            a: 255,
            motion_x: 20.0,
            motion_y: 10.0,
            scale_amplitude: 0.0,
            rotation_amplitude: 0.0,
            color_shift: false,
        };
        // At progress 0, sin(0) = 0, cos(0) = 1
        let (x0, y0) = shape.position_at(0.0);
        assert!((x0 - 50.0).abs() < 0.01);
        assert!((y0 - 60.0).abs() < 0.01);

        // At progress 0.25, sin(pi/2) = 1, cos(pi/2) = 0
        let (x25, y25) = shape.position_at(0.25);
        assert!((x25 - 70.0).abs() < 0.01);
        assert!((y25 - 50.0).abs() < 0.01);
    }

    #[test]
    fn rive_widget_draw_does_not_panic_with_shapes() {
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(100, 100), 1.0);
        let mut ctx = crate::render::RenderContext::new(&mut backend);

        let mut rive = RiveWidget::new(Rect::new(0, 0, 100, 100));
        rive.load_animation_from_json("test", &make_rive_json()).unwrap();
        rive.play();

        // Should not panic when drawing with shapes.
        rive.draw(&mut ctx);
        // No crash = test passes.
    }

    #[test]
    fn rive_widget_draw_empty_no_crash() {
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(100, 100), 1.0);
        let mut ctx = crate::render::RenderContext::new(&mut backend);

        let mut rive = RiveWidget::new(Rect::new(0, 0, 100, 100));
        // No animation loaded - empty state.
        rive.draw(&mut ctx);
        // No crash = test passes.
    }

    #[test]
    fn rive_widget_draw_with_json_shapes_no_crash() {
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(100, 100), 1.0);
        let mut ctx = crate::render::RenderContext::new(&mut backend);

        let mut rive = RiveWidget::new(Rect::new(0, 0, 100, 100));
        let json = make_rive_json();
        rive.load_animation_from_json("test", &json).unwrap();
        rive.play();
        rive.draw(&mut ctx);
        // No crash = test passes.
    }

    #[test]
    fn rive_animated_shape_size_scales_with_progress() {
        let shape = RiveAnimatedShape {
            shape_type: RiveShapeType::Circle,
            x: 50.0,
            y: 50.0,
            width: 40.0,
            height: 40.0,
            r: 255,
            g: 0,
            b: 0,
            a: 255,
            motion_x: 0.0,
            motion_y: 0.0,
            scale_amplitude: 0.5,
            rotation_amplitude: 0.0,
            color_shift: false,
        };
        // At t=0, sin(0)=0 -> scale = 1.0, so w=40
        let (w0, _h0) = shape.size_at(0.0);
        assert!((w0 - 40.0).abs() < 0.01);
        // At t=0.25, sin(pi/2)=1 -> scale = 1.5, so w=60
        let (w25, _h25) = shape.size_at(0.25);
        assert!((w25 - 60.0).abs() < 0.01);
    }

    /// N-V-07: a huge `delta_ms` must not spin an unbounded frame-consumption loop.
    ///
    /// `frame_timer += delta_ms` used to overflow to a small/large wrapped value that a `while` loop
    /// then consumed one 16ms frame at a time, potentially billions of iterations. The frame count is
    /// now computed once and capped, so the call returns promptly.
    #[test]
    fn a_huge_tick_does_not_loop_unbounded() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 100, 100));
        rive.load_animation_from_json("wave", &make_rive_json()).unwrap();
        rive.play();
        // If the old `while` loop still ran per-frame, this call would take a very long time; the
        // capped implementation returns immediately. Debug builds also panicked on the wrapping `+=`.
        let completed = rive.tick(u64::MAX / 2);
        // The widget is still in a sane state afterwards.
        let svg = crate::widget::svg::render_to_svg(&mut rive);
        assert!(svg.starts_with("<svg"));
        // `completed` may be true or false depending on loop_count; touching it proves the call ran.
        let _ = completed;
    }

    /// A path with no readable file is an explicit error, and no animation data is fabricated.
    #[test]
    fn rive_widget_load_animation_without_a_readable_file_is_an_error() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        let error = rive
            .load_animation("definitely-not-a-riv-file.riv")
            .expect_err("an unreadable path is not animation data");
        assert!(
            error.contains("load_animation_from_riv"),
            "the refusal must point at the in-memory entry point, got: {error:?}"
        );
        assert!(rive.animation_data.is_none(), "a failed load must not fabricate animation data");
        assert!(!rive.is_playing(), "a failed load must not start playback");

        let svg = crate::widget::svg::render_to_svg(&mut rive);
        assert!(
            !crate::widget::svg::text_ink_boxes(&svg).is_empty(),
            "the placeholder must paint a label:\n{svg}"
        );
        assert!(!svg.contains("<circle"), "no fake circle shape may be drawn:\n{svg}");
    }

    /// Encodes an unsigned LEB128 value the way a `.riv` header/object stream does.
    fn push_var_uint(mut value: u64, out: &mut Vec<u8>) {
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if value == 0 {
                break;
            }
        }
    }

    /// A minimal valid `.riv`: one artboard, one 24 fps / 48-frame animation and one state machine.
    fn make_riv_file() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"RIVE");
        for value in [7u64, 4, 1] {
            push_var_uint(value, &mut out);
        }
        // Property keys: artboard/animation/state-machine names, animation fps and duration.
        for key in [4u64, 55, 56, 57, 138, 0] {
            push_var_uint(key, &mut out);
        }
        // Two bits per key: key 4 = string, 55 = string, 56 = uint, 57 = uint, 138 = string.
        // The header starts a fresh `u32` for every four keys, so the fifth key begins a second
        // word even though only two of its bits are used.
        let kinds = 1u32 | (1 << 2);
        out.extend_from_slice(&kinds.to_le_bytes());
        let kinds_tail = 1u32; // key 138: string
        out.extend_from_slice(&kinds_tail.to_le_bytes());

        let push_string = |out: &mut Vec<u8>, text: &str| {
            push_var_uint(text.len() as u64, out);
            out.extend_from_slice(text.as_bytes());
        };

        // Artboard.
        push_var_uint(1, &mut out);
        push_var_uint(4, &mut out);
        push_string(&mut out, "Main");
        push_var_uint(0, &mut out);

        // Linear animation: name, fps, duration.
        push_var_uint(31, &mut out);
        push_var_uint(55, &mut out);
        push_string(&mut out, "Bounce");
        push_var_uint(56, &mut out);
        push_var_uint(24, &mut out);
        push_var_uint(57, &mut out);
        push_var_uint(48, &mut out);
        push_var_uint(0, &mut out);

        // State machine.
        push_var_uint(53, &mut out);
        push_var_uint(138, &mut out);
        push_string(&mut out, "Hover");
        push_var_uint(0, &mut out);
        out
    }

    #[test]
    fn load_animation_from_riv_decodes_names_and_duration() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation_from_riv("Bounce", &make_riv_file()).expect("a valid .riv file loads");
        assert_eq!(rive.artboard_names(), vec!["Main".to_string()]);
        assert_eq!(rive.animation_names(), vec!["Bounce".to_string()]);
        assert_eq!(rive.state_machine_names(), vec!["Hover".to_string()]);
        let info = rive.file_info().expect("the header was decoded");
        assert_eq!((info.major_version, info.minor_version), (7, 4));
        assert_eq!(info.animations[0].duration_frames, 48);
        assert_eq!(info.animations[0].frame_rate, 24.0);
        assert!(rive.animation_data.is_some(), "the animation is playable");
        assert_eq!(rive.state(), RiveState::Paused);
    }

    #[test]
    fn malformed_riv_files_are_refused_with_a_specific_error() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        let bad_magic = rive.load_animation_from_riv("x", b"NOPE").expect_err("bad magic");
        assert!(bad_magic.contains("RIVE"), "got {bad_magic:?}");

        let mut truncated = make_riv_file();
        truncated.truncate(12);
        let error = rive.load_animation_from_riv("x", &truncated).expect_err("truncated file");
        assert!(error.contains("malformed Rive file"), "got {error:?}");

        let mut unknown = make_riv_file();
        // Rewrite the first object's property key (4) to a key absent from the ToC. Search from the
        // end: the object stream follows the header's own property-key list.
        let position = unknown
            .windows(2)
            .rposition(|window| window == [1, 4])
            .expect("the artboard object header is present");
        unknown[position + 1] = 22;
        let error = rive.load_animation_from_riv("x", &unknown).expect_err("unknown key");
        assert!(error.contains("unknown property key 22"), "got {error:?}");
        assert!(rive.animation_data.is_none(), "a failed load leaves nothing playable");
    }

    /// The documented `playing` input gates the tick.
    #[test]
    fn rive_widget_playing_false_input_pauses_tick() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation_from_json("run", &make_rive_json()).unwrap();
        rive.play();
        rive.set_input("playing", RiveInputValue::Bool(false));

        let before = rive.animation_progress();
        assert!(!rive.tick(1000), "a gated tick cannot complete a loop");
        assert_eq!(rive.animation_progress(), before, "playing=false must pause advancement");
    }

    /// The documented `progress` input pins the playhead and clamps it.
    #[test]
    fn rive_widget_progress_input_pins_the_playhead() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation_from_json("run", &make_rive_json()).unwrap();
        rive.play();
        rive.set_input("progress", RiveInputValue::Number(0.5));

        rive.tick(1000);
        assert!((rive.animation_progress() - 0.5).abs() < 1e-6, "the playhead must sit at half");
        rive.tick(1000);
        assert!(
            (rive.animation_progress() - 0.5).abs() < 1e-6,
            "a pinned playhead must not drift across ticks"
        );

        rive.set_input("progress", RiveInputValue::Number(2.0));
        rive.tick(16);
        assert_eq!(rive.animation_progress(), 1.0, "an out-of-range progress is clamped");
    }

    #[test]
    fn declared_frame_rate_and_duration_drive_the_timeline() {
        let json = r#"{"duration":24,"fr":24,"shapes":[]}"#;
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation_from_json("fast", json).unwrap();
        rive.set_loop_count(1);
        rive.play();
        // 24 frames at 24 fps is one second, so one 1000 ms tick completes the loop.
        assert!(rive.tick(1000));
        assert!((rive.animation_progress() - 1.0).abs() < 1e-6);
        assert_eq!(rive.loop_count(), 1, "the configured count is not consumed");
        assert_eq!(rive.loops_completed(), 1);
    }

    #[test]
    fn the_lifecycle_state_is_observable_and_triggers_fire_edges() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        assert_eq!(rive.state(), RiveState::Idle);
        rive.load_animation_from_json("run", &make_rive_json()).unwrap();
        assert_eq!(rive.state(), RiveState::Paused);

        let states = Arc::new(Mutex::new(Vec::new()));
        let s = states.clone();
        rive.state_changed.connect(move |value: Arc<String>| {
            s.lock().unwrap().push((*value).clone());
        });
        let fired = Arc::new(Mutex::new(Vec::new()));
        let f = fired.clone();
        rive.input_fired.connect(move |value: Arc<String>| {
            f.lock().unwrap().push((*value).clone());
        });

        rive.set_input("play", RiveInputValue::Trigger);
        rive.set_input("ping", RiveInputValue::Trigger);
        assert_eq!(rive.state_machine_names(), vec!["play".to_string(), "ping".to_string()]);

        assert!(rive.fire_input("play"));
        assert_eq!(rive.state(), RiveState::Playing);
        assert!(rive.fire_input("ping"), "an unrelated trigger still reports its edge");
        assert_eq!(rive.state(), RiveState::Playing, "an unrelated trigger does not move state");
        assert!(!rive.fire_input("missing"));
        assert_eq!(
            *states.lock().unwrap(),
            vec!["playing".to_string()],
            "only a real transition is announced"
        );
        assert_eq!(*fired.lock().unwrap(), vec!["play".to_string(), "ping".to_string()]);
    }
}
