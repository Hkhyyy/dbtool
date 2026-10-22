import { describe, expect, it } from "vitest";
import { splitStatements, statementAt } from "./sqlSplit";

const parts = (sql: string, bs = false) => splitStatements(sql, bs).map((s) => sql.slice(s.start, s.end).trim());

describe("splitStatements", () => {
  it("세미콜론으로 나누고 빈 문장은 버린다", () => {
    expect(parts("select 1; select 2;;\n")).toEqual(["select 1", "select 2"]);
  });

  it("문자열·주석·달러 인용 안의 세미콜론은 건너뛴다", () => {
    expect(parts("select ';'; -- a;b\nselect 2 /* ; */; select $f$ ; $f$")).toEqual(["select ';'", "-- a;b\nselect 2 /* ; */", "select $f$ ; $f$"]);
  });

  it("MySQL 은 백슬래시 이스케이프를 따른다", () => {
    expect(parts("select 'a\\';b'; select 2", true)).toEqual(["select 'a\\';b'", "select 2"]);
  });
});

describe("statementAt", () => {
  it("커서가 있는 문장, 문장 사이면 바로 앞 문장", () => {
    const sql = "select 1;\n\nselect 2;";
    expect(statementAt(sql, 3)?.sql).toBe("select 1");
    expect(statementAt(sql, sql.indexOf("2"))?.sql).toBe("select 2");
    expect(statementAt("", 0)).toBeNull();
  });
});
