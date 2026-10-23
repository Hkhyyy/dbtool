//! 콘솔 자동완성 (GUI src/lib/completion.ts 의 TUI 판)
//! - `alias.` / `table.` → 그 테이블의 컬럼, `schema.` → 그 스키마의 테이블
//! - 그 밖: 문장에 나온 테이블의 컬럼 → 테이블 → 스키마 → 키워드
use qurry_core::meta::CompletionTable;
use std::collections::HashMap;

/// 자동완성 메타: 불러올 때 한 번 소문자 이름과 이름별 색인을 만들어 둔다 (키마다 다시 만들지 않게)
pub struct Meta {
    pub tables: Vec<CompletionTable>,
    /// (스키마, 테이블) 소문자
    lower: Vec<(String, String)>,
    /// 소문자 테이블 이름 → tables 위치들
    by_name: HashMap<String, Vec<usize>>,
}

impl Meta {
    pub fn new(tables: Vec<CompletionTable>) -> Self {
        let lower: Vec<(String, String)> = tables.iter().map(|t| (t.schema.to_lowercase(), t.name.to_lowercase())).collect();
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, (_, n)) in lower.iter().enumerate() {
            by_name.entry(n.clone()).or_default().push(i);
        }
        Meta { tables, lower, by_name }
    }
}

const KEYWORDS: &str = "select from where and or not in is null like ilike between exists group by order having limit offset join left right inner outer full cross on using as \
distinct union all intersect except insert into values update set delete returning create table view index alter drop truncate with case when then else end \
asc desc nulls first last count sum avg min max coalesce now current_date current_timestamp interval true false begin commit rollback explain analyze";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ItemKind {
    Column,
    Table,
    Schema,
    Keyword,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub label: String,
    pub detail: String,
    pub insert: String,
    pub kind: ItemKind,
}

fn keywords() -> impl Iterator<Item = &'static str> {
    KEYWORDS.split_whitespace()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `from|join|update|into <ref> [as] [alias]` 를 찾는다
fn tables_in_statement(text: &str) -> Vec<(String, Option<String>)> {
    let toks: Vec<String> = tokenize(text);
    let mut out = vec![];
    for (i, t) in toks.iter().enumerate() {
        if !["from", "join", "update", "into"].contains(&t.to_lowercase().as_str()) {
            continue;
        }
        let Some(r) = toks.get(i + 1).filter(|r| r.chars().next().is_some_and(|c| is_word(c) || c == '"')) else { continue };
        let mut j = i + 2;
        if toks.get(j).is_some_and(|t| t.eq_ignore_ascii_case("as")) {
            j += 1;
        }
        let alias = toks.get(j).filter(|a| a.chars().all(is_word) && !keywords().any(|k| k.eq_ignore_ascii_case(a))).cloned();
        out.push((r.replace('"', ""), alias));
    }
    out
}

/// 단어(`schema.table`, `"quoted"` 포함) 와 기호로 나눈다
fn tokenize(text: &str) -> Vec<String> {
    let cs: Vec<char> = text.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if is_word(c) || c == '"' {
            let start = i;
            loop {
                if i < cs.len() && cs[i] == '"' {
                    i += 1;
                    while i < cs.len() && cs[i] != '"' {
                        i += 1;
                    }
                    i = (i + 1).min(cs.len());
                } else {
                    while i < cs.len() && is_word(cs[i]) {
                        i += 1;
                    }
                }
                // schema.table 은 한 토큰
                if i + 1 < cs.len() && cs[i] == '.' && (is_word(cs[i + 1]) || cs[i + 1] == '"') {
                    i += 1;
                    continue;
                }
                break;
            }
            out.push(cs[start..i].iter().collect());
        } else {
            if !c.is_whitespace() {
                out.push(c.to_string());
            }
            i += 1;
        }
    }
    out
}

fn find_table<'a>(m: &'a Meta, r: &str, db: &str) -> Option<&'a CompletionTable> {
    let r = r.to_lowercase();
    let (schema, name) = match r.split_once('.') {
        Some((s, n)) => (Some(s), n),
        None => (None, r.as_str()),
    };
    let hits = m.by_name.get(name)?;
    let t = |i: &usize| &m.tables[*i];
    match schema {
        Some(s) => hits.iter().find(|&&i| m.lower[i].0 == s).map(t),
        None => hits.iter().find(|&&i| m.tables[i].schema == "public" || m.tables[i].schema == db).or(hits.first()).map(t),
    }
}

