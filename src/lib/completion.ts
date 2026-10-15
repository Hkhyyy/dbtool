import { monaco } from "./monaco";
import { CompletionTable, meta } from "./api";

/** 콘솔(모델 URI) → 접속 id, 접속 id → 메타 캐시 */
const modelConn = new Map<string, string>();
const cache = new Map<string, Promise<CompletionTable[]>>();

export function bindModel(uri: string, connId: string) {
  modelConn.set(uri, connId);
  if (!cache.has(connId)) cache.set(connId, meta.completion(connId).catch(() => []));
}
export function invalidateCompletion(connId: string) {
  cache.delete(connId);
}

const KEYWORDS = (
  "select from where and or not in is null like ilike between exists group by order having limit offset join left right inner outer full cross on using as " +
  "distinct union all intersect except insert into values update set delete returning create table view index alter drop truncate with case when then else end " +
  "asc desc nulls first last count sum avg min max coalesce now current_date current_timestamp interval true false begin commit rollback explain analyze"
).split(" ");

function tablesInStatement(text: string) {
  const out: { ref: string; alias?: string }[] = [];
  const re = /\b(?:from|join|update|into)\s+((?:"[^"]+"|\w+)(?:\.(?:"[^"]+"|\w+))?)(?:\s+(?:as\s+)?(\w+))?/gi;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text))) {
    const alias = m[2] && !KEYWORDS.includes(m[2].toLowerCase()) ? m[2] : undefined;
    out.push({ ref: m[1].replace(/"/g, ""), alias });
  }
  return out;
}

function findTable(all: CompletionTable[], ref: string) {
  const [a, b] = ref.toLowerCase().split(".");
  return b ? all.find((t) => t.schema.toLowerCase() === a && t.name.toLowerCase() === b) : all.find((t) => t.name.toLowerCase() === a && t.schema === "public") ?? all.find((t) => t.name.toLowerCase() === a);
}

for (const lang of ["pgsql", "mysql"]) {
  monaco.languages.registerCompletionItemProvider(lang, {
    triggerCharacters: ["."],
    async provideCompletionItems(model, position) {
      const connId = modelConn.get(model.uri.toString());
      const all = connId ? await (cache.get(connId) ?? Promise.resolve([])) : [];
      const K = monaco.languages.CompletionItemKind;
      const word = model.getWordUntilPosition(position);
      const range = new monaco.Range(position.lineNumber, word.startColumn, position.lineNumber, word.endColumn);
      const line = model.getLineContent(position.lineNumber).slice(0, word.startColumn - 1);
      const text = model.getValue();
      const dot = /((?:"[^"]+"|\w+))\.$/.exec(line);

      if (dot) {
        const q = dot[1].replace(/"/g, "");
        // alias. 또는 table. → 컬럼
        const refs = tablesInStatement(text);
        const hit = refs.find((r) => r.alias?.toLowerCase() === q.toLowerCase()) ?? refs.find((r) => r.ref.split(".").pop()!.toLowerCase() === q.toLowerCase());
        const t = hit ? findTable(all, hit.ref) : undefined;
        if (t) {
          return { suggestions: t.columns.map(([c, ty], i) => ({ label: { label: c, description: ty }, kind: K.Field, insertText: c, range, sortText: String(i).padStart(4, "0") })) };
        }
        // schema. → 테이블
        const inSchema = all.filter((x) => x.schema.toLowerCase() === q.toLowerCase());
        return { suggestions: inSchema.map((x) => ({ label: { label: x.name, description: `${x.columns.length} cols` }, kind: K.Class, insertText: x.name, range })) };
      }

      const referenced = tablesInStatement(text).map((r) => findTable(all, r.ref)).filter(Boolean) as CompletionTable[];
      const cols = new Map<string, string>();
      for (const t of referenced) for (const [c, ty] of t.columns) if (!cols.has(c)) cols.set(c, `${ty} · ${t.name}`);
      const schemas = [...new Set(all.map((t) => t.schema))];
      const suggestions: monaco.languages.CompletionItem[] = [
        ...[...cols].map(([c, d]) => ({ label: { label: c, description: d }, kind: K.Field, insertText: c, range, sortText: "0" + c })),
        ...all.map((t) => ({
          label: { label: t.name, description: t.schema },
          kind: K.Class,
          insertText: t.schema === "public" ? t.name : `${t.schema}.${t.name}`,
          filterText: t.name,
          range,
          sortText: "1" + t.name,
        })),
        ...schemas.map((s) => ({ label: s, kind: K.Module, insertText: s, range, sortText: "2" + s })),
        ...KEYWORDS.map((k) => ({ label: k, kind: K.Keyword, insertText: k, range, sortText: "3" + k })),
      ];
      return { suggestions };
    },
  });
}
