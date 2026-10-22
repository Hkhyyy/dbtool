//! Manage Shown Schemas: 트리에 보일 DB·스키마를 고르는 창 (GUI src/components/ScopePopup.tsx 와 같은 규칙).
//! 상태와 체크 규칙만 여기 있고, 그리기는 ui.rs, 키는 input.rs 가 한다
use dbtool_core::meta::SchemaInfo;
use dbtool_core::store::{Scope, SCOPE_ALL, SCOPE_DEFAULT};
use std::collections::{HashMap, HashSet};

/// 창의 한 줄
#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    AllDbs,
    DefaultDb,
    Db(String),
    AllSchemas(String),
    DefaultSchema(String),
    Schema { db: String, name: String, system: bool },
    /// 스키마를 읽는 중이거나 실패 (db, 글)
    Note(String, String),
}

/// 체크 표시
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Check {
    Off,
    On,
    /// All databases / Default database 규칙으로만 보인다
    Implicit,
}

pub struct ScopeEditor {
    pub conn: String,
    pub dbs: Vec<String>,
    pub default_db: String,
    /// 보일 DB ("*" = 전부, "@default" = 기본 DB)
    databases: Vec<String>,
    /// DB → 보일 스키마 (없으면 전부)
    schemas: HashMap<String, Vec<String>>,
    open: HashSet<String>,
    /// 읽은 스키마 (없으면 읽는 중)
    loaded: HashMap<String, Result<Vec<SchemaInfo>, String>>,
    pub sel: usize,
}

impl ScopeEditor {
    /// focus_db: DB 줄에서 열었으면 그 DB 를 펼치고 고른 채로 (스키마는 불러와서 set_schemas 로 넣는다)
    pub fn new(conn: String, dbs: Vec<String>, default_db: String, scope: &Scope, focus_db: Option<&str>) -> Self {
        let databases = match &scope.databases {
            Some(d) if !d.is_empty() => d.clone(),
            _ => vec![SCOPE_ALL.into()],
        };
        let mut e = ScopeEditor { conn, dbs, default_db, databases, schemas: scope.schemas.clone(), open: HashSet::new(), loaded: HashMap::new(), sel: 0 };
        if let Some(db) = focus_db {
            e.open.insert(db.to_string());
            e.sel = e.rows().iter().position(|r| *r == Row::Db(db.to_string())).unwrap_or(0);
        }
        e
    }

    pub fn rows(&self) -> Vec<Row> {
        let mut out = vec![Row::AllDbs, Row::DefaultDb];
        for db in &self.dbs {
            out.push(Row::Db(db.clone()));
            if !self.open.contains(db) {
                continue;
            }
            match self.loaded.get(db) {
                None => out.push(Row::Note(db.clone(), "Loading…".into())),
                Some(Err(e)) => out.push(Row::Note(db.clone(), e.clone())),
                Some(Ok(list)) => {
                    out.push(Row::AllSchemas(db.clone()));
                    out.push(Row::DefaultSchema(db.clone()));
                    // 시스템 스키마 먼저, 그다음 이름순 (GUI 와 같다)
                    let mut list: Vec<&SchemaInfo> = list.iter().collect();
                    list.sort_by(|a, b| b.is_system.cmp(&a.is_system).then_with(|| a.name.cmp(&b.name)));
                    out.extend(list.into_iter().map(|s| Row::Schema { db: db.clone(), name: s.name.clone(), system: s.is_system }));
                }
            }
        }
        out
    }

    /// 이 DB 에서 보일 스키마 (필터가 없으면 전부)
    fn sch_list(&self, db: &str) -> Vec<String> {
        self.schemas.get(db).cloned().unwrap_or_else(|| vec![SCOPE_ALL.into()])
    }

    pub fn check(&self, row: &Row) -> Check {
        let on = |b: bool| if b { Check::On } else { Check::Off };
        let has = |v: &[String], x: &str| v.iter().any(|d| d == x);
        match row {
            Row::AllDbs => on(has(&self.databases, SCOPE_ALL)),
            Row::DefaultDb => on(has(&self.databases, SCOPE_DEFAULT)),
            Row::Db(db) if has(&self.databases, db) => Check::On,
            Row::Db(db) if has(&self.databases, SCOPE_ALL) || (has(&self.databases, SCOPE_DEFAULT) && *db == self.default_db) => Check::Implicit,
            Row::Db(_) | Row::Note(..) => Check::Off,
            Row::AllSchemas(db) => on(has(&self.sch_list(db), SCOPE_ALL)),
            Row::DefaultSchema(db) => on(has(&self.sch_list(db), SCOPE_DEFAULT)),
            Row::Schema { db, name, .. } => {
                let cur = self.sch_list(db);
                on(has(&cur, SCOPE_ALL) || has(&cur, name) || (has(&cur, SCOPE_DEFAULT) && name == "public"))
            }
        }
    }

