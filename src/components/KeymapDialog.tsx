import { useState } from "react";
import { KEYMAP, toOther } from "../lib/keymap";

const isMac = navigator.platform.toLowerCase().includes("mac");

/** Help › Keymap Reference: 검색되는 단축키 목록 */
export function KeymapDialog({ onClose }: { onClose: () => void }) {
  const [q, setQ] = useState("");
  const f = q.trim().toLowerCase();
  const sections = KEYMAP.map((s) => ({
    ...s,
    rows: s.rows.filter(([k, d]) => !f || d.toLowerCase().includes(f) || k.toLowerCase().includes(f) || toOther(k).toLowerCase().includes(f)),
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
              {s.rows.map(([k, d]) => (
                <div key={s.title + k + d} className="keymap-row">
                  <span className="keymap-keys">{isMac ? k : toOther(k)}</span>
                  <span className="keymap-desc">{d}</span>
                </div>
              ))}
            </div>
          ))}
          {!sections.length && <div className="popup-empty">Nothing found</div>}
        </div>
        <div className="modal-actions">
          <span className="hint" style={{ margin: 0 }}>Also: Find Action ({isMac ? "⇧⌘A" : "Ctrl+Shift+A"}) lists every command with its shortcut.</span>
          <span className="grow" />
          <button className="primary" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}
