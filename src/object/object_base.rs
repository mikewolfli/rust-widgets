use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::core::{CoreObject, ObjectId};

use super::PropertyValue;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Lightweight object identity and dynamic property container.
#[derive(Debug, Clone)]
pub struct Object {
    /// Unique object id allocated from global atomic counter.
    id: ObjectId,
    /// Lightweight runtime class tag.
    class_name: &'static str,
    /// Shared marker used for reference-count introspection.
    ref_count: Arc<()>,
    /// Dynamic property bag for reflection-style metadata.
    properties: Arc<Mutex<HashMap<String, PropertyValue>>>,
}

impl Object {
    /// Create an object with generated id and class name.
    pub fn new(class_name: &'static str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            class_name,
            ref_count: Arc::new(()),
            properties: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns runtime class name.
    pub fn class_name(&self) -> &'static str {
        self.class_name
    }

    /// Returns stable object id.
    pub fn id(&self) -> ObjectId {
        self.id
    }

    /// Returns strong reference count for internal marker.
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.ref_count)
    }

    /// Set or replace a dynamic property.
    pub fn set_property(&self, key: impl Into<String>, value: PropertyValue) {
        self.properties
            .lock()
            .expect("object properties lock poisoned")
            .insert(key.into(), value);
    }

    /// Get a dynamic property by key.
    pub fn property(&self, key: &str) -> Option<PropertyValue> {
        self.properties
            .lock()
            .expect("object properties lock poisoned")
            .get(key)
            .cloned()
    }

    /// Remove a dynamic property.
    pub fn remove_property(&self, key: &str) -> Option<PropertyValue> {
        self.properties
            .lock()
            .expect("object properties lock poisoned")
            .remove(key)
    }

    /// List all known property keys.
    pub fn property_keys(&self) -> Vec<String> {
        self.properties
            .lock()
            .expect("object properties lock poisoned")
            .keys()
            .cloned()
            .collect()
    }
}

impl CoreObject for Object {
    fn id(&self) -> ObjectId {
        self.id
    }

    fn set_id(&mut self, id: ObjectId) {
        self.id = id;
    }
}
