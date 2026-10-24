// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! 2D affine and homogeneous matrix maths for the feature-gated transform path.
//!
//! The module is compiled only with the `3d` feature. It owns the small amount of linear
//! algebra the batch replay stack and the matrix vertex path share, so neither re-derives
//! the composition order. Everything here is pure: no allocation, no device access, and no
//! dependency on a render backend.
//!
//! # Storage convention
//!
//! [`Mat3`] stores a 2D affine transform as six numbers in the order used by the common
//! `matrix(a, b, c, d, e, f)` spelling:
//!
//! ```text
//! | a c e |
//! | b d f |
//! | 0 0 1 |
//! ```
//!
//! so `x' = a*x + c*y + e` and `y' = b*x + d*y + f`. [`Mat4`] is the same transform embedded
//! in a homogeneous 4x4, stored row-major in sixteen numbers.
//!
//! Rotation is the mathematical convention (positive radians turn the positive x axis toward
//! the positive y axis). Because the render coordinate system has y growing downward, a
//! positive angle reads as a clockwise turn on screen.

#![cfg(feature = "3d")]

use crate::core::Point;

/// The largest number of saved matrices a [`TransformStack`] can hold.
pub const TRANSFORM_STACK_MAX: usize = 16;

/// A 2D affine transform stored as `[a, b, c, d, e, f]`.
///
/// The implied third row is always `[0, 0, 1]`, so this is an affine map and not a general
/// projective one. Use [`Mat3::then`] to compose transforms; the argument is applied second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat3 {
    /// The six affine coefficients, in `[a, b, c, d, e, f]` order.
    pub m: [f32; 6],
}

impl Mat3 {
    /// The identity transform.
    pub const IDENTITY: Self = Self { m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] };

    /// Translation by `(dx, dy)`.
    pub fn translation(dx: f32, dy: f32) -> Self {
        Self { m: [1.0, 0.0, 0.0, 1.0, dx, dy] }
    }

    /// Axis-aligned scaling by `(sx, sy)`.
    pub fn scaling(sx: f32, sy: f32) -> Self {
        Self { m: [sx, 0.0, 0.0, sy, 0.0, 0.0] }
    }

    /// Rotation by `radians` about the origin.
    pub fn rotation(radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self { m: [cos, sin, -sin, cos, 0.0, 0.0] }
    }

    /// Composition: `self` first, then `next`.
    ///
    /// In the column-vector convention this is the matrix product `next * self`, so
    /// `Mat3::translation(dx, dy).then(&Mat3::rotation(a))` rotates the translated point.
    pub fn then(&self, next: &Self) -> Self {
        let [a1, b1, c1, d1, e1, f1] = self.m;
        let [a2, b2, c2, d2, e2, f2] = next.m;
        Self {
            m: [
                a2 * a1 + c2 * b1,
                b2 * a1 + d2 * b1,
                a2 * c1 + c2 * d1,
                b2 * c1 + d2 * d1,
                a2 * e1 + c2 * f1 + e2,
                b2 * e1 + d2 * f1 + f2,
            ],
        }
    }

    /// Maps an integer point through this transform, rounding each coordinate to the nearest
    /// logical pixel.
    pub fn map_point(&self, p: Point) -> Point {
        let (x, y) = self.map_point_f32(p.x as f32, p.y as f32);
        Point::new(x.round() as i32, y.round() as i32)
    }

    /// Maps a floating-point point through this transform without rounding.
    fn map_point_f32(&self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, c, d, e, f] = self.m;
        (a * x + c * y + e, b * x + d * y + f)
    }

    /// The uniform scale this transform applies to lengths, as the square root of the area
    /// scale.
    ///
    /// A pure rotation answers `1`, an axis-aligned scaling by `(s, s)` answers `s`, and a
    /// rotation composed with a scaling answers that scaling — which is the factor a primitive
    /// without corners (a glyph's point size, a stroke's width) has to follow when the four
    /// corners of its box are not available to transform. A degenerate matrix (zero area)
    /// answers `0`.
    pub fn length_scale(&self) -> f32 {
        let [a, b, c, d, _, _] = self.m;
        (a * d - b * c).abs().sqrt()
    }
}

