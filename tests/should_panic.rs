//! # Tests that should panic
//!
//! The test framework of the standard library supports a `#[should_panic]`
//! attribute that allows to construct tests that should fail. This is useful
//! for example to verify that a function fails when an invalid argument is
//! passed. Unfortunately this attribute isn’t supported in `#[no_std]` crates
//! since it requires support from the standard library.
//! 
//! While we can’t use the `#[should_panic]` attribute in our kernel, we can get
//! similar behavior by creating an integration test that exits with a success
//! error code from the panic handler.
//! 
//! A significant drawback of this approach is that it only works for a single
//! test function. With multiple `#[test_case]` functions, only the first
//! function is executed because the execution cannot continue after the panic
//! handler has been called. I currently don’t know of a good way to solve this
//! problem.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tiny_os::{QemuExitCode, exit_qemu, serial_println, serial_print};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}
