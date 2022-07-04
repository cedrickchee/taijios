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
    structures::paging::{
        PageTable, OffsetPageTable, Page, PhysFrame, Mapper, Size4KiB, FrameAllocator,
    },
    VirtAddr, PhysAddr,
};
use bootloader::bootinfo::{ MemoryMap, MemoryRegionType };

/// Initialize a new `OffsetPageTable`.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    // Returns a new OffsetPageTable instance with a 'static lifetime.
    // This means that the instance stays valid for the complete runtime of our
    // kernel.
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the complete
/// physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once to
/// avoid aliasing `&mut` references (which is undefined behavior).
/// 
/// This function should be only called from the `init` function from now on
/// because it can easily lead to aliased mutable references when called
/// multiple times, which can cause undefined behavior. For this reason, we make
/// the function private.
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
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

/// Creates an example mapping for the given virtual page to frame `0xb8000`,
/// the physical frame of the VGA text buffer. We choose that frame because it
/// allows us to easily test if the mapping was created correctly: We just need
/// to write to the newly mapped page and see whether we see the write appear on
/// the screen.
/// 
/// The `frame_allocator` parameter uses the `impl Trait` syntax to be generic
/// over all types that implement the `FrameAllocator` trait. The trait is
/// generic over the `PageSize` trait to work with both standard 4KiB pages and
/// huge 2MiB/1GiB pages.
pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    // We set the `PRESENT` flag because it is required for all valid entries
    // and the `WRITABLE` flag to make the mapped page writable.
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe {
        // The `map_to` method is unsafe because the caller must ensure that the
        // frame is not already in use. The reason for this is that mapping the
        // same frame twice could result in undefined behavior. In our case, we
        // reuse the VGA text buffer frame, which is already mapped, so we break
        // the required condition. However, the create_example_mapping function
        // is only a temporary testing function and will be removed after this
        // post, so it is OK.

        // FIXME: this is not safe, we do it only for testing.
        mapper.map_to(page, frame, flags, frame_allocator)
        // Note: The `map_to` function can fail, so it returns a `Result`. Since
        // this is just some example code that does not need to be robust, we
        // just use `expect` to panic when an error occurs. On success, the
        // function returns a `MapperFlush` type that provides an easy way to
        // flush the newly mapped page from the translation lookaside buffer
        // (TLB) with its `flush` method.
    };
    map_to_result.expect("map_to failed").flush();
}

/// A simple case and assume that we don’t need to create new page tables.
/// For this case, a `FrameAllocator` that always returns `None`.
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

/// A `FrameAllocator` that returns usable frames from the bootloader's memory
/// map.
pub struct BootInfoFrameAllocator {
    /// A `'static` reference to the memory map passed by the bootloader.
    memory_map: &'static MemoryMap,
    /// Keeps track of number of the next frame that the allocator should
    /// return.
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a `FrameAllocator` from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the
    /// passed memory map is valid. The main requirement is that all frames that
    /// are marked as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            // Initialized with 0 and will be increased for every frame
            // allocation to avoid returning the same frame twice.
            next: 0,
        }
    }

    /// An auxiliary method that returns an iterator over the usable frames
    /// specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // Get usable regions from memory map.
        //
        // Note: The `iter` method convert the memory map to an iterator of
        // `MemoryRegions`. The `filter` method to skip any reserved or
        // otherwise unavailable regions. The bootloader updates the memory map
        // for all the mappings it creates, so frames that are used by our
        // kernel (code, data or stack) or to store the boot information are
        // already marked as InUse or similar. Thus we can be sure that Usable
        // frames are not used somewhere else.
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .filter(|r| r.region_type == MemoryRegionType::Usable);
        // Map each region to its address range.
        //
        // Note: `map` combinator transform our iterator of memory regions to an
        // iterator of address ranges.
        //
        // `start_addr` method returns the physical start address of the memory
        // region.
        let addr_ranges = usable_regions
            .map(|r| r.range.start_addr()..r.range.end_addr());
        // Transform to an iterator of frame start addresses.
        //
        // Note: `flat_map` to transform the address ranges into an iterator of
        // frame start addresses, choosing every 4096th address using `step_by`.
        // Since 4096 bytes (= 4 KiB) is the page size, we get the start address
        // of each frame. The bootloader page aligns all usable memory areas so
        // that we don’t need any alignment or rounding code here.
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // Create `PhysFrame` types from the start addresses.
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        // Before returning that frame, we increase `self.next` by one so that
        // we return the following frame on the next call.
        self.next += 1;
        frame
    }
}

/*

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

*/

// ********** Sidenote **********
// 
// # Allocating frames
//
// The memory map is passed by the bootloader. It is provided by the BIOS/UEFI
// firmware. It can only be queried very early in the boot process, so the
// bootloader already calls the respective functions for us.
//
// The memory map is a map of the physical memory regions of the underlying
// machine. Thus it consists of a list of `MemoryRegion` structs, which contain
// the start address, the length, and the type (e.g. unused, reserved, etc.) of
// each memory region.
//
// ## Implementing the `FrameAllocator` trait
//
// This implementation is not quite optimal since it recreates the
// `usable_frame` allocator on every allocation. It would be better to directly
// store the iterator as a struct field instead. Then we wouldn’t need the `nth`
// method and could just call `next` on every allocation. The problem with this
// approach is that it’s not possible to store an `impl Trait` type in a struct
// field currently. It might work someday when [named existential
// types](https://github.com/rust-lang/rfcs/pull/2071) are fully implemented.
//
// With the boot info frame allocator, the mapping succeeds. Behind the scenes,
// the `map_to` method creates the missing page tables in the following way:
// - Allocate an unused frame from the passed `frame_allocator`.
// - Zero the frame to create a new, empty page table.
// - Map the entry of the higher level table to that frame.
// - Continue with the next table level.
//
// While our `create_example_mapping` function is just some example code, we are
// now able to create new mappings for arbitrary pages. This will be essential
// for allocating memory or implementing multithreading in future.
