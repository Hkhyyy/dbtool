//! 키 입력 → 동작 (DataGrip 키맵을 터미널에 맞게)
use crate::app::{Action, App, Drag, DropTarget, FieldKind, Focus, Popup, SplitDir, TabKind};
use crate::tree::NodeKind;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::Rect;

fn inside(r: Rect, x: u16, y: u16) -> bool {
    r.width > 0 && x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

/// 마우스 휠: 포인터 아래 영역을 스크롤
fn scroll_at(app: &mut App, x: u16, y: u16, d: isize) {
    if app.popup.is_some() {
        return;
    }
    if inside(app.hits.tree, x, y) {
        app.focus = Focus::Tree;
    } else if inside(app.hits.results, x, y) || inside(app.hits.redis_value, x, y) {
        app.focus = Focus::Grid;
        if let Some(TabKind::Redis(r)) = app.tabs.get_mut(app.active).map(|t| &mut t.kind) {
            r.focus_value = true;
        }
    } else if inside(app.hits.redis_keys, x, y) {
        if let Some(TabKind::Redis(r)) = app.tabs.get_mut(app.active).map(|t| &mut t.kind) {
            r.focus_value = false;
            let n = r.keys.len();
            r.sel = (r.sel as isize + d).clamp(0, n.saturating_sub(1) as isize) as usize;
        }
        return;
    } else if inside(app.hits.editor, x, y) {
        app.focus = Focus::Editor;
        if let Some(TabKind::Console(c)) = app.tabs.get_mut(app.active).map(|t| &mut t.kind) {
            let m = if d > 0 { tui_textarea::CursorMove::Down } else { tui_textarea::CursorMove::Up };
            for _ in 0..d.unsigned_abs() {
                c.editor.move_cursor(m);
            }
        }
        return;
    }
    scroll(app, d);
}

/// 가로 스크롤 (트랙패드 좌우·Shift+휠): 트리는 옆으로 밀고, 그리드는 열을 옮긴다
fn hscroll_at(app: &mut App, x: u16, y: u16, d: isize) {
    if app.popup.is_some() {
        return;
    }
    if inside(app.hits.tree, x, y) {
        app.tree.hoff = (app.tree.hoff as isize + d).max(0) as usize;
    } else if inside(app.hits.results, x, y) || inside(app.hits.redis_value, x, y) {
        if let Some(g) = app.grid_mut() {
            g.hscroll(d.signum());
        }
    }
}

/// 포인터가 그리드 헤더나 가로 스크롤바 위인지
fn over_hbar(app: &mut App, x: u16, y: u16) -> bool {
    app.popup.is_none() && app.grid_mut().is_some_and(|g| inside(g.header, x, y) || inside(g.hbar, x, y))
}

/// 끌고 있는 탭을 (x, y) 에 놓으면 어디로 가는지
pub fn drop_target(app: &App, id: u64, x: u16, y: u16) -> Option<DropTarget> {
    let from = app.tab_index(id).map(|i| app.tabs[i].group)?;
    for &(g, bar, body) in &app.hits.groups {
        if inside(bar, x, y) {
            let before = app.hits.tabs.iter().find(|(_, x0, x1, ty)| *ty == y && x >= *x0 && x < *x1).map(|t| t.0);
            return Some(DropTarget::Group(g, before));
        }
        if inside(body, x, y) {
            // 칸의 오른쪽 / 아래쪽 가장자리에 놓으면 그 칸을 나눈다 (최대 4칸. 제 칸에 하나뿐인 탭은 나눠도 그대로라 빼고)
            let alone = g == from && app.tabs.iter().filter(|t| t.group == g).count() < 2;
            if app.panes.leaves().len() < crate::panes::MAX && !alone {
                if x >= body.x + body.width * 2 / 3 {
                    return Some(DropTarget::NewSplit(g, SplitDir::Right));
                }
                if y >= body.y + body.height * 2 / 3 {
                    return Some(DropTarget::NewSplit(g, SplitDir::Down));
                }
            }
            return (g != from).then_some(DropTarget::Group(g, None));
        }
    }
    None
}

fn mouse_down(app: &mut App, x: u16, y: u16) {
    if app.popup.is_none() {
        if inside(app.hits.divider, x, y) {
            app.drag = Some(Drag::Console);
            return;
        }
        if let Some(i) = app.hits.splits.iter().position(|s| inside(s.2, x, y)) {
            app.drag = Some(Drag::Split(i));
            return;
        }
        if inside(app.hits.explorer_divider, x, y) {
            app.drag = Some(Drag::Explorer);
            return;
        }
        if let Some(g) = app.grid_mut().filter(|g| inside(g.hbar, x, y)) {
            g.hscroll_to(x);
            app.drag = Some(Drag::HBar);
            return;
        }
        // 탭: 누르면 고르고, 끌면 옮긴다
        if let Some(&(id, ..)) = app.hits.tabs.iter().find(|(_, x0, x1, ty)| *ty == y && x >= *x0 && x < *x1) {
            if let Some(i) = app.tab_index(id) {
                app.activate(i);
            }
            app.drag = Some(Drag::Tab { id, moved: false });
            app.drag_pos = (x, y);
            return;
        }
        // 상하 분할: 아래 칸 탭 바의 빈 곳도 경계선처럼 잡을 수 있다 (테두리 한 줄만으로는 맞히기 어렵다)
        if let Some(i) = app.hits.splits.iter().position(|s| inside(s.3, x, y)) {
            app.drag = Some(Drag::Split(i));
            return;
        }
        // 포커스 없는 칸을 누르면 그 칸으로
        let focused = app.focused_group();
        if let Some(&(g, ..)) = app.hits.groups.iter().find(|(g, bar, body)| *g != focused && (inside(*bar, x, y) || inside(*body, x, y))) {
            if let Some(i) = app.group_tab(g) {
                app.activate(i);
            }
            return;
        }
    }
    click(app, x, y);
}

fn mouse_drag(app: &mut App, x: u16, y: u16) {
    match app.drag {
        Some(Drag::Console) => {
            let c = app.hits.console;
            if c.height > 0 {
                app.console_pct = ((y.saturating_sub(c.y) as u32 * 100 / c.height as u32) as u16).clamp(10, 90);
            }
        }
        Some(Drag::Split(i)) => {
            let Some((path, a, _, _, dir)) = app.hits.splits.get(i).cloned() else { return };
            let (p, start, len) = match dir {
                SplitDir::Right => (x, a.x, a.width),
                SplitDir::Down => (y, a.y, a.height),
            };
            if let (Some(pct), true) = (app.panes.pct_mut(&path), len > 0) {
                *pct = ((p.saturating_sub(start) as u32 * 100 / len as u32) as u16).clamp(15, 85);
            }
        }
        Some(Drag::Explorer) => {
            let w = x.saturating_sub(app.hits.main.x) as i16 + 1;
            app.set_explorer_width(w);
        }
        Some(Drag::HBar) => {
            if let Some(g) = app.grid_mut() {
                g.hscroll_to(x);
            }
        }
        Some(Drag::Tab { id, moved }) => {
            if moved || (x, y) != app.drag_pos {
                app.drag = Some(Drag::Tab { id, moved: true });
                app.status = "Drop on another tab bar or pane to move · on the right / bottom edge to split".into();
            }
            app.drag_pos = (x, y);
        }
        None => {}
    }
}

fn mouse_up(app: &mut App, x: u16, y: u16) {
    if let Some(Drag::Tab { id, moved: true }) = app.drag {
        app.status.clear();
        if let Some(t) = drop_target(app, id, x, y) {
            app.drop_tab(id, t);
        }
    }
    app.drag = None;
}

/// 같은 곳을 0.4초 안에 두 번 누르면 더블클릭
fn double(app: &mut App, x: u16, y: u16) -> bool {
    let now = std::time::Instant::now();
    let dbl = matches!(app.hits.last_click, Some((t, px, py)) if now.duration_since(t).as_millis() < 400 && px == x && py == y);
    app.hits.last_click = if dbl { None } else { Some((now, x, y)) };
    dbl
}

fn click(app: &mut App, x: u16, y: u16) {
    let dbl = double(app, x, y);
    // 팝업: 팔레트 항목만 클릭으로 고른다 (그 밖은 닫기)
    if let Some(p) = &mut app.popup {
        if let Popup::Palette(pal) = p {
            if inside(app.hits.palette, x, y) {
                let i = app.hits.palette_off + (y - app.hits.palette.y) as usize;
                let items = App::palette_matches(pal);
                if let Some(it) = items.get(i).map(|x| (*x).clone()) {
                    pal.sel = i;
                    app.run_palette(it, false);
                }
            }
        }
        return;
    }
    // 트리
    if inside(app.hits.tree, x, y) {
        app.focus = Focus::Tree;
        let i = app.tree.offset + (y - app.hits.tree.y) as usize;
        // 누른 줄 하나만 복사한다
        let Some((depth, node)) = app.tree.visible().get(i).map(|(d, n)| (*d, (*n).clone())) else { return };
        app.tree.sel = i;
        // ▸ 위를 누르면 펼치기/접기
        let chev = app.hits.tree.x + (depth * 2) as u16;
        if node.expandable && (x == chev || x == chev + 1) {
            app.toggle_node(&node);
        } else if dbl {
            app.activate_node(&node, false);
        }
        return;
    }
    // 에디터
    if inside(app.hits.editor, x, y) {
        app.focus = Focus::Editor;
        return;
    }
    // Redis 키 목록
    if inside(app.hits.redis_keys, x, y) {
        let id = app.tabs[app.active].id;
        if let Some(TabKind::Redis(r)) = app.tabs.get_mut(app.active).map(|t| &mut t.kind) {
            app.focus = Focus::Grid;
            r.focus_value = false;
            let i = r.off + (y - app.hits.redis_keys.y) as usize;
            if let Some(k) = r.keys.get(i).map(|k| k.key.clone()) {
                r.sel = i;
                app.redis_load_key(id, k);
            }
        }
        return;
    }
    // 그리드 (결과 / 테이블 / Redis 값)
    if inside(app.hits.results, x, y) || inside(app.hits.redis_value, x, y) {
        app.focus = Focus::Grid;
        if let Some(TabKind::Redis(r)) = app.tabs.get_mut(app.active).map(|t| &mut t.kind) {
            r.focus_value = true;
        }
        let is_table = matches!(app.tabs.get(app.active).map(|t| &t.kind), Some(TabKind::Table(_)));
        if let Some(g) = app.grid_mut() {
            if let Some((r, c)) = g.hit(x, y) {
                g.row = r;
                if let Some(c) = c {
                    g.col = c;
                }
                if dbl && is_table {
                    grid_key(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }
            }
        }
        app.fetch_more();
    }
}

pub fn on_event(app: &mut App, ev: Event) {
    match ev {
        Event::Key(k) if k.kind != KeyEventKind::Release => on_key(app, k),
        Event::Paste(s) => paste(app, s),
        Event::Mouse(m) => match m.kind {
            // Shift+휠 = 가로 스크롤
            MouseEventKind::ScrollDown if m.modifiers.contains(KeyModifiers::SHIFT) => hscroll_at(app, m.column, m.row, 3),
            MouseEventKind::ScrollUp if m.modifiers.contains(KeyModifiers::SHIFT) => hscroll_at(app, m.column, m.row, -3),
            // ⌥+휠, 또는 그리드 헤더·스크롤바 위에서 휠 = 가로 스크롤 (가로 휠을 안 넘기는 터미널용)
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp if m.modifiers.contains(KeyModifiers::ALT) || over_hbar(app, m.column, m.row) => {
                hscroll_at(app, m.column, m.row, if m.kind == MouseEventKind::ScrollDown { 1 } else { -1 })
            }
            MouseEventKind::ScrollDown => scroll_at(app, m.column, m.row, 3),
            MouseEventKind::ScrollUp => scroll_at(app, m.column, m.row, -3),
            MouseEventKind::ScrollRight => hscroll_at(app, m.column, m.row, 3),
            MouseEventKind::ScrollLeft => hscroll_at(app, m.column, m.row, -3),
            MouseEventKind::Down(MouseButton::Left) => mouse_down(app, m.column, m.row),
            MouseEventKind::Drag(MouseButton::Left) => mouse_drag(app, m.column, m.row),
            MouseEventKind::Up(MouseButton::Left) => mouse_up(app, m.column, m.row),
            _ => {}
        },
        _ => {}
    }
}

/// ⌘V / 터미널 붙여넣기: 열린 입력칸(한 줄 입력·팔레트·Data Source 폼)이 있으면 거기로, 아니면 콘솔 에디터로
fn paste(app: &mut App, s: String) {
    // 한 줄 입력칸에는 줄바꿈을 공백으로
    let one = || s.trim_end_matches(['\r', '\n']).replace("\r\n", " ").replace(['\r', '\n'], " ");
    match &mut app.popup {
        Some(Popup::Input { input, .. }) => {
            input.insert_str(one());
        }
        Some(Popup::Palette(p)) => {
            if p.input.insert_str(one()) {
                p.sel = 0;
            }
        }
        Some(Popup::ConnForm(form)) => {
            let f = &mut form.fields[form.sel];
            if matches!(f.kind, FieldKind::Text | FieldKind::Secret) {
                if f.value == "<saved>" {
                    f.value.clear();
                }
                f.value.push_str(&one());
            }
        }
        Some(_) => {}
        None => {
            if let Some(TabKind::Console(c)) = app.tabs.get_mut(app.active).map(|t| &mut t.kind) {
                if app.focus == Focus::Editor {
                    c.editor.insert_str(s);
                }
            }
        }
    }
}

fn scroll(app: &mut App, d: isize) {
    match app.focus {
        Focus::Tree => {
            let n = app.tree.visible().len();
            app.tree.sel = (app.tree.sel as isize + d).clamp(0, n.saturating_sub(1) as isize) as usize;
        }
        _ => {
            if let Some(g) = app.grid_mut() {
                g.move_by(d, 0);
            }
            app.fetch_more();
        }
    }
}

pub fn on_key(app: &mut App, k: KeyEvent) {
    if app.popup.is_some() {
        return popup_key(app, k);
    }
    // ── 전역 명령 (키맵: keys.rs, 바꾸기: F1) ──
    if let Some(a) = app.keys.lookup(&k) {
        // 탐색기에서 실행 키 = 새로 고침
        if a == Action::Execute && app.focus == Focus::Tree {
            return app.run_action(Action::Refresh);
        }
        return app.run_action(a);
    }
    match app.focus {
        Focus::Tree => tree_key(app, k),
        Focus::Editor => editor_key(app, k),
        Focus::Grid => grid_key(app, k),
    }
}

fn tree_key(app: &mut App, k: KeyEvent) {
    let n = app.tree.visible().len();
    // 스피드 서치 입력 중
    if !app.tree.filter.is_empty() || k.code == KeyCode::Char('/') {
        match k.code {
            // 필터가 걸려 있으면 지금 필터를 채워서 연다 (비우고 Enter = 해제)
            KeyCode::Char('/') => {
                let cur = app.tree.filter.clone();
                app.input("Speed search (filters loaded nodes · empty = clear)", &cur, |a, s| {
                    a.tree.filter = s.trim().to_string();
                    a.tree.sel = 0;
                    a.tree.clamp();
                });
                return;
            }
            KeyCode::Esc => {
                app.tree.filter.clear();
                app.tree.clamp();
                return;
            }
            _ => {}
        }
    }
    let sel = app.tree.selected();
    match k.code {
        // Ctrl+↓ (⌘↓ Edit Data / Jump to Source)
        KeyCode::Down if k.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(nd) = sel {
                app.activate_node(&nd, true)
            }
        }
        KeyCode::Down | KeyCode::Char('j') => app.tree.sel = (app.tree.sel + 1).min(n.saturating_sub(1)),
        KeyCode::Up | KeyCode::Char('k') => app.tree.sel = app.tree.sel.saturating_sub(1),
        KeyCode::PageDown => app.tree.sel = (app.tree.sel + 20).min(n.saturating_sub(1)),
        KeyCode::PageUp => app.tree.sel = app.tree.sel.saturating_sub(20),
        KeyCode::Home => app.tree.sel = 0,
        KeyCode::End => app.tree.sel = n.saturating_sub(1),
        KeyCode::Right if k.modifiers.contains(KeyModifiers::SHIFT) => app.tree.hoff += 4,
        KeyCode::Left if k.modifiers.contains(KeyModifiers::SHIFT) => app.tree.hoff = app.tree.hoff.saturating_sub(4),
        KeyCode::Right | KeyCode::Char('l') => {
            if let Some(nd) = sel {
                if nd.expandable && !app.tree.expanded.contains(&nd.key) {
                    app.toggle_node(&nd);
                } else if nd.expandable {
                    app.tree.sel = (app.tree.sel + 1).min(n.saturating_sub(1));
                }
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if let Some(nd) = sel {
                if app.tree.expanded.contains(&nd.key) {
                    app.toggle_node(&nd);
                } else if let Some(p) = app.tree.parent_index() {
                    app.tree.sel = p;
                }
            }
        }
        KeyCode::Enter => {
            if let Some(nd) = sel {
                app.activate_node(&nd, false)
            }
        }
        KeyCode::F(4) => {
            if let Some(nd) = sel {
                app.activate_node(&nd, true)
            }
        }
        KeyCode::Char('n') => app.run_action(Action::NewDataSource),
        KeyCode::Char('e') => app.run_action(Action::EditDataSource),
        KeyCode::Char('D') => app.run_action(Action::DeleteDataSource),
        KeyCode::Char('c') => app.run_action(Action::Connect),
        KeyCode::Char('X') => app.run_action(Action::Disconnect),
        KeyCode::Char('r') => app.run_action(Action::Refresh),
        KeyCode::Char('?') => app.run_action(Action::Help),
        KeyCode::Char('y') => {
            if let Some(nd) = sel {
                let r = match (&nd.schema, &nd.name) {
                    (Some(s), Some(n)) if !matches!(nd.kind, NodeKind::Column { .. }) => format!("{s}.{n}"),
                    (_, Some(n)) => n.clone(),
                    _ => nd.label.clone(),
                };
                copy(app, &r);
            }
        }
        KeyCode::Tab | KeyCode::Esc => {
            if !app.tabs.is_empty() {
                app.activate(app.active);
            }
        }
        _ => {}
    }
}

/// OSC 52 로 클립보드 복사 (대부분의 최신 터미널 지원)
pub fn copy(app: &mut App, text: &str) {
    use std::io::Write;
    let b64 = base64_encode(text.as_bytes());
    let _ = write!(std::io::stdout(), "\x1b]52;c;{b64}\x07");
    let _ = std::io::stdout().flush();
    app.status = format!("Copied: {}", text.chars().take(60).collect::<String>());
}
fn base64_encode(b: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn editor_key(app: &mut App, k: KeyEvent) {
    let Some(TabKind::Console(c)) = app.tabs.get_mut(app.active).map(|t| &mut t.kind) else {
        app.focus = Focus::Grid;
        return grid_key(app, k);
    };
    // 자동완성 목록이 열려 있을 때
    if let Some(cmp) = &mut c.complete {
        let n = cmp.items.len();
        match k.code {
            KeyCode::Up => return cmp.sel = (cmp.sel + n - 1) % n,
            KeyCode::Down => return cmp.sel = (cmp.sel + 1) % n,
            KeyCode::PageUp => return cmp.sel = cmp.sel.saturating_sub(10),
            KeyCode::PageDown => return cmp.sel = (cmp.sel + 10).min(n - 1),
            KeyCode::Enter | KeyCode::Tab => return app.accept_completion(),
            KeyCode::Esc => return c.complete = None,
            _ => {}
        }
    }
    let open = c.complete.is_some();
    match k.code {
        // Esc: 탐색기로, Tab(빈 곳 아님)은 들여쓰기이므로 결과로 가는 건 Shift+Tab/F6
        KeyCode::Esc => {
            app.explorer = true;
            app.focus = Focus::Tree;
        }
        KeyCode::BackTab => app.focus = Focus::Grid,
        _ => {
            c.editor.input(k);
        }
    }
    // 단어나 '.' 을 치면 열고(GUI 처럼), 열린 채로 지우면 다시 거른다. 그 밖의 키는 닫는다
    // (macOS 는 Ctrl+Space 가 입력 소스 전환이라 타이핑으로 여는 게 기본)
    let plain = !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    let typed = plain && matches!(k.code, KeyCode::Char(ch) if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.');
    if typed || (open && k.code == KeyCode::Backspace) {
        app.update_completion(false);
    } else if let Some(TabKind::Console(c)) = app.tabs.get_mut(app.active).map(|t| &mut t.kind) {
        c.complete = None;
    }
}

fn grid_key(app: &mut App, k: KeyEvent) {
    let Some(t) = app.tabs.get_mut(app.active) else {
        app.focus = Focus::Tree;
        return;
    };
    let id = t.id;
    match &mut t.kind {
        TabKind::Text(tt) => match k.code {
            KeyCode::Down | KeyCode::Char('j') => tt.scroll = tt.scroll.saturating_add(1),
            KeyCode::Up | KeyCode::Char('k') => tt.scroll = tt.scroll.saturating_sub(1),
            KeyCode::PageDown => tt.scroll = tt.scroll.saturating_add(20),
            KeyCode::PageUp => tt.scroll = tt.scroll.saturating_sub(20),
            KeyCode::Char('y') => {
                let s = tt.text.clone();
                copy(app, &s)
            }
            KeyCode::Esc => app.focus = Focus::Tree,
            _ => {}
        },
        TabKind::Redis(_) => redis_key(app, id, k),
        TabKind::Console(c) => {
            if matches!(k.code, KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab) {
                app.focus = Focus::Editor;
                return;
            }
            if c.show_output || c.grid.is_none() {
                return;
            }
            nav(app, k);
        }
        TabKind::Table(tt) => {
            match k.code {
                KeyCode::Char('w') => {
                    let w = tt.where_.clone();
                    app.input("WHERE", &w, move |a, s| {
                        if let Some(TabKind::Table(tt)) = a.tab_mut(id).map(|t| &mut t.kind) {
                            tt.where_ = s;
                        }
                        a.reload_table_checked(id);
                    });
                }
                KeyCode::Char('o') => {
                    let o = tt.order.clone();
                    app.input("ORDER BY", &o, move |a, s| {
                        if let Some(TabKind::Table(tt)) = a.tab_mut(id).map(|t| &mut t.kind) {
                            tt.order = s;
                        }
                        a.reload_table_checked(id);
                    });
                }
                KeyCode::Enter | KeyCode::F(2) => {
                    let Some(g) = &tt.grid else { return };
                    let (r, c) = (g.row, g.col);
                    if g.cols() == 0 {
                        return;
                    }
                    let name = g.result.columns[c].name.clone();
                    let cur = g.value(r, c).unwrap_or_default();
                    app.input(format!("{name} (row {})", r + 1), &cur, move |a, s| {
                        if let Some(TabKind::Table(tt)) = a.tab_mut(id).map(|t| &mut t.kind) {
                            if let Some(g) = &mut tt.grid {
                                let orig = g.result.rows.get(r).and_then(|row| row.get(c)).and_then(crate::grid::raw_text);
                                if orig.as_deref() == Some(s.as_str()) {
                                    g.edits.remove(&(r, c));
                                } else {
                                    g.edits.insert((r, c), Some(s));
                                }
                            }
                        }
                    });
                }
                KeyCode::Char('n') => app.run_action(Action::SetNull),
                KeyCode::Char('a') | KeyCode::Insert => app.run_action(Action::AddRow),
                KeyCode::Char('d') | KeyCode::Delete => app.run_action(Action::DeleteRow),
                KeyCode::Char('z') if k.modifiers.contains(KeyModifiers::CONTROL) => app.run_action(Action::Revert),
                KeyCode::Char('r') => app.reload_table_checked(id),
                KeyCode::Char('s') => app.sort_table(id),
                KeyCode::Char('x') => app.run_action(Action::ExportCsv),
                KeyCode::Char('/') => {
                    let w = tt.where_.clone();
                    app.input("WHERE", &w, move |a, s| {
                        if let Some(TabKind::Table(tt)) = a.tab_mut(id).map(|t| &mut t.kind) {
                            tt.where_ = s;
                        }
                        a.reload_table_checked(id);
                    });
                }
                KeyCode::Esc => app.focus = Focus::Tree,
                _ => nav(app, k),
            }
        }
    }
}

/// 그리드 이동
fn nav(app: &mut App, k: KeyEvent) {
    let Some(g) = app.grid_mut() else { return };
    let page = g.page as isize;
    match k.code {
        KeyCode::Right if k.modifiers.contains(KeyModifiers::SHIFT) => g.hscroll(1),
        KeyCode::Left if k.modifiers.contains(KeyModifiers::SHIFT) => g.hscroll(-1),
        KeyCode::Down | KeyCode::Char('j') => g.move_by(1, 0),
        KeyCode::Up | KeyCode::Char('k') => g.move_by(-1, 0),
        KeyCode::Right | KeyCode::Char('l') => g.move_by(0, 1),
        KeyCode::Left | KeyCode::Char('h') => g.move_by(0, -1),
        KeyCode::PageDown => g.move_by(page, 0),
        KeyCode::PageUp => g.move_by(-page, 0),
        KeyCode::Home => g.col = 0,
        KeyCode::End => g.col = g.cols().saturating_sub(1),
        KeyCode::Char('g') => g.row = 0,
        KeyCode::Char('G') => g.row = g.rows().saturating_sub(1),
        KeyCode::Char('y') => {
            let v = g.value(g.row, g.col).unwrap_or_else(|| "NULL".into());
            copy(app, &v);
            return;
        }
        KeyCode::Char('x') => return app.run_action(Action::ExportCsv),
        KeyCode::Char('s') => {
            g.toggle_local_sort();
            if g.result.truncated {
                app.status = "Sorted the loaded rows only".into();
            }
            return;
        }
        _ => return,
    }
    app.fetch_more();
}

fn redis_key(app: &mut App, id: u64, k: KeyEvent) {
    let Some(TabKind::Redis(r)) = app.tab_mut(id).map(|t| &mut t.kind) else { return };
    let cur_key = r.detail.as_ref().map(|d| d.0.clone());
    let cur_ty = r.detail.as_ref().map(|d| d.1.ty.clone()).unwrap_or_default();
    if r.focus_value {
        match k.code {
            KeyCode::Esc | KeyCode::Tab | KeyCode::Left if r.value.is_none() || k.code != KeyCode::Left => {
                r.focus_value = false;
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                let Some(key) = cur_key else { return };
                // 선택 요소 편집
                let (field, val) = match (&r.value, &r.detail) {
                    (Some(g), _) => (g.value(g.row, 0).unwrap_or_default(), g.value(g.row, g.cols().saturating_sub(1)).unwrap_or_default()),
                    (None, Some((_, d))) => (String::new(), d.value.as_str().unwrap_or("").to_string()),
                    _ => return,
                };
                let ty = cur_ty.clone();
                let title = match ty.as_str() {
                    "hash" => format!("HSET {key} {field}"),
                    "list" => format!("LSET {key} {field}"),
                    "zset" => format!("score of {field}"),
                    "set" => "replace member".into(),
                    "string" => format!("SET {key}"),
                    _ => return,
                };
                let init = if ty == "set" { field.clone() } else { val };
                app.input(title, &init, move |a, v| {
                    let cmds: Vec<Vec<String>> = match ty.as_str() {
                        "hash" => vec![vec!["HSET".into(), key.clone(), field.clone(), v]],
                        "list" => vec![vec!["LSET".into(), key.clone(), field.clone(), v]],
                        "zset" => vec![vec!["ZADD".into(), key.clone(), v, field.clone()]],
                        "set" => vec![vec!["SREM".into(), key.clone(), field.clone()], vec!["SADD".into(), key.clone(), v]],
                        "string" => vec![vec!["SET".into(), key.clone(), v, "KEEPTTL".into()]],
                        _ => vec![],
                    };
                    for c in cmds {
                        a.redis_cmd(id, c, false);
                    }
                });
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                let (Some(key), Some(g)) = (cur_key, &r.value) else { return };
                let item = g.value(g.row, 0).unwrap_or_default();
                let cmd: Vec<String> = match cur_ty.as_str() {
                    "hash" => vec!["HDEL".into(), key, item],
                    "set" => vec!["SREM".into(), key, item],
                    "zset" => vec!["ZREM".into(), key, item],
                    "stream" => vec!["XDEL".into(), key, item],
                    "list" => {
                        // 인덱스 위치를 표식으로 바꾼 뒤 LREM
                        let mark = format!("__dbtool_del_{}", uuid::Uuid::new_v4());
                        app.redis_cmd(id, vec!["LSET".into(), key.clone(), item, mark.clone()], false);
                        vec!["LREM".into(), key, "1".into(), mark]
                    }
                    _ => return,
                };
                app.popup = Some(Popup::Confirm { text: format!("{}?", cmd.join(" ")), on_yes: Box::new(move |a| a.redis_cmd(id, cmd, false)) });
            }
            KeyCode::Char('a') => {
                let Some(key) = cur_key else { return };
                let ty = cur_ty.clone();
                let hint = match ty.as_str() {
                    "hash" => "field value",
                    "zset" => "score member",
                    "stream" => "field value",
                    _ => "value",
                };
                app.input(format!("Add to {key} ({hint})"), "", move |a, v| {
                    let parts: Vec<String> = v.splitn(2, ' ').map(String::from).collect();
                    let two = |p: &Vec<String>| p.len() == 2;
                    let cmd: Vec<String> = match ty.as_str() {
                        "hash" if two(&parts) => vec!["HSET".into(), key.clone(), parts[0].clone(), parts[1].clone()],
                        "zset" if two(&parts) => vec!["ZADD".into(), key.clone(), parts[0].clone(), parts[1].clone()],
                        "stream" if two(&parts) => vec!["XADD".into(), key.clone(), "*".into(), parts[0].clone(), parts[1].clone()],
                        "list" => vec!["RPUSH".into(), key.clone(), v],
                        "set" => vec!["SADD".into(), key.clone(), v],
                        _ => {
                            a.status = format!("Expected: {hint}");
                            return;
                        }
                    };
                    a.redis_cmd(id, cmd, false);
                });
            }
            _ => {
                if r.value.is_some() {
                    nav(app, k)
                }
            }
        }
        return;
    }
    let n = r.keys.len();
    match k.code {
        KeyCode::Down | KeyCode::Char('j') => {
            r.sel = (r.sel + 1).min(n.saturating_sub(1));
            if let Some(k) = r.keys.get(r.sel).map(|x| x.key.clone()) {
                app.redis_load_key(id, k)
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            r.sel = r.sel.saturating_sub(1);
            if let Some(k) = r.keys.get(r.sel).map(|x| x.key.clone()) {
                app.redis_load_key(id, k)
            }
        }
        KeyCode::Enter | KeyCode::Right | KeyCode::Tab => r.focus_value = true,
        KeyCode::Char('/') => {
            let p = r.pattern.clone();
            app.input("Key pattern (SCAN MATCH)", &p, move |a, s| {
                if let Some(TabKind::Redis(r)) = a.tab_mut(id).map(|t| &mut t.kind) {
                    r.pattern = if s.trim().is_empty() { "*".into() } else { s };
                }
                a.redis_scan(id, true);
            });
        }
        KeyCode::Char('m') => app.redis_scan(id, false),
        KeyCode::Char('r') => app.redis_scan(id, true),
        KeyCode::Char('D') | KeyCode::Delete => {
            if let Some(key) = cur_key {
                app.popup = Some(Popup::Confirm { text: format!("Delete key \"{key}\"?"), on_yes: Box::new(move |a| {
                    if let Some(TabKind::Redis(r)) = a.tab_mut(id).map(|t| &mut t.kind) {
                        r.detail = None;
                        r.value = None;
                    }
                    a.redis_cmd(id, vec!["DEL".into(), key], true)
                }) });
            }
        }
        KeyCode::Char('R') => {
            if let Some(key) = cur_key {
                let k2 = key.clone();
                app.input("Rename key", &key, move |a, to| {
                    if to != k2 {
                        if let Some(TabKind::Redis(r)) = a.tab_mut(id).map(|t| &mut t.kind) {
                            r.detail = None;
                        }
                        a.redis_cmd(id, vec!["RENAME".into(), k2, to], true);
                    }
                });
            }
        }
        KeyCode::Char('t') => {
            if let Some(key) = cur_key {
                let ttl = r.detail.as_ref().map(|d| if d.1.ttl >= 0 { (d.1.ttl / 1000).to_string() } else { String::new() }).unwrap_or_default();
                app.input("TTL seconds (empty = no expiry)", &ttl, move |a, s| {
                    let cmd = if s.trim().is_empty() { vec!["PERSIST".into(), key] } else { vec!["EXPIRE".into(), key, s.trim().into()] };
                    a.redis_cmd(id, cmd, true);
                });
            }
        }
        KeyCode::Char('a') => {
            app.input("Add key: SET key value | HSET key field value | RPUSH key v | SADD key m | ZADD key score m", "SET ", move |a, line| {
                let st = a.state.clone();
                let target = a.tab_mut(id).map(|t| t.target.clone()).unwrap_or_default();
                a.spawn(async move { st.redis_exec_line(target, line).await.map_err(|e| e.to_string()) }, move |a, r| match r {
                    Ok(v) => {
                        a.status = crate::ui::format_reply(&v);
                        a.redis_scan(id, true);
                    }
                    Err(e) => a.status = e,
                });
            });
        }
        KeyCode::Char(':') => {
            app.input("redis>", "", move |a, line| {
                let run = move |a: &mut App, line: String| {
                    let st = a.state.clone();
                    let target = a.tab_mut(id).map(|t| t.target.clone()).unwrap_or_default();
                    let shown = line.clone();
                    a.spawn(async move { st.redis_exec_line(target, line).await.map_err(|e| e.to_string()) }, move |a, r| match r {
                        Ok(v) => a.popup = Some(Popup::Message { title: shown, text: crate::ui::format_reply(&v) }),
                        Err(e) => a.popup = Some(Popup::Message { title: shown, text: format!("(error) {e}") }),
                    });
                };
                // FLUSHALL·FLUSHDB·DEL 같은 지우는 명령은 한 번 더 묻는다
                match dbtool_core::guard::unsafe_statement(&line, dbtool_core::store::DbKind::Redis) {
                    Some(why) => a.popup = Some(Popup::Confirm { text: format!("{why}:\n  {line}\nRun it anyway?"), on_yes: Box::new(move |a| run(a, line)) }),
                    None => run(a, line),
                }
            });
        }
        KeyCode::Esc => app.focus = Focus::Tree,
        _ => {}
    }
}

fn popup_key(app: &mut App, k: KeyEvent) {
    let popup = app.popup.take().unwrap();
    match popup {
        Popup::Help { mut sel, scroll, record: Some(add), .. } => {
            // 새 키 녹화 중: Esc = 취소
            let message = if k.code == KeyCode::Esc {
                None
            } else {
                let a = crate::keys::DEFAULT_KEYS[sel].0;
                match crate::keys::spec(&k).filter(|s| crate::keys::bindable(s)) {
                    None => Some(format!("{} can't be a command key — use a {} combination or an F key", crate::keys::spec(&k).map(|s| crate::keys::disp(&s)).unwrap_or_else(|| "That key".into()), if cfg!(target_os = "macos") { "⌃/⌥" } else { "Ctrl/Alt" })),
                    Some(key) => {
                        let taken = app.keys.bind(a, key.clone(), add);
                        let mut m = format!("{} → {}", name_of(a), app.keys.label(a));
                        if let Some(t) = taken {
                            m += &format!(" · removed {key} from {}", name_of(t));
                        }
                        if let Err(e) = app.save_keys() {
                            m = format!("Save failed: {e}");
                        }
                        Some(m)
                    }
                }
            };
            sel = sel.min(crate::keys::DEFAULT_KEYS.len() - 1);
            app.popup = Some(Popup::Help { sel, scroll, record: None, message });
        }
        Popup::Help { sel, scroll, message, .. } => {
            let n = crate::keys::DEFAULT_KEYS.len();
            let last = n + crate::help::HELP.iter().map(|s| s.1.len()).sum::<usize>() - 1;
            let (mut sel, mut record) = (sel, None);
            let mut message = message;
            let fixed = || Some("Fixed key — only the Commands at the top can be changed".to_string());
            match k.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(1) => return,
                KeyCode::Down | KeyCode::Char('j') => sel = (sel + 1).min(last),
                KeyCode::Up | KeyCode::Char('k') => sel = sel.saturating_sub(1),
                KeyCode::PageDown | KeyCode::Char(' ') => sel = (sel + 15).min(last),
                KeyCode::PageUp => sel = sel.saturating_sub(15),
                KeyCode::Home | KeyCode::Char('g') => sel = 0,
                KeyCode::End | KeyCode::Char('G') => sel = last,
                KeyCode::Enter if sel < n => record = Some(false),
                KeyCode::Char('a') if sel < n => record = Some(true),
                KeyCode::Enter | KeyCode::Char('a') | KeyCode::Delete | KeyCode::Backspace if sel >= n => message = fixed(),
                KeyCode::Delete | KeyCode::Backspace => {
                    let a = crate::keys::DEFAULT_KEYS[sel].0;
                    app.keys.reset(a);
                    message = Some(match app.save_keys() {
                        Ok(()) => format!("{} reset to {}", name_of(a), app.keys.label(a)),
                        Err(e) => format!("Save failed: {e}"),
                    });
                }
                _ => {}
            }
            app.popup = Some(Popup::Help { sel, scroll, record, message });
        }
        Popup::Message { .. } => {
            if !matches!(k.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::F(1)) {
                app.popup = Some(popup);
            }
        }
        Popup::Confirm { text, on_yes } => match k.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => on_yes(app),
            KeyCode::Char('n') | KeyCode::Esc => {}
            _ => app.popup = Some(Popup::Confirm { text, on_yes }),
        },
        Popup::Input { title, mut input, on_ok } => match k.code {
            KeyCode::Esc => {}
            KeyCode::Enter => on_ok(app, input.lines().join("")),
            _ => {
                input.input(k);
                app.popup = Some(Popup::Input { title, input, on_ok });
            }
        },
        Popup::Palette(mut p) => {
            let n = App::palette_matches(&p).len();
            match k.code {
                KeyCode::Esc => return,
                KeyCode::Down => p.sel = (p.sel + 1).min(n.saturating_sub(1)),
                KeyCode::Up => p.sel = p.sel.saturating_sub(1),
                KeyCode::PageDown => p.sel = (p.sel + 15).min(n.saturating_sub(1)),
                KeyCode::PageUp => p.sel = p.sel.saturating_sub(15),
                KeyCode::Enter => {
                    if let Some(it) = App::palette_matches(&p).get(p.sel).map(|x| (*x).clone()) {
                        return app.run_palette(it, false);
                    }
                }
                KeyCode::Char('b') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(it) = App::palette_matches(&p).get(p.sel).map(|x| (*x).clone()) {
                        return app.run_palette(it, true);
                    }
                }
                _ => {
                    if p.input.input(k) {
                        p.sel = 0;
                    }
                }
            }
            app.popup = Some(Popup::Palette(p));
        }
        Popup::ConnForm(mut form) => {
            let n = form.fields.len();
            match k.code {
                KeyCode::Esc => return,
                KeyCode::Down | KeyCode::Tab => form.sel = (form.sel + 1) % n,
                KeyCode::Up | KeyCode::BackTab => form.sel = (form.sel + n - 1) % n,
                // SSH 처음 보는 서버: 지문을 확인한 뒤 신뢰
                KeyCode::Char('y') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    let v = |l: &str| form.fields.iter().find(|f| f.label == l).map(|f| f.value.trim().to_string()).unwrap_or_default();
                    let port = v("SSH port").parse().unwrap_or(22);
                    form.message = Some(match app.state.trust_ssh_host(v("SSH host"), port) {
                        Ok(m) => (true, format!("✓ {m} — press Ctrl+T to test again")),
                        Err(e) => (false, format!("✕ {e}")),
                    });
                }
                KeyCode::Char('t') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    match App::form_config(&form) {
                        Ok((cfg, pw, ssh)) => {
                            let st = app.state.clone();
                            form.message = Some((true, "Testing…".into()));
                            app.spawn(async move { st.test_connection(cfg, pw, ssh).await.map_err(|e| e.to_string()) }, |a, r| {
                                if let Some(Popup::ConnForm(f)) = &mut a.popup {
                                    f.message = Some(match r {
                                        Ok(v) => (true, format!("✓ {v}")),
                                        Err(e) if e.contains(dbtool_core::tunnel::UNKNOWN_HOST) => (false, format!("✕ {e} Press Ctrl+Y to trust it.")),
                                        Err(e) => (false, format!("✕ {e}")),
                                    });
                                }
                            });
                        }
                        Err(e) => form.message = Some((false, e)),
                    }
                }
                KeyCode::Char('s') if k.modifiers.contains(KeyModifiers::CONTROL) => return save_form(app, form),
                KeyCode::Enter => return save_form(app, form),
                KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if !matches!(form.fields[form.sel].kind, FieldKind::Text | FieldKind::Secret) => {
                    let f = &mut form.fields[form.sel];
                    match &f.kind {
                        FieldKind::Toggle => f.value = if f.value == "on" { "off".into() } else { "on".into() },
                        FieldKind::Choice(opts) => {
                            let i = opts.iter().position(|o| *o == f.value).unwrap_or(0);
                            let j = if k.code == KeyCode::Left { (i + opts.len() - 1) % opts.len() } else { (i + 1) % opts.len() };
                            f.value = opts[j].to_string();
                            // DB 종류를 바꾸면 기본 포트도
                            if f.label == "Type" {
                                let port = match f.value.as_str() {
                                    "mysql" => "3306",
                                    "redis" => "6379",
                                    _ => "5432",
                                };
                                if let Some(p) = form.fields.iter_mut().find(|x| x.label == "Port") {
                                    if ["5432", "3306", "6379", ""].contains(&p.value.as_str()) {
                                        p.value = port.into();
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                KeyCode::Backspace => {
                    let f = &mut form.fields[form.sel];
                    if f.value == "<saved>" {
                        f.value.clear();
                    } else {
                        f.value.pop();
                    }
                }
                KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                    let f = &mut form.fields[form.sel];
                    if matches!(f.kind, FieldKind::Text | FieldKind::Secret) {
                        if f.value == "<saved>" {
                            f.value.clear();
                        }
                        f.value.push(c);
                    }
                }
                _ => {}
            }
            app.popup = Some(Popup::ConnForm(form));
        }
    }
}

fn save_form(app: &mut App, mut form: crate::app::ConnForm) {
    match App::form_config(&form) {
        Ok((cfg, pw, ssh)) => {
            let st = app.state.clone();
            app.spawn(async move { st.save_connection(cfg, pw, ssh).await.map_err(|e| e.to_string()) }, |a, r| match r {
                Ok(c) => {
                    a.status = format!("Saved data source {}", c.name);
                    a.tree.forget(&c.id);
                    a.tree.expanded.remove(&c.id);
                    a.reload_conns();
                }
                Err(e) => a.status = format!("Save failed: {e}"),
            });
        }
        Err(e) => {
            form.message = Some((false, e));
            app.popup = Some(Popup::ConnForm(form));
        }
    }
}

/// 팔레트·키맵에 보이는 명령 이름
pub fn name_of(a: Action) -> &'static str {
    crate::app::ACTIONS.iter().find(|x| x.0 == a).map(|x| x.1).unwrap_or("?")
}