    /// Space: 고른 줄을 켜고 끈다
    pub fn toggle(&mut self) {
        fn flip(v: &mut Vec<String>, x: &str) {
            if let Some(i) = v.iter().position(|d| d == x) {
                v.remove(i);
            } else {
                v.push(x.to_string());
            }
        }
        let Some(row) = self.rows().get(self.sel).cloned() else { return };
        let (db, x) = match row {
            Row::AllDbs => return flip(&mut self.databases, SCOPE_ALL),
            Row::DefaultDb => return flip(&mut self.databases, SCOPE_DEFAULT),
            Row::Db(db) => return flip(&mut self.databases, &db),
            Row::Note(..) => return,
            Row::AllSchemas(db) => (db, SCOPE_ALL.to_string()),
            Row::DefaultSchema(db) => (db, SCOPE_DEFAULT.to_string()),
            Row::Schema { db, name, .. } => (db, name),
        };
        let mut cur = self.sch_list(&db);
        flip(&mut cur, &x);
        if cur.is_empty() {
            self.schemas.remove(&db);
        } else {
            self.schemas.insert(db, cur);
        }
    }

    /// →: DB 를 펼친다. 아직 스키마를 안 읽었으면 그 DB 이름 (불러올 것)
    pub fn expand(&mut self) -> Option<String> {
        let Some(Row::Db(db)) = self.rows().get(self.sel).cloned() else { return None };
        self.open.insert(db.clone());
        (!self.loaded.contains_key(&db)).then_some(db)
    }

    /// ←: DB 를 접는다 (스키마 줄에서는 그 DB 로)
    pub fn collapse(&mut self) {
        let parent = match self.rows().get(self.sel).cloned() {
            Some(Row::Db(db)) => {
                self.open.remove(&db);
                return;
            }
            Some(Row::AllSchemas(db) | Row::DefaultSchema(db) | Row::Schema { db, .. } | Row::Note(db, _)) => db,
            _ => return,
        };
        self.sel = self.rows().iter().position(|r| *r == Row::Db(parent.clone())).unwrap_or(self.sel);
    }

    pub fn set_schemas(&mut self, db: &str, r: Result<Vec<SchemaInfo>, String>) {
        self.loaded.insert(db.to_string(), r);
    }

    pub fn move_by(&mut self, d: isize) {
        let n = self.rows().len();
        self.sel = (self.sel as isize + d).clamp(0, n.saturating_sub(1) as isize) as usize;
    }

