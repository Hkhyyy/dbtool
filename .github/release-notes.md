## Qurry {{VERSION}} (macOS, Apple Silicon)

A free DataGrip alternative for PostgreSQL, MySQL/MariaDB and Redis (Standalone / Sentinel / Cluster), with SSH tunnels and SSL/TLS. Formerly **dbtool**.

### Downloads
| File | Contents |
|---|---|
| `Qurry_{{VERSION}}_aarch64.dmg` | GUI app — drag it into Applications |
| `qurry-cli_{{VERSION}}_aarch64-apple-darwin.tar.gz` | `qurry-tui` (terminal UI) + `qurry-mcp` (MCP server) |

Use any of them on its own or together. When used together they share data sources and passwords. **The TUI works without the GUI** — add and edit connections inside it with `n` / `e`.

### Install
**GUI** — open the DMG and drag `Qurry.app` into Applications.
The app is not code-signed yet, so the first launch shows an "unidentified developer" / "damaged" warning. Either run
```bash
xattr -dr com.apple.quarantine /Applications/Qurry.app
```
or right-click the app in Finder → Open.

**TUI / MCP**
```bash
tar -xzf qurry-cli_{{VERSION}}_aarch64-apple-darwin.tar.gz
xattr -d com.apple.quarantine qurry-tui qurry-mcp 2>/dev/null
mkdir -p ~/.local/bin && mv qurry-tui qurry-mcp ~/.local/bin/
qurry-tui                             # F1 = keymap
claude mcp add qurry -- qurry-mcp     # register with Claude Code (read-only by default)
```
Already installed? In the TUI run ⌃k › **Update Qurry & Restart** (or `qurry-tui --update`). The GUI shows `⬆ available` in its status bar; **Update & Restart** installs the new version in place.

Coming from **dbtool**? Your data sources, passwords and settings are kept. Updating an old `dbtool-tui` replaces `dbtool-tui` / `dbtool-mcp` with Qurry under the old names (so existing MCP settings keep working); the next update also adds `qurry-tui` / `qurry-mcp`. The `dbtool-cli_…` file in this release is the same archive, kept so older versions can update.

See the [README](https://github.com/Hkhyyy/dbtool#readme) for all features and shortcuts.
