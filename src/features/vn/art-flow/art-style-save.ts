// 畫風輸入框的「離開就存」不保證存完：生圖入口（開始、重試、單格生成）送出前一律先等這桌畫風存好，
// 存失敗就丟錯、不生圖（plans/vn-player-art-style.md §4）。輸入框掛著時登記存檔函式，沒掛就不必等。
import { t } from "../../../i18n";

type Saver = () => Promise<boolean>;

const savers = new Map<string, Set<Saver>>();

export function registerArtStyleSaver(worldId: string, save: Saver): () => void {
  const set = savers.get(worldId) ?? new Set<Saver>();
  set.add(save);
  savers.set(worldId, set);
  return () => {
    set.delete(save);
    if (set.size === 0) savers.delete(worldId);
  };
}

/** 等這桌最新的畫風存好；任一個沒存成就丟出給玩家看的那句（呼叫端照一般動作失敗顯示，不送生圖） */
export async function flushArtStyle(worldId: string): Promise<void> {
  const results = await Promise.all([...(savers.get(worldId) ?? [])].map((save) => save()));
  if (results.includes(false)) throw t("vnStyleSaveFailed");
}
