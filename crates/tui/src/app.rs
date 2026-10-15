//! TUI 상태와 동작. 실제 DB 작업은 전부 dbtool_core::app::AppState (GUI 와 같은 엔진).
use crate::grid::{raw_text, Grid};
use crate::tree::{Node, NodeKind, Tree};
use dbtool_core::app::{AppState, ConnectionView};
use dbtool_core::db::{ColumnInfo, QueryResult};
use dbtool_core::meta::{IndexEntry, SchemaObjects};
use dbtool_core::redisdb::{KeyDetail, KeyRow};
use dbtool_core::sql;
use dbtool_core::store::{ConnectionConfig, DbKind, RedisMode, SshAuth, SshConfig, SslConfig};
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::TextArea;

pub type Apply = Box<dyn FnOnce(&mut App) + Send>;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Focus {
    Tree,
    Editor,
    Grid,
}

pub struct Console {
    pub editor: TextArea<'static>,
    pub session: String,
    pub grid: Option<Grid>,
    pub result_title: String,
    pub output: Vec<(bool, String)>,
    pub show_output: bool,
    pub running: bool,
    pub tx_manual: bool,
    pub in_tx: bool,
}

pub struct TableTab {
    pub schema: String,
    pub table: String,
    pub where_: String,
    pub order: String,
    pub session: String,
    pub grid: Option<Grid>,
    pub running: bool,
    pub pk: Vec<String>,
    pub error: Option<String>,
}

pub struct TextTab {
    pub text: String,
    pub scroll: u16,
}

/// Medis 식 Redis 브라우저: 왼쪽 키 목록, 오른쪽 값
pub struct RedisTab {
    pub pattern: String,
    pub keys: Vec<KeyRow>,
    pub cursor: String,
    pub sel: usize,
    pub off: usize,
    pub detail: Option<(String, KeyDetail)>,
    pub value: Option<Grid>,
    pub focus_value: bool,
    pub scanning: bool,
    /// 열 때 지정한 키 (스캔이 끝나면 선택)
    pub want: Option<String>,
}

pub enum TabKind {
    Console(Console),
    Table(TableTab),
    Text(TextTab),
    Redis(RedisTab),
}

pub struct Tab {
    pub id: u64,
    pub title: String,
    pub conn: String,
    pub db: String,
    pub target: String,
    pub kind: TabKind,
}

// ───────── 팝업 ─────────

#[derive(Clone)]
pub enum PAction {
    Act(Action),
    Object { conn: String, db: String, e: IndexEntryLite },
    Tab(u64),
}
#[derive(Clone)]
pub struct IndexEntryLite {
    pub schema: String,
    pub name: String,
    pub kind: String,
}
#[derive(Clone)]
pub struct PItem {
    pub label: String,
    pub hint: String,
    pub action: PAction,
}
pub struct Palette {
    pub title: String,
    pub input: TextArea<'static>,
    pub items: Vec<PItem>,
    pub sel: usize,
    pub loading: usize,
}

#[derive(Clone, PartialEq)]
pub enum FieldKind {
    Text,
    Secret,
    Toggle,
    Choice(Vec<&'static str>),
}
pub struct Field {
    pub label: &'static str,
    pub value: String,
    pub kind: FieldKind,
}
pub struct ConnForm {
    pub edit: Option<ConnectionConfig>,
    pub fields: Vec<Field>,
    pub sel: usize,
    pub message: Option<(bool, String)>,
}

pub enum Popup {
    Help { scroll: u16 },
    Message { title: String, text: String },
    Input { title: String, input: TextArea<'static>, on_ok: Box<dyn FnOnce(&mut App, String)> },
    Confirm { text: String, on_yes: Box<dyn FnOnce(&mut App)> },
    Palette(Palette),
    ConnForm(ConnForm),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    NewConsole,
    NewDataSource,
    EditDataSource,
    DeleteDataSource,
    Connect,
    Disconnect,
    Refresh,
    Execute,
    Explain,
    Cancel,
    TxToggle,
    Commit,
    Rollback,
    CloseTab,
    NextTab,
    PrevTab,
    GoToObject,
    RecentTabs,
    Ddl,
    EditData,
    Submit,
    Revert,
    AddRow,
    DeleteRow,
    SetNull,
    ExportCsv,
    ToggleExplorer,
    ToggleOutput,
    Help,
    Quit,
}

pub const ACTIONS: &[(Action, &str, &str)] = &[
    (Action::NewConsole, "New Query Console", "Ctrl+L"),
    (Action::Execute, "Execute", "F5 / Ctrl+Enter / Alt+Enter"),
    (Action::Explain, "Explain Plan", "Alt+E"),
    (Action::Cancel, "Cancel Running Statement", "Ctrl+C"),
    (Action::TxToggle, "Toggle Transaction Mode (Auto/Manual)", "Alt+T"),
    (Action::Commit, "Commit", "Alt+C"),
    (Action::Rollback, "Roll Back", "Alt+R"),
    (Action::GoToObject, "Go to Database Object", "Ctrl+O"),
    (Action::RecentTabs, "Recent Tabs / Switcher", "Ctrl+E"),
    (Action::EditData, "Edit Data (open table)", "F4 / Enter"),
    (Action::Ddl, "Go to DDL", "Ctrl+B"),
    (Action::Submit, "Submit Changes", "Ctrl+S"),
    (Action::Revert, "Revert Changes", "Ctrl+Z (grid)"),
    (Action::AddRow, "Add Row", "Insert / a"),
    (Action::DeleteRow, "Delete Row", "Delete / d"),
    (Action::SetNull, "Set NULL", "n (grid)"),
    (Action::ExportCsv, "Export Data to CSV…", "x (grid)"),
    (Action::NewDataSource, "New Data Source…", "n (explorer)"),
    (Action::EditDataSource, "Data Source Properties…", "e (explorer)"),
    (Action::DeleteDataSource, "Remove Data Source", "D (explorer)"),
    (Action::Connect, "Connect", "c (explorer)"),
    (Action::Disconnect, "Disconnect", "Ctrl+F2 / X (explorer)"),
    (Action::Refresh, "Refresh", "r (explorer) / F5 (table)"),
    (Action::CloseTab, "Close Tab", "Ctrl+W"),
    (Action::NextTab, "Next Tab", "Alt+→ / Ctrl+PgDn"),
    (Action::PrevTab, "Previous Tab", "Alt+← / Ctrl+PgUp"),
    (Action::ToggleExplorer, "Database Explorer", "Alt+1"),
    (Action::ToggleOutput, "Toggle Output / Result", "Alt+O"),
    (Action::Help, "Keymap", "F1"),
    (Action::Quit, "Quit", "Ctrl+Q"),
];

/// 마지막으로 그린 화면 영역 (마우스 클릭 판정)
#[derive(Default)]
pub struct Hits {
    pub tree: ratatui::layout::Rect,
    /// (탭 id, x 시작, x 끝)
    pub tabs: Vec<(u64, u16, u16)>,
    pub tab_bar_y: u16,
    pub editor: ratatui::layout::Rect,
    pub results: ratatui::layout::Rect,
    pub redis_keys: ratatui::layout::Rect,
    pub redis_value: ratatui::layout::Rect,
    pub palette: ratatui::layout::Rect,
    pub palette_off: usize,
    /// 더블클릭 판정: (시각, x, y)
    pub last_click: Option<(std::time::Instant, u16, u16)>,
}

pub struct App {
    pub hits: Hits,
    pub state: Arc<AppState>,
    pub tx: UnboundedSender<Apply>,
    pub conns: Vec<ConnectionView>,
    pub tree: Tree,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub mru: Vec<u64>,
    pub focus: Focus,
    pub explorer: bool,
    pub popup: Option<Popup>,
    pub status: String,
    pub quit: bool,
    next_id: u64,
}

pub fn fuzzy(q: &str, text: &str) -> Option<i64> {
    if q.is_empty() {
        return Some(0);
    }
    let (q, t) = (q.to_lowercase(), text.to_lowercase());
    if t == q {
        return Some(1000);
    }
    if t.starts_with(&q) {
        return Some(800 - t.len() as i64);
    }
    if let Some(i) = t.find(&q) {
        return Some(500 - i as i64 - t.len() as i64 / 10);
    }
    let mut score = 200;
    let mut it = t.char_indices().peekable();
    let mut last = 0usize;
    for qc in q.chars().filter(|c| !c.is_whitespace()) {
        loop {
            match it.next() {
                Some((i, c)) if c == qc => {
                    score -= (i.saturating_sub(last)).min(10) as i64;
                    last = i + 1;
                    break;
                }
                Some(_) => continue,
                None => return None,
            }
        }
    }
    Some(score - t.len() as i64 / 10)
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn color_of(c: &ConnectionView) -> Option<String> {
    c.config.color.clone()
}

impl App {
    pub fn new(state: Arc<AppState>, tx: UnboundedSender<Apply>) -> Self {
        let mut a = App {
            hits: Hits::default(),
            state,
            tx,
            conns: vec![],
            tree: Tree::default(),
            tabs: vec![],
            active: 0,
            mru: vec![],
            focus: Focus::Tree,
            explorer: true,
            popup: None,
            status: "Ready — F1 keymap · Ctrl+K actions · Ctrl+O go to object".into(),
            quit: false,
            next_id: 1,
        };
        a.reload_conns();
        a
    }

    pub fn spawn<T: Send + 'static>(&self, fut: impl Future<Output = T> + Send + 'static, apply: impl FnOnce(&mut App, T) + Send + 'static) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let v = fut.await;
            let _ = tx.send(Box::new(move |a: &mut App| apply(a, v)));
        });
    }

