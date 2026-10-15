import { useSyncExternalStore } from "react";

/** Settings › Appearance › Theme. "system" = Sync with OS */
export type ThemePref = "dark" | "light" | "hc" | "system";
export type Theme = "dark" | "light" | "hc";

const KEY = "dbtool.theme";
const media = window.matchMedia("(prefers-color-scheme: dark)");
let pref: ThemePref = (() => {
  try {
    return (localStorage.getItem(KEY) as ThemePref) || "dark";
  } catch {
    return "dark";
  }
})();
const subs = new Set<() => void>();

const resolve = (): Theme => (pref === "system" ? (media.matches ? "dark" : "light") : pref);
let current: Theme = resolve();

function apply() {
  current = resolve();
  document.documentElement.dataset.theme = current;
  subs.forEach((f) => f());
}
media.addEventListener("change", () => pref === "system" && apply());
apply();

export function setThemePref(p: ThemePref) {
  pref = p;
  try {
    localStorage.setItem(KEY, p);
  } catch {
    /* 저장 못 해도 이번 실행에는 적용 */
  }
  apply();
}
export const getThemePref = () => pref;
export const THEME_LABEL: Record<Theme, string> = { dark: "Dark", light: "Light", hc: "High contrast" };

/** 현재 테마 (바뀌면 다시 렌더) */
export function useTheme(): Theme {
  return useSyncExternalStore(
    (f) => (subs.add(f), () => subs.delete(f)),
    () => current,
  );
}
export const monacoTheme = (t: Theme) => (t === "light" ? "dbtool-light" : t === "hc" ? "dbtool-hc" : "dbtool-dark");

/** 접속 색: 밝은 테마에서는 흰색과 섞어 연하게 (DataGrip 과 같은 느낌) */
export const connBg = (color: string | null | undefined, t: Theme) =>
  !color ? undefined : t === "light" ? `color-mix(in srgb, ${color} 30%, #ffffff)` : color;
