#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

use core::panic::PanicInfo;

mod vga_buffer;

// static HELLO: &[u8] = b"Hello World!";

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start` by default
    loop {}
}

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    /*
    let vga_buffer = 0xb8000 as *mut u8; // cast the integer 0xb8000 into a raw pointer.
    
    // Iterate over the bytes of the static HELLO byte string.
    for (i, &byte) in HELLO.iter().enumerate() {
        // An unsafe block around all memory writes.
        // 
        // The reason is that the Rust compiler can’t prove that the raw
        // pointers we create are valid. They could point anywhere and lead to
        // data corruption. By putting them into an unsafe block we’re basically
        // telling the compiler that we are absolutely sure that the operations
        // are valid. Note that an unsafe block does not turn off Rust’s safety
        // checks. It only allows you to do [five additional
        // things](https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html#unsafe-superpowers).
        unsafe {
            // offset method calculates the offset from a pointer.
            // 
            // Use the offset method to write the string byte and the
            // corresponding color byte (0xb is a light cyan).
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }
    */

    // Write some characters to the screen.
    use core::fmt::Write;
    vga_buffer::WRITER.lock().write_str("Hello again").unwrap();
    vga_buffer::WRITER.lock().write_byte(b'H'); // the b prefix creates a byte literal, which represents an ASCII character.
    vga_buffer::WRITER.lock().write_string("ello ");
    vga_buffer::WRITER.lock().write_string("Wörld!\n"); // test the handling of unprintable characters.
    write!(vga_buffer::WRITER.lock(), "The numbers are {} and {}", 42, 1.0/3.0).unwrap();

    loop {}
}
