//! 에디터 화면 분할 트리 (GUI 의 Split Right / Split Down). 칸(그룹)은 잎, 나눈 곳은 가지. 칸은 최대 MAX 개
use crate::app::SplitDir;

pub const MAX: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub enum Pane {
    /// 칸 id (Tab.group)
    Leaf(usize),
    /// pct = 첫 번째(a: 왼쪽/위) 가 차지하는 비율(%)
    Split { dir: SplitDir, pct: u16, a: Box<Pane>, b: Box<Pane> },
}

/// 가지 위치: 뿌리에서 a(false)/b(true) 로 내려가는 길
pub type Path = Vec<bool>;

impl Pane {
    /// 칸 id 들 (왼쪽/위부터)
    pub fn leaves(&self) -> Vec<usize> {
        match self {
            Pane::Leaf(g) => vec![*g],
            Pane::Split { a, b, .. } => [a.leaves(), b.leaves()].concat(),
        }
    }

    pub fn is_split(&self) -> bool {
        matches!(self, Pane::Split { .. })
    }

    /// 칸 g 를 dir 방향으로 나눠 새 칸 id 를 돌려준다 (이미 MAX 개면 None)
    pub fn split(&mut self, g: usize, dir: SplitDir) -> Option<usize> {
        let ls = self.leaves();
        if ls.len() >= MAX || !ls.contains(&g) {
            return None;
        }
        let new = (0..MAX).find(|i| !ls.contains(i))?;
        self.replace_leaf(g, &mut |_| Pane::Split { dir, pct: 50, a: Box::new(Pane::Leaf(g)), b: Box::new(Pane::Leaf(new)) });
        Some(new)
    }

    fn replace_leaf(&mut self, g: usize, f: &mut dyn FnMut(usize) -> Pane) -> bool {
        match self {
            Pane::Leaf(x) if *x == g => {
                *self = f(g);
                true
            }
            Pane::Leaf(_) => false,
            Pane::Split { a, b, .. } => a.replace_leaf(g, f) || b.replace_leaf(g, f),
        }
    }

    /// 칸 g 를 없애고 옆 칸이 그 자리를 채운다 (마지막 칸은 없애지 않는다)
    pub fn remove(&mut self, g: usize) {
        if let Pane::Split { a, b, .. } = self {
            if **a == Pane::Leaf(g) {
                *self = (**b).clone();
            } else if **b == Pane::Leaf(g) {
                *self = (**a).clone();
            } else {
                a.remove(g);
                b.remove(g);
            }
        }
    }

    /// 칸 g 를 감싼 가장 가까운 가지
    pub fn parent_of(&self, g: usize) -> Option<Path> {
        fn walk(p: &Pane, g: usize, path: &mut Path) -> Option<Path> {
            let Pane::Split { a, b, .. } = p else { return None };
            if **a == Pane::Leaf(g) || **b == Pane::Leaf(g) {
                return Some(path.clone());
            }
            for (side, c) in [(false, a), (true, b)] {
                path.push(side);
                if let Some(r) = walk(c, g, path) {
                    return Some(r);
                }
                path.pop();
            }
            None
        }
        walk(self, g, &mut vec![])
    }

    /// 가지의 비율 (path 가 가지가 아니면 None)
    pub fn pct_mut(&mut self, path: &[bool]) -> Option<&mut u16> {
        match (self, path.split_first()) {
            (Pane::Split { pct, .. }, None) => Some(pct),
            (Pane::Split { a, b, .. }, Some((side, rest))) => if *side { b } else { a }.pct_mut(rest),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_up_to_four_and_collapse() {
        let mut p = Pane::Leaf(0);
        assert_eq!(p.split(0, SplitDir::Right), Some(1));
        assert_eq!(p.split(0, SplitDir::Down), Some(2));
        assert_eq!(p.split(1, SplitDir::Down), Some(3));
        // 2x2: 왼쪽 위·왼쪽 아래·오른쪽 위·오른쪽 아래
        assert_eq!(p.leaves(), [0, 2, 1, 3]);
        assert_eq!(p.split(3, SplitDir::Right), None);
        assert_eq!(p.parent_of(3), Some(vec![true]));
        *p.pct_mut(&[true]).unwrap() = 30;
        p.remove(2);
        assert_eq!(p.leaves(), [0, 1, 3]);
        p.remove(1);
        p.remove(3);
        assert_eq!(p, Pane::Leaf(0));
        // 지운 id 를 다시 쓴다
        assert_eq!(p.split(0, SplitDir::Down), Some(1));
    }
}
