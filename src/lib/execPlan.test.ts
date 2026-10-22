import { describe, expect, it } from "vitest";
import { DEFAULT_EXEC, planExecution } from "./execPlan";
import { splitStatements } from "./sqlSplit";

const SQL = "BEGIN;\nGRANT a TO x;\n\nGRANT b TO x;\nCOMMIT;";
const plan = (offset: number, s = DEFAULT_EXEC) => planExecution(SQL, offset, splitStatements(SQL), s);
const at = (needle: string) => SQL.indexOf(needle) + 1;

describe("planExecution (DataGrip Query Execution)", () => {
  it("기본값: 커서가 문장 안이면 무엇을 실행할지 묻는다", () => {
    expect(DEFAULT_EXEC.inside).toBe("ask");
    const p = plan(at("GRANT b"));
    expect(p && "ask" in p && p.ask.map((o) => o.label)).toEqual(["GRANT b TO x", "All statements (4)", "From caret to end"]);
    expect(p && "ask" in p && p.ask[2].stmts.map((s) => s.sql)).toEqual(["GRANT b TO x", "COMMIT"]);
  });

  it("문장이 하나뿐이면 묻지 않는다", () => {
    expect(planExecution("select 1", 3, splitStatements("select 1"), DEFAULT_EXEC)).toEqual({ run: [{ start: 0, end: 8, sql: "select 1" }] });
  });

  it("첫 문장에서는 '커서부터 끝까지' 가 전체와 같아서 뺀다", () => {
    const p = plan(at("BEGIN"));
    expect(p && "ask" in p && p.ask.map((o) => o.label)).toEqual(["BEGIN", "All statements (4)"]);
  });

  it("smallest / whole / fromCaret", () => {
    const sql = (p: ReturnType<typeof plan>) => (p && "run" in p ? p.run.map((s) => s.sql) : null);
    expect(sql(plan(at("GRANT b"), { ...DEFAULT_EXEC, inside: "smallest" }))).toEqual(["GRANT b TO x"]);
    expect(sql(plan(at("GRANT b"), { ...DEFAULT_EXEC, inside: "whole" }))).toHaveLength(4);
    expect(sql(plan(at("GRANT b"), { ...DEFAULT_EXEC, inside: "fromCaret" }))).toEqual(["GRANT b TO x", "COMMIT"]);
  });

  it("커서가 문장 밖(빈 줄)이면: nothing / whole / below", () => {
    const blank = SQL.indexOf("\n\n") + 1;
    const sql = (p: ReturnType<typeof plan>) => (p && "run" in p ? p.run.map((s) => s.sql) : null);
    expect(plan(blank)).toBeNull();
    expect(sql(plan(blank, { ...DEFAULT_EXEC, outside: "whole" }))).toHaveLength(4);
    expect(sql(plan(blank, { ...DEFAULT_EXEC, outside: "below" }))).toEqual(["GRANT b TO x", "COMMIT"]);
  });

  it("세미콜론 바로 뒤 커서는 그 문장", () => {
    const end = SQL.indexOf("GRANT a TO x;") + "GRANT a TO x;".length;
    const p = plan(end, { ...DEFAULT_EXEC, inside: "smallest" });
    expect(p && "run" in p && p.run.map((s) => s.sql)).toEqual(["GRANT a TO x"]);
  });
});
