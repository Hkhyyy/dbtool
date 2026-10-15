import React from "react";
import ReactDOM from "react-dom/client";
import "./lib/monaco";
import "./lib/completion";
import App from "./App";

async function boot() {
  // 개발용: 브라우저에서 ?mock 으로 열면 가짜 백엔드 사용 (화면 확인용)
  if (import.meta.env.DEV && location.search.includes("mock")) await import("./lib/mock");
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
boot();
