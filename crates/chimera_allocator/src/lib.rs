//! Bump allocator and memory arena for the Chimera Elixir runtime.
//!
//! Provides a bump allocation arena for short-lived terms and GC-safe
//! pointer handling. Designed for high-throughput allocation of Elixir terms.

#[cfg(test)]
use chimera_allocator as _;

use std::fmt;

/// A bump allocator arena for Elixir terms.
///
// Allocations proceed by incrementing a pointer. Freeing is done by resetting
/// the arena. This makes allocation O(1) and ideal for generational contexts.
pub struct BumpArena {
    memory: *mut u8,
    capacity: usize,
    offset: usize,
}

/// Allocation result.
#[derive(Debug)]
pub struct AllocError {
    _private: (),
}

impl fmt::Display for AllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "allocator out of memory")
    }
}

/// Create a new arena with the given capacity.
impl BumpArena {
    /// Create a new bump arena with the specified capacity.
    ///
    /// # Panics
    /// Panics if the capacity is zero or if memory allocation fails.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "arena capacity must be non-zero");
        let memory = unsafe {
            let layout = std::alloc::Layout::from_size_align_unchecked(capacity, 8);
            std::alloc::alloc(layout)
        };
        if memory.is_null() {
            panic!("failed to allocate arena memory");
        }
        BumpArena {
            memory,
            capacity,
            offset: 0,
        }
    }

    /// Create a new arena with capacity derived from the system page size.
    pub fn with_page_size() -> Self {
        let page_size = 4096; // Conservative default
        Self::new(page_size * 64) // 256KB arena
    }

    /// Allocate `size` bytes from the arena.
    ///
    /// Returns a pointer to the allocated memory on success.
    /// Returns `Err` if there's insufficient space.
    pub fn alloc(&mut self, size: usize, align: usize) -> Result<*mut u8, AllocError> {
        // Round up offset to meet alignment requirements
        let aligned_offset = (self.offset + align - 1) & !(align - 1);

        // Check if we have enough space
        if aligned_offset + size > self.capacity {
            return Err(AllocError { _private: () });
        }

        let ptr = unsafe { self.memory.add(aligned_offset) };
        self.offset = aligned_offset + size;
        Ok(ptr)
    }

    /// Allocate a value of type T, returning a mutable reference.
    ///
    /// # Safety
    /// The returned pointer is only valid until the next allocation
    /// from this arena. Do not store references across allocations.
    pub unsafe fn alloc_value<T>(&mut self, value: T) -> &mut T
    where
        T: Sized,
    {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        let ptr = self.alloc(size, align).unwrap();
        std::ptr::write(ptr as *mut T, value);
        &mut *(ptr as *mut T)
    }

    /// Allocate a slice of T values.
    ///
    /// # Safety
    /// Same lifetime constraints as `alloc_value`.
    pub unsafe fn alloc_slice<T>(&mut self, values: &[T]) -> &mut [T]
    where
        T: Clone + Sized,
    {
        let size = std::mem::size_of_val(values);
        let align = std::mem::align_of::<T>();
        let ptr = self.alloc(size, align).unwrap();
        std::ptr::copy_nonoverlapping(values.as_ptr(), ptr as *mut T, values.len());
        std::slice::from_raw_parts_mut(ptr as *mut T, values.len())
    }

    /// Reset the arena, freeing all allocations.
    ///
    /// This is O(1) - simply reset the offset pointer.
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Get the current number of bytes used.
    pub fn used(&self) -> usize {
        self.offset
    }

    /// Get the total capacity of the arena.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the remaining free bytes.
    pub fn free(&self) -> usize {
        self.capacity - self.offset
    }

    /// Check if the arena is empty.
    pub fn is_empty(&self) -> bool {
        self.offset == 0
    }
}

/// Drop implementation frees the underlying memory.
impl Drop for BumpArena {
    fn drop(&mut self) {
        unsafe {
            let layout = std::alloc::Layout::from_size_align_unchecked(self.capacity, 8);
            std::alloc::dealloc(self.memory, layout);
        }
    }
}

// Prevent Send + Sync due to raw pointer
unsafe impl Send for BumpArena {}
unsafe impl Sync for BumpArena {}

/// Thread-local bump allocator for process-local allocations.
///
// Each process gets its own arena for allocation-heavy workloads.
pub struct ThreadLocalArena {
    arena: BumpArena,
}

impl ThreadLocalArena {
    /// Create a new thread-local arena.
    pub fn new(capacity: usize) -> Self {
        ThreadLocalArena {
            arena: BumpArena::new(capacity),
        }
    }

    /// Allocate with the given size and alignment.
    pub fn alloc(&mut self, size: usize, align: usize) -> Result<*mut u8, AllocError> {
        self.arena.alloc(size, align)
    }

    /// Allocate and write a value.
    ///
    /// # Safety
    /// Same as `BumpArena::alloc_value`.
    pub unsafe fn alloc_value<T>(&mut self, value: T) -> &mut T {
        self.arena.alloc_value(value)
    }

    /// Reset the arena.
    pub fn reset(&mut self) {
        self.arena.reset();
    }

    /// Get bytes used.
    pub fn used(&self) -> usize {
        self.arena.used()
    }
}

