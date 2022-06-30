//! # Interrupts module
//! 
//! Handle CPU exceptions in our kernel.

use x86_64::structures::idt::{ InterruptDescriptorTable, InterruptStackFrame };
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use crate::{ print, println, gdt };

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

/// The timer uses line 0 of the primary PIC. This means that it arrives at the
/// CPU as interrupt 32 (0 + offset 32). Instead of hardcoding index 32, we
/// store it in an InterruptIndex enum.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard, // the keyboard uses line 1 of the primary PIC. This means that it arrives at the CPU as interrupt 33 (1 + offset 32).
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

/// Sets the offsets for the 8259 Programmable Interrupt Controllers (PICs) to
/// the range 32–47.
pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

// Fix compile error: "`idt` does not live long enough".
// 
// In order to fix this problem, we need to store our `idt` at a place where it
// has a `'static` lifetime. To achieve this we could allocate our IDT on the
// heap using `Box` and then convert it to a `'static` reference, but we are
// writing an OS kernel and thus don’t have a heap (yet).
// 
// As an alternative we could try to store the IDT as a `static`.
//
// However, there is a problem: Statics are immutable, so we can’t modify the
// breakpoint entry from our `init` function. We could solve this problem by
// using a `static mut`.
//
// This variant compiles without errors but it’s far from idiomatic. `static
// mut`s are very prone to data races.
//
// Fortunately the `lazy_static` macro exists.
lazy_static! {
    /// creates a new `InterruptDescriptorTable`.
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        // Add breakpoint handler to our IDT.
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                // Assigns a IST stack to this handler in the IDT
                // by setting the stack index.
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt[InterruptIndex::Timer.as_usize()]
            .set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()]
            .set_handler_fn(keyboard_interrupt_handler);
        idt
    };
}

pub fn init_idt() {
    // In order that the CPU uses our new interrupt descriptor table, we need to
    // load it using the `lidt` instruction.
    IDT.load();
}

/// A handler for the breakpoint exception.
/// 
/// The breakpoint exception is the perfect exception to test exception
/// handling. Its only purpose is to temporarily pause a program when the
/// breakpoint instruction `int3` is executed.
/// 
/// The breakpoint exception is commonly used in debuggers: When the user sets a
/// breakpoint, the debugger overwrites the corresponding instruction with the
/// `int3` instruction so that the CPU throws the breakpoint exception when it
/// reaches that line. When the user wants to continue the program, the debugger
/// replaces the `int3` instruction with the original instruction again and
/// continues the program.
/// 
/// For our use case, we don’t need to overwrite any instructions. Instead, we
/// just want to print a message when the breakpoint instruction is executed and
/// then continue the program.
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

/// A double fault handler.
/// 
/// A double fault is a normal exception with an error code.
/// 
/// One difference to the breakpoint handler is that the double fault handler is
/// diverging. The reason is that the x86_64 architecture does not permit
/// returning from a double fault exception.
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    // Prints a short error message and dumps the exception stack frame. The
    // error code of the double fault handler is always zero, so there’s no
    // reason to print it.
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

// A handler function for the timer interrupt.
extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    // As the timer interrupt happens periodically, we would expect to see a dot
    // appearing on each timer tick.
    print!(".");

    // End of interrupt.
    //
    // The PIC expects an explicit “end of interrupt” (EOI) signal from our
    // interrupt handler. This signal tells the controller that the interrupt
    // was processed and that the system is ready to receive the next interrupt.
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
        // ********** Sidenote **********
        //
        // The `notify_end_of_interrupt` figures out whether the primary or
        // secondary PIC sent the interrupt and then uses the command and data
        // ports to send an EOI signal to respective controllers. If the
        // secondary PIC sent the interrupt both PICs need to be notified
        // because the secondary PIC is connected to an input line of the
        // primary PIC.
        // 
        // We need to be careful to use the correct interrupt vector number,
        // otherwise we could accidentally delete an important unsent interrupt
        // or cause our system to hang. This is the reason that the function is
        // unsafe.
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use x86_64::instructions::port::Port;

    // To find out which key was pressed, we need to query the keyboard
    // controller. We do this by reading from the data port of the PS/2
    // controller, which is the I/O port with number `0x60`.
    let mut port = Port::new(0x60);
    // Read a byte from the keyboard's data port. This byte is called the
    // scancode and is a number that represents the key press/release.
    let scancode: u8 = unsafe { port.read() };
    
    // Translate the scancodes to keys.
    // 
    // Translates keypresses of the number keys 0-9 and ignores all other keys.
    let key = match scancode {
        0x02 => Some('1'),
        0x03 => Some('2'),
        0x04 => Some('3'),
        0x05 => Some('4'),
        0x06 => Some('5'),
        0x07 => Some('6'),
        0x08 => Some('7'),
        0x09 => Some('8'),
        0x0a => Some('9'),
        0x0b => Some('0'),
        _ => None,
    };
    if let Some(key) = key {
        print!("{}", key);
    }


    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

