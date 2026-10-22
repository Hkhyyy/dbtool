import type { Segment } from "./sqlSplit";

/** DataGrip Settings › Database › Query Execution */
export interface ExecSettings {
  inside: "ask" | "smallest" | "whole" | "fromCaret";
  outside: "nothing" | "whole" | "below";
  selection: "single" | "separate";
}
/** DataGrip 처럼 커서가 문장 안이면 무엇을 실행할지 묻는다 */
export const DEFAULT_EXEC: ExecSettings = { inside: "ask", outside: "nothing", selection: "separate" };

export interface Stmt {
  start: number;
  end: number;
  sql: string;
}
export type ExecPlan = { run: Stmt[] } | { ask: { label: string; stmts: Stmt[] }[] } | null;

/** 선택 영역이 없을 때 커서 위치로 실행할 문장을 정한다 */
export function planExecution(text: string, offset: number, segs: Segment[], s: ExecSettings): ExecPlan {
  const all: Stmt[] = segs.map((g) => {
    const raw = text.slice(g.start, g.end);
    const lead = raw.length - raw.trimStart().length;
    return { start: g.start + lead, end: g.start + lead + raw.trim().length, sql: raw.trim() };
  });
  if (!all.length) return null;
  const idx = all.findIndex((x) => offset >= x.start && offset <= x.end + 1);
  if (idx < 0) {
    if (s.outside === "whole") return { run: all };
    if (s.outside === "below") return { run: all.filter((x) => x.start >= offset) };
    // Nothing: 바로 앞 문장이 같은 줄 끝이면 그 문장 (세미콜론 직후 캐럿)
    const prev = [...all].reverse().find((x) => x.end <= offset && !text.slice(x.end, offset).includes("\n"));
    return prev ? { run: [prev] } : null;
  }
  if (s.inside === "smallest") return { run: [all[idx]] };
  if (s.inside === "whole") return { run: all };
  if (s.inside === "fromCaret") return { run: all.slice(idx) };
  const ask = [
    { label: all[idx].sql.replace(/\s+/g, " ").slice(0, 80), stmts: [all[idx]] },
    ...(all.length > 1 ? [{ label: `All statements (${all.length})`, stmts: all }] : []),
    ...(idx > 0 && idx < all.length - 1 ? [{ label: "From caret to end", stmts: all.slice(idx) }] : []),
  ];
  return ask.length === 1 ? { run: [all[idx]] } : { ask };
}
