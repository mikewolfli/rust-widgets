use std::sync::{Arc, Mutex};

pub trait Poolable: Default + Clone {
    fn reset(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoolConfig {
    pub initial_size: usize,
    pub max_size: usize,
    pub growth_factor: f32,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            initial_size: 16,
            max_size: 1024,
            growth_factor: 1.5,
        }
    }
}

pub struct ObjectPool<T: Poolable> {
    pool: Vec<T>,
    config: PoolConfig,
    allocated: usize,
}

impl<T: Poolable> ObjectPool<T> {
    pub fn new(config: PoolConfig) -> Self {
        let mut pool = Vec::with_capacity(config.initial_size);
        for _ in 0..config.initial_size {
            pool.push(T::default());
        }

        Self {
            pool,
            config,
            allocated: 0,
        }
    }

    pub fn acquire(&mut self) -> T {
        if let Some(mut obj) = self.pool.pop() {
            obj.reset();
            self.allocated += 1;
            obj
        } else if self.pool.len() < self.config.max_size {
            self.allocated += 1;
            T::default()
        } else {
            self.allocated += 1;
            T::default()
        }
    }

    pub fn release(&mut self, mut obj: T) {
        obj.reset();
        if self.pool.len() < self.config.max_size {
            self.pool.push(obj);
        }
        self.allocated = self.allocated.saturating_sub(1);
    }

    pub fn available(&self) -> usize {
        self.pool.len()
    }

    pub fn allocated(&self) -> usize {
        self.allocated
    }

    pub fn capacity(&self) -> usize {
        self.pool.capacity()
    }

    pub fn clear(&mut self) {
        self.pool.clear();
        self.allocated = 0;
    }

    pub fn shrink_to_fit(&mut self) {
        self.pool.shrink_to_fit();
    }
}

impl<T: Poolable> Default for ObjectPool<T> {
    fn default() -> Self {
        Self::new(PoolConfig::default())
    }
}

pub struct SharedPool<T: Poolable + Send> {
    pool: Arc<Mutex<ObjectPool<T>>>,
}

impl<T: Poolable + Send> SharedPool<T> {
    pub fn new(config: PoolConfig) -> Self {
        Self {
            pool: Arc::new(Mutex::new(ObjectPool::new(config))),
        }
    }

    pub fn acquire(&self) -> T {
        self.pool.lock().unwrap().acquire()
    }

    pub fn release(&self, obj: T) {
        self.pool.lock().unwrap().release(obj);
    }

    pub fn stats(&self) -> PoolStats {
        let pool = self.pool.lock().unwrap();
        PoolStats {
            available: pool.available(),
            allocated: pool.allocated(),
            capacity: pool.capacity(),
        }
    }
}

impl<T: Poolable + Send> Clone for SharedPool<T> {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
        }
    }
}

impl<T: Poolable + Send> Default for SharedPool<T> {
    fn default() -> Self {
        Self::new(PoolConfig::default())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    pub available: usize,
    pub allocated: usize,
    pub capacity: usize,
}

pub struct PoolManager {
    pools: Vec<Box<dyn std::any::Any + Send>>,
}

impl PoolManager {
    pub fn new() -> Self {
        Self { pools: Vec::new() }
    }

    pub fn register<T: Poolable + Send + 'static>(&mut self, config: PoolConfig) -> SharedPool<T> {
        let pool = SharedPool::new(config);
        self.pools.push(Box::new(pool.clone()));
        pool
    }

    pub fn clear_all(&mut self) {
        self.pools.clear();
    }
}

impl Default for PoolManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BufferPool {
    buffers: Vec<Vec<u8>>,
    buffer_size: usize,
    max_buffers: usize,
}

impl BufferPool {
    pub fn new(buffer_size: usize, initial_count: usize, max_buffers: usize) -> Self {
        let mut buffers = Vec::with_capacity(initial_count);
        for _ in 0..initial_count {
            buffers.push(vec![0u8; buffer_size]);
        }

        Self {
            buffers,
            buffer_size,
            max_buffers,
        }
    }

