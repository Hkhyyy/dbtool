//! 화면 그리기
use crate::app::{App, FieldKind, Focus, Popup, TabKind};
use crate::tree::NodeKind;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap};
use serde_json::Value;

pub const ACCENT: Color = Color::Rgb(84, 138, 247);
const DIM: Color = Color::DarkGray;

pub fn hex(c: &str) -> Option<Color> {
    let c = c.trim_start_matches('#');
    if c.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(c, 16).ok()?;
    Some(Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

pub fn type_color(t: &str) -> Color {
    match t {
        "string" => Color::Rgb(95, 158, 160),
        "hash" => Color::Rgb(176, 124, 198),
        "list" => Color::Rgb(106, 159, 216),
        "set" => Color::Rgb(208, 139, 76),
        "zset" => Color::Rgb(201, 96, 92),
        "stream" => Color::Rgb(90, 164, 105),
        _ => Color::Gray,
    }
}

/// redis-cli 와 같은 출력
pub fn format_reply(v: &Value) -> String {
    fn go(v: &Value, indent: usize) -> String {
        match v {
            Value::Null => "(nil)".into(),
            Value::Number(n) => format!("(integer) {n}"),
            Value::Bool(b) => format!("({b})"),
            Value::String(s) => format!("{s:?}"),
            Value::Object(o) => o.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string(),
            Value::Array(a) if a.is_empty() => "(empty array)".into(),
            Value::Array(a) => {
                let w = a.len().to_string().len();
                a.iter()
                    .enumerate()
                    .map(|(i, x)| {
                        let head = format!("{:>w$}) ", i + 1);
                        let body = go(x, indent + head.len());
                        format!("{}{head}{body}", if i == 0 { String::new() } else { " ".repeat(indent) })
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
    }
    go(v, 0)
}

fn block(title: impl Into<Line<'static>>, focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(if focused { BorderType::Thick } else { BorderType::Plain })
        .title(title)
        .border_style(Style::default().fg(if focused { ACCENT } else { DIM }))
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let [main, status] = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(f.area());
    let right = if app.explorer {
        let w = (main.width / 4).clamp(26, 48);
        let [l, r] = Layout::horizontal([Constraint::Length(w), Constraint::Min(10)]).areas(main);
        draw_tree(f, app, l);
        r
    } else {
        main
    };
    draw_tabs(f, app, right);
    draw_status(f, app, status);
    if app.popup.is_some() {
        draw_popup(f, app);
    }
}

fn draw_tree(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Tree;
    let title = if app.tree.filter.is_empty() { " Database Explorer ".to_string() } else { format!(" Database Explorer — /{} ", app.tree.filter) };
    let b = block(title, focused);
    let inner = b.inner(area);
    f.render_widget(b, area);
    app.hits.tree = inner;
    let h = inner.height as usize;
    if app.tree.sel < app.tree.offset {
        app.tree.offset = app.tree.sel;
    } else if h > 0 && app.tree.sel >= app.tree.offset + h {
        app.tree.offset = app.tree.sel + 1 - h;
    }
    let vis = app.tree.visible();
    let mut lines = vec![];
    for (i, (depth, n)) in vis.iter().enumerate().skip(app.tree.offset).take(h) {
        let open = app.tree.expanded.contains(&n.key);
        let loading = app.tree.loading.contains(&n.key);
        let chev = if n.expandable { if open { "▾ " } else { "▸ " } } else { "  " };
        let conn_color = app.conn_color(&n.conn).and_then(|c| hex(&c));
        let connected = app.conn(&n.conn).map(|c| c.connected).unwrap_or(false);
        let (icon, icolor) = match &n.kind {
            NodeKind::Conn => (if connected { "◉ " } else { "○ " }, conn_color.unwrap_or(Color::Rgb(111, 179, 210))),
            NodeKind::Db => ("⛁ ", Color::Rgb(111, 179, 210)),
            NodeKind::Schema => ("◇ ", Color::Gray),
            NodeKind::Group => ("▤ ", Color::Rgb(84, 138, 247)),
            NodeKind::Object(k) => match k.as_str() {
                "table" | "partitioned" | "foreign" => ("▦ ", Color::Gray),
                "view" | "matview" => ("◫ ", Color::Rgb(127, 176, 110)),
                "sequence" => ("# ", Color::Rgb(201, 162, 109)),
                "key" => ("● ", type_color(&n.hint)),
                _ => ("ƒ ", Color::Rgb(86, 168, 245)),
            },
            NodeKind::Column { pk, .. } => (if *pk { "⚿ " } else { "· " }, if *pk { Color::Yellow } else { DIM }),
            NodeKind::Info => ("", Color::Red),
        };
        let mut spans = vec![Span::raw("  ".repeat(*depth)), Span::styled(chev, Style::default().fg(DIM)), Span::styled(icon, Style::default().fg(icolor))];
        let mut name_style = Style::default();
        if matches!(n.kind, NodeKind::Conn) {
            name_style = name_style.add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(n.label.clone(), name_style));
        let hint = if loading { "loading…".to_string() } else { n.hint.clone() };
        if !hint.is_empty() {
            spans.push(Span::styled(format!(" {hint}"), Style::default().fg(DIM)));
        }
        let mut line = Line::from(spans);
        if i == app.tree.sel {
            line = line.style(Style::default().bg(if focused { Color::Rgb(46, 67, 110) } else { Color::Rgb(50, 52, 56) }));
        } else if let (NodeKind::Conn, Some(c)) = (&n.kind, conn_color) {
            if let Color::Rgb(r, g, b) = c {
                line = line.style(Style::default().bg(Color::Rgb(r / 2, g / 2, b / 2)));
            }
        }
        lines.push(line);
    }
    if vis.is_empty() {
        lines.push(Line::styled("No data sources. Press n to add one.", Style::default().fg(DIM)));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_tabs(f: &mut Frame, app: &mut App, area: Rect) {
    if app.tabs.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled("dbtool", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Line::raw(""),
            Line::raw("Enter on a data source to connect · Enter on a table to edit data"),
            Line::raw("Ctrl+L new query console · Ctrl+O go to object · Ctrl+K actions · F1 keymap"),
        ])
        .alignment(Alignment::Center)
        .block(block("", false));
        f.render_widget(p, area);
        return;
    }
    let [bar, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(area);
    // 탭 위치: Tabs 위젯은 " 제목 " + "│" 순서로 그린다
    {
        use unicode_width::UnicodeWidthStr;
        app.hits.tabs.clear();
        app.hits.tab_bar_y = bar.y;
        let mut x = bar.x;
        for t in &app.tabs {
            let dirty = matches!(&t.kind, TabKind::Table(tt) if tt.grid.as_ref().map(|g| g.dirty()).unwrap_or(false));
            // " " + 아이콘(2) + 제목 + " *"? + " "
            let w = 1 + 2 + t.title.width() as u16 + if dirty { 2 } else { 0 } + 1;
            app.hits.tabs.push((t.id, x, x + w));
            x += w + 1;
        }
    }
    app.hits.editor = Rect::default();
    app.hits.results = Rect::default();
    app.hits.redis_keys = Rect::default();
    app.hits.redis_value = Rect::default();
    let titles: Vec<Line> = app
        .tabs
        .iter()
        .map(|t| {
            let icon = match t.kind {
                TabKind::Console(_) => "⌘ ",
                TabKind::Table(_) => "▦ ",
                TabKind::Text(_) => "ƒ ",
                TabKind::Redis(_) => "◆ ",
            };
            let dirty = matches!(&t.kind, TabKind::Table(tt) if tt.grid.as_ref().map(|g| g.dirty()).unwrap_or(false));
            let c = app.conn_color(&t.conn).and_then(|c| hex(&c));
            let mut l = Line::from(vec![Span::raw(icon), Span::raw(t.title.clone()), Span::raw(if dirty { " *" } else { "" })]);
            if let Some(c) = c {
                l = l.style(Style::default().fg(c));
            }
            l
        })
        .collect();
    f.render_widget(
        Tabs::new(titles).select(app.active).highlight_style(Style::default().fg(Color::White).bg(Color::Rgb(46, 67, 110)).add_modifier(Modifier::BOLD)).divider("│"),
        bar,
    );
    let focus = app.focus;
    let ro = app.tabs.get(app.active).and_then(|t| app.conn(&t.conn)).map(|c| c.config.read_only).unwrap_or(false);
    let tab = &mut app.tabs[app.active];
    let db = tab.db.clone();
    match &mut tab.kind {
        TabKind::Console(c) => {
            let [ed, res] = Layout::vertical([Constraint::Percentage(42), Constraint::Percentage(58)]).areas(body);
            let title = format!(
                " {db} · Tx: {}{}{}{} ",
                if c.tx_manual { "Manual" } else { "Auto" },
                if c.in_tx { " (pending — Alt+C commit / Alt+R rollback)" } else { "" },
                if c.running { " · running… Ctrl+C cancel" } else { "" },
                if ro { " · read-only" } else { "" }
            );
            c.editor.set_block(block(title, focus == Focus::Editor));
            c.editor.set_line_number_style(Style::default().fg(DIM));
            f.render_widget(&c.editor, ed);
            app.hits.editor = ed;
            app.hits.results = res;
            if c.show_output || c.grid.is_none() {
                let lines: Vec<Line> = c
                    .output
                    .iter()
                    .rev()
                    .take(res.height as usize)
                    .rev()
                    .map(|(e, s)| Line::styled(s.clone(), Style::default().fg(if *e { Color::Red } else if s.contains("> ") { ACCENT } else { Color::Gray })))
                    .collect();
                let p = Paragraph::new(if lines.is_empty() { vec![Line::styled("Execute a statement (F5 / Ctrl+Enter) to see the output.", Style::default().fg(DIM))] } else { lines })
                    .wrap(Wrap { trim: false })
                    .block(block(" Output (Alt+O: result) ", focus == Focus::Grid));
                f.render_widget(p, res);
            } else if let Some(g) = &mut c.grid {
                let t = format!(" {} — {}{} rows (Alt+O: output) ", c.result_title, g.rows(), if g.result.truncated { "+" } else { "" });
                g.render(f, res, t, focus == Focus::Grid);
            }
        }
        TabKind::Table(tt) => {
            let [filt, grid] = Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(body);
            app.hits.results = grid;
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(" WHERE ", Style::default().fg(DIM)),
                    Span::raw(if tt.where_.is_empty() { "—".to_string() } else { tt.where_.clone() }),
                    Span::styled("  ORDER BY ", Style::default().fg(DIM)),
                    Span::raw(if tt.order.is_empty() { "—".to_string() } else { tt.order.clone() }),
                    Span::styled("   (w / o to edit)", Style::default().fg(DIM)),
                ])),
                filt,
            );
            if let Some(e) = &tt.error {
                f.render_widget(Paragraph::new(e.clone()).style(Style::default().fg(Color::Red)).wrap(Wrap { trim: false }).block(block(" Error ", true)), grid);
            } else if let Some(g) = &mut tt.grid {
                let pend = if g.dirty() { format!(" · {} pending (Ctrl+S submit, Ctrl+Z revert)", g.edits.len() + g.deleted.len() + g.added) } else { String::new() };
                let t = format!(" {}.{} — {}{} rows{}{}{} ", tt.schema, tt.table, g.rows(), if g.result.truncated { "+" } else { "" }, if tt.pk.is_empty() { " · no PK (read-only)" } else { "" }, if ro { " · read-only" } else { "" }, pend);
                g.render(f, grid, t, focus == Focus::Grid);
            } else {
                f.render_widget(Paragraph::new("Loading…").block(block(format!(" {}.{} ", tt.schema, tt.table), true)), grid);
            }
        }
        TabKind::Text(t) => {
            let p = Paragraph::new(t.text.clone()).scroll((t.scroll, 0)).block(block(" DDL ", focus != Focus::Tree));
            f.render_widget(p, body);
        }
        TabKind::Redis(r) => {
            let [left, right] = Layout::horizontal([Constraint::Percentage(38), Constraint::Percentage(62)]).areas(body);
            let kf = focus == Focus::Grid && !r.focus_value;
            let title = format!(" Keys {} — {}{} {} ", r.pattern, r.keys.len(), if r.cursor != "0" { "+" } else { "" }, if r.scanning { "scanning…" } else { "" });
            let b = block(title, kf);
            let inner = b.inner(left);
            f.render_widget(b, left);
            let h = inner.height as usize;
            if r.sel < r.off {
                r.off = r.sel;
            } else if h > 0 && r.sel >= r.off + h {
                r.off = r.sel + 1 - h;
            }
            let items: Vec<ListItem> = r
                .keys
                .iter()
                .enumerate()
                .skip(r.off)
                .take(h)
                .map(|(i, k)| {
                    let badge = format!(" {:<6} ", k.ty.to_uppercase());
                    let ttl = if k.ttl >= 0 { format!(" {}s", k.ttl / 1000) } else { String::new() };
                    let mut l = Line::from(vec![Span::styled(badge, Style::default().fg(Color::Black).bg(type_color(&k.ty))), Span::raw(" "), Span::raw(k.key.clone()), Span::styled(ttl, Style::default().fg(DIM))]);
                    if i == r.sel {
                        l = l.style(Style::default().bg(if kf { Color::Rgb(46, 67, 110) } else { Color::Rgb(50, 52, 56) }));
                    }
                    ListItem::new(l)
                })
                .collect();
            f.render_widget(List::new(items), inner);
            app.hits.redis_keys = inner;
            app.hits.redis_value = right;
            let vf = focus == Focus::Grid && r.focus_value;
            match &mut r.detail {
                None => f.render_widget(Paragraph::new("Select a key").block(block(" Value ", vf)), right),
                Some((key, d)) => {
                    let ttl = if d.ttl >= 0 { format!("TTL {}s", d.ttl / 1000) } else { "no TTL".into() };
                    let title = format!(" {} {key} · {} · {} {} ", d.ty.to_uppercase(), ttl, d.length, if d.ty == "string" { "bytes" } else { "items" });
                    match &mut r.value {
                        Some(g) => g.render(f, right, title, vf),
                        None => {
                            let text = d.value.as_str().map(|s| match serde_json::from_str::<Value>(s) {
                                Ok(v) if v.is_object() || v.is_array() => serde_json::to_string_pretty(&v).unwrap_or_default(),
                                _ => s.to_string(),
                            });
                            f.render_widget(Paragraph::new(text.unwrap_or_default()).wrap(Wrap { trim: false }).block(block(title, vf)), right);
                        }
                    }
                }
            }
        }
    }
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let hints = match app.focus {
        Focus::Tree => "↑↓ move · → expand · Enter open · F4 console · Ctrl+B DDL · n new · e edit · / filter",
        Focus::Editor => "F5/Ctrl+Enter run · Alt+E explain · Alt+T tx · Tab→ result · Ctrl+O object · Ctrl+K actions",
        Focus::Grid => match app.tabs.get(app.active).map(|t| &t.kind) {
            Some(TabKind::Table(_)) => "Enter edit · n NULL · a add · d delete · Ctrl+S submit · w where · o order · x export",
            Some(TabKind::Redis(_)) => "Enter value · / pattern · m more · e edit · x del item · D del key · t TTL · R rename · : command",
            _ => "arrows move · PgUp/PgDn · Home/End · Esc editor · x export",
        },
    };
    let [l, r] = Layout::horizontal([Constraint::Min(10), Constraint::Length(hints.chars().count() as u16 + 1)]).areas(area);
    f.render_widget(Paragraph::new(format!(" {}", app.status)).style(Style::default().fg(Color::Gray).bg(Color::Rgb(43, 45, 48))), l);
    f.render_widget(Paragraph::new(hints).style(Style::default().fg(DIM).bg(Color::Rgb(43, 45, 48))), r);
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(4));
    let h = h.min(area.height.saturating_sub(2));
    Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height.saturating_sub(h)) / 3, width: w, height: h }
}

