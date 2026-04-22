//! Hardware-adaptive buffer pool management for GPU/CPU rendering.
//!
//! This module provides a ring buffer-based staging buffer pool that automatically
//! adapts its configuration based on the detected GPU type:
//! - Discrete GPU: Large buffers, aggressive upload batching
//! - Integrated GPU: Medium buffers, memory bandwidth optimization
//! - CPU Software: Small buffers, CPU-cache friendly layout
//!
//! This module integrates with the existing memory pool system in `crate::memory`.

/// GPU memory profile based on device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuMemoryProfile {
    /// Discrete GPU profile - high memory, high bandwidth
    Discrete,
    /// Integrated GPU profile - shared memory, bandwidth constrained
    Integrated,
    /// CPU Software profile - CPU cache optimized
    Cpu,
}

impl GpuMemoryProfile {
    /// Creates a memory profile from GPU device type
    pub fn from_device_type(device_type: super::adapter::GpuDeviceType) -> Self {
        match device_type {
            super::adapter::GpuDeviceType::DiscreteGpu => Self::Discrete,
            super::adapter::GpuDeviceType::IntegratedGpu => Self::Integrated,
            _ => Self::Cpu,
        }
    }

    /// Returns the recommended buffer pool size
    pub fn buffer_pool_size(&self) -> usize {
        match self {
            Self::Discrete => 64 * 1024 * 1024,   // 64 MB for discrete GPU
            Self::Integrated => 16 * 1024 * 1024, // 16 MB for integrated GPU
            Self::Cpu => 4 * 1024 * 1024,         // 4 MB for CPU rendering
        }
    }

    /// Returns the number of ring buffer slots
    pub fn ring_buffer_slots(&self) -> usize {
        match self {
            Self::Discrete => 3,   // Triple buffering for discrete GPU
            Self::Integrated => 2, // Double buffering for integrated
            Self::Cpu => 2,        // Double buffering for CPU
        }
    }

    /// Returns the maximum upload batch size
    pub fn max_upload_batch_size(&self) -> usize {
        match self {
            Self::Discrete => 4 * 1024 * 1024,   // 4 MB batches
            Self::Integrated => 1 * 1024 * 1024, // 1 MB batches
            Self::Cpu => 256 * 1024,             // 256 KB batches
        }
    }

    /// Returns whether to merge small uploads
    pub fn merge_small_uploads(&self) -> bool {
        matches!(self, Self::Discrete | Self::Integrated)
    }

    /// Returns the small upload threshold
    pub fn small_upload_threshold(&self) -> usize {
        match self {
            Self::Discrete => 64 * 1024,   // 64 KB
            Self::Integrated => 16 * 1024, // 16 KB
            Self::Cpu => 4 * 1024,         // 4 KB
        }
    }

    /// Returns the mapping strategy
    pub fn mapping_strategy(&self) -> MappingStrategy {
        match self {
            Self::Discrete => MappingStrategy::PersistentMapped,
            Self::Integrated => MappingStrategy::PersistentMapped,
            Self::Cpu => MappingStrategy::WriteCombined,
        }
    }

    /// Returns the fence wait strategy
    pub fn fence_strategy(&self) -> FenceStrategy {
        match self {
            Self::Discrete => FenceStrategy::GpuTimestamp,
            Self::Integrated => FenceStrategy::CpuFence,
            Self::Cpu => FenceStrategy::CpuFence,
        }
    }

    /// Returns whether to use coherent memory
    pub fn use_coherent_memory(&self) -> bool {
        matches!(self, Self::Integrated | Self::Cpu)
    }
}

/// Buffer mapping strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingStrategy {
    /// Persistent mapped buffers for frequent updates
    PersistentMapped,
    /// Write-combined memory for CPU uploads
    WriteCombined,
    /// Cached memory for read-back
    Cached,
}

/// Fence synchronization strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceStrategy {
    /// GPU timestamp-based synchronization
    GpuTimestamp,
    /// CPU fence-based synchronization
    CpuFence,
    /// Spinlock for low-latency
    Spinlock,
}

/// Configuration for the staging buffer pool
#[derive(Debug, Clone)]
pub struct StagingBufferPoolConfig {
    /// Total pool size in bytes
    pub pool_size: usize,
    /// Number of ring buffer slots
    pub ring_slots: usize,
    /// Maximum upload batch size
    pub max_batch_size: usize,
    /// Whether to merge small uploads
    pub merge_uploads: bool,
    /// Small upload threshold
    pub small_upload_threshold: usize,
    /// Mapping strategy
    pub mapping_strategy: MappingStrategy,
    /// Fence strategy
    pub fence_strategy: FenceStrategy,
    /// Use coherent memory
    pub use_coherent_memory: bool,
    /// Alignment for buffer offsets
    pub alignment: usize,
}

