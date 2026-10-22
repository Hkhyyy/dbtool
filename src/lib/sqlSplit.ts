export interface Segment {
  start: number;
  end: number;
}

/** 세미콜론으로 문장을 나눈다. 문자열, 주석, PostgreSQL 달러 인용은 건너뛴다. */
export function splitStatements(text: string, backslashEscapes = false): Segment[] {
  const out: Segment[] = [];
  let start = 0;
  let i = 0;
  const n = text.length;
  while (i < n) {
    const c = text[i];
    const d = text[i + 1];
    if (c === "-" && d === "-") {
      const j = text.indexOf("\n", i);
      i = j < 0 ? n : j + 1;
      continue;
    }
    if (c === "/" && d === "*") {
      const j = text.indexOf("*/", i + 2);
      i = j < 0 ? n : j + 2;
      continue;
    }
    if (c === "'" || c === '"' || c === "`") {
      let j = i + 1;
      while (j < n) {
        if (backslashEscapes && text[j] === "\\") {
          j += 2;
          continue;
        }
        if (text[j] === c) {
          if (text[j + 1] === c) {
            j += 2;
            continue;
          }
          break;
        }
        j++;
      }
      i = j + 1;
      continue;
    }
    if (c === "$") {
      const m = /^\$[A-Za-z_][A-Za-z_0-9]*\$|^\$\$/.exec(text.slice(i, i + 64));
      if (m) {
        const j = text.indexOf(m[0], i + m[0].length);
        i = j < 0 ? n : j + m[0].length;
        continue;
      }
    }
    if (c === ";") {
      out.push({ start, end: i });
      start = i + 1;
    }
    i++;
  }
  out.push({ start, end: n });
  return out.filter((s) => text.slice(s.start, s.end).trim().length > 0);
}

/** 커서가 있는 문장. 커서가 문장 사이 빈 곳이면 바로 앞 문장을 고른다. */
export function statementAt(text: string, offset: number, backslashEscapes = false) {
  const segs = splitStatements(text, backslashEscapes);
  if (segs.length === 0) return null;
  let pick = segs.find((s) => offset >= s.start && offset <= s.end + 1);
  if (!pick) pick = [...segs].reverse().find((s) => s.end < offset) ?? segs[0];
  const raw = text.slice(pick.start, pick.end);
  const lead = raw.length - raw.trimStart().length;
  const sql = raw.trim();
  return { sql, start: pick.start + lead, end: pick.start + lead + sql.length };
}

/** 줄 머리에 오면 새 문장일 수 있는 키워드 (core sql.rs 의 SCRIPT_STARTERS 와 같다) */
const STARTERS = new Set(["SELECT", "INSERT", "UPDATE", "DELETE", "WITH", "CREATE", "ALTER", "DROP", "TRUNCATE", "GRANT", "REVOKE", "EXPLAIN", "SHOW", "MERGE", "CALL"]);
/** 문장 머리로 오는 키워드 (이게 아니면 앞은 SQL 이 아닌 메모) — SQL_HEADS */
const HEADS = new Set([
  ...STARTERS,
  ..."BEGIN START COMMIT ROLLBACK END ABORT SAVEPOINT RELEASE SET RESET VALUES TABLE DO COPY VACUUM ANALYZE REFRESH LOCK REINDEX CLUSTER COMMENT DECLARE FETCH CLOSE PREPARE EXECUTE DEALLOCATE LISTEN NOTIFY UNLISTEN USE DESC DESCRIBE REPLACE LOAD OPTIMIZE RENAME IMPORT DISCARD CHECKPOINT SECURITY".split(" "),
]);
/** 이 낱말 뒤의 줄은 앞 문장이 이어진다 — SCRIPT_CONT */
const CONT = new Set(
  "UNION ALL INTERSECT EXCEPT MINUS DISTINCT AS DO THEN ELSE WHEN AND OR NOT IN EXISTS ANY SOME BEFORE AFTER OF ON FOR KEY INSTEAD BY FROM JOIN WHERE TABLE INTO RETURNING GRANT REVOKE EXPLAIN ANALYZE VERBOSE IS LIKE BETWEEN WITH".split(" "),
);

