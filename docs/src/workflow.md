# Typical workflow

## Branching to explore a variant

```bash
cadvm branch second-hole      # create a branch at HEAD
cadvm switch second-hole      # move onto it (refuses a dirty tree without --force)

# …edit the part, then…
cadvm snapshot -m "Added a second hole"

cadvm switch main             # back to the trunk
cadvm branch                  # list branches; * marks the current one
cadvm branch -d second-hole   # delete a branch (not the current one)
```

`switch` restores the files of the target branch and never silently discards
local changes: it refuses when the working tree is dirty unless you pass
`--force`.

## Undoing the last commit

```bash
cadvm revert HEAD
```

This creates a **new** commit that restores the state of HEAD's parent (it does
not rewrite history). V1 supports reverting HEAD only.

## Restoring files without moving the branch

```bash
cadvm checkout HEAD~2                 # restore the whole tree to that revision
cadvm checkout HEAD~2 -- piece.step   # restore a single file
cadvm checkout HEAD~2 --force         # discard local modifications
```

`checkout` is **restore-like**: it changes files on disk but does **not** move
HEAD or the current branch. It refuses to overwrite locally modified files
without `--force`, and never deletes untracked files.

## Revisions you can name

Anywhere a revision is expected:

- `HEAD`, `HEAD~1`, `HEAD~2`, … (and `HEAD^`)
- a branch name
- a full 64-char hash (with or without the `blake3:` prefix)
- an unambiguous short-hash prefix

## Reclaiming space

```bash
cadvm gc            # report unreferenced objects (safe, no deletion)
cadvm gc --prune    # actually delete them
```

`gc --prune` also reclaims legacy raw blobs from the old V1 storage scheme (see
[Storage model](storage.md)).

Next: [Interactive dashboard](tui.md).
