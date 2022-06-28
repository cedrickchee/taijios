//! # Interrupts module
//! 
//! Handle CPU exceptions in our kernel.

use x86_64::structures::idt::{ InterruptDescriptorTable, InterruptStackFrame };
use lazy_static::lazy_static;
use crate::println;

// Fix compile error: "`idt` does not live long enough".
// 
// In order to fix this problem, we need to store our `idt` at a place where it
// has a `'static` lifetime. To achieve this we could allocate our IDT on the
// heap using `Box` and then convert it to a `'static` reference, but we are
// writing an OS kernel and thus don’t have a heap (yet).
// 
// As an alternative we could try to store the IDT as a `static`.
//
// However, there is a problem: Statics are immutable, so we can’t modify the
// breakpoint entry from our `init` function. We could solve this problem by
// using a `static mut`.
//
// This variant compiles without errors but it’s far from idiomatic. `static
// mut`s are very prone to data races.
//
// Fortunately the `lazy_static` macro exists.
lazy_static! {
    /// creates a new `InterruptDescriptorTable`.
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        // Add breakpoint handler to our IDT.
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt
    };
}

pub fn init_idt() {
    // In order that the CPU uses our new interrupt descriptor table, we need to
    // load it using the `lidt` instruction.
    IDT.load();
}

/// A handler for the breakpoint exception.
/// 
/// The breakpoint exception is the perfect exception to test exception
/// handling. Its only purpose is to temporarily pause a program when the
/// breakpoint instruction `int3` is executed.
/// 
/// The breakpoint exception is commonly used in debuggers: When the user sets a
/// breakpoint, the debugger overwrites the corresponding instruction with the
/// `int3` instruction so that the CPU throws the breakpoint exception when it
/// reaches that line. When the user wants to continue the program, the debugger
/// replaces the `int3` instruction with the original instruction again and
/// continues the program.
/// 
/// For our use case, we don’t need to overwrite any instructions. Instead, we
/// just want to print a message when the breakpoint instruction is executed and
/// then continue the program.
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}
