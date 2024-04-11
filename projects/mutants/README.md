# Short-running projects for evaluation of safety using mutation testing

| Project                  | Stars | Domain                | Commit                                               | #Integr. | #Unit | #Mutants |
|--------------------------|-------|-----------------------|------------------------------------------------------|----------|-------|----------|
| orion-rs/orion           | 491   | cryptography library  | 0.17.0      cfa2c0c1e89f1ec3d2ab1ab1d57f88c1201e452c | 121      | 486   | 1856     |
| raphlinus/pulldown-cmark | 1.6k  | pull parser           | v0.9.0      967dd38554399573279855a9e124dc598a0e3200 | 839      | 50    | 1586     |
| rbatis/rbatis            | 2.1k  | compile-time ORM      | v4.4.1      0149c2862842771dd5a22a7ef69c9501053f546a | 87       | 18    | 230      |
| rust-lang/regex          | 2.8k  | regex engine          | 1.0.0       b5ef0ec281220d9047fed199ed48c29af9749570 | 5994     | 40    | 1703     |
| BurntSushi/ripgrep       | 37.4k | file content search   | 13.0.0      af6b6c543b224d348a8876f0c06245d9ea7929c5 | 271      | 2     | 484      |
| sfackler/rust-openssl    | 1.1k  | OpenSSL bindings      | v0.10.63    cc2850fff7c4b0d50a23e09059b0040044dd9616 | 4        | 380   | 1444     |
| rustls/rustls            | 4.4k  | TLS library           | v/0.21.0    45197b807cf0699c842fcb85eb8eca555c74cc04 | 150      | 169   | 1046     |
| zhiburt/tabled           | 1.4k  | Table printing        | v0.11.0     cc4a110d5963b7eede0e634c83c44d9e8b8250e4 | 1092     | 32    | 3887     |
| tokio-rs/tracing         | 3.8k  | instrumentation       | 0.1.38      3de7f8c6016aebc22228375dc9100c02e955c6d4 | 297      | 178   | 1009     |



## Special requirements

### rust-openssl
Requires `pkg-config`, `libssl-dev` and `libssl1.0` (last one not listed in the docs) to be installed, in case of Ubuntu.
On other operating systems this may differ, see https://docs.rs/openssl/latest/openssl/#automatic
