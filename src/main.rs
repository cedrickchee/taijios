#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(custom_test_frameworks)] // Rust supports replacing the default test framework through the unstable custom_test_frameworks feature
#![test_runner(tiny_os::test_runner)]
#![reexport_test_harness_main = "test_main"] // set the name of the test framework entry function to test_main

extern crate alloc;

use core::panic::PanicInfo;
use bootloader::{ BootInfo, entry_point };
use alloc::boxed::Box;
use tiny_os::{println, print};

// To make sure that the entry point function has always the correct signature
// that the bootloader expects, the `bootloader` crate provides an `entry_point`
// macro that provides a type-checked way to define a Rust function as the entry
// point.
entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use x86_64::{
        // structures::paging::Page,
        VirtAddr,
    }; // need to import the `Translate` trait in order to use the `translate_addr` method it provides.
    use tiny_os::memory::{ self, BootInfoFrameAllocator };
    use tiny_os::allocator;
    
    // Write some characters to the screen.
    print!("H");
    print!("ello ");
    println!("Wörld!"); // test the handling of unprintable characters.
    println!("The numbers are {} and {}", 42, 1.0/3.0);

    tiny_os::init();

    // Use the allocation and collection types of `alloc`.
    // For example we can use a `Box` to allocate a value on the heap.
    
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");
        // in case the fn returns an error, we panic using the `expect` method
        // since there is currently no sensible way for us to handle this error.

    let x = Box::new(42);
    // When we run the above code, we see that our `alloc_error_handler`
    // function is called because the `Box::new` function implicitly calls the
    // `alloc` function of the global allocator. Our dummy allocator always
    // returns a null pointer, so every allocation fails. To fix this we need to
    // create an allocator that actually returns usable memory.

    /* Uncomment lines below to access the page tables.
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    
    // Test memory translation by translating some addresses using
    // `OffsetPageTable` type from the `x86_64` crate.

    // Initialize a Mapper.
    let mapper = unsafe { memory::init(phys_mem_offset) };

    let addresses = [
        // The identity-mapped vga buffer page.
        0xb8000,
        // Some code page.
        0x201008,
        // Some stack page.
        0x0100_0020_1a10,
        // Virtual address mapped to physical address 0.
        boot_info.physical_memory_offset
    ];

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        // Use the `Translate::translate_addr` method (from the `x86_64` crate)
        // instead of our own `memory::translate_addr` function.
        let phys = mapper.translate_addr(virt);

        // Old code: Uncomment line below to use our memory translation function.
        //let phys = unsafe { translate_addr(virt, phys_mem_offset) };
        println!("{:?} -> {:?}", virt, phys);
        // As expected, the identity-mapped address `0xb8000` translates to the
        // same physical address. The code page and the stack page translate to
        // some arbitrary physical addresses, which depend on how the bootloader
        // created the initial mapping for our kernel. It’s worth noting that
        // the last 12 bits always stay the same after translation, which makes
        // sense because these bits are the _page offset_ and not part of the
        // translation.
        //
        // Since each physical address can be accessed by adding the
        // `physical_memory_offset`, the translation of the
        // `physical_memory_offset` address itself should point to physical
        // address `0`. However, the translation fails because the mapping uses
        // huge pages for efficiency, which is not supported in our
        // implementation yet.
        // (update: huge page translation now also works.)
    }
    */

    /* Uncomment lines below to allocate frames and create new page mapping.

    // Create a new mapping for a previously unmapped page.
    // Until now we only looked at the page tables without modifying anything.
    
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    // With the `BootInfoFrameAllocator`, behind the scenes, the `map_to` method
    // creates the missing page tables.
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };
    // Map an unused page.
    // This maps the page to the VGA text buffer frame, so we should see any
    // write to it on the screen.
    let page = Page::containing_address(VirtAddr::new(0));
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);
    // Convert the page to a raw pointer.
    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    // Write the string `New!` to the screen through the new mapping.
    unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e) };
    // Note: We don’t write to the start of the page because the top line of the
    // VGA buffer is directly shifted off the screen by the next `println`. We
    // write the value `0x_f021_f077_f065_f04e`, which represents the string
    // “New!” on white background.
    
    */

    // Uncomment lines below to print the entries of the level 4 page table.    
    // use tiny_os::memory::active_level_4_table;
    // use x86_64::structures::paging::PageTable;
    // let l4_table = unsafe { active_level_4_table(phys_mem_offset) };

    // for (i, entry) in l4_table.iter().enumerate() {
    //     // Print non-empty entries because all 512 entries wouldn’t fit on the screen.
    //     if !entry.is_unused() {
    //         // We see that there are various non-empty entries, which all map to
    //         // different level 3 tables. There are so many regions because
    //         // kernel code, kernel stack, the physical memory mapping, and the
    //         // boot information all use separate memory areas.
    //         println!("L4 Entry {}: {:?}", i, entry);

    //         // To traverse the page tables further and take a look at a level 3
    //         // table, we can take the mapped frame of an entry and convert it to
    //         // a virtual address again.

    //         // Get the physical address from the entry and convert it.
    //         let phys = entry.frame().unwrap().start_address();
    //         let virt = phys.as_u64() + boot_info.physical_memory_offset;
    //         let ptr = VirtAddr::new(virt).as_mut_ptr();
    //         let l3_table: &PageTable = unsafe { &*ptr };

    //         // Print non-empty entries of the level 3 table.
    //         for (i, entry) in l3_table.iter().enumerate() {
    //             if !entry.is_unused() {
    //                 println!("  L3 Entry {}: {:?}", i, entry);
    //             }
    //         }
    //     }
    // }

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
// # Boot information
//
// The `bootloader` crate defines a `BootInfo` struct that contains all the
// information it passes to our kernel. With the `map_physical_memory` feature
// enabled, it currently has the two fields `memory_map` and
// `physical_memory_offset`:
//
// - The `memory_map` field contains an overview of the available physical
//   memory. This tells our kernel how much physical memory is available in the
//   system and which memory regions are reserved for devices such as the VGA
//   hardware. The memory map can be queried from the BIOS or UEFI firmware, but
//   only very early in the boot process. For this reason, it must be provided
//   by the bootloader because there is no way for the kernel to retrieve it
//   later.
// - The `physical_memory_offset` tells us the virtual start address of the
//   physical memory mapping. By adding this offset to a physical address, we
//   get the corresponding virtual address. This allows us to access arbitrary
//   physical memory from our kernel.
//
// # The `entry_point` macro
//
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
//
// # Using `OffsetPageTable`
// 
// Translating virtual to physical addresses is a common task in an OS kernel,
// therefore the `x86_64` crate provides an abstraction for it. The
// implementation already supports huge pages and several other page table
// functions apart from `translate_addr`, so we will use it in the following
// instead of adding huge page support to our own implementation.
//
// The `OffsetPageTable` type assumes that the complete physical memory is
// mapped to the virtual address space at some offset.
//
// In our case, the bootloader maps the complete physical memory at a virtual
// address specified by the `physical_memory_offset` variable, so we can use the
// `OffsetPageTable` type.
//
// # Creating a new mapping
//
// We will create a new mapping for a previously unmapped page.
//
// We will use the `map_to` function of the `Mapper` trait for our
// implementation. The frame allocator is needed because mapping the given page
// might require creating additional page tables, which need unused frames as
// backing storage.
//
// ## Choosing a virtual page
// 
// The difficulty of creating a new mapping depends on the virtual page that we
// want to map. In the easiest case, the level 1 page table for the page already
// exists and we just need to write a single entry. In the most difficult case,
// the page is in a memory region for that no level 3 exists yet so that we need
// to create new level 3, level 2 and level 1 page tables first.
//
// For calling our `create_example_mapping` function with the
// `EmptyFrameAllocator`, we need to choose a page for that all page tables
// already exist. To find such a page, we can utilize the fact that the
// bootloader loads itself in the first megabyte of the virtual address space.
// This means that a valid level 1 table exists for all pages this region. Thus,
// we can choose any unused page in this memory region for our example mapping,
// such as the page at address `0`. Normally, this page should stay unused to
// guarantee that dereferencing a null pointer causes a page fault, so we know
// that the bootloader leaves it unmapped.