    pub fn conn(&self, id: &str) -> Option<&ConnectionView> {
        self.conns.iter().find(|c| c.config.id == id)
    }
    pub fn kind_of(&self, conn: &str) -> DbKind {
        self.conn(conn).map(|c| c.config.kind).unwrap_or(DbKind::Postgres)
    }
    pub fn conn_color(&self, conn: &str) -> Option<String> {
        self.conn(conn).and_then(color_of)
    }

    // ───────── Data Sources ─────────

    pub fn reload_conns(&mut self) {
        let st = self.state.clone();
        self.spawn(async move { st.list_connections().await.map_err(err) }, |a, r| match r {
            Ok(cs) => {
                a.tree.roots = cs
                    .iter()
                    .map(|c| Node {
                        key: c.config.id.clone(),
                        label: c.config.name.clone(),
                        hint: format!("{:?}", c.config.kind).to_lowercase() + if c.config.active_ssh().is_some() { " · ssh" } else { "" },
                        kind: NodeKind::Conn,
                        conn: c.config.id.clone(),
                        db: None,
                        schema: None,
                        name: None,
                        expandable: true,
                    })
                    .collect();
                a.conns = cs;
                a.tree.clamp();
            }
            Err(e) => a.status = format!("Failed to load data sources: {e}"),
        });
    }

    fn default_db(&self, conn: &str) -> String {
        self.conn(conn).map(|c| sql::default_db(&c.config)).unwrap_or_default()
    }

    // ───────── 트리 ─────────

    pub fn toggle_node(&mut self, n: &Node) {
        if !n.expandable {
            return;
        }
        if self.tree.expanded.contains(&n.key) {
            self.tree.expanded.remove(&n.key);
        } else {
            self.tree.expanded.insert(n.key.clone());
            self.load_children(n.clone());
        }
        self.tree.clamp();
    }

    pub fn load_children(&mut self, n: Node) {
        let key = n.key.clone();
        if self.tree.children.contains_key(&key) || self.tree.loading.contains(&key) {
            return;
        }
        self.tree.loading.insert(key.clone());
        let st = self.state.clone();
        let conn = n.conn.clone();
        let kind = self.kind_of(&conn);
        let cfg = self.conn(&conn).map(|c| c.config.clone());
        let done = move |a: &mut App, key: String, r: Result<Vec<Node>, String>| {
            a.tree.loading.remove(&key);
            match r {
                Ok(nodes) => {
                    a.tree.children.insert(key, nodes);
                }
                Err(e) => {
                    a.tree.children.insert(key.clone(), vec![Node::info(&key, "", format!("⚠ {e}"))]);
                    a.status = e;
                }
            }
            a.tree.clamp();
        };
        match n.kind {
            NodeKind::Conn => {
                let cfg = cfg.unwrap();
                let def = sql::default_db(&cfg);
                self.spawn(
                    async move {
                        let dbs = st.list_databases(conn.clone()).await.map_err(err)?;
                        Ok::<_, String>((dbs, def))
                    },
                    move |a, r: Result<(Vec<String>, String), String>| {
                        let r = r.map(|(dbs, def)| {
                            let hidden: &[&str] = match cfg.kind {
                                DbKind::Postgres => &["template0", "template1", "rdsadmin"],
                                DbKind::Mysql => &dbtool_core::mysql::SYSTEM_DBS,
                                DbKind::Redis => &[],
                            };
                            // GUI 와 같은 표시 범위 (Data Source › Schemas 탭)
                            let scoped = cfg.scope.databases.as_ref().map(|d| !d.is_empty()).unwrap_or(false);
                            dbs.into_iter()
                                .filter(|d| scoped || d == &def || !hidden.contains(&d.as_str()))
                                .filter(|d| !cfg.options.single_db || d == &def)
                                .filter(|d| cfg.options.single_db || cfg.scope.db_visible(d, &def))
                                .map(|d| Node {
                                    key: format!("{}|{d}", cfg.id),
                                    label: d.clone(),
                                    hint: if d == def { "default".into() } else { String::new() },
                                    kind: NodeKind::Db,
                                    conn: cfg.id.clone(),
                                    db: Some(d),
                                    schema: None,
                                    name: None,
                                    expandable: true,
                                })
                                .collect::<Vec<_>>()
                        });
                        // 자동으로 펼칠 DB: 보이는 DB 가 하나면 그것, 아니면 기본 DB (보일 때만)
                        let auto = r.as_ref().ok().and_then(|ns| if ns.len() == 1 { ns.first().cloned() } else { ns.iter().find(|n| n.hint == "default").cloned() });
                        done(a, key, r);
                        a.reload_conns();
                        if let Some(n) = auto {
                            a.tree.expanded.insert(n.key.clone());
                            a.load_children(n);
                        }
                    },
                );
            }
            NodeKind::Db => {
                let db = n.db.clone().unwrap();
                let target = sql::target_id(&conn, Some(&db));
                let flat = kind != DbKind::Postgres;
                self.spawn(
                    async move {
                        let schemas = st.list_schemas(target.clone()).await.map_err(err)?;
                        if flat {
                            let s = schemas.first().map(|s| s.name.clone()).unwrap_or_else(|| db.clone());
                            let objs = st.list_objects(target, s.clone()).await.map_err(err)?;
                            Ok::<_, String>((schemas, Some((s, objs))))
                        } else {
                            Ok((schemas, None))
                        }
                    },
                    move |a, r| match r {
                        Err(e) => done(a, key, Err(e)),
                        Ok((_, Some((schema, objs)))) => {
                            let groups = a.groups(&key, &n.conn, n.db.as_deref().unwrap(), &schema, objs);
                            done(a, key, Ok(groups));
                        }
                        Ok((schemas, None)) => {
                            let scope = a.conn(&n.conn).map(|c| c.config.scope.clone()).unwrap_or_default();
                            let db = n.db.clone().unwrap_or_default();
                            let user: Vec<_> = schemas.iter().filter(|s| scope.schema_visible(&db, &s.name, s.is_system, false)).collect();
                            let nodes: Vec<Node> = user
                                .iter()
                                .map(|s| Node {
                                    key: format!("{key}|{}", s.name),
                                    label: s.name.clone(),
                                    hint: String::new(),
                                    kind: NodeKind::Schema,
                                    conn: n.conn.clone(),
                                    db: n.db.clone(),
                                    schema: Some(s.name.clone()),
                                    name: None,
                                    expandable: true,
                                })
                                .collect();
                            let auto = if nodes.len() == 1 { nodes.first().cloned() } else { nodes.iter().find(|x| x.label == "public").cloned() };
                            done(a, key, Ok(nodes));
                            if let Some(s) = auto {
                                a.tree.expanded.insert(s.key.clone());
                                a.load_children(s);
                            }
                        }
                    },
                );
            }
            NodeKind::Schema => {
                let (db, schema) = (n.db.clone().unwrap(), n.schema.clone().unwrap());
                let target = sql::target_id(&conn, Some(&db));
                let s2 = schema.clone();
                self.spawn(async move { st.list_objects(target, s2).await.map_err(err) }, move |a, r| {
                    let r = r.map(|objs| a.groups(&key, &n.conn, &db, &schema, objs));
                    done(a, key, r);
                });
            }
            NodeKind::Object(_) if n.is_relation() => {
                let target = sql::target_id(&conn, n.db.as_deref());
                let (schema, name) = (n.schema.clone().unwrap(), n.name.clone().unwrap());
                self.spawn(async move { st.table_detail(target, schema, name).await.map_err(err) }, move |a, r| {
                    let r = r.map(|d| {
                        d.columns
                            .into_iter()
                            .map(|c| Node {
                                key: format!("{key}|c:{}", c.name),
                                label: c.name.clone(),
                                hint: format!("{}{}", c.data_type, if c.nullable { "" } else { " not null" }),
                                kind: NodeKind::Column { ty: c.data_type, pk: c.is_pk },
                                conn: n.conn.clone(),
                                db: n.db.clone(),
                                schema: n.schema.clone(),
                                name: Some(c.name),
                                expandable: false,
                            })
                            .collect()
                    });
                    done(a, key, r);
                });
            }
            _ => {
                self.tree.loading.remove(&key);
            }
        }
    }

