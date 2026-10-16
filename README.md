# dbtool

A DataGrip-style database client. One engine (`dbtool-core`), three ways to use it:

| Binary | What it is |
|---|---|
| **dbtool** (GUI, Tauri) | Desktop app modeled on DataGrip |
| **dbtool-tui** | Terminal UI (great over SSH) — works on its own, no GUI needed |
| **dbtool-mcp** | MCP server so AI tools such as Claude Code can explore and query your databases |

All three share the same data sources and passwords, so a connection created in one shows up in the others.

- Config folder: `~/Library/Application Support/com.hyunkyo.dbtool` (macOS), `~/.config/com.hyunkyo.dbtool` (Linux)
- Custom shortcuts are stored in `keymap.json` in that folder (`"gui"` and `"tui"` sections, only the ones you changed)
- Passwords are stored in `secrets.json` in that folder, encrypted with AES-GCM (key in `secret.key`). The folder is 0700 and its files 0600. The macOS Keychain is not used, so anyone who can read your files (or a copy of this folder) can decrypt the passwords.

## Download

Get the latest build from [Releases](https://github.com/Hkhyyy/dbtool/releases):

- `dbtool_<version>_aarch64.dmg` — GUI app (Apple Silicon). Drag `dbtool.app` to Applications.
- `dbtool-cli_<version>_aarch64-apple-darwin.tar.gz` — `dbtool-tui` and `dbtool-mcp`.

The app is not code-signed yet, so macOS shows an "unidentified developer" / "damaged" warning on first launch. Run `xattr -dr com.apple.quarantine /Applications/dbtool.app` (or right-click the app → Open).

A new release is published automatically every time `main` changes.

### Updates

Both apps check GitHub for a newer release once a day (one request to the GitHub API) and tell you when there is one.

- **GUI** — a `⬆ vX.Y.Z available` button appears in the status bar. It shows the release notes and **Download** fetches the new DMG (drag it into Applications to replace the old app). Turn the check off or run it now in **Settings › Appearance › Updates**.
- **TUI** — the status line shows `⬆ vX.Y.Z available`. Quit and run `dbtool-tui --update` to download the release and replace `dbtool-tui` and `dbtool-mcp` in place. A TUI built from source (`target/release`) is not checked; update it with `git pull` and `cargo build`.
- Turn the check off with `dbtool-tui --no-update-check` or `DBTOOL_NO_UPDATE_CHECK=1`.

## Supported databases

| Database | Features |
|---|---|
| PostgreSQL | Explorer tree (databases, schemas, tables, views, sequences, routines), query console (server-side cursor paging, cancel, manual transactions), data editing, explain plan, DDL, full-text search |
| MySQL / MariaDB | Same as above. A MySQL database is its own schema, so tables appear directly under the database |
| Redis | Medis-style browser (key list with type and TTL, per-type value editor), redis-cli style terminal, **Standalone / Sentinel / Cluster** |

Connection options:
- **SSH tunnel** with a key or password. Host keys are checked against `~/.ssh/known_hosts`: a changed key is refused, and an unknown server stops with its SHA256 fingerprint until you choose **Trust host key** (GUI: data source dialog; TUI: `Ctrl+Y` in the form or `Ctrl+K › Trust SSH Host Key`).
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

Command shortcuts can be changed in **Settings › Keymap** (or **Edit Keymap…** in the Keymap Reference): click *Change* and press the new keys. The tables below show the defaults. Navigation keys inside the tree, grid and popups are fixed.

<!-- keymap:gui:start -->

#### Search & Navigation

| macOS | Action |
|---|---|
| `⇧⇧` | Search Everywhere (All / Database / Files / Symbols / Actions, ⇥ switches tabs). Data sources you are not connected to are searched from the list saved the last time they were loaded (marked *cached*, stored 0600 in the config folder's `search-cache/`); opening a result connects |
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
| `⌃⇧.` | Editor font size bigger |
| `⌃⇧,` | Editor font size smaller |

#### Editor tabs

| macOS | Action |
|---|---|
| `⌘W` | Close tab (middle-click also closes) |
| `⌥ + click ×` | Close other tabs |
| `⇧⌘T` | Reopen closed tab |
| `⇧⌘]` | Next tab |
| `⇧⌘[` | Previous tab |
| `⌥⇥` | Go to the other split |
| `right-click tab` | Pin, Split Right/Down, Move to Opposite Group, Close others… |
| `drag tab` | Reorder, or drop on the right/bottom edge to split |

#### Database Explorer

| macOS | Action |
|---|---|
| `⌘↓ / F4` | Edit data (table) / open console (data source; Redis: key browser) |
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
| `⌘R` | Refresh |
| `⌥⌘Y` | Refresh (anywhere) |
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
| `⌘/` | Line comment |
| `⌥⌘/` | Block comment |
| `⌘D` | Duplicate line |
| `⌘⌫` | Delete line |
| `⌥⇧↑` | Move line up |
| `⌥⇧↓` | Move line down |
| `⌥↑` | Extend selection |
| `⌥↓` | Shrink selection |
| `⌃G` | Select next occurrence |
| `⌃⌘G` | Select all occurrences |
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

Press **F1** inside `dbtool-tui`, or run `dbtool-tui --help`, to see the same list. In the F1 window the **Commands** at the top can be changed: select one, press Enter and then the new key (`a` adds a second key, `Del` resets it). The mouse works too: click to select, double-click to open or edit, scroll with the wheel.
Ctrl+Enter is only distinguishable in terminals that support the kitty keyboard protocol (iTerm2, kitty, WezTerm, Ghostty). Elsewhere use F5 or Alt+Enter.

<!-- keymap:tui:start -->

#### Commands (default keys — change them in the F1 window)

| Key | Action |
|---|---|
| `Ctrl+Q` | Quit |
| `F1` | Keymap |
| `Ctrl+K` | Actions Palette |
| `Ctrl+P` | Search Everywhere (objects · columns · tabs · actions; not-connected data sources from the saved list) |
| `Ctrl+O` | Go to Database Object |
| `Ctrl+B` | Go to DDL |
| `Ctrl+E` | Recent Tabs / Switcher |
| `Ctrl+L` | New Query Console |
| `Ctrl+W` | Close Tab |
| `Alt+Right / Ctrl+PgDn` | Next Tab |
| `Alt+Left / Ctrl+PgUp` | Previous Tab |
| `Alt+1` | Database Explorer |
| `F6` | Cycle Focus (explorer → editor → results) |
| `Alt+\` | Split Right (focused pane, up to 4) |
| `Alt+-` | Split Down (focused pane, up to 4) |
| `Alt+Tab` | Go to the Next Split |
| `Alt+Shift+Left` | Move Split Divider Left / Up |
| `Alt+Shift+Right` | Move Split Divider Right / Down |
| `Alt+[` | Database Explorer Narrower |
| `Alt+]` | Database Explorer Wider |
| `F5 / Ctrl+Enter / Alt+Enter` | Execute |
| `Alt+E` | Explain Plan |
| `Ctrl+Space` | Code Completion |
| `Ctrl+C` | Cancel Running Statement |
| `Alt+T` | Toggle Transaction Mode (Auto/Manual) |
| `Alt+C` | Commit |
| `Alt+R` | Roll Back |
| `Alt+O` | Toggle Output / Result |
| `Alt+Up` | Shrink Editor (console) |
| `Alt+Down` | Grow Editor (console) |
| `Ctrl+S` | Submit Changes |
| `Ctrl+F2` | Disconnect |

#### Database Explorer

| Key | Action |
|---|---|
| `↑ ↓  (j k)` | Move |
| `→ / ←  (l h)` | Expand / collapse (← on a child jumps to its parent) |
| `Enter / double-click` | Expand / collapse (connects) · open table data · open a Redis key · show DDL of routines |
| `F4 / Ctrl+↓` | Open a query console for the data source / database (Redis: the key browser) |
| `/` | Filter loaded nodes (/ again edits it · empty or Esc clears) |
| `n / e / D` | New / edit / remove data source |
| `c / X` | Connect / disconnect |
| `r / F5` | Refresh |
| `y` | Copy name (OSC 52 clipboard) |
| `Tab / Esc` | Go to the open tab |

#### Query console

| Key | Action |
|---|---|
| `completion list` | Opens as you type (or Ctrl+Space) · ↑↓ choose · Enter/Tab insert · Esc close |
| `Shift+Tab` | Go to the result grid (Esc / Tab returns) |
| `Esc` | Go to the explorer |
| `Ctrl+U / Ctrl+R` | Undo / redo (editor) |
| `mouse drag` | Drag the editor/result border to resize |

#### Tabs & split (mouse)

| Key | Action |
|---|---|
| `drag a tab` | Onto a pane's right / bottom edge = split it (up to 4 panes) · onto another pane or tab bar = move · within a bar = reorder |
| `drag the border` | Resize the Database Explorer (its right border) or the panes (the border between them; in a top/bottom split also the empty part of the lower tab bar) |
| `click a pane` | Focus it |
| `Shift+wheel / trackpad` | Scroll the grid and the explorer sideways (Shift+← / Shift+→ in the explorer) |

#### Data grid (results & table data)

| Key | Action |
|---|---|
| `arrows  (h j k l)` | Move |
| `PgUp / PgDn · g / G` | Page · first / last row (more rows load automatically) |
| `Home / End` | First / last column |
| `Shift+← / Shift+→` | Scroll columns sideways (mouse: wheel over the header row, ⌥+wheel, or click / drag the bar on the bottom border) |
| `Enter / F2` | Edit cell (table data) |
| `n` | Set NULL |
| `a / Insert` | Add row |
| `d / Delete` | Mark row deleted (again = undo) |
| `Ctrl+Z` | Revert pending changes |
| `s` | Sort by the selected column: ▲ → ▼ → off (table data: ORDER BY · results: loaded rows) |
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
| `Space / ← →` | Toggle on/off · change choice (Type, Color, SSL mode, Redis mode…) |
| `Ctrl+T` | Test connection |
| `Ctrl+Y` | Trust the SSH server's host key (after Ctrl+T showed its fingerprint) |
| `Enter / Ctrl+S` | Save |
| `Esc` | Cancel |

#### Popups

| Key | Action |
|---|---|
| `Ctrl+B` | Go to DDL (in Search Everywhere / Go to Database Object) |
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
| `export_csv` | Save a read-only query result as CSV in `~/Downloads/dbtool-exports` (file name only, never overwrites) |
| `execute` | Only with `--allow-write`. Runs statements in one transaction; refused for read-only data sources |

**How read-only is enforced**
- PostgreSQL / MySQL: exactly one statement, starting with SELECT / WITH / VALUES / TABLE / SHOW / EXPLAIN / DESCRIBE, executed inside a READ ONLY transaction with a 60 s time limit that is rolled back afterwards. Statements are split with the server's own comment rules (MySQL `--x` and `/*! … */`, PostgreSQL nested comments), and MySQL runs over the prepared-statement protocol, which accepts a single statement. `COMMIT; DELETE …`, `WITH … DELETE`, `EXPLAIN ANALYZE DELETE`, `SELECT … INTO OUTFILE` and functions with side effects outside the transaction (`pg_terminate_backend`, `pg_read_file`, `dblink`, `load_file`, `sleep`, …) are rejected.
- Redis: only read commands, and not `CONFIG` (it can reveal passwords) or `KEYS` (use `redis_scan`).
- Still, give the MCP server a **least-privilege database user** (read-only role, no superuser/FILE privilege): the checks above are a second line of defence, not a replacement for database permissions.

Options: `--config-dir DIR` (use another config folder), `--verbose` (engine logs to stderr).

## Safety

- **Read-only data sources** are enforced by the engine, not only the UI: in every console, the table editor, Redis and exports, only reading statements run (plus transaction control and `SET`, but not switching read-only off).
- **Transactions**: a transaction you open (Manual mode, or `BEGIN` in Auto mode) is never committed or rolled back behind your back by result paging. Switching Manual → Auto with uncommitted changes asks first, and closing a tab, disconnecting, switching database or quitting with uncommitted changes asks before rolling them back. Data sources with open transactions are not auto-disconnected.
- **Confirmations** before `UPDATE`/`DELETE` without `WHERE`, `DROP`, `TRUNCATE` and Redis `FLUSHALL`/`FLUSHDB`/`DEL` (GUI and TUI).
- **Scripts in Auto mode** run statement by statement: if one fails, the output says how many earlier statements were already committed.
- **Table editor** saves all edits in one transaction, and each `UPDATE`/`DELETE` must change exactly one row, otherwise nothing is saved.
- **Import** of data source files shows what changes first (e.g. "turns Read-only OFF", new startup script).
- **Query history** (GUI) hides passwords in `CREATE/ALTER USER … PASSWORD '…'`, `IDENTIFIED BY '…'` and Redis `AUTH`.
- **Releases** are built by GitHub Actions pinned to commit SHAs, with `SHA256SUMS` and build provenance attestations; `dbtool-tui --update` verifies the checksum before installing.

## Tests

```bash
cargo test --workspace -- --include-ignored --skip tunnel   # needs a local PostgreSQL and the containers below
DBTOOL_TEST_PG=127.0.0.1:15499:postgres:shop cargo test -p dbtool-core session -- --ignored   # PostgreSQL elsewhere (host:port:user:db)
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
