// AI 卡重構套用介面規則時，除了狀態樹初始值，可能順便產一份靜態渲染殼（interface-shell.html，
// 後端 refactor_interface_shell 讀回來，讀不到是 null）。殼裡用 `{{狀態樹路徑}}` 佔位符代表資料
// （例如 `{{World.Time}}`、`{{亞瑟.HP}}`，見 src-tauri/src/refactor_ai.rs 的產殼提示詞），這裡把
// 佔位符換成狀態樹目前的值；殼本身照舊走 interface-card.ts 既有的 buildShellDocument 沙盒包裝。
// 純函式、零 UI／invoke 依賴，App.tsx 只管接線。

/** 狀態樹節點：葉子是值，分支是子節點（對應後端 StateNode 的 untagged 序列化）。 */
export type StateNode = string | { [key: string]: StateNode };

// 佔位符只認 `{{...}}`：內容不含花括號或換行的簡單形式。CSS／JS 常見的單花括號區塊
// （如 `.foo { color: red }`、`{a: 1}`）天生就不會命中，不必另外排除。
const PLACEHOLDER_REGEX = /\{\{([^{}\n]+)\}\}/g;

// 逐層查狀態樹；查不到節點、或路徑中途／終點落在分支（非葉子）都回空字串——殼只讀不寫，缺值就是沒東西可顯示。
function lookupPath(tree: Record<string, StateNode>, path: string[]): string {
  let node: StateNode | undefined = tree[path[0]];
  for (const key of path.slice(1)) {
    if (typeof node !== "object" || node === null) return "";
    node = node[key];
  }
  return typeof node === "string" ? node : "";
}

// 安全紅線：狀態值來自模型／卡片資料，一律 HTML escape，殼不能靠佔位符注入標籤或執行邏輯。
// & 一定先換，否則後面幾個實體裡的 & 會被二次轉義。
function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/**
 * 把殼裡的 `{{狀態樹路徑}}` 佔位符換成狀態樹的葉子值（HTML escape 過的純文字）。
 * 路徑點分、逐層查樹；查不到或落在分支節點都換成空字串。
 */
export function fillShellPlaceholders(shell: string, tree: Record<string, StateNode>): string {
  return shell.replace(PLACEHOLDER_REGEX, (_match, rawPath: string) => {
    const path = rawPath.trim().split(".");
    return escapeHtml(lookupPath(tree, path));
  });
}
