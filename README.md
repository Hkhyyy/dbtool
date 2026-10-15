# dbtool

A DataGrip-style database client. One engine (`dbtool-core`), three ways to use it:

| Binary | What it is |
|---|---|
| **dbtool** (GUI, Tauri) | Desktop app modeled on DataGrip |
| **dbtool-tui** | Terminal UI (great over SSH) — works on its own, no GUI needed |
| **dbtool-mcp** | MCP server so AI tools such as Claude Code can explore and query your databases |

All three share the same data sources and passwords, so a connection created in one shows up in the others.

- Config folder: `~/Library/Application Support/com.hyunkyo.dbtool` (macOS), `~/.config/com.hyunkyo.dbtool` (Linux)
- Passwords are stored in `secrets.json` in that folder, encrypted with AES-GCM (key in `secret.key`, mode 0600). The macOS Keychain is not used.

## Download

Get the latest build from [Releases](https://github.com/Hkhyyy/dbtool/releases):

- `dbtool_<version>_aarch64.dmg` — GUI app (Apple Silicon). Drag `dbtool.app` to Applications.
- `dbtool-cli_<version>_aarch64-apple-darwin.tar.gz` — `dbtool-tui` and `dbtool-mcp`.

The app is not code-signed yet, so macOS shows an "unidentified developer" / "damaged" warning on first launch. Run `xattr -dr com.apple.quarantine /Applications/dbtool.app` (or right-click the app → Open).

## Supported databases

| Database | Features |
|---|---|
| PostgreSQL | Explorer tree (databases, schemas, tables, views, sequences, routines), query console (server-side cursor paging, cancel, manual transactions), data editing, explain plan, DDL, full-text search |
| MySQL / MariaDB | Same as above. A MySQL database is its own schema, so tables appear directly under the database |
| Redis | Medis-style browser (key list with type and TTL, per-type value editor), redis-cli style terminal, **Standalone / Sentinel / Cluster** |

Connection options:
- **SSH tunnel** with a key or password. Host keys are checked against `~/.ssh/known_hosts`.
- **SSL/TLS**: Require / Verify CA / Full Verification, with optional CA and client certificate. Works for PostgreSQL, MySQL and Redis.
- **Options tab**: read-only, transaction control and isolation level, time zone, keep-alive, auto-disconnect when idle, single database mode, startup script.

> Redis Sentinel and Cluster nodes advertise their own addresses, so they can't be combined with an SSH tunnel.

## Build & run

```bash
# GUI (dev mode)
pnpm install
pnpm tauri dev

# GUI installer (.dmg)
pnpm tauri build --bundles dmg

# TUI / MCP
cargo build --release -p dbtool-tui -p dbtool-mcp
ln -sf "$PWD/target/release/dbtool-tui" ~/.local/bin/dbtool-tui
ln -sf "$PWD/target/release/dbtool-mcp" ~/.local/bin/dbtool-mcp

dbtool-tui            # press F1 for the keymap
dbtool-tui --help     # print the keymap
```

## Layout

```
crates/core   dbtool-core: drivers (PostgreSQL, MySQL, Redis), sessions, SSH tunnels, storage, AppState (every feature)
src-tauri     GUI backend: Tauri commands call AppState methods directly
src           GUI frontend (React + Monaco)
crates/tui    TUI (ratatui)
crates/mcp    MCP server (stdio JSON-RPC)
```

## GUI keyboard shortcuts

Inside the app the same list is searchable via **Help › Keymap Reference**, the ⌨ toolbar button, or `⇧⌘/`. **Find Action** (`⇧⌘A`) lists every command together with its shortcut.
On Windows/Linux read `⌘` as `Ctrl` and `⌥` as `Alt`.

<!-- keymap:gui:start -->

#### Search & Navigation

| macOS | Action |
|---|---|
| `⇧⇧` | Search Everywhere (All / Database / Files / Symbols / Actions, ⇥ switches tabs) |
| `⌘O` | Go to Database Object |
| `⌥⌘O` | Go to Symbol (columns) |
| `⇧⌘O` | Go to File (open and recent tabs) |
| `⇧⌘A` | Find Action |
| `⌥⇧⌘F` | Full-text Search in database |
| `⌘E` | Recent Files |
| `⌃⇥ / ⌃⇧⇥` | Switcher (hold ⌃, release to open) |
| `⇧⌘/` | Keymap Reference (this window) |

#### In search results (Search Everywhere / Go to)

| macOS | Action |
|---|---|
| `↩` | Select in Database Explorer |
| `⌘↓` | Edit data (open table) |
| `⌘B` | Go to DDL |
| `F1` | Quick documentation |

#### Tool windows & appearance

| macOS | Action |
|---|---|
| `⌘1` | Database Explorer (focus / hide) |
| `⌘3` | Find tool window (Full-text Search results) |
| `⇧⎋` | Hide active tool window |
| `⇧⌘F12` | Maximize editor (also: double-click a tab) |
| `⌘,` | Settings (theme) |
| `⌃`` | Quick switch theme (Dark / Light / High contrast / Sync with OS) |
| `⌃⇧. / ⌃⇧,` | Editor font size bigger / smaller |

#### Editor tabs

| macOS | Action |
|---|---|
| `⌘W` | Close tab (middle-click also closes) |
| `⌥ + click ×` | Close other tabs |
| `⇧⌘T` | Reopen closed tab |
| `⇧⌘] / ⇧⌘[` | Next / previous tab |
| `⌥⇥` | Go to the other split |
| `right-click tab` | Pin, Split Right/Down, Move to Opposite Group, Close others… |
| `drag tab` | Reorder, or drop on the right/bottom edge to split |

#### Database Explorer

| macOS | Action |
|---|---|
| `⌘↓ / F4` | Edit data (table) / open console (data source) |
| `⇧⌘L` | New query console |
| `⇧⌘F10` | Jump to query console |
| `⌘B` | Go to DDL |
| `⌥⌘G` | SQL Generator |
| `⌥⇧⌘G` | Copy DDL to clipboard |
| `⌥⇧⌘B` | Open DDL in console |
| `⌥⇧⌘C` | Copy reference |
| `F1` | Quick documentation |
| `⌘;` | Data source properties |
| `⌘N` | New data source |
| `⌘D` | Duplicate data source |
| `⌘R / ⌥⌘Y` | Refresh |
| `⌘F2` | Disconnect |
| `⌦` | Remove data source / drop object (asks first) |
| `type letters` | Speed search in the tree |

#### Query console

| macOS | Action |
|---|---|
| `⌘↩` | Execute (statement at caret, or the selection) |
| `⌘F2` | Cancel running statement |
| `⌥⌘E` | Query history |
| `⌃Space` | Code completion |
| `⌥⌘L` | Reformat code |
| `⌘/ · ⌥⌘/` | Line comment · block comment |
| `⌘D` | Duplicate line |
| `⌘⌫` | Delete line |
| `⌥⇧↑ / ⌥⇧↓` | Move line up / down |
| `⌥↑ / ⌥↓` | Extend / shrink selection |
| `⌃G · ⌃⌘G` | Select next occurrence · all occurrences |
| `⇧⌘U` | Toggle case |
| `⌘L` | Go to line |
| `⌘F` | Find in editor |

#### Data grid (results & table editor)

| macOS | Action |
|---|---|
| `↩ / F2` | Edit cell |
| `⇧↩` | Open value editor |
| `⌥⌘N` | Set NULL |
| `⌥⌘D` | Set DEFAULT |
| `⌘N` | Add row |
| `⌘D` | Clone row |
| `⌘⌫` | Delete rows |
| `⌥⌘Z` | Revert selected changes |
| `⌘↩` | Submit changes |
| `⌘C / ⌘V` | Copy (with chosen extractor) / paste |
| `⌘A` | Select all |
| `⌘F` | Find in grid |
| `⌘R` | Reload |
| `⌘F2` | Cancel |
| `click header · ⌥ click` | Sort · add to multi-column sort |

<!-- keymap:gui:end -->

## TUI keyboard shortcuts

Press **F1** inside `dbtool-tui`, or run `dbtool-tui --help`, to see the same list. The mouse works too: click to select, double-click to open or edit, scroll with the wheel.
Ctrl+Enter is only distinguishable in terminals that support the kitty keyboard protocol (iTerm2, kitty, WezTerm, Ghostty). Elsewhere use F5 or Alt+Enter.

<!-- keymap:tui:start -->

#### Global

| Key | Action |
|---|---|
| `Ctrl+Q` | Quit |
| `F1` | This keymap |
| `Ctrl+K` | Actions palette (run any command by name) |
| `Ctrl+O` | Go to Database Object (Ctrl+B in the list = DDL) |
| `Ctrl+E` | Recent tabs |
| `Ctrl+L` | New query console |
| `Ctrl+W` | Close tab |
| `Alt+← / Alt+→` | Previous / next tab (also Ctrl+PgUp / Ctrl+PgDn) |
| `Alt+1` | Database Explorer: focus / hide |
| `F6` | Cycle focus: explorer → editor → results |

#### Database Explorer

| Key | Action |
|---|---|
| `↑ ↓  (j k)` | Move |
| `→ / ←  (l h)` | Expand / collapse (← on a child jumps to its parent) |
| `Enter` | Connect · open table data · open Redis browser · show DDL of routines |
| `F4 / Ctrl+↓` | Open a query console for the data source / database |
| `Ctrl+B` | Go to DDL |
| `/` | Filter loaded nodes (Esc clears) |
| `n / e / D` | New / edit / remove data source |
| `c / X` | Connect / disconnect (also Ctrl+F2) |
| `r / F5` | Refresh |
| `y` | Copy name (OSC 52 clipboard) |
| `Tab / Esc` | Go to the open tab |

#### Query console

| Key | Action |
|---|---|
| `F5 / Ctrl+Enter / Alt+Enter` | Execute statement at caret (or the selection) |
| `Alt+E` | Explain plan |
| `Ctrl+C` | Cancel running statement |
| `Alt+T` | Transaction mode Auto / Manual |
| `Alt+C / Alt+R` | Commit / roll back |
| `Alt+O` | Toggle result grid / output log |
| `Shift+Tab` | Go to the result grid (Esc / Tab returns) |
| `Esc` | Go to the explorer |
| `Ctrl+U / Ctrl+R` | Undo / redo (editor) |

#### Data grid (results & table data)

| Key | Action |
|---|---|
| `arrows  (h j k l)` | Move |
| `PgUp / PgDn · g / G` | Page · first / last row (more rows load automatically) |
| `Home / End` | First / last column |
| `Enter / F2` | Edit cell (table data) |
| `n` | Set NULL |
| `a / Insert` | Add row |
| `d / Delete` | Mark row deleted (again = undo) |
| `Ctrl+S` | Submit changes (one transaction) |
| `Ctrl+Z` | Revert pending changes |
| `w / o` | Edit WHERE / ORDER BY |
| `r` | Reload |
| `x` | Export to CSV |
| `y` | Copy cell value |
| `Esc` | Back |

#### Redis browser (Medis style)

| Key | Action |
|---|---|
| `↑ ↓` | Select key |
| `Enter / → / Tab` | Go to the value |
| `/` | Key pattern (SCAN MATCH) |
| `m` | Scan more |
| `r` | Rescan |
| `a` | Add key (SET / HSET / RPUSH / SADD / ZADD …) |
| `D` | Delete key |
| `R` | Rename key |
| `t` | Set TTL (empty = persist) |
| `:` | Run any command |
| `e / Enter (value)` | Edit selected element / string |
| `a (value)` | Add element |
| `x (value)` | Delete element |
| `Esc` | Back to keys |

#### Data source form

| Key | Action |
|---|---|
| `↑ ↓ / Tab` | Move between fields |
| `type / Backspace` | Edit text fields |
| `Space / ← →` | Toggle on/off · change choice (Type, SSL mode, Redis mode…) |
| `Ctrl+T` | Test connection |
| `Enter / Ctrl+S` | Save |
| `Esc` | Cancel |

#### Popups

| Key | Action |
|---|---|
| `type` | Filter (palettes) |
| `↑ ↓ / PgUp PgDn` | Move / scroll |
| `Enter` | Run / confirm |
| `y / n` | Answer confirmations |
| `Esc` | Close |

<!-- keymap:tui:end -->

## MCP server

```bash
claude mcp add dbtool -- dbtool-mcp                  # read-only (default)
claude mcp add dbtool-rw -- dbtool-mcp --allow-write # adds the execute tool
```

| Tool | Description |
|---|---|
| `list_data_sources` | Configured connections (kind, host, default database, read-only, SSH) |
| `list_databases` / `list_schemas` / `list_objects` | Browse the tree (Redis: keys and their types) |
| `describe_table` | Columns, indexes, constraints |
| `get_ddl` | DDL (Redis: commands that recreate the key) |
| `search_objects` | Fuzzy search by object name (Go to Database Object), across the databases visible in each data source |
| `query` | Read-only query (100 rows by default, max 1000) |
| `explain` | Execution plan (optionally ANALYZE) |
| `full_text_search` | Find a value in every table |
| `redis_scan` / `redis_get` | Redis key list / key details |
| `export_csv` | Save a read-only query result as CSV |
| `execute` | Only with `--allow-write`. Runs statements in one transaction; refused for read-only data sources |

**How read-only is enforced**
- PostgreSQL / MySQL: exactly one statement, starting with SELECT / WITH / VALUES / TABLE / SHOW / EXPLAIN / DESCRIBE, executed inside a READ ONLY transaction that is rolled back afterwards. `COMMIT; DELETE …`, `WITH … DELETE` and `EXPLAIN ANALYZE DELETE` are all rejected.
- Redis: only read commands are allowed.

Options: `--config-dir DIR` (use another config folder), `--verbose` (engine logs to stderr).

## Tests

```bash
cargo test --workspace -- --include-ignored --skip tunnel   # needs a local PostgreSQL and the containers below
```

Containers for the integration tests:

```bash
docker run -d --name dbtool-mysql-test -e MYSQL_ROOT_PASSWORD=dbtool -e MYSQL_DATABASE=shop -p 13306:3306 mysql:8.4
docker run -d --name dbtool-redis-test -p 16379:6379 redis:7-alpine
# TLS (16391), Sentinel (26392 → 16392) and Cluster (17001-17003) servers: see the test comments in crates/core/src/redisdb.rs
# (TLS certificate folder: DBTOOL_TEST_TLS_DIR, default /tmp/dbtool-tls)
```

After changing a shortcut, regenerate the tables in this README with `python3 scripts/gen-readme-keymap.py`.

## License

[MIT](LICENSE)