/// 커서 앞 텍스트로 후보를 만든다. 반환: (바꿀 접두어의 글자 수, 후보)
/// `text` 는 현재 문장 전체, `before` 는 커서가 있는 줄의 커서 앞부분, `db` 는 탭의 DB (MySQL 은 스키마 = DB)
pub fn suggest(m: &Meta, text: &str, before: &str, db: &str) -> (usize, Vec<Item>) {
    let all = &m.tables;
    let prefix: String = before.chars().rev().take_while(|c| is_word(*c)).collect::<Vec<_>>().into_iter().rev().collect();
    let head = &before[..before.len() - prefix.len()];
    let p = prefix.to_lowercase();
    // 이미 다 친 단어와 똑같은 후보는 빼서 Enter 가 줄바꿈으로 가게 한다
    let matches = |label: &str| p.is_empty() || (label.to_lowercase().starts_with(&p) && label.to_lowercase() != p);
    let n = prefix.chars().count();

    if let Some(h) = head.strip_suffix('.') {
        let q: String = h.chars().rev().take_while(|c| is_word(*c) || *c == '"').collect::<Vec<_>>().into_iter().rev().collect();
        let q = q.replace('"', "").to_lowercase();
        if q.is_empty() {
            return (n, vec![]);
        }
        // alias. 또는 table. → 컬럼
        let refs = tables_in_statement(text);
        let hit = refs
            .iter()
            .find(|(_, a)| a.as_ref().is_some_and(|a| a.to_lowercase() == q))
            .or_else(|| refs.iter().find(|(r, _)| r.rsplit('.').next().unwrap_or(r).to_lowercase() == q));
        let t = hit.and_then(|(r, _)| find_table(m, r, db)).or_else(|| find_table(m, &q, db));
        if let Some(t) = t {
            let items = t.columns.iter().filter(|(c, _)| matches(c)).map(|(c, ty)| Item { label: c.clone(), detail: ty.clone(), insert: c.clone(), kind: ItemKind::Column }).collect();
            return (n, items);
        }
        // schema. → 테이블
        let items = all
            .iter()
            .zip(&m.lower)
            .filter(|(_, (ls, ln))| *ls == q && (p.is_empty() || (ln.starts_with(&p) && *ln != p)))
            .map(|(t, _)| t)
            .map(|t| Item { label: t.name.clone(), detail: format!("{} cols", t.columns.len()), insert: t.name.clone(), kind: ItemKind::Table })
            .collect();
        return (n, items);
    }

    let mut items: Vec<Item> = vec![];
    for (r, _) in tables_in_statement(text) {
        if let Some(t) = find_table(m, &r, db) {
            for (c, ty) in &t.columns {
                if matches(c) && !items.iter().any(|i| &i.label == c) {
                    items.push(Item { label: c.clone(), detail: format!("{ty} · {}", t.name), insert: c.clone(), kind: ItemKind::Column });
                }
            }
        }
    }
    let mut tables: Vec<Item> = all
        .iter()
        .zip(&m.lower)
        .filter(|(_, (_, ln))| p.is_empty() || (ln.starts_with(&p) && *ln != p))
        .map(|(t, _)| Item {
            label: t.name.clone(),
            detail: t.schema.clone(),
            insert: if t.schema == "public" || t.schema == db { t.name.clone() } else { format!("{}.{}", t.schema, t.name) },
            kind: ItemKind::Table,
        })
        .collect();
    tables.sort_by(|a, b| a.label.cmp(&b.label));
    items.extend(tables);
    let mut schemas: Vec<&str> = all.iter().map(|t| t.schema.as_str()).filter(|s| matches(s)).collect();
    schemas.sort();
    schemas.dedup();
    items.extend(schemas.into_iter().map(|s| Item { label: s.into(), detail: "schema".into(), insert: s.into(), kind: ItemKind::Schema }));
    // 키워드는 한 글자 이상 쳤을 때만 (빈 접두어에 키워드 수십 개는 방해)
    if !p.is_empty() {
        items.extend(keywords().filter(|k| matches(k)).map(|k| Item { label: k.into(), detail: "keyword".into(), insert: k.into(), kind: ItemKind::Keyword }));
    }
    (n, items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> Meta {
        let t = |s: &str, n: &str, cols: &[&str]| CompletionTable { schema: s.into(), name: n.into(), columns: cols.iter().map(|c| (c.to_string(), "text".to_string())).collect() };
        Meta::new(vec![t("public", "users", &["id", "user_name", "email"]), t("public", "orders", &["id", "user_id", "total"]), t("billing", "invoices", &["id", "amount"])])
    }

    fn labels(v: &[Item]) -> Vec<&str> {
        v.iter().map(|i| i.label.as_str()).collect()
    }

    #[test]
    fn alias_dot_gives_columns() {
        let text = "select u. from users u join orders o on o.user_id = u.id";
        let (n, v) = suggest(&meta(), text, "select u.", "app");
        assert_eq!(n, 0);
        assert_eq!(labels(&v), ["id", "user_name", "email"]);
        let (n, v) = suggest(&meta(), text, "select u.us", "app");
        assert_eq!(n, 2);
        assert_eq!(labels(&v), ["user_name"]);
    }

    #[test]
    fn table_dot_and_schema_dot() {
        let (_, v) = suggest(&meta(), "select orders. from orders", "select orders.", "app");
        assert_eq!(labels(&v), ["id", "user_id", "total"]);
        let (_, v) = suggest(&meta(), "select * from billing.", "select * from billing.", "app");
        assert_eq!(labels(&v), ["invoices"]);
    }

    #[test]
    fn plain_prefix_orders_columns_tables_schemas_keywords() {
        let (n, v) = suggest(&meta(), "select * from users where u", "select * from users where u", "app");
        assert_eq!(n, 1);
        assert_eq!(labels(&v), ["user_name", "users", "using", "union", "update"]);
        let (_, v) = suggest(&meta(), "select * from inv", "select * from inv", "app");
        assert_eq!(v[0].insert, "billing.invoices");
    }

    #[test]
    fn typed_word_is_not_suggested_again() {
        let (_, v) = suggest(&meta(), "select * from users", "select * from users", "app");
        assert!(v.is_empty(), "{:?}", labels(&v));
        let (_, v) = suggest(&meta(), "sel", "sel", "app");
        assert_eq!(labels(&v), ["select"]);
    }

    #[test]
    fn mysql_schema_is_db() {
        let all = Meta::new(vec![CompletionTable { schema: "shop".into(), name: "items".into(), columns: vec![] }]);
        let (_, v) = suggest(&all, "select * from it", "select * from it", "shop");
        assert_eq!(v[0].insert, "items");
    }
}
