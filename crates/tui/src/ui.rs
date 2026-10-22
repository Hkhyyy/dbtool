//! 화면 그리기
use crate::keys::disp;
use crate::app::{App, Drag, DropTarget, FieldKind, Focus, Popup, SplitDir, TabKind};
use crate::tree::NodeKind;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap};
use serde_json::Value;

pub const ACCENT: Color = Color::Rgb(84, 138, 247);
const DIM: Color = Color::DarkGray;

/// GUI(src/lib/api.ts COLOR_PRESETS) 와 같은 Data Source 색
pub const COLOR_PRESETS: &[(&str, &str)] = &[
    ("red", "#6e2a2a"),
    ("orange", "#6e4a22"),
    ("yellow", "#5f5b24"),
    ("green", "#2f5634"),
    ("teal", "#22524f"),
    ("blue", "#28406e"),
    ("violet", "#4f2f68"),
    ("gray", "#45474d"),
];

/// 폼에 보일 색 이름 (프리셋이 아니면 custom)
pub fn color_name(c: Option<&str>) -> &'static str {
    match c {
        None => "none",
        Some(h) => COLOR_PRESETS.iter().find(|p| p.1.eq_ignore_ascii_case(h)).map(|p| p.0).unwrap_or("custom"),
    }
}

/// 활성 탭처럼 강조할 때 쓰는 밝은 색
fn lighten(c: Color) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(r.saturating_add(r / 2 + 16), g.saturating_add(g / 2 + 16), b.saturating_add(b / 2 + 16)),
        c => c,
    }
}

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
    app.hits.main = main;
    app.hits.explorer_divider = Rect::default();
    let right = if app.explorer {
        let w = app.explorer_width(main.width);
        let [l, r] = Layout::horizontal([Constraint::Length(w), Constraint::Min(10)]).areas(main);
        draw_tree(f, app, l);
        // 트리 오른쪽 테두리와 맞닿은 칸까지 두 칸 (끌어서 폭 조절. 한 칸은 맞히기 어렵다)
        app.hits.explorer_divider = Rect { x: l.right().saturating_sub(1), y: l.y, width: 2, height: l.height };
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
    let title = if app.tree.filter.is_empty() { " Database Explorer ".to_string() } else { format!(" Database Explorer — /{} (Esc clears) ", app.tree.filter) };
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
        // 배경색이 줄 끝까지 가도록 채운다
        {
            use unicode_width::UnicodeWidthStr;
            let used: usize = spans.iter().map(|s| s.content.width()).sum();
            let want = inner.width as usize + app.tree.hoff;
            if used < want {
                spans.push(Span::raw(" ".repeat(want - used)));
            }
        }
        let mut line = Line::from(spans);
        if i == app.tree.sel {
            line = line.style(Style::default().bg(if focused { Color::Rgb(46, 67, 110) } else { Color::Rgb(50, 52, 56) }));
        } else if let Some(c) = conn_color {
            // GUI 처럼 Data Source 아래 전체를 그 색으로
            line = line.style(Style::default().bg(c));
        }
        lines.push(line);
    }
    if vis.is_empty() {
        lines.push(Line::styled("No data sources. Press n to add one.", Style::default().fg(DIM)));
    }
    // 가로 스크롤 (Shift+←/→, 트랙패드 좌우)
    f.render_widget(Paragraph::new(lines).scroll((0, app.tree.hoff as u16)), inner);
}

