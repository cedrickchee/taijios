//! # A Stack Overflow Test
//! 
//! An integration test to test the `gdt` module and ensure that the double
//! fault handler is correctly called on a stack overflow.
//! 
//! The idea is to do provoke a double fault in the test function and verify
//! that the double fault handler is called.

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use tiny_os::{ exit_qemu, QemuExitCode, serial_print, serial_println };

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("stack_overflow::stack_overflow...\t");

    tiny_os::gdt::init();
    // Initialize a new GDT.    
    // Instead of calling our `interrupts::init_idt` function, we call a
    // `init_test_idt` function.
    init_test_idt();

    // Trigger a stack overflow.
    stack_overflow();

    panic!("Execution continued after stack overflow");
}

#[allow(unconditional_recursion)] // silence the compiler warning that the function recurses endlessly
fn stack_overflow() {
    stack_overflow(); // for each recursion, the return address is pushed
    volatile::Volatile::new(0).read(); // prevent tail recursion optimizations
    // ********** Sidenote **********
    //
    // We want that the stack overflow happens, so we add a dummy volatile read
    // statement at the end of the function, which the compiler is not allowed
    // to remove. Thus, the function is no longer tail recursive and the
    // transformation into a loop is prevented.
}

lazy_static! {
    // Like in the normal IDT, we set a stack index into the IST for the double
    // fault handler in order to switch to a separate stack.
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            idt.double_fault
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(tiny_os::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt
    };
}

/// Register a custom double fault handler that does a
/// `exit_qemu(QemuExitCode::Success)` instead of panicking.
pub fn init_test_idt() {
    // Loads the IDT on the CPU.
    TEST_IDT.load();
}

/// When the double fault handler is called, we exit QEMU with a success exit
/// code, which marks the test as passed.
extern "x86-interrupt" fn test_double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    tiny_os::test_panic_handler(info)
}