impl StagingBufferPoolConfig {
    /// Creates a default configuration
    pub fn new() -> Self {
        Self::for_profile(GpuMemoryProfile::Discrete)
    }

    /// Creates a configuration for a specific GPU profile
    pub fn for_profile(profile: GpuMemoryProfile) -> Self {
        Self {
            pool_size: profile.buffer_pool_size(),
            ring_slots: profile.ring_buffer_slots(),
            max_batch_size: profile.max_upload_batch_size(),
            merge_uploads: profile.merge_small_uploads(),
            small_upload_threshold: profile.small_upload_threshold(),
            mapping_strategy: profile.mapping_strategy(),
            fence_strategy: profile.fence_strategy(),
            use_coherent_memory: profile.use_coherent_memory(),
            alignment: 256, // Standard GPU alignment
        }
    }

    /// Creates a configuration for discrete GPU
    pub fn discrete() -> Self {
        Self::for_profile(GpuMemoryProfile::Discrete)
    }

    /// Creates a configuration for integrated GPU
    pub fn integrated() -> Self {
        Self::for_profile(GpuMemoryProfile::Integrated)
    }

    /// Creates a configuration for CPU rendering
    pub fn cpu() -> Self {
        Self::for_profile(GpuMemoryProfile::Cpu)
    }

    /// Adjusts configuration based on available memory
    pub fn with_memory_limit(mut self, available_memory: usize) -> Self {
        // Ensure pool size doesn't exceed available memory
        let max_pool_size = available_memory / 4; // Use at most 25% of available memory
        self.pool_size = self.pool_size.min(max_pool_size);
        self
    }
}

impl Default for StagingBufferPoolConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// A single ring buffer slot
#[derive(Debug)]
pub struct GpuRingBufferSlot {
    /// Slot index
    pub index: usize,
    /// Buffer offset
    pub offset: usize,
    /// Buffer size
    pub size: usize,
    /// Whether this slot is currently in use
    pub in_use: bool,
    /// Frame index when this slot was last used
    pub last_used_frame: u64,
}

/// Staging buffer pool with ring buffer design for GPU uploads.
///
/// This is specialized for GPU staging buffers and complements the general
/// purpose `BufferPool` in `crate::memory::pool`.
pub struct GpuStagingBufferPool {
    config: StagingBufferPoolConfig,
    slots: Vec<GpuRingBufferSlot>,
    current_slot: usize,
    current_frame: u64,
    total_allocated: usize,
    total_used: usize,
    /// Optional reference to the system buffer pool for fallback
    fallback_pool: Option<crate::memory::BufferPool>,
}

impl GpuStagingBufferPool {
    /// Creates a new staging buffer pool with the given configuration
    pub fn new(config: StagingBufferPoolConfig) -> Self {
        let slot_size = config.pool_size / config.ring_slots;
        let mut slots = Vec::with_capacity(config.ring_slots);

        for i in 0..config.ring_slots {
            slots.push(GpuRingBufferSlot {
                index: i,
                offset: i * slot_size,
                size: slot_size,
                in_use: false,
                last_used_frame: 0,
            });
        }

        Self {
            config,
            slots,
            current_slot: 0,
            current_frame: 0,
            total_allocated: 0,
            total_used: 0,
            fallback_pool: None,
        }
    }

    /// Creates a pool optimized for the given GPU type
    pub fn for_gpu_type(device_type: super::adapter::GpuDeviceType) -> Self {
        let profile = GpuMemoryProfile::from_device_type(device_type);
        Self::new(StagingBufferPoolConfig::for_profile(profile))
    }

    /// Sets a fallback buffer pool for overflow allocations
    pub fn with_fallback_pool(mut self, pool: crate::memory::BufferPool) -> Self {
        self.fallback_pool = Some(pool);
        self
    }

    /// Advances to the next frame
    pub fn next_frame(&mut self) {
        self.current_frame += 1;

        // Mark current slot as used and move to next
        if let Some(slot) = self.slots.get_mut(self.current_slot) {
            slot.in_use = true;
            slot.last_used_frame = self.current_frame;
        }

        self.current_slot = (self.current_slot + 1) % self.config.ring_slots;

        // Reset the new current slot
        if let Some(slot) = self.slots.get_mut(self.current_slot) {
            slot.in_use = false;
            self.total_used = 0;
        }
    }