fn draw_tabs(f: &mut Frame, app: &mut App, area: Rect) {
    if app.tabs.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled("dbtool", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Line::raw(""),
            Line::raw("Enter on a data source to connect · Enter on a table to edit data"),
            Line::raw(disp("Ctrl+L new query console · Ctrl+P search everywhere · Ctrl+K actions · F1 keymap")),
        ])
        .alignment(Alignment::Center)
        .block(block("", false));
        f.render_widget(p, area);
        return;
    }
    for r in [&mut app.hits.editor, &mut app.hits.results, &mut app.hits.redis_keys, &mut app.hits.redis_value, &mut app.hits.console, &mut app.hits.divider] {
        *r = Rect::default();
    }
    app.hits.tabs.clear();
    app.hits.groups.clear();
    app.hits.splits.clear();
    if app.panes.is_split() {
        let panes = app.panes.clone();
        let fg = app.focused_group();
        draw_panes(f, app, &panes, area, &mut vec![], fg);
    } else {
        draw_group(f, app, None, area, true);
    }
    // 탭을 끄는 중: 놓일 곳을 칠한다
    if let Some(Drag::Tab { id, moved: true }) = app.drag {
        let (x, y) = app.drag_pos;
        let r = crate::input::drop_target(app, id, x, y).and_then(|t| {
            let body = |g: usize| app.hits.groups.iter().find(|x| x.0 == g).map(|x| (x.1, x.2));
            match t {
                DropTarget::NewSplit(g, SplitDir::Right) => body(g).map(|(_, b)| Rect { x: b.x + b.width / 2, width: b.width - b.width / 2, ..b }),
                DropTarget::NewSplit(g, SplitDir::Down) => body(g).map(|(_, b)| Rect { y: b.y + b.height / 2, height: b.height - b.height / 2, ..b }),
                DropTarget::Group(g, None) => body(g).map(|(bar, b)| if bar.y == y { bar } else { b }),
                DropTarget::Group(_, Some(before)) => app.hits.tabs.iter().find(|t| t.0 == before).map(|t| Rect { x: t.1, y: t.3, width: t.2 - t.1, height: 1 }),
            }
        });
        if let Some(r) = r {
            f.buffer_mut().set_style(r, Style::default().bg(Color::Rgb(40, 70, 120)));
        }
    }
}

/// 나눈 화면: 가지는 둘로 나누고 잎(칸)은 그린다. 경계선 위치를 기록한다 (끌어서 비율 조절)
fn draw_panes(f: &mut Frame, app: &mut App, p: &crate::panes::Pane, area: Rect, path: &mut Vec<bool>, fg: usize) {
    use crate::panes::Pane;
    match p {
        Pane::Leaf(g) => draw_group(f, app, Some(*g), area, *g == fg),
        Pane::Split { dir, pct, a, b } => {
            let (p, q) = (Constraint::Percentage(*pct), Constraint::Percentage(100 - pct));
            let [ra, rb] = match dir {
                SplitDir::Right => Layout::horizontal([p, q]).areas(area),
                SplitDir::Down => Layout::vertical([p, q]).areas(area),
            };
            // 경계선: 좌우면 맞닿은 두 테두리, 상하면 첫 칸의 아래 테두리 (탭 바 줄은 빼고)
            let divider = match dir {
                SplitDir::Right => Rect { x: ra.right().saturating_sub(1), y: ra.y + 1, width: 2, height: ra.height.saturating_sub(1) },
                SplitDir::Down => Rect { x: ra.x, y: ra.bottom().saturating_sub(1), width: ra.width, height: 1 },
            };
            // 위아래: 아래 첫 칸의 탭 바 (빈 곳을 잡으면 경계선처럼 끈다. 테두리 한 줄만으로는 맞히기 어렵다)
            let grab = if *dir == SplitDir::Down { Rect { height: 1, ..rb } } else { Rect::default() };
            app.hits.splits.push((path.clone(), area, divider, grab, *dir));
            path.push(false);
            draw_panes(f, app, a, ra, path, fg);
            path.pop();
            path.push(true);
            draw_panes(f, app, b, rb, path, fg);
            path.pop();
        }
    }
}

