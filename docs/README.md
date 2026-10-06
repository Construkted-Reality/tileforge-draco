# tileforge-draco documentation

> **Status:** Current.
> **Summary:** Map of the tileforge-draco documents, the open items, the consumer pins, and the locations of decisions that source comments cite.

Each document starts with a **Status** line and a **Summary** line. Status is Current
for shipped behavior or an active rule, Proposed for an unimplemented design, and
Historical for dated evidence.

Markdown files longer than 100 lines, excluding `CHANGELOG.md`, have a generated
line-numbered contents block near the top. At present, `README.md` and
`reviews/2026-10-05-open-items.md` have one.

Removed documents are listed in [REMOVED.md](REMOVED.md) with the commit that still
contains them. After you edit a long document, regenerate the contents blocks with
`python3 <tileforge umbrella>/scripts/docs/doc-index.py index .`, and check them and
the relative links with `python3 <tileforge umbrella>/scripts/docs/doc-index.py check .`.

Family conventions (build hosts, pinning rule, review process) live in the umbrella
repository `tileforge-workspace`, under `docs/`. This repository documents only the
behavior of this crate. The API reference is the rustdoc in `src/lib.rs`.

The four 2026-09 review records each name their family-review finding (DRACO-01 to
DRACO-06). The 2026-10-01 record documents the 0.2.0 lossless feature. The 2026-10-05
record documents the 0.2.1 open-item fixes. All validation ran on build host .212.

| Document | Status | Read when |
|---|---|---|
| [reviews/2026-10-05-open-items.md](reviews/2026-10-05-open-items.md) | Current | You touch allocation failure, the allocation probe, sanitizer runs, the grid-domain guard, the corpus gate, or the link order in `build.rs`. |
| [reviews/2026-10-01-lossless-positions.md](reviews/2026-10-01-lossless-positions.md) | Current | You change lossless mode or verify geometry preservation. |
| [reviews/2026-09-07-native-boundary.md](reviews/2026-09-07-native-boundary.md) | Historical | You touch the C++ wrapper, exception handling, or `build.rs` rebuild tracking. |
| [reviews/2026-09-07-grid-domain.md](reviews/2026-09-07-grid-domain.md) | Historical | You change grid spacing rules or the pre-encode range checks. |
| [reviews/2026-09-07-finite-encode.md](reviews/2026-09-07-finite-encode.md) | Historical | You change input validation in `encode`. |
| [reviews/2026-09-06-grid-snapping.md](reviews/2026-09-06-grid-snapping.md) | Historical | You change `snap_positions` or the corpus gate. |
| [REMOVED.md](REMOVED.md) | Current | You need a file that was deleted as stale. |

## Open items

- The submodule pin 673ee67 is on the fork branch `fix/rans-bit-encoder-destructor`, which exists only in a local clone. Push that branch to `Construkted-Reality/draco` before a consumer moves its pin. See [reviews/2026-10-05-open-items.md](reviews/2026-10-05-open-items.md).
- The allocation probe covers four small meshes. It does not prove recovery from memory exhaustion for every input.
- Sanitizer runs instrument the C++ code only. Stable Rust cannot instrument the Rust code.
- The corpus gate checks positions and texture coordinates. The TFGD dumps hold no normals or colors, so the gate does not check them. Renderer fidelity is out of scope.

## Raw evidence

Raw logs stay outside Git on build host .212. These directories existed on 2026-10-05:

- `/mnt/data2/draco/review-fixes-20260906/evidence/` (the four 2026-09 records).
- `/mnt/data2/release-fixes-20261001/evidence/` (the lossless record; shared with other repositories).
- `/mnt/data2/draco/open-items-20261005/evidence/` (the 2026-10-05 record).
- `/mnt/data2/gridcorpus/dump/` (932 TFGD corpus files).

## Consumer pins

Consumers pin this crate by Git revision in `tileforge-mesh/Cargo.toml` and
`tileforge-optimize/Cargo.toml`. Read those files for the current pins.

On 2026-10-05, both consumers pin `f25b881` on their `origin/main`. That revision has
all 0.2.0 code. Version 0.2.1 moves the Draco pin and changes the link order in
`build.rs`. Encoded output does not change. A consumer gets 0.2.1 only when it moves
its pin.

## Cross-repository references

Source comments cite decisions that live in `tileforge-mesh`:

- `ADR-048`: `tileforge-mesh/docs/design/adr/048-google-draco-grid-quantization.md`.
- The grid validation measurement: `tileforge-mesh/docs/design/investigations/2026-08-21-draco-cpp-grid-validation.md`.
  `csrc/tileforge_draco.cc` and `src/lib.rs` cite this path and name the repository.
