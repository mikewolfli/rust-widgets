//! Freeform shape widget — a path-based non-rectangular clickable shape.
use crate::core::{Color, Point, Rect};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BubbleTailDirection {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSegment {
    MoveTo(Point),
    LineTo(Point),
    CurveTo(Point, Point, Point),
    QuadTo(Point, Point),
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShapePath {
    Heart,
    Star { points: u8, inner_radius: f32 },
    Polygon(Vec<Point>),
    RoundedRect { radius: u32 },
    Bubble { tail_direction: BubbleTailDirection },
    Custom(Vec<PathSegment>),
}

pub struct FreeformShapeWidget {
    base: BaseWidget,
    path: ShapePath,
    fill_color: Color,
    stroke_color: Option<Color>,
    stroke_width: u32,
    hovered: bool,
    pressed: bool,
    pub clicked: GenericSignal,
    pub hovered_changed: Signal1<bool>,
    pub pressed_changed: Signal1<bool>,
}

impl FreeformShapeWidget {
    pub fn new(geometry: Rect, path: ShapePath) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FreeformShape, geometry, "FreeformShapeWidget"),
            path,
            fill_color: Color::from_rgb(200, 220, 255),
            stroke_color: Some(Color::from_rgb(80, 120, 200)),
            stroke_width: 2,
            hovered: false,
            pressed: false,
            clicked: GenericSignal::new(),
            hovered_changed: Signal1::new(),
            pressed_changed: Signal1::new(),
        }
    }

    pub fn path(&self) -> &ShapePath {
        &self.path
    }
    pub fn set_path(&mut self, path: ShapePath) {
        self.path = path;
        self.base.request_redraw();
    }
    pub fn fill_color(&self) -> Color {
        self.fill_color
    }
    pub fn set_fill_color(&mut self, color: Color) {
        self.fill_color = color;
        self.base.request_redraw();
    }
    pub fn stroke_color(&self) -> Option<Color> {
        self.stroke_color
    }
    pub fn set_stroke_color(&mut self, color: Option<Color>) {
        self.stroke_color = color;
        self.base.request_redraw();
    }
    pub fn stroke_width(&self) -> u32 {
        self.stroke_width
    }
    pub fn set_stroke_width(&mut self, width: u32) {
        self.stroke_width = width;
        self.base.request_redraw();
    }

    pub fn contains(&self, point: Point) -> bool {
        let rect = self.base.geometry();
        if !rect.contains_point(point) {
            return false;
        }
        let local = Point::new(point.x - rect.x, point.y - rect.y);
        let w = rect.width as i32;
        let h = rect.height as i32;
        if w <= 0 || h <= 0 {
            return false;
        }
        match &self.path {
            ShapePath::Heart => self.contains_heart(local, w, h),
            ShapePath::Star { points, inner_radius } => {
                self.contains_star(local, w, h, *points, *inner_radius)
            }
            ShapePath::Polygon(v) => self.contains_polygon(local, v),
            ShapePath::RoundedRect { radius } => self.contains_rounded_rect(local, w, h, *radius),
            ShapePath::Bubble { tail_direction } => {
                self.contains_bubble(local, w, h, *tail_direction)
            }
            ShapePath::Custom(s) => self.contains_custom(local, s),
        }
    }

    fn contains_heart(&self, local: Point, w: i32, h: i32) -> bool {
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let scale = (h as f32).max(1.0) / 2.0;
        let x = (local.x as f32 - cx) / scale;
        let y = (local.y as f32 - cy) / scale;
        (x * x + y * y - 1.0).powi(3) - x * x * (y * y * y) <= 0.0
    }

    fn contains_star(
        &self,
        local: Point,
        w: i32,
        h: i32,
        num_points: u8,
        inner_ratio: f32,
    ) -> bool {
        let n = num_points.max(3) as usize;
        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let outer_r = (cx.min(cy)).max(1.0);
        let inner_r = outer_r * inner_ratio.clamp(0.05, 0.95);
        let mut vertices = Vec::with_capacity(n * 2);
        let step = std::f32::consts::PI / n as f32;
        for i in 0..n {
            let angle =
                i as f32 * 2.0 * std::f32::consts::PI / n as f32 - std::f32::consts::FRAC_PI_2;
            vertices.push(Point::new(
                (cx + outer_r * angle.cos()).round() as i32,
                (cy + outer_r * angle.sin()).round() as i32,
            ));
            let ia = angle + step;
            vertices.push(Point::new(
                (cx + inner_r * ia.cos()).round() as i32,
                (cy + inner_r * ia.sin()).round() as i32,
            ));
        }
        self.ray_cast(local, &vertices)
    }

    fn contains_polygon(&self, local: Point, vertices: &[Point]) -> bool {
        self.ray_cast(local, vertices)
    }

    fn contains_rounded_rect(&self, local: Point, w: i32, h: i32, radius: u32) -> bool {
        let r = (radius as i32).min(w / 2).min(h / 2);
        if r <= 0 {
            return local.x >= 0 && local.y >= 0 && local.x < w && local.y < h;
        }
        let (x, y) = (local.x, local.y);
        if x < 0 || y < 0 || x >= w || y >= h {
            return false;
        }
        if x < r && y < r {
            return (x - r).pow(2) + (y - r).pow(2) <= r * r;
        }
        if x >= w - r && y < r {
            return (x - (w - r)).pow(2) + (y - r).pow(2) <= r * r;
        }
        if x < r && y >= h - r {
            return (x - r).pow(2) + (y - (h - r)).pow(2) <= r * r;
        }
        if x >= w - r && y >= h - r {
            return (x - (w - r)).pow(2) + (y - (h - r)).pow(2) <= r * r;
        }
        true
    }

    fn contains_bubble(&self, local: Point, w: i32, h: i32, td: BubbleTailDirection) -> bool {
        let ts = 12i32;
        let (x, y) = (local.x, local.y);
        let (x_min, y_min, x_max, y_max) = match td {
            BubbleTailDirection::Left => (-ts, 0, w, h),
            BubbleTailDirection::Right => (0, 0, w + ts, h),
            BubbleTailDirection::Top => (0, -ts, w, h),
            BubbleTailDirection::Bottom => (0, 0, w, h + ts),
            BubbleTailDirection::TopLeft => (-ts, -ts, w, h),
            BubbleTailDirection::TopRight => (0, -ts, w + ts, h),
            BubbleTailDirection::BottomLeft => (-ts, 0, w, h + ts),
            BubbleTailDirection::BottomRight => (0, 0, w + ts, h + ts),
        };
        if x < x_min || y < y_min || x >= x_max || y >= y_max {
            return false;
        }
        let r = 8u32.min(w as u32 / 2).min(h as u32 / 2).max(1);
        if x >= 0 && x < w && y >= 0 && y < h {
            return self.contains_rounded_rect(local, w, h, r);
        }
        let (tcx, tcy, tdx, tdy) = match td {
            BubbleTailDirection::Left => (0, h / 2, -1, 0),
            BubbleTailDirection::Right => (w - 1, h / 2, 1, 0),
            BubbleTailDirection::Top => (w / 2, 0, 0, -1),
            BubbleTailDirection::Bottom => (w / 2, h - 1, 0, 1),
            BubbleTailDirection::TopLeft => (0, 0, -1, -1),
            BubbleTailDirection::TopRight => (w - 1, 0, 1, -1),
            BubbleTailDirection::BottomLeft => (0, h - 1, -1, 1),
            BubbleTailDirection::BottomRight => (w - 1, h - 1, 1, 1),
        };
        let (bdx, bdy) = match td {
            BubbleTailDirection::Left | BubbleTailDirection::Right => (0, ts / 2),
            BubbleTailDirection::Top | BubbleTailDirection::Bottom => (ts / 2, 0),
            BubbleTailDirection::TopLeft | BubbleTailDirection::BottomRight => (ts / 2, -ts / 2),
            BubbleTailDirection::TopRight | BubbleTailDirection::BottomLeft => (ts / 2, ts / 2),
        };
        let tip = Point::new(tcx + tdx * ts, tcy + tdy * ts);
        let b1 = Point::new(tcx + bdx, tcy + bdy);
        let b2 = Point::new(tcx - bdx, tcy - bdy);
        self.point_in_triangle(local, tip, b1, b2)
    }

    fn contains_custom(&self, local: Point, segments: &[PathSegment]) -> bool {
        if segments.is_empty() {
            return false;
        }
        let mut pv = Vec::new();
        let mut cur = Point::new(0, 0);
        let mut start = Point::new(0, 0);
        for seg in segments {
            match seg {
                PathSegment::MoveTo(p) => {
                    if pv.len() > 1 {
                        pv.push(start);
                    }
                    start = *p;
                    cur = *p;
                    pv.push(*p);
                }
                PathSegment::LineTo(p) => {
                    cur = *p;
                    pv.push(*p);
                }
                PathSegment::CurveTo(c1, c2, p) => {
                    self.flatten_cubic_to(&mut pv, cur, *c1, *c2, *p);
                    cur = *p;
                }
                PathSegment::QuadTo(c, p) => {
                    self.flatten_quad_to(&mut pv, cur, *c, *p);
                    cur = *p;
                }
                PathSegment::Close => {
                    if pv.last() != Some(&start) {
                        pv.push(start);
                    }
                    cur = start;
                }
            }
        }
        if pv.len() > 1 && pv.last() != Some(&start) {
            pv.push(start);
        }
        if pv.len() < 3 {
            return false;
        }
        self.ray_cast(local, &pv)
    }

    fn point_in_triangle(&self, pt: Point, a: Point, b: Point, c: Point) -> bool {
        let s = |px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32| -> f32 {
            (px - x2) * (y1 - y2) - (x1 - x2) * (py - y2)
        };
        let (ax, ay, bx, by, cx, cy, px, py) = (
            a.x as f32,
            a.y as f32,
            b.x as f32,
            b.y as f32,
            c.x as f32,
            c.y as f32,
            pt.x as f32,
            pt.y as f32,
        );
        let d1 = s(px, py, ax, ay, bx, by);
        let d2 = s(px, py, bx, by, cx, cy);
        let d3 = s(px, py, cx, cy, ax, ay);
        let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(has_neg && has_pos)
    }

    fn flatten_cubic_to(
        &self,
        vertices: &mut Vec<Point>,
        p0: Point,
        p1: Point,
        p2: Point,
        p3: Point,
    ) {
        let flatness_sq = 4.0;
        let dx = p3.x - p0.x;
        let dy = p3.y - p0.y;
        let d2 = ((p1.x - p3.x) * dy - (p1.y - p3.y) * dx).abs() as f32;
        let d3 = ((p2.x - p3.x) * dy - (p2.y - p3.y) * dx).abs() as f32;
        if (d2 + d3).powi(2) <= flatness_sq * (dx * dx + dy * dy) as f32 + 0.0001 {
            vertices.push(p3);
            return;
        }
        let lerp = |a: i32, b: i32| -> i32 { ((a as f32 + b as f32) / 2.0).round() as i32 };
        let mid1 = Point::new(lerp(p0.x, p1.x), lerp(p0.y, p1.y));
        let mid2 = Point::new(lerp(p1.x, p2.x), lerp(p1.y, p2.y));
        let mid3 = Point::new(lerp(p2.x, p3.x), lerp(p2.y, p3.y));
        let mid12 = Point::new(lerp(mid1.x, mid2.x), lerp(mid1.y, mid2.y));
        let mid23 = Point::new(lerp(mid2.x, mid3.x), lerp(mid2.y, mid3.y));
        let mid = Point::new(lerp(mid12.x, mid23.x), lerp(mid12.y, mid23.y));
        self.flatten_cubic_to(vertices, p0, mid1, mid12, mid);
        self.flatten_cubic_to(vertices, mid, mid23, mid3, p3);
    }

    fn flatten_quad_to(&self, vertices: &mut Vec<Point>, p0: Point, p1: Point, p2: Point) {
        let flatness_sq = 4.0;
        let dx = p2.x - p0.x;
        let dy = p2.y - p0.y;
        let d = ((p1.x - p2.x) * dy - (p1.y - p2.y) * dx).abs() as f32;
        if d * d <= flatness_sq * (dx * dx + dy * dy) as f32 + 0.0001 {
            vertices.push(p2);
            return;
        }
        let lerp = |a: i32, b: i32| -> i32 { ((a as f32 + b as f32) / 2.0).round() as i32 };
        let mid1 = Point::new(lerp(p0.x, p1.x), lerp(p0.y, p1.y));
        let mid2 = Point::new(lerp(p1.x, p2.x), lerp(p1.y, p2.y));
        let mid = Point::new(lerp(mid1.x, mid2.x), lerp(mid1.y, mid2.y));
        self.flatten_quad_to(vertices, p0, mid1, mid);
        self.flatten_quad_to(vertices, mid, mid2, p2);
    }

    fn draw_shape(&self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        match &self.path {
            ShapePath::Heart => self.draw_heart(context, rect),
            ShapePath::Star { points, inner_radius } => {
                self.draw_star(context, rect, *points, *inner_radius)
            }
            ShapePath::Polygon(v) => self.draw_polygon(context, rect, v),
            ShapePath::RoundedRect { radius } => self.draw_rounded_rect(context, rect, *radius),
            ShapePath::Bubble { tail_direction } => {
                self.draw_bubble(context, rect, *tail_direction)
            }
            ShapePath::Custom(s) => self.draw_custom(context, rect, s),
        }
    }

    fn draw_heart(&self, context: &mut RenderContext, rect: Rect) {
        let cx = rect.x + rect.width as i32 / 2;
        let cy = rect.y + rect.height as i32 / 2;
        let r = (rect.width.min(rect.height) as i32 / 10).max(3);
        let (lcx, lcy) = (cx - r * 2, cy - r);
        let (rcx, rcy) = (cx + r * 2, cy - r);
        context.fill_circle(Point::new(lcx, lcy), r as u32, self.fill_color);
        context.fill_circle(Point::new(rcx, rcy), r as u32, self.fill_color);
        let tip_y = cy + 2 * r;
        let (blx, brx) = (cx - 3 * r, cx + 3 * r);
        for x in blx..=brx {
            let t = if brx > blx { (x - blx) as f32 / (brx - blx) as f32 } else { 0.5 };
            let y0 = (lcy as f32 + (tip_y - lcy) as f32 * (1.0 - (t - 0.5).abs() * 2.0)) as i32;
            if y0 <= tip_y {
                context.fill_rect(Rect::new(x, y0, 1, (tip_y - y0 + 1) as u32), self.fill_color);
            }
        }
        if let Some(sc) = self.stroke_color {
            let sw = self.stroke_width;
            context.draw_circle_stroke(Point::new(lcx, lcy), r as u32, sc, sw);
            context.draw_circle_stroke(Point::new(rcx, rcy), r as u32, sc, sw);
            context.draw_line_stroke(Point::new(blx, lcy), Point::new(cx, tip_y), sc, sw);
            context.draw_line_stroke(Point::new(brx, lcy), Point::new(cx, tip_y), sc, sw);
        }
    }

    fn draw_star(&self, context: &mut RenderContext, rect: Rect, num_points: u8, inner_ratio: f32) {
        let cx = rect.x + rect.width as i32 / 2;
        let cy = rect.y + rect.height as i32 / 2;
        let outer_r = (rect.width.min(rect.height) as i32 / 2).max(1) as f32;
        let inner_r = outer_r * inner_ratio.clamp(0.05, 0.95);
        let n = num_points.max(3) as usize;
        let mut vertices = Vec::with_capacity(n * 2);
        let step = std::f32::consts::PI / n as f32;
        for i in 0..n {
            let angle =
                i as f32 * 2.0 * std::f32::consts::PI / n as f32 - std::f32::consts::FRAC_PI_2;
            vertices.push(Point::new(
                (cx as f32 + outer_r * angle.cos()).round() as i32,
                (cy as f32 + outer_r * angle.sin()).round() as i32,
            ));
            let ia = angle + step;
            vertices.push(Point::new(
                (cx as f32 + inner_r * ia.cos()).round() as i32,
                (cy as f32 + inner_r * ia.sin()).round() as i32,
            ));
        }
        for i in 0..vertices.len() {
            let j = (i + 1) % vertices.len();
            self.fill_triangle_approx(context, Point::new(cx, cy), vertices[i], vertices[j]);
        }
        if let Some(sc) = self.stroke_color {
            for i in 0..vertices.len() {
                let j = (i + 1) % vertices.len();
                context.draw_line_stroke(vertices[i], vertices[j], sc, self.stroke_width);
            }
        }
    }

    fn draw_polygon(&self, context: &mut RenderContext, rect: Rect, vertices: &[Point]) {
        if vertices.len() < 3 {
            return;
        }
        let centroid = Point::new(
            vertices.iter().map(|v| v.x).sum::<i32>() / vertices.len() as i32,
            vertices.iter().map(|v| v.y).sum::<i32>() / vertices.len() as i32,
        );
        let vs: Vec<Point> =
            vertices.iter().map(|v| Point::new(rect.x + v.x, rect.y + v.y)).collect();
        for i in 0..vs.len() {
            let j = (i + 1) % vs.len();
            self.fill_triangle_approx(context, centroid, vs[i], vs[j]);
        }
        if let Some(sc) = self.stroke_color {
            for i in 0..vs.len() {
                let j = (i + 1) % vs.len();
                context.draw_line_stroke(vs[i], vs[j], sc, self.stroke_width);
            }
        }
    }

    fn draw_rounded_rect(&self, context: &mut RenderContext, rect: Rect, radius: u32) {
        let r = radius.min(rect.width / 2).min(rect.height / 2);
        if r > 0 {
            context.fill_rounded_rect(rect, r, self.fill_color);
        } else {
            context.fill_rect(rect, self.fill_color);
        }
        if let Some(sc) = self.stroke_color {
            if r > 0 {
                context.draw_rounded_rect_stroke(rect, r, sc, self.stroke_width);
            } else {
                context.draw_rect_stroke(rect, sc, self.stroke_width);
            }
        }
    }

    fn draw_bubble(&self, context: &mut RenderContext, rect: Rect, td: BubbleTailDirection) {
        let r = 8u32.min(rect.width / 2).min(rect.height / 2).max(1);
        let ts = 12i32;
        context.fill_rounded_rect(rect, r, self.fill_color);
        let (tcx, tcy, tdx, tdy) = match td {
            BubbleTailDirection::Left => (rect.x, rect.y + rect.height as i32 / 2, -1, 0),
            BubbleTailDirection::Right => {
                (rect.x + rect.width as i32 - 1, rect.y + rect.height as i32 / 2, 1, 0)
            }
            BubbleTailDirection::Top => (rect.x + rect.width as i32 / 2, rect.y, 0, -1),
            BubbleTailDirection::Bottom => {
                (rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 - 1, 0, 1)
            }
            BubbleTailDirection::TopLeft => (rect.x, rect.y, -1, -1),
            BubbleTailDirection::TopRight => (rect.x + rect.width as i32 - 1, rect.y, 1, -1),
            BubbleTailDirection::BottomLeft => (rect.x, rect.y + rect.height as i32 - 1, -1, 1),
            BubbleTailDirection::BottomRight => {
                (rect.x + rect.width as i32 - 1, rect.y + rect.height as i32 - 1, 1, 1)
            }
        };
        let tip = Point::new(tcx + tdx * ts, tcy + tdy * ts);
        let (bdx, bdy) = match td {
            BubbleTailDirection::Left | BubbleTailDirection::Right => (0, ts / 2),
            BubbleTailDirection::Top | BubbleTailDirection::Bottom => (ts / 2, 0),
            BubbleTailDirection::TopLeft | BubbleTailDirection::BottomRight => (ts / 2, -ts / 2),
            BubbleTailDirection::TopRight | BubbleTailDirection::BottomLeft => (ts / 2, ts / 2),
        };
        let b1 = Point::new(tcx + bdx, tcy + bdy);
        let b2 = Point::new(tcx - bdx, tcy - bdy);
        self.fill_triangle_approx(context, tip, b1, b2);
        if let Some(sc) = self.stroke_color {
            context.draw_rounded_rect_stroke(rect, r, sc, self.stroke_width);
            context.draw_line_stroke(b1, tip, sc, self.stroke_width);
            context.draw_line_stroke(b2, tip, sc, self.stroke_width);
        }
    }

    fn draw_custom(&self, context: &mut RenderContext, rect: Rect, segments: &[PathSegment]) {
        let mut pv = Vec::new();
        let mut cur = Point::new(0, 0);
        let mut start = Point::new(0, 0);
        for seg in segments {
            match seg {
                PathSegment::MoveTo(p) => {
                    if pv.len() > 1 {
                        pv.push(start);
                    }
                    start = *p;
                    cur = *p;
                    pv.push(*p);
                }
                PathSegment::LineTo(p) => {
                    cur = *p;
                    pv.push(*p);
                }
                PathSegment::CurveTo(c1, c2, p) => {
                    self.flatten_cubic_to(&mut pv, cur, *c1, *c2, *p);
                    cur = *p;
                }
                PathSegment::QuadTo(c, p) => {
                    self.flatten_quad_to(&mut pv, cur, *c, *p);
                    cur = *p;
                }
                PathSegment::Close => {
                    if pv.last() != Some(&start) {
                        pv.push(start);
                    }
                    cur = start;
                }
            }
        }
        if pv.len() > 1 && pv.last() != Some(&start) {
            pv.push(start);
        }
        if pv.len() < 3 {
            return;
        }
        let vs: Vec<Point> = pv.iter().map(|v| Point::new(rect.x + v.x, rect.y + v.y)).collect();
        let centroid = Point::new(
            vs.iter().map(|v| v.x).sum::<i32>() / vs.len() as i32,
            vs.iter().map(|v| v.y).sum::<i32>() / vs.len() as i32,
        );
        for i in 0..vs.len() {
            let j = (i + 1) % vs.len();
            self.fill_triangle_approx(context, centroid, vs[i], vs[j]);
        }
        if let Some(sc) = self.stroke_color {
            for i in 0..vs.len() {
                let j = (i + 1) % vs.len();
                context.draw_line_stroke(vs[i], vs[j], sc, self.stroke_width);
            }
        }
    }

    fn fill_triangle_approx(&self, context: &mut RenderContext, v0: Point, v1: Point, v2: Point) {
        let mut vs = [v0, v1, v2];
        vs.sort_by_key(|v| v.y);
        let [a, b, c] = vs;
        if a.y == c.y {
            let min_x = a.x.min(b.x).min(c.x);
            let max_x = a.x.max(b.x).max(c.x);
            if max_x > min_x {
                context
                    .fill_rect(Rect::new(min_x, a.y, (max_x - min_x) as u32, 1), self.fill_color);
            }
            return;
        }
        let total_height = c.y - a.y;
        if total_height <= 0 {
            return;
        }
        let lerp_x =
            |p1: Point, p2: Point, t: f32| -> f32 { p1.x as f32 + (p2.x - p1.x) as f32 * t };
        for y in a.y..=c.y {
            let (x1, x2) = if y < b.y {
                let sub_h = b.y - a.y;
                if sub_h == 0 {
                    continue;
                }
                (
                    lerp_x(a, c, (y - a.y) as f32 / total_height as f32),
                    lerp_x(a, b, (y - a.y) as f32 / sub_h as f32),
                )
            } else {
                let sub_h = c.y - b.y;
                if sub_h == 0 {
                    continue;
                }
                (
                    lerp_x(a, c, (y - a.y) as f32 / total_height as f32),
                    lerp_x(b, c, (y - b.y) as f32 / sub_h as f32),
                )
            };
            let x_start = x1.min(x2).round() as i32;
            let x_end = x1.max(x2).round() as i32;
            if x_end > x_start {
                context
                    .fill_rect(Rect::new(x_start, y, (x_end - x_start) as u32, 1), self.fill_color);
            }
        }
    }

    fn ray_cast(&self, point: Point, vertices: &[Point]) -> bool {
        let n = vertices.len();
        if n < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let vi = vertices[i];
            let vj = vertices[j];
            if ((vi.y > point.y) != (vj.y > point.y))
                && (point.x as i64)
                    < ((vj.x as i64 - vi.x as i64) * (point.y as i64 - vi.y as i64)
                        / (vj.y as i64 - vi.y as i64)
                        + vi.x as i64)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }
}

