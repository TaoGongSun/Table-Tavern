// menu-keyboard-webkit 最小重現：真的 WorldEditor＋假 Tauri 後端（world.html），用法見 plans/menu-keyboard-webkit.md 附錄
import { createRoot } from "react-dom/client";
import "/src/App.css";
import { WorldEditor } from "/src/features/worldbook/WorldEditor";

const w = window as unknown as { __log: string[] };
w.__log = [];
const log = (m: string) => w.__log.push(m);
const name = (el: Element | null) =>
  el ? (el.textContent || el.tagName).slice(0, 30) + (el === document.body ? "(body)" : "") : "null";
document.addEventListener(
  "keydown",
  (e) => log(`keydown ${e.key} target=${name(e.target as Element)} prevented=${e.defaultPrevented}`),
  true,
);
document.addEventListener("focusin", (e) => log(`focusin ${name(e.target as Element)}`), true);

createRoot(document.getElementById("root")!).render(
  <div className="app" style={{ height: "100vh", display: "flex" }}>
    <main className="chat-main" style={{ flex: 1 }}>
      <WorldEditor
        title="World"
        world="w1"
        worldName="W"
        onBack={() => {}}
        leaveGuard={{ current: null }}
        convertColor="#888"
        onEntryConverted={async () => {}}
        onRefactorApplied={async () => {}}
      />
    </main>
  </div>,
);
