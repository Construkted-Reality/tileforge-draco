# Changelog

## [Unreleased]

## [0.2.0] - 2026-10-01

### Added
- Add `Quantization::Lossless` to compress geometry without position quantization. Other attributes retain their requested quantization.
- Test oriented triangles, winding, repeated triangles, wide coordinates, unquantized texture coordinates, and invalid inputs.

### Changed
- Extend the public quantization enum. Callers with exhaustive matches must handle the lossless variant.

Grid arithmetic and default 14-bit position quantization remain unchanged. Zero-bit position quantization remains invalid.
