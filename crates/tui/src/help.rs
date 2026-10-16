//! TUI 키맵 (F1 도움말, `dbtool-tui --help`, README 의 기준).
//! 바꿀 수 있는 명령 단축키는 keys.rs 의 DEFAULT_KEYS 에 있고, 여기는 화면별로 고정된 키만 둔다
pub const HELP: &[(&str, &[(&str, &str)])] = &[
    ("Database Explorer", &[
        ("↑ ↓  (j k)", "Move"),
        ("→ / ←  (l h)", "Expand / collapse (← on a child jumps to its parent)"),
        ("Enter / double-click", "Expand / collapse (connects) · open table data · open a Redis key · show DDL of routines"),
        ("F4 / Ctrl+↓", "Open a query console for the data source / database (Redis: the key browser)"),
        ("Ctrl+B", "Go to DDL"),
        ("/", "Filter loaded nodes (/ again edits it · empty or Esc clears)"),
        ("n / e / D", "New / edit / remove data source"),
        ("c / X", "Connect / disconnect"),
        ("r / F5", "Refresh"),
        ("y", "Copy name (OSC 52 clipboard)"),
        ("Tab / Esc", "Go to the open tab"),
    ]),
    ("Query console", &[
        ("completion list", "Opens as you type (or Ctrl+Space) · ↑↓ choose · Enter/Tab insert · Esc close"),
        ("Shift+Tab", "Go to the result grid (Esc / Tab returns)"),
        ("Esc", "Go to the explorer"),
        ("Ctrl+U / Ctrl+R", "Undo / redo (editor)"),
        ("mouse drag", "Drag the editor/result border to resize"),
    ]),
    ("Tabs & split (mouse)", &[
        ("drag a tab", "Onto the right / bottom edge = split · onto the other pane or tab bar = move · within a bar = reorder"),
        ("drag the border", "Resize the Database Explorer (its right border) or the two panes (the border between them; in a top/bottom split also the empty part of the lower tab bar)"),
        ("click a pane", "Focus it"),
        ("Shift+wheel / trackpad", "Scroll the grid and the explorer sideways (Shift+← / Shift+→ in the explorer)"),
    ]),
    ("Data grid (results & table data)", &[
        ("arrows  (h j k l)", "Move"),
        ("PgUp / PgDn · g / G", "Page · first / last row (more rows load automatically)"),
        ("Home / End", "First / last column"),
        ("Shift+← / Shift+→", "Scroll columns sideways (mouse: wheel over the header row, ⌥+wheel, or click / drag the bar on the bottom border)"),
        ("Enter / F2", "Edit cell (table data)"),
        ("n", "Set NULL"),
        ("a / Insert", "Add row"),
        ("d / Delete", "Mark row deleted (again = undo)"),
        ("Ctrl+Z", "Revert pending changes"),
        ("w / o", "Edit WHERE / ORDER BY"),
        ("r", "Reload"),
        ("x", "Export to CSV"),
        ("y", "Copy cell value"),
        ("Esc", "Back"),
    ]),
    ("Redis browser (Medis style)", &[
        ("↑ ↓", "Select key"),
        ("Enter / → / Tab", "Go to the value"),
        ("/", "Key pattern (SCAN MATCH)"),
        ("m", "Scan more"),
        ("r", "Rescan"),
        ("a", "Add key (SET / HSET / RPUSH / SADD / ZADD …)"),
        ("D", "Delete key"),
        ("R", "Rename key"),
        ("t", "Set TTL (empty = persist)"),
        (":", "Run any command"),
        ("e / Enter (value)", "Edit selected element / string"),
        ("a (value)", "Add element"),
        ("x (value)", "Delete element"),
        ("Esc", "Back to keys"),
    ]),
    ("Data source form", &[
        ("↑ ↓ / Tab", "Move between fields"),
        ("type / Backspace", "Edit text fields"),
        ("Space / ← →", "Toggle on/off · change choice (Type, Color, SSL mode, Redis mode…)"),
        ("Ctrl+T", "Test connection"),
        ("Enter / Ctrl+S", "Save"),
        ("Esc", "Cancel"),
    ]),
    ("Popups", &[
        ("Ctrl+B", "Go to DDL (in Go to Database Object)"),
        ("type", "Filter (palettes)"),
        ("↑ ↓ / PgUp PgDn", "Move / scroll"),
        ("Enter", "Run / confirm"),
        ("y / n", "Answer confirmations"),
        ("Esc", "Close"),
    ]),
];

/// `--help` 출력 (명령 단축키는 기본값)
pub fn text() -> String {
    let mut s = String::from("\nCommands (change them with F1)\n");
    for (a, keys) in crate::keys::DEFAULT_KEYS {
        s.push_str(&format!("  {:<30} {}\n", keys.join(" / "), crate::input::name_of(*a)));
    }
    for (title, rows) in HELP {
        s.push_str(&format!("\n{title}\n"));
        for (k, d) in *rows {
            s.push_str(&format!("  {k:<30} {d}\n"));
        }
    }
    s
}
