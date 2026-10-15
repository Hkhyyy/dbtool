import { invoke } from "@tauri-apps/api/core";

export type DbKind = "postgres" | "mysql" | "redis";

export interface ConnectionConfig {
  id: string;
  name: string;
  kind: DbKind;
  host: string;
  port: number;
  user: string;
  database: string;
  color?: string | null;
  ssh?: SshConfig | null;
  /** 트리에 보일 DB·스키마. databases 가 없으면 전부, schemas[db] 가 없으면 그 DB의 스키마 전부 */
  scope?: { databases?: string[] | null; schemas?: Record<string, string[]> };
  /** Options → Read-only */
  readOnly?: boolean;
  /** Options → 나머지 Connection 설정 */
  options?: ConnOptions;
  /** SSH/SSL 탭의 SSL */
  ssl?: SslConfig | null;
  /** Redis 연결 방식 */
  redis?: RedisMode | null;
}

export interface SslConfig {
  enabled: boolean;
  /** require | verify-ca | verify-full */
  mode: string;
  caFile: string;
  certFile: string;
  keyFile: string;
}
export interface RedisMode {
  mode: "standalone" | "sentinel" | "cluster";
  sentinelMaster: string;
  /** host:port, 쉼표 구분 */
  nodes: string;
}

export interface ConnOptions {
  txManual?: boolean;
  isolation?: string | null;
  switchSchema?: "auto" | "manual" | "disable" | null;
  timeZone?: string | null;
  keepAliveSecs?: number | null;
  autoDisconnectSecs?: number | null;
  singleDb?: boolean;
  startupScript?: string;
}

export interface SshConfig {
  enabled: boolean;
  host: string;
  port: number;
  user: string;
  auth: "password" | "key";
  keyPath: string;
}

export interface ConnectionView extends ConnectionConfig {
  hasPassword: boolean;
  connected: boolean;
}

export interface ColumnInfo {
  name: string;
  typeName: string;
}

export type Cell = string | number | boolean | null;

export interface QueryResult {
  columns: ColumnInfo[];
  rows: Cell[][];
  rowsAffected: number;
  truncated: boolean;
  elapsedMs: number;
  hasResultSet: boolean;
  inTransaction?: boolean;
}

/** password: undefined → 기존 유지, "" → 삭제 */
export const api = {
  listConnections: () => invoke<ConnectionView[]>("list_connections"),
  saveConnection: (config: ConnectionConfig, password?: string, sshSecret?: string) =>
    invoke<ConnectionConfig>("save_connection", { config, password: password ?? null, sshSecret: sshSecret ?? null }),
  deleteConnection: (id: string) => invoke<void>("delete_connection", { id }),
  testConnection: (config: ConnectionConfig, password?: string, sshSecret?: string) =>
    invoke<string>("test_connection", { config, password: password ?? null, sshSecret: sshSecret ?? null }),
  testSsh: (config: ConnectionConfig, sshSecret?: string) => invoke<string>("test_ssh", { config, sshSecret: sshSecret ?? null }),
  connect: (id: string) => invoke<string>("connect", { id }),
  disconnect: (id: string) => invoke<void>("disconnect", { id }),
  runQuery: (id: string, sql: string, tabId?: string, limit = 500) => invoke<QueryResult>("run_query", { id, sql, limit, tabId: tabId ?? null }),
  fetchMore: (tabId: string, limit = 500) => invoke<QueryResult>("fetch_more", { tabId, limit }),
  cancelQuery: (tabId: string) => invoke<boolean>("cancel_query", { tabId }),
  setTxMode: (id: string, tabId: string, manual: boolean) => invoke<void>("set_tx_mode", { id, tabId, manual }),
  commitTx: (tabId: string) => invoke<void>("commit_tx", { tabId }),
  rollbackTx: (tabId: string) => invoke<void>("rollback_tx", { tabId }),
  closeSession: (tabId: string) => invoke<void>("close_session", { tabId }),
  executeBatch: (id: string, statements: string[]) => invoke<number>("execute_batch", { id, statements }),
  exportConnections: (path: string) => invoke<number>("export_connections", { path }),
  importConnections: (path: string) => invoke<{ added: number; updated: number }>("import_connections", { path }),
  writeTextFile: (path: string, text: string) => invoke<void>("write_text_file", { path, text }),
  exportCsv: (id: string, sql: string, path: string) => invoke<number>("export_csv", { id, sql, path }),
};

