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

/** 문장을 시작할 수 있는 키워드 (이 줄 앞의 SQL 이 아닌 줄은 제목·메모로 보고 뺀다) */
const SQL_START = /^\s*(\(|select|with|insert|update|delete|merge|upsert|begin|start|commit|rollback|end|abort|savepoint|release|grant|revoke|create|alter|drop|truncate|comment|set|reset|show|explain|analyze|vacuum|values|table|copy|call|do|lock|declare|fetch|close|prepare|execute|deallocate|listen|notify|unlisten|refresh|reindex|cluster|discard|checkpoint|use|describe|desc|replace|load|handler|optimize|kill|flush|rename|import)\b/i;
const NOTE_KEEP = /^\s*(--|\/\*|#|$)/;

/** 앞의 SQL 이 아닌 줄(번호 제목 등)을 뺀 시작 위치. 키워드로 시작하는 줄이 없으면 그대로 */
function skipNotes(text: string, start: number, end: number): number {
  let pos = start;
  let cut = start;
  let dropped = false;
  while (pos < end) {
    const nl = text.indexOf("\n", pos);
    const lineEnd = nl < 0 || nl > end ? end : nl;
    const line = text.slice(pos, lineEnd);
    if (SQL_START.test(line)) return dropped ? cut : start;
    if (!NOTE_KEEP.test(line)) {
      dropped = true;
      cut = lineEnd + 1;
    }
    pos = lineEnd + 1;
  }
  return start;
}

const TX_BEGIN = /^(begin|start\s+transaction)\b/i;
const TX_END = /^(commit|rollback|end|abort)\b/i;

/** 선택 영역이 없을 때 커서 위치로 실행할 문장을 정한다 */
export function planExecution(text: string, offset: number, segs: Segment[], s: ExecSettings): ExecPlan {
  // froms: 커서 판정용 시작 (제목 줄 포함). all: 실행·강조할 범위 (제목 줄 뺌)
  const froms: number[] = [];
  const all: Stmt[] = segs.map((g) => {
    const raw = text.slice(g.start, g.end);
    const from = g.start + raw.length - raw.trimStart().length;
    const end = g.start + raw.trimEnd().length;
    const at = skipNotes(text, from, end);
    const start = at + text.slice(at, end).length - text.slice(at, end).trimStart().length;
    froms.push(from);
    return { start, end, sql: text.slice(start, end) };
  });
  if (!all.length) return null;
  const idx = all.findIndex((x, i) => offset >= froms[i] && offset <= x.end + 1);
  if (idx < 0) {
    if (s.outside === "whole") return { run: all };
    if (s.outside === "below") return { run: all.filter((_, i) => froms[i] >= offset) };
    // Nothing: 바로 앞 문장이 같은 줄 끝이면 그 문장 (세미콜론 직후 캐럿)
    const prev = [...all].reverse().find((x) => x.end <= offset && !text.slice(x.end, offset).includes("\n"));
    return prev ? { run: [prev] } : null;
  }
  if (s.inside === "smallest") return { run: [all[idx]] };
  if (s.inside === "whole") return { run: all };
  if (s.inside === "fromCaret") return { run: all.slice(idx) };
  const tx = txAround(all, idx);
  const word = (x: Stmt) => x.sql.split(/\s+/)[0].toUpperCase();
  const ask = [
    { label: all[idx].sql.replace(/\s+/g, " ").slice(0, 80), stmts: [all[idx]] },
    ...(tx && tx.length < all.length ? [{ label: `Transaction (${word(tx[0])} … ${word(tx[tx.length - 1])}, ${tx.length} statements)`, stmts: tx }] : []),
    ...(all.length > 1 ? [{ label: `All statements (${all.length})`, stmts: all }] : []),
    ...(idx > 0 && idx < all.length - 1 ? [{ label: "From caret to end", stmts: all.slice(idx) }] : []),
  ];
  return ask.length === 1 ? { run: [all[idx]] } : { ask };
}

/** 커서 문장(idx)을 감싼 가장 가까운 BEGIN … COMMIT/ROLLBACK */
function txAround(all: Stmt[], idx: number): Stmt[] | null {
  let b = idx;
  while (b >= 0 && !TX_BEGIN.test(all[b].sql)) {
    if (b < idx && TX_END.test(all[b].sql)) return null;
    b--;
  }
  if (b < 0) return null;
  let e = Math.max(idx, b + 1);
  if (idx > b && TX_END.test(all[idx].sql)) e = idx;
  while (e < all.length && !TX_END.test(all[e].sql)) e++;
  return e < all.length ? all.slice(b, e + 1) : null;
}
