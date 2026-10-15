//! 결과 그리드: 셀 이동·가로/세로 스크롤·편집 표시 (DataGrip 데이터 에디터의 TUI 판)
use dbtool_core::db::QueryResult;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Row, Table};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use unicode_width::UnicodeWidthStr;

pub struct Grid {
    pub result: QueryResult,
    pub row: usize,
    pub col: usize,
    pub row_off: usize,
    pub col_off: usize,
    /// 서버 커서 세션 (콘솔) — 끝까지 내리면 더 불러온다
    pub more_session: Option<String>,
    pub loading_more: bool,
    /// 편집: (행, 열) → 새 값 (None = NULL)
    pub edits: BTreeMap<(usize, usize), Option<String>>,
    pub deleted: BTreeSet<usize>,
    /// 새로 추가한 행 수 (result.rows 끝에 붙어 있다)
    pub added: usize,
    pub widths: Vec<u16>,
    /// 마지막 렌더 높이 (PgUp/PgDn)
    pub page: usize,
    /// 마지막 렌더 위치 (마우스 클릭): 본문 영역, 보이는 열의 (열 번호, x 시작, x 끝)
    pub body: Rect,
    pub col_x: Vec<(usize, u16, u16)>,
}

pub fn cell_text(v: &Value) -> String {
    match v {
        Value::Null => "<null>".into(),
        Value::String(s) => s.replace('\n', "↵").replace('\t', "→"),
        other => other.to_string(),
    }
}
pub fn raw_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

impl Grid {
    pub fn new(result: QueryResult) -> Self {
        let mut g = Grid { result, row: 0, col: 0, row_off: 0, col_off: 0, more_session: None, loading_more: false, edits: BTreeMap::new(), deleted: BTreeSet::new(), added: 0, widths: vec![], page: 10, body: Rect::default(), col_x: vec![] };
        g.compute_widths();
        g
    }

