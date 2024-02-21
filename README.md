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

Lastly, the checksums of [`VtblEntry`s](https://doc.rust-lang.org/stable/nightly-rustc/rustc_middle/ty/vtable/enum.VtblEntry.html) (vtable entries) are contributing to the checksum of the function that they are pointing to.
Assume a vtable entry that was pointing to a function a) in the old revision is now pointing to a different function b) in the new revision.
Because static RustyRTS is working entirely on the graph data of the new revision, it is sufficient to consider function b) changed, as long as there is a continuous path from a corresponding test to function b).
Dynamic RustyRTS is comparing traces originating from the old revision, which is why function a) would be considered changed.
Because static RustyRTS can distinguish whether a function is called via dynamic or static dispatch, these additional checksums of vtable entries only contribute in the case of dynamic dispatch.

## Dynamic
Dynamic RustyRTS collects traces containing the names of all functions that are called during the execution of a test. Some helper functions and global variables are used to obtain those traces:
- a lock-free linked list (using nodes *compiled into the binary as static variables*) for collecting names of traced functions (technically a set, since every node can appear at most once)
- `trace(input: &'static str, ..)` is used to append `input` to the hash set 
- `pre_test()` which clears the list
- `post_test(test_name: &str)` which writes the content of the list, i.e. the traces to a file identified by the name of the test, where the traces can be inspected in the subsequent run

Further, on unix-like systems only:
- `post_main()` which appends the content of the list to a file identified by the `ppid` of the currently running process
- in both `post_test(test_name: &str)` and `post_main()` traces in files identified by the `pid` of the process (i.e. the `ppid` of any forked process), are appended to the hash set before exporting the traces

During compilation, the MIR is modified, automatically generating MIR code that does not reflect in source code. Dynamic RustyRTS injects function calls into certain MIR `Body`s:
- a call to `trace(<fn_name>)` at the beginning of every MIR `Body`
- a call to `pre_test()` at the beginning of every test function
- a call to `post_test(<test_name>)` at the end of every test function

On unix-like systems only:
- a call to `post_main()` at the end of every main function

Calls to `post_test(test_name: &str)` and `post_main()` are injected in such a way, that as long as the process terminates gracefully (i.e. either by exiting or by unwinding) the traces are written to the filesystem. **A process crashing will result in the traces not being written!**

On unix-like systems, a special test runner is used to fork for every test case, thus *isolating the tests in their own process*.
Forking ensures that traces do not intermix, when executing tests in parallel. When executing tests sequentially, forking is not necessary and can be omitted.

During the subsequent run, the traces are compared to the set of changed `Body`s. If these two sets overlap, the corresponding test is considered affected.

## Static
Static RustyRTS analyzes the MIR during compilation, without modifying it, to build a (directed) dependency graph.
The way this is done is derived from the algorithm used for monomorphization in `rustc`.

Edges are created according to the following criteria:
1. `EdgeType::Call`             function -> callee function (static dispatch)

2. `EdgeType::Contained`        function -> contained closure

3. 1. `EdgeType::Unsize`        function -> function in the vtable of a type that is converted into a dynamic trait object (unsized coercion) + !dyn
3. 2. `EdgeType::Unsize`        function in vtable (see above) + !dyn -> same function (without suffix)

4. `EdgeType::Drop`             function -> destructor (`drop()` function) of types that are dropped (manually or automatically)

5. 1. `EdgeType::Static`        function -> accessed static variable
5. 2. `EdgeType::Static`        static variable -> static variable that is pointed to
5. 3. `EdgeType::FnPtr`         static variable (see above) -> function that is pointed to

6. 1. `EdgeType::ReifyPtr`      function -> function that is coerced to a function pointer
6. 1. `EdgeType::ClosurePtr`    function -> closure that is coerced to a function pointer

The suffix !dyn is used to distinguish static and dynamic dispatch. Checksums from vtable entries only contribute to the function they are pointing to with suffix !dyn.

When there is a path from a test to a changed `Body`, the test is considered affected.
