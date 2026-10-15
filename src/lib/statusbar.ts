import { useSyncExternalStore } from "react";

/** 상태 표시줄 오른쪽 위젯 (Line:Column, 그리드 Aggregator). 마지막으로 포커스된 쪽이 쓴다 */
let widgets: { caret?: string; aggregate?: string } = {};
const subs = new Set<() => void>();
export function setStatusWidget(k: "caret" | "aggregate", v: string | undefined) {
  if (widgets[k] === v) return;
  widgets = { ...widgets, [k]: v };
  subs.forEach((f) => f());
}
export const useStatusWidgets = () =>
  useSyncExternalStore(
    (f) => (subs.add(f), () => subs.delete(f)),
    () => widgets,
  );
