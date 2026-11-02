import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyLang, initialLang } from "./i18n";
import "./App.css";
import { followVisualViewport } from "./features/room/viewport";

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null; // 瀏覽器不讓存：deviceId 只在這次有效
  }
}

followVisualViewport(window);

// 語系在第一次繪製前定好（存過的優先，否則照瀏覽器語系）
const lang = initialLang();
applyLang(lang);

const env = {
  href: window.location.href,
  storage: storage(),
  open: (url: string) => new WebSocket(url),
  fetch: window.fetch.bind(window),
  userAgent: navigator.userAgent,
  replaceHref: (href: string) => window.history.replaceState(null, "", href),
};

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App env={env} lang={lang} />
  </React.StrictMode>,
);
