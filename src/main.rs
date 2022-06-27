#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(custom_test_frameworks)] // Rust supports replacing the default test framework through the unstable custom_test_frameworks feature
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"] // set the name of the test framework entry function to test_main

use core::panic::PanicInfo;

mod vga_buffer;

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

/// This function is the entry point, since the linker looks for a function
/// named `_start` by default.
#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // Write some characters to the screen.
    print!("H");
    print!("ello ");
    println!("Wörld!"); // test the handling of unprintable characters.
    println!("The numbers are {} and {}", 42, 1.0/3.0);

    // Call the renamed test framework entry function.
    #[cfg(test)] // use conditional compilation to add the call to `test_main` only in test contexts.
    test_main();

    loop {}
}

/// Runner just prints a short debug message and then calls each test function
/// in the list.
#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    // ********** Sidenote **********
    //
    // syntax: &[&dyn Fn()] is a slice of trait object references of the Fn()
    // trait. It is basically a list of references to types that can be called
    // like a function.

    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}
