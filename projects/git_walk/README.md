# Long-running projects

| Project                    | Stars   | Domain                  | #Integr.   | #Unit   | time retest-all    | comment                                  |
|----------------------------|---------|-------------------------|------------|---------|--------------------|------------------------------------------|
| actix/actix-web            | 17,5k   | web framework           | 156        | 727     | 51s                |                                          |
| apache/arrow-datafusion    | 3.7k    | SQL Query Engine        | 1066       | 3964    | 2min 55s           | uses different allocator from stdlib     |
| exonum/exonum              | 1.2k    | blockchain framework    | 359        | 690     | 2min 16s           |                                          |
| epi052/feroxbuster         | 4.2k    | discovery tool          | 113        | 250     | 49s                |                                          |
| meilisearch/meilisearch    | 36.7k   | search engine           | 498        | 380     | 1min 31s           |                                          |
| nushell/nushell            | 24.5k   | shell                   | 417        | 477     | 2min               |                                          |
| rayon-rs/rayon             | 8.6k    | parallelism library     | 114        | 205     | 55s                |                                          |
| rust-lang/rust-analyzer    | 12k     | compiler frontend       | 19         | 4825    | 1min               |                                          |
| spacejam/sled              | 7k      | embedded database       | 124        | 56      | 1min 46s           | fast compilation, special test feature   |
| quickwit-oss/tantivy       | 8.1k    | text search             | 5          | 777     | 1min 21s           |                                          |
| wasmerio/wasmer            | 15.2k   | web assembly runtime    | 1427       | 24      | 9min 14s           | special test features                    |
| eclipse-zenoh/zenoh        | 807     | data query protocol     | 101        | 51      | 3min 37s           |                                          |
| -------------------------- | ------- | ----------------------- | ---------- | ------- | ------------------ | ---------------------------------------- |
| penumbra-zone/penumbra     | 225     | blockchain              | 22         | 143     | 38min 33s          |                                          |
| -------------------------- | ------- | ----------------------- | ---------- | ------- | ------------------ | ---------------------------------------- |

# Special requirements

## penumbra
Requires `RUSTFLAGS="--cfg tokio_unstable`
Requires `git-lfs` to be installed and setup

## meilisearch
Tends to open a lot of files.
Increase soft limit of open files, append to `nano ~/.bashrc`:
```
ulimit -n 4096
```
