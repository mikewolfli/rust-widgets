/// Dynamic property value used by reflective object metadata APIs.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// Boolean scalar value.
    Bool(bool),
    /// Signed integer scalar value.
    Int(i64),
    /// Floating-point scalar value.
    Float(f64),
    /// UTF-8 string scalar value.
    String(String),
}
