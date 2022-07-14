//! # Executor module
//! 
//! An executor with waker support.
//! 
//! To fix the performance problem in simple executor, we need to create an
//! executor that properly utilizes the `Waker` notifications. This way, the
//! executor is notified when for example, the next keyboard interrupt occurs,
//! so it does not need to keep polling the `print_keypresses` task over and
//! over again.

use super::{ Task, TaskId };
use alloc::{ collections::BTreeMap, sync::Arc, task::Wake };
use core::task::{ Waker, Context, Poll };
use crossbeam_queue::ArrayQueue;

// Instead of storing tasks in a `VecDeque` like we did for our
// `SimpleExecutor`, we use a `task_queue` of task IDs and a `BTreeMap` named
// `tasks` that contains the actual `Task` instances. The map is indexed by the
// `TaskId` to allow efficient continuation of a specific task.
pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    // The `task_queue` field is an `ArrayQueue` of task IDs, wrapped into the
    // `Arc` type that implements _reference counting_. Reference counting makes
    // it possible to share ownership of the value between multiple owners. It
    // works by allocating the value on the heap and counting the number of
    // active references to it. When the number of active references reaches
    // zero, the value is no longer needed and can be deallocated.
    //
    // We use this `Arc<ArrayQueue>` type for the `task_queue` because it will
    // be shared between the executor and wakers. The idea is that the wakers
    // push the ID of the woken task to the queue. The executor sits on the
    // receiving end of the queue, retrieves the woken tasks by their ID from
    // the `tasks` map, and then runs them. The reason for using a fixed-size
    // queue instead of an unbounded queue such as `SegQueue` is that interrupt
    // handlers should not allocate on push to this queue.
    task_queue: Arc<ArrayQueue<TaskId>>,
    // This map caches the [`Waker`] of a task after its creation. This has two
    // reasons: First, it improves performance by reusing the same waker for
    // multiple wake-ups of the same task instead of creating a new waker each
    // time. Second, it ensures that reference-counted wakers are not
    // deallocated inside interrupt handlers because it could lead to deadlocks.
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
    // Creates an `Executor`.
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            // We choose a capacity of 100 for the `task_queue`, which should be
            // more than enough for the foreseeable future. In case our system
            // will have more than 100 concurrent tasks at some point, we can
            // easily increase this size.
            task_queue: Arc::new(ArrayQueue::new(100)),
            waker_cache: BTreeMap::new(),
        }
    }

    // Spaw task.
    //
    // Adds a given task to the tasks map and immediately wakes it by pushing
    // its ID to the task_queue.
    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;

        // If there is already a task with the same ID in the map, the
        // `BTreeMap::insert` method returns it. This should never happen since
        // each task has an unique ID, so we panic in this case since it
        // indicates a bug in our code. Similarly, we panic when the
        // `task_queue` is full since this should never happen if we choose a
        // large-enough queue size.
        if self.tasks.insert(task.id, task).is_some() {
            panic!("task with same ID already in tasks");
        }
        self.task_queue.push(task_id).expect("queue full");
    }

    // A run method for executor. It is efficient (in contrast to the simple
    // executor) since it utilize the notifications of the `Waker` type.
    pub fn run(&mut self) -> ! {
        // While we could theoretically return from the function when the
        // `tasks` map becomes empty, this would never happen since task for
        // example, our `keyboard_task` never finishes, so a simple `loop`
        // should suffice. Since the function never returns, we use the `!`
        // return type to mark the function as diverging to the compiler.
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    // Execute all tasks in the `task_queue`.
    //
    // The basic idea of this function is similar to our `SimpleExecutor`: Loop
    // over all tasks in the `task_queue`, create a waker for each task, and
    // then poll it. However, instead of adding pending tasks back to the end of
    // the `task_queue`, we let our `TaskWaker` implementation take care of of
    // adding woken tasks back to the queue.
    fn run_ready_tasks(&mut self) {
        // We use _destructuring_ to split `self` into its three fields to avoid
        // some borrow checker errors. Namely, our implementation needs to
        // access the `self.task_queue` from within a closure, which currently
        // tries to borrow `self` completely. This is a fundamental borrow
        // checker issue that will be resolved when [RFC 2229] is
        // [implemented][RFC 2229 impl].
        // 
        // [RFC 2229]: https://github.com/rust-lang/rfcs/pull/2229
        // [RFC 2229 impl]: https://github.com/rust-lang/rust/issues/53488
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        while let Ok(task_id) = task_queue.pop() {
            // For each popped task ID, we retrieve a mutable reference to the
            // corresponding task from the `tasks` map. Since our
            // `ScancodeStream` implementation registers wakers before checking
            // whether a task needs to be put to sleep, it might happen that a
            // wake-up occurs for a task that no longer exists. In this case, we
            // simply ignore the wake-up and continue with the next ID from the
            // queue.
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                None => continue, // task no longer exists
            };
            // To avoid the performance overhead of creating a waker on each
            // poll, we use the `waker_cache` map to store the waker for each
            // task after it has been created. For this, we use the
            // `BTreeMap::entry` method in combination with
            // `Entry::or_insert_with` to create a new waker if it doesn't exist
            // yet and then get a mutable reference to it. For creating a new
            // waker, we clone the `task_queue` and pass it together with the
            // task ID to the `TaskWaker::new` function. Since the `task_queue`
            // is wrapped into `Arc`, the `clone` only increases the reference
            // count of the value, but still points to the same heap allocated
            // queue. Note that reusing wakers like this is not possible for all
            // waker implementations, but our `TaskWaker` type will allow it.
            let waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));
            let mut context = Context::from_waker(waker);
            // A task is finished when it returns `Poll::Ready`. In that case,
            // we remove it from the `tasks` map using the `BTreeMap::remove`
            // method. We also remove its cached waker, if it exists.
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // task done -> remove it and its cached waker
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }

    // When using this executor, the CPU utilization of QEMU did not get any
    // better. The reason for this is that we still keep the CPU busy for the
    // whole time. We no longer poll tasks until they are woken again, but we
    // still check the `task_queue` in a busy loop. To fix this, we need to put
    // the CPU to sleep if there is no more work to do.
    fn sleep_if_idle(&self) {
        // ********** Sidenote **********
        //
        // The basic idea is to execute the [`hlt` instruction] when the
        // `task_queue` is empty. This instruction puts the CPU to sleep until
        // the next interrupt arrives. The fact that the CPU immediately becomes
        // active again on interrupts ensures that we can still directly react
        // when an interrupt handler pushes to the `task_queue`.
        // 
        // [`hlt` instruction]:
        //     https://en.wikipedia.org/wiki/HLT_(x86_instruction)

        // Since we call `sleep_if_idle` directly after `run_ready_tasks`, which
        // loops until the `task_queue` becomes empty, checking the queue again
        // might seem unnecessary. However, a hardware interrupt might occur
        // directly after `run_ready_tasks` returns, so there might be a new
        // task in the queue at the time the `sleep_if_idle` function is called.
        // Only if the queue is still empty, we put the CPU to sleep by
        // executing the `hlt` instruction through the [`instructions::hlt`]
        // wrapper function provided by the [`x86_64`] crate.
        // 
        // [`instructions::hlt`]:
        //     https://docs.rs/x86_64/0.14.2/x86_64/instructions/fn.hlt.html
        // [`x86_64`]: https://docs.rs/x86_64/0.14.2/x86_64/index.html
        if self.task_queue.is_empty() {
            x86_64::instructions::hlt();
        }
    }
}

