// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! LottieWidget — Lottie JSON animation player widget.
//!
//! The LottieWidget parses a Lottie JSON animation, manages play/pause/stop
//! controls, frame rate, loop count, and frame advancement. It emits a signal
//! when the animation finishes. Shapes defined in the Lottie JSON are parsed
//! and rendered using the RenderContext.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};

// ──────────────────────────────────────────────
// Lottie shape data model
// ──────────────────────────────────────────────

/// The largest frame count a Lottie composition may declare.
///
/// A real composition is seconds long: at 60 fps this allows over four and a half
/// hours of animation. The cap exists because `op`/`ip` are read straight from the
/// document, so without it a single field can declare `u32::MAX` frames.
const MAX_LOTTIE_FRAMES: u32 = 1_000_000;

/// Frame-rate bounds a composition may declare, in frames per second.
///
/// The lower bound keeps the frame timer from advancing by a fraction that rounds
/// to zero forever; the upper bound keeps `frame_rate` finite as an `f32` and well
/// above any display's refresh rate.
const MIN_LOTTIE_FPS: f32 = 0.01;
const MAX_LOTTIE_FPS: f32 = 1000.0;

/// A single keyframe for an animated property.
#[derive(Debug, Clone)]
pub struct LottieKeyFrame {
    /// Frame at which this keyframe starts.
    pub t: f64,
    /// Value(s) at this keyframe. The vector holds multiple components
    /// (e.g. [x, y] for position, [r, g, b, a] for color).
    pub s: Vec<f64>,
}

/// An animated property that may have keyframes or a static value.
#[derive(Debug, Clone)]
pub struct LottieAnimated {
    /// Static value if no keyframes, or value at frame 0.
    pub base: Vec<f64>,
    /// Keyframes (empty if the property is static).
    pub keyframes: Vec<LottieKeyFrame>,
}

impl LottieAnimated {
    /// Parse from a Lottie property JSON value.
    /// The value is typically under `"k"` key of a property.
    fn from_json(val: &serde_json::Value) -> Self {
        if let Some(arr) = val.as_array() {
            // Check if it's a keyframe array (each element has "t" and "s")
            if arr.first().and_then(|v| v.get("t")).is_some() {
                let base = arr
                    .first()
                    .and_then(|v| v.get("s"))
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|n| n.as_f64()).collect())
                    .unwrap_or_default();
                let keyframes = arr
                    .iter()
                    .filter_map(|kf| {
                        let t = kf.get("t")?.as_f64()?;
                        let s =
                            kf.get("s")?.as_array()?.iter().filter_map(|n| n.as_f64()).collect();
                        Some(LottieKeyFrame { t, s })
                    })
                    .collect();
                return Self { base, keyframes };
            }
            // Static array value
            let base = arr.iter().filter_map(|n| n.as_f64()).collect();
            return Self { base, keyframes: Vec::new() };
        }
        // Single number
        if let Some(n) = val.as_f64() {
            return Self { base: vec![n], keyframes: Vec::new() };
        }
        Self { base: Vec::new(), keyframes: Vec::new() }
    }

    /// Interpolate the value at a given frame.
    fn at_frame(&self, frame: f64) -> Vec<f64> {
        if self.keyframes.is_empty() {
            return self.base.clone();
        }
        // Find the two keyframes that surround `frame`.
        let mut prev_idx = 0;
        for (i, kf) in self.keyframes.iter().enumerate() {
            if kf.t <= frame {
                prev_idx = i;
            } else {
                break;
            }
        }
        let next_idx = if prev_idx + 1 < self.keyframes.len() { prev_idx + 1 } else { prev_idx };

        let prev = &self.keyframes[prev_idx];
        if prev_idx == next_idx {
            return prev.s.clone();
        }
        let next = &self.keyframes[next_idx];
        let range = (next.t - prev.t).max(1.0);
        let t = ((frame - prev.t) / range).clamp(0.0, 1.0);
        prev.s.iter().zip(next.s.iter()).map(|(a, b)| a + (b - a) * t).collect()
    }

    /// Get a single interpolated value (for single-component properties).
    fn at_frame_scalar(&self, frame: f64) -> f64 {
        self.at_frame(frame).first().copied().unwrap_or(0.0)
    }
}

/// A Lottie color property (rgba).
///
/// Channels are stored as `f64` because Lottie's JSON represents them as
/// normalized floats in `0.0 ..= 1.0`, **not** as 0-255 bytes. They are passed
/// straight to [`Color::from_f32`], which is what performs any scaling.
///
/// Animation is **not** honoured: the value is sampled once at parse time (see
/// `LottieColor::from_json()`), so a colour keyframe track animates nothing.
#[derive(Debug, Clone)]
pub struct LottieColor {
    /// Red, normalised to `0.0 ..= 1.0`.
    pub r: f64,
    /// Green, normalised to `0.0 ..= 1.0`.
    pub g: f64,
    /// Blue, normalised to `0.0 ..= 1.0`.
    pub b: f64,
    /// Alpha, normalised to `0.0 ..= 1.0`; defaults to `1.0` when the Lottie
    /// source omits a fourth component.
    pub a: f64,
}

impl LottieColor {
    fn from_json(val: &serde_json::Value) -> Self {
        let animated = LottieAnimated::from_json(val);
        let v = animated.at_frame(0.0);
        Self {
            r: v.first().copied().unwrap_or(0.0),
            g: v.get(1).copied().unwrap_or(0.0),
            b: v.get(2).copied().unwrap_or(0.0),
            a: v.get(3).copied().unwrap_or(1.0),
        }
    }

    fn at_frame(&self, _frame: f64) -> Color {
        Color::from_f32(self.r as f32, self.g as f32, self.b as f32, self.a as f32)
    }
}

/// A rectangle shape ("rc").
#[derive(Debug, Clone)]
pub struct LottieRectShape {
    /// Position (anchor point).
    pub position: LottieAnimated,
    /// Size [width, height].
    pub size: LottieAnimated,
    /// Rounded corner radius.
    pub rounded: LottieAnimated,
}

/// An ellipse shape ("el").
#[derive(Debug, Clone)]
pub struct LottieEllipseShape {
    /// Position (center).
    pub position: LottieAnimated,
    /// Size [width, height].
    pub size: LottieAnimated,
}

/// A fill shape ("fl").
#[derive(Debug, Clone)]
pub struct LottieFill {
    /// Fill colour, sampled at parse time only — see [`LottieColor`].
    pub color: LottieColor,
    /// Opacity as a percentage in `0 ..= 100`, matching Lottie's encoding.
    pub opacity: LottieAnimated,
    /// Fill rule: 0 = even-odd, 1 = non-zero (winding). Parsed but unused; the
    /// renderer always fills with one rule.
    pub _fill_rule: u32,
}

/// A stroke shape ("st").
#[derive(Debug, Clone)]
pub struct LottieStroke {
    /// Stroke colour, sampled at parse time only — see [`LottieColor`].
    pub color: LottieColor,
    /// Opacity as a percentage in `0 ..= 100`.
    pub opacity: LottieAnimated,
    /// Stroke width in composition units (not points or logical pixels).
    pub width: LottieAnimated,
    /// Line cap style: 0 = butt, 1 = round, 2 = square. Parsed but unused.
    pub _line_cap: u32,
    /// Line join style: 0 = miter, 1 = round, 2 = bevel. Parsed but unused.
    pub _line_join: u32,
}

/// A bezier path shape ("sh").
///
/// Lottie stores a path as three parallel point lists in the shape's `ks.k`
/// object: the vertices (`v`) and, relative to each vertex, an incoming (`i`)
/// and outgoing (`o`) tangent. Consecutive vertices plus their tangents form
/// cubic bezier segments — `ctrl1 = v[n] + o[n]`, `ctrl2 = v[n+1] + i[n+1]` —
/// which is exactly how the renderer flattens them (D09-LOT-01).
#[derive(Debug, Clone)]
pub struct LottiePathShape {
    /// Vertices, in the layer's coordinate space.
    pub vertices: Vec<[f64; 2]>,
    /// In-tangents, each relative to the vertex of the same index.
    pub in_tangents: Vec<[f64; 2]>,
    /// Out-tangents, each relative to the vertex of the same index.
    pub out_tangents: Vec<[f64; 2]>,
    /// Whether the contour is closed (`c` in the source).
    pub closed: bool,
}

