//! # Interrupts module
//! 
//! Handle CPU exceptions in our kernel.

use x86_64::structures::idt::{ InterruptDescriptorTable, InterruptStackFrame };
use crate::println;

/// creates a new `InterruptDescriptorTable`.
pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
    // Add breakpoint handler to our IDT.
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    // In order that the CPU uses our new interrupt descriptor table, we need to
    // load it using the `lidt` instruction.
    idt.load();
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