/// An axis of rotation for [`Mat4::rotation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Rotation about the x axis.
    X,
    /// Rotation about the y axis.
    Y,
    /// Rotation about the z axis.
    Z,
}

/// A homogeneous 4x4 matrix stored row-major.
///
/// The first two rows of a matrix produced by [`Mat4::from_affine`] carry the 2D affine
/// coefficients, the third keeps depth unchanged and the fourth is `[0, 0, 0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    /// The sixteen coefficients, row-major.
    pub m: [f32; 16],
}

impl Mat4 {
    /// The identity transform.
    pub const IDENTITY: Self = Self {
        m: [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ],
    };

    /// Embeds a 2D affine transform in the `z = 0` plane.
    pub fn from_affine(m: Mat3) -> Self {
        let [a, b, c, d, e, f] = m.m;
        Self {
            m: [
                a, c, 0.0, e, //
                b, d, 0.0, f, //
                0.0, 0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Rotation by `radians` about `axis`.
    pub fn rotation(axis: Axis, radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        let mut m = Self::IDENTITY.m;
        match axis {
            Axis::X => {
                m[5] = cos;
                m[6] = -sin;
                m[9] = sin;
                m[10] = cos;
            }
            Axis::Y => {
                m[0] = cos;
                m[2] = sin;
                m[8] = -sin;
                m[10] = cos;
            }
            Axis::Z => {
                m[0] = cos;
                m[1] = -sin;
                m[4] = sin;
                m[5] = cos;
            }
        }
        Self { m }
    }

    /// A standard right-handed perspective projection.
    ///
    /// `fov_y` is the vertical field of view in radians, `aspect` is width divided by height,
    /// and `z_near`/`z_far` are positive distances from the eye. The caller must pass
    /// `0 < z_near < z_far`; a zero depth is not a projection. The matrix leaves depth in
    /// clip space and writes `w = -z`; points at or behind the eye therefore have `w <= 0`
    /// and must be clipped before the divide.
    pub fn perspective(fov_y: f32, aspect: f32, z_near: f32, z_far: f32) -> Self {
        let (sin, cos) = (fov_y * 0.5).sin_cos();
        let f = if sin.abs() > f32::EPSILON { cos / sin } else { 0.0 };
        let depth = z_near - z_far;
        let mut m = [0.0f32; 16];
        m[0] = f / aspect;
        m[5] = f;
        m[10] = (z_far + z_near) / depth;
        m[11] = (2.0 * z_far * z_near) / depth;
        m[14] = -1.0;
        Self { m }
    }

    /// Composition: `self` first, then `next` (the product `next * self`).
    // The two matrices are indexed with different strides, so the usual iterator rewrite would
    // obscure which row is being multiplied by which column.
    #[allow(clippy::needless_range_loop)]
    pub fn then(&self, next: &Self) -> Self {
        let mut out = [0.0f32; 16];
        for row in 0..4 {
            for col in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += next.m[row * 4 + k] * self.m[k * 4 + col];
                }
                out[row * 4 + col] = sum;
            }
        }
        Self { m: out }
    }

    /// Projects the point `(x, y, 1, 1)` and returns `(x', y', w)`.
    ///
    /// The point is taken at unit depth so a perspective matrix yields a non-zero `w`. A
    /// caller performing a perspective divide must first discard `w <= 0`: those points lie
    /// on or behind the view plane and need clipping, not division.
    pub fn project_row(&self, x: f32, y: f32) -> (f32, f32, f32) {
        let m = &self.m;
        let px = m[0] * x + m[1] * y + m[2] + m[3];
        let py = m[4] * x + m[5] * y + m[6] + m[7];
        let w = m[12] * x + m[13] * y + m[14] + m[15];
        (px, py, w)
    }
}

/// A saved transform for [`TransformStack`].
///
/// The 2D affine is carried in a homogeneous [`Mat4`] so the stack has one representation
/// regardless of how a caller built the matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    matrix: Mat4,
}

impl Transform {
    /// The identity transform.
    pub const IDENTITY: Self = Self { matrix: Mat4::IDENTITY };

