//! # Simple executor module
//! 
//! A basic executor.

use super::Task;
use alloc::collections::VecDeque;
use core::task::{ Waker, RawWaker, RawWakerVTable, Context, Poll };

pub struct SimpleExecutor {
    // `VecDeque` is basically a vector that allows to push and pop operations
    // on both ends.
    task_queue: VecDeque<Task>,
}

// The idea behind using this type is that we insert new tasks through the
// `spawn` method at the end and pop the next task for execution from the front.
// This way, we get a simple FIFO queue ("first in, first out").
impl SimpleExecutor {
    pub fn new() -> SimpleExecutor {
        SimpleExecutor {
            task_queue: VecDeque::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        self.task_queue.push_back(task)
    }

    // The most simple `run` method is to repeatedly poll all queued tasks in a
    // loop until all are done. This is not very efficient since it does not
    // utilize the notifications of the `Waker` type, but it is an easy way to
    // get things running.
    pub fn run(&mut self) {
        // Popping the task from the front of the `task_queue`.
        while let Some(mut task) = self.task_queue.pop_front() {
            // Creating a `RawWaker` for the task, converting it to a [`Waker`]
            // instance, and then creating a [`Context`] instance from it.

            let waker = dummy_waker();
            // For each task, first creates a `Context` type by wrapping a
            // `Waker` instance returned by our `dummy_waker` function.
            let mut context = Context::from_waker(&waker);

            // Calling the `poll` method on the future of the task, using the
            // `Context` we just created.
            match task.poll(&mut context) {
                // If the `poll` method returns `Poll::Ready`, the task is
                // finished and we can continue with the next task.
                Poll::Ready(())  => {}
                // If the task is still `Poll::Pending`, we add it to the back
                // of the queue again so that it will be polled again in a
                // subsequent loop iteration.
                Poll::Pending => self.task_queue.push_back(task),
            }
        }
    }
}

// To start simple, we will first create a dummy waker that does nothing.

// Defines the implementation of the different `Waker` methods.
fn dummy_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {} // does nothing
    fn clone(_: *const ()) -> RawWaker {
        // calling `dummy_raw_waker` again.
        dummy_raw_waker()
    }

    // We use the above two functions to create a minimal `RawWakerVTable`: The
    // `clone` function is used for the cloning operations and the `no_op`
    // function is used for all other operations.
    //
    // Since the `RawWaker` does nothing, it does not matter that we return a
    // new `RawWaker` from `clone` instead of cloning it.
    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    // After creating the `vtable`, we use the `RawWaker::new` function to
    // create the `RawWaker`. The passed `*const ()` does not matter since none
    // of the vtable functions uses it. For this reason, we simply pass a null
    // pointer.
    RawWaker::new(0 as *const (), vtable)
}

// Turn `RawWaker` instance into a `Waker`.
fn dummy_waker() -> Waker {    
    // The `from_raw` function is unsafe because undefined behavior can occur if
    // the programmer does not uphold the documented requirements of `RawWaker`. 
    unsafe { Waker::from_raw(dummy_raw_waker()) }
}

// ********** Sidenote **********
//
// # Dummy Waker
// 
// # RawWaker
// 
// The [`RawWaker`] type requires the programmer to explicitly define a
// [_virtual method table_] (_vtable_) that specifies the functions that should
// be called when the `RawWaker` is cloned, woken, or dropped. The layout of
// this vtable is defined by the [`RawWakerVTable`] type. Each function receives
// a `*const ()` argument that is basically a _type-erased_ `&self` pointer to
// some struct, e.g. allocated on the heap. The reason for using a `*const ()`
// pointer instead of a proper reference is that the `RawWaker` type should be
// non-generic but still support arbitrary types. The pointer value that is
// passed to the functions is the `data` pointer given to [`RawWaker::new`].
// 
// [_virtual method table_]: https://en.wikipedia.org/wiki/Virtual_method_table
// [`RawWakerVTable`]:
//     https://doc.rust-lang.org/stable/core/task/struct.RawWakerVTable.html
// [`RawWaker::new`]:
//     https://doc.rust-lang.org/stable/core/task/struct.RawWaker.html#method.new
// 
// Typically, the `RawWaker` is created for some heap allocated struct that is
// wrapped into the [`Box`] or [`Arc`] type. For such types, methods like
// [`Box::into_raw`] can be used to convert the `Box<T>` to a `*const T`
// pointer. This pointer can then be casted to an anonymous `*const ()` pointer
// and passed to `RawWaker::new`. Since each vtable function receives the same
// `*const ()` as argument, the functions can safely cast the pointer back to a
// `Box<T>` or a `&T` to operate on it. As you can imagine, this process is
// highly dangerous and can easily lead to undefined behavior on mistakes. For
// this reason, manually creating a `RawWaker` is not recommended unless
// necessary.
// 
// [`Box`]: https://doc.rust-lang.org/stable/alloc/boxed/struct.Box.html
// [`Arc`]: https://doc.rust-lang.org/stable/alloc/sync/struct.Arc.html
// [`Box::into_raw`]:
//     https://doc.rust-lang.org/stable/alloc/boxed/struct.Box.html#method.into_raw
