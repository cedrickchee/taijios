//! # Interrupts module
//! 
//! Handle CPU exceptions in our kernel.

use x86_64::structures::idt::{ InterruptDescriptorTable, InterruptStackFrame };
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use crate::{ println, gdt };

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

/// Sets the offsets for the 8259 Programmable Interrupt Controllers (PICs) to
/// the range 32–47.
pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

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
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                // Assigns a IST stack to this handler in the IDT
                // by setting the stack index.
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
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

/// A double fault handler.
/// 
/// A double fault is a normal exception with an error code.
/// 
/// One difference to the breakpoint handler is that the double fault handler is
/// diverging. The reason is that the x86_64 architecture does not permit
/// returning from a double fault exception.
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    // Prints a short error message and dumps the exception stack frame. The
    // error code of the double fault handler is always zero, so there’s no
    // reason to print it.
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

// ********** Sidenote **********
// 
// # Hardware interrupts
// 
// Interrupts provide a way to notify the CPU from attached hardware devices.
// 
// Connecting all hardware devices directly to the CPU is not possible. Instead,
// a separate interrupt controller aggregates the interrupts from all devices
// and then notifies the CPU.
//
// Most interrupt controllers are programmable, which means that they support
// different priority levels for interrupts.
//
// Unlike exceptions, hardware interrupts occur asynchronously. This means that
// they are completely independent from the executed code and can occur at any
// time.
// 
// ## The 8259 PIC
// 
// The 8259 has 8 interrupt lines and several lines for communicating with the
// CPU. The typical systems back then were equipped with two instances of the
// 8259 PIC, one primary and one secondary PIC connected to one of the interrupt
// lines of the primary.
// 
// Each controller can be configured through two I/O ports, one “command” port
// and one “data” port. For the primary controller these ports are 0x20
// (command) and 0x21 (data). For the secondary controller they are 0xa0
// (command) and 0xa1 (data).
//
// ### Implementation
// 
// The default configuration of the PICs is not usable, because it sends
// interrupt vector numbers in the range 0–15 to the CPU. These numbers are
// already occupied by CPU exceptions, for example number 8 corresponds to a
// double fault.
// 
// The configuration happens by writing special values to the command and data
// ports of the PICs.
