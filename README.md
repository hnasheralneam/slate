# Slate

A TUI markdown note editor, built with a little bit of vibe coding. Wouldn't recommend using right now, it's a bit janky, but I'll actually fix the code later.

Built using the [Ratatui](https://github.com/ratatui-org/ratatui) TUI library.

## Features

| Feature | Shortcut |
|---|---|
| Toggle sidebar | `Ctrl+B` |
| Focus sidebar / file tree | `Ctrl+E` |
| Search within current file | `Ctrl+F` |
| Open file by name (fuzzy) | `Ctrl+P` |
| Search across all files | `Ctrl+G` |
| Quit | `Ctrl+Q` |
| Scroll up/down | `j` / `k` or arrow keys |
| Page up/down | `PgUp` / `PgDn` |
| Jump to top/bottom | `g` / `G` |
| Next/prev search match | `n` / `N` |

### File Tree (Sidebar)
- `j`/`k` or `↑`/`↓` — navigate
- `Enter` or `l` — open file / expand folder
- `h` or `←` — collapse folder / go to parent
- `Space` — toggle expand/collapse
- `Esc` — return to editor.

## Install

### Build & Install

```bash
# Clone or extract the project
cd slate

# Build (release)
cargo build --release

# Install to ~/.local/bin (or /usr/local/bin)
cp target/release/slate ~/.local/bin/
# Make sure ~/.local/bin is on your PATH
```

### Run

```bash
# Open current directory as vault
slate

# Open a specific directory as vault
slate ~/notes
slate ~/Documents/obsidian-vault
```

## Tips

- Run `late ~/your-notes-folder` to point it at any directory.
- The sidebar works just like VS Code's file explorer — expand folders with `Enter` or `Space`, collapse with `h`.
- Global search (`Ctrl+G`) searches all `.md`, `.txt`, and `.sh` files in the vault. Matches show filename + line number + context with the match highlighted.
- After closing the in-file search bar with `Enter`, use `n`/`N` to jump between matches.

Liscense is GPLv3