    /// Builds a transform from a 2D affine matrix.
    pub fn from_affine(m: Mat3) -> Self {
        Self { matrix: Mat4::from_affine(m) }
    }

    /// Returns the transform's 2D affine part.
    ///
    /// A transform created by [`Transform::from_affine`] round-trips exactly. For a matrix
    /// with genuine depth terms this keeps only the `x`/`y` affine rows.
    pub fn affine(&self) -> Mat3 {
        Mat3 {
            m: [
                self.matrix.m[0],
                self.matrix.m[4],
                self.matrix.m[1],
                self.matrix.m[5],
                self.matrix.m[3],
                self.matrix.m[7],
            ],
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// A fixed-capacity save/restore stack for batch replay.
///
/// [`TransformStack::push`] reports failure instead of panicking when the stack is full, so a
/// malformed batch cannot abort a replay. `current` answers with the identity when the stack
/// is empty.
#[derive(Debug, Clone, Copy)]
pub struct TransformStack {
    /// Saved transforms, oldest first.
    stack: [Transform; TRANSFORM_STACK_MAX],
    /// Number of occupied slots.
    len: usize,
}

impl TransformStack {
    /// An empty stack.
    pub const fn new() -> Self {
        Self { stack: [Transform::IDENTITY; TRANSFORM_STACK_MAX], len: 0 }
    }

    /// Saves `t`, returning `false` when the stack is already at capacity.
    pub fn push(&mut self, t: Transform) -> bool {
        if self.len >= TRANSFORM_STACK_MAX {
            return false;
        }
        self.stack[self.len] = t;
        self.len += 1;
        true
    }

    /// Removes and returns the most recently saved transform, or `None` when empty.
    pub fn pop(&mut self) -> Option<Transform> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.stack[self.len])
    }

    /// The most recently saved transform, or the identity when empty.
    pub fn current(&self) -> Transform {
        if self.len == 0 {
            Transform::IDENTITY
        } else {
            self.stack[self.len - 1]
        }
    }
}

impl Default for TransformStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f32::consts::{FRAC_PI_2, PI};

    fn assert_close(actual: f32, expected: f32, what: &str) {
        assert!((actual - expected).abs() < 1.0e-4, "{what}: expected {expected}, got {actual}");
    }

    #[test]
    fn identity_leaves_a_point_unchanged() {
        assert_eq!(Mat3::IDENTITY.map_point(Point::new(13, -7)), Point::new(13, -7));
    }

    #[test]
    fn translation_offsets_a_point() {
        let m = Mat3::translation(10.0, -4.0);
        assert_eq!(m.map_point(Point::new(3, 7)), Point::new(13, 3));
    }

    #[test]
    fn scaling_multiplies_each_axis() {
        let m = Mat3::scaling(2.0, 3.0);
        assert_eq!(m.map_point(Point::new(4, 5)), Point::new(8, 15));
    }

    #[test]
    fn a_quarter_turn_rotates_the_basis_vectors() {
        let m = Mat3::rotation(FRAC_PI_2);
        assert_eq!(m.map_point(Point::new(10, 0)), Point::new(0, 10));
        assert_eq!(m.map_point(Point::new(0, 10)), Point::new(-10, 0));
    }

    #[test]
    fn then_applies_the_argument_second() {
        let translate_then_rotate = Mat3::translation(10.0, 0.0).then(&Mat3::rotation(FRAC_PI_2));
        assert_eq!(translate_then_rotate.map_point(Point::new(10, 0)), Point::new(0, 20));

        let rotate_then_translate = Mat3::rotation(FRAC_PI_2).then(&Mat3::translation(10.0, 0.0));
        assert_eq!(rotate_then_translate.map_point(Point::new(10, 0)), Point::new(10, 10));
    }

    #[test]
    fn mat4_from_affine_matches_mat3_for_a_projected_point() {
        let affine = Mat3::translation(4.0, -2.0).then(&Mat3::rotation(0.37));
        let m4 = Mat4::from_affine(affine);
        let expected = affine.map_point(Point::new(11, 5));
        let (x, y, w) = m4.project_row(11.0, 5.0);
        assert_close(w, 1.0, "an affine point has w = 1");
        assert_eq!(Point::new(x.round() as i32, y.round() as i32), expected);
    }

