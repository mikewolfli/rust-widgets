//! Index-based widget registry (ObjectId lookup).
//!
//! Provides a `WidgetRegistry` that maps `ObjectId` → metadata for
//! runtime widget introspection, cross-module lookup, and debugging.

mod registry;

pub use registry::{WidgetEntry, WidgetKind, WidgetRegistry};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_register_and_get() {
        let mut reg = WidgetRegistry::new();
        let id = 1;
        let entry = WidgetEntry {
            id,
            kind: WidgetKind::Button,
            parent: None,
            label: "Click me".to_string(),
        };
        reg.register(entry);
        let fetched = reg.get(id);
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().label, "Click me");
    }
    #[test]
    fn registry_find_by_kind() {
        let mut reg = WidgetRegistry::new();
        let id1 = 10;
        let id2 = 20;
        reg.register(WidgetEntry {
            id: id1,
            kind: WidgetKind::Label,
            parent: None,
            label: "Label 1".to_string(),
        });
        reg.register(WidgetEntry {
            id: id2,
            kind: WidgetKind::Label,
            parent: None,
            label: "Label 2".to_string(),
        });
        assert_eq!(reg.find_by_kind(WidgetKind::Label).len(), 2);
        assert!(reg.find_by_kind(WidgetKind::Button).is_empty());
    }
    #[test]
    fn registry_unregister() {
        let mut reg = WidgetRegistry::new();
        let id = 30;
        reg.register(WidgetEntry {
            id,
            kind: WidgetKind::Window,
            parent: None,
            label: "Main".to_string(),
        });
        assert!(reg.get(id).is_some());
        reg.unregister(id);
        assert!(reg.get(id).is_none());
    }
}
