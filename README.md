# tileforge-draco

A Rust entry point onto Google's C++ Draco. It exists so that two TileForge
sub-repositories encode geometry onto the **same** lattice.

## Why this crate is shared

`docs/cross-cutting-decisions.md` in the umbrella repository forbids a shared
Rust crate between the TileForge sub-repositories. The stated reason is that
"shared" rarely means the same thing in every caller.

This crate is the exception, and the reason for the rule is the reason for the
exception. `tileforge-mesh` produces tilesets. `tileforge-optimize` recompresses
tilesets that other people produced. Both must put a shared vertex on the same
lattice point, or a crack opens in the render. Identical arithmetic in both
callers is the whole purpose. A copy in each repository would mean two foreign
function interface wrappers, two Draco submodules, and two copies of the
`Options::SetFloat` patch. Drift between the copies breaks seams silently, and
no test in either repository would notice.

## What it gives you

- `encode` and `decode` over Draco meshes.
- `Quantization::Grid`, which takes a lattice **spacing** instead of a bit
  count, so every tile shares one lattice anchored at zero.
- `snap_positions`, which puts vertices on the lattice before the encode.
- `power_of_two_at_most`, the rounding rule the two callers share.

Read the crate documentation in `src/lib.rs` for the two rules that the
measurement produced. Both are load bearing.

## Build requirements

The build compiles Google Draco from source. A host needs:

1. `cmake` 3.22 or later.
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

## How the callers depend on it

Both callers pin a git revision. Neither uses a version range.

    tileforge-draco = { git = "https://github.com/Construkted-Reality/tileforge-draco.git", rev = "<sha>" }

To move both callers onto a new revision:

1. Merge the change here and note the new commit SHA.
2. Update the `rev` in `tileforge-mesh/Cargo.toml`.
3. Update the `rev` in `tileforge-optimize/Cargo.toml`.
4. Run the seam test in each caller before you merge either one.

Do not update one caller without the other. A lattice difference between them
is exactly the failure this crate prevents.

## History

The crate started inside `tileforge-mesh` as `crates/tileforge-draco`. Its
history moved here commit by commit, so `git log` and `git blame` still work.