fn draw_popup(f: &mut Frame, app: &mut App) {
    let area = f.area();
    match app.popup.as_mut().unwrap() {
        Popup::Help { scroll } => {
            let mut lines = vec![];
            for (title, rows) in crate::help::HELP {
                lines.push(Line::styled(title.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
                for (k, d) in *rows {
                    lines.push(Line::from(vec![Span::styled(format!("  {k:<30}"), Style::default().fg(ACCENT)), Span::raw(d.to_string())]));
                }
                lines.push(Line::raw(""));
            }
            let r = centered(area, 104, area.height.saturating_sub(2));
            let max = (lines.len() as u16).saturating_sub(r.height.saturating_sub(2));
            if *scroll > max {
                *scroll = max;
            }
            f.render_widget(Clear, r);
            f.render_widget(Paragraph::new(lines).scroll((*scroll, 0)).block(block(" Keymap — ↑↓ PgUp PgDn scroll · Esc close ", true)), r);
        }
        Popup::Message { title, text } => {
            let r = centered(area, 90, (text.lines().count() as u16 + 4).min(30));
            f.render_widget(Clear, r);
            f.render_widget(Paragraph::new(text.clone()).wrap(Wrap { trim: false }).block(block(format!(" {title} (Esc) "), true)), r);
        }
        Popup::Confirm { text, .. } => {
            let r = centered(area, 70, 5);
            f.render_widget(Clear, r);
            f.render_widget(Paragraph::new(vec![Line::raw(text.clone()), Line::raw(""), Line::styled("y = yes · n / Esc = no", Style::default().fg(DIM))]).block(block(" Confirm ", true)), r);
        }
        Popup::Input { title, input, .. } => {
            let r = centered(area, 90, 3);
            f.render_widget(Clear, r);
            input.set_block(block(format!(" {title} (Enter / Esc) "), true));
            f.render_widget(&*input, r);
        }
        Popup::Palette(p) => {
            let r = centered(area, 100, 24);
            f.render_widget(Clear, r);
            let [inp, list] = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(r);
            app.hits.palette = Block::default().borders(Borders::ALL).inner(list);
            let t = format!(" {}{} ", p.title, if p.loading > 0 { " — loading…" } else { "" });
            p.input.set_block(block(t, true));
            p.input.set_cursor_line_style(Style::default());
            f.render_widget(&p.input, inp);
            let matches = App::palette_matches(p);
            let h = list.height.saturating_sub(2) as usize;
            let sel = p.sel.min(matches.len().saturating_sub(1));
            let off = sel.saturating_sub(h.saturating_sub(1));
            app.hits.palette_off = off;
            let items: Vec<ListItem> = matches
                .iter()
                .enumerate()
                .skip(off)
                .take(h)
                .map(|(i, it)| {
                    let mut l = Line::from(vec![Span::raw(it.label.clone()), Span::styled(format!("  {}", it.hint), Style::default().fg(DIM))]);
                    if i == sel {
                        l = l.style(Style::default().bg(Color::Rgb(46, 67, 110)));
                    }
                    ListItem::new(l)
                })
                .collect();
            p.sel = sel;
            let hint = if p.title.starts_with("Go to") { " Enter open · Ctrl+B DDL · Esc " } else { " Enter · Esc " };
            f.render_widget(List::new(items).block(block(hint, false)), list);
        }
        Popup::ConnForm(form) => {
            let h = form.fields.len() as u16 + 6;
            let r = centered(area, 72, h);
            f.render_widget(Clear, r);
            let mut lines: Vec<Line> = form
                .fields
                .iter()
                .enumerate()
                .map(|(i, fl)| {
                    let shown = match fl.kind {
                        FieldKind::Secret if fl.value != "<saved>" => "•".repeat(fl.value.chars().count()),
                        FieldKind::Toggle => format!("[{}]", if fl.value == "on" { "x" } else { " " }),
                        FieldKind::Choice(_) => format!("◂ {} ▸", fl.value),
                        _ => fl.value.clone(),
                    };
                    let sel = i == form.sel;
                    Line::from(vec![
                        Span::styled(format!("{:>14} ", fl.label), Style::default().fg(if sel { ACCENT } else { DIM })),
                        Span::styled(shown + if sel && matches!(fl.kind, FieldKind::Text | FieldKind::Secret) { "▏" } else { "" }, if sel { Style::default().bg(Color::Rgb(46, 67, 110)) } else { Style::default() }),
                    ])
                })
                .collect();
            lines.push(Line::raw(""));
            if let Some((ok, m)) = &form.message {
                lines.push(Line::styled(m.clone(), Style::default().fg(if *ok { Color::Green } else { Color::Red })));
            }
            lines.push(Line::styled("↑↓/Tab move · type to edit · Space/←→ toggle · Ctrl+T test · Ctrl+S / Enter save · Esc cancel", Style::default().fg(DIM)));
            let t = if form.edit.is_some() { " Data Source Properties " } else { " New Data Source " };
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(block(t, true)), r);
        }
    }
}