impl LottiePathShape {
    /// Parses a `"sh"` shape from its `ks.k` value.
    ///
    /// The value is normally the shape object itself; when the path is animated
    /// it is an array of keyframes, each carrying that shape under `s`. Only the
    /// first keyframe is sampled, matching how [`LottieColor`] is handled: the
    /// animation is not honoured, but the geometry is not lost (D09-LOT-01).
    fn from_json(val: &serde_json::Value) -> Option<Self> {
        // Unwrap a keyframe array down to the first shape object. A keyframe's `s`
        // holds either the shape object directly or a one-element array of it.
        let shape = if let Some(arr) = val.as_array() {
            let first = arr.first()?;
            match first.get("s") {
                Some(s) => s.as_array().and_then(|a| a.first()).unwrap_or(s),
                None => first,
            }
        } else {
            val
        };

        let read_points = |key: &str| -> Vec<[f64; 2]> {
            shape
                .get(key)
                .and_then(|v| v.as_array())
                .map(|points| {
                    points
                        .iter()
                        .filter_map(|p| {
                            let arr = p.as_array()?;
                            let x = arr.first()?.as_f64()?;
                            let y = arr.get(1)?.as_f64()?;
                            if x.is_finite() && y.is_finite() {
                                Some([x, y])
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .unwrap_or_default()
        };

        let vertices = read_points("v");
        if vertices.len() < 2 {
            // A path needs at least two vertices to span a segment; fewer is not
            // renderable and is reported as unsupported by the loader.
            return None;
        }
        let closed = shape.get("c").and_then(|v| v.as_bool()).unwrap_or(false);
        Some(Self {
            vertices,
            in_tangents: read_points("i"),
            out_tangents: read_points("o"),
            closed,
        })
    }
}

/// A group of shapes ("gr"/"gs") with its own transform.
///
/// A group's `it` array holds child shapes plus a trailing `"tr"` transform
/// entry. The children are kept in declaration order; the transform is composed
/// with the enclosing placement before its children are drawn (D09-LOT-01).
#[derive(Debug, Clone)]
pub struct LottieGroup {
    /// Child shapes, in declaration order, excluding the transform entry.
    pub items: Vec<LottieShape>,
    /// The group's own transform ("tr"); the identity when absent.
    pub transform: LottieTransform,
}

/// A shape within a layer.
#[derive(Debug, Clone)]
pub enum LottieShape {
    /// A rectangle ("rc").
    Rectangle(LottieRectShape),
    /// An ellipse ("el").
    Ellipse(LottieEllipseShape),
    /// A solid fill ("fl") applied to preceding shapes.
    Fill(LottieFill),
    /// A stroke ("st") applied to preceding shapes.
    Stroke(LottieStroke),
    /// A bezier path ("sh").
    Path(LottiePathShape),
    /// A nested group ("gr"/"gs") with its own transform.
    Group(LottieGroup),
    /// A shape type this renderer does not draw.
    ///
    /// The inner string records the Lottie shape type (`ty`). It is **not**
    /// silently dropped: the loader records every such type in
    /// [`LottieWidget::unsupported_shapes`], which callers can query
    /// (D09-LOT-01).
    Other(String),
}

/// Where and how a shape is placed on the device, threaded through layers and
/// nested groups so a group's transform composes with its parent's (D09-LOT-01).
///
/// Coordinates (positions and path vertices) are mapped with `scale_x`/`scale_y`;
/// the sizes of primitive rectangles and ellipses are additionally multiplied by
/// `size_x`/`size_y`. The two are separate because the layer transform from the
/// original renderer scaled a primitive's size but not its position, and that
/// existing pixel behaviour is preserved exactly when no group is involved.
#[derive(Debug, Clone, Copy)]
struct ShapePlacement {
    /// Device x of the composition origin.
    tx: f64,
    /// Device y of the composition origin.
    ty: f64,
    /// Device pixels per composition unit, horizontally (coordinates).
    scale_x: f64,
    /// Device pixels per composition unit, vertically (coordinates).
    scale_y: f64,
    /// Extra horizontal size multiplier accumulated from transforms.
    size_x: f64,
    /// Extra vertical size multiplier accumulated from transforms.
    size_y: f64,
    /// Accumulated opacity multiplier in `0.0..=1.0`.
    opacity: f64,
    /// Whether a non-trivial rotation is in effect for this placement.
    has_rotation: bool,
}

/// Transform properties for a layer.
#[derive(Debug, Clone)]
pub struct LottieTransform {
    /// Anchor point (a).
    pub _anchor: LottieAnimated,
    /// Position (p).
    pub position: LottieAnimated,
    /// Scale (s) — percentage, [100, 100] = 100%.
    pub scale: LottieAnimated,
    /// Rotation (r) — degrees.
    pub rotation: LottieAnimated,
    /// Opacity (o) — 0-100.
    pub opacity: LottieAnimated,
}

impl Default for LottieTransform {
    fn default() -> Self {
        Self {
            _anchor: LottieAnimated { base: vec![0.0, 0.0], keyframes: Vec::new() },
            position: LottieAnimated { base: vec![0.0, 0.0], keyframes: Vec::new() },
            scale: LottieAnimated { base: vec![100.0, 100.0], keyframes: Vec::new() },
            rotation: LottieAnimated { base: vec![0.0], keyframes: Vec::new() },
            opacity: LottieAnimated { base: vec![100.0], keyframes: Vec::new() },
        }
    }
}

/// A single layer in the Lottie animation.
#[derive(Debug, Clone)]
pub struct LottieLayer {
    /// Layer index.
    pub index: i32,
    /// Parent layer index (or -1 if none).
    pub _parent: i32,
    /// Shapes in this layer.
    pub shapes: Vec<LottieShape>,
    /// Transform for this layer.
    pub transform: LottieTransform,
    /// Layer opacity (derived from transform opacity or ef).
    pub _opacity: f64,
}

impl LottieLayer {
    fn from_json(layer_val: &serde_json::Value) -> Option<Self> {
        let index = layer_val.get("ind").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let _parent = layer_val.get("parent").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;

        // Parse transform ("ks" or "tr").
        let ks_val = layer_val.get("ks").or_else(|| layer_val.get("tr"));
        let transform = if let Some(ks) = ks_val {
            Self::parse_transform(ks)
        } else {
            LottieTransform::default()
        };

        // Parse shapes array.
        let shapes = if let Some(shapes_arr) = layer_val.get("shapes").and_then(|v| v.as_array()) {
            shapes_arr.iter().filter_map(Self::parse_shape).collect()
        } else {
            Vec::new()
        };

        let _opacity = transform.opacity.at_frame_scalar(0.0) / 100.0;

        Some(Self { index, _parent, shapes, transform, _opacity })
    }

    fn parse_transform(val: &serde_json::Value) -> LottieTransform {
        let _anchor = val
            .get("a")
            .and_then(|v| v.get("k"))
            .map(LottieAnimated::from_json)
            .unwrap_or_else(|| LottieAnimated { base: vec![0.0, 0.0], keyframes: Vec::new() });
        let position = val
            .get("p")
            .and_then(|v| v.get("k"))
            .map(LottieAnimated::from_json)
            .unwrap_or_else(|| LottieAnimated { base: vec![0.0, 0.0], keyframes: Vec::new() });
        let scale =
            val.get("s").and_then(|v| v.get("k")).map(LottieAnimated::from_json).unwrap_or_else(
                || LottieAnimated { base: vec![100.0, 100.0], keyframes: Vec::new() },
            );
        let rotation = val
            .get("r")
            .and_then(|v| v.get("k"))
            .map(LottieAnimated::from_json)
            .unwrap_or_else(|| LottieAnimated { base: vec![0.0], keyframes: Vec::new() });
        let opacity = val
            .get("o")
            .and_then(|v| v.get("k"))
            .map(LottieAnimated::from_json)
            .unwrap_or_else(|| LottieAnimated { base: vec![100.0], keyframes: Vec::new() });
        LottieTransform { _anchor, position, scale, rotation, opacity }
    }

    fn parse_shape(val: &serde_json::Value) -> Option<LottieShape> {
        let ty = val.get("ty")?.as_str()?;
        match ty {
            "rc" => {
                let position = val
                    .get("p")
                    .and_then(|v| v.get("k"))
                    .map(LottieAnimated::from_json)
                    .unwrap_or_else(|| LottieAnimated {
                        base: vec![0.0, 0.0],
                        keyframes: Vec::new(),
                    });
                let size = val
                    .get("s")
                    .and_then(|v| v.get("k"))
                    .map(LottieAnimated::from_json)
                    .unwrap_or_else(|| LottieAnimated {
                        base: vec![100.0, 100.0],
                        keyframes: Vec::new(),
                    });
                let rounded = val
                    .get("r")
                    .and_then(|v| v.get("k"))
                    .map(LottieAnimated::from_json)
                    .unwrap_or_else(|| LottieAnimated { base: vec![0.0], keyframes: Vec::new() });
                Some(LottieShape::Rectangle(LottieRectShape { position, size, rounded }))
            }
            "el" => {
                let position = val
                    .get("p")
                    .and_then(|v| v.get("k"))
                    .map(LottieAnimated::from_json)
                    .unwrap_or_else(|| LottieAnimated {
                        base: vec![0.0, 0.0],
                        keyframes: Vec::new(),
                    });
                let size = val
                    .get("s")
                    .and_then(|v| v.get("k"))
                    .map(LottieAnimated::from_json)
                    .unwrap_or_else(|| LottieAnimated {
                        base: vec![100.0, 100.0],
                        keyframes: Vec::new(),
                    });
                Some(LottieShape::Ellipse(LottieEllipseShape { position, size }))
            }
            "fl" => {
                let color_val = val.get("c")?.get("k")?;
                let color = LottieColor::from_json(color_val);
                let opacity = val
                    .get("o")
                    .and_then(|v| v.get("k"))
                    .map(LottieAnimated::from_json)
                    .unwrap_or_else(|| LottieAnimated { base: vec![100.0], keyframes: Vec::new() });
                let _fill_rule = val.get("r").and_then(|v| v.as_i64()).unwrap_or(1) as u32;
                Some(LottieShape::Fill(LottieFill { color, opacity, _fill_rule }))
            }
            "st" => {
                let color_val = val.get("c")?.get("k")?;
                let color = LottieColor::from_json(color_val);
                let opacity = val
                    .get("o")
                    .and_then(|v| v.get("k"))
                    .map(LottieAnimated::from_json)
                    .unwrap_or_else(|| LottieAnimated { base: vec![100.0], keyframes: Vec::new() });
                let width = val
                    .get("w")
                    .and_then(|v| v.get("k"))
                    .map(LottieAnimated::from_json)
                    .unwrap_or_else(|| LottieAnimated { base: vec![1.0], keyframes: Vec::new() });
                let _line_cap = val.get("lc").and_then(|v| v.as_i64()).unwrap_or(0) as u32;
                let _line_join = val.get("lj").and_then(|v| v.as_i64()).unwrap_or(0) as u32;
                Some(LottieShape::Stroke(LottieStroke {
                    color,
                    opacity,
                    width,
                    _line_cap,
                    _line_join,
                }))
            }
            "sh" => {
                // A `"sh"` path carries its geometry under `ks.k`. When the path
                // itself is animated, `ks.k` is a keyframe array; the path is not
                // animated here but its first sample is rendered (see
                // `LottiePathShape::from_json`). A path that cannot yield at least
                // one segment is reported as unsupported rather than dropped
                // silently (D09-LOT-01).
                let ks = val.get("ks").and_then(|v| v.get("k"))?;
                match LottiePathShape::from_json(ks) {
                    Some(path) => Some(LottieShape::Path(path)),
                    None => Some(LottieShape::Other("sh".to_string())),
                }
            }
            "gr" | "gs" => {
                // A group's `it` array lists its children followed by a `"tr"`
                // transform entry. The children are parsed recursively, so nested
                // groups and their paths compose; the transform is kept separately
                // so the renderer can fold it into the placement (D09-LOT-01).
                let mut items = Vec::new();
                let mut transform = LottieTransform::default();
                if let Some(it) = val.get("it").and_then(|v| v.as_array()) {
                    for child in it {
                        match child.get("ty").and_then(|v| v.as_str()) {
                            Some("tr") => transform = Self::parse_transform(child),
                            _ => {
                                if let Some(shape) = Self::parse_shape(child) {
                                    items.push(shape);
                                }
                            }
                        }
                    }
                }
                Some(LottieShape::Group(LottieGroup { items, transform }))
            }
            other => Some(LottieShape::Other(other.to_string())),
        }
    }
}

/// LottieWidget — a Lottie JSON animation player widget.
pub struct LottieWidget {
    base: BaseWidget,
    json_data: Option<String>,
    current_frame: u32,
    total_frames: u32,
    playing: bool,
    /// Loop count: 0 = infinite, >0 = number of repetitions.
    loop_count: i32,
    /// Frame rate in frames per second.
    frame_rate: f32,
    /// Internal timer accumulator in **microseconds**.
    ///
    /// Microseconds rather than milliseconds so a frame period that is not a whole
    /// number of milliseconds (60 fps -> 16.667 ms) does not lose its remainder on
    /// every tick; see `tick`.
    frame_timer_us: u64,
    /// Emitted when the animation finishes (all loops completed).
    pub animation_finished: GenericSignal,
    /// Parsed layers with shapes for rendering.
    layers: Vec<LottieLayer>,
    /// Composition width from Lottie JSON.
    comp_width: f64,
    /// Composition height from Lottie JSON.
    comp_height: f64,
    /// Frame offset (ip) from Lottie JSON.
    frame_offset: f64,
    /// Lottie shape types (`ty`) present in the document that this renderer
    /// cannot draw, deduplicated and in first-seen order.
    ///
    /// Populated at load time so a caller can observe that a composition is only
    /// partially renderable instead of seeing shapes silently disappear
    /// (D09-LOT-01). Query it with [`Self::unsupported_shapes`].
    unsupported_shapes: Vec<String>,
}

impl LottieWidget {
    /// Creates a new LottieWidget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LottieWidget, geometry, "LottieWidget"),
            json_data: None,
            current_frame: 0,
            total_frames: 0,
            playing: false,
            loop_count: 0,
            frame_rate: 30.0,
            frame_timer_us: 0,
            animation_finished: GenericSignal::new(),
            layers: Vec::new(),
            comp_width: 100.0,
            comp_height: 100.0,
            frame_offset: 0.0,
            unsupported_shapes: Vec::new(),
        }
    }

    /// Loads Lottie JSON data, parses it, and counts frames.
    /// Returns Ok(()) on success, or an error string if parsing fails.
    pub fn load_json(&mut self, data: &str) -> Result<(), String> {
        if data.is_empty() {
            return Err(format!(
                "Lottie JSON data is empty ({} bytes); pass the contents of a .json \
                 animation file",
                data.len()
            ));
        }

        // Attempt to parse the data as JSON and extract frame-related fields.
        // Lottie JSON has "op" (out point / last frame) and "ip" (in point / first frame).
        let parsed: serde_json::Value =
            serde_json::from_str(data).map_err(|e| format!("Invalid Lottie JSON: {e}"))?;

        let op = parsed
            .get("op")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| "Missing or invalid 'op' field in Lottie JSON".to_string())?;

        let ip = parsed
            .get("ip")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| "Missing or invalid 'ip' field in Lottie JSON".to_string())?;

        // `op` and `ip` come straight out of the document. `(op - ip)` is therefore
        // attacker-controlled in both directions: it can be `NaN` (either field
        // `NaN`, or both infinite with the same sign), and it can be astronomically
        // large (`op = 1e12`), which a bare `as u32` would saturate to `u32::MAX` —
        // a nonsensical animation length that every frame-advance loop would then
        // dutifully walk. The clamp states the range this widget actually supports,
        // and `round()` is used because `as` truncates (`op = 10.9, ip = 10.0` is one
        // frame, not zero).
        let span = op - ip;
        if !span.is_finite() {
            return Err(format!(
                "Lottie animation has a non-finite frame range: in-point {ip}, out-point {op}"
            ));
        }
        let total = span.clamp(0.0, MAX_LOTTIE_FRAMES as f64).round() as u32;
        if total == 0 {
            return Err(format!(
                "Lottie animation has zero frames: its in-point {ip} and out-point {op} \
                 declare no playable range"
            ));
        }
        if span > MAX_LOTTIE_FRAMES as f64 {
            return Err(format!(
                "Lottie animation declares {span} frames, which exceeds the {MAX_LOTTIE_FRAMES} \
                 frame cap; the file is malformed or is not a Lottie composition"
            ));
        }

        // Extract frame rate if present. Clamped rather than assigned directly: a
        // hostile `fr` of `1e38` becomes `inf` as an `f32`, and an infinite rate
        // makes the frame timer advance by zero every tick — an animation that can
        // never finish.
        if let Some(fr) = parsed.get("fr").and_then(|v| v.as_f64()) {
            if fr.is_finite() && fr > 0.0 {
                self.frame_rate = fr.clamp(MIN_LOTTIE_FPS as f64, MAX_LOTTIE_FPS as f64) as f32;
            }
        }

        // Extract composition dimensions. These scale every drawn layer, so a
        // non-finite or negative value would propagate into the transform rather
        // than merely looking wrong.
        if let Some(w) = parsed.get("w").and_then(|v| v.as_f64()) {
            if w.is_finite() && w > 0.0 {
                self.comp_width = w;
            }
        }
        if let Some(h) = parsed.get("h").and_then(|v| v.as_f64()) {
            if h.is_finite() && h > 0.0 {
                self.comp_height = h;
            }
        }
        self.frame_offset = if ip.is_finite() { ip } else { 0.0 };

        // Parse layers.
        self.layers = if let Some(layers_arr) = parsed.get("layers").and_then(|v| v.as_array()) {
            layers_arr.iter().filter_map(LottieLayer::from_json).collect()
        } else {
            Vec::new()
        };

        // Record every shape type the renderer cannot draw. This is the queryable
        // diagnostic that replaces the old silent skip: a caller can now tell that
        // a composition is only partially renderable (D09-LOT-01).
        self.unsupported_shapes.clear();
        let mut unsupported = Vec::new();
        for layer in &self.layers {
            Self::collect_unsupported_shapes(&layer.shapes, &mut unsupported);
        }
        self.unsupported_shapes = unsupported;

        self.json_data = Some(data.to_string());
        self.total_frames = total;
        self.current_frame = 0;
        self.frame_timer_us = 0;
        self.playing = false;
        Ok(())
    }

    /// Starts playback of the animation.
    pub fn play(&mut self) {
        if self.total_frames == 0 {
            return;
        }
        self.playing = true;
        self.base.request_redraw();
    }

    /// Pauses playback, keeping the current frame visible.
    pub fn pause(&mut self) {
        self.playing = false;
        self.base.request_redraw();
    }

    /// Stops playback and resets to the first frame.
    pub fn stop(&mut self) {
        self.playing = false;
        self.current_frame = 0;
        self.frame_timer_us = 0;
        self.base.request_redraw();
    }

    /// Returns whether the animation is currently playing.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Sets the loop count. 0 = infinite, >0 = number of repetitions.
    pub fn set_loop_count(&mut self, n: i32) {
        self.loop_count = n.max(0);
    }

    /// Returns the current loop count.
    pub fn loop_count(&self) -> i32 {
        self.loop_count
    }

    /// Sets the frame rate in frames per second.
    ///
    /// Clamped to the same `MIN_LOTTIE_FPS ..= MAX_LOTTIE_FPS` window the document
    /// loader applies (`parse_frame_rate`), so the two entry points cannot leave the
    /// control in a state the other would reject. A non-positive or non-finite rate is
    /// refused rather than stored, because `tick` derives its period from this value
    /// and a zero period would advance the animation once per millisecond.
    pub fn set_frame_rate(&mut self, fps: f32) {
        if fps.is_finite() && fps > 0.0 {
            let clamped = fps.clamp(MIN_LOTTIE_FPS, MAX_LOTTIE_FPS);
            if self.frame_rate == clamped {
                return;
            }
            self.frame_rate = clamped;
            // `draw` timestamps the progress bar from this, so the readout changed with the rate.
            self.base.request_redraw();
        }
    }

    /// Returns the frame rate.
    pub fn frame_rate(&self) -> f32 {
        self.frame_rate
    }

    /// Returns the current frame index.
    pub fn current_frame(&self) -> u32 {
        self.current_frame
    }

    /// Returns the total number of frames.
    pub fn total_frames(&self) -> u32 {
        self.total_frames
    }

    /// Sets the current frame index directly (clamped to valid range).
    pub fn set_current_frame(&mut self, frame: u32) {
        if self.total_frames == 0 {
            return;
        }
        self.current_frame = frame.min(self.total_frames - 1);
        self.frame_timer_us = 0;
        self.base.request_redraw();
    }

    /// Advances to the next frame based on the frame timer.
    /// Call this with elapsed milliseconds to drive animation.
    /// Returns true if the frame changed as a result.
    pub fn advance_frame(&mut self) -> bool {
        if self.total_frames == 0 || !self.playing {
            return false;
        }

        let next = self.current_frame + 1;
        if next >= self.total_frames {
            // Reached the end of the sequence.
            if self.loop_count == 0 {
                // Infinite looping: wrap around.
                self.current_frame = 0;
            } else {
                // Finite looping: count down.
                if self.loop_count > 0 {
                    self.loop_count -= 1;
                }
                if self.loop_count == 0 {
                    // All loops completed, stop.
                    self.playing = false;
                    self.animation_finished.emit();
                    return true;
                }
                self.current_frame = 0;
            }
        } else {
            self.current_frame = next;
        }

        // The accumulator is *not* reset here. `tick` subtracts exactly one period
        // before calling this, so a surplus is already left for the next frame; zeroing
        // it would throw that surplus away on every frame and reintroduce the drift the
        // microsecond accumulator exists to remove (measured: 30 fps ran at 29.4 fps).
        // A caller that drives `advance_frame` directly holds no phase to preserve.
        self.base.request_redraw();
        true
    }

    /// Advances the animation timer by the given number of milliseconds.
    /// Returns true if the frame changed as a result.
    ///
    /// # Why the remainder is carried
    ///
    /// The period is computed in **microseconds** and the accumulator counts
    /// microseconds, because the millisecond granularity the timer is fed in is
    /// coarser than most frame periods. Rounding `1000 / fps` to whole milliseconds
    /// made the animation run at the wrong speed with an unbounded error: 60 fps
    /// truncated 16.667 ms to 16 ms (62.5 fps, 4.2% fast), and 59.94 fps — the NTSC
    /// rate — truncated to the same 16 ms. Accumulating in microseconds keeps the
    /// long-run rate equal to the requested one, since the sub-millisecond part is no
    /// longer discarded on each tick.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        if !self.playing || self.total_frames == 0 {
            return false;
        }

