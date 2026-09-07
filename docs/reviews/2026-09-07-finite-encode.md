# Finite encoder input validation

DRACO-01. Base: merged grid-snapping revision f27a329. All compilation and tests run on 192.168.8.212 with Rust 1.94.1, CARGO_BUILD_JOBS=4, and CMAKE_BUILD_PARALLEL_LEVEL=4.

The new subprocess regression reproduces SIGABRT for a NaN position in grid mode before the fix. The native assertion occurs in std::vector<int>::operator[]. Core dumps are disabled for the test launcher. The parent test survives and reports failure.

The fix checks all attribute values, explicit origins, and explicit ranges before the native call. Non-finite input returns argument error code 1. Ranges must also be positive. The regression covers nine invalid-input cases, including grid and bit-count positions, other attributes, origins, and ranges.

Validation commands, run from /mnt/data2/draco/review-fixes-20260906/source with CARGO_TARGET_DIR=/mnt/data2/draco/review-fixes-20260906/target:

```sh
cargo test
cargo test --release
cargo clippy --all-targets -- -D warnings
cargo test --release --test grid_corpus -- --ignored --nocapture
```

All commands exit 0. The 17 library tests and subprocess regression pass in debug and release. The native corpus check passes on 932 files, 930 nonempty primitives, and 10,849,344 vertices in 15.20 seconds. It checks lattice membership and decoded positions; it does not establish visual equivalence for textures or colors.

Raw logs and completion markers remain outside Git on .212 under /mnt/data2/draco/review-fixes-20260906/evidence/invalid-*. Corpus files remain under /mnt/data2/gridcorpus/dump/*.tfgd.

This change fixes non-finite input. Finite grid arithmetic limits, subnormal helper behavior, native exception containment, and build invalidation remain separate review findings.
