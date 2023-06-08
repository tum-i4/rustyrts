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
RustyRTS (both static and dynamic) keeps track of modifications to the code by calculating and comparing checksums of MIR [`Body`s](https://doc.rust-lang.org/stable/nightly-rustc/rustc_middle/mir/struct.Body.html), which correspond to functions. When the checksum of a `Body` differs between old and new revision, it is considered changed.

Furthermore, the checksums of [`ConstAllocation`s](https://doc.rust-lang.org/stable/nightly-rustc/rustc_middle/mir/interpret/allocation/struct.ConstAllocation.html) corresponding to `static var` or `static mut var` are contributing to the checksum of every `Body` that accesses the respective variable. This enables dynamic RustyRTS to recognize changes in compile-time evaluation, where instrumentation for tracing is not possible. In static RustyRTS this allows to restrict the analysis to functions that are relevant at runtime (i.e. not only used in compile-time evaluation).

Lastly, the checksums of [`VtblEntry`s](https://doc.rust-lang.org/stable/nightly-rustc/rustc_middle/ty/vtable/enum.VtblEntry.html) (vtable entries) are contributing to the checksum of the function that they are pointing to. If a vtable entry that was pointing to a function a) in the old revision is now pointing to a different function b) in the new revision, both functions would be considered changed in static RustyRTS. In dynamic RustyRTS only function a) would be considered changed.

## Dynamic
Dynamic RustyRTS collects traces containing the names of all functions that are called during the execution of a test. Some helper functions and global variables are used to obtain those traces:
- a `static HashSet<(&'static str, ..)>` for collecting names of traced functions
- `trace(input: &'static str, ..)` is used to append `input` to the hash set 
- `pre_test()` which initializes the hash set
- `post_test(test_name: &str)` which writes the content of the hash set, i.e. the traces to a file identified by the name of the test, where the traces can be inspected in the subsequent run

Further, on unix-like systems only:
- `pre_main()` which initializes the hash set, in case this has not already been done
- `post_main()` which appends the content of the hash set to a file identified by the `ppid` of the currently running process
- in both `post_test(test_name: &str)` and `post_main()` traces in files identified by the `pid` of the process (i.e. the `ppid` of any forked process), are appended to the hash set before exporting the traces

During compilation, the MIR is modified, automatically generating MIR code that does not reflect in source code. Dynamic RustyRTS injects function calls into certain MIR `Body`s:
- a call to `trace(<fn_name>)` at the beginning of every MIR `Body`
- a call to `pre_test()` at the beginning of every test function
- a call to `post_test(<test_name>)` at the end of every test function

On unix-like systems only:
- a call to `pre_main()` at the beginning of every main function
- a call to `post_main()` at the end of every main function

Calls to `post_test(test_name: &str)` and `post_main()` are injected in such a way, that as long as the process terminates gracefully (i.e. either by exiting or by unwinding) the traces are written to the filesystem. A process crashing will result in the traces not being written!

Warning: `trace(<>)` internally uses allocations and locks, such that using custom allocators or signals may lead to a deadlock because of non-reentrant locks. (Using reentrant locks would lead to a stack overflow, which is equally bad.)

On unix-like systems, a special test runner is used to fork for every test case, thus isolating the tests in their own process.
Forking ensures that traces do not intermix, when executing tests in parallel. When executing tests sequentially, forking is not necessary and can be omitted.

During the subsequent run, the traces are compared to the set of changed `Body`s. If these two sets overlap, the corresponding test is considered affected.

## Static
Static RustyRTS analyzes the MIR during compilation, without modifying it, to build a (directed) dependency graph. Edges are created according to the following criteria:
1. `EdgeType::Closure`:         function  -> contained Closure
2. `EdgeType::Generator`:       function  -> contained Generator
3. 1. `EdgeType::FnDefTrait`:   caller function -> callee `fn` (for functions in `trait {..})
3. 2. `EdgeType::FnDef`:        caller function  -> callee `fn` (for non-assoc `fn`s, i.e. not inside `impl .. {..}`)
4. `EdgeType::Adt`:             function -> referenced abstract data type (`struct` or `enum`)
5. `EdgeType::AdtImpl`:         abstract data type -> fn in (not necessarily trait) impl (`impl <trait>? for ..`)
6. `EdgeType::TraitImpl`:       function in `trait` definition -> function in trait impl (`impl <trait> for ..`)

Abstract data types and traits, which are not corresponding to actual code are just used as "transit" nodes here. To not unnecessarily decrease precision, the names of these nodes are fully qualified, including substituted generics.
Using generics on function nodes as well (i.e. names of fully monomorphized functions) is not that useful, since RustyRTS compares checksums of non-monomorphized functions. Additionally, it would bloat up the graph, such that reading the graph would take a long time.

When there is a path from a test to a changed `Body`, the test is considered affected.