/**
 * 편집기용 문장 나누기: 세미콜론으로 나눈 뒤 `;` 없이 이어 쓴 문장도 줄 머리의 문장 키워드로 나눈다 (DataGrip 처럼, core sql::split_script 와 같은 규칙).
 * UNION · INSERT … SELECT · WITH … SELECT · GRANT SELECT 등은 한 문장.
 */
export function splitScript(text: string, backslashEscapes = false): Segment[] {
  const out: Segment[] = [];
  for (const seg of splitStatements(text, backslashEscapes)) {
    let start = seg.start;
    for (const cut of scriptCuts(text, seg.start, seg.end, backslashEscapes)) {
      out.push({ start, end: cut });
      start = cut;
    }
    out.push({ start, end: seg.end });
  }
  return out.filter((s) => text.slice(s.start, s.end).trim().length > 0);
}

function scriptCuts(text: string, s: number, e: number, backslashEscapes: boolean): number[] {
  const cuts: number[] = [];
  let i = s;
  let depth = 0;
  let lineStart = s;
  let lineHasCode = false;
  let head = "";
  let prev = "";
  let prevPunct = false;
  let withMain = false;
  while (i < e) {
    const c = text[i];
    const d = text[i + 1];
    if (c === "\n") {
      lineStart = i + 1;
      lineHasCode = false;
      i++;
      continue;
    }
    if (/\s/.test(c)) {
      i++;
      continue;
    }
    if (c === "-" && d === "-") {
      const j = text.indexOf("\n", i);
      i = j < 0 || j > e ? e : j;
      continue;
    }
    if (c === "/" && d === "*") {
      const j = text.indexOf("*/", i + 2);
      const end = j < 0 ? e : Math.min(j + 2, e);
      if (text.slice(i, end).includes("\n")) lineHasCode = true;
      i = end;
      continue;
    }
    const firstOnLine = !lineHasCode;
    lineHasCode = true;
    if (c === "'" || c === '"' || c === "`") {
      let j = i + 1;
      while (j < e) {
        if (backslashEscapes && text[j] === "\\") {
          j += 2;
          continue;
        }
        if (text[j] === c) {
          if (text[j + 1] === c) {
            j += 2;
            continue;
          }
          break;
        }
        j++;
      }
      i = Math.min(j + 1, e);
      prev = "''";
      prevPunct = false;
      continue;
    }
    if (c === "$") {
      const m = /^\$[A-Za-z_][A-Za-z_0-9]*\$|^\$\$/.exec(text.slice(i, i + 64));
      if (m) {
        const j = text.indexOf(m[0], i + m[0].length);
        i = j < 0 || j > e ? e : j + m[0].length;
        prev = "$$";
        prevPunct = false;
        continue;
      }
    }
    if (/[A-Za-z0-9_\u0080-￿]/.test(c)) {
      let j = i;
      while (j < e && /[A-Za-z0-9_$\u0080-￿]/.test(text[j])) j++;
      const w = text.slice(i, j).toUpperCase();
      if (!head) head = w;
      else if (depth === 0) {
        const starter = STARTERS.has(w);
        const withBody = head === "WITH" && !withMain && ["SELECT", "INSERT", "UPDATE", "DELETE", "MERGE"].includes(w);
        if (withBody) withMain = true;
        else if (starter && firstOnLine && !HEADS.has(head)) head = w;
        else if (starter && firstOnLine && !CONT.has(prev) && !(prevPunct && prev !== ")")) {
          const split =
            head === "CREATE" || head === "ALTER" || head === "EXPLAIN"
              ? false
              : head === "INSERT" || head === "REPLACE"
                ? w !== "SELECT" && w !== "WITH"
                : head === "GRANT" || head === "REVOKE"
                  ? w === "GRANT" || w === "REVOKE"
                  : true;
          if (split) {
            cuts.push(lineStart);
            head = w;
            withMain = false;
          }
        }
      }
      prev = w;
      prevPunct = false;
      i = j;
      continue;
    }
    if (c === "(") depth++;
    else if (c === ")") depth--;
    prev = c;
    prevPunct = true;
    i++;
  }
  return cuts;
}