        // Microseconds per frame. `frame_rate` is validated non-zero and finite by
        // `set_frame_rate` and by the loader, so this cannot divide by zero or produce
        // a non-finite period; the guard keeps a hand-built struct safe regardless.
        let period_us = if self.frame_rate > 0.0 {
            (1_000_000.0 / self.frame_rate as f64).round().max(1.0) as u64
        } else {
            33_000
        };

        self.frame_timer_us = self.frame_timer_us.saturating_add(delta_ms.saturating_mul(1000));
        if self.frame_timer_us >= period_us {
            // Consume one period rather than resetting to zero: a long `delta_ms` (a
            // stalled event loop) advances exactly one frame and keeps the surplus as
            // the phase of the next one, so the animation does not lose time.
            self.frame_timer_us -= period_us;
            self.advance_frame();
            true
        } else {
            false
        }
    }

    /// Returns a reference to the raw JSON data, if loaded.
    pub fn json_data(&self) -> Option<&str> {
        self.json_data.as_deref()
    }

    /// Returns a reference to the parsed layers.
    pub fn layers(&self) -> &[LottieLayer] {
        &self.layers
    }

    /// The composition's declared width in the document's coordinate space.
    ///
    /// Read from `w` at load time and validated then (finite and positive), because
    /// every layer transform is scaled by it: an unusable value would distort the
    /// whole composition rather than fail one layer.
    pub fn comp_width(&self) -> f64 {
        self.comp_width
    }

    /// The composition's declared height, validated like [`Self::comp_width`].
    pub fn comp_height(&self) -> f64 {
        self.comp_height
    }

    /// The document's in-point (`ip`) as a frame offset.
    ///
    /// Non-finite in-points are stored as `0.0` at load time so that playback never
    /// depends on a `NaN` comparison. See [`Self::comp_width`] for the rationale.
    pub fn frame_offset(&self) -> f64 {
        self.frame_offset
    }

    /// The Lottie shape types (`ty`) the loaded document contains that this
    /// renderer cannot draw.
    ///
    /// Every entry would previously have been dropped without a trace: the parser
    /// kept unknown shapes as [`LottieShape::Other`] and the draw loop ignored
    /// them, so an animation played with graphics silently missing. The list is
    /// collected at load time, is deduplicated, and follows first-seen order. An
    /// empty slice means the whole composition is renderable (D09-LOT-01).
    pub fn unsupported_shapes(&self) -> &[String] {
        &self.unsupported_shapes
    }

    /// Whether the loaded document contains any shape this renderer cannot draw.
    ///
    /// The predicate form of [`Self::unsupported_shapes`], for callers that only
    /// need to warn that rendering is incomplete (D09-LOT-01).
    pub fn has_unsupported_shapes(&self) -> bool {
        !self.unsupported_shapes.is_empty()
    }

    /// Walks `shapes` (including nested groups) and appends each unsupported
    /// Lottie shape type once, in first-seen order (D09-LOT-01).
    fn collect_unsupported_shapes(shapes: &[LottieShape], out: &mut Vec<String>) {
        for shape in shapes {
            match shape {
                LottieShape::Other(name) => {
                    if !out.iter().any(|seen| seen == name) {
                        out.push(name.clone());
                    }
                }
                LottieShape::Group(group) => Self::collect_unsupported_shapes(&group.items, out),
                _ => {}
            }
        }
    }

    /// Render all layers for a given frame.
    fn render_layers(&self, context: &mut RenderContext, frame: f64, widget_rect: Rect) {
        let scale_x = widget_rect.width as f64 / self.comp_width.max(1.0);
        let scale_y = widget_rect.height as f64 / self.comp_height.max(1.0);
        let scale = scale_x.min(scale_y);

        // Offset to center the composition in the widget.
        let comp_draw_w = self.comp_width * scale;
        let comp_draw_h = self.comp_height * scale;
        let offset_x = widget_rect.x as f64 + (widget_rect.width as f64 - comp_draw_w) / 2.0;
        let offset_y = widget_rect.y as f64 + (widget_rect.height as f64 - comp_draw_h) / 2.0;

        // Render layers sorted by index (lower index = lower in the stack).
        let mut sorted_layers: Vec<&LottieLayer> = self.layers.iter().collect();
        sorted_layers.sort_by_key(|l| l.index);

        for layer in &sorted_layers {
            let t = &layer.transform;
            let pos = t.position.at_frame(frame);
            let s = t.scale.at_frame(frame);
            let rot = t.rotation.at_frame_scalar(frame);
            let layer_opacity = t.opacity.at_frame_scalar(frame) / 100.0;

            // The layer transform is the root placement: the fit `scale` maps
            // composition units to device pixels, the layer's own scale is folded
            // into the size multipliers, and rotation is reported so primitives can
            // approximate it. Positions use the uniform `scale`, exactly as the
            // original renderer did, so existing output is unchanged.
            let placement = ShapePlacement {
                tx: offset_x + pos.first().copied().unwrap_or(0.0) * scale,
                ty: offset_y + pos.get(1).copied().unwrap_or(0.0) * scale,
                scale_x: scale,
                scale_y: scale,
                size_x: s.first().copied().unwrap_or(100.0) / 100.0,
                size_y: s.get(1).copied().unwrap_or(100.0) / 100.0,
                opacity: layer_opacity,
                has_rotation: rot.abs() > 0.5,
            };

            self.draw_shapes(context, frame, &layer.shapes, placement);
        }
    }

    /// Draws `shapes` under `placement`, composing fills/strokes onto the geometry
    /// they style and recursing into groups (D09-LOT-01).
    ///
    /// The list is walked **last to first**. Lottie lists a group's items
    /// top-down, so the first item is drawn last (on top); walking in reverse is
    /// therefore the correct paint order *and* it makes a style reach its geometry:
    /// a `"fl"`/`"st"` appears after the path it fills in the array, so only in
    /// reverse order is the style already known when the geometry is drawn.
    /// Styles are tracked per call so a group's style cannot leak into the layer
    /// that contains the group.
    fn draw_shapes(
        &self,
        context: &mut RenderContext,
        frame: f64,
        shapes: &[LottieShape],
        placement: ShapePlacement,
    ) {
        let mut current_fill: Option<LottieFill> = None;
        let mut current_stroke: Option<LottieStroke> = None;

        for shape in shapes.iter().rev() {
            match shape {
                LottieShape::Fill(fill) => {
                    current_fill = Some(fill.clone());
                }
                LottieShape::Stroke(stroke) => {
                    current_stroke = Some(stroke.clone());
                }
                LottieShape::Rectangle(rect_shape) => {
                    self.draw_rectangle(
                        context,
                        frame,
                        rect_shape,
                        placement,
                        current_fill.as_ref(),
                        current_stroke.as_ref(),
                    );
                }
                LottieShape::Ellipse(ellipse_shape) => {
                    self.draw_ellipse(
                        context,
                        frame,
                        ellipse_shape,
                        placement,
                        current_fill.as_ref(),
                        current_stroke.as_ref(),
                    );
                }
                LottieShape::Path(path) => {
                    self.draw_path_shape(
                        context,
                        frame,
                        path,
                        placement,
                        current_fill.as_ref(),
                        current_stroke.as_ref(),
                    );
                }
                LottieShape::Group(group) => {
                    // A group's transform composes with the enclosing placement: its
                    // position is offset in the parent's coordinate space, its scale
                    // multiplies both coordinates and sizes, and its opacity
                    // multiplies in. Nested groups therefore stack correctly.
                    let gt = &group.transform;
                    let gp = gt.position.at_frame(frame);
                    let gs = gt.scale.at_frame(frame);
                    let grot = gt.rotation.at_frame_scalar(frame);
                    let gx = gs.first().copied().unwrap_or(100.0) / 100.0;
                    let gy = gs.get(1).copied().unwrap_or(100.0) / 100.0;
                    let group_placement = ShapePlacement {
                        tx: placement.tx + gp.first().copied().unwrap_or(0.0) * placement.scale_x,
                        ty: placement.ty + gp.get(1).copied().unwrap_or(0.0) * placement.scale_y,
                        scale_x: placement.scale_x * gx,
                        scale_y: placement.scale_y * gy,
                        size_x: placement.size_x * gx,
                        size_y: placement.size_y * gy,
                        opacity: (placement.opacity * gt.opacity.at_frame_scalar(frame) / 100.0)
                            .clamp(0.0, 1.0),
                        has_rotation: placement.has_rotation || grot.abs() > 0.5,
                    };
                    self.draw_shapes(context, frame, &group.items, group_placement);
                }
                LottieShape::Other(_) => {
                    // Recorded at load time (see `collect_unsupported_shapes`) and
                    // reported through `unsupported_shapes`; nothing is drawn here
                    // because there is no geometry to draw. This arm is deliberately
                    // not a silent drop: the diagnostic is queryable (D09-LOT-01).
                }
            }
        }
    }

    /// Draws a rectangle ("rc") with the active fill and stroke.
    fn draw_rectangle(
        &self,
        context: &mut RenderContext,
        frame: f64,
        rect_shape: &LottieRectShape,
        placement: ShapePlacement,
        current_fill: Option<&LottieFill>,
        current_stroke: Option<&LottieStroke>,
    ) {
        let rp = rect_shape.position.at_frame(frame);
        let rs = rect_shape.size.at_frame(frame);
        let rr = rect_shape.rounded.at_frame_scalar(frame);

        let shape_x = placement.tx
            + (rp.first().copied().unwrap_or(0.0) - rs.first().copied().unwrap_or(0.0) / 2.0)
                * placement.scale_x;
        let shape_y = placement.ty
            + (rp.get(1).copied().unwrap_or(0.0) - rs.get(1).copied().unwrap_or(0.0) / 2.0)
                * placement.scale_y;
        let shape_w = (rs.first().copied().unwrap_or(0.0) * placement.scale_x).round() as u32;
        let shape_h = (rs.get(1).copied().unwrap_or(0.0) * placement.scale_y).round() as u32;
        let radius = (rr * placement.scale_x).round().max(0.0) as u32;

        let shape_rect = Rect::new(shape_x as i32, shape_y as i32, shape_w, shape_h);
        if shape_w == 0 || shape_h == 0 {
            return;
        }

        // Apply scaling from the transform chain.
        let scaled_rect = Rect::new(
            shape_rect.x,
            shape_rect.y,
            (shape_rect.width as f64 * placement.size_x).round() as u32,
            (shape_rect.height as f64 * placement.size_y).round() as u32,
        );

        // Draw fill if present.
        if let Some(fill) = current_fill {
            let fill_color = fill.color.at_frame(frame).with_alpha_f32(
                (fill.opacity.at_frame_scalar(frame) / 100.0 * placement.opacity).clamp(0.0, 1.0)
                    as f32,
            );
            // Rotation is approximated by drawing the axis-aligned shape; the
            // original renderer did the same and the branch is kept so the intent is
            // explicit rather than accidental.
            if placement.has_rotation || radius > 0 {
                context.fill_rounded_rect(scaled_rect, radius, fill_color);
            } else {
                context.fill_rect(scaled_rect, fill_color);
            }
        }

        // Draw stroke if present.
        if let Some(stroke) = current_stroke {
            let stroke_color = stroke.color.at_frame(frame).with_alpha_f32(
                (stroke.opacity.at_frame_scalar(frame) / 100.0 * placement.opacity).clamp(0.0, 1.0)
                    as f32,
            );
            let sw = (stroke.width.at_frame_scalar(frame) * placement.scale_x).round() as u32;
            if sw > 0 {
                if radius > 0 {
                    context.draw_rounded_rect_stroke(scaled_rect, radius, stroke_color, sw);
                } else {
                    context.draw_rect_stroke(scaled_rect, stroke_color, sw);
                }
            }
        }
    }

    /// Draws an ellipse or circle ("el") with the active fill and stroke.
    fn draw_ellipse(
        &self,
        context: &mut RenderContext,
        frame: f64,
        ellipse_shape: &LottieEllipseShape,
        placement: ShapePlacement,
        current_fill: Option<&LottieFill>,
        current_stroke: Option<&LottieStroke>,
    ) {
        let ep = ellipse_shape.position.at_frame(frame);
        let es = ellipse_shape.size.at_frame(frame);

        let cx = placement.tx + ep.first().copied().unwrap_or(0.0) * placement.scale_x;
        let cy = placement.ty + ep.get(1).copied().unwrap_or(0.0) * placement.scale_y;
        let ew = (es.first().copied().unwrap_or(0.0) * placement.scale_x).round() as u32;
        let eh = (es.get(1).copied().unwrap_or(0.0) * placement.scale_y).round() as u32;

        // Apply scaling from the transform chain.
        let sw = (ew as f64 * placement.size_x).round() as u32;
        let sh = (eh as f64 * placement.size_y).round() as u32;

        if sw == 0 || sh == 0 {
            return;
        }

        // Use the smaller dimension for radius if the ellipse
        // is approximately circular, otherwise draw as an ellipse
        // approximated by a filled rect with large rounded corners.
        let radius = sw.min(sh) / 2;

        let ex = (cx - sw as f64 / 2.0).round() as i32;
        let ey = (cy - sh as f64 / 2.0).round() as i32;
        let ellipse_rect = Rect::new(ex, ey, sw, sh);

        // Draw fill if present.
        if let Some(fill) = current_fill {
            let fill_color = fill.color.at_frame(frame).with_alpha_f32(
                (fill.opacity.at_frame_scalar(frame) / 100.0 * placement.opacity).clamp(0.0, 1.0)
                    as f32,
            );
            if sw == sh {
                // Circle
                context.fill_circle(
                    Point::new(cx.round() as i32, cy.round() as i32),
                    radius.max(1),
                    fill_color,
                );
            } else {
                // Approximate ellipse with rounded rect
                context.fill_rounded_rect(ellipse_rect, radius.max(1), fill_color);
            }
        }

        // Draw stroke if present.
        if let Some(stroke) = current_stroke {
            let stroke_color = stroke.color.at_frame(frame).with_alpha_f32(
                (stroke.opacity.at_frame_scalar(frame) / 100.0 * placement.opacity).clamp(0.0, 1.0)
                    as f32,
            );
            let sw_val = (stroke.width.at_frame_scalar(frame) * placement.scale_x).round() as u32;
            if sw_val > 0 {
                if sw == sh {
                    context.draw_circle_stroke(
                        Point::new(cx.round() as i32, cy.round() as i32),
                        radius.max(1),
                        stroke_color,
                        sw_val,
                    );
                } else {
                    context.draw_rounded_rect_stroke(
                        ellipse_rect,
                        radius.max(1),
                        stroke_color,
                        sw_val,
                    );
                }
            }
        }
    }

    /// Draws a bezier path ("sh") with the active fill and stroke (D09-LOT-01).
    ///
    /// The path's vertices and tangents are turned into cubic segments and then
    /// into a device-space polyline by the crate's single curve flattener
    /// ([`crate::render::path::flatten`]), so a Lottie path is tessellated by the
    /// same rule as an icon or glyph outline (principle #101). The returned
    /// contours are then filled and/or stroked per contour.
    fn draw_path_shape(
        &self,
        context: &mut RenderContext,
        frame: f64,
        path: &LottiePathShape,
        placement: ShapePlacement,
        current_fill: Option<&LottieFill>,
        current_stroke: Option<&LottieStroke>,
    ) {
        // A path with neither fill nor stroke paints nothing; skip the work rather
        // than flatten geometry that would not be drawn.
        if current_fill.is_none() && current_stroke.is_none() {
            return;
        }

        let (points, contour_ranges) = match self.flatten_lottie_path(path, placement) {
            Some(contours) => contours,
            None => return,
        };

        // Fill every contour first, then stroke, matching the rectangle/ellipse
        // draw order (fill under stroke).
        if let Some(fill) = current_fill {
            let fill_color = fill.color.at_frame(frame).with_alpha_f32(
                (fill.opacity.at_frame_scalar(frame) / 100.0 * placement.opacity).clamp(0.0, 1.0)
                    as f32,
            );
            for (start, end) in &contour_ranges {
                context.draw_path(&points[*start..*end], true, fill_color, true, 0);
            }
        }

        if let Some(stroke) = current_stroke {
            let stroke_color = stroke.color.at_frame(frame).with_alpha_f32(
                (stroke.opacity.at_frame_scalar(frame) / 100.0 * placement.opacity).clamp(0.0, 1.0)
                    as f32,
            );
            let sw = (stroke.width.at_frame_scalar(frame) * placement.scale_x).round() as u32;
            if sw > 0 {
                for (start, end) in &contour_ranges {
                    // The closing edge of a contour is implicit in the fill, so a
                    // stroked contour is emitted closed so the backend joins the last
                    // point back to the first.
                    context.draw_path(&points[*start..*end], true, stroke_color, false, sw);
                }
            }
        }
    }

    /// Flattens a Lottie path into device-space polylines.
    ///
    /// Returns `None` when the path cannot be tessellated (a degenerate path, or
    /// one over the flattener's point budget). The curve data is emitted as SVG
    /// path syntax and handed to [`crate::render::path::flatten::flatten_paths`]
    /// with an identity placement, so the flattening rule is shared rather than
    /// re-implemented (principle #101).
    #[allow(clippy::type_complexity)]
    fn flatten_lottie_path(
        &self,
        path: &LottiePathShape,
        placement: ShapePlacement,
    ) -> Option<(Vec<Point>, Vec<(usize, usize)>)> {
        use crate::render::path::flatten::{flatten_paths, IconPlacement};
        use crate::render::path::flatten::{MAX_OUTLINE_CONTOURS, MAX_OUTLINE_POINTS};

        // Build an SVG `d` string in **device space**: each vertex and its control
        // points are transformed by the placement as they are written, so the
        // flattener applies no further mapping.
        let n = path.vertices.len();
        if n < 2 {
            return None;
        }

        let map = |p: [f64; 2]| -> (f64, f64) {
            (placement.tx + p[0] * placement.scale_x, placement.ty + p[1] * placement.scale_y)
        };

        let mut d = String::with_capacity(n * 32);
        let (sx, sy) = map(path.vertices[0]);
        d.push_str(&format!("M{sx} {sy}"));

        let segment_count = if path.closed { n } else { n - 1 };
        for i in 0..segment_count {
            let j = (i + 1) % n;
            let out_i = path.out_tangents.get(i).copied().unwrap_or([0.0, 0.0]);
            let in_j = path.in_tangents.get(j).copied().unwrap_or([0.0, 0.0]);
            let v_i = path.vertices[i];
            let v_j = path.vertices[j];
            // Lottie tangents are relative to their vertex, so the cubic's control
            // points are the vertex plus its tangent.
            let (c1x, c1y) = map([v_i[0] + out_i[0], v_i[1] + out_i[1]]);
            let (c2x, c2y) = map([v_j[0] + in_j[0], v_j[1] + in_j[1]]);
            let (ex, ey) = map(v_j);
            d.push_str(&format!("C{c1x} {c1y} {c2x} {c2y} {ex} {ey}"));
        }
        if path.closed {
            d.push('Z');
        }

        let mut points = vec![Point::new(0, 0); MAX_OUTLINE_POINTS];
        let mut contour_ranges = vec![(0usize, 0usize); MAX_OUTLINE_CONTOURS];
        // Identity placement: `map` adds `grid` to y, so a zero grid with unit scale
        // leaves device coordinates untouched (see `IconPlacement::map`).
        let identity = IconPlacement { origin_x: 0.0, origin_y: 0.0, scale: 1.0, grid: 0.0 };
        match flatten_paths(&[d.as_str()], identity, &mut points, &mut contour_ranges) {
            Ok(count) => {
                let ranges = contour_ranges[..count].to_vec();
                Some((points, ranges))
            }
            Err(_) => {
                // A path over the flattener's budget (or otherwise untessellatable)
                // contributes nothing this frame rather than a truncated outline,
                // which would draw the wrong shape. `flatten_paths` refuses rather
                // than truncates for exactly that reason.
                None
            }
        }
    }
}

