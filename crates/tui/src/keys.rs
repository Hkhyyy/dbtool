//! 바꿀 수 있는 명령 단축키. 기본값은 DEFAULT_KEYS, 사용자 설정은 keymap.json 의 "tui" 구역 (기본값과 다른 것만)
//! 키 표기: "Ctrl+L", "Alt+Shift+X", "F5", "Ctrl+Enter", "Alt+\\" … (수식키 순서: Ctrl, Alt, Shift)
use crate::app::Action;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

pub const DEFAULT_KEYS: &[(Action, &[&str])] = &[
    (Action::Quit, &["Ctrl+Q"]),
    (Action::Help, &["F1"]),
    (Action::Palette, &["Ctrl+K"]),
    (Action::SearchEverywhere, &["Ctrl+P"]),
    (Action::GoToObject, &["Ctrl+O"]),
    (Action::Ddl, &["Ctrl+B"]),
    (Action::RecentTabs, &["Ctrl+E"]),
    (Action::NewConsole, &["Ctrl+L"]),
    (Action::CloseTab, &["Ctrl+W"]),
    (Action::NextTab, &["Alt+Right", "Ctrl+PgDn"]),
    (Action::PrevTab, &["Alt+Left", "Ctrl+PgUp"]),
    (Action::ToggleExplorer, &["Alt+1"]),
    (Action::CycleFocus, &["F6"]),
    (Action::SplitRight, &["Alt+\\"]),
    (Action::SplitDown, &["Alt+-"]),
    (Action::OtherSplit, &["Alt+Tab"]),
    (Action::SplitShrink, &["Alt+Shift+Left"]),
    (Action::SplitGrow, &["Alt+Shift+Right"]),
    (Action::ExplorerNarrower, &["Alt+["]),
    (Action::ExplorerWider, &["Alt+]"]),
    (Action::Execute, &["F5", "Ctrl+Enter", "Alt+Enter"]),
    (Action::Explain, &["Alt+E"]),
    (Action::Complete, &["Ctrl+Space"]),
    (Action::Cancel, &["Ctrl+C"]),
    (Action::TxToggle, &["Alt+T"]),
    (Action::Commit, &["Alt+C"]),
    (Action::Rollback, &["Alt+R"]),
    (Action::ToggleOutput, &["Alt+O"]),
    // 칸 이동은 ⌃⌥ + 방향키 (⌥↑/↓, ⌃⇧←/→ 는 예전 키)
    (Action::PaneUp, &["Ctrl+Alt+Up", "Alt+Up"]),
    (Action::PaneDown, &["Ctrl+Alt+Down", "Alt+Down"]),
    (Action::PaneLeft, &["Ctrl+Alt+Left", "Ctrl+Shift+Left"]),
    (Action::PaneRight, &["Ctrl+Alt+Right", "Ctrl+Shift+Right"]),
    (Action::ShrinkEditor, &["Alt+Shift+Up"]),
    (Action::GrowEditor, &["Alt+Shift+Down"]),
    (Action::Submit, &["Ctrl+S"]),
    (Action::Disconnect, &["Ctrl+F2"]),
];

/// keymap.json 에 쓰는 이름 (enum 이름 그대로)
pub fn action_id(a: Action) -> String {
    format!("{a:?}")
}

fn defaults(a: Action) -> Vec<String> {
    DEFAULT_KEYS.iter().find(|d| d.0 == a).map(|d| d.1.iter().map(|s| s.to_string()).collect()).unwrap_or_default()
}

/// 키 입력 → 표기. 바꿀 수 있는 키가 아니면(수식키 없는 글자 등) None
pub fn spec(k: &KeyEvent) -> Option<String> {
    let mut ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    let mut shift = k.modifiers.contains(KeyModifiers::SHIFT);
    let key = match k.code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) if c.is_ascii_alphabetic() => {
            shift |= c.is_ascii_uppercase();
            c.to_ascii_uppercase().to_string()
        }
        // 한국어 자판은 \ 키가 ₩ 로 들어온다
        KeyCode::Char('₩') => {
            shift = false;
            "\\".to_string()
        }
        KeyCode::Char(c) => {
            // ? ! 같은 기호는 Shift 가 글자에 이미 들어 있다
            shift = false;
            c.to_string()
        }
        KeyCode::Null => {
            // 일부 터미널의 Ctrl+Space
            ctrl = true;
            "Space".to_string()
        }
        KeyCode::Enter => "Enter".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::BackTab => {
            shift = true;
            "Tab".into()
        }
        KeyCode::Esc => "Esc".into(),
        KeyCode::Backspace => "Backspace".into(),
        KeyCode::Delete => "Delete".into(),
        KeyCode::Insert => "Insert".into(),
        KeyCode::Home => "Home".into(),
        KeyCode::End => "End".into(),
        KeyCode::PageUp => "PgUp".into(),
        KeyCode::PageDown => "PgDn".into(),
        KeyCode::Up => "Up".into(),
        KeyCode::Down => "Down".into(),
        KeyCode::Left => "Left".into(),
        KeyCode::Right => "Right".into(),
        KeyCode::F(n) => format!("F{n}"),
        _ => return None,
    };
    let mut s = String::new();
    for (on, m) in [(ctrl, "Ctrl+"), (alt, "Alt+"), (shift, "Shift+")] {
        if on {
            s.push_str(m);
        }
    }
    Some(s + &key)
}

