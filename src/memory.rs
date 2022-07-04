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
    VirtAddr, PhysAddr,
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

/// Translates the given virtual address to the mapped physical address, or
/// `None` if the address is not mapped.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`.
pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    // We forward the function to a safe `translate_addr_inner` function to
    // limit the scope of unsafe.
    translate_addr_inner(addr, physical_memory_offset)
}

/// Private function that is called by `translate_addr`.
/// 
/// Instead of reusing our `active_level_4_table` function, we read the level 4
/// frame from the CR3 register again. We do this because it simplifies this
/// prototype implementation.
///
/// This function is safe to limit the scope of `unsafe` because Rust treats the
/// whole body of unsafe functions as an unsafe block. This function must only
/// be reachable through `unsafe fn` from outside of this module.
fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    use x86_64::structures::paging::page_table::FrameError;
    use x86_64::registers::control::Cr3;

    // Read the active level 4 frame from the CR3 register.
    let (level_4_table_frame, _) = Cr3::read();

    // The `VirtAddr` struct already provides methods to compute the indexes
    // into the page tables of the four levels.
    let table_indexes = [
        addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()
    ];
    // Outside of the loop (below), we remember the last visited frame to
    // calculate the physical address later. The frame points to page table
    // frames while iterating, and to the mapped frame after the last iteration.
    let mut frame = level_4_table_frame;

    // Traverse the multi-level page table.
    for &index in &table_indexes {
        // Convert the frame into a page table reference.
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe { &*table_ptr };

        // Read the entry of the current page table and update `frame`.
        let entry = &table[index];
        frame = match entry.frame() { // use the `frame` fn to retrieve the mapped frame
            Ok(frame) => frame,
            // If the entry is not mapped to a frame we return `None`.
            Err(FrameError::FrameNotPresent) => return None,
            // If the entry maps a huge 2MiB or 1GiB page we panic for now.
            Err(FrameError::HugeFrame) => panic!("huge pages not supported"),
        };
    }

    // Calculate the physical address by adding the page offset.
    Some(frame.start_address() + u64::from(addr.page_offset()))
}