/// 칸 하나: 탭 바 + 보이는 탭. g = None 이면 나뉘지 않은 화면 (모든 탭)
fn draw_group(f: &mut Frame, app: &mut App, g: Option<usize>, area: Rect, focused: bool) {
    use unicode_width::UnicodeWidthStr;
    let [bar, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(area);
    let idx: Vec<usize> = (0..app.tabs.len()).filter(|&i| g.is_none_or(|g| app.tabs[i].group == g)).collect();
    let shown = if focused { app.active } else { g.and_then(|g| app.group_tab(g)).unwrap_or(app.active) };
    // 탭 위치: Tabs 위젯은 " 제목 " + "│" 순서로 그린다
    let mut x = bar.x;
    for &i in &idx {
        let t = &app.tabs[i];
        let dirty = matches!(&t.kind, TabKind::Table(tt) if tt.grid.as_ref().map(|g| g.dirty()).unwrap_or(false));
        // " " + 아이콘(2) + 제목 + " *"? + " "
        let w = 1 + 2 + t.title.width() as u16 + if dirty { 2 } else { 0 } + 1;
        app.hits.tabs.push((t.id, x, (x + w).min(bar.right()), bar.y));
        x += w + 1;
    }
    app.hits.groups.push((g.unwrap_or(0), bar, body));
    // Data Source 색은 탭 배경으로 (GUI 와 같이). 보이는 탭은 굵게 + 밝게 (포커스 없는 칸은 덜 밝게)
    let titles: Vec<Line> = idx
        .iter()
        .map(|&i| {
            let t = &app.tabs[i];
            let icon = match t.kind {
                TabKind::Console(_) => "⌘ ",
                TabKind::Table(_) => "▦ ",
                TabKind::Text(_) => "ƒ ",
                TabKind::Redis(_) => "◆ ",
            };
            let dirty = matches!(&t.kind, TabKind::Table(tt) if tt.grid.as_ref().map(|g| g.dirty()).unwrap_or(false));
            let c = app.conn_color(&t.conn).and_then(|c| hex(&c));
            let l = Line::from(vec![Span::raw(icon), Span::raw(t.title.clone()), Span::raw(if dirty { " *" } else { "" })]);
            let mut st = Style::default();
            if let Some(c) = c {
                st = st.bg(c);
            }
            if i == shown {
                st = st.add_modifier(Modifier::BOLD);
                st = if focused { st.fg(Color::White).bg(c.map(lighten).unwrap_or(Color::Rgb(46, 67, 110))) } else { st.bg(c.unwrap_or(Color::Rgb(50, 52, 56))).add_modifier(Modifier::UNDERLINED) };
            }
            l.style(st)
        })
        .collect();
    let sel = idx.iter().position(|&i| i == shown).unwrap_or(0);
    f.render_widget(Tabs::new(titles).select(sel).highlight_style(Style::default()).divider("│"), bar);
    draw_pane(f, app, shown, body, focused);
}

/// 탭 하나의 본문. focused = 포커스가 있는 칸 (마우스 판정 영역도 이 칸만 기록)
fn draw_pane(f: &mut Frame, app: &mut App, idx: usize, body: Rect, focused: bool) {
    // 포커스 없는 칸은 Tree 포커스처럼 (테두리·커서를 흐리게) 그린다
    let focus = if focused { app.focus } else { Focus::Tree };
    let ro = app.tabs.get(idx).and_then(|t| app.conn(&t.conn)).map(|c| c.config.read_only).unwrap_or(false);
    let pct = app.console_pct;
    let tab = &mut app.tabs[idx];
    let db = tab.db.clone();
    match &mut tab.kind {
        TabKind::Console(c) => {
            let [ed, res] = Layout::vertical([Constraint::Percentage(pct), Constraint::Percentage(100 - pct)]).areas(body);
            let title = format!(
                " {db} · Tx: {}{}{}{} ",
                if c.tx_manual { "Manual" } else { "Auto" },
                if c.in_tx { disp(" (pending — Alt+C commit / Alt+R rollback)") } else { String::new() },
                if c.running { disp(" · running… Ctrl+C cancel") } else { String::new() },
                if ro { " · read-only" } else { "" }
            );
            let editing = focus == Focus::Editor;
            if !editing {
                c.complete = None;
            }
            c.editor.set_block(block(title, editing));
            c.editor.set_line_number_style(Style::default().fg(DIM));
            c.editor.set_cursor_style(if editing { Style::default().add_modifier(Modifier::REVERSED) } else { Style::default() });
            f.render_widget(&c.editor, ed);
            // 자동완성 목록 위치: 방금 그린 커서 칸 (tui-textarea 는 스크롤 위치를 공개하지 않는다)
            let caret = if editing && c.complete.is_some() { find_caret(f, ed) } else { None };
            if focused {
                app.hits.editor = ed;
                app.hits.results = res;
                app.hits.console = body;
                app.hits.divider = Rect { x: res.x, y: res.y, width: res.width, height: 1 };
            }
            if c.show_output || c.grid.is_none() {
                let lines: Vec<Line> = c
                    .output
                    .iter()
                    .rev()
                    .take(res.height as usize)
                    .rev()
                    .map(|(e, s)| Line::styled(s.clone(), Style::default().fg(if *e { Color::Red } else if s.contains("> ") { ACCENT } else { Color::Gray })))
                    .collect();
                let p = Paragraph::new(if lines.is_empty() { vec![Line::styled(disp("Execute a statement (F5 / Ctrl+Enter) to see the output."), Style::default().fg(DIM))] } else { lines })
                    .wrap(Wrap { trim: false })
                    .block(block(disp(" Output (Alt+O: result) "), focus == Focus::Grid));
                f.render_widget(p, res);
            } else if let Some(g) = &mut c.grid {
                let t = format!(" {} — {}{} rows ({}O: output) ", c.result_title, g.rows(), if g.result.truncated { "+" } else { "" }, disp("Alt+"));
                g.render(f, res, t, focus == Focus::Grid);
            }
            if let (Some((x, y)), Some(cmp)) = (caret, &c.complete) {
                draw_completion(f, cmp, x, y);
            }
        }
        TabKind::Table(tt) => {
            let [filt, grid] = Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(body);
            if focused {
                app.hits.results = grid;
            }
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
            // 쿼리가 실패하면: 표 위에 오류 한 줄과 고칠 방법, 표는 비운 채로 (어떻게 빠져나가는지 보이게)
            let grid = if let Some(e) = &tt.error {
                let [msg, rest] = Layout::vertical([Constraint::Length(2), Constraint::Min(3)]).areas(grid);
                let mut hint = vec![Span::styled(" w ", Style::default().fg(ACCENT)), Span::raw("fix WHERE · "), Span::styled("o ", Style::default().fg(ACCENT)), Span::raw("fix ORDER BY · "), Span::styled("r ", Style::default().fg(ACCENT)), Span::raw("run again")];
                if let Some(n) = tt.grid.as_ref().filter(|g| g.dirty()).map(|g| g.edits.len() + g.deleted.len() + g.added) {
                    hint.push(Span::styled(disp(&format!("   · {n} pending change(s) not saved yet (Ctrl+S submit · Ctrl+Z revert)")), Style::default().fg(Color::Yellow)));
                }
                let first = e.lines().next().unwrap_or_default().to_string();
                f.render_widget(Paragraph::new(vec![Line::styled(format!(" ✗ {first}"), Style::default().fg(Color::Red)), Line::from(hint)]), msg);
                rest
            } else {
                grid
            };
            if let Some(g) = &mut tt.grid {
                let pend = if g.dirty() { disp(&format!(" · {} pending (Ctrl+S submit, Ctrl+Z revert)", g.edits.len() + g.deleted.len() + g.added)) } else { String::new() };
                let rows = if tt.error.is_some() && !g.dirty() { "query failed — no rows".to_string() } else { format!("{}{} rows", g.rows(), if g.result.truncated { "+" } else { "" }) };
                let t = format!(" {}.{} — {rows}{}{}{} ", tt.schema, tt.table, if tt.pk.is_empty() { " · no PK (read-only)" } else { "" }, if ro { " · read-only" } else { "" }, pend);
                g.render(f, grid, t, focus == Focus::Grid);
            } else {
                let msg = if tt.error.is_some() { "" } else { "Loading…" };
                f.render_widget(Paragraph::new(msg).block(block(format!(" {}.{} ", tt.schema, tt.table), focused)), grid);
            }
        }
        TabKind::Text(t) => {
            let p = Paragraph::new(t.text.as_str()).scroll((t.scroll, 0)).block(block(" DDL ", focus != Focus::Tree));
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
            if focused {
                app.hits.redis_keys = inner;
                app.hits.redis_value = right;
            }
            let vf = focus == Focus::Grid && r.focus_value;
            match &mut r.detail {
                None => f.render_widget(Paragraph::new("Select a key").block(block(" Value ", vf)), right),
                Some((key, d)) => {
                    let ttl = if d.ttl >= 0 { format!("TTL {}s", d.ttl / 1000) } else { "no TTL".into() };
                    let title = format!(" {} {key} · {} · {} {} ", d.ty.to_uppercase(), ttl, d.length, if d.ty == "string" { "bytes" } else { "items" });
                    match &mut r.value {
                        Some(g) => g.render(f, right, title, vf),
                        None => {
                            f.render_widget(Paragraph::new(r.text.as_deref().unwrap_or_default()).wrap(Wrap { trim: false }).block(block(title, vf)), right);
                        }
                    }
                }
            }
        }
    }
}

/// 에디터 안에서 커서(반전 칸) 위치
fn find_caret(f: &mut Frame, area: Rect) -> Option<(u16, u16)> {
    let buf = f.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if buf[(x, y)].modifier.contains(Modifier::REVERSED) {
                return Some((x, y));
            }
        }
    }
    None
}