export interface SchemaInfo { name: string; isSystem: boolean }
export interface RelationInfo { name: string; kind: "table" | "partitioned" | "view" | "matview" | "foreign" | "sequence" | "key"; comment: string | null; rowEstimate: number | null }
export interface RoutineInfo { name: string; args: string; returns: string | null; kind: string }
export interface SchemaObjects { relations: RelationInfo[]; routines: RoutineInfo[] }
export interface ColumnMeta { name: string; dataType: string; nullable: boolean; defaultValue: string | null; comment: string | null; isPk: boolean }
export interface IndexMeta { name: string; definition: string; isUnique: boolean; isPrimary: boolean }
export interface ConstraintMeta { name: string; kind: "p" | "f" | "u" | "c"; definition: string }
export interface TableDetail { columns: ColumnMeta[]; indexes: IndexMeta[]; constraints: ConstraintMeta[] }
export interface CompletionTable { schema: string; name: string; columns: [string, string][] }

export const meta = {
  databases: (id: string) => invoke<string[]>("list_databases", { id }),
  schemas: (id: string) => invoke<SchemaInfo[]>("list_schemas", { id }),
  objects: (id: string, schema: string) => invoke<SchemaObjects>("list_objects", { id, schema }),
  table: (id: string, schema: string, table: string) => invoke<TableDetail>("table_detail", { id, schema, table }),
  completion: (id: string) => invoke<CompletionTable[]>("completion_meta", { id }),
  ddl: (id: string, schema: string, name: string, kind: string) => invoke<string>("object_ddl", { id, schema, name, kind }),
  objectIndex: (id: string) => invoke<IndexEntry[]>("object_index", { id }),
  fullTextSearch: (id: string, req: FtsRequest) => invoke<FtsHit[]>("full_text_search", { id, req }),
};
/** Redis 기본 DB 이름: "" / "3" / "db3" → "db3" */
export const redisDb = (database?: string | null) => `db${String(database ?? "").replace(/^db/, "").trim() || "0"}`;
export interface RedisKeyRow { key: string; type: string; ttl: number }
export interface RedisKeyDetail { type: string; ttl: number; length: number; memory: number | null; encoding: string | null; value: unknown; truncated: boolean }
export type RedisReply = null | number | string | boolean | { status: string } | RedisReply[];
export const redis = {
  scan: (id: string, cursor: string, pattern: string, count: number, typeFilter?: string) =>
    invoke<{ cursor: string; keys: RedisKeyRow[] }>("redis_scan", { id, cursor, pattern, count, typeFilter: typeFilter || null }),
  key: (id: string, key: string, limit?: number) => invoke<RedisKeyDetail>("redis_key", { id, key, limit: limit ?? null }),
  exec: (id: string, args: string[]) => invoke<RedisReply>("redis_exec", { id, args }),
  execLine: (id: string, line: string) => invoke<RedisReply>("redis_exec_line", { id, line }),
};
export interface IndexEntry { schema: string; name: string; kind: string }
export interface FtsRequest {
  text: string;
  mode: "contains" | "starts" | "ends" | "matches" | "like";
  matchCase: boolean;
  columns: "text" | "numeric" | "all";
  schemas: string[];
  tables: [string, string][];
  limit: number;
}
export interface FtsHit { schema: string; table: string; whereClause: string; result: QueryResult }

export const quoteIdent = (s: string) => (/^[a-z_][a-z0-9_]*$/.test(s) ? s : `"${s.replace(/"/g, '""')}"`);

