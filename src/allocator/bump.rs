//! # Bump allocator module
//! 
//! The most simple allocator design is a bump allocator (also known as stack
//! allocator). It allocates memory linearly and only keeps track of the number
//! of allocated bytes and the number of allocations. It is only useful in very
//! specific use cases because it has a severe limitation: it can only free all
//! memory at once.

use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;
use super::{ Locked, align_up };

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    /// Always point to the first unused byte of the heap, i.e. the start
    /// address of the next allocation. On each allocation, this field will be
    /// increased by the allocation size (“bumped”) to ensure that we don’t
    /// return the same memory region twice.
    next: usize,
    /// A simple counter for the active allocations with the goal of resetting
    /// the allocator after the last allocation was freed.
    allocations: usize,
}

impl BumpAllocator {
    /// Creates a new empty bump allocator.
    /// 
    /// It is important that we declared `new` as const function. If they were
    /// normal functions, a compilation error would occur because the
    /// initialization expression of a `static` (the place where it will be
    /// used) must evaluable at compile time.
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0
        }
    }

    /// Initializes the bump allocator with the given heap bounds.
    ///
    /// This method is unsafe because the caller must ensure that the given
    /// memory range is unused. Also, this method must be called only once.    
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock(); // get a mutable reference
        // The instance remains locked until the end of the method, so that no
        // data race can occur in multithreaded contexts.

        // Alignment and bounds check.
        //
        // The method respects alignment requirements and performs a bounds
        // check to ensure that the allocations stay inside the heap memory
        // region.
        //
        // (OUTDATED): Note that we don’t perform any bounds checks or alignment
        // adjustments, so this implementation is not safe yet.
        
        // Use the `next` field as the start address for our allocation.
        // This step round up the `next` address to the alignment specified by
        // the `Layout` argument.
        let alloc_start = align_up(bump.next, layout.align());
        // Add the requested allocation size to `lloc_start` to get the end
        // address of the allocation.
        //
        // To prevent integer overflow on large allocations, we use the
        // `checked_add` method. If an overflow occurs or if the resulting end
        // address of the allocation is larger than the end address of the heap,
        // we return a null pointer to signal an out-of-memory situation.
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.heap_end {
            ptr::null_mut() // out of memory
        } else {
            // Update the `next` field to point at the end address of the
            // allocation, which is the next unused address on the heap.            
            bump.next = alloc_end;
            // Before returning the start address of the allocation as a `*mut u8`
            // pointer, we increase the `allocations` counter by 1.
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.lock(); // get a mutable reference

        bump.allocations -= 1;
        // If the counter reaches `0` again, it means that all `allocations`
        // were freed again. In this case, it resets the `next` address to the
        // `heap_start` address to make the complete heap memory available
        // again.
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}
