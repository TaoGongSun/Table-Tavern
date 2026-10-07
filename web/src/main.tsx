import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { takeCallback } from "./features/openrouter/oauth";
import "./App.css";

// OAuth 回呼參數在 React 掛載前就讀進記憶體並清掉網址（計畫 2.4 的順序）
const callback = takeCallback(window);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App callback={callback} />
  </React.StrictMode>,
);
