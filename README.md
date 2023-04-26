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