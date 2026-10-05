# Changelog

## [Unreleased]

## [0.2.0] - 2026-10-01

### Added
- Add `Quantization::Lossless` to compress geometry without position quantization. Other attributes retain their requested quantization.
- Test oriented triangles, winding, repeated triangles, wide coordinates, unquantized texture coordinates, and invalid inputs.

### Fixed
These fixes landed after 0.1.0 without a version change. Each one is recorded in `docs/reviews/`.
- `snap_positions` uses binary64 intermediate arithmetic, preserves aligned values, and validates before it changes the slice (DRACO-02).
- `encode` rejects non-finite attribute values, origins, and ranges with argument error code 1 (DRACO-01).
- The wrapper rejects grid spacings and coordinate spans outside the native arithmetic domain, and `power_of_two_at_most` handles subnormal targets (DRACO-03, DRACO-05).
- The native wrapper catches exceptions from encode and decode, and edits to the vendored source invalidate the Cargo build (DRACO-04, DRACO-06).

### Changed
- Extend the public quantization enum. Callers with exhaustive matches must handle the lossless variant.

Grid arithmetic and default 14-bit position quantization remain unchanged. Zero-bit position quantization remains invalid.

## [0.1.0] - 2026-08-25

First standalone revision (`991ef55`). The crate started on 2026-08-21 inside `tileforge-mesh` as `crates/tileforge-draco` and moved here with its history.

### Added
- `encode` and `decode` over Google C++ Draco meshes, with a list of attributes and their unique ids.
- `Quantization::Grid`, which takes a lattice spacing, and `Quantization::Bits`.
- `snap_positions` and `power_of_two_at_most`.
- The `glbpos` example, which prints the decoded positions of a Draco GLB.
- The `third_party/draco` submodule with the `Options::SetFloat` precision patch.