    pub fn compute_widths(&mut self) {
        let r = &self.result;
        self.widths = r
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let body = r.rows.iter().take(300).map(|row| row.get(i).map(cell_text).unwrap_or_default().width()).max().unwrap_or(0);
                (c.name.width().max(body).clamp(4, 40) + 1) as u16
            })
            .collect();
    }

    pub fn append(&mut self, more: QueryResult) {
        self.result.rows.extend(more.rows);
        self.result.truncated = more.truncated;
    }

    pub fn rows(&self) -> usize {
        self.result.rows.len()
    }
    pub fn cols(&self) -> usize {
        self.result.columns.len()
    }

    /// 편집 반영된 값
    pub fn value(&self, r: usize, c: usize) -> Option<String> {
        if let Some(v) = self.edits.get(&(r, c)) {
            return v.clone();
        }
        self.result.rows.get(r).and_then(|row| row.get(c)).and_then(raw_text)
    }

    pub fn dirty(&self) -> bool {
        !self.edits.is_empty() || !self.deleted.is_empty() || self.added > 0
    }

    pub fn revert(&mut self) {
        self.edits.clear();
        self.deleted.clear();
        let n = self.result.rows.len() - self.added;
        self.result.rows.truncate(n);
        self.added = 0;
        self.row = self.row.min(n.saturating_sub(1));
    }

    pub fn add_row(&mut self) {
        self.result.rows.push(vec![Value::Null; self.cols()]);
        self.added += 1;
        self.row = self.rows() - 1;
    }

    pub fn is_added(&self, r: usize) -> bool {
        r >= self.rows() - self.added
    }

    /// 화면 좌표 → (행, 열)
    pub fn hit(&self, x: u16, y: u16) -> Option<(usize, Option<usize>)> {
        let b = self.body;
        if x < b.x || x >= b.x + b.width || y < b.y || y >= b.y + b.height {
            return None;
        }
        let r = self.row_off + (y - b.y) as usize;
        if r >= self.rows() {
            return None;
        }
        Some((r, self.col_x.iter().find(|(_, x0, x1)| x >= *x0 && x <= *x1).map(|c| c.0)))
    }

    pub fn move_by(&mut self, dr: isize, dc: isize) {
        let rows = self.rows() as isize;
        let cols = self.cols() as isize;
        if rows > 0 {
            self.row = (self.row as isize + dr).clamp(0, rows - 1) as usize;
        }
        if cols > 0 {
            self.col = (self.col as isize + dc).clamp(0, cols - 1) as usize;
        }
    }

    /// 끝 근처면 더 불러올 때
    pub fn wants_more(&self) -> bool {
        self.result.truncated && self.more_session.is_some() && !self.loading_more && self.row + 5 >= self.rows()
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, title: String, focused: bool) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(if focused { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) });
        let inner = block.inner(area);
        f.render_widget(block, area);
        if inner.height < 2 || self.cols() == 0 {
            return;
        }
        let body_h = (inner.height - 1) as usize;
        self.page = body_h.max(1);
        // 세로 스크롤
        if self.row < self.row_off {
            self.row_off = self.row;
        } else if self.row >= self.row_off + body_h {
            self.row_off = self.row + 1 - body_h;
        }
        // 가로 스크롤: 선택 열이 보이도록
        let num_w = (self.rows().max(1).to_string().len() + 1) as u16;
        let avail = inner.width.saturating_sub(num_w + 1);
        if self.col < self.col_off {
            self.col_off = self.col;
        }
        loop {
            let used: u16 = self.widths[self.col_off..=self.col].iter().map(|w| w + 1).sum();
            if used <= avail || self.col_off == self.col {
                break;
            }
            self.col_off += 1;
        }
        let mut cols = vec![];
        let mut used = 0u16;
        for c in self.col_off..self.cols() {
            let w = self.widths[c];
            if used + w + 1 > avail && !cols.is_empty() {
                break;
            }
            used += w + 1;
            cols.push(c);
        }
        // 클릭 판정용 위치 기록
        self.body = Rect { x: inner.x, y: inner.y + 1, width: inner.width, height: inner.height - 1 };
        self.col_x.clear();
        let mut x = inner.x + num_w + 1;
        for &c in &cols {
            self.col_x.push((c, x, x + self.widths[c]));
            x += self.widths[c] + 1;
        }
        let header = Row::new(
            std::iter::once(Cell::from("")).chain(cols.iter().map(|&c| {
                let col = &self.result.columns[c];
                let st = if c == self.col { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().add_modifier(Modifier::BOLD) };
                Cell::from(col.name.clone()).style(st)
            })),
        )
        .style(Style::default().bg(Color::Rgb(43, 45, 48)));
        let numeric: Vec<bool> = self
            .result
            .columns
            .iter()
            .map(|c| {
                let t = c.type_name.to_uppercase();
                ["INT", "NUMERIC", "DECIMAL", "FLOAT", "DOUBLE", "REAL", "SERIAL", "OID"].iter().any(|k| t.contains(k))
            })
            .collect();
        let end = (self.row_off + body_h).min(self.rows());
        let rows: Vec<Row> = (self.row_off..end)
            .map(|r| {
                let deleted = self.deleted.contains(&r);
                let added = self.is_added(r);
                let num = Cell::from(format!("{:>w$}", r + 1, w = num_w as usize - 1)).style(Style::default().fg(if added { Color::Green } else if deleted { Color::Red } else { Color::DarkGray }));
                Row::new(std::iter::once(num).chain(cols.iter().map(|&c| {
                    let edited = self.edits.contains_key(&(r, c));
                    let v = self.value(r, c);
                    let text = match &v {
                        None if added && !edited => "<default>".to_string(),
                        None => "<null>".to_string(),
                        Some(s) => s.replace('\n', "↵").replace('\t', "→"),
                    };
                    let w = self.widths[c] as usize;
                    let text = if numeric[c] && v.is_some() { format!("{text:>w$}") } else { text };
                    let mut st = if v.is_none() { Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC) } else if numeric[c] { Style::default().fg(Color::Rgb(42, 172, 184)) } else { Style::default() };
                    if edited {
                        st = st.fg(Color::Yellow);
                    }
                    if deleted {
                        st = st.fg(Color::Red).add_modifier(Modifier::CROSSED_OUT);
                    }
                    if added {
                        st = st.fg(Color::Green);
                    }
                    if r == self.row && c == self.col {
                        st = st.bg(if focused { Color::Rgb(46, 67, 110) } else { Color::Rgb(50, 52, 56) });
                    } else if r == self.row {
                        st = st.bg(Color::Rgb(38, 40, 46));
                    }
                    Cell::from(text).style(st)
                })))
            })
            .collect();
        let widths: Vec<Constraint> = std::iter::once(Constraint::Length(num_w)).chain(cols.iter().map(|&c| Constraint::Length(self.widths[c]))).collect();
        f.render_widget(Table::new(rows, widths).header(header).column_spacing(1), inner);
    }
}