    /// 저장할 범위 ("전부"는 필터 없음으로 정리)
    pub fn scope(&self) -> Scope {
        let all = |v: &Vec<String>| v.len() == 1 && v[0] == SCOPE_ALL;
        Scope {
            databases: (!self.databases.is_empty() && !all(&self.databases)).then(|| self.databases.clone()),
            schemas: self.schemas.iter().filter(|(_, v)| !all(v)).map(|(k, v)| (k.clone(), v.clone())).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sch(names: &[(&str, bool)]) -> Vec<SchemaInfo> {
        names.iter().map(|(n, s)| SchemaInfo { name: n.to_string(), is_system: *s }).collect()
    }

    fn editor(scope: Scope) -> ScopeEditor {
        ScopeEditor::new("c1".into(), vec!["app".into(), "postgres".into(), "shop".into()], "postgres".into(), &scope, None)
    }

    fn select(e: &mut ScopeEditor, row: Row) {
        e.sel = e.rows().iter().position(|r| *r == row).unwrap_or_else(|| panic!("no row {row:?} in {:?}", e.rows()));
    }

    /// 범위가 없으면 All databases 만 켜져 있고, DB 들은 그 규칙으로 보인다
    #[test]
    fn starts_from_all_databases() {
        let e = editor(Scope::default());
        assert_eq!(e.rows(), vec![Row::AllDbs, Row::DefaultDb, Row::Db("app".into()), Row::Db("postgres".into()), Row::Db("shop".into())]);
        assert_eq!(e.check(&Row::AllDbs), Check::On);
        assert_eq!(e.check(&Row::DefaultDb), Check::Off);
        assert_eq!(e.check(&Row::Db("shop".into())), Check::Implicit);
        assert_eq!(e.scope(), Scope::default());
    }

    /// DB 를 골라 보이게: All databases 를 끄고 shop 을 켜면 shop 만
    #[test]
    fn pick_databases() {
        let mut e = editor(Scope::default());
        select(&mut e, Row::AllDbs);
        e.toggle();
        select(&mut e, Row::Db("shop".into()));
        e.toggle();
        assert_eq!(e.check(&Row::Db("shop".into())), Check::On);
        assert_eq!(e.check(&Row::Db("app".into())), Check::Off);
        assert_eq!(e.scope().databases, Some(vec!["shop".to_string()]));
        // Default database 를 켜면 postgres 도 (규칙으로)
        select(&mut e, Row::DefaultDb);
        e.toggle();
        assert_eq!(e.check(&Row::Db("postgres".into())), Check::Implicit);
        assert_eq!(e.scope().databases, Some(vec!["shop".to_string(), SCOPE_DEFAULT.to_string()]));
    }

    /// DB 를 펼치면 스키마를 불러오고 (시스템 스키마 먼저), 하나만 고르면 그 DB 는 그 스키마만
    #[test]
    fn pick_schemas_of_a_database() {
        let mut e = editor(Scope::default());
        select(&mut e, Row::Db("shop".into()));
        assert_eq!(e.expand(), Some("shop".into()));
        assert!(e.rows().contains(&Row::Note("shop".into(), "Loading…".into())));
        e.set_schemas("shop", Ok(sch(&[("public", false), ("sales", false), ("pg_catalog", true)])));
        let rows = e.rows();
        let i = rows.iter().position(|r| *r == Row::Db("shop".into())).unwrap();
        assert_eq!(
            rows[i + 1..].to_vec(),
            vec![
                Row::AllSchemas("shop".into()),
                Row::DefaultSchema("shop".into()),
                Row::Schema { db: "shop".into(), name: "pg_catalog".into(), system: true },
                Row::Schema { db: "shop".into(), name: "public".into(), system: false },
                Row::Schema { db: "shop".into(), name: "sales".into(), system: false },
            ]
        );
        // 필터가 없으면 All schemas 가 켜져 있다
        assert_eq!(e.check(&Row::AllSchemas("shop".into())), Check::On);
        // GUI 와 같은 순서: sales 를 켜고 All schemas 를 끈다 (전부 끄면 다시 "전부")
        select(&mut e, Row::Schema { db: "shop".into(), name: "sales".into(), system: false });
        e.toggle();
        select(&mut e, Row::AllSchemas("shop".into()));
        e.toggle();
        assert_eq!(e.check(&Row::Schema { db: "shop".into(), name: "public".into(), system: false }), Check::Off);
        assert_eq!(e.scope().schemas.get("shop"), Some(&vec!["sales".to_string()]));
        // 스키마 줄에서 ← 는 DB 로 가고, 한 번 더 ← 는 접는다
        e.collapse();
        assert_eq!(e.rows()[e.sel], Row::Db("shop".into()));
        e.collapse();
        assert!(!e.rows().contains(&Row::AllSchemas("shop".into())));
        // 이미 읽었으면 다시 펼칠 때 또 부르지 않는다
        assert_eq!(e.expand(), None);
    }

    /// 지금 범위에서 시작: Default schema 는 public 을 켠 것으로 보인다
    #[test]
    fn starts_from_saved_scope() {
        let scope = Scope { databases: Some(vec!["shop".into()]), schemas: HashMap::from([("shop".to_string(), vec![SCOPE_DEFAULT.to_string()])]) };
        let mut e = ScopeEditor::new("c1".into(), vec!["app".into(), "shop".into()], "app".into(), &scope, Some("shop"));
        assert_eq!(e.rows()[e.sel], Row::Db("shop".into()));
        e.set_schemas("shop", Ok(sch(&[("public", false), ("sales", false)])));
        assert_eq!(e.check(&Row::AllDbs), Check::Off);
        assert_eq!(e.check(&Row::DefaultSchema("shop".into())), Check::On);
        assert_eq!(e.check(&Row::Schema { db: "shop".into(), name: "public".into(), system: false }), Check::On);
        assert_eq!(e.check(&Row::Schema { db: "shop".into(), name: "sales".into(), system: false }), Check::Off);
        assert_eq!(e.scope(), scope);
    }

    /// 실패하면 그 줄에 오류를 보인다
    #[test]
    fn schema_load_error_is_shown() {
        let mut e = editor(Scope::default());
        select(&mut e, Row::Db("app".into()));
        e.expand();
        e.set_schemas("app", Err("permission denied".into()));
        assert!(e.rows().contains(&Row::Note("app".into(), "permission denied".into())));
    }
}
