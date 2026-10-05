# Removed documents

> **Status:** Current. Append a row for every file that you delete.
> **Summary:** Files deleted as stale or redundant, with the reason and the commit that still contains each one.

Git history keeps every removed file. To read one, run `git show <commit>:<path>`.
The commit column is the last commit that contained the file.

| Path | Commit | Reason |
|---|---|---|
| `docs/reviews/evidence/2026-09-06-grid-snapping/` (6 `.log` and 6 `.done` files) | `a49d6b5b7293` | Raw build and test logs. The family rule keeps raw evidence outside Git; the review record summarizes the results. |
