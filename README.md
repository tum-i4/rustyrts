# `rustyRTS`

`rustyRTS` is a regression test selection tool for Rust projects.

# Prerequisites
- developed on Rust nightly-2023-01-20 - in other versions the API of rustc_driver may differ slightly
- components `rustc_dev` and `llvm-tools-preview`
```
$ rustup component add rustc-dev llvm-tools-preview
```

## Setup Rust version
```
$ rustup toolchain install nightly-2023-01-20
$ rustup default nightly-2023-01-20 # (recommended: to use this toolchain by default everywhere)
$ rustup override set nightly-2023-01-20 # (to use this toolchain in current directory only)
```

# Usage
| Command  | Explanation |
| -------- | ----------- |
| `cargo rustyrts` | perform regression test selection and execute tests |
| `cargo rustyrts clean` | clean temporary directories |

# Setup
To build `rustyRTS` simply run:
```
$ cargo install --path .
```
This will build the required executables and install them to your local cargo directory.