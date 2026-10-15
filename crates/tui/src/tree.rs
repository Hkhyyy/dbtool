//! Database Explorer 트리 (지연 로딩). 키: "conn" / "conn|db" / "conn|db|schema" / "conn|db|schema|g:tables" / ".../o:name"
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Conn,
    Db,
    Schema,
    Group,
    /// table | view | matview | foreign | partitioned | sequence | function | procedure | key
    Object(String),
    Column { ty: String, pk: bool },
    Info,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub key: String,
    pub label: String,
    /// 오른쪽에 흐리게 (타입, 개수 등)
    pub hint: String,
    pub kind: NodeKind,
    pub conn: String,
    pub db: Option<String>,
    pub schema: Option<String>,
    pub name: Option<String>,
    pub expandable: bool,
}

impl Node {
    pub fn info(key: &str, conn: &str, text: impl Into<String>) -> Self {
        Node { key: format!("{key}|info"), label: text.into(), hint: String::new(), kind: NodeKind::Info, conn: conn.into(), db: None, schema: None, name: None, expandable: false }
    }
    pub fn is_relation(&self) -> bool {
        matches!(&self.kind, NodeKind::Object(k) if ["table", "view", "matview", "foreign", "partitioned"].contains(&k.as_str()))
    }
}

#[derive(Default)]
pub struct Tree {
    pub roots: Vec<Node>,
    pub children: HashMap<String, Vec<Node>>,
    pub expanded: HashSet<String>,
    pub loading: HashSet<String>,
    pub sel: usize,
    pub offset: usize,
    /// 스피드 서치 (/)
    pub filter: String,
}

impl Tree {
    /// 화면에 보이는 줄: (깊이, 노드)
    pub fn visible(&self) -> Vec<(usize, &Node)> {
        let mut out = Vec::new();
        let f = self.filter.to_lowercase();
        fn walk<'a>(t: &'a Tree, nodes: &'a [Node], depth: usize, f: &str, out: &mut Vec<(usize, &'a Node)>) {
            for n in nodes {
                let open = t.expanded.contains(&n.key);
                let kids = if open { t.children.get(&n.key) } else { None };
                // 필터: 이름이 맞거나 펼쳐진 자식 중에 맞는 게 있으면 보인다
                if !f.is_empty() && !matches!(n.kind, NodeKind::Conn | NodeKind::Db | NodeKind::Schema | NodeKind::Group) && !n.label.to_lowercase().contains(f) {
                    continue;
                }
                out.push((depth, n));
                if let Some(k) = kids {
                    walk(t, k, depth + 1, f, out);
                } else if open && t.loading.contains(&n.key) {
                    // 로딩 표시는 렌더러가 hint 로 보여 준다
                }
            }
        }
        walk(self, &self.roots, 0, &f, &mut out);
        out
    }

    pub fn selected(&self) -> Option<Node> {
        self.visible().get(self.sel).map(|(_, n)| (*n).clone())
    }

    pub fn select_key(&mut self, key: &str) -> bool {
        if let Some(i) = self.visible().iter().position(|(_, n)| n.key == key) {
            self.sel = i;
            true
        } else {
            false
        }
    }

    pub fn clamp(&mut self) {
        let n = self.visible().len();
        if n == 0 {
            self.sel = 0;
        } else if self.sel >= n {
            self.sel = n - 1;
        }
    }

    /// 접속 끊김·새로 고침: 그 아래 캐시를 지운다
    pub fn forget(&mut self, prefix: &str) {
        self.children.retain(|k, _| !(k == prefix || k.starts_with(&format!("{prefix}|"))));
        self.loading.retain(|k| !(k == prefix || k.starts_with(&format!("{prefix}|"))));
    }

    pub fn parent_index(&self) -> Option<usize> {
        let vis = self.visible();
        let (depth, _) = *vis.get(self.sel)?;
        if depth == 0 {
            return None;
        }
        (0..self.sel).rev().find(|&i| vis[i].0 == depth - 1)
    }
}
