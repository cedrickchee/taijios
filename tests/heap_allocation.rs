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
