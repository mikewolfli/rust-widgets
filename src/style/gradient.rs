use crate::core::{Color, Point};
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GradientType {
    #[default]
    Linear,
    Radial,
    Conic,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    pub position: f32,
    pub color: Color,
}
impl GradientStop {
    pub fn new(position: f32, color: Color) -> Self {
        Self { position: position.clamp(0.0, 1.0), color }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    pub gradient_type: GradientType,
    pub stops: Vec<GradientStop>,
    pub start_point: Point,
    pub end_point: Point,
    pub angle: f32,
    pub center: Point,
    pub radius: f32,
}
impl Gradient {
    pub fn linear(start: Point, end: Point) -> Self {
        Self {
            gradient_type: GradientType::Linear,
            stops: Vec::new(),
            start_point: start,
            end_point: end,
            angle: 0.0,
            center: Point::new(0, 0),
            radius: 0.0,
        }
    }
    pub fn radial(center: Point, radius: f32) -> Self {
        Self {
            gradient_type: GradientType::Radial,
            stops: Vec::new(),
            start_point: Point::new(0, 0),
            end_point: Point::new(0, 0),
            angle: 0.0,
            center,
            radius,
        }
    }
    pub fn conic(center: Point, angle: f32) -> Self {
        Self {
            gradient_type: GradientType::Conic,
            stops: Vec::new(),
            start_point: Point::new(0, 0),
            end_point: Point::new(0, 0),
            angle,
            center,
            radius: 0.0,
        }
    }
    pub fn with_angle(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }
    pub fn add_stop(mut self, position: f32, color: Color) -> Self {
        self.stops.push(GradientStop::new(position, color));
        self.stops.sort_by(|a, b| a.position.total_cmp(&b.position));
        self
    }
    pub fn with_stops(mut self, stops: Vec<GradientStop>) -> Self {
        self.stops = stops;
        self.stops.sort_by(|a, b| a.position.total_cmp(&b.position));
        self
    }
    pub fn interpolate(&self, position: f32) -> Color {
        let position = position.clamp(0.0, 1.0);
        if self.stops.is_empty() {
            return Color::TRANSPARENT;
        }
        if self.stops.len() == 1 {
            return self.stops[0].color;
        }
        if position <= self.stops[0].position {
            return self.stops[0].color;
        }
        if position >= self.stops[self.stops.len() - 1].position {
            return self.stops[self.stops.len() - 1].color;
        }
        for i in 0..self.stops.len() - 1 {
            let current = &self.stops[i];
            let next = &self.stops[i + 1];
            if position >= current.position && position <= next.position {
                let t = (position - current.position) / (next.position - current.position);
                return Self::interpolate_color(current.color, next.color, t);
            }
        }
        self.stops[self.stops.len() - 1].color
    }
    fn interpolate_color(from: Color, to: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let r = ((1.0 - t) * from.r as f32 + t * to.r as f32) as u8;
        let g = ((1.0 - t) * from.g as f32 + t * to.g as f32) as u8;
        let b = ((1.0 - t) * from.b as f32 + t * to.b as f32) as u8;
        let a = ((1.0 - t) * from.a as f32 + t * to.a as f32) as u8;
        Color::rgba(r, g, b, a)
    }
    pub fn reverse(&self) -> Self {
        let mut reversed = self.clone();
        reversed.stops.reverse();
        for stop in &mut reversed.stops {
            stop.position = 1.0 - stop.position;
        }
        reversed
    }
    pub fn is_valid(&self) -> bool {
        self.stops.len() >= 2
    }
}
impl Default for Gradient {
    fn default() -> Self {
        Self::linear(Point::new(0, 0), Point::new(100, 0))
    }
}
pub struct GradientBuilder {
    gradient: Gradient,
}
impl GradientBuilder {
    pub fn linear(start: Point, end: Point) -> Self {
        Self { gradient: Gradient::linear(start, end) }
    }
    pub fn radial(center: Point, radius: f32) -> Self {
        Self { gradient: Gradient::radial(center, radius) }
    }
    pub fn conic(center: Point, angle: f32) -> Self {
        Self { gradient: Gradient::conic(center, angle) }
    }
    pub fn stop(mut self, position: f32, color: Color) -> Self {
        self.gradient.stops.push(GradientStop::new(position, color));
        self
    }
    pub fn build(mut self) -> Gradient {
        self.gradient.stops.sort_by(|a, b| a.position.total_cmp(&b.position));
        self.gradient
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_gradient_creation() {
        let gradient = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
            .add_stop(0.0, Color::RED)
            .add_stop(1.0, Color::BLUE);
        assert_eq!(gradient.gradient_type, GradientType::Linear);
        assert_eq!(gradient.stops.len(), 2);
    }
    #[test]
    fn test_gradient_interpolation() {
        let gradient = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
            .add_stop(0.0, Color { r: 255, g: 0, b: 0, a: 255 })
            .add_stop(1.0, Color { r: 0, g: 0, b: 255, a: 255 });
        let mid_color = gradient.interpolate(0.5);
        assert_eq!(mid_color.r, 127);
        assert_eq!(mid_color.g, 0);
        assert_eq!(mid_color.b, 127);
    }
    #[test]
    fn test_gradient_reverse() {
        let gradient = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
            .add_stop(0.0, Color::RED)
            .add_stop(1.0, Color::BLUE);
        let reversed = gradient.reverse();
        assert_eq!(reversed.stops[0].color, Color::BLUE);
        assert_eq!(reversed.stops[1].color, Color::RED);
    }
    #[test]
    fn test_gradient_builder() {
        let gradient = GradientBuilder::linear(Point::new(0, 0), Point::new(100, 100))
            .stop(0.0, Color::WHITE)
            .stop(0.5, Color::GRAY)
            .stop(1.0, Color::BLACK)
            .build();
        assert_eq!(gradient.stops.len(), 3);
    }
}
