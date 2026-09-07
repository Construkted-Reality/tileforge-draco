# Grid snapping regression fix

Date: 2026-09-06. Finding: DRACO-02 from the family review.

The old binary32 half-step addition changes an aligned 8,388,609 to 8,388,610 at spacing 1. Dividing f32::MAX by 0.5 also overflows even though its snapped result is representable. New regressions reproduce both classes on .212. The replacement uses binary64 intermediate arithmetic and validates the complete result before mutating the slice. Halfway values still round toward positive infinity. Non-finite input and genuinely unrepresentable results produce argument errors without partial mutation.

All compilation and testing occurs on 192.168.8.212 under /mnt/data2/draco/review-fixes-20260906. Cargo and CMake use at most four build workers. The default tests pass after the fix. A corpus gate now reads the staged TFGD mesh dumps, checks displacement and idempotence, then checks native Draco roundtrip positions against the snapped input lattice. Corpus verification and final Clippy pass. The release all-target suite also passes.

Scope: the public snapping helper only. Non-finite values passed directly to encode, subnormal spacing policy, and native exception containment remain separate findings. Neither consumer dependency pin changes in this branch. After the library PR merges, both consumers need coordinated pin updates and seam checks; mesh also has a copied snapping helper to remove.

## Final evidence

The native corpus gate passes across 932 TFGD files, 930 nonempty primitives, and 10,849,344 input vertices in 189.98 seconds. Empty geometry is excluded from encoding. At spacing 1/256, it checks the maximum snapping displacement, preservation of aligned input, idempotence, and exact membership of decoded positions in the snapped input lattice. The gate exercises position-only native encoding; it does not establish UV, color, or renderer fidelity.

All 17 default library tests pass in both debug and release. The corpus gate passes, and Clippy passes with warnings treated as errors. Logs are preserved in `docs/reviews/evidence/2026-09-06-grid-snapping/`. All execution took place on .212; no local compilation occurred.
