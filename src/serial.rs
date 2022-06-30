//! # Serial module
//! 
//! Use the `uart_16550` crate to initialize the UART and send data over the
//! serial port.

use uart_16550::SerialPort; // struct that represents the UART registers
use spin::Mutex;
use lazy_static::lazy_static;

// By using lazy_static we can ensure that the init method is called exactly
// once on its first use.
lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        // The UART is programmed using port I/O. Since the UART is more
        // complex, it uses multiple I/O ports for programming different device
        // registers. The `unsafe` `SerialPort::new` function expects the
        // address of the first I/O port of the UART as argument, from which it
        // can calculate the addresses of all needed ports. We’re passing the
        // port address `0x3F8`, which is the standard port number for the first
        // serial interface.
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}

// 
// Serial port helpers
// 
// To make the serial port easily usable, we add serial_print! and
// serial_println! macros.
//

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;
    // To avoid deadlock, we can disable interrupts as long as the `Mutex` is
    // locked.
    interrupts::without_interrupts(|| {
        SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
    });
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
