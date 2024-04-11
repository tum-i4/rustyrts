# Long-running projects

| Project                    | Stars   | Domain                  | #Tests (avg)   |
|----------------------------|---------|-------------------------|----------------|
| actix/actix-web            | 17,5k   | web framework           | 374.9          |
| apache/arrow-datafusion    | 3.7k    | SQL Query Engine        | 1603.17        |
| exonum/exonum              | 1.2k    | blockchain framework    | 921.13         |
| epi052/feroxbuster         | 4.2k    | discovery tool          | 261.73         |
| meilisearch/meilisearch    | 36.7k   | search engine           | 331.53         |
| nushell/nushell            | 24.5k   | shell                   | 500.3          |
| penumbra-zone/penumbra     | 225     | blockchain              | 170.93         |
| rayon-rs/rayon             | 8.6k    | parallelism library     | 265.13         |
| rust-lang/rust-analyzer    | 12k     | compiler frontend       | 2201.47        |
| quickwit-oss/tantivy       | 8.1k    | text search             | 582.53         |
| wasmerio/wasmer            | 15.2k   | web assembly runtime    | 516.73         |
| eclipse-zenoh/zenoh        | 807     | data query protocol     | 97.57          |
| -------------------------- | ------- | ----------------------- | ---------------|

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