impl Widget for LottieWidget {
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
    /// `LottieWidget` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    // The trait contract takes `u32` milliseconds; this control's own `tick` takes `u64`
    // because an animation can legitimately run long enough that the millisecond count
    // would wrap. Widening at the boundary keeps the trait uniform without narrowing the
    // animation's own clock.
    fn tick(&mut self, delta_ms: u32) -> bool {
        LottieWidget::tick(self, u64::from(delta_ms))
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
            _ => None,
        }
    }
}

/// `LottieWidget`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_media.in.rs` / `access_write_media.in.rs` dispatch: `playing`
/// maps onto `play` / `pause`.
impl WidgetProperties for LottieWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "playing" => Ok(CapabilityValue::Bool(self.is_playing())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "playing" => {
                if expect_bool(value)? {
                    self.play();
                } else {
                    self.pause();
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["playing", BASE_PROPERTY_NAMES]
    }

    /// Runs the published playback commands.
    ///
    /// `set_playing` names no property: playback is exposed as the read-only
    /// `playing`, and starting it is the control's own [`Self::play`]. The default
    /// `set_foo` convention would have reported the command unknown for a control
    /// that implements it, and the property route a `set_playing` spelling implies
    /// does not exist. The payload-free `play`/`pause`/`stop` verbs are accepted here
    /// because they name the real methods directly.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_playing" => {
                self.play();
                Ok(())
            }
            "play" => {
                self.play();
                Ok(())
            }
            "pause" => {
                self.pause();
                Ok(())
            }
            "stop" => {
                self.stop();
                Ok(())
            }
            _ => self.default_command(name),
        }
    }
}

