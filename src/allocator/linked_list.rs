//! # Linked list allocator module
//!
//! A heap backed by a linked list of free memory blocks.
//! 
//! This approach construct a single linked list in the freed memory, with each
//! node being a freed memory region.

use super::{ align_up, Locked };
use core::{ mem, ptr };
use alloc::alloc::{ GlobalAlloc, Layout };

struct ListNode {
    size: usize,
    // An optional pointer to the next node. The `&'static mut` type
    // semantically describes an owned object behind a pointer. Basically, it’s
    // a `Box` without a destructor that frees the object at the end of the
    // scope.
    next: Option<&'static mut ListNode>
}

// The type has a simple constructor function named `new` and methods to
// calculate the start and end addresses of the represented region.
impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

pub struct LinkedListAllocator {
    // A head node that points to the first heap region.
    head: ListNode,
}

impl LinkedListAllocator {
    /// Creates an empty LinkedListAllocator.
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// This function is unsafe because the caller must guarantee that the given
    /// heap bounds are valid and that the heap is unused. This method must be
    /// called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
    }

    /// Adds the given memory region to the front of the list.
    /// 
    /// This method provides the fundamental push operation on the linked list.
    /// We currently only call this method from `init`, but it will also be the
    /// central method in our `dealloc` implementation. Remember, the `dealloc`
    /// method is called when an allocated memory region is freed again. To keep
    /// track of this freed memory region, we want to push it to the linked
    /// list.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // ensure that the freed region is capable of holding ListNode
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        // create a new list node and append it at the start of the list
        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        node_ptr.write(node);
        self.head.next = Some(&mut *node_ptr)
    }

    /// Looks for a free region with the given size and alignment and removes it
    /// from the list.
    ///
    /// Returns a tuple of the list node and the start address of the
    /// allocation.
    fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        // Reference to current list node, updated for each iteration.
        let mut current = &mut self.head; // at the beginning, current is set to the (dummy) `head` node.
        // Look for a large enough memory region in linked list.
        // Iterate over the list elements.
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // Region suitable for allocation -> remove node from list
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // Region not suitable -> continue with next region
                current = current.next.as_mut().unwrap();
            }
        }
        // When the `current.next` pointer becomes None, the loop exits. This
        // means that we iterated over the whole list but found no region that
        // is suitable for an allocation. In that case, we return None.

        // no suitable region found
        None
    }

    /// The function checks whether a region is suitable for an allocation with
    /// given size and alignment.
    /// 
    /// Try to use the given region for an allocation with given size and
    /// alignment.
    ///
    /// Returns the allocation start address on success.
    fn alloc_from_region(region: &ListNode, size: usize, align: usize)
        -> Result<usize, ()>
    {
        // First, calculates the start and end address of a potential
        // allocation.
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        // This check is necessary because most of the time an allocation does
        // not fit a suitable region perfectly, so that a part of the region
        // remains usable after the allocation. This part of the region must
        // store its own ListNode after the allocation, so it must be large
        // enough to do so. The check verifies exactly that: either the
        // allocation fits perfectly (`excess_size == 0`) or the excess size is
        // large enough to store a ListNode.
        if alloc_end > region.end_addr() {
            // region too small
            return Err(());
        }
        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            // rest of region too small to hold a ListNode (required because the
            // allocation splits the region in a used and a free part)
            return Err(());
        }

        // region suitable for allocation
        Ok(alloc_start)
    }
    /// Adjust the given layout so that the resulting allocated memory region is
    /// also capable of storing a `ListNode`.
    ///
    /// Returns the adjusted size and alignment as a (size, align) tuple.
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            // increase the alignment to the alignment of a ListNode if
            // necessary.
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            // round up the size to a multiple of the alignment to ensure that
            // the start address of the next memory block will have the correct
            // alignment for storing a ListNode too.
            .pad_to_align();
        // the `max` method enforce a minimum allocation size of
        // `mem::size_of::<ListNode>`.
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Perform layout adjustments
        //
        // `size_align` function ensure that each allocated block is capable of
        // storing a `ListNode`. This is important because the memory block is
        // going to be deallocated at some point, where we want to write a
        // `ListNode` to it. If the block is smaller than a `ListNode` or does
        // not have the correct alignment, undefined behavior can occur.
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        // Uses the `find_region` method to find a suitable memory region for
        // the allocation and remove it from the list. If this doesn’t succeed
        // and `None` is returned, it returns `null_mut` to signal an error as
        // there is no suitable memory region.
        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                allocator.add_free_region(alloc_end, excess_size);
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Perform layout adjustments
        let (size, _) = LinkedListAllocator::size_align(layout);

        // add the deallocated region to the free list.
        self.lock().add_free_region(ptr as usize, size)
    }
}