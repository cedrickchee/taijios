//! # Memory allocator module
//! 
//! This module adds support for heap allocation to our kernel.
//! 
//! It provides a simple dummy allocator.
//! 
//! It implements the basic allocation interface of Rust and creates a heap
//! memory region.

use alloc::alloc::{ GlobalAlloc, Layout };
use core::ptr::null_mut;
use x86_64::{
    structures::paging::{
        Mapper, Size4KiB, FrameAllocator, Page, PageTableFlags,
        mapper::MapToError,
    },
    VirtAddr,
};
use linked_list_allocator::LockedHeap;

// We can choose any virtual address range that we like, as long as it is not
// already used for a different memory region.
pub const HEAP_START: usize = 0x_4444_4444_0000;
// If we need more space in the future, we can simply increase it.
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

// The attribute tells the Rust compiler which allocator instance it should use
// as the global heap allocator.
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty(); // create a static allocator

/// Creates a heap memory region from which the allocator can allocate memory.
///
/// We define a virtual memory range for the heap region and then map this
/// region to physical frames.
/// 
/// Maps the heap pages using the Mapper API implementation
/// (`structures::paging::OffsetPageTable`) in the `memory` module.
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    // Creating the page range.
    // 
    // To create a range of the pages that we want to map, we convert the
    // HEAP_START pointer to a VirtAddr type. Then we calculate the heap end
    // address from it by adding the HEAP_SIZE. We want an inclusive bound (the
    // address of the last byte of the heap), so we subtract 1. Next, we convert
    // the addresses into Page types using the containing_address function.
    // Finally, we create a page range from the start and end pages using the
    // Page::range_inclusive function.
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // Mapping the pages.
    //
    // For each page, we do the following:
    //
    // - We allocate a physical frame that the page should be mapped to using
    //   the FrameAllocator::allocate_frame method. This method returns None
    //   when there are no more frames left. We deal with that case by mapping
    //   it to a MapToError::FrameAllocationFailed error through the
    //   Option::ok_or method and then apply the question mark operator to
    //   return early in the case of an error.
    // - We set the required PRESENT flag and the WRITABLE flag for the page.
    //   With these flags both read and write accesses are allowed, which makes
    //   sense for heap memory.
    // - We use the Mapper::map_to method for creating the mapping in the active
    //   page table. The method can fail, therefore we use the question mark
    //   operator again to forward the error to the caller. On success, the
    //   method returns a MapperFlush instance that we can use to update the
    //   translation lookaside buffer using the flush method.
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush()
        }
    }

    // Initialize the allocator after creating the heap.
    unsafe {
        // We use the `lock` method on the inner spinlock of the `LockedHeap`
        // type to get an exclusive reference to the wrapped
        // [`Heap`](https://docs.rs/linked_list_allocator/0.9.0/linked_list_allocator/struct.Heap.html)
        // instance, on which we then call the `init` method with the heap bounds
        // as arguments.
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
        // ********** Sidenote **********
        // It is important that we initialize the heap _after_ mapping the heap
        // pages, since the `init` function already tries to write to the heap
        // memory.
    }

    Ok(())
}

/// Dummy allocator
///
/// It does the absolute minimum to implement the `GlobalAlloc` trait and always
/// return an error when `alloc` is called.
pub struct Dummy; // the struct does not need any fields, so we create it as a zero sized type

unsafe impl GlobalAlloc for Dummy {
    // This method takes a `Layout` instance as argument, which describes the
    // desired size and alignment that the allocated memory should have. It
    // returns a raw pointer to the first byte of the allocated memory block.
    // Instead of an explicit error value, the method returns a null pointer to
    // signal an allocation error. This is a bit non-idiomatic, but it has the
    // advantage that wrapping existing system allocators is easy, since they
    // use the same convention.
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }

    // This method is the counterpart and responsible for freeing a memory block
    // again.
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Since the allocator never returns any memory, a call to `dealloc`
        // should never occur. For this reason we simply panic here.
        panic!("dealloc should be never called")
    }
}

// ********** Sidenote **********
//
// # The `GlobalAlloc` Trait
//
// The trait defines the functions that a heap allocator must provide. The trait
// is special because it is almost never used directly by the programmer.
// Instead, the compiler will automatically insert the appropriate calls to the
// trait methods when using the allocation and collection types of `alloc`.
//
// # Using an allocator crate
//
// Since implementing an allocator is somewhat complex, we start by using an
// external allocator crate. We will implement our own allocator later.
//
// A simple allocator crate for `no_std` applications is the
// [linked_list_allocator](https://github.com/phil-opp/linked-list-allocator/)
// crate. It’s name comes from the fact that it uses a linked list data
// structure to keep track of deallocated memory regions.
//
// `use linked_list_allocator::LockedHeap;` The struct is named `LockedHeap`
// because it uses the `spinning_top::Spinlock` type for synchronization.
//
// Setting the `LockedHeap` as global allocator is not enough. The reason is
// that we use the `empty` constructor function, which creates an allocator
// without any backing memory. Like our dummy allocator, it always returns an
// error on `alloc`. To fix this, we need to initialize the allocator after
// creating the heap.
