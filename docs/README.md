# tileforge-draco documentation

Each document starts with a **Status** line and a **Summary** line.
Removed documents are listed in [REMOVED.md](REMOVED.md) with the commit that still contains them.
The four 2026-09 review records each name their family-review finding (DRACO-01 to DRACO-06). The 2026-10-01 record documents the 0.2.0 lossless feature. All validation ran on build host .212.

| Document | Status | Read when |
|---|---|---|
| [reviews/2026-10-01-lossless-positions.md](reviews/2026-10-01-lossless-positions.md) | Current | You change lossless mode or verify geometry preservation. |
| [reviews/2026-09-07-native-boundary.md](reviews/2026-09-07-native-boundary.md) | Current | You touch the C++ wrapper, exception handling, or `build.rs` rebuild tracking. |
| [reviews/2026-09-07-grid-domain.md](reviews/2026-09-07-grid-domain.md) | Historical | You change grid spacing rules or the pre-encode range checks. |
| [reviews/2026-09-07-finite-encode.md](reviews/2026-09-07-finite-encode.md) | Historical | You change input validation in `encode`. |
| [reviews/2026-09-06-grid-snapping.md](reviews/2026-09-06-grid-snapping.md) | Historical | You change `snap_positions` or the corpus gate. |
| [REMOVED.md](REMOVED.md) | Current | You need a file that was deleted as stale. |
