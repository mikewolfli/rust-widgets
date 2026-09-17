// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Object system and identity management.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/widget/base.rs:1` (every `BaseWidget` owns an `Object` identity).
mod object_base;
mod properties;
pub use crate::core::ObjectId;
pub use object_base::Object;
pub use properties::PropertyValue;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_creation() {
        let obj = Object::new("test");
        assert_eq!(obj.class_name(), "test");
    }
    #[test]
    fn object_id_unique() {
        let o1 = Object::new("a");
        let o2 = Object::new("b");
        assert_ne!(o1.id(), o2.id());
    }
}
