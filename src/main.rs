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

    // Access the page tables.
    use x86_64::registers::control::Cr3;
    let (level_4_page_table, _) = Cr3::read(); // returns the currently active level 4 page table from the CR3 register.
    println!("Level 4 page table at: {:?}", level_4_page_table.start_address()); // print "Level 4 page table at: PhysAddr(0x1000)"

    // Uncomment lines below to trigger a stack overflow.
    // fn stack_overflow() {
    //     stack_overflow(); // for each recursion, the return address is pushed
    // }
    // stack_overflow();

    // Uncomment lines below to try to cause a page fault by accessing some
    // memory outside of our kernel.
    // let ptr = 0xdeadbeef as *mut u32; // 0x206cd0
    // unsafe { *ptr = 42; }

    // Uncomment lines below to try to read from a code page.

    // We see that the current instruction pointer is `0x206cd0`, so we know
    // that this address points to a code page. Code pages are mapped read-only
    // by the bootloader, so reading from this address works but writing causes
    // a page fault.
    // 
    // Note: The actual address might be different for you. Use the address that
    // your page fault handler reports.
    // let ptr = 0x206cd0 as *mut u32;
    // read from a code page
    // unsafe { let x = *ptr; }
    // println!("read worked");
    // write to a code page
    // unsafe { *ptr = 42; }
    // println!("write worked");

    // Call the renamed test framework entry function.
    #[cfg(test)] // use conditional compilation to add the call to `test_main` only in test contexts.
    test_main();

    println!("It did not crash!");
    tiny_os::hlt_loop(); // use this `hlt_loop` instead of the endless loops
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    tiny_os::hlt_loop();
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
