# Lossless position mode

> **Status:** Current. Shipped in 0.2.0.
> **Summary:** Why `Quantization::Lossless` exists for the optimizer and how geometry preservation is verified under codec reordering.

Date: 2026-10-01. Native validation host: 192.168.8.212.

The optimizer must preserve geometry positions when it cannot safely update enclosing volumes or placement. Grid and bit quantization can move those positions. Attempting quantized encoding and then discarding the result leaves such tiles uncompressed and repeats expensive work. An explicit lossless position mode permits compression without that movement.

`Quantization::Lossless` selects zero-bit position encoding inside the reference codec. This selection uses a separate native option. `Quantization::Bits { bits: 0 }` remains an argument error. The public enum gains a variant, so the crate version increases to 0.2.0. Exhaustive matches in callers must handle the variant.

The regression tests fail before the native mode exists. They pass after implementation in debug and release builds. They compare oriented triangles with exact position and unquantized texture values. Cyclic rotation permits codec reordering. Sorting retains duplicate faces. The fixtures include opposite winding, shared vertices, coordinates spanning 2e20, and both encoder speed endpoints. Additional tests check quantized texture attributes and invalid numeric values and indices.

The complete debug and release suites and strict Clippy checks pass. The existing native grid corpus passes: 932 files, 930 nonempty primitives, and 10,849,344 source vertices. Grid arithmetic and default position settings remain unchanged. Lossless mode avoids position snapping and grid-domain restrictions. It does not remove finite-value or index validation.

Raw logs remain at `/mnt/data2/release-fixes-20261001/evidence/`. Full optimizer corpus validation and independent decoder checks are separate release gates. The native allocation probe is compiled with the extended options structure. Allocation exhaustion can still terminate inside the upstream codec, as documented by the existing native-boundary investigation.
