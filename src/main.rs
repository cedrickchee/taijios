#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(custom_test_frameworks)] // Rust supports replacing the default test framework through the unstable custom_test_frameworks feature
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"] // set the name of the test framework entry function to test_main

use core::panic::PanicInfo;

mod vga_buffer;
mod serial;

/// This function is the entry point, since the linker looks for a function
/// named `_start` by default.
#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // Write some characters to the screen.
    print!("H");
    print!("ello ");
    println!("Wörld!"); // test the handling of unprintable characters.
    println!("The numbers are {} and {}", 42, 1.0/3.0);

    // Call the renamed test framework entry function.
    #[cfg(test)] // use conditional compilation to add the call to `test_main` only in test contexts.
    test_main();

    loop {}
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

/// Panic handler in test mode.
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

/// Enum specify the exit status.
/// 
/// Exit with the success exit code if all tests succeeded and with the failure
/// exit code otherwise.
/// 
/// We use exit code `0x10` for success and `0x11` for failure. The actual exit
/// codes do not matter much, as long as they don’t clash with the default exit
/// codes of QEMU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    // Creates a new Port at 0xf4, which is the iobase of the `isa-debug-exit`
    // device. Then it writes the passed exit code to the port. We use `u32`
    // because we specified the `iosize` of the `isa-debug-exit` device as 4
    // bytes. Both operations are unsafe, because writing to an I/O port can
    // generally result in arbitrary behavior.
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

pub trait Testable {
    fn run(&self) -> ();
}

impl <T> Testable for T
where
    T: Fn()
{
    fn run(&self) {
        // Print the function name. `core::any::type_name` function is
        // implemented directly in the compiler and returns a string description
        // of every type.
        serial_print!("{}...\t", core::any::type_name::<T>());
        self(); // invoke the test function
        serial_println!("[ok]");
    }
}

/// Runner just prints a short debug message and then calls each test function
/// in the list.
#[cfg(test)]
fn test_runner(tests: &[&dyn Testable]) {
    // ********** Sidenote **********
    //
    // syntax: &[&dyn Fn()] is a slice of trait object references of the Fn()
    // trait. It is basically a list of references to types that can be called
    // like a function.

    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}