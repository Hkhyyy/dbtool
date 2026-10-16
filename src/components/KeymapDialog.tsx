import { useState } from "react";
import { KEYMAP, KeyRow, toOther } from "../lib/keymap";
import { isCustom, shortcutLabel } from "../lib/shortcuts";

const isMac = navigator.platform.toLowerCase().includes("mac");

/** 줄에 보일 키: 바꿀 수 있는 명령은 현재 키, 나머지는 고정 표기 */
const keysOf = ([k, , id]: KeyRow) => (id ? shortcutLabel(id) || "—" : isMac ? k : toOther(k));

/** Help › Keymap Reference: 검색되는 단축키 목록 */
export function KeymapDialog({ onClose, onEdit }: { onClose: () => void; onEdit: () => void }) {
  const [q, setQ] = useState("");
  const f = q.trim().toLowerCase();
  const sections = KEYMAP.map((s) => ({
    ...s,
    rows: s.rows.filter((r) => !f || r[1].toLowerCase().includes(f) || keysOf(r).toLowerCase().includes(f)),
  })).filter((s) => s.rows.length);
  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div className="modal keymap" onMouseDown={(e) => e.stopPropagation()} onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Escape") onClose(); }}>
        <div className="modal-title">Keymap Reference</div>
        <input className="keymap-search" autoFocus placeholder="Search actions or shortcuts" value={q} onChange={(e) => setQ(e.target.value)} />
        <div className="keymap-body">
          {sections.map((s) => (
            <div key={s.title} className="keymap-section">
              <div className="keymap-title">{s.title}</div>
              {s.rows.map((r) => (
                <div key={s.title + r[0] + r[1]} className={"keymap-row" + (r[2] && isCustom(r[2]) ? " custom" : "")}>
                  <span className="keymap-keys">{keysOf(r)}</span>
                  <span className="keymap-desc">{r[1]}</span>
                </div>
              ))}
            </div>
          ))}
          {!sections.length && <div className="popup-empty">Nothing found</div>}
        </div>
        <div className="modal-actions">
          <span className="hint" style={{ margin: 0 }}>Also: Find Action ({shortcutLabel("find.action")}) lists every command with its shortcut.</span>
          <span className="grow" />
          <button onClick={onEdit}>Edit Keymap…</button>
          <button className="primary" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}
