//! DB 종류별 SQL 유틸 (TUI·MCP 공용). 프론트엔드 lib/api.ts 의 Dialect 와 같은 규칙.
use crate::store::{ConnectionConfig, DbKind};

/// 접속 대상 id: "<data source id>::<database>"
pub fn target_id(conn: &str, db: Option<&str>) -> String {
    match db.filter(|d| !d.is_empty()) {
        Some(d) => format!("{conn}::{d}"),
        None => conn.to_string(),
    }
}

/// 트리·콘솔의 기본 DB 이름
pub fn default_db(cfg: &ConnectionConfig) -> String {
    match cfg.kind {
        DbKind::Postgres => if cfg.database.trim().is_empty() { "postgres".into() } else { cfg.database.trim().into() },
        DbKind::Mysql => cfg.database.trim().into(),
        DbKind::Redis => format!("db{}", crate::redisdb::db_index(&cfg.database)),
    }
}

/// 따옴표 없이 쓰면 문법이 깨지는 예약어 (PG·MySQL 합집합. 불필요하게 따옴표를 쳐도 해는 없다)
const RESERVED: &[&str] = &[
    "all", "analyse", "analyze", "and", "any", "array", "as", "asc", "asymmetric", "between", "both", "by", "case", "cast", "change", "check",
    "collate", "column", "condition", "constraint", "create", "cross", "current_date", "current_role", "current_time", "current_timestamp",
    "current_user", "database", "default", "deferrable", "delete", "desc", "distinct", "do", "drop", "else", "end", "except", "exists", "false",
    "fetch", "for", "foreign", "from", "full", "grant", "group", "having", "in", "index", "initially", "inner", "insert", "interval", "intersect",
    "into", "is", "join", "key", "keys", "lateral", "leading", "left", "like", "limit", "localtime", "localtimestamp", "match", "natural", "not",
    "null", "offset", "on", "only", "or", "order", "outer", "placing", "primary", "range", "rank", "read", "references", "returning", "right",
    "row", "rows", "select", "session_user", "set", "some", "symmetric", "table", "then", "to", "trailing", "true", "union", "unique", "update",
    "usage", "user", "using", "values", "variadic", "when", "where", "window", "with", "write",
];

pub fn quote_ident(kind: DbKind, s: &str) -> String {
    let reserved = RESERVED.contains(&s.to_lowercase().as_str());
    let simple = |lower_only: bool| {
        let mut ch = s.chars();
        !reserved
            && matches!(ch.next(), Some(c) if c == '_' || if lower_only { c.is_ascii_lowercase() } else { c.is_ascii_alphabetic() })
            && s.chars().all(|c| c == '_' || c.is_ascii_digit() || if lower_only { c.is_ascii_lowercase() } else { c.is_ascii_alphabetic() })
    };
    match kind {
        DbKind::Mysql => if simple(false) { s.into() } else { format!("`{}`", s.replace('`', "``")) },
        DbKind::Redis => s.into(),
        DbKind::Postgres => if simple(true) { s.into() } else { format!("\"{}\"", s.replace('"', "\"\"")) },
    }
}

pub fn literal(kind: DbKind, v: Option<&str>) -> String {
    match v {
        None => "NULL".into(),
        Some(v) if kind == DbKind::Mysql => format!("'{}'", v.replace('\\', "\\\\").replace('\'', "''")),
        Some(v) => format!("'{}'", v.replace('\'', "''")),
    }
}

