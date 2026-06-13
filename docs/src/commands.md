# Command reference

Run `cadvm <command> --help` for full flags. All commands operate on the
repository discovered by walking up from the current directory.

| Command | Description |
|---------|-------------|
| `cadvm init` | Create a `.cadvm/` repository in the current directory. |
| `cadvm snapshot -m "msg"` | Record a snapshot (commit) of all tracked CAD files (STEP/STP/STL/OBJ). |
| `cadvm status` | Show new / modified / deleted files vs. HEAD. |
| `cadvm log` | Show the commit history of HEAD. |
| `cadvm show [<rev>]` | Show one commit's details and per-file metadata. |
| `cadvm diff [<a> <b>]` | Metadata diff (default `HEAD~1..HEAD`). |
| `cadvm checkout <rev> [-- <file>…]` | Restore the working tree (or named files) to a revision. |
| `cadvm branch` | List branches. |
| `cadvm branch <name>` | Create a branch at HEAD. |
| `cadvm branch -d <name>` | Delete a branch. |
| `cadvm switch <name>` | Switch branches, restoring their files. |
| `cadvm revert <rev>` | Create a commit that restores HEAD's parent state. |
| `cadvm gc [--dry-run \| --prune]` | Report / delete unreferenced objects. |
| `cadvm geom-diff <a> <b>` | Geometric diff of modified files (STEP via OCCT; STL/OBJ pure Rust). |
| `cadvm view <a> <b>` | Generate a standalone 3D HTML viewer of the diff. |
| `cadvm ui` | Interactive full-screen terminal dashboard. |
| `cadvm config [<key>] [<value>]` | Get / set / list config (e.g. `user.name`). |
| `cadvm completions <shell>` | Print a shell completion script. |

## Common flags

- `--force` — on `checkout` / `switch` / `revert`: proceed even when it would
  overwrite locally modified files or a dirty tree. **cadvm never overwrites your
  work without it.**
- `-- <files>` — on `checkout` / `geom-diff` / `view`: restrict to specific files.

## Safety rules

- `switch` and `revert` refuse a dirty working tree without `--force`.
- `checkout` refuses to overwrite a locally modified file without `--force`.
- Untracked files are never deleted.
- `gc` only deletes with `--prune`; `gc` alone is a dry run.

See [Typical workflow](workflow.md) for examples and the revision syntax.
