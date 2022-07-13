//! # Keyboard module
//! 
//! Async keyboard input:
//! 
//! - An asynchronous task based on the keyboard interrupt.
//! - A global keyboard scancode queue.

use crate::{ print, println };

use core::{
    pin::Pin,
    task::{Context, Poll},
};
use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use futures_util::{
    stream::{ Stream, StreamExt },
    task::AtomicWaker,
};
use pc_keyboard::{ layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1 };

// Since the `ArrayQueue::new` performs a heap allocation, which is not possible
// at compile time (yet), we can’t initialize the static variable directly.
// Instead, we use the `OnceCell` type of the `conquer_once` crate, which makes
// it possible to perform safe one-time initialization of static values.
//
// Instead of the `OnceCell` primitive, we could also use the `lazy_static`
// macro here. However, the `OnceCell` type has the advantage that we can ensure
// that the initialization does not happen in the interrupt handler, thus
// preventing that the interrupt handler performs a heap allocation.
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
// To implement the `Waker` notification for our `ScancodeStream`, we need a
// place where we can store the `Waker` between poll calls. We can't store it as
// a field in the `ScancodeStream` itself because it needs to be accessible from
// the `add_scancode` function. The solution for this is to use a static
// variable of the `AtomicWaker` type provided by the `futures-util` crate. Like
// the `ArrayQueue` type, this type is based on atomic instructions and can be
// safely stored in a static and modified concurrently.
static WAKER: AtomicWaker = AtomicWaker::new();

/// Fill the scancode queue.
/// 
/// Called by the keyboard interrupt handler
///
/// Must not block or allocate heap.
pub(crate) fn add_scancode(scancode: u8) {
    // Since this function should not be callable from `main.rs`, we use the
    // `pub(crate)` visibility to make it only available to `lib.rs`.

    // Use the `OnceCell::try_get` to get a reference to the initialized queue.
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            // In case the queue is full, we print a warning too.
            println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            // Wake the stored Waker, which notifies the executor. Otherwise,
            // the operation is a no-op, i.e. nothing happens.
            WAKER.wake();
        }
    } else {
        // If the queue is not initialized yet, we ignore the keyboard scancode
        // and print a warning.
        println!("WARNING: scancode queue uninitialized");
    }
}

// `ScancodeStream` type initializes the `SCANCODE_QUEUE` and read the scancodes
// from the queue in an asynchronous way.
pub struct ScancodeStream {
    // Field prevent construction of the struct from outside of the module.
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        // Try to initialize the `SCANCODE_QUEUE` static. Panic if it is already
        // initialized to ensure that only a single `ScancodeStream` instance
        // can be created.
        SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}

// Make the scancodes available to asynchronous tasks by implementing
// `poll`-like method that tries to pop the next scancode off the queue. While
// this sounds like we should implement the `Future` trait for our type, this
// does not quite fit here. The problem is that the `Future` trait only
// abstracts over a single asynchronous value and expects that the `poll` method
// is not called again after it returns `Poll::Ready`. Our scancode queue,
// however, contains multiple asynchronous values so that it is ok to keep
// polling it.
impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        // Get a reference to the initialized scancode queue. This should never
        // fail since we initialize the queue in the `new` function, so we can
        // safely use the `expect` method to panic if it's not initialized.
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("not initialized");

        // Fast path
        //
        // Optimistically try to `pop` from the queue and return `Poll::Ready`
        // when it succeeds. This way, we can avoid the performance overhead of
        // registering a waker when the queue is not empty.
        if let Ok(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }
        // ********** Sidenote **********
        //
        // If the first call to `queue.pop()` does not succeed, the queue is
        // potentially empty. Only potentially because the interrupt handler
        // might have filled the queue asynchronously immediately after the
        // check. Since this race condition can occur again for the next check,
        // we need to register the `Waker` in the `WAKER` static before the
        // second check. This way, a wakeup might happen before we return
        // `Poll::Pending`, but it is guaranteed that we get a wakeup for any
        // scancodes pushed after the check.

        // Stores the current waker in the static WAKER.
        //
        // The contract defined by `poll_next` requires that the task registers
        // a wakeup for the passed `Waker` when it returns `Poll::Pending`.
        WAKER.register(&cx.waker());

        // Try popping from the queue a second time.
        //
        // Try to get the next element from the queue. If it succeeds we return
        // the scancode wrapped in `Poll::Ready(Some(…))`. If it fails, it means
        // that the queue is empty. In that case, we return `Poll::Pending`.
        match queue.pop() {
            Ok(scancode) => {
                // Remove the registered waker again using `AtomicWaker::take`
                // because a waker notification is no longer needed.
                WAKER.take();
                Poll::Ready(Some(scancode))
            },
            // In case `queue.pop()` fails for a second time, we return
            // `Poll::Pending` like before, but this time with a registered
            // wakeup.
            Err(crossbeam_queue::PopError) => Poll::Pending,
        }
    }
}

