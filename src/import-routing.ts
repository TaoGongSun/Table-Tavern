// 第二張卡路由：桌上已有匯入紀錄時再匯卡，判斷要不要跳提醒框。
// merge_worldbook 是提醒不是封鎖——兩本書合進同一桌是允許的（後端 import_worldbook 本來就接在既有條目後面並去重）。
// 純函式、零 UI／invoke 依賴——App.tsx 只管接線，判斷邏輯在這裡單獨測。

export type ImportIdentity = "character" | "worldbook";
export type ImportReceiptKind = "character" | "worldbook";
export type ImportRoute = "direct" | "companion" | "merge_worldbook" | "ask";

/**
 * 收據是主判準。條目只在收據為空時當保險——那種桌可能是收據功能之前的舊桌、
 * 手建的桌或範例桌，收據說「沒匯過」其實桌上早有內容，別把第二本書無聲合進去。
 *
 * @param identity 這次要匯入的身分（分流直接判定，或身分框答完後決定）
 * @param isPureWorldbookFile 檔案本身有沒有角色欄位：純世界書檔（含 V2 獨立書 JSON）才 true
 * @param receiptKinds 這桌現有匯入收據的身分清單，手建角色不算在內
 * @param tableHasWorldbookEntries 這桌的世界書現在有沒有條目（收據為空時才會被看）
 */
export function decideImportRoute(
  identity: ImportIdentity,
  isPureWorldbookFile: boolean,
  receiptKinds: ImportReceiptKind[],
  tableHasWorldbookEntries: boolean,
): ImportRoute {
  if (receiptKinds.length === 0) {
    return identity === "worldbook" && tableHasWorldbookEntries ? "merge_worldbook" : "direct";
  }
  if (identity !== "worldbook") return "ask";
  const hasWorldbook = receiptKinds.includes("worldbook");
  if (isPureWorldbookFile && !hasWorldbook) return "companion";
  return hasWorldbook ? "merge_worldbook" : "ask";
}