    /// 스키마 객체 → 폴더(tables/views/…/keys) + 그 아래 노드. 첫 폴더는 펼친다
    fn groups(&mut self, parent: &str, conn: &str, db: &str, schema: &str, objs: SchemaObjects) -> Vec<Node> {
        let defs: [(&str, &[&str]); 5] = [
            ("tables", &["table", "partitioned", "foreign"]),
            ("views", &["view"]),
            ("materialized views", &["matview"]),
            ("sequences", &["sequence"]),
            ("keys", &["key"]),
        ];
        let mut out = vec![];
        let mk_obj = |gkey: &str, name: &str, kind: &str, hint: String| Node {
            key: format!("{gkey}|o:{name}"),
            label: name.to_string(),
            hint,
            kind: NodeKind::Object(kind.to_string()),
            conn: conn.into(),
            db: Some(db.into()),
            schema: Some(schema.into()),
            name: Some(name.into()),
            expandable: ["table", "view", "matview", "foreign", "partitioned"].contains(&kind),
        };
        for (label, kinds) in defs {
            let items: Vec<_> = objs.relations.iter().filter(|r| kinds.contains(&r.kind.as_str())).collect();
            if items.is_empty() {
                continue;
            }
            let gkey = format!("{parent}|g:{label}");
            let kids = items
                .iter()
                .map(|r| mk_obj(&gkey, &r.name, if r.kind == "key" { "key" } else { &r.kind }, if r.kind == "key" { r.comment.clone().unwrap_or_default() } else { r.comment.clone().unwrap_or_default() }))
                .collect();
            self.tree.children.insert(gkey.clone(), kids);
            out.push(Node { key: gkey, label: label.into(), hint: items.len().to_string(), kind: NodeKind::Group, conn: conn.into(), db: Some(db.into()), schema: Some(schema.into()), name: None, expandable: true });
        }
        if !objs.routines.is_empty() {
            let gkey = format!("{parent}|g:routines");
            let kids = objs
                .routines
                .iter()
                .map(|r| {
                    let mut n = mk_obj(&gkey, &r.name, &r.kind, format!("({}){}", r.args, r.returns.as_ref().map(|x| format!(" → {x}")).unwrap_or_default()));
                    n.key = format!("{gkey}|o:{}({})", r.name, r.args);
                    n
                })
                .collect();
            self.tree.children.insert(gkey.clone(), kids);
            out.push(Node { key: gkey, label: "routines".into(), hint: objs.routines.len().to_string(), kind: NodeKind::Group, conn: conn.into(), db: Some(db.into()), schema: Some(schema.into()), name: None, expandable: true });
        }
        if let Some(first) = out.first() {
            self.tree.expanded.insert(first.key.clone());
        }
        out
    }

    /// Enter / F4 / 더블클릭
    pub fn activate_node(&mut self, n: &Node, f4: bool) {
        let kind = self.kind_of(&n.conn);
        match &n.kind {
            NodeKind::Conn | NodeKind::Db if f4 && kind != DbKind::Redis => self.open_console(&n.conn, n.db.clone()),
            NodeKind::Db if kind == DbKind::Redis => self.open_redis(&n.conn, n.db.clone().unwrap(), None),
            NodeKind::Object(k) if k == "key" => self.open_redis(&n.conn, n.db.clone().unwrap(), n.name.clone()),
            NodeKind::Object(_) if n.is_relation() => self.open_table(&n.conn, n.db.clone().unwrap(), n.schema.clone().unwrap(), n.name.clone().unwrap()),
            NodeKind::Object(k) => {
                let k = if k == "function" || k == "procedure" { "routine".to_string() } else { k.clone() };
                self.open_ddl(&n.conn, n.db.clone().unwrap(), n.schema.clone().unwrap(), n.name.clone().unwrap(), k)
            }
            NodeKind::Column { .. } => {}
            _ => self.toggle_node(n),
        }
    }

    // ───────── 탭 ─────────