/** DataGrip처럼 접속을 구분하는 색 (어두운 배경용) */
export const COLOR_PRESETS: { name: string; value: string | null }[] = [
  { name: "None", value: null },
  { name: "Red", value: "#6e2a2a" },
  { name: "Orange", value: "#6e4a22" },
  { name: "Yellow", value: "#5f5b24" },
  { name: "Green", value: "#2f5634" },
  { name: "Teal", value: "#22524f" },
  { name: "Blue", value: "#28406e" },
  { name: "Violet", value: "#4f2f68" },
  { name: "Gray", value: "#45474d" },
];

export const DEFAULT_PORT: Record<DbKind, number> = { postgres: 5432, mysql: 3306, redis: 6379 };
export const KIND_LABEL: Record<DbKind, string> = { postgres: "PostgreSQL", mysql: "MySQL", redis: "Redis" };

/** 데이터 편집 SQL에 쓰는 리터럴. PG가 컬럼 타입으로 캐스팅한다. */
export const sqlLiteral = (v: Cell) => (v === null ? "NULL" : `'${String(v).replace(/'/g, "''")}'`);

/** DB 종류별 SQL 방언: 식별자 따옴표, 문자열 리터럴, 텍스트 캐스트 */
export interface Dialect {
  q: (s: string) => string;
  lit: (v: Cell) => string;
  text: (expr: string) => string;
  monaco: string;
  /** 콘솔 문장 나누기 (없으면 세미콜론 기준) */
  split?: (text: string) => { start: number; end: number }[];
}
const PG: Dialect = { q: quoteIdent, lit: sqlLiteral, text: (e) => `${e}::text`, monaco: "pgsql" };
const MY: Dialect = {
  q: (s) => (/^[A-Za-z_][A-Za-z0-9_]*$/.test(s) ? s : "`" + s.replace(/`/g, "``") + "`"),
  // MySQL 문자열에서는 백슬래시도 이스케이프 문자
  lit: (v) => (v === null ? "NULL" : `'${String(v).replace(/\\/g, "\\\\").replace(/'/g, "''")}'`),
  text: (e) => `CAST(${e} AS CHAR)`,
  monaco: "mysql",
};
/** Redis: 한 줄 = 명령 하나 (# 주석) */
const RD: Dialect = {
  q: (s) => s,
  lit: (v) => (v === null ? '""' : JSON.stringify(String(v))),
  text: (e) => e,
  monaco: "redis",
  split: (text) => {
    const out: { start: number; end: number }[] = [];
    let pos = 0;
    for (const line of text.split("\n")) {
      const t = line.trim();
      if (t && !t.startsWith("#") && !t.startsWith("//")) out.push({ start: pos, end: pos + line.length });
      pos += line.length + 1;
    }
    return out;
  },
};
export const dialectOf = (kind?: DbKind) => (kind === "mysql" ? MY : kind === "redis" ? RD : PG);

/** 백엔드 접속 대상: Data Source id + DB 이름 */
export const targetId = (connId: string, database?: string | null) => (database ? `${connId}::${database}` : connId);

/** 스키마 선택(DataGrip "N of M" 팝업) 규칙. 특수값: "*" = 전부, "@default" = 기본 DB / 기본 스키마(public) */
export type Scope = NonNullable<ConnectionConfig["scope"]>;
export const SCOPE_ALL = "*";
export const SCOPE_DEFAULT = "@default";

export function dbVisible(scope: Scope | undefined, db: string, defaultDb: string) {
  const d = scope?.databases;
  if (!d || d.length === 0) return true;
  return d.includes(SCOPE_ALL) || d.includes(db) || (d.includes(SCOPE_DEFAULT) && db === defaultDb);
}

export function schemaVisible(scope: Scope | undefined, db: string, schema: string, isSystem: boolean, showSystem: boolean) {
  const s = scope?.schemas?.[db];
  if (!s || s.length === 0 || s.includes(SCOPE_ALL)) return showSystem || !isSystem;
  return s.includes(schema) || (s.includes(SCOPE_DEFAULT) && schema === "public");
}
