import { describe, expect, it } from "vitest";
import { leaf, leaves, MAX_PANES, prunePanes, splitPane } from "./panes";

describe("panes", () => {
  it("최대 4칸까지 나누고, 지운 칸 id 를 다시 쓴다", () => {
    let p = leaf(0);
    const r1 = splitPane(p, 0, "right")!;
    const r2 = splitPane(r1.pane, 0, "down")!;
    const r3 = splitPane(r2.pane, 1, "down")!;
    p = r3.pane;
    expect([r1.g, r2.g, r3.g]).toEqual([1, 2, 3]);
    expect(leaves(p)).toEqual([0, 2, 1, 3]);
    expect(leaves(p)).toHaveLength(MAX_PANES);
    expect(splitPane(p, 3, "right")).toBeNull();
    p = prunePanes(p, new Set([0, 1, 3]));
    expect(leaves(p)).toEqual([0, 1, 3]);
    expect(splitPane(p, 0, "down")!.g).toBe(2);
  });

  it("없는 칸은 나누지 않는다", () => {
    expect(splitPane(leaf(0), 5, "right")).toBeNull();
  });

  it("전부 비면 칸 하나만 남긴다", () => {
    const p = splitPane(leaf(0), 0, "right")!.pane;
    expect(prunePanes(p, new Set())).toEqual(leaf(0));
  });
});