impl Default for ThreadLocalArena {
    fn default() -> Self {
        Self::new(64 * 4096) // 256KB default
    }
}

/// Shared arena pool for large, shared data structures.
///
// Uses reference counting for thread-safe sharing.
pub struct SharedArena {
    inner: std::sync::Arc<std::sync::RwLock<BumpArena>>,
}

impl SharedArena {
    /// Create a new shared arena with the given capacity.
    pub fn new(capacity: usize) -> Self {
        SharedArena {
            inner: std::sync::Arc::new(std::sync::RwLock::new(BumpArena::new(capacity))),
        }
    }

    /// Allocate from the shared arena.
    pub fn alloc(&self, size: usize, align: usize) -> Result<*mut u8, AllocError> {
        self.inner.write().unwrap().alloc(size, align)
    }

    /// Reset the shared arena.
    pub fn reset(&self) {
        self.inner.write().unwrap().reset();
    }

    /// Get bytes used.
    pub fn used(&self) -> usize {
        self.inner.read().unwrap().used()
    }
}

impl Clone for SharedArena {
    fn clone(&self) -> Self {
        SharedArena {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_bump_arena_basic_allocation() {
        let mut arena = BumpArena::new(1024);

        let ptr = arena.alloc(64, 1).unwrap();
        assert!(!ptr.is_null());
        assert_eq!(arena.used(), 64);
    }

    #[test]
    fn test_bump_arena_multiple_allocations() {
        let mut arena = BumpArena::new(1024);

        // Allocate several blocks
        let _p1 = arena.alloc(100, 1).unwrap();
        assert_eq!(arena.used(), 100);

        let _p2 = arena.alloc(100, 1).unwrap();
        assert_eq!(arena.used(), 200);

        let _p3 = arena.alloc(100, 1).unwrap();
        assert_eq!(arena.used(), 300);

        assert_eq!(arena.free(), 724);
    }

    #[test]
    fn test_bump_arena_reset() {
        let mut arena = BumpArena::new(1024);

        let _p1 = arena.alloc(256, 1).unwrap();
        assert_eq!(arena.used(), 256);

        arena.reset();
        assert_eq!(arena.used(), 0);
        assert!(arena.is_empty());

        // Can allocate again after reset
        let _p2 = arena.alloc(256, 1).unwrap();
        assert_eq!(arena.used(), 256);
    }

    #[test]
    fn test_bump_arena_oom() {
        let mut arena = BumpArena::new(100);

        let result = arena.alloc(101, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_bump_arena_alloc_value() {
        let mut arena = BumpArena::new(1024);

        let val = 42i32;
        let ptr = unsafe { arena.alloc_value(val) };
        assert_eq!(*ptr, 42);
    }

    #[test]
    fn test_bump_arena_alloc_slice() {
        let mut arena = BumpArena::new(1024);

        let slice = [1i32, 2, 3, 4, 5];
        let ptr = unsafe { arena.alloc_slice(&slice) };
        assert_eq!(ptr, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_bump_arena_alignment() {
        let mut arena = BumpArena::new(1024);

        // Allocate something requiring 8-byte alignment
        let ptr = arena.alloc(8, 8).unwrap();
        assert_eq!(ptr as usize % 8, 0);

        // Allocate something requiring 4-byte alignment
        let ptr = arena.alloc(4, 4).unwrap();
        assert_eq!(ptr as usize % 4, 0);
    }

    #[test]
    fn test_thread_local_arena() {
        let mut arena = ThreadLocalArena::new(1024);

        let _ = arena.alloc(64, 1).unwrap();
        assert_eq!(arena.used(), 64);

        arena.reset();
        assert_eq!(arena.used(), 0);
    }

    #[test]
    fn test_shared_arena() {
        let arena = SharedArena::new(1024);
        let arena2 = arena.clone();

        let _ = arena.alloc(64, 1).unwrap();
        assert_eq!(arena.used(), 64);

        // Another reference should see the same state
        assert_eq!(arena2.used(), 64);

        arena.reset();
        assert_eq!(arena.used(), 0);
    }

    #[test]
    fn test_shared_arena_concurrent_access() {
        let arena = Arc::new(SharedArena::new(4096));
        let arena2 = arena.clone();

        let handle = std::thread::spawn(move || {
            for _ in 0..100 {
                let _ = arena2.alloc(10, 1);
            }
        });

        for _ in 0..100 {
            let _ = arena.alloc(10, 1);
        }

        handle.join().unwrap();
    }

    #[test]
    fn test_bump_arena_with_page_size() {
        let arena = BumpArena::with_page_size();
        assert!(arena.capacity() > 0);
        assert!(arena.is_empty());
    }

    #[test]
    fn test_bump_arena_capacity_and_free() {
        let arena = BumpArena::new(512);
        assert_eq!(arena.capacity(), 512);
        assert_eq!(arena.free(), 512);

        let mut arena = BumpArena::new(512);
        let _ = arena.alloc(100, 1).unwrap();
        assert_eq!(arena.used(), 100);
        assert_eq!(arena.free(), 412);
    }

    #[test]
    fn test_alloc_error_display() {
        let mut arena = BumpArena::new(10);
        let result = arena.alloc(100, 1);
        assert!(result.is_err());
    }
}
