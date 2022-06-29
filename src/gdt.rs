//! # Global Descriptor Table (GDT) module
//! 
//! Creates a Task State Segment (TSS). On x86_64, holds two stack tables (the
//! Interrupt Stack Table (IST) is one of them).

use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;
use lazy_static::lazy_static;

/// Define that the 0th IST entry is the double fault stack (any other IST index
/// would work too).
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    /// Creates a new TSS that contains a separate double fault stack in its
    /// interrupt stack table.
    /// 
    /// ********** Sidenote **********
    /// Note that this double fault stack has no guard page that protects
    /// against stack overflow. This means that we should not do anything stack
    /// intensive in our double fault handler because a stack overflow might
    /// corrupt the memory below the stack.
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        // Writes the top address of a double fault stack to the 0th entry. We
        // write the top address because stacks on x86 grow downwards, i.e. from
        // high addresses to low addresses.
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            // We haven’t implemented memory management yet, so we don’t have a
            // proper way to allocate a new stack. Instead, we use a `static
            // mut` array as stack storage for now.
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };
}

// ********** Sidenote **********
//
// Avoid stack overflow problem
//
// We need to ensure somehow that the stack is always valid when a double fault
// exception occurs. Fortunately, the x86_64 architecture has a solution to this
// problem.
//
// The x86_64 architecture is able to switch to a predefined, known-good stack
// when an exception occurs. This switch happens at hardware level, so it can be
// performed before the CPU pushes the exception stack frame.
//
// The switching mechanism is implemented as an Interrupt Stack Table (IST). The
// IST is a table of 7 pointers to known-good stacks.
