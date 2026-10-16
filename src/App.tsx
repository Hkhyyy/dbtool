import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { api, ConnectionView, dialectOf, meta, QueryResult, redisDb, targetId } from "./lib/api";
import { RedisBrowser } from "./components/RedisBrowser";
import { KeymapDialog } from "./components/KeymapDialog";
import { KeymapEditor } from "./components/KeymapEditor";
import { ContextMenu, Explorer, MenuItem } from "./components/Explorer";
import { ConnectionDialog } from "./components/ConnectionDialog";
import { ConsoleEditor, EditorApi } from "./components/ConsoleEditor";
import { ConsolePane, OutputLine, parsePgPlan, ResultEntry } from "./components/ConsolePane";
import { HistoryDialog, pushHistory } from "./components/HistoryDialog";
import { EditTarget } from "./components/ResultGrid";
import { TableEditor } from "./components/TableEditor";
import { RecentFiles, Switcher, SwitchItem } from "./components/Switcher";
import { ActionItem, DbObject, invalidateSearchIndex, SearchEverywhere, SeTab } from "./components/SearchEverywhere";
import { FindPanel, FindRun, FtsScope, FullTextSearchDialog } from "./components/FullTextSearch";
import { DEFAULT_FONT, editorFontSize, setEditorFontSize } from "./lib/prefs";
import { bindings, eventSpec, formatSpec, hit, loadKeymap, onKeymapChange } from "./lib/shortcuts";
import { invalidateCompletion } from "./lib/completion";
import { useStatusWidgets } from "./lib/statusbar";
import type { FtsRequest } from "./lib/api";
import { isRelation, NodeInfo, revealInTree } from "./components/SchemaTree";
import { connBg, getThemePref, setThemePref, THEME_LABEL, ThemePref, useTheme } from "./lib/theme";
import "./App.css";

type Group = 0 | 1;

interface Tab {
  id: string;
  /** console: SQL 콘솔 / table: 테이블 데이터 에디터 / ddl: 읽기 전용 DDL */
  kind: "console" | "table" | "ddl" | "redis";
  group: Group;
  connId: string;
  /** 접속한 DB (없으면 Data Source 기본 DB) */
  database?: string;
  /** 백엔드 접속 대상 id (connId 또는 connId::database) */
  target: string;
  title: string;
  sql: string;
  running: boolean;
  result: QueryResult | null;
  error: string | null;
  lastSql: string | null;
  loadingMore: boolean;
  where: string;
  orderBy: string;
  /** 콘솔의 현재 스키마 (search_path) */
  schema?: string;
  tableSql?: string;
  edit?: EditTarget;
  /** 콘솔: 결과 탭들, Output 로그, 트랜잭션 */
  results: ResultEntry[];
  activeResult: string;
  output: OutputLine[];
  txManual: boolean;
  inTx: boolean;
  /** 서버 커서가 열린 결과 탭 (더 불러오기 대상) */
  cursorEntry?: string;
  /** Pin Tab */
  pinned?: boolean;
  /** Redis 브라우저: 처음 선택할 키 */
  redisKey?: string;
}

/** 닫은 탭 / 최근 파일 (Reopen Closed Tab, ⌘E Recent Files) */
interface TabSnapshot {
  key: string;
  kind: Tab["kind"];
  title: string;
  connId: string;
  database?: string;
  sql: string;
  edit?: EditTarget;
  where: string;
  orderBy: string;
}
const TAB_LIMIT = 10; // DataGrip 기본 Tab limit
const snapKey = (t: { kind: string; connId: string; database?: string; title: string; edit?: EditTarget }) =>
  `${t.kind}|${t.connId}|${t.database ?? ""}|${t.edit ? t.edit.schema + "." + t.edit.table : t.title}`;

const isMac = navigator.platform.toLowerCase().includes("mac");
/** 결과 그리드에 쌓아 두는 최대 행 수 (TUI 와 같다). 그 이상은 CSV 로 내보낸다 */
const MAX_ROWS = 100_000;
const capMsg = (n: number) => `Showing the first ${n.toLocaleString()} rows — export to CSV for all of them`;
/** 메뉴·Find Action 에 보일 단축키 (첫 번째 키) */
const sk = (id: string) => {
  const b = bindings(id);
  return b.length ? formatSpec(b[0]) : undefined;
};
const last = <T,>(a: T[]): T | undefined => a[a.length - 1];

