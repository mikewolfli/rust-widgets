//! Object system and identity management.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::{CoreObject, ObjectId};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct Object {
    id: ObjectId,
    class_name: &'static str,
    ref_count: Arc<()>,
}

impl Object {
    pub fn new(class_name: &'static str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            class_name,
            ref_count: Arc::new(()),
        }
    }

    pub fn class_name(&self) -> &'static str {
        self.class_name
    }

    pub fn id(&self) -> ObjectId {
        self.id
    }

    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.ref_count)
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