    /// Allocates a buffer from the current slot
    pub fn allocate(&mut self, size: usize) -> Option<GpuBufferAllocation> {
        let aligned_size = (size + self.config.alignment - 1) & !(self.config.alignment - 1);

        // Check if we should merge small uploads
        if self.config.merge_uploads && size < self.config.small_upload_threshold {
            // Try to merge with existing allocation
            if let Some(allocation) = self.try_merge_allocate(size) {
                return Some(allocation);
            }
        }

        // Check batch size limit
        if aligned_size > self.config.max_batch_size {
            // Try fallback pool for large allocations
            return self.allocate_fallback(size);
        }

        let slot = self.slots.get(self.current_slot)?;

        if self.total_used + aligned_size > slot.size {
            // Pool exhausted, try fallback
            return self.allocate_fallback(size);
        }

        let offset = slot.offset + self.total_used;
        self.total_used += aligned_size;
        self.total_allocated += aligned_size;

        Some(GpuBufferAllocation {
            slot_index: self.current_slot,
            offset,
            size: aligned_size,
            frame_index: self.current_frame,
            is_fallback: false,
        })
    }

    /// Allocates from fallback pool
    fn allocate_fallback(&mut self, size: usize) -> Option<GpuBufferAllocation> {
        if let Some(ref mut pool) = self.fallback_pool {
            let buffer = pool.acquire_sized(size);
            // Return a special allocation indicating fallback
            Some(GpuBufferAllocation {
                slot_index: usize::MAX, // Marker for fallback
                offset: 0,
                size: buffer.len(),
                frame_index: self.current_frame,
                is_fallback: true,
            })
        } else {
            None
        }
    }

    /// Tries to merge a small allocation with existing data in the current slot
    fn try_merge_allocate(&mut self, size: usize) -> Option<GpuBufferAllocation> {
        let aligned_size = (size + self.config.alignment - 1) & !(self.config.alignment - 1);

        // Get current slot
        let slot = self.slots.get(self.current_slot)?;

        // Check if we can merge with existing allocation
        if slot.in_use {
            // Slot is already in use, cannot merge
            return None;
        }

        // Check if there's enough remaining space in the current slot
        let remaining_space = slot.size - self.total_used;
        if remaining_space >= aligned_size {
            // Merge with existing allocation
            let offset = self.total_used;
            self.total_used += aligned_size;
            self.total_allocated += aligned_size;

            Some(GpuBufferAllocation {
                slot_index: slot.index,
                offset,
                size: aligned_size,
                frame_index: self.current_frame,
                is_fallback: false,
            })
        } else {
            // Not enough space, cannot merge
            None
        }
    }

    /// Returns the current frame index
    pub fn current_frame(&self) -> u64 {
        self.current_frame
    }

    /// Returns the pool configuration
    pub fn config(&self) -> &StagingBufferPoolConfig {
        &self.config
    }

    /// Returns memory statistics
    pub fn memory_stats(&self) -> GpuBufferPoolStats {
        GpuBufferPoolStats {
            total_size: self.config.pool_size,
            used_size: self.total_used,
            allocated_size: self.total_allocated,
            slot_count: self.config.ring_slots,
            current_slot: self.current_slot,
            current_frame: self.current_frame,
            fallback_used: self
                .fallback_pool
                .as_ref()
                .map(|p| p.available())
                .unwrap_or(0),
        }
    }

    /// Waits for a slot to be available (CPU fence)
    pub fn wait_for_slot(&mut self, slot_index: usize) {
        // In a real implementation, this would wait on a GPU fence
        // For now, we just ensure the slot is from a previous frame
        if let Some(slot) = self.slots.get(slot_index) {
            if slot.in_use && slot.last_used_frame >= self.current_frame {
                // Slot is still in use, would wait here
            }
        }
    }
}

/// GPU buffer allocation info
#[derive(Debug, Clone, Copy)]
pub struct GpuBufferAllocation {
    /// Slot index (usize::MAX indicates fallback allocation)
    pub slot_index: usize,
    /// Offset within the buffer
    pub offset: usize,
    /// Size of the allocation
    pub size: usize,
    /// Frame index when allocated
    pub frame_index: u64,
    /// Whether this is a fallback allocation
    pub is_fallback: bool,
}

/// GPU buffer pool statistics
#[derive(Debug, Clone, Copy)]
pub struct GpuBufferPoolStats {
    /// Total pool size
    pub total_size: usize,
    /// Currently used size in active slot
    pub used_size: usize,
    /// Total allocated since creation
    pub allocated_size: usize,
    /// Number of slots
    pub slot_count: usize,
    /// Current slot index
    pub current_slot: usize,
    /// Current frame index
    pub current_frame: u64,
    /// Available buffers in fallback pool
    pub fallback_used: usize,
}

