// AI 卡重構接管介面時產的骨架（interface-shell.html，後端 refactor_interface_shell 讀回來，讀不到是
// null）：照搬卡規定的每回合輸出格式，變動處是 `{{狀態樹路徑}}` 佔位符（例如 `{{World.Time}}`），
// 正文槽固定 `{{本回合.正文}}`。這裡把佔位符換成狀態樹目前的值；填完的骨架交給卡自己的顯示
// 腳本渲染（選路見 card-interface/card-shell-route.ts）。純函式、零 UI／invoke 依賴。

/** 狀態樹節點：葉子是值，分支是子節點（對應後端 StateNode 的 untagged 序列化）。 */
export type StateNode = string | { [key: string]: StateNode };

// 佔位符只認 `{{...}}`：內容不含花括號或換行的簡單形式。
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

/**
 * 把骨架裡的 `{{狀態樹路徑}}` 換成狀態樹的葉子值；路徑點分、逐層查樹，查不到或落在分支節點都換成
 * 空字串。不 escape，值原文放回。骨架填完是餵給卡自己顯示腳本（regex＋模板）的
 * 「每回合輸出」，清單欄位的值照卡原文含內層標籤（如 `<Item>…</Item>`），escape 會讓卡的殼
 * 解析不到；信任模型與直玩餵 event.raw 相同——模型原文進沙盒 iframe，不多做一層。
 */
export function fillSkeletonPlaceholders(skeleton: string, tree: Record<string, StateNode>): string {
  return skeleton.replace(PLACEHOLDER_REGEX, (_match, rawPath: string) => {
    const path = rawPath.trim().split(".");
    return lookupPath(tree, path);
  });
}
