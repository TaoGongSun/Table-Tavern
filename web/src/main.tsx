import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { takeCallback } from "./features/openrouter/oauth";
import { applyLang, initialLang } from "./i18n/lang-store";
import "./App.css";

// OAuth 回呼參數在 React 掛載前就讀進記憶體並清掉網址（計畫 2.4 的順序）
const callback = takeCallback(window);
// 語系在第一次繪製前定好（存過的優先，否則照瀏覽器語系）
const lang = initialLang();
applyLang(lang);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App callback={callback} lang={lang} />
  </React.StrictMode>,
);