/// Hardware-adaptive upload batcher
pub struct GpuUploadBatcher {
    config: StagingBufferPoolConfig,
    pending_uploads: Vec<GpuPendingUpload>,
    current_batch_size: usize,
}

#[derive(Debug)]
struct GpuPendingUpload {
    data: Vec<u8>,
    destination_offset: usize,
}

impl GpuUploadBatcher {
    /// Creates a new upload batcher
    pub fn new(config: StagingBufferPoolConfig) -> Self {
        Self {
            config,
            pending_uploads: Vec::new(),
            current_batch_size: 0,
        }
    }

    /// Adds an upload to the batch
    pub fn add_upload(&mut self, data: Vec<u8>, destination_offset: usize) -> bool {
        let size = data.len();

        // Check if adding this would exceed batch size
        if self.current_batch_size + size > self.config.max_batch_size {
            return false; // Would exceed batch size
        }

        // Check if we should merge
        if self.config.merge_uploads && size < self.config.small_upload_threshold {
            // Try to find adjacent upload to merge with
            if let Some(merged) = self.try_merge(&data, destination_offset) {
                self.current_batch_size += merged;
                return true;
            }
        }

        self.pending_uploads.push(GpuPendingUpload {
            data,
            destination_offset,
        });
        self.current_batch_size += size;

        true
    }

    /// Tries to merge with existing pending uploads
    fn try_merge(&mut self, data: &[u8], offset: usize) -> Option<usize> {
        for upload in &mut self.pending_uploads {
            let end = upload.destination_offset + upload.data.len();
            if end == offset {
                // Adjacent, can merge
                upload.data.extend_from_slice(data);
                return Some(data.len());
            }
        }
        None
    }

    /// Returns the current batch size
    pub fn batch_size(&self) -> usize {
        self.current_batch_size
    }

    /// Returns true if the batch is full
    pub fn is_full(&self) -> bool {
        self.current_batch_size >= self.config.max_batch_size
    }

    /// Clears the batch and returns all pending uploads
    pub fn flush(&mut self) -> Vec<(Vec<u8>, usize)> {
        let uploads = self
            .pending_uploads
            .drain(..)
            .map(|u| (u.data, u.destination_offset))
            .collect();
        self.current_batch_size = 0;
        uploads
    }

    /// Returns the number of pending uploads
    pub fn pending_count(&self) -> usize {
        self.pending_uploads.len()
    }
}

/// Performance monitor for GPU buffer pool
pub struct GpuBufferPoolMonitor {
    stats_history: Vec<GpuBufferPoolStats>,
    max_history: usize,
}

impl GpuBufferPoolMonitor {
    /// Creates a new monitor
    pub fn new(max_history: usize) -> Self {
        Self {
            stats_history: Vec::with_capacity(max_history),
            max_history,
        }
    }

    /// Records a stats sample
    pub fn record(&mut self, stats: GpuBufferPoolStats) {
        if self.stats_history.len() >= self.max_history {
            self.stats_history.remove(0);
        }
        self.stats_history.push(stats);
    }

    /// Returns the average utilization
    pub fn average_utilization(&self) -> f32 {
        if self.stats_history.is_empty() {
            return 0.0;
        }

        let total: f32 = self
            .stats_history
            .iter()
            .map(|s| s.used_size as f32 / s.total_size as f32)
            .sum();

        total / self.stats_history.len() as f32
    }

    /// Returns true if the pool is under memory pressure
    pub fn is_under_pressure(&self) -> bool {
        if self.stats_history.len() < 3 {
            return false;
        }

        // Check if recent utilization is consistently high
        let recent: Vec<_> = self.stats_history.iter().rev().take(3).collect();
        recent
            .iter()
            .all(|s| s.used_size as f32 / s.total_size as f32 > 0.8)
    }

    /// Returns true if the pool is underutilized
    pub fn is_underutilized(&self) -> bool {
        if self.stats_history.len() < 10 {
            return false;
        }

        self.average_utilization() < 0.3
    }

    /// Returns true if fallback pool is being used frequently
    pub fn is_fallback_heavy(&self) -> bool {
        if self.stats_history.len() < 5 {
            return false;
        }

        let recent: Vec<_> = self.stats_history.iter().rev().take(5).collect();
        recent.iter().any(|s| s.fallback_used > 0)
    }
}

/// Integration with the existing memory pool system
pub mod integration {
    use super::*;
    use crate::memory::{BufferPool, PoolConfig};

