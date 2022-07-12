//! # Task module

use core::{ 
    future::Future, 
    pin::Pin,
    task::{ Context, Poll },
};
use alloc::boxed::Box;

// A newtype wrapper around a pinned, heap allocated, and dynamically dispatched
// future with the empty type `()` as output.
pub struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    // Create a new Task structs from futures.
    //
    // The `'static` lifetime is required here because the returned `Task` can
    // live for an arbitrary time, so the future needs to be valid for that time
    // too.
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            // Pins `future` in memory.
            future: Box::pin(future),
        }
    }

    // Allow the executor to poll the stored future.
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        // Since the `poll` method of the `Future` trait expects to be called on
        // a `Pin<&mut T>` type, we use the `Pin::as_mut` method to convert the
        // `self.future` field of type `Pin<Box<T>>` first. Then we `call` poll
        // on the converted `self.future` field and return the result. Since the
        // `Task::poll` method should be only called by the executor, we keep
        // the function private to the `task` module.
        self.future.as_mut().poll(context)
    }
}

// ********** SIdenote **********
//
// # Implementation
//
// ## Task
//
// The `Task` struct is a newtype wrapper around a pinned, heap allocated, and
// dynamically dispatched future with the empty type `()` as output. Let’s go
// through it in detail:
// - We require that the future associated with a task returns `()`. This means
//   that tasks don’t return any result, they are just executed for its side
//   effects. For example, the `example_task` function we defined in `main.rs`
//   has no return value, but it prints something to the screen as a side
//   effect.
// - The `dyn` keyword indicates that we store a trait object in the Box. This
//   means that the methods on the future are dynamically dispatched, which
//   makes it possible to store different types of futures in the Task type.
//   This is important because each `async fn` has its own type and we want to
//   be able to create multiple different tasks.
// - The `Pin<Box>` type ensures that a value cannot be moved in memory by
//   placing it on the heap and preventing the creation of `&mut` references to
//   it. This is important because futures generated by async/await might be
//   self-referential, i.e. contain pointers to itself that would be invalidated
//   when the future is moved.
