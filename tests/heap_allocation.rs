//! # A Memory Allocation Test
//! 
//! The integration test ensure that we don't accidentally break our new
//! allocation code.

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(tiny_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc; // since we want to test allocations, we enable the `alloc` crate

use bootloader::{ entry_point, BootInfo };
use core::panic::PanicInfo;
use alloc::{ boxed::Box, vec::Vec };
use tiny_os::allocator::HEAP_SIZE;

entry_point!(main);

// The implementation of the `main` function is very similar to the
// `kernel_main` function in `main.rs`.
fn main(boot_info: &'static BootInfo) -> ! {
    use x86_64::VirtAddr;
    use tiny_os::memory::{ self, BootInfoFrameAllocator };
    use tiny_os::allocator;

    tiny_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    test_main();
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    tiny_os::test_panic_handler(info)
}

// A test that performs some simple allocations using `Box` and checks the
// allocated values, to ensure that basic allocations work.
#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}

// Iteratively build a large vector, to test both large allocations and multiple
// allocations (due to reallocations).
#[test_case]
fn large_vec() {
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    // Verify the sum by comparing it with the formula for the n-th partial sum.
    // This gives us some confidence that the allocated values are all correct.
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

// Create ten thousand allocations after each other.
//
// This test ensures that the allocator reuses freed memory for subsequent
// allocations since it would run out of memory otherwise. This might seem like
// an obvious requirement for an allocator, but there are allocator designs that
// don’t do this.
#[test_case]
fn many_boxes() {
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}

// Like the `many_boxes` test, this test creates a large number of allocations
// to provoke an out-of-memory failure if the allocator does not reuse freed
// memory. Additionally, the test creates a `long_lived` allocation, which lives
// for the whole loop execution.
#[test_case]
fn many_boxes_long_lived() {
    let long_lived = Box::new(1); // test the limitation of a bump allocator
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1);
}

// ********** Sidenote **********
//
// # Bump Allocator
//
// ## The Drawback of a Bump Allocator
// 
// The main limitation of a bump allocator is that it can only reuse deallocated
// memory after all allocations have been freed. This means that a single
// long-lived allocation suffices to prevent memory reuse. We can see this when
// we add a variation of the `many_boxes` test (`many_boxes_long_lived`).
//
// When we try run `many_boxes_long_lived` test, we see that it fails.
//
// Why this failure occurs in detail: First, the `long_lived` allocation is
// created at the start of the heap, thereby increasing the `allocations`
// counter by 1. For each iteration of the loop, a short lived allocation is
// created and directly freed again before the next iteration starts. This means
// that the `allocations` counter is temporarily increased to 2 at the beginning
// of an iteration and decreased to 1 at the end of it. The problem now is that
// the bump allocator can only reuse memory when all `allocations` have been
// freed, i.e. the `allocations` counter falls to 0. Since this doesn’t happen
// before the end of the loop, each loop iteration allocates a new region of
// memory, leading to an out-of-memory error after a number of iterations.
//
// # Linked List Allocator
//
// When we run our `heap_allocation` tests again, we see that all tests pass
// now, including the `many_boxes_long_lived` test that failed with the bump
// allocator.
//
// This shows that our linked list allocator is able to reuse freed memory for
// subsequent allocations.