/// 세미콜론으로 문장을 나눈다 (문자열·주석·PG 달러 인용 건너뜀). Redis 는 한 줄 = 명령. (start, end) 바이트 범위.
/// 서버와 같은 규칙으로 읽어야 한다 (읽기 전용 검사가 이걸로 문장 수를 센다):
/// MySQL 은 `--` 뒤에 공백이 있어야 주석이고 `/*! … */` 는 실행되는 코드, PG 는 주석이 중첩되고 E'…' 안에서 \ 가 이스케이프
pub fn split_statements(text: &str, kind: DbKind) -> Vec<(usize, usize)> {
    if kind == DbKind::Redis {
        let mut out = Vec::new();
        let mut pos = 0;
        for line in text.split('\n') {
            let t = line.trim();
            if !t.is_empty() && !t.starts_with('#') && !t.starts_with("//") {
                out.push((pos, pos + line.len()));
            }
            pos += line.len() + 1;
        }
        return out;
    }
    let b = text.as_bytes();
    let n = b.len();
    let backslash = kind == DbKind::Mysql;
    let (mut out, mut start, mut i) = (Vec::new(), 0usize, 0usize);
    while i < n {
        let c = b[i];
        let d = if i + 1 < n { b[i + 1] } else { 0 };
        if c == b'-' && d == b'-' && (kind != DbKind::Mysql || i + 2 >= n || b[i + 2].is_ascii_whitespace() || b[i + 2].is_ascii_control()) {
            i = text[i..].find('\n').map(|j| i + j + 1).unwrap_or(n);
            continue;
        }
        if c == b'#' && kind == DbKind::Mysql {
            i = text[i..].find('\n').map(|j| i + j + 1).unwrap_or(n);
            continue;
        }
        if c == b'/' && d == b'*' {
            // MySQL /*! … */ 는 주석이 아니라 실행되는 코드: 안쪽도 그대로 읽는다
            if kind == DbKind::Mysql && i + 2 < n && b[i + 2] == b'!' {
                i += 3;
                continue;
            }
            // PG 주석은 중첩된다
            let (mut depth, mut j) = (1, i + 2);
            while j < n && depth > 0 {
                if b[j] == b'/' && j + 1 < n && b[j + 1] == b'*' && kind == DbKind::Postgres {
                    depth += 1;
                    j += 2;
                } else if b[j] == b'*' && j + 1 < n && b[j + 1] == b'/' {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            i = j;
            continue;
        }
        if c == b'\'' || c == b'"' || c == b'`' {
            // PG E'…' 문자열은 \ 가 이스케이프
            let e_string = kind == DbKind::Postgres && c == b'\'' && i > 0 && (b[i - 1] == b'e' || b[i - 1] == b'E') && (i < 2 || !(b[i - 2].is_ascii_alphanumeric() || b[i - 2] == b'_'));
            let mut j = i + 1;
            while j < n {
                if (backslash || e_string) && b[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if b[j] == c {
                    if j + 1 < n && b[j + 1] == c {
                        j += 2;
                        continue;
                    }
                    break;
                }
                j += 1;
            }
            i = (j + 1).min(n);
            continue;
        }
        if c == b'$' && kind == DbKind::Postgres {
            let rest = &text[i..];
            let tag_end = rest[1..].find('$').map(|j| j + 2);
            if let Some(te) = tag_end {
                let tag = &rest[..te];
                if tag[1..te - 1].chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') && !tag[1..].starts_with(|ch: char| ch.is_ascii_digit()) {
                    i = text[i + te..].find(tag).map(|j| i + te + j + te).unwrap_or(n);
                    continue;
                }
            }
        }
        if c == b';' {
            out.push((start, i));
            start = i + 1;
        }
        i += 1;
    }
    out.push((start, n));
    out.into_iter().filter(|(s, e)| !text[*s..*e].trim().is_empty()).collect()
}

/// 캐럿 위치(바이트)의 문장. 문장 사이면 바로 앞 문장
pub fn statement_at(text: &str, offset: usize, kind: DbKind) -> Option<String> {
    let segs = split_statements(text, kind);
    let pick = segs.iter().find(|(s, e)| offset >= *s && offset <= e + 1).or_else(|| segs.iter().rev().find(|(_, e)| *e < offset)).or(segs.first())?;
    Some(text[pick.0..pick.1].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_and_quote() {
        let t = "select ';' as a; -- x;\nselect $$a;b$$; select \"x;y\"";
        let s: Vec<_> = split_statements(t, DbKind::Postgres).into_iter().map(|(a, b)| t[a..b].trim().to_string()).collect();
        assert_eq!(s, ["select ';' as a", "-- x;\nselect $$a;b$$", "select \"x;y\""]);
        assert_eq!(statement_at(t, 3, DbKind::Postgres).unwrap(), "select ';' as a");
        let m = "select 'a\\';b'; select 2";
        assert_eq!(split_statements(m, DbKind::Mysql).len(), 2);
        assert_eq!(split_statements("GET a\n# c\n\nSET b 1", DbKind::Redis).len(), 2);
        assert_eq!(quote_ident(DbKind::Postgres, "User"), "\"User\"");
        assert_eq!(quote_ident(DbKind::Mysql, "my col"), "`my col`");
        assert_eq!(quote_ident(DbKind::Postgres, "user"), "\"user\"");
        assert_eq!(quote_ident(DbKind::Mysql, "order"), "`order`");
        assert_eq!(quote_ident(DbKind::Postgres, "users"), "users");
        assert_eq!(literal(DbKind::Mysql, Some("a\\'b")), "'a\\\\''b'");
        assert_eq!(literal(DbKind::Postgres, None), "NULL");
    }

    #[test]
    fn split_follows_server_comment_rules() {
        let n = |t: &str, k| split_statements(t, k).len();
        // MySQL: `--x` 는 주석이 아니다 → 문장 셋 (실제로 MCP 읽기 전용이 뚫렸던 형태)
        assert_eq!(n("select 1 --1; COMMIT; DELETE FROM t", DbKind::Mysql), 3);
        assert_eq!(n("select 1 -- x; delete", DbKind::Mysql), 1);
        // MySQL /*! … */ 안은 코드
        assert_eq!(n("/*! COMMIT; DELETE FROM t; */ select 1", DbKind::Mysql), 3);
        assert_eq!(n("/* c; */ select 1", DbKind::Mysql), 1);
        // PG: 중첩 주석, E'' 의 \'
        assert_eq!(n("/* a /* b */ ; */ select 1", DbKind::Postgres), 1);
        assert_eq!(n("select E'\\'; x'; select 2", DbKind::Postgres), 2);
        assert_eq!(n("select '\\'; select 2", DbKind::Postgres), 2);
    }
}
