#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(tiny_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use tiny_os::println;

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    tiny_os::test_panic_handler(info)
}

// The test might seem a bit useless right now since it’s almost identical to
// one of the VGA buffer tests. However, in the future the `_start` functions of
// our `main.rs` and `lib.rs` might grow and call various initialization
// routines before running the `test_main` function, so that the two tests are
// executed in very different environments.
// 
// By testing `println` in a `basic_boot` environment without calling any
// initialization routines in `_start`, we can ensure that `println` works right
// after booting. This is important because we rely on it e.g. for printing
// panic messages.
#[test_case]
fn test_println() {
    println!("test_println output");
}
