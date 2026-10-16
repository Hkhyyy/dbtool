/** 에디터 화면 분할 트리 (TUI 의 panes.rs 와 같은 규칙). 칸(그룹)은 잎, 나눈 곳은 가지. 칸은 최대 MAX_PANES 개 */
export type Pane = { t: "leaf"; g: number } | { t: "split"; dir: "right" | "down"; a: Pane; b: Pane };

export const MAX_PANES = 4;
export const leaf = (g: number): Pane => ({ t: "leaf", g });

/** 칸 id 들 (왼쪽/위부터) */
export const leaves = (p: Pane): number[] => (p.t === "leaf" ? [p.g] : [...leaves(p.a), ...leaves(p.b)]);

/** 칸 g 를 dir 방향으로 나눈다. 새 칸 id 와 새 트리 (이미 MAX_PANES 개면 null) */
export function splitPane(p: Pane, g: number, dir: "right" | "down"): { pane: Pane; g: number } | null {
  const ls = leaves(p);
  if (ls.length >= MAX_PANES || !ls.includes(g)) return null;
  const n = [0, 1, 2, 3].find((i) => !ls.includes(i))!;
  const walk = (x: Pane): Pane => (x.t === "leaf" ? (x.g === g ? { t: "split", dir, a: leaf(g), b: leaf(n) } : x) : { ...x, a: walk(x.a), b: walk(x.b) });
  return { pane: walk(p), g: n };
}

/** 탭이 없는 칸을 없애고 옆 칸이 그 자리를 채운다 (전부 비면 칸 하나) */
export function prunePanes(p: Pane, used: Set<number>): Pane {
  const walk = (x: Pane): Pane | null => {
    if (x.t === "leaf") return used.has(x.g) ? x : null;
    const a = walk(x.a);
    const b = walk(x.b);
    return a && b ? { ...x, a, b } : a ?? b;
  };
  return walk(p) ?? leaf(leaves(p)[0]);
}
