import { useMemo, useState } from "react";

/** DataGrip "Browse Query History" (⌥⌘E): 데이터 소스별 실행 기록. 타이핑 = 필터, 더블클릭/↩ = 콘솔에 붙여넣기, ⌦ = 삭제 */
export interface HistoryItem {
  id: string;
  connId: string;
  database?: string;
  sql: string;
  ts: number;
}
const KEY = "dbtool.queryHistory";
const MAX_ITEMS = 2000;
/** 한 문장은 64KB 까지만 기록, 전체는 약 3MB (localStorage 한도 5MB 안쪽) */
const MAX_SQL = 64 * 1024;
const MAX_BYTES = 3 * 1024 * 1024;

// 메모리에 들고 있다가 모아서 쓴다 (스크립트를 실행하면 문장마다 불려서, 매번 전체를 파싱·직렬화하지 않게)
let mem: HistoryItem[] | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;
const load = (): HistoryItem[] => {
  if (!mem) {
    try {
      mem = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    } catch {
      mem = [];
    }
  }
  return mem!;
};
const flush = () => {
  timer = undefined;
  if (!mem) return;
  let items = mem.slice(-MAX_ITEMS);
  let json = JSON.stringify(items);
  // 너무 크면 오래된 것부터 버린다
  while (json.length > MAX_BYTES && items.length > 1) {
    items = items.slice(Math.ceil(items.length / 10));
    json = JSON.stringify(items);
  }
  mem = items;
  try {
    localStorage.setItem(KEY, json);
  } catch {
    /* 저장 실패 무시 */
  }
};
const store = (items: HistoryItem[]) => {
  mem = items;
  if (!timer) timer = setTimeout(flush, 1000);
};
window.addEventListener("beforeunload", () => timer && flush());

export function pushHistory(h: Omit<HistoryItem, "id" | "ts">) {
  const items = load();
  const sql = h.sql.length > MAX_SQL ? h.sql.slice(0, MAX_SQL) : h.sql;
  const last = items[items.length - 1];
  if (last && last.connId === h.connId && last.sql === sql) {
    last.ts = Date.now();
  } else items.push({ ...h, sql, id: crypto.randomUUID(), ts: Date.now() });
  store(items);
}

export function HistoryDialog({ connId, title, onPick, onClose }: { connId: string; title: string; onPick: (sql: string) => void; onClose: () => void }) {
  const [all, setAll] = useState(() => [...load()]);
  const [q, setQ] = useState("");
  const [idx, setIdx] = useState(0);
  const list = useMemo(
    () =>
      all
        .filter((h) => h.connId === connId && (!q || h.sql.toLowerCase().includes(q.toLowerCase())))
        .sort((a, b) => b.ts - a.ts),
    [all, q, connId],
  );
  const cur = list[Math.min(idx, list.length - 1)];
  const del = (h: HistoryItem) => {
    const next = all.filter((x) => x.id !== h.id);
    setAll(next);
    store(next);
  };
  const fmt = (ts: number) => new Date(ts).toLocaleString();

  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div
        className="modal history"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") setIdx((i) => Math.min(i + 1, list.length - 1));
          else if (e.key === "ArrowUp") setIdx((i) => Math.max(i - 1, 0));
          else if (e.key === "Enter" && cur) onPick(cur.sql);
          else if (e.key === "Escape") onClose();
          else if ((e.key === "Delete" || (e.metaKey && e.key === "Backspace")) && cur) del(cur);
          else return;
          e.preventDefault();
        }}
      >
        <div className="modal-title">{title}</div>
        <input autoFocus className="hist-filter" placeholder="Type to filter" value={q} onChange={(e) => { setQ(e.target.value); setIdx(0); }} />
        <div className="hist-body">
          <div className="hist-list">
            {list.length === 0 && <div className="muted pad">No queries yet.</div>}
            {list.map((h, i) => (
              <div key={h.id} className={"hist-item" + (h === cur ? " on" : "")} onClick={() => setIdx(i)} onDoubleClick={() => onPick(h.sql)}>
                <span className="hist-sql">{h.sql.replace(/\s+/g, " ").slice(0, 120)}</span>
                <span className="hist-ts">{fmt(h.ts)}</span>
              </div>
            ))}
          </div>
          <pre className="hist-preview">{cur?.sql ?? ""}</pre>
        </div>
        <div className="modal-actions">
          <span className="muted">Double-click or ↩ to paste into the console · ⌦ to delete</span>
          <span className="grow" />
          <button onClick={onClose}>Close</button>
          <button className="primary" disabled={!cur} onClick={() => cur && onPick(cur.sql)}>Paste to Console</button>
        </div>
      </div>
    </div>
  );
}
