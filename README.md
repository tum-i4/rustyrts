# RustyRTS

RustyRTS is a regression test selection tool for Rust projects.
Whether it is invoked _manually by a developer_ or _automatically in a CI pipeline_, it aims to reduce the time that is spent on waiting for the test results by excluding tests that do not actually run changed code.

RustyRTS provides multiple ways of selecting tests:

## Function-level RTS

- `cargo rustyrts dynamic` instruments all binaries to trace which functions are executed during the tests
  - ${\color{lightgreen}+++}$ extremely precise
  - ${\color{lightgreen}+}$ can trace child processes (linux only)
  - ${\color{red}-}$ prone to flakiness in case of random test input
  - ${\color{red}-}$ tampers with binaries (not always desired)
  - ${\color{red}-}$ needs to isolate tests in separate processes if tests are executed in parallel (not always feasible)
  - ${\color{red}-}$ needs to execute test sequentially/single-threaded on Windows
  - ${\color{orange}/}$ small compilation overhead, moderate runtime overhead

- `cargo rustyrts static` creates a directed dependency graph via static analysis
  - ${\color{lightgreen}++}$ quite precise
  - ${\color{lightgreen}+}$ does not tamper with binaries at all
  - ${\color{lightgreen}+}$ no runtime overhead
  - ${\color{red}-}$ cannot track dependencies of child processes
  - ${\color{orange}/}$ moderate compilation overhead

Whenever RustyRTS detects that some test depends on a function that has changed, this test is selected.

## Crate-level RTS

- `cargo rustyrts basic` runs tests only if the corresponding target has been (re-)compiled
  - ${\color{lightgreen}+}$ moderately precise
  - ${\color{lightgreen}+}$ does not tamper with binaries at all
  - ${\color{lightgreen}+}$ no runtime overhead
  - ${\color{lightgreen}+}$ negligible compiletime overhead
  - ${\color{red}-}$ cannot track dependencies of child processes

Whenever a test target is (re-)compiled, all of its tests are executed.

## Are you really trying to sell me three tools? Which one should I use?

In case your tests spawn additional processes, you may use `cargo rustyrts dynamic` **with extreme caution only if you know what you are doing**.

Otherwise, since it is the least invasive and thus safest technique, we recommend using `cargo rustyrts basic`, which will be applicable in (almost) any case.

If you want more fine-grained test selection and thus better testing time reduction, feel free to use `cargo rustyrts static`.

For even more precise selection use `cargo rustyrts dynamic`, after having thought through all the quirks that come with using it.

# Rust version

RustyRTS depends on the internals of the `rustc` compiler, which are quite unstable.
It has been developed for _v1.77.0-nightly_ and can currently only be used with this specific toolchain version.

## Setup Rust toolchain

The correct toolchain should be installed automatically when building `rustyRTS`.
When applying it to any project use one of the following approaches:

```
$ rustup default nightly-2023-12-28 # (recommended: to use this toolchain by default everywhere)
$ rustup override set nightly-2023-12-28 # (to use this toolchain in current directory only)
```

# How to install

To install RustyRTS clone the repository and run:

```
$ git clone https://github.com/tum-i4/rustyrts.git
$ cd rustyrts
$ cargo install --path . --locked
```

This will first install the required toolchain, if it is not present, and then build the required executables and install them to your local cargo directory.

# Usage

| Command                  | Explanation                                                     |
| ------------------------ | --------------------------------------------------------------- |
| `cargo rustyrts basic`   | perform crate-level regression test selection and execute tests |
| `cargo rustyrts static`  | perform static regression test selection and execute tests      |
| `cargo rustyrts dynamic` | perform dynamic regression test selection and execute tests     |

<!-- | `cargo rustyrts clean`   | clean temporary directories created by RustyRTS by default (or just use `cargo clean`) | -->

Using RustyRTS is **straight-forward and easy** since it has more or less the exact same command line interface as `cargo test`.
You can simply replace any invocation of `cargo test` by `cargo rustyrts <static|dynamic>`, keeping the arguments the same.
In case any command line argument you are about to use conflicts with the goal of regression test selection in general, RustyRTS will let you know via an error message.

On the first invocation, RustyRTS will execute all available tests. On every following one, tests will be selected based on the changes applied in between invocations.

## In CI pipelines

Since RustyRTS creates intermediate files that are processed on the following invocation, it is required to use some kind of caching mechanism, for example [rust-cache](https://github.com/Swatinem/rust-cache).

## Notable Examples

- `cargo rustyrts static -v` - to enable verbose mode

- `cargo rustyrts static -- -Z unstable-options --format=json` - to print test results in json format

- `cargo rustyrts dynamic --all-features` - to enable all features

- `RUSTFLAGS="--emit=mir" cargo rustyrts dynamic` - to obtain a human-readable dump of the MIR, including function calls injected for tracing

- `cargo rustyrts dynamic -- --test-threads=1` - to execute tests single-threaded without forking for every test
