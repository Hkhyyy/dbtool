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