// The job of the waker is to push the ID of the woken task to the `task_queue`
// of the executor. We implement this by creating a new `TaskWaker` struct that
// stores the task ID and a reference to the `task_queue`.
struct TaskWaker {
    task_id: TaskId,
    // Since the ownership of the `task_queue` is shared between the executor
    // and wakers, we use the `Arc` wrapper type to implement shared
    // reference-counted ownership.
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    // Creates waker.
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        // Convert `Arc`-wrapped values that implement the `Wake` trait.
        // 
        // This `from` method takes care of constructing a `RawWakerVTable` and
        // a `RawWaker` instance for our `TaskWaker` type.
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }

    // Wake operation.
    // 
    // Note: Since modifications of the `ArrayQueue` type only require a shared
    // reference, we can implement this method on `&self` instead of `&mut
    // self`.
    fn wake_task(&self) {
        self.task_queue.push(self.task_id).expect("task queue full");
    }
}

// In order to use our `TaskWaker` type for polling futures, we need to convert
// it to a [`Waker`] instance first. This is required because the
// [`Future::poll`] method takes a [`Context`] instance as argument, which can
// only be constructed from the `Waker` type. While we could do this by
// providing an implementation of the [`RawWaker`] type, it's both simpler and
// safer to instead implement the `Arc`-based [`Wake`][wake-trait] trait and
// then use the [`From`] implementations provided by the standard library to
// construct the `Waker`.
// 
// [wake-trait]: https://doc.rust-lang.org/nightly/alloc/task/trait.Wake.html
impl Wake for TaskWaker {
    // Note: Since wakers are commonly shared between the executor and the
    // asynchronous tasks, the trait methods require that the `Self` instance is
    // wrapped in the [`Arc`] type, which implements reference-counted
    // ownership. This means that we have to move our `TaskWaker` to an `Arc` in
    // order to call them.
    // 
    // The difference between the `wake` and `wake_by_ref` methods is that the
    // latter only requires a reference to the `Arc`, while the former takes
    // ownership of the `Arc` and thus often requires an increase of the
    // reference count. Not all types support waking by reference, so
    // implementing the `wake_by_ref` method is optional, however it can lead to
    // better performance because it avoids unnecessary reference count
    // modifications. In our case, we can simply forward both trait methods to
    // our `wake_task` function, which requires only a shared `&self` reference.
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}
