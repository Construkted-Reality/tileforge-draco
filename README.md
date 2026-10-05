# tileforge-draco

A Rust entry point onto Google's C++ Draco. It exists so that two TileForge
sub-repositories encode geometry onto the **same** lattice.

## Why this crate is shared

`docs/cross-cutting-decisions.md` in the umbrella repository forbids a shared
Rust crate between the TileForge sub-repositories, with two named exceptions:
this crate and `tileforge-crs`. The stated reason for the rule is that "shared"
rarely means the same thing in every caller.

The reason for the rule is also the reason for this exception. `tileforge-mesh`
produces tilesets. `tileforge-optimize` recompresses tilesets that other people
produced. Both must put a shared vertex on the same lattice point, or a crack
opens in the render. Identical arithmetic in both
callers is the whole purpose. A copy in each repository would mean two foreign
function interface wrappers, two Draco submodules, and two copies of the
`Options::SetFloat` patch. Drift between the copies breaks seams silently, and
no test in either repository would notice.

## What it gives you

- `encode` and `decode` over Draco meshes.
- `Quantization::Grid`, which takes a lattice **spacing** instead of a bit
  count, so every tile shares one lattice anchored at zero.
- `Quantization::Lossless`, which preserves finite position values without quantization.
- `snap_positions`, which puts vertices on the lattice before the encode.
- `power_of_two_at_most`, the rounding rule the two callers share.
- `examples/glbpos.rs`, which prints the decoded positions of a Draco GLB for the seam oracle.

`snap_positions` preserves already aligned coordinates and rounds halfway values toward positive infinity. It rejects non-finite input or output before changing any position in the slice.

`encode` rejects non-finite attribute values and explicit origins before entering the native codec. Explicit ranges must be finite and positive. Invalid numeric input returns argument error code 1.

`power_of_two_at_most` handles positive subnormal targets without returning zero. Native grid encoding and snapping require a normal power-of-two spacing. The encoder rejects grid indices outside signed 32-bit bounds, spans requiring more than 30 bits, and non-finite quantization ranges. Choose a supported spacing or coordinate frame before encoding.

Read the crate documentation in `src/lib.rs` for the two rules that the
measurement produced. Both are load bearing.

Lossless positions retain the source coordinates while Draco compresses mesh topology and attributes. Draco can reorder vertices, split vertices, and reorder faces. Callers must compare oriented triangle values when they need to verify geometry preservation. Unquantized attributes retain their finite values; other attributes keep their requested quantization. Signed zero can normalize to positive zero.

Lossless mode does not require a grid-domain check or position snapping. It still rejects non-finite values, invalid indices, and invalid explicit attribute ranges. `Quantization::Bits { bits: 0 }` remains invalid. The default remains 14-bit position quantization. Grid encoding retains its existing arithmetic and seam contract.

## Build requirements

The build compiles Google Draco from source. A host needs:

1. `cmake` 3.22 or later. Draco's own `CMakeLists.txt` declares 3.12, but the family build hosts are only tested with 3.22 or later.
2. A C++17 compiler.
3. The submodules. Run `git submodule update --init --recursive`.

`third_party/draco` is a submodule of `Construkted-Reality/draco`, branch
`fix/options-float-precision`. That fork carries one patch against
`google/draco` 1.5.7. Read `third_party/draco/CONSTRUKTED-CHANGES.md` before you
move the pin. The patch makes `Options::SetFloat` keep full precision, which the
grid spacing needs.

`DRACO_TRANSCODER_SUPPORTED` is not optional. Without it, Draco compiles out
`ExpertEncoder::SetAttributeGridQuantization`, which is the reason this crate
exists.

The native wrapper catches exceptions from encode and decode and reports an internal error. This does not guarantee recovery from memory exhaustion: the upstream codec can terminate if allocation fails again during destructor cleanup. Edits to the vendored native source invalidate the Cargo build.

## Testing

```sh
cargo test
cargo test --release
cargo clippy --all-targets -- -D warnings
cargo test --release --test grid_corpus -- --ignored --nocapture
```

Compile and test on build host .212, not on a desktop. The corpus gate in `tests/grid_corpus.rs` is ignored by default. It reads TFGD mesh dumps from `/mnt/data2/gridcorpus/dump/` on .212 (932 files). `tests/native_allocation.cc` is a standalone allocation-failure probe; [docs/reviews/2026-09-07-native-boundary.md](docs/reviews/2026-09-07-native-boundary.md) has its compile command.

## How the callers depend on it

Both callers pin a git revision. Neither uses a version range.

    tileforge-draco = { git = "https://github.com/Construkted-Reality/tileforge-draco.git", rev = "<sha>" }

To move both callers onto a new revision:

1. Merge the change here and note the new commit SHA.
2. Update the `rev` in `tileforge-mesh/Cargo.toml`.
3. Update the `rev` in `tileforge-optimize/Cargo.toml`.
4. Run the seam tests in each caller before you merge either one. In `tileforge-optimize` that is
   `tests/shared_grid.rs`. In `tileforge-mesh` it is the lattice tests in
   `crates/tileforge-glb/src/mesh_quantize.rs`.

When a revision changes grid arithmetic, update both callers together. A revision that adds a separate encoding mode can be adopted independently if the existing grid behavior remains unchanged and the seam tests pass.

## Documentation

Start at [docs/README.md](docs/README.md). Release history is in [CHANGELOG.md](CHANGELOG.md).

## History

The crate started inside `tileforge-mesh` as `crates/tileforge-draco`. Its
history moved here commit by commit, so `git log` and `git blame` still work.
