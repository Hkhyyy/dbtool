//! ⌃Enter 로 무엇을 실행할지 (DataGrip Settings › Database › Query Execution, GUI 의 src/lib/execPlan.ts 와 같은 규칙)
use dbtool_core::sql;
use dbtool_core::store::DbKind;

/// 커서가 문장 안일 때
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Inside {
    Ask,
    Smallest,
    Whole,
    FromCaret,
}
/// 커서가 문장 밖(빈 줄 등)일 때
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Outside {
    Nothing,
    Whole,
    Below,
}
/// 선택 영역을 실행할 때
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Selection {
    Single,
    Separate,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ExecSettings {
    pub inside: Inside,
    pub outside: Outside,
    pub selection: Selection,
}

/// DataGrip 처럼 커서가 문장 안이면 무엇을 실행할지 묻는다
impl Default for ExecSettings {
    fn default() -> Self {
        ExecSettings { inside: Inside::Ask, outside: Outside::Nothing, selection: Selection::Separate }
    }
}

impl ExecSettings {
    /// workspace-tui.json 에 쓰는 값 (GUI 의 ExecSettings 와 같은 이름)
    pub fn to_json(self) -> serde_json::Value {
        let inside = match self.inside {
            Inside::Ask => "ask",
            Inside::Smallest => "smallest",
            Inside::Whole => "whole",
            Inside::FromCaret => "fromCaret",
        };
        let outside = match self.outside {
            Outside::Nothing => "nothing",
            Outside::Whole => "whole",
            Outside::Below => "below",
        };
        let selection = match self.selection {
            Selection::Single => "single",
            Selection::Separate => "separate",
        };
        serde_json::json!({ "inside": inside, "outside": outside, "selection": selection })
    }

    /// 모르는 값은 기본값
    pub fn from_json(v: &serde_json::Value) -> Self {
        let d = Self::default();
        let g = |k: &str| v.get(k).and_then(serde_json::Value::as_str).unwrap_or_default();
        ExecSettings {
            inside: match g("inside") {
                "ask" => Inside::Ask,
                "smallest" => Inside::Smallest,
                "whole" => Inside::Whole,
                "fromCaret" => Inside::FromCaret,
                _ => d.inside,
            },
            outside: match g("outside") {
                "nothing" => Outside::Nothing,
                "whole" => Outside::Whole,
                "below" => Outside::Below,
                _ => d.outside,
            },
            selection: match g("selection") {
                "single" => Selection::Single,
                "separate" => Selection::Separate,
                _ => d.selection,
            },
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum Plan {
    Run(Vec<String>),
    /// (보기 이름, 실행할 문장들)
    Ask(Vec<(String, Vec<String>)>),
    Nothing,
}

/// 선택 영역이 없을 때 커서 위치(offset, 바이트)로 실행할 문장을 정한다
pub fn plan(text: &str, offset: usize, kind: DbKind, s: ExecSettings) -> Plan {
    // (시작, 끝, 문장) — 앞뒤 공백을 뺀 위치
    let all: Vec<(usize, usize, String)> = sql::split_statements(text, kind)
        .into_iter()
        .map(|(a, b)| {
            let raw = &text[a..b];
            let start = a + (raw.len() - raw.trim_start().len());
            let t = raw.trim();
            (start, start + t.len(), t.to_string())
        })
        .collect();
    let sqls = |v: &[(usize, usize, String)]| v.iter().map(|x| x.2.clone()).collect::<Vec<_>>();
    if all.is_empty() {
        return Plan::Nothing;
    }
    let Some(idx) = all.iter().position(|x| offset >= x.0 && offset <= x.1 + 1) else {
        return match s.outside {
            Outside::Whole => Plan::Run(sqls(&all)),
            Outside::Below => Plan::Run(all.iter().filter(|x| x.0 >= offset).map(|x| x.2.clone()).collect()),
            // 바로 앞 문장이 같은 줄 끝이면 그 문장 (세미콜론 직후 커서)
            Outside::Nothing => match all.iter().rev().find(|x| x.1 <= offset && !text[x.1..offset].contains('\n')) {
                Some(x) => Plan::Run(vec![x.2.clone()]),
                None => Plan::Nothing,
            },
        };
    };
    match s.inside {
        Inside::Smallest => Plan::Run(vec![all[idx].2.clone()]),
        Inside::Whole => Plan::Run(sqls(&all)),
        Inside::FromCaret => Plan::Run(sqls(&all[idx..])),
        Inside::Ask => {
            let label: String = all[idx].2.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(80).collect();
            let mut opts = vec![(label, vec![all[idx].2.clone()])];
            if all.len() > 1 {
                opts.push((format!("All statements ({})", all.len()), sqls(&all)));
            }
            if idx > 0 && idx < all.len() - 1 {
                opts.push(("From caret to end".into(), sqls(&all[idx..])));
            }
            if opts.len() == 1 { Plan::Run(opts.remove(0).1) } else { Plan::Ask(opts) }
        }
    }
}

/// 선택 영역을 실행할 때
pub fn plan_selection(sel: &str, kind: DbKind, s: ExecSettings) -> Plan {
    let v: Vec<String> = match s.selection {
        Selection::Single => vec![sel.trim().to_string()],
        Selection::Separate => sql::split_statements(sel, kind).into_iter().map(|(a, b)| sel[a..b].trim().to_string()).collect(),
    };
    let v: Vec<String> = v.into_iter().filter(|x| !x.is_empty()).collect();
    if v.is_empty() { Plan::Nothing } else { Plan::Run(v) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQL: &str = "BEGIN;\nGRANT a TO x;\n\nGRANT b TO x;\nCOMMIT;";
    fn at(needle: &str) -> usize {
        SQL.find(needle).unwrap() + 1
    }
    fn p(offset: usize, s: ExecSettings) -> Plan {
        plan(SQL, offset, DbKind::Postgres, s)
    }
    fn run(v: &[&str]) -> Plan {
        Plan::Run(v.iter().map(|s| s.to_string()).collect())
    }
    const ALL: [&str; 4] = ["BEGIN", "GRANT a TO x", "GRANT b TO x", "COMMIT"];

    #[test]
    fn default_asks_inside_statement() {
        let Plan::Ask(opts) = p(at("GRANT b"), ExecSettings::default()) else { panic!("should ask") };
        let labels: Vec<_> = opts.iter().map(|o| o.0.as_str()).collect();
        assert_eq!(labels, ["GRANT b TO x", "All statements (4)", "From caret to end"]);
        assert_eq!(opts[1].1, ALL);
        assert_eq!(opts[2].1, ["GRANT b TO x", "COMMIT"]);
        // 첫 문장: '커서부터 끝까지' 는 전체와 같아서 뺀다
        let Plan::Ask(opts) = p(at("BEGIN"), ExecSettings::default()) else { panic!("should ask") };
        assert_eq!(opts.len(), 2);
        // 문장이 하나면 묻지 않는다
        assert_eq!(plan("select 1", 3, DbKind::Postgres, ExecSettings::default()), run(&["select 1"]));
    }

    #[test]
    fn inside_modes() {
        let s = |inside| ExecSettings { inside, ..Default::default() };
        assert_eq!(p(at("GRANT b"), s(Inside::Smallest)), run(&["GRANT b TO x"]));
        assert_eq!(p(at("GRANT b"), s(Inside::Whole)), run(&ALL));
        assert_eq!(p(at("GRANT b"), s(Inside::FromCaret)), run(&["GRANT b TO x", "COMMIT"]));
        // 세미콜론 바로 뒤 커서는 그 문장
        let end = SQL.find("GRANT a TO x;").unwrap() + "GRANT a TO x;".len();
        assert_eq!(p(end, s(Inside::Smallest)), run(&["GRANT a TO x"]));
    }

    #[test]
    fn outside_modes() {
        let blank = SQL.find("\n\n").unwrap() + 1;
        let s = |outside| ExecSettings { outside, ..Default::default() };
        assert_eq!(p(blank, s(Outside::Nothing)), Plan::Nothing);
        assert_eq!(p(blank, s(Outside::Whole)), run(&ALL));
        assert_eq!(p(blank, s(Outside::Below)), run(&["GRANT b TO x", "COMMIT"]));
    }

    #[test]
    fn settings_round_trip() {
        let e = ExecSettings { inside: Inside::FromCaret, outside: Outside::Below, selection: Selection::Single };
        assert_eq!(ExecSettings::from_json(&e.to_json()), e);
        assert_eq!(ExecSettings::from_json(&serde_json::json!({ "inside": "bogus" })), ExecSettings::default());
    }

    #[test]
    fn selection_modes() {
        let sel = "GRANT a TO x;\n\nGRANT b TO x;";
        assert_eq!(plan_selection(sel, DbKind::Postgres, ExecSettings::default()), run(&["GRANT a TO x", "GRANT b TO x"]));
        let single = ExecSettings { selection: Selection::Single, ..Default::default() };
        assert_eq!(plan_selection(sel, DbKind::Postgres, single), run(&[sel]));
    }
}
