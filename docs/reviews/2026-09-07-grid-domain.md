# Grid arithmetic domain

DRACO-03 and DRACO-05. This change follows the finite-input fix b1dacf8.

The subnormal regression fails before the fix: is_power_of_two rejects a positive subnormal power, and power_of_two_at_most can return zero. The mathematical helpers now recognize those values. Native grid encoding and snapping explicitly require a normal power-of-two spacing.

Source inspection establishes the native domain: expert_encode.cc divides coordinates by spacing as floats, converts bounds to signed integers, subtracts them, and uses signed shifts for the bit count. The wrapper now checks the same float-rounded bounds before native conversion. Signed indices must fit i32; the inclusive span must fit 2^30 values; the resulting range must be finite. This does not change the native codec or recenter vertices.

All builds and tests run on 192.168.8.212 with Rust 1.94.1 and four workers. From /mnt/data2/draco/review-fixes-20260906/source, with CARGO_TARGET_DIR=/mnt/data2/draco/review-fixes-20260906/target:

```sh
cargo test
cargo test --release
cargo clippy --all-targets -- -D warnings
cargo test --release --test grid_corpus -- --ignored --nocapture
```

All commands pass. The two new tests cover all 23 subnormal powers and unsafe positive, negative, span, and overflow grid cases. Existing finite-input subprocess regressions and seam tests pass. The 932-file corpus gate passes on 10,849,344 vertices in 15.27 seconds. Corpus inputs: /mnt/data2/gridcorpus/dump/*.tfgd. Raw logs stay outside Git under /mnt/data2/draco/review-fixes-20260906/evidence/domain-*.

The arithmetic guard is based on the native source, not a sanitizer execution. Exception containment and native build invalidation remain separate findings. Consumers must select a supported grid; this PR does not introduce automatic spacing changes.
