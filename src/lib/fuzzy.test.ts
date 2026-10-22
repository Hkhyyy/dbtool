import { describe, expect, it } from "vitest";
import { fuzzyScore } from "./fuzzy";

describe("fuzzyScore", () => {
  it("같음 > 앞부분 > 단어 시작 > 중간 > 흩어진 글자 순", () => {
    const s = (t: string) => fuzzyScore("pay", t)!;
    expect(s("pay")).toBeGreaterThan(s("payment"));
    expect(s("payment")).toBeGreaterThan(s("user_payment"));
    expect(s("user_payment")).toBeGreaterThan(s("repay"));
    expect(s("repay")).toBeGreaterThan(fuzzyScore("upy", "user_payment")!);
  });

  it("글자가 없으면 null", () => {
    expect(fuzzyScore("xyz", "users")).toBeNull();
    expect(fuzzyScore("", "users")).toBe(0);
  });
});
