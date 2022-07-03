//! # Memory management module
//! 
//! We want to access page tables from our kernel.
//! 
//! This module implements support for paging in our kernel, which makes it
//! possible to access the page tables that our kernel runs on.
//! 
//! It also implements:
//! - a function that traverses the page table hierarchy in order to translate
//!   virtual to physical addresses.
//! - a function to create new mappings in the page tables and to find unused
//!   memory frames for creating new page tables.

use x86_64::{
    structures::paging::PageTable,
    VirtAddr,
};

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the complete
/// physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once to
/// avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;

    // First, we read the physical frame of the active level 4 table from the
    // CR3 register.
    let (level_4_table_frame, _) = Cr3::read();

    // Then take its physical start address, convert it to an u64, and add it to
    // physical_memory_offset to get the virtual address where the page table
    // frame is mapped.
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    // Finally, we convert the virtual address to a `*mut PageTable` raw pointer
    // and then unsafely create a `&mut PageTable` reference from it. We create
    // a `&mut` reference instead of a `&` reference because we will mutate the
    // page tables later.
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}
