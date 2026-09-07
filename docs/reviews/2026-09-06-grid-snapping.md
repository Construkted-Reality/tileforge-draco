# Grid snapping regression fix

Date: 2026-09-06. Finding: DRACO-02 from the family review.

The old binary32 half-step addition changes an aligned 8,388,609 to 8,388,610 at spacing 1. Dividing f32::MAX by 0.5 also overflows even though its snapped result is representable. New regressions reproduce both classes on .212. The replacement uses binary64 intermediate arithmetic and validates the complete result before mutating the slice. Halfway values still round toward positive infinity. Non-finite input and genuinely unrepresentable results produce argument errors without partial mutation.

All compilation and testing occurs on 192.168.8.212 under /mnt/data2/draco/review-fixes-20260906. Cargo and CMake use at most four build workers. The default tests pass after the fix. A corpus gate now reads the staged TFGD mesh dumps, checks displacement and idempotence, then checks native Draco roundtrip positions against the snapped input lattice. Corpus verification and final Clippy are in progress; no PR is open yet.

Scope: the public snapping helper only. Non-finite values passed directly to encode, subnormal spacing policy, and native exception containment remain separate findings. Neither consumer dependency pin changes in this branch. After the library PR merges, both consumers need coordinated pin updates and seam checks; mesh also has a copied snapping helper to remove.
