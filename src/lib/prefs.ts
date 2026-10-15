import { monaco } from "./monaco";

/** 에디터 글꼴 크기 (⌃⇧. / ⌃⇧,). 브라우저 저장소가 막혀도 동작한다 */
const KEY = "dbtool.editorFontSize";
export const DEFAULT_FONT = 13;
let size = (() => {
  try {
    return Number(localStorage.getItem(KEY)) || DEFAULT_FONT;
  } catch {
    return DEFAULT_FONT;
  }
})();
export const editorFontSize = () => size;
export function setEditorFontSize(n: number) {
  size = Math.min(32, Math.max(8, n));
  try {
    localStorage.setItem(KEY, String(size));
  } catch {
    /* 무시 */
  }
  for (const ed of monaco.editor.getEditors()) ed.updateOptions({ fontSize: size });
  document.documentElement.style.setProperty("--grid-font", `${size}px`);
  return size;
}
