// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Hardware-adaptive buffer pool management for GPU/CPU rendering.
//!
//! This module provides a ring buffer-based staging buffer pool that automatically
//! adapts its configuration based on the detected GPU type:
//! - Discrete GPU: Large buffers, aggressive upload batching
//! - Integrated GPU: Medium buffers, memory bandwidth optimization
//! - CPU Software: Small buffers, CPU-cache friendly layout
//!
//! This module integrates with the existing memory pool system in `crate::memory`.
use crate::compat::Vec;
use core::fmt;
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
            Self::Discrete => 4 * 1024 * 1024, // 4 MB batches
            Self::Integrated => 1024 * 1024,   // 1 MB batches
            Self::Cpu => 256 * 1024,           // 256 KB batches
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
/// Errors produced when configuring a [`GpuStagingBufferPool`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBufferPoolError {
    /// `ring_slots` must be at least one; zero would divide by zero when the
    /// pool is carved into slots.
    NoRingSlots,
    /// `alignment` must be a non-zero power of two; any other value would make
    /// the alignment mask wrap or fail to align real addresses.
    InvalidAlignment,
}
impl fmt::Display for GpuBufferPoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRingSlots => write!(f, "ring_slots must be at least one"),
            Self::InvalidAlignment => write!(f, "alignment must be a non-zero power of two"),
        }
    }
}
#[cfg(not(alloc_frugal))]
impl core::error::Error for GpuBufferPoolError {}
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
    /// Validates the configuration, rejecting values that would otherwise
    /// divide by zero or overflow inside the pool.
    pub fn validate(&self) -> Result<(), GpuBufferPoolError> {
        if self.ring_slots == 0 {
            return Err(GpuBufferPoolError::NoRingSlots);
        }
        if self.alignment == 0 || !self.alignment.is_power_of_two() {
            return Err(GpuBufferPoolError::InvalidAlignment);
        }
        Ok(())
    }
}
crate::impl_default_via_new!(StagingBufferPoolConfig);
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
/// Rounds `value` up to the next multiple of `alignment` (a power of two),
/// returning `None` on overflow so a `usize::MAX` request is rejected instead of
/// wrapping. `alignment` is validated to be a non-zero power of two before this
/// helper is reached, so the mask is well-formed.
fn align_up_checked(value: usize, alignment: usize) -> Option<usize> {
    value.checked_add(alignment - 1).map(|v| v & !(alignment - 1))
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
    /// Monotonic counter distinguishing simultaneously-live fallback handles.
    next_fallback_id: u64,
}
impl GpuStagingBufferPool {
    /// Creates a new staging buffer pool with the given configuration.
    ///
    /// Returns an error when the configuration is invalid (`ring_slots == 0` or
    /// a non-power-of-two `alignment`); a rejected config never leaves a
    /// half-constructed pool behind.
    pub fn new(config: StagingBufferPoolConfig) -> Result<Self, GpuBufferPoolError> {
        config.validate()?;
        let alignment = config.alignment;
        // Carve the pool into slots whose base offset is a multiple of
        // `alignment`, so every allocation handed out starts on a real aligned
        // address (not merely an aligned offset within an unaligned slot).
        let slot_size = (config.pool_size / config.ring_slots) & !(alignment - 1);
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
        Ok(Self {
            config,
            slots,
            current_slot: 0,
            current_frame: 0,
            total_allocated: 0,
            total_used: 0,
            fallback_pool: None,
            next_fallback_id: 0,
        })
    }
    /// Creates a pool optimized for the given GPU type.
    ///
    /// The profile-derived configuration is always valid (a non-zero slot count
    /// and a power-of-two alignment), so this never fails; it delegates to
    /// [`Self::new`] and unwraps the validated result.
    pub fn for_gpu_type(device_type: super::adapter::GpuDeviceType) -> Self {
        let profile = GpuMemoryProfile::from_device_type(device_type);
        Self::new(StagingBufferPoolConfig::for_profile(profile))
            .expect("profile-derived pool configuration is always valid")
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
        let aligned_size = align_up_checked(size, self.config.alignment)?;
        // Enforce the batch-size limit *before* the merge path: the merge branch
        // used to return early, so a small request that was still over
        // `max_batch_size` (or a pool whose limits the merge path never checked)
        // bypassed the large-request fallback policy entirely, and the two paths
        // disagreed about what is allowed (N-S-59).
        if aligned_size > self.config.max_batch_size {
            // Try fallback pool for large allocations
            return self.allocate_fallback(size);
        }
        // Check if we should merge small uploads
        if self.config.merge_uploads && size < self.config.small_upload_threshold {
            // Try to merge with existing allocation
            if let Some(allocation) = self.try_merge_allocate(size) {
                return Some(allocation);
            }
        }
        let slot = self.slots.get(self.current_slot)?;
        let new_used = self.total_used.checked_add(aligned_size)?;
        if new_used > slot.size {
            // Pool exhausted, try fallback
            return self.allocate_fallback(size);
        }
        let offset = slot.offset + self.total_used;
        self.total_used = new_used;
        self.total_allocated = self.total_allocated.saturating_add(aligned_size);
        Some(GpuBufferAllocation {
            slot_index: self.current_slot,
            offset,
            size: aligned_size,
            frame_index: self.current_frame,
            is_fallback: false,
            storage: None,
            fallback_id: 0,
        })
    }
    /// Allocates from fallback pool
    fn allocate_fallback(&mut self, size: usize) -> Option<GpuBufferAllocation> {
        if let Some(ref mut pool) = self.fallback_pool {
            let buffer = pool.acquire_sized(size);
            let size = buffer.len();
            let fallback_id = self.next_fallback_id;
            self.next_fallback_id = self.next_fallback_id.saturating_add(1);
            // The handle owns the real storage so it stays alive (and can be
            // released) after this call returns; `fallback_id` distinguishes
            // several simultaneously-live fallback handles.
            Some(GpuBufferAllocation {
                slot_index: usize::MAX, // Marker for fallback
                offset: 0,
                size,
                frame_index: self.current_frame,
                is_fallback: true,
                storage: Some(buffer),
                fallback_id,
            })
        } else {
            None
        }
    }
    /// Tries to merge a small allocation with existing data in the current slot
    fn try_merge_allocate(&mut self, size: usize) -> Option<GpuBufferAllocation> {
        let aligned_size = align_up_checked(size, self.config.alignment)?;
        // The merge path must honour the same batch-size limit as the normal
        // path; it is called after the check in `allocate`, but keeping the guard
        // here too means the helper is safe on its own and cannot drift (N-S-59).
        if aligned_size > self.config.max_batch_size {
            return None;
        }
        // Get current slot
        let slot = self.slots.get(self.current_slot)?;
        // Check if we can merge with existing allocation
        if slot.in_use {
            // Slot is already in use, cannot merge
            return None;
        }
        // Check if there's enough remaining space in the current slot
        let remaining_space = slot.size.saturating_sub(self.total_used);
        if remaining_space < aligned_size {
            return None;
        }
        // Merge with existing allocation. The offset is slot-relative here too,
        // so it uses the same `slot.offset + used` coordinate space as
        // [`Self::allocate`] — a merge must not report a different origin.
        let offset = slot.offset + self.total_used;
        self.total_used = self.total_used.checked_add(aligned_size)?;
        self.total_allocated = self.total_allocated.saturating_add(aligned_size);
        Some(GpuBufferAllocation {
            slot_index: slot.index,
            offset,
            size: aligned_size,
            frame_index: self.current_frame,
            is_fallback: false,
            storage: None,
            fallback_id: 0,
        })
    }
    /// Returns the storage of a fallback allocation to the pool for reuse.
    ///
    /// Ring-slot allocations own no storage (they borrow space inside the pool's
    /// slots and are recycled with [`Self::wait_for_slot`]), so releasing one is
    /// a no-op. Releasing an oversized fallback whose capacity no longer matches
    /// the pool simply drops it, which is the pool's own recycling contract.
    pub fn release(&mut self, mut allocation: GpuBufferAllocation) {
        if !allocation.is_fallback {
            return;
        }
        if let (Some(ref mut pool), Some(buffer)) =
            (&mut self.fallback_pool, allocation.storage.take())
        {
            pool.release(buffer);
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
            fallback_used: self.fallback_pool.as_ref().map(|p| p.available()).unwrap_or(0),
        }
    }
    /// Recycles the slot at `slot_index`, marking it available for reuse.
    ///
    /// This is a CPU-side staging pool: it has no GPU fence primitive to block
    /// on, so "waiting for a slot" resolves to *releasing* it — a slot that was
    /// marked in-use by a previous frame is returned to the free pool. The call
    /// is a no-op for a slot that is already free or does not exist, which keeps
    /// callers that recycle eagerly from disturbing a still-active slot elsewhere.
    pub fn wait_for_slot(&mut self, slot_index: usize) {
        if let Some(slot) = self.slots.get_mut(slot_index) {
            slot.in_use = false;
        }
    }
}
/// GPU buffer allocation info
///
/// A handle either borrows a region of one of the pool's ring slots (no owned
/// storage) or — for a fallback allocation — owns a `Vec<u8>` so the bytes
/// outlive the `allocate` call and can be returned with
/// [`GpuStagingBufferPool::release`].
#[derive(Debug)]
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
    /// The owned backing store of a fallback allocation; `None` for ring-slot
    /// allocations, which borrow space inside the pool's own slots.
    pub storage: Option<Vec<u8>>,
    /// Monotonic id assigned to fallback allocations (zero for ring-slot
    /// allocations) so two simultaneously-live fallbacks are distinguishable.
    pub fallback_id: u64,
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
        Self { config, pending_uploads: Vec::new(), current_batch_size: 0 }
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
        self.pending_uploads.push(GpuPendingUpload { data, destination_offset });
        self.current_batch_size += size;
        true
    }
    /// Tries to merge with existing pending uploads
    fn try_merge(&mut self, data: &[u8], offset: usize) -> Option<usize> {
        let new_end = offset.saturating_add(data.len());
        let candidate = self.pending_uploads.iter().position(|upload| {
            upload.destination_offset.saturating_add(upload.data.len()) == offset
        })?;
        // Folding this write into an earlier upload is only valid when no other
        // pending upload overlaps the range it would occupy. Otherwise applying
        // the earlier (merged) write would reorder it ahead of a later write
        // that covers the same bytes.
        let overlaps = self.pending_uploads.iter().enumerate().any(|(i, other)| {
            if i == candidate {
                return false;
            }
            let other_end = other.destination_offset.saturating_add(other.data.len());
            other.destination_offset < new_end && offset < other_end
        });
        if overlaps {
            return None;
        }
        self.pending_uploads[candidate].data.extend_from_slice(data);
        Some(data.len())
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
        let uploads =
            self.pending_uploads.drain(..).map(|u| (u.data, u.destination_offset)).collect();
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
    ///
    /// A history of 0 is clamped to 1: `record` evicts the oldest entry when the
    /// history is full, and `remove(0)` on an empty `Vec` panics. A zero-sized
    /// history would make the very first `record` call do exactly that (N-S-32).
    pub fn new(max_history: usize) -> Self {
        let max_history = max_history.max(1);
        Self { stats_history: Vec::with_capacity(max_history), max_history }
    }
    /// Records a stats sample
    pub fn record(&mut self, stats: GpuBufferPoolStats) {
        // Evict from the front only when there is something to evict; `new`
        // guarantees a non-zero capacity, but the guard makes the invariant local
        // to `record` as well and removes any dependence on construction order.
        while self.stats_history.len() >= self.max_history && !self.stats_history.is_empty() {
            self.stats_history.remove(0);
        }
        self.stats_history.push(stats);
    }
    /// Returns the average utilization
    pub fn average_utilization(&self) -> f32 {
        if self.stats_history.is_empty() {
            return 0.0;
        }
        let total: f32 =
            self.stats_history.iter().map(|s| s.used_size as f32 / s.total_size as f32).sum();
        total / self.stats_history.len() as f32
    }
    /// Returns true if the pool is under memory pressure
    pub fn is_under_pressure(&self) -> bool {
        if self.stats_history.len() < 3 {
            return false;
        }
        // Check if recent utilization is consistently high
        let recent: Vec<_> = self.stats_history.iter().rev().take(3).collect();
        recent.iter().all(|s| s.used_size as f32 / s.total_size as f32 > 0.8)
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
    ///
    /// # Why there is no byte size here
    ///
    /// `PoolConfig` carries the pool's **slot counts** (`initial_size`, `max_size`); the
    /// per-buffer byte size is a property of the pool the config builds, not of the config. This
    /// function used to compute a per-profile byte size into a `_buffer_size` binding and then
    /// discard it — the same value [`create_fallback_pool`] computes for real. Removed rather than
    /// wired, because `PoolConfig` has no field to receive it; the byte size lives where it is used.
    pub fn create_gpu_buffer_pool_config(profile: GpuMemoryProfile) -> PoolConfig {
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
            GpuMemoryProfile::Integrated => 1024 * 1024,
            GpuMemoryProfile::Cpu => 256 * 1024,
        };
        BufferPool::new(buffer_size, profile.ring_buffer_slots(), profile.ring_buffer_slots() * 2)
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
        let mut pool = GpuStagingBufferPool::new(config).unwrap();
        let allocation = pool.allocate(1024).unwrap();
        assert_eq!(allocation.slot_index, 0);
        assert_eq!(allocation.offset, 0);
        assert!(allocation.size >= 1024);
        assert!(!allocation.is_fallback);
    }
    #[test]
    fn test_buffer_pool_ring_rotation() {
        let config = StagingBufferPoolConfig::discrete();
        let mut pool = GpuStagingBufferPool::new(config).unwrap();
        pool.next_frame();
        assert_eq!(pool.current_frame(), 1);
        pool.next_frame();
        assert_eq!(pool.current_frame(), 2);
    }
    #[test]
    fn test_wait_for_slot_recycles_a_busy_slot() {
        let config = StagingBufferPoolConfig::discrete();
        let mut pool = GpuStagingBufferPool::new(config).unwrap();
        // Advance one frame: slot 0 is marked in-use, slot 1 becomes current.
        pool.next_frame();
        assert!(pool.slots[0].in_use);

        pool.wait_for_slot(0);
        assert!(!pool.slots[0].in_use);
    }
    #[test]
    fn test_wait_for_slot_is_noop_for_free_or_missing_slots() {
        let config = StagingBufferPoolConfig::discrete();
        let mut pool = GpuStagingBufferPool::new(config).unwrap();
        // Freshly constructed, every slot is free.
        pool.wait_for_slot(0);
        assert!(!pool.slots[0].in_use);
        // An out-of-range index must not panic and must not mutate anything.
        pool.wait_for_slot(usize::MAX);
        assert!(!pool.slots[0].in_use);
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
        assert_eq!(GpuMemoryProfile::from_device_type(GpuDeviceType::Cpu), GpuMemoryProfile::Cpu);
    }
    #[test]
    fn test_integration_config() {
        let config = integration::create_gpu_buffer_pool_config(GpuMemoryProfile::Discrete);
        assert_eq!(config.initial_size, 3);
        assert_eq!(config.max_size, 6);
    }

    /// S-10: every slot starts on an `alignment` boundary, and merge/normal
    /// allocations report the same `slot.offset + used` coordinate space.
    #[test]
    fn test_slot_offsets_are_aligned_and_merge_matches_normal() {
        // `pool_size / ring_slots` is *not* a multiple of alignment here, so the
        // align-down in the constructor is what makes the slot bases land on
        // `alignment` boundaries (0, 256, 512) instead of 0, 341, 682.
        let base = || {
            let mut config = StagingBufferPoolConfig::discrete();
            config.pool_size = 1024;
            config.ring_slots = 3;
            config.alignment = 256;
            config.max_batch_size = 1024;
            config.small_upload_threshold = 1024;
            config.merge_uploads = true;
            config
        };
        let mut pool = GpuStagingBufferPool::new(base()).unwrap();
        assert_eq!(pool.slots[0].offset, 0);
        assert_eq!(pool.slots[1].offset, 256);
        assert_eq!(pool.slots[2].offset, 512);
        for slot in &pool.slots {
            assert_eq!(slot.offset % 256, 0);
        }

        // Small allocations go through the merge path and still report a
        // slot-relative (aligned) origin.
        let a = pool.allocate(64).unwrap();
        assert_eq!((a.slot_index, a.offset), (0, 0));
        pool.next_frame();
        let b = pool.allocate(64).unwrap();
        assert_eq!((b.slot_index, b.offset), (1, 256));
        assert_eq!(b.offset % 256, 0);

        // With merge off the normal path must report the same origin.
        let mut config = base();
        config.merge_uploads = false;
        let mut pool = GpuStagingBufferPool::new(config).unwrap();
        let a = pool.allocate(64).unwrap();
        assert_eq!((a.slot_index, a.offset), (0, 0));
        pool.next_frame();
        let b = pool.allocate(64).unwrap();
        assert_eq!((b.slot_index, b.offset), (1, 256));
        assert_eq!(b.offset % 256, 0);
    }

    /// S-11: invalid configuration is rejected explicitly, not by a panic.
    #[test]
    fn test_invalid_config_is_rejected() {
        let mut config = StagingBufferPoolConfig::discrete();
        config.ring_slots = 0;
        assert!(matches!(GpuStagingBufferPool::new(config), Err(GpuBufferPoolError::NoRingSlots)));

        let mut config = StagingBufferPoolConfig::discrete();
        config.alignment = 0;
        assert!(matches!(
            GpuStagingBufferPool::new(config),
            Err(GpuBufferPoolError::InvalidAlignment)
        ));

        let mut config = StagingBufferPoolConfig::discrete();
        config.alignment = 3; // not a power of two
        assert!(matches!(
            GpuStagingBufferPool::new(config),
            Err(GpuBufferPoolError::InvalidAlignment)
        ));
    }

    /// S-11: a `usize::MAX` request is rejected without corrupting pool state,
    /// in both debug and release builds.
    #[test]
    fn test_huge_request_is_rejected_without_corrupting_state() {
        let mut pool = GpuStagingBufferPool::new(StagingBufferPoolConfig::discrete()).unwrap();
        let before = pool.memory_stats();
        assert!(pool.allocate(usize::MAX).is_none());
        let after = pool.memory_stats();
        assert_eq!(after.used_size, before.used_size);
        assert_eq!(after.allocated_size, before.allocated_size);
        assert_eq!(after.current_slot, before.current_slot);
        // The pool is still usable afterwards.
        assert!(pool.allocate(1024).is_some());
    }

    /// S-12: a fallback handle owns its storage and releases it back to the
    /// pool for reuse.
    #[test]
    fn test_fallback_owns_storage_and_releases_back() {
        let mut config = StagingBufferPoolConfig::discrete();
        config.pool_size = 512;
        config.ring_slots = 1;
        config.max_batch_size = 1024;
        config.merge_uploads = false;
        config.alignment = 256;
        let mut pool = GpuStagingBufferPool::new(config)
            .unwrap()
            .with_fallback_pool(crate::memory::BufferPool::new(512, 1, 2));

        assert_eq!(pool.memory_stats().fallback_used, 1);
        let _ = pool.allocate(256).unwrap(); // normal, total_used 256
        let _ = pool.allocate(256).unwrap(); // normal, total_used 512 (slot full)
        let a = pool.allocate(256).expect("slot exhausted, must fall back");
        assert!(a.is_fallback);
        assert_eq!(a.storage.as_ref().map(|s| s.len()), Some(256));
        assert_eq!(pool.memory_stats().fallback_used, 0);

        pool.release(a);
        assert_eq!(pool.memory_stats().fallback_used, 1);
    }

    /// S-12: two fallback handles alive at once are distinguishable.
    #[test]
    fn test_simultaneous_fallbacks_are_distinguishable() {
        let mut config = StagingBufferPoolConfig::discrete();
        config.pool_size = 512;
        config.ring_slots = 1;
        config.max_batch_size = 1024;
        config.merge_uploads = false;
        config.alignment = 256;
        let mut pool = GpuStagingBufferPool::new(config)
            .unwrap()
            .with_fallback_pool(crate::memory::BufferPool::new(512, 1, 4));

        let _ = pool.allocate(256).unwrap();
        let _ = pool.allocate(256).unwrap();
        let a = pool.allocate(256).expect("first fallback");
        let b = pool.allocate(256).expect("second fallback");
        assert!(a.is_fallback && b.is_fallback);
        assert_ne!(a.fallback_id, b.fallback_id);
        assert!(a.storage.is_some() && b.storage.is_some());
    }

    /// S-13: an overlapping trailing write must not be folded into an earlier
    /// upload across an intermediate overlap, which would reorder writes.
    #[test]
    fn test_upload_batcher_preserves_write_order_on_overlap() {
        let config = StagingBufferPoolConfig::discrete();
        let mut batcher = GpuUploadBatcher::new(config);
        assert!(batcher.add_upload(vec![1, 1, 1], 0));
        assert!(batcher.add_upload(vec![2, 2], 2));
        assert!(batcher.add_upload(vec![3], 3));
        let uploads = batcher.flush();
        assert_eq!(
            uploads,
            vec![(vec![1, 1, 1], 0), (vec![2, 2], 2), (vec![3], 3)],
            "the trailing write must not be folded into the earlier upload across an overlap",
        );
    }

    /// N-S-59: a small request that exceeds `max_batch_size` must take the
    /// fallback path even when merging is enabled — the merge branch used to
    /// short-circuit the limit check.
    #[test]
    fn test_small_over_batch_request_takes_fallback_even_with_merge() {
        let mut config = StagingBufferPoolConfig::discrete();
        config.pool_size = 64 * 1024;
        config.ring_slots = 1;
        config.merge_uploads = true;
        config.small_upload_threshold = 4096; // merge applies below this
        config.max_batch_size = 256; // ... but a 272-byte request is over the batch limit
        config.alignment = 256;
        let mut pool = GpuStagingBufferPool::new(config)
            .unwrap()
            .with_fallback_pool(crate::memory::BufferPool::new(512, 1, 4));

        // 272 bytes < small_upload_threshold, so it *would* have gone through the
        // merge path; it is over max_batch_size, so it must fall back instead.
        let alloc = pool.allocate(272).expect("falls back to the fallback pool");
        assert!(alloc.is_fallback, "an over-batch small request must not merge");
        // A request within the batch limit still uses the ring slot.
        let normal = pool.allocate(128).expect("normal allocation");
        assert!(!normal.is_fallback);
    }

    /// N-S-59 / N-S-32: a monitor created with zero history must not panic on its
    /// first `record` (the old `remove(0)` on an empty vec did), and must retain
    /// exactly one sample.
    #[test]
    fn test_buffer_pool_monitor_zero_history_does_not_panic() {
        let mut monitor = GpuBufferPoolMonitor::new(0);
        let stats = GpuBufferPoolStats {
            total_size: 1024,
            used_size: 512,
            allocated_size: 512,
            slot_count: 1,
            current_slot: 0,
            current_frame: 0,
            fallback_used: 0,
        };
        monitor.record(stats);
        assert_eq!(monitor.average_utilization(), 0.5);
        // Recording again keeps the single slot and does not panic.
        monitor.record(GpuBufferPoolStats { used_size: 1024, ..stats });
        assert_eq!(monitor.average_utilization(), 1.0);
    }
}