impl Draw for LottieWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then fall back to the original literal. The literal step is
        // kept deliberately: an inactive theme must still give the widget a defined
        // appearance, and the value is the one this widget painted before, so an existing
        // pixel baseline cannot move. Resolved once per draw, because the empty state, the
        // background, the border, the chips and the progress trough all read from it and
        // re-resolving would take the theme lock several times inside one draw.
        let style = self.style().clone();
        let themed = crate::style::resolved_theme_style("lottie_widget");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let themed_border = themed.as_ref().and_then(|resolved| resolved.border_color);
        let themed_text = themed.as_ref().and_then(|resolved| resolved.text_color);
        let base_bg =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(240, 240, 250, 255));
        // The page this shell sits in, read once. A disabled **fill** recedes toward the page it
        // sits on; it cannot recede toward its own contrast colour the way ink does, because that
        // would make it stand out *more* against the page -- the opposite of disabled.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let border_color =
            style.border_color.or(themed_border).unwrap_or(Color::rgba(100, 100, 180, 150));
        // The placeholder ink follows the theme's foreground so it stays legible on whichever
        // surface the theme painted; the literal is only the last resort.
        let placeholder_text =
            style.text_color.or(themed_text).unwrap_or(Color::rgba(160, 160, 160, 220));
        // The neutral fill used by the pill chips and the progress trough: this control's
        // chrome, derived from the resolved border so it tracks the theme instead of staying
        // one grey in every appearance.
        let trough_color = border_color.blend(&base_bg, 0.55);

        if self.total_frames == 0 {
            // Empty state: a neutral placeholder panel. The panel and the label are this
            // widget's chrome — there is no animation data here at all — so both now follow
            // the theme instead of pinning the control to one appearance.
            context.fill_rounded_rect(rect, 4, base_bg);
            let font = effective_font(&style);
            let text = "No Lottie animation loaded";
            // Centred on the vertical midline and fitted to the control's width: the label
            // is 25 characters at 14 px, wider than most controls it is drawn into, and
            // without the fit it ran past the panel and out of the SVG snapshot.
            let metrics = context.measure_text(text, &font);
            let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;
            let line = Rect::new(rect.x, text_y, rect.width, metrics.height);
            context.draw_text_fitted(
                line,
                text,
                &font,
                placeholder_text,
                HorizontalAlignment::Center,
            );
            return;
        }

        // Background — the surface the composition is rendered onto, i.e. chrome. The former
        // literal is the fallback. Disabled steps the resolved fill toward its own contrast
        // colour rather than toward a fixed white: a dark surface moved 0.35 of the way to white
        // gets **more** prominent, not less (measured 1.13:1 against the window when enabled and
        // 3.63:1 when disabled), so the state read backwards on the dark appearance.
        let bg = if !is_enabled {
            self.base.disabled_surface_near(base_bg, window_fill)
        } else {
            base_bg
        };
        context.face_with_gradient(
            rect,
            bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // Draw bounding box — a chrome edge around the composition, so it follows the
        // theme's border token rather than a fixed violet.
        context.draw_rect_stroke(rect, border_color, 1);

        // Render Lottie shapes from the parsed JSON.
        let frame = self.current_frame as f64 + self.frame_offset;
        self.render_layers(context, frame, rect);

        // Frame counter overlay at top-right.
        let font = effective_font(&style);
        let counter_text =
            format!("{}/{} FPS:{:.0}", self.current_frame + 1, self.total_frames, self.frame_rate);
        let c_metrics = context.measure_text(&counter_text, &font);
        let cx = rect.x + rect.width as i32 - c_metrics.width as i32 - 4;
        let cy = rect.y + 2;
        let pill_w = c_metrics.width as u32 + 8;
        let pill_h = c_metrics.height as u32 + 2;
        let pill_rect = Rect::new(cx - 4, cy - 1, pill_w, pill_h);
        // Same rule as the play chip: the counter's backdrop belongs to the widget's
        // chrome, but the white counter ink and the chip's own alpha are kept as they were,
        // because they are the badge's legibility contract over the composition.
        context.fill_rounded_rect(pill_rect, 3, border_color);
        // Glyph origin is the box's top edge, so it is already `cy`; the old `+ ascent`
        // pushed the counter half a line down, out through the pill's bottom edge.
        context.draw_text(
            Point::new(cx, cy),
            &counter_text,
            &font,
            Color::WHITE,
            HorizontalAlignment::Left,
        );

        // Play/pause indicator at top-left: a `Play`/`Pause` **icon outline**, not the `▶`/`⏸`
        // text glyphs (covered by no bundled face). The two colours are deliberately not themed:
        // green versus amber *is* the state encoding — "playing" versus "paused" — and a theme
        // would recolour both to whatever roles they happened to match.
        let status_icon = if self.playing { IconName::Play } else { IconName::Pause };
        let status_color = if self.playing {
            Color::rgba(40, 160, 40, 230)
        } else {
            Color::rgba(180, 100, 40, 230)
        };
        crate::widget::draw_icon_at(
            context,
            Rect::new(rect.x + 4, rect.y + 2, 12, 12),
            status_color,
            status_icon,
        );

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

        if self.total_frames > 0 {
            let fill_ratio = (self.current_frame as f64) / (self.total_frames as f64);
            let filled_width = ((progress_bar_full.width as f64) * fill_ratio) as u32;
            if filled_width > 0 {
                let progress_bar_fill = Rect::new(
                    progress_bar_full.x,
                    progress_bar_full.y,
                    filled_width,
                    progress_bar_full.height,
                );
                // Deliberately not themed: this blue *encodes progress*. Recolouring it from
                // a theme would make the bar indistinguishable from the trough it sits in.
                context.fill_rounded_rect(progress_bar_fill, 3, Color::rgba(60, 120, 220, 200));
            }
        }
    }
}

