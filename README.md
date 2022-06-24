# Tiny OS

A tiny OS develop in Rust for learning systems programming and OSdev.

This repository contains the source code for the [A Freestanding Rust Binary][post] post of the [Writing an OS in Rust](https://os.phil-opp.com) series.

[post]: https://os.phil-opp.com/freestanding-rust-binary/

## Building

**Install Rust nightly**

This project requires a nightly version of Rust because it uses some unstable
features. At least nightly _2020-07-15_ is required for building. You might need
to run `rustup update nightly --force` to update to the latest nightly even if
some components such as `rustfmt` are missing it.

**The [`build-std` feature][cargo-build-std] of Cargo**

Building the kernel for our new target will fail if we don't use the feature. To
use the feature, we need to create a [Cargo configuration][cargo-config] file at
`.cargo/config.toml` with the following content:

```toml
...

[unstable]
build-std = ["core", "compiler_builtins"]
```

[cargo-build-std]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std
[cargo-config]: https://doc.rust-lang.org/cargo/reference/config.html


Then, to build this project, run:

```sh
$ cargo build --target x86_64-tiny_os.json
  Downloaded getopts v0.2.21
  ...
  Downloaded libc v0.2.126
  Downloaded compiler_builtins v0.1.73
  Downloaded cc v1.0.69
  ...
  Downloaded 14 crates (2.1 MB) in 1.36s
   Compiling core v0.0.0 (~/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core)
   Compiling compiler_builtins v0.1.73
   Compiling rustc-std-workspace-core v1.99.0 (~/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/rustc-std-workspace-core)
   Compiling tiny-os v0.1.0 (~/repo/github/tiny-os)
    Finished dev [unoptimized + debuginfo] target(s) in 11.90s
```

This compile for our custom target (bare metal).

### Linker Errors

The linker is a program that combines the generated code into an executable.
Since the executable format differs between Linux, Windows, and macOS, each
system has its own linker that throws a different error. The fundamental cause
of the errors is the same: the default configuration of the linker assumes that
our program depends on the C runtime, which it does not.

To solve the errors, we need to tell the linker that it should not include the C
runtime. We can do this either by passing a certain set of arguments to the
linker or by building for a bare metal target.

**Building for a Bare Metal Target**

By default Rust tries to build an executable that is able to run in your current
system environment. For example, if you’re using Windows on `x86_64`, Rust tries
to build a `.exe` Windows executable that uses `x86_64` instructions. This
environment is called your “host” system.

To describe different environments, Rust uses a string called [target
triple](https://clang.llvm.org/docs/CrossCompilation.html#target-triple).

By compiling for our host triple, the Rust compiler and the linker assume that
there is an underlying operating system such as Linux or Windows that use the C
runtime by default, which causes the linker errors. So to avoid the linker
errors, we can compile for a different environment with no underlying operating
system.

An example for such a bare metal environment is the `thumbv7em-none-eabihf` target
triple, which describes an embedded ARM system. The details are not important,
all that matters is that the target triple has no underlying operating system,
which is indicated by the `none` in the target triple. To be able to compile for
this target, we need to add it in rustup:

```sh
$ rustup target add thumbv7em-none-eabihf
info: downloading component 'rust-std' for 'thumbv7em-none-eabihf'
info: installing component 'rust-std' for 'thumbv7em-none-eabihf'
```

This downloads a copy of the standard (and core) library for the system. Now we
can build our freestanding executable for this target:

```sh
$ cargo build --target thumbv7em-none-eabihf
   Compiling tiny-os v0.1.0 (/home/neo/dev/work/repo/github/tiny-os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.78s
```

By passing a `--target` argument we cross compile our executable for a bare
metal target system. Since the target system has no operating system, the
linker does not try to link the C runtime and our build succeeds without any
linker errors.

This is the approach that we will use for building our OS kernel. Instead of
`thumbv7em-none-eabihf`, we will use a [custom target]
(https://doc.rust-lang.org/rustc/targets/custom.html) that describes a `x86_64`
bare metal environment. The details will be explained in the next post.
