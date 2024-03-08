# RustyRTS

RustyRTS is a regression test selection tool for Rust projects.
It provides two ways of selecting tests:

- `cargo rustyrts dynamic` instruments all binaries to trace which functions are executed during the tests
  - ${\color{lightgreen}+++}$ extremely precise
  - ${\color{lightgreen}+}$ can trace child processes (linux only)
  - ${\color{red}-}$ tampers with binaries (not always desired)
  - ${\color{red}-}$ needs to isolate tests in separate processes if tests are executed in parallel (not always feasible)
  - ${\color{red}-}$ needs to execute test sequentially/single-threaded on Windows
  - ${\color{orange}/}$ small compilation overhead, moderate runtime overhead

- `cargo rustyrts static` creates a directed dependency graph via static analysis
  - ${\color{lightgreen}+}$ quite precise
  - ${\color{lightgreen}+}$ does not tamper with binaries at all
  - ${\color{lightgreen}+}$ no runtime overhead
  - ${\color{orange}/}$ moderate compilation overhead

Whenever it detects that some test depends on a function that has changed, this test is selected.

# Rust version
RustyRTS depends on the internals of the `rustc` compiler, which are quite unstable.
It has been developed for *v1.77.0-nightly* and can currently only be used with this specific toolchain version. 

## Setup Rust toolchain
The correct toolchain should be installed automatically when building `rustyRTS`.
When applying it to any project use one of the following approaches:
```
$ rustup default nightly-2023-01-20 # (recommended: to use this toolchain by default everywhere)
$ rustup override set nightly-2023-01-20 # (to use this toolchain in current directory only)
```

# How to install
To install RustyRTS simply run:
```
$ cargo install --path .
```
This will first install the required toolchain, if it is not present, and then build the required executables and install them to your local cargo directory.

# Usage
| Command  | Explanation |
| -------- | ----------- |
| `cargo rustyrts static` | perform static regression test selection and execute tests |
| `cargo rustyrts dynamic` | perform dynamic regression test selection and execute tests |
| `cargo rustyrts clean` | clean temporary directories |

## Custom arguments to `rustc`, `cargo build` or `cargo test`
`cargo rustyrts [dynamic|static] <args rustyrts> -- <args cargo build> -- <args rustc> -- <args cargo test (may contain --)>`
(We are planning to make this more ergonomic soon...)

For example:
`cargo rustyrts dynamic -- -- --emit=mir` - to generate a human-readable representation of the MIR
`cargo rustyrts dynamic -- -- -- -- --test-threads 1` - to execute test single-threaded without forking
