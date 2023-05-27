# `rustyRTS`

`rustyRTS` is a regression test selection tool for Rust projects.

# Prerequisites
- developed on Rust nightly-2023-01-20 - in other versions the API of rustc_driver may differ slightly

# Setup
To build `rustyRTS` simply run:
```
$ cargo install --path .
```
This will build the required executables and install them to your local cargo directory.

## Setup Rust toolchain
The correct toolchain should be installed automatically when building `rustyRTS`.
When applying it to any project use one of the following approaches:
```
$ rustup default nightly-2023-01-20 # (recommended: to use this toolchain by default everywhere)
$ rustup override set nightly-2023-01-20 # (to use this toolchain in current directory only)
```

# Usage
| Command  | Explanation |
| -------- | ----------- |
| `cargo rustyrts static` | perform static regression test selection and execute tests |
| `cargo rustyrts dynamic` | perform dynamic regression test selection and execute tests |
| `cargo rustyrts clean` | clean temporary directories |

## Custom arguments to `rustc`, `cargo build` or `cargo test`
`cargo rustyrts [dynamic|static] <args rustyrts> -- <args cargo build> -- <args rustc> -- <args cargo test (may contain --)>`

For example:
`cargo rustyrts dynamic -- -- --emit=mir` - to generate a human-readable representation of the MIR
`cargo rustyrts dynamic -- -- -- -- --test-threads 1` - to execute test single-threaded without forking

# How tests are selected

## Checksums
TODO

## Dynamic
Dynamic RustyRTS collects traces containing all functions that are called during the execution of a test. Some helper functions and global variables are used to obtain those traces:
- a `static HashSet<(&'static str, ..)>` for collecting names of traced functions
- `trace(input: &'static str, ..)` is used to append `input` to the hash set 
- `pre_test()` which initializes the hash set
- `post_test(test_name: &str)` which writes the content of the hash set, i.e. the traces to a file identified by the name of the test, where the traces can be inspected in the subsequent run

Further, on unix-like systems only:
- `pre_main()` which initializes the hash set, in case this has not already been done
- `post_main()` which appends the content of the hash set to a file identified by the `ppid` of the currently running process
- in both `post_test(test_name: &str)` and `post_main()` traces in files identified by the `pid` of the process (i.e. the `ppid` of any forked process), are appended to the hash set before exporting the traces

During compilation, the MIR is modified, automatically generating MIR code that does not reflect in source code. Dynamic RustyRTS injects function calls into certain MIR bodies:
- a call to `trace(<fn_name>)` at the beginning of every MIR body
- a call to `trace(<const_name>)` at the beginning of every MIR body for every `static` or `const` that is accessed in this particular body
- a call to `pre_test()` at the beginning of every test function
- a call to `post_test(<test_name>)` at the end of every test function

On unix-like systems only:
- a call to `pre_main()` at the beginning of every main function
- a call to `post_main()` at the end of every main function

Calls to `post_test(test_name: &str)` and `post_main()` are injected in such a way, that as long as the process terminates gracefully (i.e. either by exiting or by unwinding) the traces are written to the filesystem. A process crashing will result in the traces not being written!

On unix-like systems, a special test runner is used to fork for every test case, thus isolating the tests in their own process.
Forking ensures that traces do not intermix, when executing tests in parallel. When executing tests sequentially, forking is not necessary and can be omitted.

During the subsequent run, the traces are compared to the set of changed nodes. If these two sets overlap, the corresponding test is considered affected.

## Static
Static RustyRTS analyzes the MIR during compilation, without modifying it, to build a (directed) dependency graph. Edges are created according to the following criteria:
1. outer node  -> contained Closure
2. outer node  -> contained Generator
3. caller node  -> callee `fn`
4. outer node -> referenced abstract data type (`struct` or `enum`)
5. borrowing node -> `const var`
6. borrowing node -> `static var` or `static mut var`
7. abstract data type -> fn in trait impl (`impl <trait> for ..`)
8. fn in `trait` definition -> fn in trait impl (`impl <trait> for ..`)

Abstract data types, which are not corresponding to actual code are just used as "transit" nodes here.

When there is a path from a test to a changed node, the test is considered affected.