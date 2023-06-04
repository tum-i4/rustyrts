# Commands
```
RUSTFLAGS="-C link-arg=-fuse-ld=mold" cargo rustyrts dynamic
```

# Long-running projects

| Project                  | Stars | Domain                | #Integr. | #Unit | time retest-all  | comment                                |
|--------------------------|-------|-----------------------|----------|-------|------------------|----------------------------------------|
| actix/actix-web          | 17,5k | web framework         | 156      | 727   | 51s              |                                        |
| apache/arrow-datafusion  | 3.7k  | SQL Query Engine      | 1066     | 3964  | 2min 55s         | uses custom allocator                  |
| epi052/feroxbuster       | 4.2k  | discovery tool        | 113      | 250   | 49s              |                                        |
| nushell/nushell          | 24.5k | shell                 | 417      | 477   | 2min             |                                        |
| build-trust/ockam        | 3.1k  | cryptography          | 122      | 138   | 5min             | has trybuild tests                     |
| rayon-rs/rayon           | 8.6k  | parallelism library   | 114      | 205   | 55s              |                                        |
| rust-lang/rust-analyzer  | 12k   | compiler frontend     | 19       | 4825  | 1min             |                                        |
| wasmerio/wasmer          | 15.2k | web assembly runtime  | 1427     | 24    | 9min 14s         | special test features                  |
| eclipse-zenoh/zenoh      | 807   | data query protocol   | 101      | 51    | 3min 37s         |                                        |
| exonum/exonum            | 1.2k  | blockchain framework  | 359      | 690   | 2min 16s         |                                        |
| quickwit-oss/tantivy     | 8.1k  | text search           | 5        | 777   | 1min 21s         |                                        |
| meilisearch/meilisearch  | 36.7k | search engine         | 498      | 380   | 1min 31s         |                                        |
| spacejam/sled            | 7k    | embedded database     | 124      | 56    | 1min 46s         | fast compilation, special test feature |
|--------------------------|-------|-----------------------|----------|-------|------------------|----------------------------------------|
| penumbra-zone/penumbra   | 225   | blockchain            | 22       | 143   | 38min 33s        |                                        |
|--------------------------|-------|-----------------------|----------|-------|------------------|----------------------------------------|


## Would fit perfectly, but cannot be used due to technical issues

| denoland/deno            | 89.3k | JavaScript runtime    | starts a server using Lazy<> in the tests, that would be started in every test process in dynamic RustyRTS
| nexttest                 |       | testing framework     | uses signals that call traced functions, results in deadlock
| vercel/turbo             |       | JavaScript building   | uses custom allocator that calls traced functions, which results in deadlock
| facebook/buck            |       |                       | uses signals that call traced functions, results in deadlock


# Special requirements

## exonum
Requires that `rocksdb` is installed on the host system.
Build time of `librocksdb-sys` can then be shortened by using `ROCKSDB_LIB_DIR=<path to>/rust-rocksdb/librocksdb-sys/`


## penumbra
Requires `RUSTFLAGS="--cfg tokio_unstable`
Requires `git-lfs` to be installed and setup
Requires that `rocksdb` is installed on the host system.
Build time of `librocksdb-sys` can then be shortened by using `ROCKSDB_LIB_DIR=<path to>/rust-rocksdb/librocksdb-sys/`


## sled
```
RUSTFLAGS="-C link-arg=-fuse-ld=mold" cargo rustyrts dynamic -- --features testing -- -- --features testing --
```

## wasmer
```
RUSTFLAGS="-C link-arg=-fuse-ld=mold" cargo rustyrts dynamic -- --features test-singlepass,test-cranelift,test-universal -- -- --features test-singlepass,test-cranelift,test-universal --
```

- Feature `test-llvm` unfortunately results in linking error
