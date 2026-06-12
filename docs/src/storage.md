# Storage model

A repository lives entirely in `.cadvm/`:

```text
.cadvm/
├── objects/
│   ├── chunks/       # fixed 256 KiB content chunks (file storage)
│   ├── blobs/        # whole-file blobs (optional; cleaned by gc)
│   ├── manifests/    # serialized snapshots
│   └── commits/      # serialized commits
├── refs/heads/<branch>   # each file holds the branch's tip commit id
├── HEAD                  # "ref: refs/heads/main" or a detached commit id
├── config.json           # user.name / user.email and other settings
├── index.json            # reserved for a future staging area
└── tmp/                  # scratch space for atomic writes
```

## Content addressing

Every object is identified by the **BLAKE3 hash** of its content
(`blake3:<hex>`), sharded by the first two hex byte-pairs:

```text
.cadvm/objects/chunks/ab/cd/<full-hex>
```

Writes are atomic (temp file + rename), so a crash can never leave a corrupt
object. Writing identical content twice is automatically deduplicated.

## Deduplication

1. **Content identity** — `raw_hash` is the BLAKE3 hash of the whole file.
   Identical files share an identity, so status/diff comparisons are exact.
2. **Fixed-size chunking** — files are split into 256 KiB chunks, each stored
   content-addressed, so identical chunks are shared across files and versions.

### Chunk-only storage

File content is stored **as chunks**; the whole file is not duplicated as a
standalone blob, so there is no on-disk redundancy. `checkout` reconstructs each
file by concatenating its chunks, and `cadvm gc --prune` removes any
unreferenced objects.

## What is tracked

cadvm tracks **`.step` and `.stp`** files only, recursively from the repository
root. Skipped: the `.cadvm/` directory, hidden directories, and anything matching
`.cadvmignore`.

A `.cadvmignore` at the root uses a small pattern syntax:

```text
# comments and blank lines are ignored
*.bak            # glob on the file name (* and ? supported)
build/           # a directory and everything beneath it
/secret/old.step # leading "/" anchors to the repo root
```

## STEP metadata

cadvm does **not** parse geometry for the VCS. It scans STEP text to record:
line count, `HEADER;`/`DATA;` sections, `FILE_SCHEMA`, the entity count, and the
top 20 entity types. This feeds `show`, `log` and the metadata `diff`.
