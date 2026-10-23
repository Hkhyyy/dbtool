import { describe, expect, it } from "vitest";
import { isReleaseUrl } from "./api";

describe("isReleaseUrl", () => {
  it("업데이트 창은 이 저장소의 릴리스 주소만 연다 (설정 폴더의 캐시가 바뀌어도)", () => {
    expect(isReleaseUrl("https://github.com/Hkhyyy/qurry/releases/tag/v0.1.17")).toBe(true);
    expect(isReleaseUrl("https://github.com/Hkhyyy/qurry/releases/download/v0.1.17/Qurry_0.1.17_aarch64.dmg")).toBe(true);
    expect(isReleaseUrl("https://github.com/Hkhyyy/dbtool/releases/tag/v0.1.15")).toBe(true);
    expect(isReleaseUrl("https://evil.example/Qurry.dmg")).toBe(false);
    expect(isReleaseUrl("https://github.com/Hkhyyy/qurry.evil/releases/x")).toBe(false);
    expect(isReleaseUrl("http://github.com/Hkhyyy/qurry/releases/x")).toBe(false);
    expect(isReleaseUrl("javascript:alert(1)")).toBe(false);
  });
});
