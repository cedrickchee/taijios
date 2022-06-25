#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

use core::panic::PanicInfo;

mod vga_buffer;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start` by default
    loop {}
}

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // Write some characters to the screen.
    use core::fmt::Write;
    vga_buffer::WRITER.lock().write_str("Hello again").unwrap();
    vga_buffer::WRITER.lock().write_byte(b'H'); // the b prefix creates a byte literal, which represents an ASCII character.
    vga_buffer::WRITER.lock().write_string("ello ");
    vga_buffer::WRITER.lock().write_string("Wörld!\n"); // test the handling of unprintable characters.
    write!(vga_buffer::WRITER.lock(), "The numbers are {} and {}", 42, 1.0/3.0).unwrap();

    loop {}
}