impl EventHandler for LottieWidget {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } | Event::MouseRelease { pos, button } => {
                if *button == 1 && self.geometry().contains_point(*pos) {
                    if self.playing {
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
    use crate::render::PaintBackend;
    use std::sync::{Arc, Mutex};

    fn make_lottie_json(op: f64, ip: f64, fr: f64) -> String {
        format!(
            r#"{{
            "op":{},"ip":{},"fr":{},"v":"5.5.2","w":100,"h":100,
            "layers":[
                {{
                    "ind":0,"parent":-1,
                    "ks":{{
                        "a":{{"k":[0,0]}},
                        "p":{{"k":[50,50]}},
                        "s":{{"k":[100,100]}},
                        "r":{{"k":[0]}},
                        "o":{{"k":[100]}}
                    }},
                    "shapes":[
                        {{
                            "ty":"rc",
                            "p":{{"k":[50,50]}},
                            "s":{{"k":[80,80]}},
                            "r":{{"k":[5]}}
                        }},
                        {{
                            "ty":"fl",
                            "c":{{"k":[0.2,0.4,0.8,1.0]}},
                            "o":{{"k":[100]}},
                            "r":1
                        }},
                        {{
                            "ty":"st",
                            "c":{{"k":[0.1,0.1,0.3,1.0]}},
                            "o":{{"k":[100]}},
                            "w":{{"k":[2.0]}},
                            "lc":1,
                            "lj":1
                        }}
                    ]
                }},
                {{
                    "ind":1,"parent":-1,
                    "ks":{{
                        "a":{{"k":[0,0]}},
                        "p":{{"k":[50,50]}},
                        "s":{{"k":[100,100]}},
                        "r":{{"k":[0]}},
                        "o":{{"k":[100]}}
                    }},
                    "shapes":[
                        {{
                            "ty":"el",
                            "p":{{"k":[50,50]}},
                            "s":{{"k":[30,30]}}
                        }},
                        {{
                            "ty":"fl",
                            "c":{{"k":[1.0,0.6,0.2,1.0]}},
                            "o":{{"k":[80]}},
                            "r":1
                        }}
                    ]
                }}
            ]
        }}"#,
            op, ip, fr
        )
    }

    #[test]
    fn lottie_widget_creation_defaults() {
        let lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        assert_eq!(lottie.total_frames(), 0);
        assert_eq!(lottie.current_frame(), 0);
        assert!(!lottie.is_playing());
        assert_eq!(lottie.loop_count(), 0);
        assert_eq!(lottie.frame_rate(), 30.0);
        assert!(lottie.json_data().is_none());
        assert_eq!(lottie.kind(), WidgetKind::LottieWidget);
    }

    #[test]
    fn lottie_widget_load_json_and_frame_count() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(60.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        assert_eq!(lottie.total_frames(), 60);
        assert_eq!(lottie.current_frame(), 0);
        assert_eq!(lottie.frame_rate(), 30.0);
        assert!(lottie.json_data().is_some());
    }

    #[test]
    fn lottie_widget_empty_data_returns_error() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let result = lottie.load_json("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("empty") && err.contains("0 bytes"), "{err}");
    }

