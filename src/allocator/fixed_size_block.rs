//! # Fixed-size block allocator module
//!
//! An allocator that uses fixed-size memory blocks for fulfilling allocation
//! requests.
//! 
//! This way, the allocator often returns blocks that are larger than needed for
//! allocations, which results in wasted memory due to internal fragmentation.
//! On the other hand, it drastically reduces the time required to find a
//! suitable block (compared to the linked list allocator), resulting in much
//! better allocation performance.

use alloc::alloc::{ Layout, GlobalAlloc };
use core::{
    ptr::{ self, NonNull },
    mem,
};
use super::Locked;

/// The block sizes to use.
///
/// The sizes must each be power of 2 because they are also used as the block
/// alignment (alignments must be always powers of 2).
/// 
/// We don’t define any block sizes smaller than 8 because each block must be
/// capable of storing a 64-bit pointer to the next block when freed. For
/// allocations greater than 2048 bytes we will fall back to a linked list
/// allocator.
/// 
/// To simplify the implementation, we define that the size of a block is also
/// its required alignment in memory. So a 16 byte block is always aligned on a
/// 16-byte boundary and a 512 byte block is aligned on a 512-byte boundary.
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

/// A helper function that choose an appropriate (lowest possible) block size
/// for the given layout.
///
/// Returns an index into the `BLOCK_SIZES` array.
fn list_index(layout: &Layout) -> Option<usize> {
    // The block must have at least the size and alignment required by the given
    // layout.
    let required_block_size = layout.size().max(layout.align());
    // To find the next-larger block in the `BLOCK_SIZES` slice, we first use
    // the `iter()` method to get an iterator and then the `position()` method
    // to find the index of the first block that is as least as large as the
    // `required_block_size`.
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

struct ListNode {
    // An optional pointer to the next node. The `&'static mut` type
    // semantically describes an owned object behind a pointer. Basically, it’s
    // a `Box` without a destructor that frees the object at the end of the
    // scope.
    next: Option<&'static mut ListNode>    
}

// The allocator type.
pub struct FixedSizeBlockAllocator {
    // An array of `head` pointers, one for each block size.
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    // As a fallback allocator for allocations larger than the largest block
    // size we use the allocator provided by the `linked_list_allocator`.
    fallback_allocator: linked_list_allocator::Heap,
}

impl FixedSizeBlockAllocator {
    /// Creates an empty FixedSizeBlockAllocator.
    /// 
    /// Initializes the `list_heads` array with empty nodes and creates an
    /// `empty` linked list allocator as `fallback_allocator`.
    pub const fn new() -> Self {
        // Tell the compiler that we want to initialize the array with a
        // constant value. Initializing the array directly as `[None;
        // BLOCK_SIZES.len()]` does not work because then the compiler requires
        // that `Option<&'static mut ListNode>` implements the `Copy` trait,
        // which it does not. This is a current limitation of the Rust compiler.
        const EMPTY: Option<&'static mut ListNode> = None;

        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// This function is unsafe because the caller must guarantee that the given
    /// heap bounds are valid and that the heap is unused. This method must be
    /// called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback_allocator.init(heap_start, heap_size);
    }

    /// A convenience method that allocates using the `fallback allocator`.
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        // Since the `Heap` type of the `linked_list_allocator` crate does not
        // implement `GlobalAlloc` (as it’s not possible without locking).
        // Instead, it provides an `allocate_first_fit` method that has a
        // slightly different interface. Instead of returning a `*mut u8` and
        // using a null pointer to signal an error, it returns a
        // `Result<NonNull<u8>`, ()>. The `NonNull` type is an abstraction for a
        // raw pointer that is guaranteed to be not the null pointer. By mapping
        // the `Ok` case to the `NonNull::as_ptr` method and the `Err` case to a
        // null pointer, we can easily translate this back to a `*mut u8` type.
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Get a mutable reference to the wrapped allocator instance.
        let mut allocator = self.lock();
        
        // Calculate the appropriate block size for the given layout and get the
        // corresponding index into the `list_heads` array.
        match list_index(&layout) {
            Some(index) => {
                // We try to remove the first node in the corresponding list
                // started by `list_heads[index]` using the `Option::take`
                // method.
                match allocator.list_heads[index].take() {
                    // If the list is not empty, we enter this branch of the
                    // match statement, where we point the head pointer of the
                    // list to the successor of the popped `node` (by using
                    // `take` again). Finally, we return the popped `node`
                    // pointer as a `*mut u8`.
                    Some(node) => {
                        allocator.list_heads[index] = node.next.take();
                        node as *mut ListNode as *mut u8
                    }
                    // If the list head is `None`, it indicates that the list of
                    // blocks is empty. This means that we need to construct a
                    // new block. For that, we first get the current block size
                    // from the `BLOCK_SIZES` slice and use it as both the size
                    // and the alignment for the new block. Then we create a new
                    // `Layout` from it and call the `fallback_alloc` method to
                    // perform the allocation. The reason for adjusting the
                    // layout and alignment is that the block will be added to
                    // the block list on deallocation.
                    None => {
                        // no block exists in list => allocate new block
                        let block_size = BLOCK_SIZES[index];
                        // only works if all block sizes are a power of 2
                        let block_align = block_size;
                        let layout = Layout::from_size_align(block_size, block_align)
                            .unwrap();
                        allocator.fallback_alloc(layout)
                    }
                }
            }
            // No block size fits for the allocation, therefore we use the
            // `fallback_allocator` using the `fallback_alloc` function.
            None => allocator.fallback_alloc(layout),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Get a mutable allocator reference.
        let mut allocator = self.lock();
     
        // Get the block list corresponding to the given layout.
        match list_index(&layout) {
            // If `list_index` returns a block index, we need to add the freed
            // memory block to the list. For that, we first create a new
            // `ListNode` that points to the current list head (by using
            // `Option::take` again). Before we write the new node into the
            // freed memory block, we first assert that the current block size
            // specified by `index` has the required size and alignment for
            // storing a `ListNode`. Then we perform the write by converting the
            // given `*mut u8` pointer to a `*mut ListNode` pointer and then
            // calling the unsafe `write` method on it. The last step is to set
            // the head pointer of the list, which is currently `None` since we
            // called take on it, to our newly written `ListNode`. For that we
            // convert the raw `new_node_ptr` to a mutable reference.
            Some(index) => {
                let new_node = ListNode {
                    next: allocator.list_heads[index].take(),
                };
                // verify that block has size and alignment required for storing node
                assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
                assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
                let new_node_ptr = ptr as *mut ListNode;
                new_node_ptr.write(new_node);
                allocator.list_heads[index] = Some(&mut *new_node_ptr);
            }
            // No fitting block size exists in `BLOCK_SIZES`, which indicates
            // that the allocation was created by the fallback allocator.
            // Therefore we use its `deallocate` to free the memory again.
            None => {
                // The `deallocate` method expects a `NonNull` instead of a
                // `*mut u8`, so we need to convert the pointer first.
                let ptr = NonNull::new(ptr).unwrap();
                allocator.fallback_allocator.deallocate(ptr, layout);
            }
        }
    }
}

// ********** Sidenote **********
//
// How a fixed-size block allocator works?
//
// # Introduction
//
// The idea behind a fixed-size block allocator is the following: Instead of
// allocating exactly as much memory as requested, we define a small number of
// block sizes and round up each allocation to the next block size. For example,
// with block sizes of 16, 64, and 512 bytes, an allocation of 4 bytes would
// return a 16-byte block, an allocation of 48 bytes a 64-byte block, and an
// allocation of 128 bytes an 512-byte block.
//
// Like the linked list allocator, we keep track of the unused memory by
// creating a linked list in the unused memory. However, instead of using a
// single list with different block sizes, we create a separate list for each
// size class. Each list then only stores blocks of a single size.
//
// Instead of a single `head` pointer, we have the three head pointers
// `head_16`, `head_64`, and `head_512` that each point to the first unused
// block of the corresponding size. All nodes in a single list have the same
// size. For example, the list started by the `head_16` pointer only contains
// 16-byte blocks. This means that we no longer need to store the size in each
// list node since it is already specified by the name of the head pointer.
//
// Since each element in a list has the same size, each list element is equally
// suitable for an allocation request. This means that we can very efficiently
// perform an allocation.
//
// Most notably, we can always return the first element of the list and no
// longer need to traverse the full list. Thus, allocations are much faster than
// with the linked list allocator.
//
// ## Block Sizes and Wasted Memory
//
// Depending on the block sizes, we lose a lot of memory by rounding up. By
// defining reasonable block sizes, it is possible to limit the amount of wasted
// memory to some degree. For example, when using the powers of 2 (4, 8, 16, 32,
// 64, 128, …) as block sizes, we can limit the memory waste to half of the
// allocation size in the worst case and a quarter of the allocation size in the
// average case.
//
// It is also common to optimize block sizes based on common allocation sizes in
// a program. For example, we could additionally add block size 24 to improve
// memory usage for programs that often perform allocations of 24 bytes. This
// way, the amount of wasted memory can be often reduced without losing the
// performance benefits.
//
// ## Deallocation
//
// Like allocation, deallocation is also very performant. Most notably, no
// traversal of the list is required for deallocation either. This means that
// the time required for a `dealloc` call stays the same regardless of the list
// length.
//
// ## Fallback Allocator
//
// Given that large allocations (>2KB) are often rare, especially in operating
// system kernels, it might make sense to fall back to a different allocator for
// these allocations. For example, we could fall back to a linked list allocator
// for allocations greater than 2048 bytes in order to reduce memory waste.
// Since only very few allocations of that size are expected, the linked list
// would stay small so that (de)allocations would be still reasonably fast.
//
// ## Creating new Blocks
//
// Above, we always assumed that there are always enough blocks of a specific
// size in the list to fulfill all allocation requests. However, at some point
// the linked list for a block size becomes empty. At this point, there are two
// ways how we can create new unused blocks of a specific size to fulfill an
// allocation request:
// - Allocate a new block from the fallback allocator (if there is one).
// - Split a larger block from a different list. This best works if block sizes
//   are powers of two. For example, a 32-byte block can be split into two
//   16-byte blocks.
// 
// For our implementation, we will allocate new blocks from the fallback
// allocator since the implementation is much simpler.
