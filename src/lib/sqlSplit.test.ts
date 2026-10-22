import { describe, expect, it } from "vitest";
import { splitScript, splitStatements, statementAt } from "./sqlSplit";

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

describe("splitScript", () => {
  const sp = (sql: string, mysql = false) => splitScript(sql, mysql).map((s) => sql.slice(s.start, s.end).trim());
  it("세미콜론 없이 줄을 바꿔 쓴 문장도 나눈다 (core sql::split_script 와 같은 규칙)", () => {
    expect(sp("select * from a where x = 3\nselect * from public.users")).toEqual(["select * from a where x = 3", "select * from public.users"]);
    expect(sp("delete from t where id = 1\nselect 1;\nselect 2")).toEqual(["delete from t where id = 1", "select 1", "select 2"]);
    expect(sp("insert into t values (1)\ninsert into t values (2)")).toHaveLength(2);
    expect(sp("grant select on a to x\ngrant select on b to x")).toHaveLength(2);
    expect(sp("with x as (\n  select 1\n)\nselect * from x\nselect 2")).toEqual(["with x as (\n  select 1\n)\nselect * from x", "select 2"]);
    expect(sp("1. 공격 탐지 (SSRF)\nselect 1\nselect 2")).toEqual(["1. 공격 탐지 (SSRF)\nselect 1", "select 2"]);
  });
  it("한 문장인 것은 나누지 않는다", () => {
    for (const one of [
      "select 1\nunion all\nselect 2",
      "insert into t (a)\nselect a from u",
      "insert into t (a)\nwith x as (select 1) select * from x",
      "update t\nset a = 1\nwhere id in (\nselect id from u\n)",
      "create view v as\nselect 1",
      "alter table t\ndrop column a",
      "explain\nselect 1",
      "grant select,\ninsert on t to u",
      "grant\nselect on t to u",
      "select * from t for\nupdate",
      "insert into t values (1) on conflict (id) do\nupdate set a = 1",
      "select 'a\nselect b'",
      "select a -- x\n-- select y\nfrom t",
      "select a /* \nselect */ from t",
      "select a,\nselect_count from t",
    ]) {
      expect(sp(one), one).toHaveLength(1);
    }
    expect(sp("insert into t values (1) on duplicate key\nupdate a = 1", true)).toHaveLength(1);
  });
});