// Use `Stream` trait to create an async keyboard task.
pub async fn print_keypresses() {
    // Instead of reading the scancode from an I/O port, we take it from the
    // ScancodeStream.
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1,
        HandleControl::Ignore);
    
    // Repeatedly use the `next` method provided by the `StreamExt` trait to get
    // a `Future` that resolves to the next element in the stream. By using the
    // `await` operator on it, we asynchronously wait for the result of the
    // future.
    //
    // We use `while let` to loop until the stream returns `None` to signal its
    // end. Since our `poll_next` method never returns `None`, this is
    // effectively an endless loop, so the `print_keypresses` task never
    // finishes.
    while let Some(scancode) = scancodes.next().await {
        // Translate the scancodes to keys.
        //
        // Pass the scancode to the `add_byte` method, which translates the
        // scancode into an `Option<KeyEvent>`. The `KeyEvent` contains which
        // key caused the event and whether it was a press or release event.
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            // To interpret this key event, we pass it to the `process_keyevent`
            // method, which translates the key event to a character if
            // possible.            
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}

// ********** Sidenote **********
//
// # Async Keyboard Input
//
// We created an asynchronous task based on the keyboard interrupt. The keyboard
// interrupt is a good candidate for this because it is both non-deterministic
// and latency-critical. Non-deterministic means that there is no way to predict
// when the next key press will occur because it is entirely dependent on the
// user. Latency-critical means that we want to handle the keyboard input in a
// timely manner, otherwise the user will feel a lag. To support such a task in
// an efficient way, it will be essential that the executor has proper support
// for `Waker` notifications.
//
// ## Scancode Queue
//
// Previously, we handle the keyboard input directly in the interrupt handler.
// This is not a good idea for the long term because interrupt handlers should
// stay as short as possible as they might interrupt important work. Instead,
// interrupt handlers should only perform the minimal amount of work necessary
// (e.g. reading the keyboard scancode) and leave the rest of the work (e.g.
// interpreting the scancode) to a background task.
//
// A common pattern for delegating work to a background task is to create some
// sort of queue. The interrupt handler pushes units of work to the queue and
// the background task handles the work in the queue. Applied to our keyboard
// interrupt, this means that the interrupt handler only reads the scancode from
// the keyboard, pushes it to the queue, and then returns. The keyboard task
// sits on the other end of the queue and interprets and handles each scancode
// that is pushed to it.
//
// A simple implementation of that queue could be a mutex-protected `VecDeque`.
// However, using mutexes in interrupt handlers is not a good idea since it can
// easily lead to deadlocks. For example, when the user presses a key while the
// keyboard task has locked the queue, the interrupt handler tries to acquire
// the lock again and hangs indefinitely. Another problem with this approach is
// that VecDeque automatically increases its capacity by performing a new heap
// allocation when it becomes full. This can lead to deadlocks again because our
// allocator also uses a mutex internally. Further problems are that heap
// allocations can fail or take a considerable amount of time when the heap is
// fragmented.
//
// To prevent these problems, we need a queue implementation that does not
// require mutexes or allocations for its push operation. Such queues can be
// implemented by using lock-free atomic operations for pushing and popping
// elements. This way, it is possible to create push and pop operations that
// only require a &self reference and are thus usable without a mutex. To avoid
// allocations on push, the queue can be backed by a pre-allocated fixed-size
// buffer. While this makes the queue bounded (i.e. it has a maximum length), it
// is often possible to define reasonable upper bounds for the queue length in
// practice so that this isn’t a big problem.
//
// ### Queue Implementation
//
// Using the `ArrayQueue` type, we can now create a global scancode queue in
// this module.
//
// ## Scancode Stream
// 
// ### The Stream Trait
//
// Since types that yield multiple asynchronous values are common, the `futures`
// crate provides a useful abstraction for such types: the `Stream` trait.
//
// This definition is quite similar to the `Future` trait.
//
// There is also a semantic difference: The `poll_next` can be called
// repeatedly, until it returns `Poll::Ready(None)` to signal that the stream is
// finished. In this regard, the method is similar to the `Iterator::next`
// method, which also returns `None` after the last value.
//
// ## Waker Support
//
// Like the `Futures::poll` method, the `Stream::poll_next` method requires that
// the asynchronous task notifies the executor when it becomes ready after
// `Poll::Pending` is returned. This way, the executor does not need to poll the
// same task again until it is notified, which greatly reduces the performance
// overhead of waiting tasks.
// 
// To send this notification, the task should extract the `Waker` from the
// passed `Context` reference and store it somewhere. When the task becomes
// ready, it should invoke the `wake` method on the stored `Waker` to notify the
// executor that the task should be polled again.
//
// The idea is that the `poll_next` implementation stores the current waker in
// this static and the `add_scancode` function calls the `wake` function on it
// when a new scancode is added to the queue.
//
// ### Waking the Stored Waker
//
// It is important that we call `wake` only after pushing to the queue because
// otherwise the task might be woken too early when the queue is still empty.
// This can for example happen when using a multi-threaded executor that starts
// the woken task concurrently on a different CPU core. While we don't have
// thread support yet, we will add it soon and we don't want things to break
// then.
//
// ## Keyboard Task
//
// Now that we implemented the `Stream` trait for our `ScancodeStream`, we can
// use it to create an asynchronous keyboard task.
//
// We add the `print_keypresses` task to our executor in our `main.rs` to get
// working keyboard input again.
//
// When we execute `cargo run` now, we see that keyboard input works again.
//
// If you keep an eye on the CPU utilization of your computer, you will see that
// the `QEMU` process now continuously keeps the CPU busy. This happens because
// our `SimpleExecutor` polls tasks over and over again in a loop. So even if we
// don't press any keys on the keyboard, the executor repeatedly calls `poll` on
// our `print_keypresses` task, even though the task cannot make any progress
// and will return `Poll::Pending` each time.
