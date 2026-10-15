//! TUI 키맵 (F1 도움말, `dbtool-tui --help`, README 의 기준)
pub const HELP: &[(&str, &[(&str, &str)])] = &[
    ("Global", &[
        ("Ctrl+Q", "Quit"),
        ("F1", "This keymap"),
        ("Ctrl+K", "Actions palette (run any command by name)"),
        ("Ctrl+O", "Go to Database Object (Ctrl+B in the list = DDL)"),
        ("Ctrl+E", "Recent tabs"),
        ("Ctrl+L", "New query console"),
        ("Ctrl+W", "Close tab"),
        ("Alt+← / Alt+→", "Previous / next tab (also Ctrl+PgUp / Ctrl+PgDn)"),
        ("Alt+1", "Database Explorer: focus / hide"),
        ("F6", "Cycle focus: explorer → editor → results"),
    ]),
    ("Database Explorer", &[
        ("↑ ↓  (j k)", "Move"),
        ("→ / ←  (l h)", "Expand / collapse (← on a child jumps to its parent)"),
        ("Enter", "Connect · open table data · open Redis browser · show DDL of routines"),
        ("F4 / Ctrl+↓", "Open a query console for the data source / database"),
        ("Ctrl+B", "Go to DDL"),
        ("/", "Filter loaded nodes (Esc clears)"),
        ("n / e / D", "New / edit / remove data source"),
        ("c / X", "Connect / disconnect (also Ctrl+F2)"),
        ("r / F5", "Refresh"),
        ("y", "Copy name (OSC 52 clipboard)"),
        ("Tab / Esc", "Go to the open tab"),
    ]),
    ("Query console", &[
        ("F5 / Ctrl+Enter / Alt+Enter", "Execute statement at caret (or the selection)"),
        ("Alt+E", "Explain plan"),
        ("Ctrl+C", "Cancel running statement"),
        ("Alt+T", "Transaction mode Auto / Manual"),
        ("Alt+C / Alt+R", "Commit / roll back"),
        ("Alt+O", "Toggle result grid / output log"),
        ("Shift+Tab", "Go to the result grid (Esc / Tab returns)"),
        ("Esc", "Go to the explorer"),
        ("Ctrl+U / Ctrl+R", "Undo / redo (editor)"),
    ]),
    ("Data grid (results & table data)", &[
        ("arrows  (h j k l)", "Move"),
        ("PgUp / PgDn · g / G", "Page · first / last row (more rows load automatically)"),
        ("Home / End", "First / last column"),
        ("Enter / F2", "Edit cell (table data)"),
        ("n", "Set NULL"),
        ("a / Insert", "Add row"),
        ("d / Delete", "Mark row deleted (again = undo)"),
        ("Ctrl+S", "Submit changes (one transaction)"),
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
        ("Space / ← →", "Toggle on/off · change choice (Type, SSL mode, Redis mode…)"),
        ("Ctrl+T", "Test connection"),
        ("Enter / Ctrl+S", "Save"),
        ("Esc", "Cancel"),
    ]),
    ("Popups", &[
        ("type", "Filter (palettes)"),
        ("↑ ↓ / PgUp PgDn", "Move / scroll"),
        ("Enter", "Run / confirm"),
        ("y / n", "Answer confirmations"),
        ("Esc", "Close"),
    ]),
];

/// `--help` 출력
pub fn text() -> String {
    let mut s = String::new();
    for (title, rows) in HELP {
        s.push_str(&format!("\n{title}\n"));
        for (k, d) in *rows {
            s.push_str(&format!("  {k:<30} {d}\n"));
        }
    }
    s
}