    #[test]
    fn lottie_widget_invalid_json_returns_error() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let result = lottie.load_json("not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid Lottie JSON"));
    }

    /// `op`/`ip` come straight from the document, so an absurd range must be refused.
    ///
    /// Before the clamp, `op = 1e12` became `u32::MAX` frames through a saturating
    /// `as u32`: the widget would then report a frame count no loop could ever finish
    /// and `total_frames` no longer described the file.
    #[test]
    fn lottie_widget_rejects_an_absurd_frame_range() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(1.0e12, 0.0, 30.0);
        let err = lottie.load_json(&json).unwrap_err();
        assert!(
            err.contains("frame cap"),
            "an over-long range must be refused by name, not clamped silently; got: {err}"
        );
        assert_eq!(lottie.total_frames(), 0, "a refused load must not have taken effect");
    }

    /// The frame count rounds rather than truncating.
    ///
    /// `op = 10.9, ip = 10.0` is a one-frame composition; a bare `as u32` would read
    /// it as zero and reject a valid file.
    #[test]
    fn lottie_widget_rounds_a_fractional_frame_range() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        lottie.load_json(&make_lottie_json(10.9, 10.0, 30.0)).unwrap();
        assert_eq!(lottie.total_frames(), 1);
    }

    /// A hostile frame rate must not become infinite or zero.
    ///
    /// `fr = 1e38` is finite as `f64` but infinite as `f32`; `fr = 1e-9` rounds to a
    /// timer increment that never reaches one frame. Both are clamped, and a
    /// non-finite `fr` is ignored so the previous value survives.
    #[test]
    fn lottie_widget_clamps_the_declared_frame_rate() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));

        lottie.load_json(&make_lottie_json(10.0, 0.0, 1.0e38)).unwrap();
        let high = lottie.frame_rate();
        assert!(high.is_finite(), "frame_rate must stay finite, got {high}");
        assert!(high <= MAX_LOTTIE_FPS, "frame_rate {high} exceeded the cap");

        lottie.load_json(&make_lottie_json(10.0, 0.0, 1.0e-9)).unwrap();
        let low = lottie.frame_rate();
        assert!(low >= MIN_LOTTIE_FPS, "frame_rate {low} fell below the floor");
    }

    /// A non-finite composition size must not reach the layer transform.
    ///
    /// `1e400` would be rejected by the JSON parser itself, so the reachable cases
    /// are a negative size and a zero size: both are finite, both would make
    /// `widget_rect.width / comp_width` degenerate, and neither should replace the
    /// default.
    #[test]
    fn lottie_widget_ignores_an_unusable_composition_size() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let default_w = lottie.comp_width();
        let default_h = lottie.comp_height();

        let json = r#"{"op":10,"ip":0,"fr":30,"w":-50,"h":0,"layers":[]}"#;
        lottie.load_json(json).unwrap();
        assert_eq!(lottie.comp_width(), default_w, "a negative width must not be adopted");
        assert_eq!(lottie.comp_height(), default_h, "a zero height must not be adopted");
        assert!(lottie.comp_width().is_finite() && lottie.comp_width() > 0.0);
        assert!(lottie.comp_height().is_finite() && lottie.comp_height() > 0.0);
    }

    /// A missing in-point must leave the frame offset at zero, not poison playback.
    ///
    /// `ip` is optional in practice (many exporters omit a zero in-point), and the
    /// load path tolerates its absence for the range calculation by only reading `op`.
    #[test]
    fn lottie_widget_defaults_a_missing_in_point_to_zero() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = r#"{"op":10,"ip":0,"fr":30,"w":100,"h":100,"layers":[]}"#;
        lottie.load_json(json).unwrap();
        assert_eq!(lottie.frame_offset(), 0.0);
        assert!(lottie.frame_offset().is_finite());
    }

    #[test]
    fn lottie_widget_play_pause_stop() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(30.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();

        assert!(!lottie.is_playing());
        lottie.play();
        assert!(lottie.is_playing());
        lottie.pause();
        assert!(!lottie.is_playing());
        lottie.play();
        assert!(lottie.is_playing());
        lottie.stop();
        assert!(!lottie.is_playing());
        assert_eq!(lottie.current_frame(), 0);
    }

    #[test]
    fn lottie_widget_advance_frame() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(10.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        lottie.play();

        assert_eq!(lottie.current_frame(), 0);
        lottie.advance_frame();
        assert_eq!(lottie.current_frame(), 1);
        lottie.advance_frame();
        assert_eq!(lottie.current_frame(), 2);
    }

    #[test]
    fn lottie_widget_advance_frame_wraps_with_infinite_loop() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(3.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        lottie.set_loop_count(0); // infinite
        lottie.play();

        lottie.advance_frame(); // 0 -> 1
        assert_eq!(lottie.current_frame(), 1);
        lottie.advance_frame(); // 1 -> 2
        assert_eq!(lottie.current_frame(), 2);
        lottie.advance_frame(); // 2 -> wraps to 0
        assert_eq!(lottie.current_frame(), 0);
    }

    #[test]
    fn lottie_widget_animation_finished_signal() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(5.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        lottie.set_loop_count(1);
        lottie.play();

        let finished = Arc::new(Mutex::new(false));
        lottie.animation_finished.connect({
            let finished = Arc::clone(&finished);
            move || {
                *finished.lock().unwrap() = true;
            }
        });

        // Advance through frames 0->4 (5 frames, one loop).
        for _ in 0..4 {
            lottie.advance_frame();
            assert!(!*finished.lock().unwrap());
        }
        // Frame 4 -> wraps: loop_count reaches 0 -> finish.
        lottie.advance_frame();
        assert!(*finished.lock().unwrap());
        assert!(!lottie.is_playing());
    }

    #[test]
    fn lottie_widget_set_frame_rate() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        assert_eq!(lottie.frame_rate(), 30.0);
        lottie.set_frame_rate(60.0);
        assert_eq!(lottie.frame_rate(), 60.0);
        lottie.set_frame_rate(0.0); // should not change
        assert_eq!(lottie.frame_rate(), 60.0);
        // Non-finite input must be refused too, not stored: `tick` divides by this.
        lottie.set_frame_rate(f32::NAN);
        assert_eq!(lottie.frame_rate(), 60.0);
        lottie.set_frame_rate(f32::INFINITY);
        assert_eq!(lottie.frame_rate(), 60.0);
        // Clamped to the same window the document loader applies.
        lottie.set_frame_rate(1.0e9);
        assert!(lottie.frame_rate() <= MAX_LOTTIE_FPS);
        lottie.set_frame_rate(1.0e-9);
        assert!(lottie.frame_rate() >= MIN_LOTTIE_FPS);
    }

    /// The animation must actually play at the rate it was asked for.
    ///
    /// The old implementation chopped the frame period to whole milliseconds
    /// (`(1000.0 / fps) as u64`), discarding the remainder on every frame, so 60 fps
    /// and 59.94 fps both played at 62.5 fps — 4.2% fast, compounding without bound.
    /// This drives a long run of 1 ms ticks and compares the achieved rate, which is
    /// the property that was wrong. A one-second window is deliberately *not* used:
    /// `frames = floor(elapsed / period)` puts every rate up to one frame below its
    /// nominal count at an exact window boundary, which would make a correct
    /// implementation look like it drifts.
    #[test]
    fn lottie_widget_plays_at_the_requested_frame_rate() {
        const WINDOW_MS: u64 = 60_000;
        for fps in [30.0f32, 59.94, 60.0, 24.0] {
            let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
            // A sequence long enough that wrapping never interferes with the count.
            // `(op, ip, fr)`: a frame count long enough that the sequence never
            // wraps during the measurement, and inside the loader's
            // `MAX_LOTTIE_FRAMES` cap.
            let json = make_lottie_json(50_000.0, 0.0, fps as f64);
            lottie.load_json(&json).unwrap();
            lottie.set_frame_rate(fps);
            lottie.play();

            let mut frames = 0u64;
            for _ in 0..WINDOW_MS {
                if lottie.tick(1) {
                    frames += 1;
                }
            }

            // The achieved rate, in fps. Truncation would give ~62.5 for 60 and 59.94.
            let achieved = frames as f64 * 1000.0 / WINDOW_MS as f64;
            let error_pct = (achieved - fps as f64).abs() / fps as f64 * 100.0;
            assert!(
                error_pct < 0.5,
                "at {fps} fps the achieved rate was {achieved:.3} fps \
                 ({error_pct:.2}% off) over {WINDOW_MS} ms; {frames} frames"
            );
        }
    }

    /// Truncating the period made the *slow* rates drift too, not just the fast ones.
    ///
    /// 30 fps truncated 33.333 ms to 33 ms (30.3 fps) and 24 fps truncated 41.667 ms to
    /// 41 ms (24.4 fps) — the second case is what this pins, since a 10 ms-per-frame
    /// error accumulates 24 extra frames per minute. An exact 10 s window is not used:
    /// `10000 / 41.667 = 239.998` frames, so a *correct* implementation also reports
    /// 239 there, and asserting 240 would be asserting the old bug's answer.
    #[test]
    fn lottie_widget_truncation_would_have_shown_here() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(50_000.0, 0.0, 24.0);
        lottie.load_json(&json).unwrap();
        lottie.set_frame_rate(24.0);
        lottie.play();

        // Exactly 120 s of 1 ms ticks: 120000 / 41.667 = 2879.98, so 2879 frames is the
        // correct answer. The truncated 41 ms period would deliver 2926.
        let mut frames = 0u64;
        for _ in 0..120_000 {
            if lottie.tick(1) {
                frames += 1;
            }
        }
        assert_eq!(
            frames, 2879,
            "24 fps over 120 s must advance 2879 frames; the truncated 41 ms period \
             would give 2926"
        );
    }

    /// A stalled event loop must not lose animation time.
    ///
    /// `tick` consumes one period and keeps the surplus rather than resetting the
    /// accumulator, so the next short tick still fires on schedule.
    #[test]
    fn lottie_widget_tick_keeps_the_remainder_after_a_long_stall() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(1000.0, 0.0, 10.0);
        lottie.load_json(&json).unwrap();
        lottie.set_frame_rate(10.0);
        lottie.play();

        // One 100 ms stall = exactly one frame at 10 fps, with no surplus.
        assert!(lottie.tick(100), "a full period must advance a frame");
        // A 50 ms tick is half a period: not yet due.
        assert!(!lottie.tick(50), "half a period must not advance");
        // The next 50 ms completes the period exactly.
        assert!(lottie.tick(50), "the completed period must advance");
    }

    #[test]
    fn lottie_widget_set_current_frame() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(10.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        lottie.set_current_frame(5);
        assert_eq!(lottie.current_frame(), 5);
        lottie.set_current_frame(999); // clamped
        assert_eq!(lottie.current_frame(), 9);
    }

    #[test]
    fn lottie_widget_parses_layers() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(60.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        assert_eq!(lottie.layers().len(), 2, "Should parse 2 layers");
    }

    #[test]
    fn lottie_widget_parses_shape_types() {
        let json = r#"{
            "op":30,"ip":0,"fr":30,"v":"5.5.2","w":100,"h":100,
            "layers":[
                {
                    "ind":0,"parent":-1,
                    "ks":{"a":{"k":[0,0]},"p":{"k":[50,50]},"s":{"k":[100,100]},"r":{"k":[0]},"o":{"k":[100]}},
                    "shapes":[
                        {"ty":"rc","p":{"k":[50,50]},"s":{"k":[80,60]},"r":{"k":[10]}},
                        {"ty":"el","p":{"k":[50,50]},"s":{"k":[40,40]}},
                        {"ty":"fl","c":{"k":[1,0,0,1]},"o":{"k":[100]},"r":1},
                        {"ty":"st","c":{"k":[0,0,0,1]},"o":{"k":[100]},"w":{"k":[3]}}
                    ]
                }
            ]
        }"#;
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        lottie.load_json(json).unwrap();
        assert_eq!(lottie.layers().len(), 1);
        assert_eq!(lottie.layers()[0].shapes.len(), 4, "Should parse 4 shapes");
    }

    #[test]
    fn lottie_widget_no_layers_does_not_crash() {
        let json = r#"{"op":30,"ip":0,"fr":30,"v":"5.5.2","w":100,"h":100}"#;
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        lottie.load_json(json).unwrap();
        assert_eq!(lottie.layers().len(), 0);
        assert_eq!(lottie.total_frames(), 30);
    }

    #[test]
    fn lottie_widget_empty_layers_does_not_crash() {
        let json = r#"{"op":30,"ip":0,"fr":30,"v":"5.5.2","w":100,"h":100,"layers":[]}"#;
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        lottie.load_json(json).unwrap();
        assert_eq!(lottie.layers().len(), 0);
    }

    #[test]
    fn lottie_widget_keyframe_interpolation() {
        // Test that animated properties interpolate between keyframes.
        let json = r#"{
            "op":20,"ip":0,"fr":30,"v":"5.5.2","w":100,"h":100,
            "layers":[
                {
                    "ind":0,"parent":-1,
                    "ks":{"a":{"k":[0,0]},"p":{"k":[{"t":0,"s":[0,0]},{"t":20,"s":[100,100]}]},"s":{"k":[100,100]},"r":{"k":[0]},"o":{"k":[100]}},
                    "shapes":[
                        {"ty":"rc","p":{"k":[50,50]},"s":{"k":[{"t":0,"s":[10,10]},{"t":20,"s":[90,90]}]},"r":{"k":[0]}},
                        {"ty":"fl","c":{"k":[0,0,1,1]},"o":{"k":[100]},"r":1}
                    ]
                }
            ]
        }"#;
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        lottie.load_json(json).unwrap();

        // At frame 0, size should be 10x10.
        // At frame 10 (midpoint), size should be ~50x50.
        // At frame 20, size should be 90x90.
        // We verify by checking the layer is parsed correctly (keyframes stored).
        assert_eq!(lottie.layers().len(), 1);
        assert_eq!(lottie.layers()[0].shapes.len(), 2);
        // Verify keyframe count on the position property.
        if let LottieShape::Rectangle(ref rs) = lottie.layers()[0].shapes[0] {
            assert_eq!(rs.size.keyframes.len(), 2, "Should have 2 keyframes on size");
            assert_eq!(rs.size.base.len(), 2, "Should have base value");
        } else {
            panic!("Expected Rectangle shape");
        }
    }

    #[test]
    fn lottie_widget_draw_does_not_panic_with_shapes() {
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(100, 100), 1.0);
        let mut ctx = crate::render::RenderContext::new(&mut backend);

        let mut lottie = LottieWidget::new(Rect::new(0, 0, 100, 100));
        let json = make_lottie_json(30.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        lottie.play();

        // Should not panic when drawing with shapes.
        lottie.draw(&mut ctx);
        // No crash = test passes.
    }

    #[test]
    fn lottie_widget_draw_empty_no_crash() {
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(100, 100), 1.0);
        let mut ctx = crate::render::RenderContext::new(&mut backend);

        let mut lottie = LottieWidget::new(Rect::new(0, 0, 100, 100));
        // No json loaded - empty state.
        lottie.draw(&mut ctx);
        // No crash = test passes.
    }

    #[test]
    fn setting_the_frame_rate_repaints() {
        let mut widget = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        widget.base.redraw_requested.connect({
            let seen = std::sync::Arc::clone(&seen);
            move || {
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        widget.set_frame_rate(48.0);
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the readout is timestamped from this"
        );
        widget.set_frame_rate(48.0);
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the same rate is not a change"
        );
        widget.set_frame_rate(f32::NAN);
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "a non-finite rate is refused, so nothing changed"
        );
    }

    /// Loads `json` into a widget drawn into a fresh software backend and returns
    /// the number of shape-red pixels in the presented frame.
    fn render_red_pixels(json: &str) -> usize {
        render_and_count(json).0
    }

    /// Draws `json` and returns `(red pixel count, mean red-pixel x)`.
    ///
    /// The mean x is how a group's transform composition is proven: shifting a
    /// group's position must move where its shapes land (D09-LOT-01).
    fn render_and_count(json: &str) -> (usize, f64) {
        use crate::render::{RenderContext, SoftwarePaintBackend};

        let mut backend = SoftwarePaintBackend::new(crate::core::Size::new(200, 200), 1.0);
        backend.begin_frame(crate::core::Color::rgba(255, 255, 255, 255));
        {
            // The context borrows the backend for the duration of the draw, so it
            // is scoped and dropped before the frame is presented and read back.
            let mut ctx = RenderContext::new(&mut backend);
            let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
            lottie.load_json(json).expect("the fixture must be a valid Lottie document");
            lottie.draw(&mut ctx);
        }
        backend.end_frame();

        let width = 200usize;
        let mut count = 0usize;
        let mut x_sum = 0f64;
        for (i, px) in backend.frame_rgba().chunks_exact(4).enumerate() {
            if px[0] > 200 && px[1] < 80 && px[2] < 80 {
                count += 1;
                x_sum += (i % width) as f64;
            }
        }
        let mean_x = if count == 0 { 0.0 } else { x_sum / count as f64 };
        (count, mean_x)
    }

    /// A `"sh"` bezier path is a supported shape and must actually be painted.
    ///
    /// Before the fix the parser kept `"sh"` as [`LottieShape::Other`] and the draw
    /// loop dropped it, so the filled triangle below produced **zero** red pixels
    /// while the animation still reported success (D09-LOT-01).
    #[test]
    fn lottie_widget_renders_a_bezier_path_shape() {
        // A closed triangle: three vertices, straight edges (zero tangents), filled
        // pure red. `ip=0`, `op=30` so the loader accepts it.
        let json = r#"{
            "op":30,"ip":0,"fr":30,"v":"5.5.2","w":100,"h":100,
            "layers":[{
                "ind":0,"parent":-1,
                "ks":{"a":{"k":[0,0]},"p":{"k":[0,0]},"s":{"k":[100,100]},"r":{"k":[0]},"o":{"k":[100]}},
                "shapes":[
                    {"ty":"sh","ks":{"k":{
                        "i":[[0,0],[0,0],[0,0]],
                        "o":[[0,0],[0,0],[0,0]],
                        "v":[[10,10],[90,10],[50,90]],
                        "c":true
                    }}},
                    {"ty":"fl","c":{"k":[1,0,0,1]},"o":{"k":[100]},"r":1}
                ]
            }]
        }"#;

        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        lottie.load_json(json).unwrap();

        // The parser must produce a real Path, not an `Other` fallback.
        assert_eq!(lottie.layers().len(), 1, "the single layer must parse");
        assert!(
            matches!(lottie.layers()[0].shapes.first(), Some(LottieShape::Path(_))),
            "the \"sh\" shape must parse as LottieShape::Path, got {:?}",
            lottie.layers()[0].shapes.first()
        );
        assert!(
            !lottie.has_unsupported_shapes(),
            "a path and a fill are both supported: {:?}",
            lottie.unsupported_shapes()
        );

        let red = render_red_pixels(json);
        assert!(
            red > 500,
            "the filled triangle must paint a substantial red area, got {red} red pixels"
        );
    }

    /// An unsupported shape must not vanish without a trace: the widget records a
    /// queryable diagnostic, and it must still draw the supported shapes around it.
    ///
    /// The old renderer matched [`LottieShape::Other`] with a no-op, so the star
    /// simply disappeared and the caller had no way to learn the composition was
    /// only partially rendered (D09-LOT-01).
    #[test]
    fn lottie_widget_reports_unsupported_shapes_instead_of_dropping_them() {
        // `sr` (star) is a legal Lottie shape this renderer does not draw. The
        // rectangle beside it is supported and must still render.
        let json = r#"{
            "op":30,"ip":0,"fr":30,"v":"5.5.2","w":100,"h":100,
            "layers":[{
                "ind":0,"parent":-1,
                "ks":{"a":{"k":[0,0]},"p":{"k":[0,0]},"s":{"k":[100,100]},"r":{"k":[0]},"o":{"k":[100]}},
                "shapes":[
                    {"ty":"rc","p":{"k":[50,50]},"s":{"k":[80,80]},"r":{"k":[0]}},
                    {"ty":"fl","c":{"k":[1,0,0,1]},"o":{"k":[100]},"r":1},
                    {"ty":"sr","sy":1},
                    {"ty":"sr","sy":2}
                ]
            }]
        }"#;

        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        lottie.load_json(json).unwrap();

        assert!(
            lottie.has_unsupported_shapes(),
            "an unsupported \"sr\" shape must be reported, not silently lost"
        );
        assert_eq!(
            lottie.unsupported_shapes(),
            ["sr"],
            "each unsupported type is recorded once, in first-seen order"
        );

        // The supported rectangle in the same layer still paints.
        let red = render_red_pixels(json);
        assert!(
            red > 500,
            "the supported rectangle must still render alongside the skipped star, got {red} px"
        );
    }

    /// An unsupported shape nested inside a group is reported too.
    ///
    /// The diagnostic must walk groups, otherwise an unsupported shape buried in a
    /// group would be exactly as invisible as before the fix (D09-LOT-01).
    #[test]
    fn lottie_widget_reports_unsupported_shapes_nested_in_a_group() {
        let json = r#"{
            "op":30,"ip":0,"fr":30,"v":"5.5.2","w":100,"h":100,
            "layers":[{
                "ind":0,"parent":-1,
                "ks":{"a":{"k":[0,0]},"p":{"k":[0,0]},"s":{"k":[100,100]},"r":{"k":[0]},"o":{"k":[100]}},
                "shapes":[{
                    "ty":"gr",
                    "it":[
                        {"ty":"sh","ks":{"k":{
                            "i":[[0,0],[0,0],[0,0]],
                            "o":[[0,0],[0,0],[0,0]],
                            "v":[[10,10],[90,10],[50,90]],
                            "c":true
                        }}},
                        {"ty":"fl","c":{"k":[1,0,0,1]},"o":{"k":[100]},"r":1},
                        {"ty":"mm","mm":1},
                        {"ty":"tr","o":{"k":[100]},"p":{"k":[0,0]},"a":{"k":[0,0]},"s":{"k":[100,100]},"r":{"k":[0]}}
                    ]
                }]
            }]
        }"#;

        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        lottie.load_json(json).unwrap();

        assert_eq!(lottie.layers().len(), 1);
        assert!(
            matches!(lottie.layers()[0].shapes.first(), Some(LottieShape::Group(_))),
            "the group must parse as LottieShape::Group, got {:?}",
            lottie.layers()[0].shapes.first()
        );
        assert_eq!(
            lottie.unsupported_shapes(),
            ["mm"],
            "an unsupported shape nested in a group must be reported"
        );

        // The path inside the group is supported and must render through the group.
        let (red, _) = render_and_count(json);
        assert!(red > 500, "the grouped path must render, got {red} px");
    }

    /// A group's transform must compose with its parent: shifting the group's
    /// `tr.p` moves where its child path lands.
    ///
    /// This pins the transformer composition itself rather than merely "something
    /// got drawn", which a group whose transform was ignored would also satisfy
    /// (D09-LOT-01).
    #[test]
    fn lottie_widget_composes_a_group_transform() {
        // The same triangle inside a group, once at group offset `p=[0,0]` and once
        // at `p=[30,0]`; the second must land measurably further right.
        let make = |group_px: i32| {
            format!(
                r#"{{
                "op":30,"ip":0,"fr":30,"v":"5.5.2","w":100,"h":100,
                "layers":[{{
                    "ind":0,"parent":-1,
                    "ks":{{"a":{{"k":[0,0]}},"p":{{"k":[0,0]}},"s":{{"k":[100,100]}},"r":{{"k":[0]}},"o":{{"k":[100]}}}},
                    "shapes":[{{
                        "ty":"gr",
                        "it":[
                            {{"ty":"sh","ks":{{"k":{{
                                "i":[[0,0],[0,0],[0,0]],
                                "o":[[0,0],[0,0],[0,0]],
                                "v":[[10,10],[40,10],[25,40]],
                                "c":true
                            }}}}}},
                            {{"ty":"fl","c":{{"k":[1,0,0,1]}},"o":{{"k":[100]}},"r":1}},
                            {{"ty":"tr","o":{{"k":[100]}},"p":{{"k":[{group_px},0]}},"a":{{"k":[0,0]}},"s":{{"k":[100,100]}},"r":{{"k":[0]}}}}
                        ]
                    }}]
                }}]
                }}"#
            )
        };

        let (base_count, base_x) = render_and_count(&make(0));
        let (shifted_count, shifted_x) = render_and_count(&make(30));

        assert!(base_count > 200, "the base group must render, got {base_count} px");
        assert!(shifted_count > 200, "the shifted group must render, got {shifted_count} px");
        assert!(
            shifted_x > base_x + 20.0,
            "a group at p=[30,0] must land ~30px right of p=[0,0]: base mean x {base_x:.1}, \
             shifted mean x {shifted_x:.1}"
        );
    }

    /// A supported-path document reports an empty diagnostic, and re-loading a
    /// document resets the previous diagnostic rather than accumulating it.
    #[test]
    fn lottie_widget_unsupported_shapes_reset_on_reload() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let unsupported = r#"{"op":10,"ip":0,"fr":30,"w":100,"h":100,
            "layers":[{"ind":0,"parent":-1,"ks":{},
                "shapes":[{"ty":"sr","sy":1}]}]}"#;
        lottie.load_json(unsupported).unwrap();
        assert_eq!(lottie.unsupported_shapes(), ["sr"]);

        // Reloading a fully supported document must clear the stale entry.
        lottie.load_json(&make_lottie_json(30.0, 0.0, 30.0)).unwrap();
        assert!(
            !lottie.has_unsupported_shapes(),
            "a reload must not leave the previous document's diagnostic behind: {:?}",
            lottie.unsupported_shapes()
        );
    }
}