    fn push_tab(&mut self, conn: &str, db: String, title: String, kind: TabKind) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let target = sql::target_id(conn, Some(&db));
        self.tabs.push(Tab { id, title, conn: conn.into(), db, target, kind });
        self.activate(self.tabs.len() - 1);
        id
    }

    pub fn activate(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        self.active = i;
        let id = self.tabs[i].id;
        self.mru.retain(|x| *x != id);
        self.mru.insert(0, id);
        self.focus = match &self.tabs[i].kind {
            TabKind::Console(_) => Focus::Editor,
            _ => Focus::Grid,
        };
    }

    pub fn tab_index(&self, id: u64) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }
    pub fn tab_mut(&mut self, id: u64) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        let t = self.tabs.remove(i);
        let session = match &t.kind {
            TabKind::Console(c) => Some(c.session.clone()),
            TabKind::Table(tt) => Some(tt.session.clone()),
            _ => None,
        };
        if let Some(s) = session {
            let st = self.state.clone();
            tokio::spawn(async move {
                let _ = st.close_session(s).await;
            });
        }
        self.mru.retain(|x| *x != t.id);
        if self.tabs.is_empty() {
            self.active = 0;
            self.focus = Focus::Tree;
        } else {
            let next = self.mru.first().and_then(|id| self.tab_index(*id)).unwrap_or(i.min(self.tabs.len() - 1));
            self.activate(next);
        }
    }

    pub fn open_console(&mut self, conn: &str, db: Option<String>) {
        let db = db.unwrap_or_else(|| self.default_db(conn));
        let name = self.conn(conn).map(|c| c.config.name.clone()).unwrap_or_default();
        let n = self.tabs.iter().filter(|t| t.conn == conn && matches!(t.kind, TabKind::Console(_))).count();
        let title = if n == 0 { format!("console [{name}]") } else { format!("console_{} [{name}]", n + 1) };
        let mut editor = TextArea::default();
        editor.set_cursor_line_style(ratatui::style::Style::default());
        let manual = self.conn(conn).map(|c| c.config.options.tx_manual).unwrap_or(false);
        let session = uuid::Uuid::new_v4().to_string();
        let id = self.push_tab(conn, db, title, TabKind::Console(Console { editor, session: session.clone(), grid: None, result_title: String::new(), output: vec![], show_output: true, running: false, tx_manual: manual, in_tx: false }));
        if manual {
            let st = self.state.clone();
            let target = self.tabs[self.tab_index(id).unwrap()].target.clone();
            self.spawn(async move { st.set_tx_mode(target, session, true).await.map_err(err) }, |a, r| {
                if let Err(e) = r {
                    a.status = e;
                }
            });
        }
    }

    pub fn open_table(&mut self, conn: &str, db: String, schema: String, table: String) {
        if let Some(i) = self.tabs.iter().position(|t| t.conn == conn && t.db == db && matches!(&t.kind, TabKind::Table(tt) if tt.schema == schema && tt.table == table)) {
            return self.activate(i);
        }
        let id = self.push_tab(conn, db, table.clone(), TabKind::Table(TableTab { schema, table, where_: String::new(), order: String::new(), session: uuid::Uuid::new_v4().to_string(), grid: None, running: false, pk: vec![], error: None }));
        self.reload_table(id);
    }

    pub fn reload_table(&mut self, id: u64) {
        let kind = match self.tab_mut(id) {
            Some(t) => t.conn.clone(),
            None => return,
        };
        let kind = self.kind_of(&kind);
        let Some(t) = self.tab_mut(id) else { return };
        let target = t.target.clone();
        let TabKind::Table(tt) = &mut t.kind else { return };
        let q = |s: &str| sql::quote_ident(kind, s);
        let mut sqls = format!("SELECT * FROM {}.{}", q(&tt.schema), q(&tt.table));
        if !tt.where_.trim().is_empty() {
            sqls += &format!(" WHERE {}", tt.where_.trim());
        }
        if !tt.order.trim().is_empty() {
            sqls += &format!(" ORDER BY {}", tt.order.trim());
        }
        tt.running = true;
        tt.error = None;
        let (session, schema, table) = (tt.session.clone(), tt.schema.clone(), tt.table.clone());
        let st = self.state.clone();
        self.spawn(
            async move {
                let pk = st.table_detail(target.clone(), schema, table).await.map(|d| d.columns.into_iter().filter(|c| c.is_pk).map(|c| c.name).collect::<Vec<_>>()).unwrap_or_default();
                let r = st.run_query(target, sqls, Some(500), Some(session)).await.map_err(err);
                (pk, r)
            },
            move |a, (pk, r)| {
                let Some(t) = a.tab_mut(id) else { return };
                let TabKind::Table(tt) = &mut t.kind else { return };
                tt.running = false;
                tt.pk = pk;
                match r {
                    Ok(res) => {
                        let n = res.rows.len();
                        let more = res.truncated;
                        let (row, col) = tt.grid.as_ref().map(|g| (g.row, g.col)).unwrap_or((0, 0));
                        let mut g = Grid::new(res);
                        g.more_session = Some(tt.session.clone());
                        g.row = row.min(n.saturating_sub(1));
                        g.col = col.min(g.cols().saturating_sub(1));
                        tt.grid = Some(g);
                        a.status = format!("{n}{} rows", if more { "+" } else { "" });
                    }
                    Err(e) => {
                        tt.error = Some(e.clone());
                        a.status = e;
                    }
                }
            },
        );
    }

    pub fn open_ddl(&mut self, conn: &str, db: String, schema: String, name: String, kind: String) {
        let target = sql::target_id(conn, Some(&db));
        let st = self.state.clone();
        let (conn, db2) = (conn.to_string(), db.clone());
        let title = format!("{name} [DDL]");
        self.spawn(async move { st.object_ddl(target, schema, name, kind).await.map_err(err) }, move |a, r| match r {
            Ok(text) => {
                if let Some(i) = a.tabs.iter().position(|t| t.title == title && t.conn == conn) {
                    a.activate(i);
                } else {
                    a.push_tab(&conn, db2, title, TabKind::Text(TextTab { text, scroll: 0 }));
                }
            }
            Err(e) => a.status = format!("DDL failed: {e}"),
        });
    }

    pub fn open_redis(&mut self, conn: &str, db: String, key: Option<String>) {
        let id = match self.tabs.iter().position(|t| t.conn == conn && t.db == db && matches!(t.kind, TabKind::Redis(_))) {
            Some(i) => {
                self.activate(i);
                self.tabs[i].id
            }
            None => {
                let name = self.conn(conn).map(|c| c.config.name.clone()).unwrap_or_default();
                let id = self.push_tab(conn, db.clone(), format!("{name} [{db}]"), TabKind::Redis(RedisTab { pattern: "*".into(), keys: vec![], cursor: "0".into(), sel: 0, off: 0, detail: None, value: None, focus_value: false, scanning: false, want: key.clone() }));
                self.redis_scan(id, true);
                id
            }
        };
        if let Some(k) = key {
            if let Some(t) = self.tab_mut(id) {
                if let TabKind::Redis(r) = &mut t.kind {
                    match r.keys.iter().position(|x| x.key == k) {
                        Some(i) => r.sel = i,
                        None => r.want = Some(k.clone()),
                    }
                }
            }
            self.redis_load_key(id, k);
        }
    }

    pub fn redis_scan(&mut self, id: u64, reset: bool) {
        let Some(t) = self.tab_mut(id) else { return };
        let target = t.target.clone();
        let TabKind::Redis(r) = &mut t.kind else { return };
        r.scanning = true;
        let (cursor, pattern) = (if reset { "0".to_string() } else { r.cursor.clone() }, r.pattern.clone());
        let st = self.state.clone();
        self.spawn(async move { st.redis_scan(target, cursor, pattern, 500, None).await.map_err(err) }, move |a, res| {
            let Some(t) = a.tab_mut(id) else { return };
            let TabKind::Redis(r) = &mut t.kind else { return };
            r.scanning = false;
            match res {
                Ok(page) => {
                    if reset {
                        r.keys.clear();
                        r.sel = 0;
                        r.off = 0;
                    }
                    for k in page.keys {
                        if !r.keys.iter().any(|x| x.key == k.key) {
                            r.keys.push(k);
                        }
                    }
                    r.keys.sort_by(|a, b| a.key.cmp(&b.key));
                    r.cursor = page.cursor;
                    let n = r.keys.len();
                    let more = r.cursor != "0";
                    let first = match r.want.take() {
                        Some(w) => {
                            if let Some(i) = r.keys.iter().position(|k| k.key == w) {
                                r.sel = i;
                            }
                            None
                        }
                        None if reset && r.detail.is_none() => r.keys.first().map(|k| k.key.clone()),
                        None => None,
                    };
                    a.status = format!("{n}{} keys", if more { "+ (m: scan more)" } else { "" });
                    if let Some(k) = first {
                        a.redis_load_key(id, k);
                    }
                }
                Err(e) => a.status = e,
            }
        });
    }

    pub fn redis_load_key(&mut self, id: u64, key: String) {
        let Some(t) = self.tab_mut(id) else { return };
        let target = t.target.clone();
        let st = self.state.clone();
        let k2 = key.clone();
        self.spawn(async move { st.redis_key(target, k2, Some(1000)).await.map_err(err) }, move |a, res| {
            let Some(t) = a.tab_mut(id) else { return };
            let TabKind::Redis(r) = &mut t.kind else { return };
            match res {
                Ok(d) => {
                    r.value = redis_grid(&d);
                    r.detail = Some((key, d));
                }
                Err(e) => {
                    r.detail = None;
                    r.value = None;
                    a.status = e;
                }
            }
        });
    }

    /// Redis 명령 실행 후 키 다시 읽기
    pub fn redis_cmd(&mut self, id: u64, args: Vec<String>, rescan: bool) {
        if self.tab_mut(id).is_none() {
            return;
        }
        if self.conn(&self.tabs[self.tab_index(id).unwrap()].conn).map(|c| c.config.read_only).unwrap_or(false) {
            self.status = "Data source is read-only".into();
            return;
        }
        let t = self.tab_mut(id).unwrap();
        let target = t.target.clone();
        let key = match &t.kind {
            TabKind::Redis(r) => r.detail.as_ref().map(|d| d.0.clone()),
            _ => None,
        };
        let st = self.state.clone();
        let shown = args.join(" ");
        self.spawn(async move { st.redis_exec(target, args).await.map_err(err) }, move |a, res| match res {
            Ok(v) => {
                a.status = format!("{shown} → {}", crate::ui::format_reply(&v).lines().next().unwrap_or(""));
                if rescan {
                    a.redis_scan(id, true);
                }
                if let Some(k) = key {
                    a.redis_load_key(id, k);
                }
            }
            Err(e) => a.status = e,
        });
    }

    // ───────── 실행 ─────────

    pub fn execute(&mut self, explain: bool) {
        let Some(t) = self.tabs.get(self.active) else { return };
        let kind = self.kind_of(&t.conn);
        let id = t.id;
        let target = t.target.clone();
        let conn_name = self.conn(&t.conn).map(|c| c.config.name.clone()).unwrap_or_default();
        let t = &mut self.tabs[self.active];
        let TabKind::Console(c) = &mut t.kind else {
            if let TabKind::Table(_) = t.kind {
                self.reload_table(id);
            }
            return;
        };
        if c.running {
            return;
        }
        let text = c.editor.lines().join("\n");
        let stmts: Vec<String> = if let Some(((r0, c0), (r1, c1))) = c.editor.selection_range() {
            let sel = selected_text(c.editor.lines(), r0, c0, r1, c1);
            sql::split_statements(&sel, kind).into_iter().map(|(s, e)| sel[s..e].trim().to_string()).collect()
        } else {
            let (row, col) = c.editor.cursor();
            let off = byte_offset(c.editor.lines(), row, col);
            sql::statement_at(&text, off, kind).into_iter().collect()
        };
        if stmts.is_empty() {
            return;
        }
        c.running = true;
        let session = c.session.clone();
        let st = self.state.clone();
        let stmts2 = stmts.clone();
        self.status = "Executing…".into();
        self.spawn(
            async move {
                let mut out = vec![];
                for s in stmts2 {
                    let q = if explain {
                        let bare = s.trim_end_matches(';');
                        match kind {
                            DbKind::Postgres => format!("EXPLAIN {bare}"),
                            DbKind::Mysql => format!("EXPLAIN {bare}"),
                            DbKind::Redis => s.clone(),
                        }
                    } else {
                        s.clone()
                    };
                    let r = st.run_query(target.clone(), q, Some(500), Some(session.clone())).await.map_err(err);
                    let stop = r.is_err();
                    out.push((s, r));
                    if stop {
                        break;
                    }
                }
                out
            },
            move |a, out| {
                let Some(t) = a.tab_mut(id) else { return };
                let TabKind::Console(c) = &mut t.kind else { return };
                c.running = false;
                let mut status = String::new();
                let n = out.len();
                for (i, (s, r)) in out.into_iter().enumerate() {
                    c.output.push((false, format!("{conn_name}> {}", s.replace('\n', " "))));
                    match r {
                        Ok(res) => {
                            c.in_tx = res.in_transaction;
                            if res.has_result_set {
                                let msg = format!("{}{} row(s) retrieved in {} ms", res.rows.len(), if res.truncated { "+" } else { "" }, res.elapsed_ms);
                                c.output.push((false, msg.clone()));
                                status = msg;
                                c.result_title = format!("Result {}: {}", i + 1, s.split_whitespace().take(6).collect::<Vec<_>>().join(" "));
                                let mut g = Grid::new(res);
                                g.more_session = Some(c.session.clone());
                                c.grid = Some(g);
                                c.show_output = false;
                            } else {
                                let msg = format!("{} row(s) affected in {} ms", res.rows_affected, res.elapsed_ms);
                                c.output.push((false, msg.clone()));
                                status = msg;
                                if i + 1 == n && c.grid.is_none() {
                                    c.show_output = true;
                                }
                            }
                        }
                        Err(e) => {
                            c.output.push((true, e.clone()));
                            c.show_output = true;
                            status = format!("Error: {}", e.lines().next().unwrap_or(""));
                        }
                    }
                }
                a.status = status;
            },
        );
    }

    pub fn fetch_more(&mut self) {
        let Some(t) = self.tabs.get_mut(self.active) else { return };
        let id = t.id;
        let g = match &mut t.kind {
            TabKind::Console(c) => c.grid.as_mut(),
            TabKind::Table(tt) => tt.grid.as_mut(),
            _ => None,
        };
        let Some(g) = g else { return };
        if !g.wants_more() {
            return;
        }
        g.loading_more = true;
        let session = g.more_session.clone().unwrap();
        let st = self.state.clone();
        self.spawn(async move { st.fetch_more(session, Some(500)).await.map_err(err) }, move |a, r| {
            let Some(t) = a.tab_mut(id) else { return };
            let g = match &mut t.kind {
                TabKind::Console(c) => c.grid.as_mut(),
                TabKind::Table(tt) => tt.grid.as_mut(),
                _ => None,
            };
            let Some(g) = g else { return };
            g.loading_more = false;
            match r {
                Ok(more) => {
                    g.append(more);
                    a.status = format!("{}{} rows", g.rows(), if g.result.truncated { "+" } else { "" });
                }
                Err(e) => {
                    g.result.truncated = false;
                    a.status = e;
                }
            }
        });
    }

    pub fn cancel(&mut self) {
        let Some(t) = self.tabs.get(self.active) else { return };
        let session = match &t.kind {
            TabKind::Console(c) if c.running => c.session.clone(),
            TabKind::Table(tt) if tt.running => tt.session.clone(),
            _ => return,
        };
        let st = self.state.clone();
        self.spawn(async move { st.cancel_query(session).await.map_err(err) }, |a, r| {
            a.status = match r {
                Ok(true) => "Cancel requested".into(),
                Ok(false) => "Nothing to cancel".into(),
                Err(e) => e,
            }
        });
    }

    pub fn tx(&mut self, what: Action) {
        let Some(t) = self.tabs.get_mut(self.active) else { return };
        let id = t.id;
        let target = t.target.clone();
        let TabKind::Console(c) = &mut t.kind else { return };
        let session = c.session.clone();
        let st = self.state.clone();
        match what {
            Action::TxToggle => {
                let manual = !c.tx_manual;
                self.spawn(async move { st.set_tx_mode(target, session, manual).await.map_err(err) }, move |a, r| match r {
                    Ok(()) => {
                        if let Some(TabKind::Console(c)) = a.tab_mut(id).map(|t| &mut t.kind) {
                            c.tx_manual = manual;
                            if !manual {
                                c.in_tx = false;
                            }
                        }
                        a.status = format!("Transaction mode: {}", if manual { "Manual" } else { "Auto" });
                    }
                    Err(e) => a.status = e,
                });
            }
            Action::Commit | Action::Rollback => {
                let commit = what == Action::Commit;
                self.spawn(
                    async move { if commit { st.commit_tx(session).await } else { st.rollback_tx(session).await }.map_err(err) },
                    move |a, r| match r {
                        Ok(()) => {
                            if let Some(TabKind::Console(c)) = a.tab_mut(id).map(|t| &mut t.kind) {
                                c.in_tx = false;
                                c.output.push((false, if commit { "Committed".into() } else { "Rolled back".into() }));
                            }
                            a.status = if commit { "Committed".into() } else { "Rolled back".into() };
                        }
                        Err(e) => a.status = e,
                    },
                );
            }
            _ => {}
        }
    }

    // ───────── 테이블 편집 ─────────

    pub fn submit(&mut self) {
        let Some(t) = self.tabs.get(self.active) else { return };
        let kind = self.kind_of(&t.conn);
        let ro = self.conn(&t.conn).map(|c| c.config.read_only).unwrap_or(false);
        let (id, target) = (t.id, t.target.clone());
        let TabKind::Table(tt) = &t.kind else { return };
        let Some(g) = &tt.grid else { return };
        if !g.dirty() {
            self.status = "No pending changes".into();
            return;
        }
        if ro {
            self.status = "Data source is read-only".into();
            return;
        }
        let q = |s: &str| sql::quote_ident(kind, s);
        let table = format!("{}.{}", q(&tt.schema), q(&tt.table));
        let cols = &g.result.columns;
        let pk_idx: Vec<usize> = tt.pk.iter().filter_map(|p| cols.iter().position(|c| &c.name == p)).collect();
        let where_of = |r: usize| -> Option<String> {
            if pk_idx.is_empty() || pk_idx.len() != tt.pk.len() {
                return None;
            }
            Some(pk_idx.iter().map(|&i| format!("{} = {}", q(&cols[i].name), sql::literal(kind, raw_text(&g.result.rows[r][i]).as_deref()))).collect::<Vec<_>>().join(" AND "))
        };
        let mut stmts = vec![];
        let base = g.rows() - g.added;
        let mut rows_with_edits: Vec<usize> = g.edits.keys().map(|(r, _)| *r).collect();
        rows_with_edits.dedup();
        for r in rows_with_edits.iter().copied().filter(|r| *r < base && !g.deleted.contains(r)) {
            let sets: Vec<String> = g.edits.iter().filter(|((rr, _), _)| *rr == r).map(|((_, c), v)| format!("{} = {}", q(&cols[*c].name), sql::literal(kind, v.as_deref()))).collect();
            match where_of(r) {
                Some(w) => stmts.push(format!("UPDATE {table} SET {} WHERE {w}", sets.join(", "))),
                None => {
                    self.status = "Table has no primary key — cannot update rows".into();
                    return;
                }
            }
        }
        for &r in &g.deleted {
            if r >= base {
                continue;
            }
            match where_of(r) {
                Some(w) => stmts.push(format!("DELETE FROM {table} WHERE {w}")),
                None => {
                    self.status = "Table has no primary key — cannot delete rows".into();
                    return;
                }
            }
        }
        for r in base..g.rows() {
            if g.deleted.contains(&r) {
                continue;
            }
            let set: Vec<(usize, &Option<String>)> = g.edits.iter().filter(|((rr, _), _)| *rr == r).map(|((_, c), v)| (*c, v)).collect();
            if set.is_empty() {
                stmts.push(match kind {
                    DbKind::Mysql => format!("INSERT INTO {table} () VALUES ()"),
                    _ => format!("INSERT INTO {table} DEFAULT VALUES"),
                });
            } else {
                stmts.push(format!(
                    "INSERT INTO {table} ({}) VALUES ({})",
                    set.iter().map(|(c, _)| q(&cols[*c].name)).collect::<Vec<_>>().join(", "),
                    set.iter().map(|(_, v)| sql::literal(kind, v.as_deref())).collect::<Vec<_>>().join(", ")
                ));
            }
        }
        let n = stmts.len();
        let st = self.state.clone();
        self.status = format!("Submitting {n} statement(s)…");
        self.spawn(async move { st.execute_batch(target, stmts).await.map_err(err) }, move |a, r| match r {
            Ok(rows) => {
                a.status = format!("{n} statement(s) submitted, {rows} row(s) affected");
                a.reload_table(id);
            }
            Err(e) => a.popup = Some(Popup::Message { title: "Submit failed".into(), text: e }),
        });
    }

    pub fn export_csv(&mut self, path: String) {
        let Some(t) = self.tabs.get(self.active) else { return };
        let kind = self.kind_of(&t.conn);
        let target = t.target.clone();
        let sqlq = match &t.kind {
            TabKind::Table(tt) => format!("SELECT * FROM {}.{}{}", sql::quote_ident(kind, &tt.schema), sql::quote_ident(kind, &tt.table), if tt.where_.trim().is_empty() { String::new() } else { format!(" WHERE {}", tt.where_) }),
            TabKind::Console(c) => {
                let text = c.editor.lines().join("\n");
                let (row, col) = c.editor.cursor();
                match sql::statement_at(&text, byte_offset(c.editor.lines(), row, col), kind) {
                    Some(s) => s,
                    None => return,
                }
            }
            _ => return,
        };
        let st = self.state.clone();
        let p2 = path.clone();
        self.spawn(async move { st.export_csv(target, sqlq, p2).await.map_err(err) }, move |a, r| {
            a.status = match r {
                Ok(n) => format!("Exported {n} rows to {path}"),
                Err(e) => format!("Export failed: {e}"),
            }
        });
    }

    // ───────── 연결 ─────────

    pub fn connect(&mut self, conn: &str) {
        if let Some(n) = self.tree.roots.iter().find(|n| n.conn == conn).cloned() {
            if !self.tree.expanded.contains(&n.key) {
                self.toggle_node(&n);
            }
        }
    }

    pub fn disconnect(&mut self, conn: &str) {
        let st = self.state.clone();
        let c = conn.to_string();
        self.spawn(async move { st.disconnect(c).await.map_err(err) }, {
            let conn = conn.to_string();
            move |a, r| {
                a.tree.forget(&conn);
                a.tree.expanded.retain(|k| !(k == &conn || k.starts_with(&format!("{conn}|"))));
                a.tree.clamp();
                a.status = match r {
                    Ok(()) => "Disconnected".into(),
                    Err(e) => e,
                };
                a.reload_conns();
            }
        });
    }

    pub fn refresh_node(&mut self, n: &Node) {
        let key = n.key.clone();
        self.tree.forget(&key);
        if self.tree.expanded.contains(&key) {
            self.load_children(n.clone());
        }
        self.reload_conns();
    }

    pub fn conn_form(&mut self, edit: Option<ConnectionConfig>) {
        let c = edit.clone().unwrap_or(ConnectionConfig {
            id: String::new(),
            name: String::new(),
            kind: DbKind::Postgres,
            host: "localhost".into(),
            port: 5432,
            user: String::new(),
            database: String::new(),
            scope: Default::default(),
            password_saved: false,
            ssh_secret_saved: false,
            ssh: None,
            color: None,
            read_only: false,
            options: Default::default(), ssl: None, redis: None,
        });
        let ssh = c.ssh.clone().unwrap_or(SshConfig { enabled: false, host: String::new(), port: 22, user: String::new(), auth: SshAuth::Key, key_path: "~/.ssh/id_ed25519".into() });
        let kind = match c.kind {
            DbKind::Postgres => "postgres",
            DbKind::Mysql => "mysql",
            DbKind::Redis => "redis",
        };
        let f = |label, value: String, kind| Field { label, value, kind };
        let saved = |b: bool| if b { "<saved>".to_string() } else { String::new() };
        self.popup = Some(Popup::ConnForm(ConnForm {
            fields: vec![
                f("Name", c.name.clone(), FieldKind::Text),
                f("Type", kind.into(), FieldKind::Choice(vec!["postgres", "mysql", "redis"])),
                f("Host", c.host.clone(), FieldKind::Text),
                f("Port", c.port.to_string(), FieldKind::Text),
                f("User", c.user.clone(), FieldKind::Text),
                f("Password", saved(c.password_saved), FieldKind::Secret),
                f("Database", c.database.clone(), FieldKind::Text),
                f("Read-only", if c.read_only { "on" } else { "off" }.into(), FieldKind::Toggle),
                f("SSH tunnel", if ssh.enabled { "on" } else { "off" }.into(), FieldKind::Toggle),
                f("SSH host", ssh.host.clone(), FieldKind::Text),
                f("SSH port", ssh.port.to_string(), FieldKind::Text),
                f("SSH user", ssh.user.clone(), FieldKind::Text),
                f("SSH auth", if ssh.auth == SshAuth::Key { "key" } else { "password" }.into(), FieldKind::Choice(vec!["key", "password"])),
                f("SSH key file", ssh.key_path.clone(), FieldKind::Text),
                f("SSH secret", saved(c.ssh_secret_saved), FieldKind::Secret),
                f("SSL", if c.active_ssl().is_some() { "on" } else { "off" }.into(), FieldKind::Toggle),
                f("SSL mode", c.ssl.as_ref().map(|x| x.mode().to_string()).unwrap_or_else(|| "require".into()), FieldKind::Choice(vec!["require", "verify-ca", "verify-full"])),
                f("CA file", c.ssl.as_ref().map(|x| x.ca_file.clone()).unwrap_or_default(), FieldKind::Text),
                f("Client cert", c.ssl.as_ref().map(|x| x.cert_file.clone()).unwrap_or_default(), FieldKind::Text),
                f("Client key", c.ssl.as_ref().map(|x| x.key_file.clone()).unwrap_or_default(), FieldKind::Text),
                f("Redis mode", c.redis_mode().into(), FieldKind::Choice(vec!["standalone", "sentinel", "cluster"])),
                f("Sentinel master", c.redis.as_ref().map(|x| x.sentinel_master.clone()).unwrap_or_default(), FieldKind::Text),
                f("Redis nodes", c.redis.as_ref().map(|x| x.nodes.clone()).unwrap_or_default(), FieldKind::Text),
            ],
            edit,
            sel: 0,
            message: None,
        }));
    }

    /// 폼 → (설정, 비밀번호, SSH 비밀)
    pub fn form_config(form: &ConnForm) -> Result<(ConnectionConfig, Option<String>, Option<String>), String> {
        let v = |l: &str| form.fields.iter().find(|f| f.label == l).map(|f| f.value.clone()).unwrap_or_default();
        let mut c = form.edit.clone().unwrap_or(ConnectionConfig {
            id: String::new(),
            name: String::new(),
            kind: DbKind::Postgres,
            host: String::new(),
            port: 0,
            user: String::new(),
            database: String::new(),
            scope: Default::default(),
            password_saved: false,
            ssh_secret_saved: false,
            ssh: None,
            color: None,
            read_only: false,
            options: Default::default(), ssl: None, redis: None,
        });
        c.kind = match v("Type").as_str() {
            "mysql" => DbKind::Mysql,
            "redis" => DbKind::Redis,
            _ => DbKind::Postgres,
        };
        c.host = v("Host").trim().into();
        c.port = v("Port").trim().parse().map_err(|_| "Port must be a number".to_string())?;
        c.user = v("User").trim().into();
        c.database = v("Database").trim().into();
        c.name = if v("Name").trim().is_empty() { format!("{}@{}", c.user, c.host) } else { v("Name").trim().into() };
        c.read_only = v("Read-only") == "on";
        c.ssh = Some(SshConfig {
            enabled: v("SSH tunnel") == "on",
            host: v("SSH host").trim().into(),
            port: v("SSH port").trim().parse().unwrap_or(22),
            user: v("SSH user").trim().into(),
            auth: if v("SSH auth") == "password" { SshAuth::Password } else { SshAuth::Key },
            key_path: v("SSH key file").trim().into(),
        });
        c.ssl = Some(SslConfig { enabled: v("SSL") == "on", mode: v("SSL mode"), ca_file: v("CA file").trim().into(), cert_file: v("Client cert").trim().into(), key_file: v("Client key").trim().into() });
        c.redis = if c.kind == DbKind::Redis { Some(RedisMode { mode: v("Redis mode"), sentinel_master: v("Sentinel master").trim().into(), nodes: v("Redis nodes").trim().into() }) } else { None };
        let secret = |s: String| if s == "<saved>" { None } else { Some(s) };
        Ok((c, secret(v("Password")), secret(v("SSH secret"))))
    }

    // ───────── 팝업: 팔레트 ─────────

    pub fn palette_actions(&mut self) {
        let items = ACTIONS.iter().map(|(a, n, k)| PItem { label: n.to_string(), hint: k.to_string(), action: PAction::Act(*a) }).collect();
        self.popup = Some(Popup::Palette(Palette { title: "Actions".into(), input: TextArea::default(), items, sel: 0, loading: 0 }));
    }

    pub fn palette_tabs(&mut self) {
        let items = self.mru.iter().filter_map(|id| self.tabs.iter().find(|t| t.id == *id)).map(|t| PItem { label: t.title.clone(), hint: t.db.clone(), action: PAction::Tab(t.id) }).collect();
        self.popup = Some(Popup::Palette(Palette { title: "Recent Tabs".into(), input: TextArea::default(), items, sel: 1, loading: 0 }));
    }

    /// Go to Database Object: 연결된 Data Source 의 기본 DB + 펼친 DB
    pub fn palette_objects(&mut self) {
        let mut targets: Vec<(String, String)> = vec![];
        for c in self.conns.iter().filter(|c| c.connected) {
            targets.push((c.config.id.clone(), sql::default_db(&c.config)));
        }
        for k in &self.tree.expanded {
            let parts: Vec<&str> = k.split('|').collect();
            if parts.len() == 2 && !targets.iter().any(|(c, d)| c == parts[0] && d == parts[1]) {
                targets.push((parts[0].into(), parts[1].into()));
            }
        }
        self.popup = Some(Popup::Palette(Palette { title: "Go to Database Object".into(), input: TextArea::default(), items: vec![], sel: 0, loading: targets.len() }));
        if targets.is_empty() {
            self.status = "Connect a data source first (Enter on it in the explorer)".into();
        }
        for (conn, db) in targets {
            let st = self.state.clone();
            let target = sql::target_id(&conn, Some(&db));
            let name = self.conn(&conn).map(|c| c.config.name.clone()).unwrap_or_default();
            self.spawn(async move { st.object_index(target).await.map_err(err) }, move |a, r: Result<Vec<IndexEntry>, String>| {
                if let Err(e) = &r {
                    a.status = format!("{name}/{db}: {e}");
                }
                if let Some(Popup::Palette(p)) = &mut a.popup {
                    p.loading = p.loading.saturating_sub(1);
                    if let Ok(es) = r {
                        p.items.extend(es.into_iter().map(|e| PItem {
                            label: e.name.clone(),
                            hint: format!("{} {} · {name}/{db}", e.kind, if e.kind == "schema" { String::new() } else { e.schema.clone() }),
                            action: PAction::Object { conn: conn.clone(), db: db.clone(), e: IndexEntryLite { schema: e.schema, name: e.name, kind: e.kind } },
                        }));
                    }
                }
            });
        }
    }

    pub fn palette_matches(p: &Palette) -> Vec<&PItem> {
        let q = p.input.lines().join("");
        let mut v: Vec<(i64, &PItem)> = p.items.iter().filter_map(|i| fuzzy(&q, &i.label).map(|s| (s, i))).collect();
        if !q.is_empty() {
            v.sort_by(|a, b| b.0.cmp(&a.0));
        }
        v.into_iter().take(300).map(|x| x.1).collect()
    }

    pub fn run_palette(&mut self, item: PItem, ddl: bool) {
        self.popup = None;
        match item.action {
            PAction::Act(a) => self.run_action(a),
            PAction::Tab(id) => {
                if let Some(i) = self.tab_index(id) {
                    self.activate(i)
                }
            }
            PAction::Object { conn, db, e } => {
                let kind = match e.kind.as_str() {
                    "function" | "procedure" => "routine".to_string(),
                    k => k.to_string(),
                };
                if ddl && kind != "schema" {
                    return self.open_ddl(&conn, db, e.schema, e.name, kind);
                }
                match kind.as_str() {
                    "table" | "view" | "matview" | "foreign" => self.open_table(&conn, db, e.schema, e.name),
                    "key" => self.open_redis(&conn, db, Some(e.name)),
                    "schema" => {
                        self.explorer = true;
                        self.focus = Focus::Tree;
                        let key = format!("{conn}|{db}|{}", e.schema);
                        self.tree.select_key(&key);
                    }
                    _ => self.open_ddl(&conn, db, e.schema, e.name, kind),
                }
            }
        }
    }

    pub fn input(&mut self, title: impl Into<String>, init: &str, on_ok: impl FnOnce(&mut App, String) + 'static) {
        let mut t = TextArea::new(vec![init.to_string()]);
        t.move_cursor(tui_textarea::CursorMove::End);
        t.set_cursor_line_style(ratatui::style::Style::default());
        self.popup = Some(Popup::Input { title: title.into(), input: t, on_ok: Box::new(on_ok) });
    }

    pub fn run_action(&mut self, a: Action) {
        let sel = self.tree.selected();
        let tab_conn = self.tabs.get(self.active).map(|t| (t.conn.clone(), t.db.clone()));
        match a {
            Action::NewConsole => {
                if let Some(n) = sel.as_ref().filter(|_| self.focus == Focus::Tree).or(None) {
                    self.open_console(&n.conn.clone(), n.db.clone())
                } else if let Some((c, d)) = tab_conn {
                    self.open_console(&c, Some(d))
                } else if let Some(n) = sel {
                    self.open_console(&n.conn.clone(), n.db.clone())
                }
            }
            Action::NewDataSource => self.conn_form(None),
            Action::EditDataSource => {
                if let Some(c) = sel.and_then(|n| self.conn(&n.conn).map(|c| c.config.clone())) {
                    self.conn_form(Some(c))
                }
            }
            Action::DeleteDataSource => {
                if let Some(c) = sel.and_then(|n| self.conn(&n.conn).map(|c| c.config.clone())) {
                    self.popup = Some(Popup::Confirm {
                        text: format!("Remove data source \"{}\"?", c.name),
                        on_yes: Box::new(move |a| {
                            let st = a.state.clone();
                            let id = c.id.clone();
                            a.spawn(async move { st.delete_connection(id).await.map_err(err) }, |a, r| {
                                if let Err(e) = r {
                                    a.status = e;
                                }
                                a.reload_conns();
                            });
                        }),
                    });
                }
            }
            Action::Connect => {
                if let Some(n) = sel {
                    self.connect(&n.conn)
                }
            }
            Action::Disconnect => {
                if let Some(n) = sel {
                    self.disconnect(&n.conn)
                }
            }
            Action::Refresh => match (self.focus, sel) {
                (Focus::Tree, Some(n)) => self.refresh_node(&n),
                _ => {
                    if let Some(t) = self.tabs.get(self.active) {
                        let id = t.id;
                        match t.kind {
                            TabKind::Table(_) => self.reload_table(id),
                            TabKind::Redis(_) => self.redis_scan(id, true),
                            _ => {}
                        }
                    }
                }
            },
            Action::Execute => self.execute(false),
            Action::Explain => self.execute(true),
            Action::Cancel => self.cancel(),
            Action::TxToggle | Action::Commit | Action::Rollback => self.tx(a),
            Action::CloseTab => self.close_tab(self.active),
            Action::NextTab => {
                if !self.tabs.is_empty() {
                    self.activate((self.active + 1) % self.tabs.len())
                }
            }
            Action::PrevTab => {
                if !self.tabs.is_empty() {
                    self.activate((self.active + self.tabs.len() - 1) % self.tabs.len())
                }
            }
            Action::GoToObject => self.palette_objects(),
            Action::RecentTabs => self.palette_tabs(),
            Action::Ddl => {
                if let Some(n) = sel.filter(|n| matches!(n.kind, NodeKind::Object(_))) {
                    let k = match &n.kind {
                        NodeKind::Object(k) if k == "function" || k == "procedure" => "routine".to_string(),
                        NodeKind::Object(k) => k.clone(),
                        _ => unreachable!(),
                    };
                    self.open_ddl(&n.conn, n.db.unwrap(), n.schema.unwrap(), n.name.unwrap(), k)
                }
            }
            Action::EditData => {
                if let Some(n) = sel {
                    self.activate_node(&n, true)
                }
            }
            Action::Submit => self.submit(),
            Action::Revert => {
                if let Some(g) = self.grid_mut() {
                    g.revert();
                }
            }
            Action::AddRow => {
                if let Some(Tab { kind: TabKind::Table(tt), .. }) = self.tabs.get_mut(self.active) {
                    if let Some(g) = &mut tt.grid {
                        g.add_row();
                    }
                }
            }
            Action::DeleteRow => {
                if let Some(Tab { kind: TabKind::Table(tt), .. }) = self.tabs.get_mut(self.active) {
                    if let Some(g) = &mut tt.grid {
                        let r = g.row;
                        if g.is_added(r) {
                            g.result.rows.remove(r);
                            g.added -= 1;
                            g.edits.retain(|(rr, _), _| *rr != r);
                            g.row = g.row.min(g.rows().saturating_sub(1));
                        } else if !g.deleted.remove(&r) {
                            g.deleted.insert(r);
                        }
                    }
                }
            }
            Action::SetNull => {
                if let Some(Tab { kind: TabKind::Table(tt), .. }) = self.tabs.get_mut(self.active) {
                    if let Some(g) = &mut tt.grid {
                        g.edits.insert((g.row, g.col), None);
                    }
                }
            }
            Action::ExportCsv => {
                let name = self.tabs.get(self.active).map(|t| t.title.clone()).unwrap_or_default();
                let home = std::env::var("HOME").unwrap_or_default();
                self.input("Export to CSV file", &format!("{home}/Downloads/{}.csv", name.split_whitespace().next().unwrap_or("result")), |a, p| a.export_csv(p));
            }
            Action::ToggleExplorer => {
                if self.focus == Focus::Tree && self.explorer && !self.tabs.is_empty() {
                    self.explorer = false;
                    self.activate(self.active);
                } else {
                    self.explorer = true;
                    self.focus = Focus::Tree;
                }
            }
            Action::ToggleOutput => {
                if let Some(Tab { kind: TabKind::Console(c), .. }) = self.tabs.get_mut(self.active) {
                    c.show_output = !c.show_output;
                }
            }
            Action::Help => self.popup = Some(Popup::Help { scroll: 0 }),
            Action::Quit => self.quit = true,
        }
    }

    pub fn grid_mut(&mut self) -> Option<&mut Grid> {
        match &mut self.tabs.get_mut(self.active)?.kind {
            TabKind::Console(c) => c.grid.as_mut(),
            TabKind::Table(t) => t.grid.as_mut(),
            TabKind::Redis(r) => r.value.as_mut(),
            TabKind::Text(_) => None,
        }
    }
}

