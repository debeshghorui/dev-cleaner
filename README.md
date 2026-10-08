# devclean

Terminal app that walks a project tree, finds regenerable developer caches, and moves the ones you confirm into the system Trash. It does not permanently delete anything.

One Rust binary covers macOS, Linux, and Windows. Path checks, the directory walk, and Trash all stay in Rust — the program does not shell out to `find`, `rm`, or PowerShell.

```text
devclean                 # scan the current directory
devclean /path/to/code   # scan that project
```

The table is a dry run. Nothing is touched until you select rows, press Enter, and confirm.

## Keys

| Key | Action |
| --- | --- |
| ↑ / ↓ or j / k | Move |
| space | Toggle the highlighted folder |
| a | Select or clear every visible folder |
| / | Filter by name, kind, or path |
| w | Show or hide scan warnings |
| enter | Move the selection to Trash (asks first) |
| q | Quit |

In the confirm dialog, `y` moves the folders and `n` goes back. Restore them from Trash (Finder Trash, Windows Recycle Bin, or the desktop trash on Linux). Every path that was moved is appended to a log:

| OS | Log |
| --- | --- |
| macOS | `~/Library/Logs/devclean/moves.log` |
| Linux | `~/.local/share/devclean/moves.log` |
| Windows | `%LOCALAPPDATA%\devclean\moves.log` |

## What it finds

Directory name only. On macOS and Windows the match is case-insensitive. The walk does not follow symlinks, does not enter `.git`, and does not descend into a hit (a `node_modules` nested inside another `node_modules` is not listed separately).

| Kind | Directories |
| --- | --- |
| JavaScript / TypeScript | `node_modules`, `.next`, `.nuxt`, `.output`, `.turbo`, `.parcel-cache`, `.vite`, `.vercel` |
| Rust | `target` |
| Python | `__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`, `.tox` |
| Other | `.gradle`, `.dart_tool`, `coverage`, `.nyc_output` |

`dist`, `build`, virtualenvs, and `.cache` are left alone. Those names often hold real source or environments you cannot recreate from a lockfile.

## Safety

- Trash only. There is no permanent-delete flag.
- The scan root itself is never trashed. Pointing devclean at a `node_modules` folder is refused.
- The filesystem root is refused (`/`, `C:\`).
- These directories are never entered:
  - macOS: `/System`, `/Library`, `/usr`, `/bin`, `/sbin`, `/private`, `/Applications`, `/Volumes` (a project on a mounted disk, `/Volumes/<name>/<project>`, is still allowed)
  - Linux: `/proc`, `/sys`, `/dev`, `/usr`, `/bin`, `/sbin`, `/boot`, `/run`
  - Windows: `C:\Windows`, `C:\Program Files`, `C:\Program Files (x86)`
- Unreadable directories are skipped and listed as warnings.
- Before each move, the path is checked again: it must be a real directory whose name is in the list above, and it must not be a symlink or a system path.

## Build

Install Rust from [https://rustup.rs](https://rustup.rs), then:

```bash
cargo build --release
```

The binary is `target/release/devclean` (`devclean.exe` on Windows). Build on the OS you will run it on. The `trash` crate calls that platform's Trash API, so a macOS binary will not run on Windows.

```bash
cargo test
```