    #[test]
    fn mat3_and_mat4_composition_agree() {
        let first = Mat3::translation(7.0, -3.0).then(&Mat3::rotation(0.41));
        let second = Mat3::scaling(1.5, 0.75).then(&Mat3::translation(2.0, 9.0));
        let through_mat3 = Mat4::from_affine(first.then(&second));
        let through_mat4 = Mat4::from_affine(first).then(&Mat4::from_affine(second));
        assert_eq!(through_mat3, through_mat4, "the two composition paths must agree");
    }

    #[test]
    fn mat4_rotation_uses_the_requested_axis() {
        let x = Mat4::rotation(Axis::X, FRAC_PI_2);
        assert_close(x.m[5], 0.0, "x axis: row 1 column 1");
        assert_close(x.m[6], -1.0, "x axis: row 1 column 2");
        assert_close(x.m[9], 1.0, "x axis: row 2 column 1");

        let y = Mat4::rotation(Axis::Y, FRAC_PI_2);
        assert_close(y.m[0], 0.0, "y axis: row 0 column 0");
        assert_close(y.m[2], 1.0, "y axis: row 0 column 2");
        assert_close(y.m[8], -1.0, "y axis: row 2 column 0");

        let z = Mat4::rotation(Axis::Z, FRAC_PI_2);
        let (x, y, w) = z.project_row(1.0, 0.0);
        assert_close(w, 1.0, "z rotation keeps w = 1");
        assert_close(x, 0.0, "z axis: x");
        assert_close(y, 1.0, "z axis: y");
    }

    #[test]
    fn project_row_exposes_the_perspective_divisor() {
        // A projective 2D matrix: w grows with x, so the divide is observable even though the
        // input is a plane point. This is the same arithmetic a 3D projection performs after
        // the vertex stage.
        let mut m = Mat4::from_affine(Mat3::IDENTITY);
        m.m[12] = 0.5;
        let (x, y, w) = m.project_row(2.0, 0.0);
        assert_close(x, 2.0, "x before the divide");
        assert_close(y, 0.0, "y before the divide");
        assert_close(w, 2.0, "w includes the x coefficient");
        assert_close(x / w, 1.0, "x after the perspective divide");
    }

    #[test]
    fn perspective_writes_the_w_row() {
        let m = Mat4::perspective(PI / 2.0, 1.0, 0.1, 100.0);
        assert_close(m.m[0], 1.0, "f / aspect with a 90 degree fov and aspect 1");
        assert_close(m.m[5], 1.0, "f with a 90 degree fov");
        assert_close(m.m[14], -1.0, "w = -z");
        let (_, _, w) = m.project_row(0.0, 0.0);
        assert_close(w, -1.0, "a point at unit depth lies at w = -1 and needs clipping");
    }

    #[test]
    fn transform_stack_saves_restores_and_refuses_overflow() {
        let mut stack = TransformStack::new();
        assert_eq!(stack.current(), Transform::IDENTITY);
        assert_eq!(stack.pop(), None);

        let first = Transform::from_affine(Mat3::translation(1.0, 2.0));
        assert!(stack.push(first));
        assert_eq!(stack.current(), first);

        let second = Transform::from_affine(Mat3::rotation(0.5));
        assert!(stack.push(second));
        assert_eq!(stack.current(), second);
        assert_eq!(stack.pop(), Some(second));
        assert_eq!(stack.current(), first);
        assert_eq!(stack.pop(), Some(first));
        assert_eq!(stack.current(), Transform::IDENTITY, "the stack is empty again");

        for _ in 0..TRANSFORM_STACK_MAX {
            assert!(stack.push(first), "the stack accepts exactly TRANSFORM_STACK_MAX entries");
        }
        assert!(!stack.push(first), "a full stack reports the refusal instead of panicking");
    }

    #[test]
    fn transform_affine_round_trips() {
        let affine = Mat3::translation(3.0, 4.0).then(&Mat3::scaling(2.0, 0.5));
        let transform = Transform::from_affine(affine);
        assert_eq!(transform.affine(), affine);
    }
}
