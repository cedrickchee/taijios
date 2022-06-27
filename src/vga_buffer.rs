//! # VGA Text Buffer
//!
//! A module that defines the structure of the VGA text buffer and encapsulates
//! the unsafety of writing to the memory mapped buffer. It also presents a safe
//! and convenient interface to the outside.

use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;
use volatile::Volatile;

//
// A Global Writer as Interface
//

lazy_static! {
    /// Create a new Writer that points to the VGA buffer at memory address
    /// 0xb8000.
    ///
    /// ********** Sidenote **********
    /// Raw pointer example - create a raw pointer to an arbitrary memory address:
    /// let address = 0xb8000;
    /// let rp = address as *mut Buffer;
    ///
    /// Recall that we can create raw pointers in safe code, but we can’t
    /// dereference raw pointers and read the data being pointed to.
    /// Example where we use the dereference operator * on a raw pointer that
    /// requires an unsafe block.
    /// let mut num = 5;
    /// let rp = &mut num as *mut i32;
    /// unsafe { *rp }
    ///
    /// Note: use the spinning Mutex to add safe interior mutability to our
    /// static WRITER
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        /// syntax: cast the integer 0xb8000 as an mutable [raw
        /// pointer](https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer).
        /// Then we convert it to a mutable reference by dereferencing it
        /// (through *) and immediately borrowing it again (through &mut). This
        /// conversion requires an unsafe block, since the compiler can’t
        /// guarantee that the raw pointer is valid.
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

//
// Colors
//

/// Represents the standard color palette in VGA text mode.
#[allow(dead_code)] // normally the compiler would issue a warning for each unused variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)] // enable copy semantics for the type and make it printable and comparable.
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// A combination of a foreground and a background color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// `repr` ensures that the ColorCode has the exact same data layout as an u8.
// Represent a full color code that specifies foreground and background color,
// by creating a newtype on top of `u8.
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        // Struct contains the full color byte, containing foreground and
        // background color.
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

//
// VGA Text Buffer
//

/// A screen character in the VGA text buffer, consisting of an ASCII character
/// and a `ColorCode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Since the field ordering in default structs is undefined in Rust, we need
// this attribute. Represent a screen character.
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

/// The height of the text buffer (normally 25 lines).
const BUFFER_HEIGHT: usize = 25;
/// The width of the text buffer (normally 80 columns).
const BUFFER_WIDTH: usize = 80;

/// A structure representing the VGA text buffer.
#[repr(transparent)]
struct Buffer {
    // Use volatile lib to make writes to the VGA buffer volatile.
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

//
// Printing
//

/// A writer type that allows writing ASCII bytes and strings to an underlying
/// `Buffer` (it's actually writing to screen).
///
/// Wraps lines at `BUFFER_WIDTH`. Supports newline characters and implements
/// the `core::fmt::Write` trait.
pub struct Writer {
    /// Keep track of the current position in the last row.
    column_position: usize,
    /// Specify current foreground and background colors.
    color_code: ColorCode,
    /// Reference to the VGA buffer.
    buffer: &'static mut Buffer, // we need an explicit lifetime here to tell
                                 // the compiler how long the reference is valid.
                                 // The 'static lifetime specifies that the reference is valid for the whole
                                 // program run time (which is true for the VGA text buffer).
}

/// Use the Writer to modify the buffer’s characters.
impl Writer {
    /// Write a single ASCII byte.
    /// 
    /// Wraps lines at `BUFFER_WIDTH`. Supports the `\n` newline character.
    /// 
    /// ********** Sidenote **********
    /// To be exact, it isn't exactly ASCII, but a character set named code page
    /// 437 with some additional characters and slight modifications.
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                // When printing a byte, the writer checks if the current line
                // is full. In that case, a new_line call is required before to
                // wrap the line.
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;

                // Writes a new ScreenChar to the buffer at the current
                // position. Volatile::write method guarantees that the
                // compiler will never optimize away this write.
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                // Finally, the current column position is advanced.
                self.column_position += 1;
            }
        }
    }

    /// Writes the given ASCII string to the buffer.
    ///
    /// Wraps lines at `BUFFER_WIDTH`. Supports the `\n` newline character. Does
    /// **not** support strings with non-ASCII characters, since they can't be
    /// printed in the VGA text mode.
    ///
    /// ********** Sidenote **********
    /// The VGA text buffer only supports ASCII and the additional bytes of code
    /// page 437. Rust strings are UTF-8 by default, so they might contain bytes
    /// that are not supported by the VGA text buffer.
    pub fn write_string(&mut self, s: &str) {
        // Convert string to bytes and print them one-by-one.
        for byte in s.bytes() {
            match byte {
                // Printable ASCII byte or newline
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // Not part of printable ASCII range.
                // For unprintable bytes, we print a ■ character, which has the
                // hex code 0xfe on the VGA hardware.
                _ => self.write_byte(0xfe),
            }
        }
    }

    /// Moves all lines one line up and clears the last row.
    fn new_line(&mut self) {
        // Iterate over all screen characters and move each character one row
        // up.
        for row in 1..BUFFER_HEIGHT {
            // Omit the 0th row (the first range starts at 1) because it’s the
            // row that is shifted off screen.
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    /// Clears a row by overwriting all of its characters with a space character.
    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

//
// println Macro
//

// We won’t try to write a macro from scratch. Instead we look at the source of
// the [println! macro](https://doc.rust-lang.org/nightly/std/macro.println!.html)
// in the standard library.
//
// To print to the VGA buffer, we just copy the println! and print! macros, but
// modify them to use our own _print function.

/// Like the `print!` macro in the standard library, but prints to the VGA text
/// buffer.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

/// Like the `println!` macro in the standard library, but prints to the VGA
/// text buffer.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}


/// Prints the given formatted string to the VGA text buffer through the global
/// `WRITER` instance.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    // Locks our static WRITER and calls the write_fmt method on it.
    WRITER.lock().write_fmt(args).unwrap();
    // ********** Sidenote **********
    // Note 1: The additional unwrap() at the end panics if printing isn’t
    // successful. But since we always return Ok in write_str, that should not
    // happen.
    //
    // Note 2: Since the macros need to be able to call _print from outside of
    // the module, the function needs to be public.
}

/// A very simple test to verify that println works without panicking.
#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}

/// A test to ensure that no panic occurs even if many lines are printed and
/// lines are shifted off the screen.
#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}

/// A test function to verify that the printed lines really appear on the
/// screen.
#[test_case]
fn test_println_output() {
    // Defines a test string, prints it using `println`, and then iterates over
    // the screen characters of the static `WRITER`, which represents the vga
    // text buffer. Since `println` prints to the last screen line and then
    // immediately appends a newline, the string should appear on line
    // `BUFFER_HEIGHT - 2`.
    let s = "Some test string that fits on a single line";
    println!("{}", s);
    for (i, c) in s.chars().enumerate() {
        let screen_char = WRITER.lock().buffer.chars[BUFFER_HEIGHT - 2][i].read();
        assert_eq!(char::from(screen_char.ascii_character), c);
    }
}
