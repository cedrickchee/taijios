#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(custom_test_frameworks)] // Rust supports replacing the default test framework through the unstable custom_test_frameworks feature
#![test_runner(tiny_os::test_runner)]
#![reexport_test_harness_main = "test_main"] // set the name of the test framework entry function to test_main

use core::panic::PanicInfo;
use tiny_os::{println, print};

/// This function is the entry point, since the linker looks for a function
/// named `_start` by default.
#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // Write some characters to the screen.
    print!("H");
    print!("ello ");
    println!("Wörld!"); // test the handling of unprintable characters.
    println!("The numbers are {} and {}", 42, 1.0/3.0);

    tiny_os::init();

    // Trigger a double fault.
    unsafe {
        // Write to the invalid address `0xdeadbeef`.
        //
        // The virtual address is not mapped to a physical address in the page
        // tables, so a page fault occurs. We haven’t registered a page fault
        // handler in our IDT, so a double fault occurs.
        *(0xdeadbeef as *mut u64) = 42;
    }

    // Invoke a breakpoint exception.
    x86_64::instructions::interrupts::int3();

    // Call the renamed test framework entry function.
    #[cfg(test)] // use conditional compilation to add the call to `test_main` only in test contexts.
    test_main();

    println!("It did not crash!");
    loop {}
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

/// Panic handler in test mode.
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    tiny_os::test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
