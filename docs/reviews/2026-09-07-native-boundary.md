# Native exception boundary and build invalidation

DRACO-04 and DRACO-06. This change follows cf59489. All compilation and tests run on 192.168.8.212 with Rust 1.94.1 and four workers.

Before the fix, allocation failure at the first encode allocation escapes the wrapper. The standalone tests/native_allocation.cc probe replaces operator new and checks encode and decode separately. The wrapper now catches exceptions across both complete operations. Error reporting uses string_view and does not allocate for literals. Output pointers are cleared before work starts.

The single-failure sweep passes at all 109 encode allocation sites and 57 decode allocation sites before each successful control. Failed calls return errors and publish no output. The probe links the same static Draco library as the Rust build. Compile on .212 using C++17, the third_party/draco/src and generated build include directories, and libdraco.a. Raw compile commands are reproducible from these paths in the remote source and target directories below.

A separate persistent-allocation-failure experiment aborts at encode allocation 29. GDB identifies Draco's RAnsBitEncoder destructor calling Clear, which allocates while unwinding. The outer wrapper cannot catch termination inside that destructor. This upstream limitation remains unresolved; this PR does not claim general recovery from memory exhaustion. The probe now injects one allocation failure at a time, explicitly testing the wrapper boundary rather than that upstream destructor behavior.

The build script tracks the full vendored source directory. Touching rans_bit_encoder.cc, previously outside the change list, causes Cargo to report the native crate dirty and rebuild successfully.

Validation commands:

```sh
cargo test
cargo build -vv
cargo test --release
cargo clippy --all-targets -- -D warnings
cargo test --release --test grid_corpus -- --ignored --nocapture
```

All commands pass on the final source. The standalone single-allocation sweep passes. The 932-file native corpus gate also passes. No sanitizer coverage is claimed. Source: /mnt/data2/draco/review-fixes-20260906/source. Target: /mnt/data2/draco/review-fixes-20260906/target. Raw logs and backtrace: /mnt/data2/draco/review-fixes-20260906/evidence/native-* on .212. They remain outside Git.

Standalone probe command for the recorded .212 build:

```sh
cd /mnt/data2/draco/review-fixes-20260906/source
ulimit -c 0
DRACO_NATIVE_BUILD=/mnt/data2/draco/review-fixes-20260906/target/release/build/tileforge-draco-158b24bed7327ed7/out/build
g++ -std=c++17 -O1 -Ithird_party/draco/src -I"$DRACO_NATIVE_BUILD" -Ithird_party/draco/third_party/eigen tests/native_allocation.cc "$DRACO_NATIVE_BUILD/libdraco.a" -o ../evidence/native-allocation
../evidence/native-allocation
```

A fresh Cargo build can use a different hash in the native build directory. Select the directory containing its libdraco.a and generated draco_features.h.
