## dbtool {{VERSION}} (macOS, Apple Silicon)

A DataGrip-style database client for PostgreSQL, MySQL/MariaDB and Redis (Standalone / Sentinel / Cluster), with SSH tunnels and SSL/TLS.

### Downloads
| File | Contents |
|---|---|
| `dbtool_{{VERSION}}_aarch64.dmg` | GUI app — drag it into Applications |
| `dbtool-cli_{{VERSION}}_aarch64-apple-darwin.tar.gz` | `dbtool-tui` (terminal UI) + `dbtool-mcp` (MCP server) |

Use any of them on its own or together. When used together they share data sources and passwords. **The TUI works without the GUI** — add and edit connections inside it with `n` / `e`.

### Install
**GUI** — open the DMG and drag `dbtool.app` into Applications.
The app is not code-signed yet, so the first launch shows an "unidentified developer" / "damaged" warning. Either run
```bash
xattr -dr com.apple.quarantine /Applications/dbtool.app
```
or right-click the app in Finder → Open.

**TUI / MCP**
```bash
tar -xzf dbtool-cli_{{VERSION}}_aarch64-apple-darwin.tar.gz
xattr -d com.apple.quarantine dbtool-tui dbtool-mcp 2>/dev/null
mkdir -p ~/.local/bin && mv dbtool-tui dbtool-mcp ~/.local/bin/
dbtool-tui                            # F1 = keymap
claude mcp add dbtool -- dbtool-mcp   # register with Claude Code (read-only by default)
```

See the [README](https://github.com/Hkhyyy/dbtool#readme) for all features and shortcuts.

