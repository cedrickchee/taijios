#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(custom_test_frameworks)] // Rust supports replacing the default test framework through the unstable custom_test_frameworks feature
#![test_runner(tiny_os::test_runner)]
#![reexport_test_harness_main = "test_main"] // set the name of the test framework entry function to test_main

use core::panic::PanicInfo;
use bootloader::{ BootInfo, entry_point };
use tiny_os::{println, print};

// To make sure that the entry point function has always the correct signature
// that the bootloader expects, the `bootloader` crate provides an `entry_point`
// macro that provides a type-checked way to define a Rust function as the entry
// point.
entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use tiny_os::memory::active_level_4_table;
    use x86_64::VirtAddr;
    use x86_64::structures::paging::PageTable;
    
    // Write some characters to the screen.
    print!("H");
    print!("ello ");
    println!("Wörld!"); // test the handling of unprintable characters.
    println!("The numbers are {} and {}", 42, 1.0/3.0);

    tiny_os::init();

    // Access the page tables.
    // Print the entries of the level 4 page table.
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let l4_table = unsafe { active_level_4_table(phys_mem_offset) };

    for (i, entry) in l4_table.iter().enumerate() {
        // Print non-empty entries because all 512 entries wouldn’t fit on the screen.
        if !entry.is_unused() {
            // We see that there are various non-empty entries, which all map to
            // different level 3 tables. There are so many regions because
            // kernel code, kernel stack, the physical memory mapping, and the
            // boot information all use separate memory areas.
            println!("L4 Entry {}: {:?}", i, entry);

            // To traverse the page tables further and take a look at a level 3
            // table, we can take the mapped frame of an entry and convert it to
            // a virtual address again.

            // Get the physical address from the entry and convert it.
            let phys = entry.frame().unwrap().start_address();
            let virt = phys.as_u64() + boot_info.physical_memory_offset;
            let ptr = VirtAddr::new(virt).as_mut_ptr();
            let l3_table: &PageTable = unsafe { &*ptr };

            // Print non-empty entries of the level 3 table.
            for (i, entry) in l3_table.iter().enumerate() {
                if !entry.is_unused() {
                    println!("  L3 Entry {}: {:?}", i, entry);
                }
            }
        }
    }

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

// ********** Sidenote **********
// 
// # The `entry_point` macro

// Since our `_start` function is called externally from the bootloader, no
// checking of our function signature occurs. This means that we could let it
// take arbitrary arguments without any compilation errors, but it would fail or
// cause undefined behavior at runtime.
//
// To make sure that the entry point function has always the correct signature
// that the bootloader expects, the `bootloader` crate provides an `entry_point`
// macro that provides a type-checked way to define a Rust function as the entry
// point. We rewrite our entry point function to use this macro.
//
// We no longer need to use `extern "C"` or `no_mangle` for our entry point, as
// the macro defines the real lower level `_start` entry point for us. The
// `kernel_main` function is now a completely normal Rust function, so we can
// choose an arbitrary name for it. The important thing is that it is
// type-checked so that a compilation error occurs when we use a wrong function
// signature.
