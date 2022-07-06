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

// The attribute tells the Rust compiler which allocator instance it should use
// as the global heap allocator.
#[global_allocator]
static ALLOCATOR: Dummy = Dummy;

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