/// 자동완성 목록: 커서 아래 (공간이 없으면 위)
fn draw_completion(f: &mut Frame, cmp: &crate::app::Completion, x: u16, y: u16) {
    use crate::complete::ItemKind;
    use unicode_width::UnicodeWidthStr;
    let screen = f.area();
    let shown = cmp.items.len().min(10);
    let lw = cmp.items.iter().map(|i| i.label.width()).max().unwrap_or(0);
    let dw = cmp.items.iter().map(|i| i.detail.width()).max().unwrap_or(0);
    let w = ((2 + lw + 2 + dw + 2) as u16).clamp(20, 70).min(screen.width);
    let h = shown as u16 + 2;
    let x = x.saturating_sub(cmp.prefix as u16 + 3).min(screen.right().saturating_sub(w));
    let y = if y + 1 + h <= screen.bottom() { y + 1 } else { y.saturating_sub(h) };
    let r = Rect { x, y, width: w, height: h.min(screen.height) };
    let off = cmp.sel.saturating_sub(shown.saturating_sub(1));
    let items: Vec<ListItem> = cmp
        .items
        .iter()
        .enumerate()
        .skip(off)
        .take(shown)
        .map(|(i, it)| {
            let (icon, color) = match it.kind {
                ItemKind::Column => ("· ", Color::Rgb(201, 162, 109)),
                ItemKind::Table => ("▦ ", Color::Rgb(111, 179, 210)),
                ItemKind::Schema => ("◇ ", Color::Gray),
                ItemKind::Keyword => ("k ", Color::Rgb(204, 120, 50)),
            };
            let mut l = Line::from(vec![Span::styled(icon, Style::default().fg(color)), Span::raw(format!("{:<lw$}  ", it.label)), Span::styled(it.detail.clone(), Style::default().fg(DIM))]);
            if i == cmp.sel {
                l = l.style(Style::default().bg(Color::Rgb(46, 67, 110)));
            }
            ListItem::new(l)
        })
        .collect();
    f.render_widget(Clear, r);
    let title = format!(" {}/{} ", cmp.sel + 1, cmp.items.len());
    f.render_widget(List::new(items).block(block(title, true)).style(Style::default().bg(Color::Rgb(30, 31, 34))), r);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let hints = match app.focus {
        Focus::Tree => "↑↓ move · → expand · Enter open · F4 console · Ctrl+B DDL · n new · e edit · / filter",
        Focus::Editor => "F5/Ctrl+Enter run · Ctrl+Space complete · Alt+E explain · Alt+T tx · Shift+Tab result · Ctrl+O object · Ctrl+K actions",
        Focus::Grid => match app.tabs.get(app.active).map(|t| &t.kind) {
            Some(TabKind::Table(_)) => "Enter edit · n NULL · a add · d delete · Ctrl+S submit · s sort · w where · o order · x export",
            Some(TabKind::Redis(_)) => "Enter value · / pattern · m more · e edit · x del item · D del key · t TTL · R rename · : command",
            Some(TabKind::Text(_)) => "arrows move · PgUp/PgDn · y copy · Esc explorer",
            _ => "arrows move · PgUp/PgDn · Home/End · s sort · Esc editor · x export",
        },
    };
    let hints = disp(hints);
    let hints = hints.as_str();
    // 새 버전 알림은 상태 글 앞에 계속 보인다
    let badge = app.update.as_ref().map(|v| format!(" ⬆ v{v} available — {} › Update dbtool & Restart ", app.keys.label(crate::app::Action::Palette)));
    let bw = badge.as_ref().map(|b| b.chars().count() as u16).unwrap_or(0);
    // 폭이 모자라면 오른쪽 힌트를 줄인다 (업데이트 알림이 잘리지 않게)
    let hw = (hints.chars().count() as u16 + 1).min(area.width.saturating_sub(bw + 12));
    let [u, l, r] = Layout::horizontal([Constraint::Length(bw), Constraint::Min(10), Constraint::Length(hw)]).areas(area);
    if let Some(b) = badge {
        f.render_widget(Paragraph::new(b).style(Style::default().fg(Color::Black).bg(Color::Rgb(201, 162, 109))), u);
    }
    f.render_widget(Paragraph::new(format!(" {}", disp(&app.status))).style(Style::default().fg(Color::Gray).bg(Color::Rgb(43, 45, 48))), l);
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
        Popup::Help { sel, scroll, record, message } => {
            let mut lines = vec![Line::styled(
                "Commands — ↑↓ select · Enter change · a add key · Del reset to default",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )];
            for (i, (a, _)) in crate::keys::DEFAULT_KEYS.iter().enumerate() {
                let custom = app.keys.is_custom(*a);
                let keys = if i == *sel && record.is_some() { "press a key… (Esc cancels)".to_string() } else { app.keys.label(*a) };
                let mut l = Line::from(vec![
                    Span::styled(format!("  {keys:<30}"), Style::default().fg(if custom { Color::Yellow } else { ACCENT })),
                    Span::raw(crate::input::name_of(*a).to_string()),
                    Span::styled(if custom { "  (custom)" } else { "" }, Style::default().fg(DIM)),
                ]);
                if i == *sel {
                    l = l.style(Style::default().bg(Color::Rgb(46, 67, 110)));
                }
                lines.push(l);
            }
            lines.push(Line::raw(""));
            // 고정 키 목록: Commands 다음 번호부터 이어서 고를 수 있다 (바꾸지는 못한다)
            let mut i = crate::keys::DEFAULT_KEYS.len();
            let mut sel_line = *sel + 1;
            for (title, rows) in crate::help::HELP {
                lines.push(Line::styled(title.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
                for (k, d) in *rows {
                    let mut l = Line::from(vec![Span::styled(format!("  {:<30}", disp(k)), Style::default().fg(ACCENT)), Span::raw(disp(d))]);
                    if i == *sel {
                        l = l.style(Style::default().bg(Color::Rgb(46, 67, 110)));
                        sel_line = lines.len();
                    }
                    lines.push(l);
                    i += 1;
                }
                lines.push(Line::raw(""));
            }
            let r = centered(area, 104, area.height.saturating_sub(2));
            let h = r.height.saturating_sub(2);
            // 고른 줄이 보이도록
            let row = sel_line as u16;
            if row < *scroll {
                *scroll = row.saturating_sub(1);
            } else if row >= *scroll + h {
                *scroll = row + 1 - h;
            }
            let max = (lines.len() as u16).saturating_sub(h);
            if *scroll > max {
                *scroll = max;
            }
            let title = match message {
                Some(m) => format!(" {m} "),
                None => " Keymap — saved to keymap.json · PgUp PgDn · Esc close ".into(),
            };
            f.render_widget(Clear, r);
            f.render_widget(Paragraph::new(lines).scroll((*scroll, 0)).block(block(title, true)), r);
        }
        Popup::Message { title, text } => {
            let r = centered(area, 90, (text.lines().count() as u16 + 4).min(30));
            f.render_widget(Clear, r);
            f.render_widget(Paragraph::new(text.clone()).wrap(Wrap { trim: false }).block(block(format!(" {title} (Esc) "), true)), r);
        }
        Popup::Confirm { text, .. } => {
            // 여러 줄 (이유 · 문장 · 질문)
            let mut lines: Vec<Line> = text.lines().map(|l| Line::raw(l.to_string())).collect();
            lines.push(Line::raw(""));
            lines.push(Line::styled("y = yes · n / Esc = no", Style::default().fg(DIM)));
            let r = centered(area, 90, lines.len() as u16 + 2);
            f.render_widget(Clear, r);
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(block(" Confirm ", true)), r);
        }
        Popup::Input { title, input, complete, words, .. } => {
            let r = centered(area, 90, 3);
            f.render_widget(Clear, r);
            let hint = if words.is_empty() { "Enter / Esc" } else { "Enter / Esc · Tab complete" };
            input.set_block(block(format!(" {title} ({hint}) "), true));
            f.render_widget(&*input, r);
            // 자동완성 목록: 입력칸 바로 아래, 커서 근처
            if let Some((items, sel, _)) = complete {
                let w = items.iter().map(|s| s.chars().count()).max().unwrap_or(0) as u16 + 4;
                let x = (r.x + 1 + input.cursor().1 as u16).min(r.right().saturating_sub(w));
                let lr = Rect { x, y: r.bottom(), width: w.min(r.width), height: (items.len() as u16 + 2).min(area.bottom().saturating_sub(r.bottom())) };
                let rows: Vec<ListItem> = items
                    .iter()
                    .enumerate()
                    .map(|(i, s)| ListItem::new(format!(" {s}")).style(if i == *sel { Style::default().bg(Color::Rgb(46, 67, 110)) } else { Style::default() }))
                    .collect();
                f.render_widget(Clear, lr);
                f.render_widget(List::new(rows).block(block("", true)).style(Style::default().bg(Color::Rgb(30, 31, 34))), lr);
            }
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
            let hint = if p.title.starts_with("Go to") || p.title == "Search Everywhere" { disp(" Enter open · Ctrl+B DDL · Esc ") } else { " Enter · Esc ".into() };
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
                    let mut spans = vec![
                        Span::styled(format!("{:>14} ", fl.label), Style::default().fg(if sel { ACCENT } else { DIM })),
                        Span::styled(shown + if sel && matches!(fl.kind, FieldKind::Text | FieldKind::Secret) { "▏" } else { "" }, if sel { Style::default().bg(Color::Rgb(46, 67, 110)) } else { Style::default() }),
                    ];
                    if fl.label == "Color" {
                        let c = COLOR_PRESETS.iter().find(|p| p.0 == fl.value).and_then(|p| hex(p.1)).or_else(|| form.edit.as_ref().and_then(|e| e.color.as_deref()).filter(|_| fl.value == "custom").and_then(hex));
                        if let Some(c) = c {
                            spans.push(Span::raw(" "));
                            spans.push(Span::styled("      ", Style::default().bg(c)));
                        }
                    }
                    Line::from(spans)
                })
                .collect();
            lines.push(Line::raw(""));
            if let Some((ok, m)) = &form.message {
                lines.push(Line::styled(m.clone(), Style::default().fg(if *ok { Color::Green } else { Color::Red })));
            }
            lines.push(Line::styled(disp("↑↓/Tab move · type to edit · Space/←→ toggle · Ctrl+T test · Ctrl+S / Enter save · Esc cancel"), Style::default().fg(DIM)));
            let t = if form.edit.is_some() { " Data Source Properties " } else { " New Data Source " };
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(block(t, true)), r);
        }
    }
}