impl Widget for FreeformShapeWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for FreeformShapeWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        self.draw_shape(context);
        if !self.base.is_enabled() {
            let overlay = Color::from_rgba(200, 200, 200, 128);
            context.fill_rect(rect, overlay);
        }
    }
}

impl crate::event::EventHandler for FreeformShapeWidget {
    fn handle_event(&mut self, event: &crate::event::Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MouseMove { pos } => {
                let was_hovered = self.hovered;
                self.hovered = self.contains(*pos);
                if was_hovered != self.hovered {
                    self.hovered_changed.emit(self.hovered);
                    self.base.request_redraw();
                }
            }
            crate::event::Event::MouseEnter { .. } => {
                self.hovered = true;
                self.hovered_changed.emit(true);
                self.base.request_redraw();
            }
            crate::event::Event::MouseLeave { .. } => {
                self.hovered = false;
                self.pressed = false;
                self.hovered_changed.emit(false);
                self.pressed_changed.emit(false);
                self.base.request_redraw();
            }
            crate::event::Event::MousePress { pos, button }
                if *button == 1 && self.contains(*pos) =>
            {
                self.pressed = true;
                self.pressed_changed.emit(true);
                self.base.set_mouse_pressed(true);
                self.base.request_redraw();
            }
            crate::event::Event::MouseRelease { pos, button } if *button == 1 => {
                let was_pressed = self.pressed;
                self.pressed = false;
                self.base.set_mouse_pressed(false);
                if was_pressed && self.contains(*pos) {
                    self.clicked.emit();
                }
                self.pressed_changed.emit(false);
                self.base.request_redraw();
            }
            _ => {}
        }
    }
}
