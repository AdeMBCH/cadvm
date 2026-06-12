# Interactive dashboard (TUI)

```bash
cadvm ui
```

`cadvm ui` opens a full-screen terminal dashboard (built with
[ratatui](https://ratatui.rs)) — a source-control panel for your CAD history.

## Layout

```text
 ◆ cadvm  /path/to/repo                                  ⎇ main  ● clean
╭ commits ───────────────────────────╮╭ details ──────────────────────╮
│▌● a81e2421  HEAD  Bloc v2  · Mat …  ││ commit a81e2421…              │
│ ● c9cb67eb  ⎇ main  Bloc v1 · Mat … ││ author: Mat <…>               │
│                                     ││ date: 2026-06-12 …            │
│                                     ││ Files (1)                     │
│                                     ││   ▪ piece.step                │
│                                     ││       70134 B · 1794 lines …  │
╰─────────────────────────────────────╯╰───────────────────────────────╯
 ↑↓ move  m anchor  d diff  g geom  v view  b branch  s status  ? help  q quit
```

- **Left** — the commit list: graph marker (`●`, green on HEAD), short hash,
  `HEAD` badge, branch chips (`⎇ name`), message, author and relative time.
- **Right** — details of the selected commit: hash, author, date, parents,
  message, and each file with its STEP metadata.

## Keys

| Key | Action |
|-----|--------|
| `↑`/`k`, `↓`/`j` | move the selection |
| `m` | set/clear the **anchor** (the diff base) |
| `d` | metadata diff |
| `g` | geometric diff (volumes + faces) |
| `v` | build & open the 3D viewer |
| `b` | switch branch |
| `s` | working-tree status |
| `r` | reload |
| `?` | help · `q`/`Esc` quit or close a modal |

## How diffs choose their two sides

`d`, `g` and `v` compare **anchor → selected**. If no anchor is set, they compare
the selected commit's **parent → selected** (i.e. "what this commit changed").

Set an anchor with `m` on one commit, move to another, then press `d`/`g`/`v` to
compare across an arbitrary range.

## Notes

- `g` and `v` need the [`cadvm-geom` helper](geometry.md) (`CADVM_GEOM_BIN`); if
  it is missing you get a red toast, not a crash.
- The TUI calls the engine directly — it is the same binary as the CLI.
