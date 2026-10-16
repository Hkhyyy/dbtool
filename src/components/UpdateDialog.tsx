import { openUrl } from "@tauri-apps/plugin-opener";
import { UpdateInfo } from "../lib/api";

/** 새 버전 안내: 릴리스 노트 + Download (dmg 를 브라우저로 내려받는다. 설치는 Applications 로 끌어다 놓기) */
export function UpdateDialog({ info, onClose }: { info: UpdateInfo; onClose: () => void }) {
  const r = info.latest!;
  const dmg = r.assets.find((a) => a.name.endsWith(".dmg"));
  const open = (url: string) => openUrl(url).catch(() => window.open(url, "_blank"));
  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div className="modal update" onMouseDown={(e) => e.stopPropagation()} onKeyDown={(e) => e.key === "Escape" && onClose()}>
        <div className="modal-title">dbtool {r.version} is available</div>
        <div className="hint" style={{ marginTop: 0 }}>
          You have {info.current}. Download the DMG, open it and drag dbtool.app into Applications (replace the old one), then restart.
        </div>
        <pre className="update-notes">{r.notes || "No release notes."}</pre>
        <div className="modal-actions">
          <button onClick={() => open(r.url)}>Release Page</button>
          <span className="grow" />
          <button onClick={onClose}>Later</button>
          <button className="primary" autoFocus onClick={() => (open(dmg?.url ?? r.url), onClose())}>
            Download{dmg ? "" : "…"}
          </button>
        </div>
      </div>
    </div>
  );
}