/// 화면 표기: macOS 는 ⌃⌥⇧ 기호 (GUI 와 같게. 터미널의 Alt = Option). 저장 형식("Alt+X")은 그대로
pub fn disp(s: &str) -> String {
    if cfg!(target_os = "macos") {
        s.replace("Ctrl+", "⌃").replace("Alt+", "⌥").replace("Shift+", "⇧")
    } else {
        s.to_string()
    }
}

/// 명령 단축키로 쓸 수 있는지: Ctrl/Alt 조합이나 F 키만 (그 밖은 입력·이동 키와 겹친다)
pub fn bindable(s: &str) -> bool {
    s.starts_with("Ctrl+") || s.starts_with("Alt+") || s.trim_start_matches("Shift+").strip_prefix('F').is_some_and(|n| n.parse::<u8>().is_ok())
}

/// 사용자 설정 표기를 정규화 ("ctrl+l" → "Ctrl+L")
fn normalize(s: &str) -> String {
    let parts: Vec<&str> = s.split('+').collect();
    // "Alt++" 처럼 키가 '+' 인 경우
    let (mods, key) = if s.ends_with("++") { (&parts[..parts.len() - 2], "+") } else { (&parts[..parts.len() - 1], parts[parts.len() - 1]) };
    let has = |m: &str| mods.iter().any(|x| x.eq_ignore_ascii_case(m));
    let mut out = String::new();
    for (m, name) in [("ctrl", "Ctrl+"), ("alt", "Alt+"), ("shift", "Shift+")] {
        if has(m) {
            out.push_str(name);
        }
    }
    let named = ["Space", "Enter", "Tab", "Esc", "Backspace", "Delete", "Insert", "Home", "End", "PgUp", "PgDn", "Up", "Down", "Left", "Right"];
    // 글자·F 키는 대문자 ("l" → "L", "f5" → "F5")
    let key = named.iter().find(|n| n.eq_ignore_ascii_case(key)).map(|n| n.to_string()).unwrap_or_else(|| key.to_uppercase());
    out + &key
}

pub struct Keymap {
    map: HashMap<Action, Vec<String>>,
}

impl Keymap {
    /// 기본값 위에 사용자 설정을 덮는다. 모르는 액션 이름은 무시
    pub fn new(user: &dbtool_core::store::Keymap) -> Self {
        let mut map: HashMap<Action, Vec<String>> = DEFAULT_KEYS.iter().map(|(a, _)| (*a, defaults(*a))).collect();
        for (a, _) in DEFAULT_KEYS {
            if let Some(v) = user.get(&action_id(*a)) {
                map.insert(*a, v.iter().map(|s| normalize(s)).collect());
            }
        }
        Keymap { map }
    }

    pub fn lookup(&self, k: &KeyEvent) -> Option<Action> {
        let s = spec(k)?;
        DEFAULT_KEYS.iter().map(|d| d.0).find(|a| self.map.get(a).is_some_and(|v| v.contains(&s)))
    }