// ********** Sidenote **********
// 
// # Hardware interrupts
// 
// Interrupts provide a way to notify the CPU from attached hardware devices.
// 
// Connecting all hardware devices directly to the CPU is not possible. Instead,
// a separate interrupt controller aggregates the interrupts from all devices
// and then notifies the CPU.
//
// Most interrupt controllers are programmable, which means that they support
// different priority levels for interrupts.
//
// Unlike exceptions, hardware interrupts occur asynchronously. This means that
// they are completely independent from the executed code and can occur at any
// time.
// 
// ## The 8259 PIC
// 
// The 8259 has 8 interrupt lines and several lines for communicating with the
// CPU. The typical systems back then were equipped with two instances of the
// 8259 PIC, one primary and one secondary PIC connected to one of the interrupt
// lines of the primary.
// 
// Each controller can be configured through two I/O ports, one “command” port
// and one “data” port. For the primary controller these ports are 0x20
// (command) and 0x21 (data). For the secondary controller they are 0xa0
// (command) and 0xa1 (data).
//
// ### Implementation
// 
// The default configuration of the PICs is not usable, because it sends
// interrupt vector numbers in the range 0–15 to the CPU. These numbers are
// already occupied by CPU exceptions, for example number 8 corresponds to a
// double fault.
// 
// The configuration happens by writing special values to the command and data
// ports of the PICs.
// 
// ## Keyboard input
// 
// Like the hardware timer, the keyboard controller is already enabled by
// default. So when you press a key the keyboard controller sends an interrupt
// to the PIC, which forwards it to the CPU. The CPU looks for a handler
// function in the IDT, but the corresponding entry is empty. Therefore a double
// fault occurs.
// 
// Note that we only handle PS/2 keyboards here, not USB keyboards. However the
// mainboard emulates USB keyboards as PS/2 devices to support older software,
// so we can safely ignore USB keyboards until we have USB support in our
// kernel.
//
// We now see that a character appears on the screen when we press a key.
// However, this only works for the first key we press, even if we continue to
// press keys no more characters appear on the screen. This is because the
// keyboard controller won’t send another interrupt until we have read the
// so-called scancode of the pressed key.
//
// ### Interpreting the Scancodes
// 
// There are three different standards for the mapping between scancodes and
// keys, the so-called scancode sets. All three go back to the keyboards of
// early IBM computers: the IBM XT, the IBM 3270 PC, and the IBM AT. Later
// computers fortunately did not continue the trend of defining new scancode
// sets, but rather emulated the existing sets and extended them. Today most
// keyboards can be configured to emulate any of the three sets.
// 
// By default, PS/2 keyboards emulate scancode set 1 (“XT”). In this set, the
// lower 7 bits of a scancode byte define the key, and the most significant bit
// defines whether it’s a press (“0”) or a release (“1”). Keys that were not
// present on the original IBM XT keyboard, such as the enter key on the keypad,
// generate two scancodes in succession: a `0xe0` escape byte and then a byte
// representing the key. For a list of all set 1 scancodes and their
// corresponding keys, check out the [OSDev
// Wiki](https://wiki.osdev.org/Keyboard#Scan_Code_Set_1).
