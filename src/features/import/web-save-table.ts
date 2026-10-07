// 匯入網頁存檔的開桌那一半：後端整桌建好 → 進桌前寫好卡片 storage → 確認匯入 → 照開新桌的流程進去。
// storage 寫不進去（配額滿等）不能當成功：問玩家要不要重試，放棄就照後端的未確認記錄撤回跨桌層、收掉新桌，
// 這次匯入算失敗（清理沒做完時丟的是後端那句含殘留位置的回報）。
import { openImportTable, type EnterResult } from "../lobby/open-import-table";
import type { WebSaveImport } from "./useImportController";

export interface WebSaveTableIo {
  /** 後端 import_web_save：成功時新桌已整桌建好 */
  importSave: (data: number[]) => Promise<WebSaveImport>;
  /** 寫進新桌的卡片 storage；false＝沒寫成 */
  writeStorage: (worldId: string, entries: Record<string, string>) => boolean;
  /** 寫不進去時問玩家：true＝再試一次，false＝放棄這次匯入 */
  askRetry: () => Promise<boolean>;
  /** 後端 discard_web_save_import：撤回這次補的跨桌層再刪新桌；清理沒做完就 reject（含殘留位置） */
  discardImport: (worldId: string, reason: string) => Promise<void>;
  /** 後端 confirm_web_save_import：storage 寫好了，清掉新桌的未確認記錄 */
  confirmImport: (worldId: string) => Promise<void>;
  refreshWorlds: () => Promise<void>;
  enterTable: (id: string) => Promise<EnterResult>;
  /** 放棄後回報給玩家的那句話（丟出去，由匯入流程的錯誤處理顯示） */
  storageFailed: string;
  /** 放棄的原因，清理沒做完時後端放進回報 */
  storageReason: string;
}

export async function openWebSaveTable(io: WebSaveTableIo, data: number[]): Promise<WebSaveImport | null> {
  let imported: WebSaveImport | null = null;
  const id = await openImportTable(
    {
      createWorld: async () => {
        const result = await io.importSave(data);
        while (!io.writeStorage(result.world_id, result.card_storage)) {
          if (!(await io.askRetry())) {
            await io.discardImport(result.world_id, io.storageReason);
            throw io.storageFailed;
          }
        }
        await io.confirmImport(result.world_id);
        imported = result;
        return result.world_id;
      },
      refreshWorlds: io.refreshWorlds,
      enterTable: io.enterTable,
    },
    "",
  );
  return id === null ? null : imported;
}