    pub fn keys(&self, a: Action) -> &[String] {
        self.map.get(&a).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// 화면 표시용 ("F5 / Ctrl+Enter")
    pub fn label(&self, a: Action) -> String {
        let k = self.keys(a);
        if k.is_empty() { "—".into() } else { disp(&k.join(" / ")) }
    }

    pub fn is_custom(&self, a: Action) -> bool {
        self.keys(a) != defaults(a).as_slice()
    }

    /// 키를 지정한다 (add = 기존 키에 더하기). 같은 키를 쓰던 다른 명령에서는 뺀다 → 뺀 명령
    pub fn bind(&mut self, a: Action, key: String, add: bool) -> Option<Action> {
        let mut taken = None;
        for (other, v) in self.map.iter_mut() {
            if *other != a && v.contains(&key) {
                v.retain(|x| x != &key);
                taken = Some(*other);
            }
        }
        let v = self.map.entry(a).or_default();
        if !add {
            v.clear();
        }
        if !v.contains(&key) {
            v.push(key);
        }
        taken
    }

    pub fn reset(&mut self, a: Action) {
        self.map.insert(a, defaults(a));
    }

    /// keymap.json 에 쓸 것: 기본값과 다른 명령만
    pub fn overrides(&self) -> dbtool_core::store::Keymap {
        DEFAULT_KEYS.iter().filter(|(a, _)| self.is_custom(*a)).map(|(a, _)| (action_id(*a), self.keys(*a).to_vec())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(code: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, m)
    }

    #[test]
    fn spec_of_events() {
        assert_eq!(spec(&ev(KeyCode::Char('l'), KeyModifiers::CONTROL)).unwrap(), "Ctrl+L");
        assert_eq!(spec(&ev(KeyCode::Char(' '), KeyModifiers::CONTROL)).unwrap(), "Ctrl+Space");
        assert_eq!(spec(&ev(KeyCode::Char('\\'), KeyModifiers::ALT)).unwrap(), "Alt+\\");
        assert_eq!(spec(&ev(KeyCode::Char('₩'), KeyModifiers::ALT)).unwrap(), "Alt+\\");
        assert_eq!(spec(&ev(KeyCode::Char('D'), KeyModifiers::SHIFT)).unwrap(), "Shift+D");
        assert_eq!(spec(&ev(KeyCode::Char('?'), KeyModifiers::SHIFT)).unwrap(), "?");
        assert_eq!(spec(&ev(KeyCode::BackTab, KeyModifiers::SHIFT)).unwrap(), "Shift+Tab");
        assert_eq!(spec(&ev(KeyCode::PageDown, KeyModifiers::CONTROL)).unwrap(), "Ctrl+PgDn");
    }

    #[test]
    fn defaults_and_overrides() {
        let mut km = Keymap::new(&Default::default());
        assert_eq!(km.lookup(&ev(KeyCode::Char('l'), KeyModifiers::CONTROL)), Some(Action::NewConsole));
        assert_eq!(km.lookup(&ev(KeyCode::Enter, KeyModifiers::ALT)), Some(Action::Execute));
        assert!(km.overrides().is_empty());

        // 다른 명령이 쓰던 키를 가져오면 그 명령에서 빠진다
        assert_eq!(km.bind(Action::NewConsole, "Ctrl+K".into(), false), Some(Action::Palette));
        assert_eq!(km.lookup(&ev(KeyCode::Char('k'), KeyModifiers::CONTROL)), Some(Action::NewConsole));
        assert_eq!(km.lookup(&ev(KeyCode::Char('l'), KeyModifiers::CONTROL)), None);
        assert_eq!(km.label(Action::Palette), "—");
        let o = km.overrides();
        assert_eq!(o.get("NewConsole").unwrap(), &vec!["Ctrl+K".to_string()]);
        assert_eq!(o.get("Palette").unwrap(), &Vec::<String>::new());

        // 저장한 설정을 다시 읽으면 같다 (표기는 정규화)
        let mut user = o.clone();
        user.insert("Help".into(), vec!["shift+f1".into()]);
        let km2 = Keymap::new(&user);
        assert_eq!(km2.lookup(&ev(KeyCode::Char('k'), KeyModifiers::CONTROL)), Some(Action::NewConsole));
        assert_eq!(km2.keys(Action::Help), ["Shift+F1"]);

        km.reset(Action::Palette);
        km.reset(Action::NewConsole);
        assert!(km.overrides().is_empty());
    }

    #[test]
    fn only_modified_or_function_keys_are_bindable() {
        assert!(bindable("Ctrl+L") && bindable("Alt+\\") && bindable("F5") && bindable("Shift+F1"));
        assert!(!bindable("L") && !bindable("Enter") && !bindable("Shift+Tab") && !bindable("Up"));
    }

    #[test]
    fn every_default_is_bindable_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for (a, keys) in DEFAULT_KEYS {
            for k in *keys {
                assert!(bindable(k), "{a:?} {k}");
                assert!(seen.insert(*k), "duplicate default {k}");
                assert_eq!(normalize(k), *k);
            }
        }
    }
}