    pub fn acquire(&mut self) -> Vec<u8> {
        self.buffers
            .pop()
            .unwrap_or_else(|| vec![0u8; self.buffer_size])
    }

    pub fn acquire_sized(&mut self, size: usize) -> Vec<u8> {
        if size <= self.buffer_size {
            let mut buf = self.acquire();
            buf.clear();
            buf.resize(size, 0);
            buf
        } else {
            vec![0u8; size]
        }
    }

    pub fn release(&mut self, mut buffer: Vec<u8>) {
        if buffer.capacity() == self.buffer_size && self.buffers.len() < self.max_buffers {
            buffer.clear();
            self.buffers.push(buffer);
        }
    }

    pub fn available(&self) -> usize {
        self.buffers.len()
    }

    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    pub fn clear(&mut self) {
        self.buffers.clear();
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new(4096, 4, 64)
    }
}

pub struct StringPool {
    strings: Vec<String>,
    default_capacity: usize,
    max_strings: usize,
}

impl StringPool {
    pub fn new(default_capacity: usize, initial_count: usize, max_strings: usize) -> Self {
        let mut strings = Vec::with_capacity(initial_count);
        for _ in 0..initial_count {
            strings.push(String::with_capacity(default_capacity));
        }

        Self {
            strings,
            default_capacity,
            max_strings,
        }
    }

    pub fn acquire(&mut self) -> String {
        self.strings
            .pop()
            .unwrap_or_else(|| String::with_capacity(self.default_capacity))
    }

    pub fn release(&mut self, mut s: String) {
        s.clear();
        if s.capacity() >= self.default_capacity && self.strings.len() < self.max_strings {
            self.strings.push(s);
        }
    }

    pub fn available(&self) -> usize {
        self.strings.len()
    }

    pub fn clear(&mut self) {
        self.strings.clear();
    }
}

impl Default for StringPool {
    fn default() -> Self {
        Self::new(64, 8, 128)
    }
}

pub struct VecPool<T> {
    vecs: Vec<Vec<T>>,
    default_capacity: usize,
    max_vecs: usize,
}

impl<T> VecPool<T> {
    pub fn new(default_capacity: usize, initial_count: usize, max_vecs: usize) -> Self {
        let mut vecs = Vec::with_capacity(initial_count);
        for _ in 0..initial_count {
            vecs.push(Vec::with_capacity(default_capacity));
        }

        Self {
            vecs,
            default_capacity,
            max_vecs,
        }
    }

    pub fn acquire(&mut self) -> Vec<T> {
        self.vecs
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(self.default_capacity))
    }

    pub fn release(&mut self, mut v: Vec<T>) {
        v.clear();
        if v.capacity() >= self.default_capacity && self.vecs.len() < self.max_vecs {
            self.vecs.push(v);
        }
    }

    pub fn available(&self) -> usize {
        self.vecs.len()
    }

    pub fn clear(&mut self) {
        self.vecs.clear();
    }
}

impl<T> Default for VecPool<T> {
    fn default() -> Self {
        Self::new(16, 4, 64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default, Clone)]
    struct TestObject {
        value: i32,
        data: Vec<u8>,
    }

    impl Poolable for TestObject {
        fn reset(&mut self) {
            self.value = 0;
            self.data.clear();
        }
    }

    #[test]
    fn test_object_pool() {
        let mut pool: ObjectPool<TestObject> = ObjectPool::new(PoolConfig {
            initial_size: 4,
            max_size: 8,
            growth_factor: 1.5,
        });

        assert_eq!(pool.available(), 4);

        let obj1 = pool.acquire();
        assert_eq!(pool.allocated(), 1);

        pool.release(obj1);
        assert_eq!(pool.available(), 4);
    }

    #[test]
    fn test_buffer_pool() {
        let mut pool = BufferPool::new(1024, 2, 4);

        let buf1 = pool.acquire();
        assert_eq!(buf1.len(), 1024);

        pool.release(buf1);
        assert_eq!(pool.available(), 2);
    }
}
