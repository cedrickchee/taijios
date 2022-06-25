use volatile::Volatile;
use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;

//
// A Global Writer as Interface
//

lazy_static! {
    // Create a new Writer that points to the VGA buffer at
    // memory address 0xb8000.

    // Note: use the spinning Mutex to add safe interior mutability to our
    // static WRITER
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        // syntax: cast the integer 0xb8000 as an mutable [raw
        // pointer](https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer).
        // Then we convert it to a mutable reference by dereferencing it
        // (through *) and immediately borrowing it again (through &mut). This
        // conversion requires an unsafe block, since the compiler can’t
        // guarantee that the raw pointer is valid.        
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },

        // ********** Sidenote **********
        // Raw pointer example - create a raw pointer to an arbitrary memory address:
        // let address = 0xb8000;
        // let rp = address as *mut Buffer;
        //         
        // Recall that we can create raw pointers in safe code, but we can’t
        // dereference raw pointers and read the data being pointed to.        
        // Example where we use the dereference operator * on a raw pointer that
        // requires an unsafe block.
        // let mut num = 5;
        // let rp = &mut num as *mut i32;
        // unsafe { *rp }        
    });
}

// 
// Colors
// 

#[allow(dead_code)] // normally the compiler would issue a warning for each unused variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)] // enable copy semantics for the type and make it printable and comparable.
#[repr(u8)]
// Represent the different colors using an enum
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)] // to ensure that the ColorCode has the exact same data layout as an u8.
// Represent a full color code that specifies foreground and background color,
// by creating a newtype on top of `u8.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)] // since the field ordering in default structs is undefined in Rust, we need this attribute.
// Represent a screen character.
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
// Represent the text buffer.
struct Buffer {
    // Use volatile lib to make writes to the VGA buffer volatile.
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

// 
// Printing
//

// To actually write to screen, we create a writer type.
pub struct Writer {
    // keep track of the current position in the last row.
    column_position: usize,
    // specify current foreground and background colors.
    color_code: ColorCode,
    // reference to the VGA buffer.
    buffer: &'static mut Buffer, // we need an explicit lifetime here to tell
    // the compiler how long the reference is valid.
    // The 'static lifetime specifies that the reference is valid for the whole
    // program run time (which is true for the VGA text buffer).
}

// Use the Writer to modify the buffer’s characters.
impl Writer {
    // Write a single ASCII byte.
    // Note:    
    // To be exact, it isn't exactly ASCII, but a character set named code
    // page 437 with some additional characters and slight modifications.
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
                // position.
                // Volatile::write method guarantees that the compiler will
                // never optimize away this write.
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                // Finally, the current column position is advanced.
                self.column_position += 1;
            }
        }
    }

    // Print whole strings.
    // 
    // The VGA text buffer only supports ASCII and the additional bytes of code
    // page 437. Rust strings are UTF-8 by default, so they might contain bytes
    // that are not supported by the VGA text buffer.
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

    // Moves all lines one line up and clears the last row.
    fn new_line(&mut self) {
        // Iterate over all screen characters and move each character one row
        // up.
        for row in 1..BUFFER_HEIGHT { // omit the 0th row (the first range starts at 1) because it’s the row that is shifted off screen.
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    // Clears a row by overwriting all of its characters with a space character.
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