export default function App() {
  const theme = useTheme();
  // 키맵이 바뀌면 메뉴의 단축키 표시를 다시 그린다
  const [, setKeymapVer] = useState(0);
  useEffect(() => {
    const off = onKeymapChange(() => setKeymapVer((n) => n + 1));
    void loadKeymap();
    return () => void off();
  }, []);
  const [settingsOpen, setSettingsOpen] = useState<false | "appearance" | "keymap">(false);
  const [schemeOpen, setSchemeOpen] = useState(false);
  const [keymapOpen, setKeymapOpen] = useState(false);
  const [conns, setConns] = useState<ConnectionView[]>([]);
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activeIds, setActiveIds] = useState<[string | null, string | null]>([null, null]);
  const activeIdsRef = useRef(activeIds);
  activeIdsRef.current = activeIds;
  const [split, setSplit] = useState<null | "right" | "down">(null);
  const [focused, setFocused] = useState<Group>(0);
  const [dialog, setDialog] = useState<{ edit?: ConnectionView } | null>(null);
  const [status, setStatus] = useState("Ready");
  const [treeSel, setTreeSel] = useState<NodeInfo | null>(null);
  const [tabMenu, setTabMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const editorApis = useRef(new Map<string, EditorApi>());
  const [historyFor, setHistoryFor] = useState<string | null>(null);
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;

  // 쿼리를 실행할 때마다 불리므로, 실제로 바뀐 게 있을 때만 상태를 바꾼다 (트리 전체 다시 그리기·단축키 핸들러 재등록 방지)
  const connsJson = useRef("");
  const refresh = useCallback(async () => {
    try {
      const cs = await api.listConnections();
      const j = JSON.stringify(cs);
      if (j !== connsJson.current) {
        connsJson.current = j;
        setConns(cs);
      }
    } catch (e) {
      setStatus(String(e));
    }
  }, []);
  useEffect(() => {
    refresh();
  }, [refresh]);
  // Help 메뉴 › Keymap Reference
  useEffect(() => {
    let off: (() => void) | undefined;
    listen("open-keymap", () => setKeymapOpen(true))
      .then((u) => (off = u))
      .catch(() => {});
    return () => off?.();
  }, []);
  // Auto-disconnect 등으로 백엔드가 접속을 닫으면 트리 상태를 갱신
  useEffect(() => {
    let off: (() => void) | undefined;
    listen<string>("conn-disconnected", (e) => {
      invalidateSearchIndex(e.payload);
      invalidateCompletion(e.payload);
      refresh();
      setStatus(`Disconnected (idle): ${e.payload}`);
    })
      .then((u) => (off = u))
      .catch(() => {});
    return () => off?.();
  }, [refresh]);
  useEffect(() => {
    const onErr = (e: ErrorEvent) => setStatus(`Error: ${e.message}`);
    const onRej = (e: PromiseRejectionEvent) => setStatus(`Error: ${String(e.reason)}`);
    const closeMenu = () => setTabMenu(null);
    window.addEventListener("error", onErr);
    window.addEventListener("unhandledrejection", onRej);
    window.addEventListener("mousedown", closeMenu);
    return () => {
      window.removeEventListener("error", onErr);
      window.removeEventListener("unhandledrejection", onRej);
      window.removeEventListener("mousedown", closeMenu);
    };
  }, []);

  const groupActive = (g: Group) => tabs.find((t) => t.id === activeIds[g] && t.group === g) ?? null;
  const active = groupActive(focused) ?? groupActive(focused === 0 ? 1 : 0);
  const activeConn = conns.find((c) => c.id === active?.connId);
  const patch = (id: string, p: Partial<Tab>) => setTabs((ts) => ts.map((t) => (t.id === id ? { ...t, ...p } : t)));
  const mru = useRef<string[]>([]);
  const recent = useRef<TabSnapshot[]>([]);
  const closedStack = useRef<TabSnapshot[]>([]);
  // 콘솔에서 치는 SQL 은 상태(setTabs)에 넣지 않고 여기 둔다 → 키 입력마다 앱 전체를 다시 그리지 않는다
  const liveSql = useRef(new Map<string, string>());
  const sqlOf = (t: Tab) => liveSql.current.get(t.id) ?? t.sql;
  const snapshot = (t: Tab): TabSnapshot => ({
    key: snapKey(t),
    kind: t.kind,
    title: t.title,
    connId: t.connId,
    database: t.database,
    sql: sqlOf(t),
    edit: t.edit,
    where: t.where,
    orderBy: t.orderBy,
  });
  const remember = (t: Tab) => {
    const snap = snapshot(t);
    recent.current = [snap, ...recent.current.filter((r) => r.key !== snap.key)].slice(0, 50);
  };
  const activate = (t: Tab) => {
    setFocused(t.group);
    setActiveIds((a) => (t.group === 0 ? [t.id, a[1]] : [a[0], t.id]));
    mru.current = [t.id, ...mru.current.filter((x) => x !== t.id)];
    remember(t);
  };

  // ───────── 탭 열기 ─────────
  const addTab = (
    c: ConnectionView,
    kind: Tab["kind"],
    opts: { title?: string; sql?: string; database?: string; tableSql?: string; edit?: EditTarget; group?: Group; where?: string; orderBy?: string },
  ) => {
    const database = opts.database && opts.database !== (c.database || "postgres") ? opts.database : undefined;
    const base = database ? `${c.name}/${database}` : c.name;
    const n = tabsRef.current.filter((t) => t.kind === "console" && t.connId === c.id && t.database === database).length;
    const group: Group = opts.group ?? (split ? focused : 0);
    const tab: Tab = {
      id: crypto.randomUUID(),
      kind,
      group,
      connId: c.id,
      database,
      target: targetId(c.id, database),
      title: opts.title ?? (n === 0 ? `console [${base}]` : `console_${n + 1} [${base}]`),
      sql: opts.sql ?? "",
      running: false,
      result: null,
      error: null,
      lastSql: null,
      loadingMore: false,
      where: opts.where ?? "",
      orderBy: opts.orderBy ?? "",
      tableSql: opts.tableSql,
      edit: opts.edit,
      results: [],
      activeResult: "output",
      output: [],
      txManual: kind === "console" && !!c.options?.txManual,
      inTx: false,
    };
    // Transaction control: Manual 이 기본이면 세션도 Manual 로 연다
    if (tab.txManual) api.setTxMode(tab.target, tab.id, true).catch((e) => setStatus(String(e)));
    setTabs((ts) => [...ts, tab]);
    activate(tab);
    // Tab limit: 고정되지 않은 탭이 10개를 넘으면 가장 오래 안 쓴 탭부터 닫는다 (DataGrip)
    setTimeout(() => {
      const inGroup = tabsRef.current.filter((t) => t.group === group && !t.pinned && t.id !== tab.id);
      const over = inGroup.length + 1 - TAB_LIMIT;
      if (over > 0) {
        const byAge = [...inGroup].sort((a, b) => {
          const ia = mru.current.indexOf(a.id), ib = mru.current.indexOf(b.id);
          return (ib < 0 ? -1 : ib) - (ia < 0 ? -1 : ia);
        });
        closeTabs(byAge.slice(0, over).map((t) => t.id));
      }
    });
    return tab;
  };

  const openConsole = (c: ConnectionView, db?: string, sql?: string) => addTab(c, "console", { database: db, sql });

  const openTable = async (c: ConnectionView, schema: string, table: string, db?: string) => {
    const target = targetId(c.id, db && db !== (c.database || "postgres") ? db : undefined);
    const existing = tabsRef.current.find((t) => t.kind === "table" && t.target === target && t.edit?.schema === schema && t.edit?.table === table);
    if (existing) return activate(existing);
    return openTableTab(c, schema, table, db);
  };

  /** 테이블 데이터 탭을 새로 연다 (split 복제 시에는 필터·정렬이 독립적인 별도 탭) */
  const openTableTab = async (c: ConnectionView, schema: string, table: string, db?: string, group?: Group, where = "", orderBy = "") => {
    const target = targetId(c.id, db && db !== (c.database || "postgres") ? db : undefined);
    const { q } = dialectOf(c.kind);
    let sql = `SELECT * FROM ${q(schema)}.${q(table)}`;
    if (where) sql += ` WHERE ${where}`;
    if (orderBy) sql += ` ORDER BY ${orderBy}`;
    sql += ";";
    let edit: EditTarget = { schema, table, pk: [] };
    try {
      const d = await meta.table(target, schema, table);
      edit = { schema, table, pk: d.columns.filter((x) => x.isPk).map((x) => x.name) };
    } catch {
      /* 메타 조회 실패 시 읽기 전용 */
    }
    const tab = addTab(c, "table", { title: table, sql, database: db, tableSql: sql, edit, group, where, orderBy });
    run(tab, sql);
  };

  const openDdl = async (info: NodeInfo) => {
    if (!info.schema || !info.name) return;
    const target = targetId(info.conn.id, info.db && info.db !== (info.conn.database || "postgres") ? info.db : undefined);
    const title = `${info.name} [DDL]`;
    const existing = tabsRef.current.find((t) => t.kind === "ddl" && t.target === target && t.title === title);
    if (existing) return activate(existing);
    try {
      setStatus(`Loading DDL of ${info.schema}.${info.name}…`);
      const ddl = await meta.ddl(target, info.schema, info.name, info.kind);
      addTab(info.conn, "ddl", { title, sql: ddl, database: info.db });
      setStatus("Ready");
    } catch (e) {
      setStatus(`DDL failed: ${String(e)}`);
    }
  };

  /** SQL Generator: 선택한 테이블로 문장을 만들어 새 콘솔에 넣는다 */
  const generate = async (info: NodeInfo, what: "select" | "insert" | "update" | "delete" | "ddl") => {
    if (!info.schema || !info.name) return;
    const target = targetId(info.conn.id, info.db && info.db !== (info.conn.database || "postgres") ? info.db : undefined);
    const quoteIdent = dialectOf(info.conn.kind).q;
    const full = `${quoteIdent(info.schema)}.${quoteIdent(info.name)}`;
    try {
      let sql: string;
      if (what === "ddl") sql = await meta.ddl(target, info.schema, info.name, info.kind);
      else {
        const d = await meta.table(target, info.schema, info.name);
        const cols = d.columns;
        const pk = cols.filter((c) => c.isPk);
        const keys = pk.length ? pk : cols.slice(0, 1);
        const ph = (c: (typeof cols)[number]) => `NULL /* ${c.name} ${c.dataType} */`;
        const where = keys.map((c) => `${quoteIdent(c.name)} = ${ph(c)}`).join("\n  AND ");
        if (what === "select") sql = `SELECT ${cols.map((c) => `t.${quoteIdent(c.name)}`).join(",\n       ")}\nFROM ${full} t;`;
        else if (what === "insert")
          sql = `INSERT INTO ${full} (${cols.map((c) => quoteIdent(c.name)).join(", ")})\nVALUES (${cols.map(ph).join(",\n        ")});`;
        else if (what === "update")
          sql = `UPDATE ${full}\nSET ${cols.filter((c) => !c.isPk).map((c) => `${quoteIdent(c.name)} = ${ph(c)}`).join(",\n    ")}\nWHERE ${where};`;
        else sql = `DELETE\nFROM ${full}\nWHERE ${where};`;
      }
      openConsole(info.conn, info.db, sql);
    } catch (e) {
      setStatus(`Generate failed: ${String(e)}`);
    }
  };

  const exportTable = async (info: NodeInfo) => {
    if (!info.schema || !info.name) return;
    const target = targetId(info.conn.id, info.db && info.db !== (info.conn.database || "postgres") ? info.db : undefined);
    const path = await save({ defaultPath: `${info.name}.csv`, filters: [{ name: "CSV (Excel)", extensions: ["csv"] }] });
    if (!path) return;
    setStatus(`Exporting ${info.schema}.${info.name}…`);
    try {
      const { q } = dialectOf(info.conn.kind);
      const n = await api.exportCsv(target, `SELECT * FROM ${q(info.schema)}.${q(info.name)}`, path);
      setStatus(`Exported ${n} rows to ${path}`);
    } catch (e) {
      setStatus(String(e));
    }
  };

  const dangerous = async (info: NodeInfo, verb: "TRUNCATE" | "DROP") => {
    if (!info.schema || !info.name) return;
    const target = targetId(info.conn.id, info.db && info.db !== (info.conn.database || "postgres") ? info.db : undefined);
    const kw = verb === "DROP" ? (info.kind === "view" ? "VIEW" : info.kind === "matview" ? "MATERIALIZED VIEW" : info.kind === "sequence" ? "SEQUENCE" : "TABLE") : "TABLE";
    const { q } = dialectOf(info.conn.kind);
    const sql = `${verb} ${verb === "DROP" ? kw + " " : ""}${q(info.schema)}.${q(info.name)};`;
    if (!confirm(`[${info.conn.name}] Execute?\n\n${sql}`)) return;
    try {
      await api.executeBatch(target, [sql]);
      setStatus(`Executed: ${sql}`);
      info.refresh?.();
    } catch (e) {
      setStatus(String(e));
    }
  };

  /** Redis 브라우저 탭 (Medis 식) */
  const openRedis = (c: ConnectionView, db?: string, key?: string) => {
    const database = redisDb(db ?? c.database);
    const target = targetId(c.id, database);
    const existing = tabsRef.current.find((t) => t.kind === "redis" && t.target === target);
    if (existing) {
      if (key) patch(existing.id, { redisKey: key });
      return activate(existing);
    }
    const tab = addTab(c, "redis", { title: `${c.name} [${database}]`, database, group: undefined });
    patch(tab.id, { target, redisKey: key });
  };

  /** 트리 노드 더블클릭 / F4 */
  const activateNode = (info: NodeInfo) => {
    if (info.conn.kind === "redis") {
      if (info.kind === "key") return openRedis(info.conn, info.db, info.name);
      if (info.kind === "conn" || info.kind === "db") return openRedis(info.conn, info.db);
    }
    if (isRelation(info.kind) && info.schema && info.name) openTable(info.conn, info.schema, info.name, info.db);
    else if (info.kind === "routine" || info.kind === "sequence") openDdl(info);
  };

  // ───────── Database Explorer 동작 (DataGrip 키맵) ─────────
  const K = { edit: sk("tree.editData"), props: sk("tree.properties"), refresh: sk("tree.refresh"), deact: sk("tree.disconnect"), jump: sk("tree.jumpConsole"), ddl: sk("tree.ddl"), gen: sk("tree.sqlGen"), ddlClip: sk("tree.ddlCopy"), ddlCons: sk("tree.ddlConsole"), ref: sk("tree.copyRef"), doc: sk("tree.quickDoc"), drop: sk("tree.remove"), dup: sk("tree.duplicate"), console: sk("console.new"), force: bindings("tree.refresh")[1] ? formatSpec(bindings("tree.refresh")[1]) : undefined, newK: sk("tree.newDs") };
  const [quickDoc, setQuickDoc] = useState<{ title: string; text: string } | null>(null);
  const nodeTarget = (info: NodeInfo) => targetId(info.conn.id, info.db && info.db !== (info.conn.database || "postgres") ? info.db : undefined);
  const fqn = (info: NodeInfo) => [info.db, info.schema, info.name].filter(Boolean).join(".") || info.conn.name;
  const ddlText = (info: NodeInfo) => meta.ddl(nodeTarget(info), info.schema!, info.name!, info.kind);

  const ddlToClipboard = async (info: NodeInfo) => {
    try {
      await navigator.clipboard.writeText(await ddlText(info));
      setStatus(`DDL of ${info.schema}.${info.name} copied to clipboard`);
    } catch (e) {
      setStatus(String(e));
    }
  };
  const ddlToConsole = async (info: NodeInfo) => {
    try {
      openConsole(info.conn, info.db, await ddlText(info));
    } catch (e) {
      setStatus(String(e));
    }
  };
  /** SQL Generator 도구 창: DDL + Copy / Save to File / Run in Console */
  const sqlGenerator = async (info: NodeInfo) => {
    if (!info.schema || !info.name) return;
    try {
      const ddl = await ddlText(info);
      addTab(info.conn, "ddl", { title: `SQL Generator: ${info.name}`, sql: ddl, database: info.db });
    } catch (e) {
      setStatus(String(e));
    }
  };
  const showQuickDoc = async (info: NodeInfo) => {
    if (info.kind === "conn") return setQuickDoc({ title: info.conn.name, text: `${info.conn.user}@${info.conn.host}:${info.conn.port}${info.conn.database ? "/" + info.conn.database : ""}${info.conn.ssh?.enabled ? `\nvia SSH ${info.conn.ssh.user}@${info.conn.ssh.host}` : ""}` });
    if (!info.schema || !info.name) return setQuickDoc({ title: fqn(info), text: info.kind });
    try {
      setQuickDoc({ title: fqn(info), text: await ddlText(info) });
    } catch (e) {
      setQuickDoc({ title: fqn(info), text: String(e) });
    }
  };
  /** Jump to Query Console…: 이 데이터 소스의 콘솔 목록 + New Query Console */
  const jumpToConsole = (info: NodeInfo, x?: number, y?: number) => {
    const consoles = tabsRef.current.filter((t) => t.kind === "console" && t.connId === info.conn.id);
    const el = document.querySelector(".tree-node.selected")?.getBoundingClientRect();
    setTabMenu({
      x: x ?? (el ? el.left + 40 : 300),
      y: y ?? (el ? el.bottom : 200),
      items: [
        { label: "New Query Console", shortcut: K.console, action: () => openConsole(info.conn, info.db) },
        ...(consoles.length ? ([{ separator: true }] as MenuItem[]) : []),
        ...consoles.map((t) => ({ label: t.title, action: () => activate(t) })),
      ],
    });
  };
  const deactivate = async (c: ConnectionView) => {
    await api.disconnect(c.id);
    invalidateCompletion(c.id);
    setStatus(`Deactivated ${c.name}`);
    refresh();
  };
  const duplicateConn = async (c: ConnectionView) => {
    const { hasPassword: _h, connected: _c, id: _id, ...config } = c;
    const saved = await api.saveConnection({ ...config, id: "", name: `${c.name} (copy)` });
    await refresh();
    setDialog({ edit: { ...c, ...saved, hasPassword: false, connected: false } });
  };

  /** DataGrip 트리 우클릭 메뉴 (문서 1.11 구조) */
  const buildMenu = (info: NodeInfo): MenuItem[] => {
    const c = info.conn;
    const isObj = !!info.name;
    const rel = isRelation(info.kind);
    const items: MenuItem[] = [
      { label: "New", submenu: [{ label: "Query Console", shortcut: K.console, action: () => openConsole(c, info.db) }, { separator: true }, { label: "Data Source…", action: () => setDialog({}) }] },
    ];
    if (rel) items.push({ label: "Edit Data", shortcut: K.edit, action: () => activateNode(info) });
    if (info.kind === "conn") items.push({ label: "Properties", shortcut: K.props, action: () => setDialog({ edit: c }) });
    items.push({ separator: true });
    items.push({
      label: "Copy/Paste",
      submenu: [
        { label: "Copy Reference", shortcut: K.ref, action: () => navigator.clipboard.writeText(fqn(info)) },
        ...(info.kind === "conn" ? ([{ label: "Duplicate", shortcut: K.dup, action: () => duplicateConn(c) }] as MenuItem[]) : []),
      ],
    });
    items.push({ label: "Refresh", shortcut: K.refresh, action: () => info.refresh?.() });
    if (info.kind === "conn" && c.connected) items.push({ label: "Deactivate", shortcut: K.deact, action: () => deactivate(c) });
    items.push({ label: "Quick Documentation", shortcut: K.doc, action: () => showQuickDoc(info) });
    items.push({ separator: true });
    items.push({
      label: "Navigation",
      submenu: [
        ...(isObj ? ([{ label: "Go to DDL", shortcut: K.ddl, action: () => openDdl(info) }] as MenuItem[]) : []),
        { label: "Jump to Query Console…", shortcut: K.jump, action: () => jumpToConsole(info) },
      ],
    });
    if (isObj)
      items.push({
        label: "SQL Scripts",
        submenu: [
          { label: "SQL Generator…", shortcut: K.gen, action: () => sqlGenerator(info) },
          { label: "Generate DDL to Clipboard", shortcut: K.ddlClip, action: () => ddlToClipboard(info) },
          { label: "Generate DDL to Query Console", shortcut: K.ddlCons, action: () => ddlToConsole(info) },
          ...(rel
            ? ([
                { separator: true },
                { label: "SELECT Statement", action: () => generate(info, "select") },
                ...(info.kind === "table" || info.kind === "partitioned"
                  ? ([
                      { label: "INSERT Statement", action: () => generate(info, "insert") },
                      { label: "UPDATE Statement", action: () => generate(info, "update") },
                      { label: "DELETE Statement", action: () => generate(info, "delete") },
                    ] as MenuItem[])
                  : []),
              ] as MenuItem[])
            : []),
        ],
      });
    if (isObj && info.kind !== "routine")
      items.push({
        label: "Object Actions",
        submenu: [
          ...(info.kind === "table" || info.kind === "partitioned" ? ([{ label: "Truncate…", danger: true, action: () => dangerous(info, "TRUNCATE") }] as MenuItem[]) : []),
          { label: "Drop…", shortcut: K.drop, danger: true, action: () => dangerous(info, "DROP") },
        ],
      });
    if (rel) items.push({ label: "Import/Export", submenu: [{ label: "Export Data to File…", action: () => exportTable(info) }] });
    items.push({ label: "Tools", submenu: [{ label: "Manage Shown Schemas…", action: () => document.querySelector<HTMLElement>(".tree-node.conn.selected .scope-badge, .tree-node.selected .scope-badge")?.click() }] });
    if (info.kind === "conn") {
      items.push({ label: "Diagnostics", submenu: [{ label: "Force Refresh", shortcut: K.force, action: () => info.refresh?.() }] });
      items.push({ separator: true });
      items.push({ label: "Remove Data Source", shortcut: K.drop, danger: true, action: () => removeConn(c) });
    }
    return items;
  };

  // ───────── 콘솔 DB / 스키마 선택 (DataGrip 콘솔 오른쪽 위) ─────────
  const [dbCache, setDbCache] = useState<Record<string, string[]>>({});
  const [schemaCache, setSchemaCache] = useState<Record<string, string[]>>({});
  const loadDbList = (connId: string) => {
    if (dbCache[connId]) return;
    meta.databases(connId).then((d) => setDbCache((c) => ({ ...c, [connId]: d })), () => {});
  };
  const loadSchemaList = (target: string) => {
    if (schemaCache[target]) return;
    meta.schemas(target).then((d) => setSchemaCache((c) => ({ ...c, [target]: d.filter((x) => !x.isSystem).map((x) => x.name) })), () => {});
  };
  const switchDatabase = (tab: Tab, db: string) => {
    const c = conns.find((x) => x.id === tab.connId);
    if (!c) return;
    const database = db !== (c.database || "postgres") ? db : undefined;
    api.closeSession(tab.id).catch(() => {}); // 다음 실행 때 새 DB로 세션을 연다
    patch(tab.id, { database, target: targetId(c.id, database), schema: undefined });
    setStatus(`Console switched to database ${db}`);
  };
  const switchSchema = async (tab: Tab, schema: string) => {
    try {
      const c = conns.find((x) => x.id === tab.connId);
      if (c?.kind === "mysql") {
        // MySQL 은 스키마 = 데이터베이스
        await api.runQuery(tab.target, `USE ${dialectOf("mysql").q(schema)}`, tab.id);
        patch(tab.id, { schema });
        setStatus(`USE ${schema}`);
        return;
      }
      await api.runQuery(tab.target, `SET search_path TO ${dialectOf("postgres").q(schema)}, public`, tab.id);
      patch(tab.id, { schema });
      setStatus(`search_path = ${schema}, public`);
    } catch (e) {
      setStatus(String(e));
    }
  };

  // ───────── 실행 ─────────
  const run = async (tab: Tab, sql: string) => {
    if (!sql.trim()) return;
    patch(tab.id, { running: true, error: null, lastSql: sql });
    setStatus("Executing…");
    try {
      const r = await api.runQuery(tab.target, sql, tab.id);
      patch(tab.id, { running: false, result: r });
      setStatus(r.hasResultSet ? `${r.rows.length}${r.truncated ? "+" : ""} rows retrieved in ${r.elapsedMs} ms` : `${r.rowsAffected} rows affected in ${r.elapsedMs} ms`);
      refresh();
    } catch (e) {
      patch(tab.id, { running: false, error: String(e), result: null });
      setStatus("Query failed");
    }
  };

  // ───────── 콘솔 실행 (문장 여러 개 → 결과 탭 여러 개, Output 로그) ─────────
  const nowTs = () => {
    const d = new Date();
    const p2 = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${p2(d.getMonth() + 1)}-${p2(d.getDate())} ${p2(d.getHours())}:${p2(d.getMinutes())}:${p2(d.getSeconds())}`;
  };
  const updTab = (id: string, fn: (t: Tab) => Tab) => setTabs((ts) => ts.map((t) => (t.id === id ? fn(t) : t)));
  const log = (id: string, level: OutputLine["level"], text: string) => updTab(id, (t) => ({ ...t, output: [...t.output.slice(-999), { ts: nowTs(), text, level }] }));
  /** 결과 탭 제목: 쿼리 앞 주석 → 테이블 이름 → Result N (DataGrip) */
  const resultTitle = (sql: string, n: number) => {
    const c = /^\s*(?:--\s*(.+)|\/\*\s*([\s\S]*?)\*\/)/.exec(sql);
    if (c) return (c[1] ?? c[2]).trim().split("\n")[0].slice(0, 40);
    const f = /\bfrom\s+((?:"[^"]+"|\w+)(?:\.(?:"[^"]+"|\w+))?)/i.exec(sql);
    return f ? f[1].replace(/"/g, "") : `Result ${n}`;
  };
  const isUnsafe = (sql: string) => {
    const s2 = sql.replace(/--.*$/gm, "").replace(/\/\*[\s\S]*?\*\//g, "").trim().toLowerCase();
    return (/^delete\s+from\b/.test(s2) || /^update\b/.test(s2)) && !/\bwhere\b/.test(s2);
  };

  const executeConsole = async (tabId: string, stmts: string[], explain?: "plan" | "analyze") => {
    const tab = tabsRef.current.find((t) => t.id === tabId);
    if (!tab || !stmts.length) return;
    const unsafe = stmts.filter(isUnsafe);
    if (unsafe.length && !confirm(`Potentially unsafe query: ${unsafe.length > 1 ? `${unsafe.length} statements have` : "statement has"} no WHERE clause.\n\n${unsafe[0].slice(0, 300)}\n\nExecute anyway?`)) return;
    const c = conns.find((x) => x.id === tab.connId);
    // 고정(pin)되지 않은 결과 탭은 새 결과로 바꾼다
    updTab(tabId, (t) => ({ ...t, running: true, results: t.results.filter((r) => r.pinned), cursorEntry: undefined }));
    setStatus("Executing…");
    let n = tab.results.filter((r) => r.pinned).length;
    let lastActive = "output";
    for (const sql0 of stmts) {
      const isMy = c?.kind === "mysql";
      const bare = sql0.replace(/;\s*$/, "");
      const sql = !explain ? sql0 : isMy ? `EXPLAIN ${explain === "analyze" ? "ANALYZE " : ""}${bare}` : `EXPLAIN (FORMAT JSON${explain === "analyze" ? ", ANALYZE, BUFFERS" : ""}) ${bare}`;
      log(tabId, "sql", `${c?.name ?? ""}> ${sql0}`);
      try {
        const r = await api.runQuery(tab.target, sql, tabId);
        pushHistory({ connId: tab.connId, database: tab.database, sql: sql0 });
        updTab(tabId, (t) => ({ ...t, inTx: !!r.inTransaction }));
        if (explain && !isMy) {
          const plan = parsePgPlan(String(r.rows[0]?.[0] ?? ""));
          const entry: ResultEntry = { id: crypto.randomUUID(), title: resultTitle(sql0, ++n), sql: sql0, kind: "plan", result: null, plan: plan ?? undefined, pinned: false };
          updTab(tabId, (t) => ({ ...t, results: [...t.results, entry] }));
          lastActive = entry.id;
          log(tabId, "info", `Plan retrieved in ${r.elapsedMs} ms`);
        } else if (r.hasResultSet) {
          const entry: ResultEntry = { id: crypto.randomUUID(), title: resultTitle(sql0, ++n), sql: sql0, kind: "result", result: r, pinned: false };
          updTab(tabId, (t) => ({ ...t, results: [...t.results, entry], cursorEntry: r.truncated ? entry.id : t.cursorEntry }));
          lastActive = entry.id;
          log(tabId, "info", `${r.rows.length}${r.truncated ? "+" : ""} row${r.rows.length === 1 ? "" : "s"} retrieved starting from 1 in ${r.elapsedMs} ms`);
        } else {
          log(tabId, "info", `${r.rowsAffected} row${r.rowsAffected === 1 ? "" : "s"} affected in ${r.elapsedMs} ms`);
        }
      } catch (e) {
        log(tabId, "error", String(e));
        lastActive = "output";
        updTab(tabId, (t) => ({ ...t, running: false, activeResult: "output" }));
        setStatus("Query failed");
        refresh();
        return;
      }
    }
    updTab(tabId, (t) => ({ ...t, running: false, activeResult: lastActive, lastSql: stmts[stmts.length - 1] }));
    setStatus(`${stmts.length} statement${stmts.length > 1 ? "s" : ""} executed`);
    refresh();
  };

  const fetchMoreEntry = async (tabId: string, entryId: string) => {
    const tab = tabsRef.current.find((t) => t.id === tabId);
    if (!tab || tab.cursorEntry !== entryId) {
      // 서버 커서는 마지막 결과에만 열려 있다
      updTab(tabId, (t) => ({ ...t, results: t.results.map((r) => (r.id === entryId && r.result ? { ...r, result: { ...r.result, truncated: false } } : r)) }));
      return;
    }
    const shown = tab.results.find((x) => x.id === entryId)?.result?.rows.length ?? 0;
    if (shown >= MAX_ROWS) return setStatus(capMsg(shown));
    updTab(tabId, (t) => ({ ...t, loadingMore: true }));
    try {
      const r = await api.fetchMore(tabId);
      updTab(tabId, (t) => ({
        ...t,
        loadingMore: false,
        cursorEntry: r.truncated ? entryId : undefined,
        results: t.results.map((x) => (x.id === entryId && x.result ? { ...x, result: { ...x.result, rows: [...x.result.rows, ...r.rows], truncated: r.truncated } } : x)),
      }));
    } catch (e) {
      updTab(tabId, (t) => ({ ...t, loadingMore: false, cursorEntry: undefined }));
      setStatus(String(e));
    }
  };

  const setTxMode = async (tab: Tab, manual: boolean) => {
    try {
      await api.setTxMode(tab.target, tab.id, manual);
      updTab(tab.id, (t) => ({ ...t, txManual: manual, inTx: manual ? t.inTx : false }));
      log(tab.id, "info", `Transaction mode: ${manual ? "Manual" : "Auto"}${!manual && tab.inTx ? " (pending transaction committed)" : ""}`);
    } catch (e) {
      setStatus(String(e));
    }
  };
  const txEnd = async (tab: Tab, what: "commit" | "rollback") => {
    try {
      if (what === "commit") await api.commitTx(tab.id);
      else await api.rollbackTx(tab.id);
      updTab(tab.id, (t) => ({ ...t, inTx: false }));
      log(tab.id, "info", what === "commit" ? "Transaction committed" : "Transaction rolled back");
    } catch (e) {
      log(tab.id, "error", String(e));
    }
  };

  const applyTableFilter = (tab: Tab, where: string, orderBy: string) => {
    if (!tab.edit) return;
    const { q } = dialectOf(conns.find((x) => x.id === tab.connId)?.kind);
    let sql = `SELECT * FROM ${q(tab.edit.schema)}.${q(tab.edit.table)}`;
    if (where) sql += ` WHERE ${where}`;
    if (orderBy) sql += ` ORDER BY ${orderBy}`;
    sql += ";";
    patch(tab.id, { where, orderBy, tableSql: sql });
    run({ ...tab, tableSql: sql }, sql);
  };

  const fetchMore = async (tab: Tab) => {
    const shown = tab.result?.rows.length ?? 0;
    if (shown >= MAX_ROWS) return setStatus(capMsg(shown));
    patch(tab.id, { loadingMore: true });
    try {
      const r = await api.fetchMore(tab.id);
      setTabs((ts) =>
        ts.map((t) => (t.id === tab.id && t.result ? { ...t, loadingMore: false, result: { ...t.result, rows: [...t.result.rows, ...r.rows], truncated: r.truncated } } : t)),
      );
      setStatus(`${(tab.result?.rows.length ?? 0) + r.rows.length}${r.truncated ? "+" : ""} rows retrieved`);
    } catch (e) {
      setTabs((ts) => ts.map((t) => (t.id === tab.id && t.result ? { ...t, loadingMore: false, result: { ...t.result, truncated: false } } : t)));
      setStatus(String(e));
    }
  };

  const cancel = async (tab: Tab) => {
    try {
      await api.cancelQuery(tab.id);
      setStatus("Cancel requested");
    } catch (e) {
      setStatus(String(e));
    }
  };

  const submitEdits = async (tab: Tab, statements: string[]) => {
    const n = await api.executeBatch(tab.target, statements);
    setStatus(`Submitted ${statements.length} statements, ${n} rows affected`);
    if (tab.lastSql) run(tab, tab.lastSql);
  };

  // ───────── 탭 닫기 / 분할 ─────────
  const closeTabs = (ids: string[]) => {
    if (!ids.length) return;
    for (const t of tabsRef.current.filter((x) => ids.includes(x.id))) closedStack.current = [snapshot(t), ...closedStack.current].slice(0, 30);
    mru.current = mru.current.filter((x) => !ids.includes(x));
    for (const id of ids) {
      editorApis.current.delete(id);
      liveSql.current.delete(id);
      api.closeSession(id).catch(() => {});
    }
    setTabs((ts) => {
      let rest = ts.filter((t) => !ids.includes(t.id));
      const leftEmpty = !rest.some((t) => t.group === 0) && rest.some((t) => t.group === 1);
      if (leftEmpty) rest = rest.map((t) => ({ ...t, group: 0 as Group }));
      setActiveIds((a) => {
        const byMru = (g: Group) => mru.current.find((id) => rest.some((t) => t.id === id && t.group === g)) ?? last(rest.filter((t) => t.group === g))?.id ?? null;
        if (leftEmpty) return [a[1] && rest.some((t) => t.id === a[1]) ? a[1] : byMru(0), null];
        const pick = (g: Group) => (a[g] && rest.some((t) => t.id === a[g]) ? a[g] : byMru(g));
        return [pick(0), pick(1)];
      });
      // 오른쪽(아래) 그룹이 비면 분할 해제
      if (!rest.some((t) => t.group === 1)) {
        setSplit(null);
        setFocused(0);
      }
      return rest;
    });
  };

  const moveTo = (id: string, g: Group, dir?: "right" | "down") => {
    if (dir) setSplit(dir);
    setTabs((ts) => {
      const next = ts.map((t) => (t.id === id ? { ...t, group: g } : t));
      setActiveIds((a) => {
        const r: [string | null, string | null] = [a[0], a[1]];
        r[g] = id;
        const other: Group = g === 0 ? 1 : 0;
        if (r[other] === id) r[other] = last(next.filter((t) => t.group === other))?.id ?? null;
        return r;
      });
      if (!next.some((t) => t.group === 1)) setSplit(null);
      return next;
    });
    setFocused(g);
  };

  // ───────── 탭 드래그 앤 드롭 (DataGrip: 순서 변경 / 다른 그룹으로 이동 / 가장자리로 끌어 분할) ─────────
  const [dragId, setDragId] = useState<string | null>(null);
  const [dropHint, setDropHint] = useState<{ g: Group; zone: "tabs" | "center" | "right" | "down"; beforeId: string | null } | null>(null);

  const dropTab = (id: string, g: Group, beforeId: string | null, dir?: "right" | "down") => {
    if (dir) setSplit(dir);
    setTabs((ts) => {
      const t = ts.find((x) => x.id === id);
      if (!t || beforeId === id) return ts;
      const rest = ts.filter((x) => x.id !== id);
      let idx = beforeId ? rest.findIndex((x) => x.id === beforeId) : -1;
      if (idx < 0) {
        const lastIdx = rest.map((x) => x.group).lastIndexOf(g);
        idx = lastIdx < 0 ? rest.length : lastIdx + 1;
      }
      let next = [...rest.slice(0, idx), { ...t, group: g }, ...rest.slice(idx)];
      // 왼쪽(위) 그룹이 비면 오른쪽 그룹을 왼쪽으로 당기고 분할 해제
      if (!next.some((x) => x.group === 0)) next = next.map((x) => ({ ...x, group: 0 as Group }));
      const splitAlive = next.some((x) => x.group === 1);
      if (!splitAlive) setSplit(null);
      const target: Group = splitAlive ? g : 0;
      setActiveIds((a) => {
        const r: [string | null, string | null] = splitAlive ? [a[0], a[1]] : [a[0], null];
        r[target] = id;
        const other: Group = target === 0 ? 1 : 0;
        if (r[other] === id || (r[other] && !next.some((x) => x.id === r[other] && x.group === other)))
          r[other] = last(next.filter((x) => x.group === other))?.id ?? null;
        return r;
      });
      setFocused(target);
      return next;
    });
  };

  const endDrag = () => {
    setDragId(null);
    setDropHint(null);
  };

  const unsplit = () => {
    setTabs((ts) => ts.map((t) => ({ ...t, group: 0 })));
    setSplit(null);
    setFocused(0);
    setActiveIds((a) => [a[focused] ?? a[0], null]);
  };

  /** Reopen Closed Tab / Recent Files 에서 다시 열기 */
  const reopen = (snap: TabSnapshot) => {
    const c = conns.find((x) => x.id === snap.connId);
    if (!c) return;
    const open = tabsRef.current.find((t) => snapKey(t) === snap.key);
    if (open) return activate(open);
    if (snap.kind === "table" && snap.edit) {
      openTableTab(c, snap.edit.schema, snap.edit.table, snap.database, undefined, snap.where, snap.orderBy);
    } else addTab(c, snap.kind, { title: snap.title, sql: snap.sql, database: snap.database });
  };
  const reopenClosed = () => {
    const snap = closedStack.current.shift();
    if (snap) reopen(snap);
  };

  /** Split Right / Down: 같은 대상을 독립된 탭으로 복제해서 오른쪽(아래) 그룹에 연다 */
  const splitClone = (t: Tab, dir: "right" | "down") => {
    const c = conns.find((x) => x.id === t.connId);
    if (!c) return;
    setSplit(dir);
    if (t.kind === "table" && t.edit) openTableTab(c, t.edit.schema, t.edit.table, t.database, 1, t.where, t.orderBy);
    else addTab(c, t.kind, { title: t.kind === "console" ? `${t.title} (copy)` : t.title, sql: sqlOf(t), database: t.database, group: 1 });
  };

  const togglePin = (t: Tab) => {
    setTabs((ts) => {
      const flipped = ts.map((x) => (x.id === t.id ? { ...x, pinned: !x.pinned } : x));
      // 고정 탭은 그룹 안에서 왼쪽으로 (Keep pinned tabs on the left)
      return [...flipped.filter((x) => x.pinned), ...flipped.filter((x) => !x.pinned)];
    });
  };

  const openTabMenu = (e: React.MouseEvent, t: Tab) => {
    e.preventDefault();
    e.stopPropagation();
    activate(t);
    const sameGroup = tabs.filter((x) => x.group === t.group);
    const idx = sameGroup.findIndex((x) => x.id === t.id);
    const other: Group = t.group === 0 ? 1 : 0;
    const c = conns.find((x) => x.id === t.connId);
    const items: MenuItem[] = [
      { label: "Close", shortcut: sk("tab.close"), action: () => closeTabs([t.id]) },
      { label: "Close Other Tabs", action: () => closeTabs(sameGroup.filter((x) => x.id !== t.id && !x.pinned).map((x) => x.id)) },
      { label: "Close All but Pinned", action: () => closeTabs(sameGroup.filter((x) => !x.pinned).map((x) => x.id)) },
      { label: "Close Tabs to the Left", action: () => closeTabs(sameGroup.slice(0, idx).filter((x) => !x.pinned).map((x) => x.id)), disabled: idx === 0 },
      { label: "Close Tabs to the Right", action: () => closeTabs(sameGroup.slice(idx + 1).filter((x) => !x.pinned).map((x) => x.id)), disabled: idx === sameGroup.length - 1 },
      { label: "Close All Tabs", action: () => closeTabs(sameGroup.map((x) => x.id)) },
      { label: "Reopen Closed Tab", action: reopenClosed, disabled: closedStack.current.length === 0 },
      { separator: true },
      { label: t.pinned ? "Unpin Tab" : "Pin Tab", action: () => togglePin(t) },
      {
        label: "Copy Path/Reference…",
        submenu: [
          { label: "Copy Reference", action: () => navigator.clipboard.writeText(t.edit ? `${t.edit.schema}.${t.edit.table}` : t.title) },
          ...(c ? ([{ label: "Copy Data Source Name", action: () => navigator.clipboard.writeText(c.name) }] as MenuItem[]) : []),
        ],
      },
      { separator: true },
      ...(split
        ? ([
            { label: "Move To Opposite Group", action: () => moveTo(t.id, other) },
            { label: "Open In Opposite Group", action: () => { const cc = c; if (!cc) return; if (t.kind === "table" && t.edit) openTableTab(cc, t.edit.schema, t.edit.table, t.database, other, t.where, t.orderBy); else addTab(cc, t.kind, { title: t.title, sql: sqlOf(t), database: t.database, group: other }); } },
            { label: "Unsplit All", action: unsplit },
          ] as MenuItem[])
        : ([
            { label: "Split Right", action: () => splitClone(t, "right") },
            { label: "Split Down", action: () => splitClone(t, "down") },
            { label: "Split and Move Right", action: () => moveTo(t.id, 1, "right"), disabled: sameGroup.length < 2 },
            { label: "Split and Move Down", action: () => moveTo(t.id, 1, "down"), disabled: sameGroup.length < 2 },
          ] as MenuItem[])),
    ];
    if (t.kind === "console" && c)
      items.push(
        { separator: true },
        { label: "Duplicate", action: () => addTab(c, "console", { title: `${t.title} (copy)`, sql: sqlOf(t), database: t.database, group: t.group }) },
        {
          label: "Rename File…",
          shortcut: "⇧F6",
          action: () => {
            const name = prompt("Rename console", t.title);
            if (name && name.trim()) patch(t.id, { title: name.trim() });
          },
        },
      );
    setTabMenu({ x: e.clientX, y: e.clientY, items });
  };

  // ───────── Switcher / Recent Files / Maximize ─────────
  const [switcher, setSwitcher] = useState<{ items: SwitchItem[]; start: number } | null>(null);
  const [recentOpen, setRecentOpen] = useState(false);
  const [maximized, setMaximized] = useState(false);
  const kindIcon = (k: Tab["kind"]) => (k === "table" ? "▦" : k === "ddl" ? "ƒ" : k === "redis" ? "◆" : "⌘");
  const hintOf = (connId: string, db?: string) => {
    const c = conns.find((x) => x.id === connId);
    return c ? c.name + (db ? ` · ${db}` : "") : "";
  };
  const openSwitcher = (back: boolean) => {
    const ts = tabsRef.current;
    const ordered = [...mru.current.map((id) => ts.find((t) => t.id === id)).filter((t): t is Tab => !!t), ...ts.filter((t) => !mru.current.includes(t.id))];
    if (!ordered.length) return;
    const items = ordered.map((t) => ({ key: t.id, icon: kindIcon(t.kind), title: t.title, hint: hintOf(t.connId, t.database), color: conns.find((c) => c.id === t.connId)?.color }));
    setSwitcher({ items, start: back ? items.length - 1 : 1 });
  };
  const recentItems = (): SwitchItem[] => {
    const openKeys = new Map(tabsRef.current.map((t) => [snapKey(t), t] as const));
    return recent.current
      .filter((r) => conns.some((c) => c.id === r.connId))
      .map((r) => ({ key: r.key, icon: kindIcon(r.kind), title: r.title, hint: hintOf(r.connId, r.database), open: openKeys.has(r.key), color: conns.find((c) => c.id === r.connId)?.color }));
  };

  // ───────── Search Everywhere (⇧⇧) / Go to Database Object (⌘O) / Find Action (⇧⌘A) ─────────
  const [se, setSe] = useState<SeTab | null>(null);

  // ── Tool windows: Database Explorer (⌘1), Find (⌘3) ──
  const [explorerHidden, setExplorerHidden] = useState(false);
  const toggleExplorer = () => {
    const tree = document.querySelector<HTMLElement>("[data-tree]");
    const focusedInTree = !!document.activeElement?.closest?.("[data-tree]");
    if (explorerHidden || maximized) {
      setExplorerHidden(false);
      setMaximized(false);
      setTimeout(() => document.querySelector<HTMLElement>("[data-tree]")?.focus(), 50);
    } else if (focusedInTree) setExplorerHidden(true);
    else tree?.focus();
  };
  const [findOpen, setFindOpen] = useState(false);
  const [findRuns, setFindRuns] = useState<FindRun[]>([]);
  const [findActive, setFindActive] = useState<string | null>(null);
  const [ftsScope, setFtsScope] = useState<FtsScope | null>(null);
  const openFts = async () => {
    const sel = treeSel;
    const c = sel?.conn ?? conns.find((x) => x.id === active?.connId);
    if (!c) return setStatus("Select a data source, database, schema or table to search in");
    const db = sel?.db ?? active?.database ?? (c.database || "postgres");
    if (sel && isRelation(sel.kind) && sel.schema && sel.name)
      return setFtsScope({ connId: c.id, connName: c.name, db, schemas: [], tables: [[sel.schema, sel.name]] });
    if (sel?.kind === "schema" && sel.schema) return setFtsScope({ connId: c.id, connName: c.name, db, schemas: [sel.schema], tables: [] });
    try {
      const schemas = (await meta.schemas(targetId(c.id, db))).filter((s) => !s.isSystem).map((s) => s.name);
      setFtsScope({ connId: c.id, connName: c.name, db, schemas, tables: [] });
    } catch (e) {
      setStatus(String(e));
    }
  };
  const runFts = async (scope: FtsScope, req: FtsRequest) => {
    setFtsScope(null);
    const id = crypto.randomUUID();
    const title = `"${req.text}"`;
    setFindRuns((rs) => [...rs.filter((r) => r.pinned), { id, title, scope, running: true, hits: [], text: req.text, matchCase: req.matchCase }]);
    setFindActive(id);
    setFindOpen(true);
    try {
      const hits = await meta.fullTextSearch(targetId(scope.connId, scope.db), req);
      setFindRuns((rs) => rs.map((r) => (r.id === id ? { ...r, running: false, hits } : r)));
      setStatus(`Full-text search: ${hits.reduce((n, h) => n + h.result.rows.length, 0)} matches in ${hits.length} tables`);
    } catch (e) {
      setFindRuns((rs) => rs.map((r) => (r.id === id ? { ...r, running: false, error: String(e) } : r)));
    }
  };

  // ── Editor font size ──
  const [, setFontTick] = useState(editorFontSize());
  const bumpFont = (d: number) => setFontTick(setEditorFontSize(d === 0 ? DEFAULT_FONT : editorFontSize() + d));
  const objNode = (o: DbObject): NodeInfo | null => {
    const conn = conns.find((c) => c.id === o.connId);
    if (!conn) return null;
    const kind = (o.kind === "function" || o.kind === "procedure" ? "routine" : o.kind) as NodeInfo["kind"];
    return { kind, conn, db: o.db, schema: o.schema, name: o.kind === "schema" ? undefined : o.name };
  };
  const seFiles = () => {
    const openKeys = new Map(tabsRef.current.map((t) => [snapKey(t), t] as const));
    const seen = new Set<string>();
    const items = [];
    for (const id of mru.current) {
      const t = tabsRef.current.find((x) => x.id === id);
      if (t) {
        seen.add(snapKey(t));
        items.push({ key: snapKey(t), icon: kindIcon(t.kind), title: t.title, hint: hintOf(t.connId, t.database), open: true });
      }
    }
    for (const r of recent.current)
      if (!seen.has(r.key) && conns.some((c) => c.id === r.connId))
        items.push({ key: r.key, icon: kindIcon(r.kind), title: r.title, hint: hintOf(r.connId, r.database), open: openKeys.has(r.key) });
    return items;
  };
  const seActions = (): ActionItem[] => {
    const t = active;
    const c = treeSel?.conn ?? conns.find((x) => x.id === t?.connId);
    return [
      { name: "New Query Console", shortcut: sk("console.new"), group: "File", run: () => c && openConsole(c, treeSel?.db ?? t?.database), disabled: !c },
      { name: "New Data Source…", group: "File", run: () => setDialog({}) },
      { name: "Data Source Properties", shortcut: sk("tree.properties"), group: "File", run: () => c && setDialog({ edit: c }), disabled: !c },
      { name: "Export Data Sources…", group: "File", run: () => exportConns() },
      { name: "Import Data Sources…", group: "File", run: () => importConns() },
      { name: "Settings…", shortcut: sk("settings"), group: "File", run: () => setSettingsOpen("appearance") },
      { name: "Keymap Reference", shortcut: sk("keymap"), group: "Help", run: () => setKeymapOpen(true) },
      { name: "Edit Keymap…", group: "Help", run: () => setSettingsOpen("keymap") },
      { name: "Theme: Dark", group: "Appearance", run: () => setThemePref("dark") },
      { name: "Theme: Light", group: "Appearance", run: () => setThemePref("light") },
      { name: "Theme: High contrast", group: "Appearance", run: () => setThemePref("hc") },
      { name: "Theme: Sync with OS", group: "Appearance", run: () => setThemePref("system") },
      { name: "Quick Switch Theme…", shortcut: sk("theme"), group: "Appearance", run: () => setSchemeOpen(true) },
      { name: "Execute", shortcut: sk("editor.run"), group: "Run", run: () => t && editorApis.current.get(t.id)?.run(), disabled: t?.kind !== "console" },
      { name: "Explain Plan", group: "Run", run: () => t && editorApis.current.get(t.id)?.run("plan"), disabled: t?.kind !== "console" },
      { name: "Explain Analyse", group: "Run", run: () => t && editorApis.current.get(t.id)?.run("analyze"), disabled: t?.kind !== "console" },
      { name: "Cancel Running Statements", shortcut: sk("editor.cancel"), group: "Run", run: () => t && cancel(t), disabled: !t?.running },
      { name: "Commit", group: "Transaction", run: () => t && txEnd(t, "commit"), disabled: !t?.inTx },
      { name: "Roll Back", group: "Transaction", run: () => t && txEnd(t, "rollback"), disabled: !t?.inTx },
      { name: "Transaction Mode: Manual", group: "Transaction", run: () => t && setTxMode(t, true), disabled: t?.kind !== "console" },
      { name: "Transaction Mode: Auto", group: "Transaction", run: () => t && setTxMode(t, false), disabled: t?.kind !== "console" },
      { name: "Browse Query History…", shortcut: sk("editor.history"), group: "Run", run: () => t && setHistoryFor(t.id), disabled: t?.kind !== "console" },
      { name: "Go to Database Object…", shortcut: sk("goto.object"), group: "Navigate", run: () => setTimeout(() => setSe("database")) },
      { name: "Go to Symbol…", shortcut: sk("goto.symbol"), group: "Navigate", run: () => setTimeout(() => setSe("symbols")) },
      { name: "Go to File…", shortcut: sk("goto.file"), group: "Navigate", run: () => setTimeout(() => setSe("files")) },
      { name: "Full-text Search…", shortcut: sk("fts"), group: "Navigate", run: () => openFts() },
      { name: "Recent Files", shortcut: sk("recent"), group: "Navigate", run: () => setRecentOpen(true) },
      { name: "Switcher", shortcut: "⌃⇥", group: "Navigate", run: () => openSwitcher(false) },
      { name: "Reopen Closed Tab", shortcut: sk("tab.reopen"), group: "Window", run: reopenClosed },
      { name: "Close Tab", shortcut: sk("tab.close"), group: "Window", run: () => t && closeTabs([t.id]), disabled: !t },
      { name: "Close All Tabs", group: "Window", run: () => closeTabs(tabsRef.current.map((x) => x.id)) },
      { name: "Split Right", group: "Window", run: () => t && splitClone(t, "right"), disabled: !t },
      { name: "Split Down", group: "Window", run: () => t && splitClone(t, "down"), disabled: !t },
      { name: "Unsplit All", group: "Window", run: unsplit, disabled: !split },
      { name: "Maximize Editor", shortcut: sk("maximize"), group: "Window", run: () => setMaximized((m) => !m) },
      { name: "Database Explorer", shortcut: sk("explorer"), group: "Tool Windows", run: () => toggleExplorer() },
      { name: "Find (Full-text Search results)", shortcut: sk("find.window"), group: "Tool Windows", run: () => setFindOpen((o) => !o) },
      { name: "Refresh", shortcut: sk("refresh"), group: "Database", run: () => refresh() },
      { name: "Increase Editor Font Size", shortcut: sk("font.bigger"), group: "Editor", run: () => bumpFont(1) },
      { name: "Decrease Editor Font Size", shortcut: sk("font.smaller"), group: "Editor", run: () => bumpFont(-1) },
      { name: "Reset Editor Font Size", group: "Editor", run: () => bumpFont(0) },
    ];
  };

  // 키 핸들러는 한 번만 등록되므로 최신 함수는 ref 로 넘긴다
  const latest = useRef({ toggleExplorer, openFts, bumpFont, setSe, setFindOpen, findOpen });
  latest.current = { toggleExplorer, openFts, bumpFont, setSe, setFindOpen, findOpen };
  const lastShift = useRef(0);

  // ───────── 단축키 (DataGrip 키맵) ─────────
  const keyState = useRef({ treeSel, active, focused, tabs, split });
  keyState.current = { treeSel, active, focused, tabs, split };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Switcher / Recent Files 팝업이 떠 있으면 팝업이 키를 처리한다
      if (document.querySelector(".switcher, .search-everywhere, .modal.fts, .modal.keymap, .keymap-recording")) return;
      const { treeSel, active, tabs } = keyState.current;
      const L = latest.current;
      // ⇧⇧: Search Everywhere
      if (e.key === "Shift") {
        if (!e.repeat) {
          const now = Date.now();
          if (now - lastShift.current < 400) {
            lastShift.current = 0;
            L.setSe("all");
          } else lastShift.current = now;
        }
        return;
      }
      lastShift.current = 0;
      // 명령 단축키는 lib/shortcuts.ts (Settings › Keymap 에서 바꾼다)
      const spec = eventSpec(e);
      const is = (id: string) => hit(spec, id);
      if (is("goto.object")) return e.preventDefault(), L.setSe("database");
      if (is("goto.symbol")) return e.preventDefault(), L.setSe("symbols");
      if (is("goto.file")) return e.preventDefault(), L.setSe("files");
      if (is("find.action")) return e.preventDefault(), L.setSe("actions");
      if (is("fts")) return e.preventDefault(), void L.openFts();
      if (is("explorer")) return e.preventDefault(), L.toggleExplorer();
      if (is("find.window")) return e.preventDefault(), L.setFindOpen((o) => !o);
      // ⇧⎋ Hide Active Tool Window
      if (e.shiftKey && e.key === "Escape") {
        const ae = document.activeElement;
        if (ae?.closest?.("[data-toolwindow=find]")) return e.preventDefault(), L.setFindOpen(false);
        if (ae?.closest?.("[data-tree]")) return e.preventDefault(), L.toggleExplorer();
      }
      if (is("font.bigger")) return e.preventDefault(), L.bumpFont(1);
      if (is("font.smaller")) return e.preventDefault(), L.bumpFont(-1);
      const inTree = (document.activeElement as HTMLElement | null)?.closest?.("[data-tree]");
      const inInput = ["INPUT", "TEXTAREA", "SELECT"].includes((document.activeElement as HTMLElement | null)?.tagName ?? "");
      if (is("console.new")) {
        e.preventDefault();
        const conn = treeSel?.conn ?? conns.find((c) => c.id === active?.connId);
        if (conn) openConsole(conn, treeSel?.db ?? active?.database);
        return;
      }
      if (inTree && treeSel && !inInput) {
        const act = (fn: () => void) => {
          e.preventDefault();
          e.stopPropagation();
          fn();
        };
        // Edit Data (데이터 소스에서는 기본 콘솔)
        if (is("tree.editData"))
          return act(() => ((treeSel.kind === "conn" || treeSel.kind === "db") && treeSel.conn.kind !== "redis" ? jumpToConsole(treeSel) : activateNode(treeSel)));
        if (is("tree.properties") && treeSel.kind === "conn") return act(() => setDialog({ edit: treeSel.conn }));
        if (is("tree.refresh")) return act(() => treeSel.refresh?.());
        if (is("tree.disconnect") && treeSel.conn.connected) return act(() => deactivate(treeSel.conn));
        if (is("tree.jumpConsole")) return act(() => jumpToConsole(treeSel));
        if (is("tree.sqlGen") && treeSel.name) return act(() => sqlGenerator(treeSel));
        if (is("tree.ddlCopy") && treeSel.name) return act(() => ddlToClipboard(treeSel));
        if (is("tree.ddlConsole") && treeSel.name) return act(() => ddlToConsole(treeSel));
        if (is("tree.quickDoc")) return act(() => showQuickDoc(treeSel));
        if (is("tree.newDs")) return act(() => setDialog({}));
        if (is("tree.duplicate") && treeSel.kind === "conn") return act(() => duplicateConn(treeSel.conn));
        if (is("tree.remove"))
          return act(() => (treeSel.kind === "conn" ? removeConn(treeSel.conn) : treeSel.name ? dangerous(treeSel, "DROP") : undefined));
      }
      if (is("tree.ddl") && inTree && treeSel && treeSel.name) {
        e.preventDefault();
        openDdl(treeSel);
        return;
      }
      if (is("tree.copyRef") && inTree && treeSel) {
        e.preventDefault();
        navigator.clipboard.writeText(treeSel.name ? `${treeSel.schema}.${treeSel.name}` : treeSel.schema ?? treeSel.db ?? treeSel.conn.name);
        return;
      }
      if (is("refresh")) {
        e.preventDefault();
        if (inTree && treeSel?.refresh) treeSel.refresh();
        else if (active?.kind === "table") applyTableFilter(active, active.where, active.orderBy);
        else {
          invalidateSearchIndex();
          for (const c of conns) invalidateCompletion(c.id);
          refresh();
        }
        return;
      }
      if (is("editor.history") && active?.kind === "console") {
        e.preventDefault();
        setHistoryFor(active.id);
        return;
      }
      if (is("editor.cancel") && !inTree && active?.running) {
        e.preventDefault();
        cancel(active);
        return;
      }
      if (is("tab.close") && active) {
        e.preventDefault();
        closeTabs([active.id]);
        return;
      }
      // ⌃⇥ / ⌃⇧⇥: Switcher (최근 사용 순)
      if (e.ctrlKey && !e.metaKey && e.key === "Tab") {
        e.preventDefault();
        e.stopPropagation();
        openSwitcher(e.shiftKey);
        return;
      }
      if (is("keymap")) {
        e.preventDefault();
        setKeymapOpen(true);
        return;
      }
      if (is("settings")) {
        e.preventDefault();
        setSettingsOpen("appearance");
        return;
      }
      if (is("theme")) {
        e.preventDefault();
        setSchemeOpen(true);
        return;
      }
      if (is("recent")) {
        e.preventDefault();
        e.stopPropagation();
        setRecentOpen(true);
        return;
      }
      if ((is("tab.next") || is("tab.prev")) && active) {
        e.preventDefault();
        const g = tabs.filter((t) => t.group === active.group);
        const i = g.findIndex((t) => t.id === active.id);
        const n = g[(i + (is("tab.prev") ? -1 : 1) + g.length) % g.length];
        if (n) activate(n);
        return;
      }
      if (is("tab.reopen")) {
        e.preventDefault();
        reopenClosed();
        return;
      }
      if (is("maximize")) {
        e.preventDefault();
        setMaximized((m) => !m);
        return;
      }
      // 분할된 에디터 그룹 사이 이동 (Goto Next Splitter)
      if (is("split.other") && keyState.current.split) {
        e.preventDefault();
        const g: Group = keyState.current.focused === 0 ? 1 : 0;
        const id = activeIdsRef.current[g];
        const t = tabs.find((x) => x.id === id);
        if (t) activate(t);
        return;
      }
      // F4 / Enter 를 입력칸에서 누른 건 무시
      if (inInput) return;
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [conns]);

  // ───────── 기타 ─────────
  const setColor = async (c: ConnectionView, color: string | null) => {
    const { hasPassword: _h, connected: _c, ...config } = c;
    await api.saveConnection({ ...config, color });
    refresh();
  };

  const setScope = async (c: ConnectionView, scope: NonNullable<ConnectionView["scope"]>) => {
    const { hasPassword: _h, connected: _c, ...config } = c;
    await api.saveConnection({ ...config, scope });
    refresh();
  };

  const exportConns = async () => {
    const path = await save({ defaultPath: "dbtool-datasources.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    try {
      const n = await api.exportConnections(path);
      setStatus(`Exported ${n} data sources (passwords excluded) to ${path}`);
    } catch (e) {
      setStatus(String(e));
    }
  };

  const importConns = async () => {
    const path = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    try {
      const r = await api.importConnections(path);
      setStatus(`Imported: ${r.added} added, ${r.updated} updated (re-enter passwords)`);
      refresh();
    } catch (e) {
      setStatus(String(e));
    }
  };

  const exportCsv = async (tab: Tab | null) => {
    if (!tab?.lastSql) return;
    const path = await save({ defaultPath: `${tab.kind === "table" ? tab.title : "result"}.csv`, filters: [{ name: "CSV (Excel)", extensions: ["csv"] }] });
    if (!path) return;
    setStatus("Exporting CSV…");
    try {
      const n = await api.exportCsv(tab.target, tab.lastSql, path);
      setStatus(`Exported ${n} rows to ${path}`);
    } catch (e) {
      setStatus(String(e));
    }
  };

  const removeConn = async (c: ConnectionView) => {
    if (!confirm(`Delete data source '${c.name}'?`)) return;
    await api.deleteConnection(c.id);
    closeTabs(tabsRef.current.filter((t) => t.connId === c.id).map((t) => t.id));
    refresh();
  };

  // ───────── 에디터 그룹 렌더링 ─────────
  const renderGroup = (g: Group) => {
    const gTabs = tabs.filter((t) => t.group === g);
    const ga = groupActive(g);
    const gConn = conns.find((c) => c.id === ga?.connId);
    return (
      <div className={"panel editor-group" + (split && focused === g ? " focused" : "")} onMouseDownCapture={() => setFocused(g)}>
        <div
          className={"tabs" + (dropHint?.zone === "tabs" && dropHint.g === g && dropHint.beforeId === null ? " drop-end" : "")}
          onDragOver={(e) => {
            if (!dragId) return;
            e.preventDefault();
            setDropHint({ g, zone: "tabs", beforeId: null });
          }}
          onDrop={(e) => {
            e.preventDefault();
            if (dragId) dropTab(dragId, g, null);
            endDrag();
          }}
        >
          {gTabs.map((t) => {
            const color = conns.find((c) => c.id === t.connId)?.color;
            return (
              <div
                key={t.id}
                className={
                  "tab" +
                  (t.id === ga?.id ? " active" : "") +
                  (t.id === dragId ? " dragging" : "") +
                  (dropHint?.zone === "tabs" && dropHint.g === g && dropHint.beforeId === t.id ? " drop-before" : "")
                }
                style={color ? { background: connBg(color, theme) } : undefined}
                draggable
                onDragStart={(e) => {
                  e.dataTransfer.setData("text/plain", t.id);
                  e.dataTransfer.effectAllowed = "move";
                  setDragId(t.id);
                }}
                onDragEnd={endDrag}
                onDragOver={(e) => {
                  if (!dragId) return;
                  e.preventDefault();
                  e.stopPropagation();
                  const r = e.currentTarget.getBoundingClientRect();
                  const i = gTabs.findIndex((x) => x.id === t.id);
                  const beforeId = e.clientX < r.left + r.width / 2 ? t.id : gTabs[i + 1]?.id ?? null;
                  setDropHint({ g, zone: "tabs", beforeId });
                }}
                onDrop={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  if (dragId && dropHint) dropTab(dragId, g, dropHint.beforeId);
                  endDrag();
                }}
                onMouseDown={(e) => {
                  if (e.button === 1) closeTabs([t.id]);
                  else activate(t);
                }}
                onContextMenu={(e) => openTabMenu(e, t)}
                onDoubleClick={() => setMaximized((m) => !m)}
                title={t.kind === "table" ? `${t.edit?.schema}.${t.edit?.table}` : t.title}
              >
                <span className={"tab-icon " + t.kind}>{t.kind === "table" ? "▦" : t.kind === "ddl" ? "ƒ" : t.kind === "redis" ? "◆" : "⌘"}</span>
                <span>{t.title}</span>
                {t.pinned ? (
                  <span
                    className="tab-pin"
                    title="Unpin Tab"
                    onMouseDown={(e) => e.stopPropagation()}
                    onClick={(e) => {
                      e.stopPropagation();
                      togglePin(t);
                    }}
                  >
                    📌
                  </span>
                ) : (
                  <span
                    className="tab-close"
                    title={`Close. Alt-Click to Close Others`}
                    onMouseDown={(e) => e.stopPropagation()}
                    onClick={(e) => {
                      e.stopPropagation();
                      // ⌥+클릭: 이 탭만 남기고 닫기
                      if (e.altKey) closeTabs(gTabs.filter((x) => x.id !== t.id && !x.pinned).map((x) => x.id));
                      else closeTabs([t.id]);
                    }}
                  >
                    ×
                  </span>
                )}
              </div>
            );
          })}
        </div>
        <div className="work-area" style={gConn?.color ? { boxShadow: `inset 0 2px 0 ${gConn.color}` } : undefined}>
          {/* 탭을 끌고 있을 때: 가운데 = 이 그룹으로, 오른쪽/아래 가장자리 = 분할 */}
          {dragId && (
            <div
              className="drop-overlay"
              onDragOver={(e) => {
                e.preventDefault();
                const r = e.currentTarget.getBoundingClientRect();
                const fx = (e.clientX - r.left) / r.width;
                const fy = (e.clientY - r.top) / r.height;
                const zone = !split && fx > 0.7 ? "right" : !split && fy > 0.7 ? "down" : "center";
                setDropHint({ g, zone, beforeId: null });
              }}
              onDragLeave={() => setDropHint((h) => (h && h.g === g && h.zone !== "tabs" ? null : h))}
              onDrop={(e) => {
                e.preventDefault();
                const h = dropHint;
                if (h && h.g === g) {
                  if (h.zone === "right" || h.zone === "down") {
                    if (gTabs.length > 1 || tabs.some((x) => x.group !== g)) dropTab(dragId, 1, null, h.zone);
                  } else dropTab(dragId, g, null);
                }
                endDrag();
              }}
            >
              {dropHint?.g === g && dropHint.zone !== "tabs" && <div className={"drop-zone " + dropHint.zone} />}
            </div>
          )}
          {/* SQL 콘솔 (DataGrip: 콘솔 툴바 + 에디터 + 결과 탭/Output) */}
          {!gTabs.length && (
            <div className="empty center">
              Right-click a data source → <b>New Query Console</b> ({isMac ? "⇧⌘L" : "Ctrl+Shift+L"}).
              <br />
              <span className="muted">Double-click a table ({isMac ? "⌘↓" : "F4"}) to edit data, {isMac ? "⌘B" : "Ctrl+B"} for DDL, {isMac ? "⌘↩" : "Ctrl+Enter"} to execute.</span>
              <br />
              <span className="muted">All shortcuts: <b>Keymap Reference</b> ({sk("keymap")}) · Search Everywhere ({isMac ? "⇧⇧" : "Shift Shift"})</span>
            </div>
          )}
          {gTabs
            .filter((t) => t.kind === "console")
            .map((t) => {
              const c = conns.find((x) => x.id === t.connId);
              return (
                <div key={t.id} className="work-layer" style={{ display: t.id === ga?.id ? "block" : "none" }}>
                  <ConsolePane
                    tabId={t.id}
                    target={t.target}
                    initialSql={sqlOf(t)}
                    connName={c?.name ?? ""}
                    database={t.database || c?.database || (c?.kind === "mysql" ? "" : "postgres")}
                    schema={t.schema ?? (c?.kind === "mysql" ? t.database || c?.database || "" : "public")}
                    dialect={dialectOf(c?.kind)}
                    dbList={dbCache[t.connId] ?? []}
                    schemaList={schemaCache[t.target] ?? []}
                    running={t.running}
                    loadingMore={t.loadingMore}
                    results={t.results}
                    activeResult={t.activeResult}
                    output={t.output}
                    txManual={t.txManual}
                    inTx={t.inTx}
                    onChange={(sql) => liveSql.current.set(t.id, sql)}
                    onExecute={(stmts, explain) => executeConsole(t.id, stmts, explain)}
                    onCancel={() => cancel(t)}
                    onSetActiveResult={(id) => patch(t.id, { activeResult: id })}
                    onPin={(id, pinned) => setTabs((ts) => ts.map((x) => (x.id === t.id ? { ...x, results: x.results.map((r) => (r.id === id ? { ...r, pinned } : r)) } : x)))}
                    onCloseResult={(id) =>
                      setTabs((ts) => ts.map((x) => (x.id === t.id ? { ...x, results: x.results.filter((r) => r.id !== id), activeResult: x.activeResult === id ? "output" : x.activeResult } : x)))
                    }
                    onFetchMore={(entryId) => fetchMoreEntry(t.id, entryId)}
                    onExport={(sql) => exportCsv({ ...t, lastSql: sql })}
                    onHistory={() => setHistoryFor(t.id)}
                    onTxMode={(manual) => setTxMode(t, manual)}
                    onCommit={() => txEnd(t, "commit")}
                    onRollback={() => txEnd(t, "rollback")}
                    onLoadDbs={() => loadDbList(t.connId)}
                    onLoadSchemas={() => loadSchemaList(t.target)}
                    onSwitchDb={(db) => switchDatabase(t, db)}
                    onSwitchSchema={(sc) => switchSchema(t, sc)}
                    schemaSwitchDisabled={conns.find((x) => x.id === t.connId)?.options?.switchSchema === "disable"}
                    registerApi={(id, a) => editorApis.current.set(id, a)}
                    onClearOutput={() => patch(t.id, { output: [] })}
                  />
                </div>
              );
            })}
          {/* Redis 브라우저 */}
          {gTabs
            .filter((t) => t.kind === "redis")
            .map((t) => {
              const c = conns.find((x) => x.id === t.connId);
              return (
                <div key={t.id} className="work-layer" style={{ display: t.id === ga?.id ? "block" : "none" }}>
                  <RedisBrowser
                    target={t.target}
                    connName={c?.name ?? ""}
                    database={t.database ?? "db0"}
                    dbList={dbCache[t.connId] ?? []}
                    readOnly={!!c?.readOnly}
                    initialKey={t.redisKey}
                    onLoadDbs={() => loadDbList(t.connId)}
                    onSwitchDb={(db) => c && patch(t.id, { database: db, target: targetId(c.id, db), title: `${c.name} [${db}]`, redisKey: undefined })}
                    onStatus={setStatus}
                  />
                </div>
              );
            })}
          {/* 테이블 데이터 에디터 */}
          {gTabs
            .filter((t) => t.kind === "table")
            .map((t) => (
              <div key={t.id} className="work-layer" style={{ display: t.id === ga?.id ? "block" : "none" }}>
                <TableEditor
                  title={t.title}
                  where={t.where}
                  orderBy={t.orderBy}
                  running={t.running}
                  error={t.error}
                  result={t.result}
                  query={t.lastSql}
                  edit={t.edit && t.lastSql === t.tableSql ? t.edit : undefined}
                  readOnly={!!conns.find((x) => x.id === t.connId)?.readOnly}
                  dialect={dialectOf(conns.find((x) => x.id === t.connId)?.kind)}
                  loadingMore={t.loadingMore}
                  onApply={(w, o) => applyTableFilter(t, w, o)}
                  onCancel={() => cancel(t)}
                  onFetchMore={() => fetchMore(t)}
                  onSubmit={(st) => submitEdits(t, st)}
                  onExportCsv={() => exportCsv(t)}
                  onDdl={() => {
                    const c = conns.find((x) => x.id === t.connId);
                    if (c && t.edit) openDdl({ kind: "table", conn: c, db: t.database, schema: t.edit.schema, name: t.edit.table });
                  }}
                  onCopyQueryToConsole={(sql) => { const c = conns.find((x) => x.id === t.connId); if (c) openConsole(c, t.database, sql); }}
                />
              </div>
            ))}
          {/* DDL (읽기 전용) */}
          {gTabs
            .filter((t) => t.kind === "ddl")
            .map((t) => (
              <div key={t.id} className="work-layer ddl-layer" style={{ display: t.id === ga?.id ? "flex" : "none" }}>
                <div className="dg-toolbar">
                  <span className="tb-info" style={{ marginLeft: 0 }}>{t.title.startsWith("SQL Generator") ? "SQL Generator" : "DDL"} · read-only</span>
                  <span className="grow" />
                  <button className="tb text" title="Copy to Clipboard" onClick={() => { navigator.clipboard.writeText(t.sql); setStatus("Copied to clipboard"); }}>Copy</button>
                  <button
                    className="tb text"
                    title="Save to File"
                    onClick={async () => {
                      const path = await save({ defaultPath: `${t.title.replace(/^SQL Generator: | \[DDL\]$/g, "")}.sql`, filters: [{ name: "SQL", extensions: ["sql"] }] });
                      if (!path) return;
                      try {
                        await api.writeTextFile(path, t.sql);
                        setStatus(`Saved ${path}`);
                      } catch (e) {
                        setStatus(String(e));
                      }
                    }}
                  >
                    Save to File
                  </button>
                  <button className="tb text" title="Run in a console" onClick={() => { const c = conns.find((x) => x.id === t.connId); if (c) openConsole(c, t.database, t.sql); }}>Run in Console</button>
                </div>
                <div className="ddl-editor">
                <ConsoleEditor
                  tabId={t.id}
                  connId={t.target}
                  initialSql={t.sql}
                  language={dialectOf(conns.find((x) => x.id === t.connId)?.kind).monaco}
                  readOnly
                  onChange={() => {}}
                  onRun={() => {}}
                  registerApi={() => {}}
                />
                </div>
              </div>
            ))}
        </div>
      </div>
    );
  };

  // Search Everywhere 의 파일·액션 목록은 열 때 한 번 만든다 (열려 있는 동안 App 이 다시 그려질 때마다 새로 만들면 안쪽 memo 가 매번 깨진다)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const seLists = useMemo(() => (se ? { files: seFiles(), actions: seActions() } : null), [se]);

  return (
    <div className="app">
      <div className="toolbar" style={activeConn?.color ? { boxShadow: `inset 4px 0 0 ${activeConn.color}` } : undefined}>
        <span className="brand">dbtool</span>
        <span className="sep" />
        <button
          className="run"
          disabled={!active || active.kind !== "console" || active.running}
          onClick={() => active && editorApis.current.get(active.id)?.run()}
          title={`Execute (${isMac ? "⌘↩" : "Ctrl+Enter"})`}
        >
          ▶
        </button>
        <button className="stop" disabled={!active?.running} onClick={() => active && cancel(active)} title="Cancel Running Statements">
          ■
        </button>
        <button className="icon tb-settings" title={`Settings (${sk("settings")})`} onClick={() => setSettingsOpen("appearance")}>⚙</button>
        <button className="icon tb-settings" title={`Keymap Reference (${isMac ? "⇧⌘/" : "Ctrl+Shift+/"})`} onClick={() => setKeymapOpen(true)}>⌨</button>
        <span className="tb-conn">
          {activeConn ? `${activeConn.name} · ${activeConn.user}@${activeConn.host} · ${active?.database || activeConn.database || "postgres"}` : "No console"}
        </span>
      </div>

      <PanelGroup direction="vertical" className="main" autoSaveId="dbtool-v">
      <Panel id="top" order={1} minSize={20}>
      <PanelGroup direction="horizontal" autoSaveId="dbtool-h">
        {!maximized && !explorerHidden && (
        <Panel id="explorer" order={1} defaultSize={20} minSize={12}>
          <Explorer
            connections={conns}
            onAdd={() => setDialog({})}
            onRefresh={refresh}
            onExport={exportConns}
            onImport={importConns}
            onSelect={setTreeSel}
            onActivate={activateNode}
            buildMenu={buildMenu}
            onSetColor={setColor}
            onScopeChange={setScope}
          />
        </Panel>
        )}
        {!maximized && !explorerHidden && <PanelResizeHandle className="resize-h" />}
        <Panel id="editors" order={2}>
          {split ? (
            <PanelGroup direction={split === "right" ? "horizontal" : "vertical"} autoSaveId={`dbtool-split-${split}`}>
              <Panel minSize={15}>{renderGroup(0)}</Panel>
              <PanelResizeHandle className={split === "right" ? "resize-h" : "resize-v"} />
              <Panel minSize={15}>{renderGroup(1)}</Panel>
            </PanelGroup>
          ) : (
            renderGroup(0)
          )}
        </Panel>
      </PanelGroup>
      </Panel>
      {findOpen && !maximized && (
        <>
          <PanelResizeHandle className="resize-v" />
          <Panel id="find" order={2} defaultSize={30} minSize={10}>
            <FindPanel
              runs={findRuns}
              active={findActive}
              onActive={setFindActive}
              onClose={() => setFindOpen(false)}
              onCloseRun={(id) => setFindRuns((rs) => rs.filter((r) => r.id !== id))}
              onPin={(id) => setFindRuns((rs) => rs.map((r) => (r.id === id ? { ...r, pinned: !r.pinned } : r)))}
              onOpen={(run, hit) => {
                const c = conns.find((x) => x.id === run.scope.connId);
                if (c) openTableTab(c, hit.schema, hit.table, run.scope.db, undefined, hit.whereClause);
              }}
            />
          </Panel>
        </>
      )}
      </PanelGroup>

      {switcher && (
        <Switcher
          items={switcher.items}
          start={switcher.start}
          onClose={() => setSwitcher(null)}
          onPick={(id) => {
            setSwitcher(null);
            const t = tabsRef.current.find((x) => x.id === id);
            if (t) activate(t);
          }}
        />
      )}
      {schemeOpen && (
        <RecentFiles
          title="Theme"
          start={["dark", "light", "hc", "system"].indexOf(getThemePref())}
          items={[
            { key: "dark", icon: getThemePref() === "dark" ? "●" : "", title: "Dark", hint: "" },
            { key: "light", icon: getThemePref() === "light" ? "●" : "", title: "Light", hint: "" },
            { key: "hc", icon: getThemePref() === "hc" ? "●" : "", title: "High contrast", hint: "" },
            { key: "system", icon: getThemePref() === "system" ? "●" : "", title: "Sync with OS", hint: `currently ${THEME_LABEL[theme]}` },
          ]}
          onClose={() => setSchemeOpen(false)}
          onPick={(k) => {
            setSchemeOpen(false);
            setThemePref(k as ThemePref);
          }}
        />
      )}
      {settingsOpen && <SettingsDialog initialPage={settingsOpen} onClose={() => setSettingsOpen(false)} />}
      {keymapOpen && <KeymapDialog onClose={() => setKeymapOpen(false)} onEdit={() => (setKeymapOpen(false), setSettingsOpen("keymap"))} />}
      {se && (
        <SearchEverywhere
          initialTab={se}
          conns={conns}
          contextConnId={treeSel?.conn.id ?? active?.connId}
          files={seLists?.files ?? []}
          actions={seLists?.actions ?? []}
          onClose={() => setSe(null)}
          onSelectInTree={(o) => {
            setExplorerHidden(false);
            setMaximized(false);
            revealInTree({ connId: o.connId, db: o.db, schema: o.schema, name: o.kind === "schema" ? undefined : o.name, kind: o.kind });
          }}
          onOpenData={(o) => {
            const n = objNode(o);
            if (n) activateNode(n);
          }}
          onDdl={(o) => {
            const n = objNode(o);
            if (n) openDdl(n);
          }}
          onQuickDoc={(o) => {
            const n = objNode(o);
            if (n) showQuickDoc(n);
          }}
          onOpenFile={(key) => {
            const snap = recent.current.find((r) => r.key === key);
            const open = tabsRef.current.find((t) => snapKey(t) === key);
            if (open) activate(open);
            else if (snap) reopen(snap);
          }}
        />
      )}
      {ftsScope && <FullTextSearchDialog scope={ftsScope} onClose={() => setFtsScope(null)} onSearch={(req) => runFts(ftsScope, req)} />}
      {recentOpen && (
        <RecentFiles
          items={recentItems()}
          onClose={() => setRecentOpen(false)}
          onPick={(key) => {
            setRecentOpen(false);
            const snap = recent.current.find((r) => r.key === key);
            if (snap) reopen(snap);
          }}
        />
      )}
      <StatusBar status={status} readOnly={!!activeConn?.readOnly} />

      {tabMenu && <ContextMenu menu={tabMenu} onClose={() => setTabMenu(null)} />}

      {historyFor && (() => {
        const t = tabs.find((x) => x.id === historyFor);
        const c = conns.find((x) => x.id === t?.connId);
        return t && c ? (
          <HistoryDialog
            connId={c.id}
            title={`${c.name} Query History`}
            onPick={(sql) => {
              editorApis.current.get(t.id)?.insert(sql);
              setHistoryFor(null);
            }}
            onClose={() => setHistoryFor(null)}
          />
        ) : null;
      })()}

      {quickDoc && (
        <div className="modal-backdrop light" onMouseDown={() => setQuickDoc(null)} onKeyDown={(e) => e.key === "Escape" && setQuickDoc(null)}>
          <div className="quick-doc" onMouseDown={(e) => e.stopPropagation()} tabIndex={-1} ref={(el) => el?.focus()} onKeyDown={(e) => e.key === "Escape" && setQuickDoc(null)}>
            <div className="qd-title">{quickDoc.title}</div>
            <pre className="qd-body">{quickDoc.text}</pre>
          </div>
        </div>
      )}

      {dialog && (
        <ConnectionDialog
          edit={dialog.edit}
          onClose={() => setDialog(null)}
          onSaved={async (saved) => {
            setDialog(null);
            await refresh();
            try {
              setStatus(`Connecting to ${saved.name}…`);
              const v = await api.connect(saved.id);
              setStatus(`Connected: ${v.split(" on ")[0]}`);
            } catch (e) {
              setStatus(`Connection failed: ${String(e)}`);
            }
            refresh();
          }}
        />
      )}
    </div>
  );
}

/** Settings › Appearance */
function SettingsDialog({ onClose, initialPage = "appearance" }: { onClose: () => void; initialPage?: "appearance" | "keymap" }) {
  const [page, setPage] = useState(initialPage);
  const [pref, setPref] = useState<ThemePref>(getThemePref());
  const theme = useTheme();
  const pick = (p: ThemePref) => {
    setPref(p);
    setThemePref(p);
  };
  const sync = pref === "system";
  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div className={"modal settings" + (page === "keymap" ? " wide" : "")} onMouseDown={(e) => e.stopPropagation()}>
        <div className="modal-title">Settings</div>
        <div className="settings-body">
          <div className="settings-nav">
            <div className={page === "appearance" ? "on" : ""} onClick={() => setPage("appearance")}>Appearance</div>
            <div className={page === "keymap" ? "on" : ""} onClick={() => setPage("keymap")}>Keymap</div>
          </div>
          {page === "keymap" && <KeymapEditor />}
          {page === "appearance" && <div className="form settings-form">
            <label>Theme</label>
            <div className="row">
              <select value={sync ? theme : pref} disabled={sync} onChange={(e) => pick(e.target.value as ThemePref)}>
                <option value="dark">Dark</option>
                <option value="light">Light</option>
                <option value="hc">High contrast</option>
              </select>
              <label className="check">
                <input type="checkbox" checked={sync} onChange={(e) => pick(e.target.checked ? "system" : theme)} />
                Sync with OS
              </label>
            </div>
          </div>}
        </div>
        {page === "appearance" && <div className="hint">Quick switch: {sk("theme")}</div>}
        <div className="modal-actions">
          <span className="grow" />
          <button className="primary" onClick={onClose}>OK</button>
        </div>
      </div>
    </div>
  );
}

function StatusBar({ status, readOnly }: { status: string; readOnly: boolean }) {
  const w = useStatusWidgets();
  return (
    <div className="statusbar">
      <span className="sb-text">{status}</span>
      <span className="grow" />
      {w.aggregate && <span className="sb-widget" title="Aggregate of selected cells">{w.aggregate}</span>}
      {readOnly && <span className="sb-widget" title="Data source is read-only">🔒 Read-only</span>}
      {w.caret && <span className="sb-widget" title="Line:Column">{w.caret}</span>}
      <span className="sb-widget">UTF-8</span>
    </div>
  );
}
