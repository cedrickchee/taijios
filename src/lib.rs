// Like the main.rs, the lib.rs is a special file that is automatically
// recognized by cargo.

#![no_std] // The library is a separate compilation unit, so we need to specify the attribute again.
#![cfg_attr(test, no_main)] // use the cfg_attr crate attribute to conditionally enable the no_main attribute.
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)] // to use the `x86-interrupt` calling convention anyway (which is still unstable).

use core::panic::PanicInfo;

pub mod vga_buffer;
pub mod serial;
pub mod interrupts;

/// A central place for initialization routines.
pub fn init() {
    // Creates a new IDT.
    interrupts::init_idt();
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
pub fn test_runner(tests: &[&dyn Testable]) {
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

// A factor out implementation of our panic handler into a public function, so
// that it is available for executables too.
pub fn test_panic_handler(info: &PanicInfo) -> ! {
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

/// Entry point for `cargo test`.
///
/// Since our `lib.rs` is tested independently of our `main.rs`, we need to add
/// a `_start` entry point and a panic handler when the library is compiled in
/// test mode.
#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    init();
    test_main();

    loop {}
}

/// Panic handler in test mode.
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[test_case]
/// Verify that our breakpoint handler is working correctly, by checking that
/// the execution continues afterwards.
fn test_breakpoint_exception() {
    // Invoke a breakpoint exception.
    x86_64::instructions::interrupts::int3();
}