    /// Creates a GPU-optimized buffer pool configuration
    pub fn create_gpu_buffer_pool_config(profile: GpuMemoryProfile) -> PoolConfig {
        let _buffer_size = match profile {
            GpuMemoryProfile::Discrete => 4 * 1024 * 1024, // 4 MB
            GpuMemoryProfile::Integrated => 1 * 1024 * 1024, // 1 MB
            GpuMemoryProfile::Cpu => 256 * 1024,           // 256 KB
        };

        PoolConfig {
            initial_size: profile.ring_buffer_slots(),
            max_size: profile.ring_buffer_slots() * 2,
            growth_factor: 1.0, // Fixed size for GPU buffers
        }
    }

    /// Creates a fallback buffer pool for GPU staging
    pub fn create_fallback_pool(profile: GpuMemoryProfile) -> BufferPool {
        let buffer_size = match profile {
            GpuMemoryProfile::Discrete => 4 * 1024 * 1024,
            GpuMemoryProfile::Integrated => 1 * 1024 * 1024,
            GpuMemoryProfile::Cpu => 256 * 1024,
        };

        BufferPool::new(
            buffer_size,
            profile.ring_buffer_slots(),
            profile.ring_buffer_slots() * 2,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::adapter::GpuDeviceType;
    use super::*;

    #[test]
    fn test_gpu_memory_profile_discrete() {
        let profile = GpuMemoryProfile::Discrete;
        assert_eq!(profile.buffer_pool_size(), 64 * 1024 * 1024);
        assert_eq!(profile.ring_buffer_slots(), 3);
        assert!(profile.merge_small_uploads());
    }

    #[test]
    fn test_gpu_memory_profile_integrated() {
        let profile = GpuMemoryProfile::Integrated;
        assert_eq!(profile.buffer_pool_size(), 16 * 1024 * 1024);
        assert_eq!(profile.ring_buffer_slots(), 2);
        assert!(profile.use_coherent_memory());
    }

    #[test]
    fn test_gpu_memory_profile_cpu() {
        let profile = GpuMemoryProfile::Cpu;
        assert_eq!(profile.buffer_pool_size(), 4 * 1024 * 1024);
        assert_eq!(profile.ring_buffer_slots(), 2);
        assert!(!profile.merge_small_uploads());
    }

    #[test]
    fn test_buffer_pool_allocation() {
        let config = StagingBufferPoolConfig::discrete();
        let mut pool = GpuStagingBufferPool::new(config);

        let allocation = pool.allocate(1024).unwrap();
        assert_eq!(allocation.slot_index, 0);
        assert_eq!(allocation.offset, 0);
        assert!(allocation.size >= 1024);
        assert!(!allocation.is_fallback);
    }

    #[test]
    fn test_buffer_pool_ring_rotation() {
        let config = StagingBufferPoolConfig::discrete();
        let mut pool = GpuStagingBufferPool::new(config);

        pool.next_frame();
        assert_eq!(pool.current_frame(), 1);

        pool.next_frame();
        assert_eq!(pool.current_frame(), 2);
    }

    #[test]
    fn test_upload_batcher() {
        let config = StagingBufferPoolConfig::discrete();
        let mut batcher = GpuUploadBatcher::new(config);

        assert!(batcher.add_upload(vec![0u8; 1024], 0));
        assert_eq!(batcher.batch_size(), 1024);

        let uploads = batcher.flush();
        assert_eq!(uploads.len(), 1);
        assert!(batcher.batch_size() == 0);
    }

    #[test]
    fn test_buffer_pool_monitor() {
        let mut monitor = GpuBufferPoolMonitor::new(10);

        let stats = GpuBufferPoolStats {
            total_size: 1024,
            used_size: 512,
            allocated_size: 512,
            slot_count: 3,
            current_slot: 0,
            current_frame: 1,
            fallback_used: 0,
        };

        monitor.record(stats);
        assert_eq!(monitor.average_utilization(), 0.5);
    }

    #[test]
    fn test_from_device_type() {
        assert_eq!(
            GpuMemoryProfile::from_device_type(GpuDeviceType::DiscreteGpu),
            GpuMemoryProfile::Discrete
        );
        assert_eq!(
            GpuMemoryProfile::from_device_type(GpuDeviceType::IntegratedGpu),
            GpuMemoryProfile::Integrated
        );
        assert_eq!(
            GpuMemoryProfile::from_device_type(GpuDeviceType::Cpu),
            GpuMemoryProfile::Cpu
        );
    }

    #[test]
    fn test_integration_config() {
        let config = integration::create_gpu_buffer_pool_config(GpuMemoryProfile::Discrete);
        assert_eq!(config.initial_size, 3);
        assert_eq!(config.max_size, 6);
    }
}