pub fn byte_offset(lines: &[String], row: usize, col: usize) -> usize {
    let mut off = 0;
    for l in lines.iter().take(row) {
        off += l.len() + 1;
    }
    let line = lines.get(row).map(|s| s.as_str()).unwrap_or("");
    off + line.char_indices().nth(col).map(|(i, _)| i).unwrap_or(line.len())
}

pub fn selected_text(lines: &[String], r0: usize, c0: usize, r1: usize, c1: usize) -> String {
    let full = lines.join("\n");
    let a = byte_offset(lines, r0, c0);
    let b = byte_offset(lines, r1, c1);
    full[a.min(b)..a.max(b)].to_string()
}

/// Redis 값 → 그리드 (string 은 None: 텍스트로 보여 준다)
pub fn redis_grid(d: &KeyDetail) -> Option<Grid> {
    let arr = d.value.as_array()?;
    let (names, rows): (Vec<&str>, Vec<Vec<Value>>) = match d.ty.as_str() {
        "hash" => (vec!["field", "value"], arr.iter().map(|x| vec![x[0].clone(), x[1].clone()]).collect()),
        "zset" => (vec!["member", "score"], arr.iter().map(|x| vec![x[0].clone(), x[1].clone()]).collect()),
        "list" => (vec!["index", "value"], arr.iter().enumerate().map(|(i, x)| vec![Value::from(i), x.clone()]).collect()),
        "set" => (vec!["member"], arr.iter().map(|x| vec![x.clone()]).collect()),
        "stream" => (vec!["id", "fields"], arr.iter().map(|x| vec![x[0].clone(), Value::from(x[1].to_string())]).collect()),
        _ => return None,
    };
    let result = QueryResult {
        columns: names.iter().map(|n| ColumnInfo { name: n.to_string(), type_name: "TEXT".into() }).collect(),
        rows,
        rows_affected: 0,
        truncated: false,
        elapsed_ms: 0,
        has_result_set: true,
        in_transaction: false,
    };
    Some(Grid::new(result))
}
