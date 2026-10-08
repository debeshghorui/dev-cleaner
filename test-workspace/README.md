# Sample project for devclean

Fake monorepo. Nothing here is a real dependency install.

From the devclean repo:

```bash
source "$HOME/.cargo/env"
cargo run -- test-workspace
```

Or, after a release build:

```bash
./target/release/devclean test-workspace
```

Press `q` to leave without deleting anything. Trash only happens after you select rows and confirm.

## Rows you should see

| Path | Kind |
| --- | --- |
| `web/node_modules` | js |
| `web/.next` | js |
| `web/.turbo` | js |
| `api/target` | rust |
| `scripts/pkg/__pycache__` | python |
| `scripts/.pytest_cache` | python |

About 100 KB total. `web/node_modules` is the large one.

## Rows you should not see

- `web/node_modules/nested-dep/node_modules` — inside a cache, so it is not listed again
- `.git/node_modules` — `.git` is not entered
- `node_modules` at the root of this folder — that one is a symlink
- `web/src`, `api/src`, `scripts/pkg/app.py` — source
- `web/dist` — not a known cache name
- `scripts/.venv` — virtualenvs are left alone
- `NOTES.txt`
