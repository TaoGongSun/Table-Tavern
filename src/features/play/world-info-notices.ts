// 世界書落地撤回時沒能還原的變數寫入（後端待回報檔 `notices.json`）：聊天、換幕、分岔之後與開桌時讀，
// 每則提示玩家一次，按下確認才刪掉（沒確認就下次再提示）。讀不到就算了，不擋別的事。
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import { t } from "../../i18n";

export interface WorldInfoNotice {
  /** `<回合鍵>:<意圖序號>` */
  id: string;
  turn_key: string;
  layer: string;
  layer_id?: string;
  /** overwritten＝之後被別人寫過；unknown＝分不出是不是那次寫的 */
  reason: "overwritten" | "unknown";
}

/** 同一則正在顯示時不重複跳（聊天回傳與換幕可能同時觸發）。 */
const showing = new Set<string>();

export function noticeText(notice: WorldInfoNotice): string {
  const layer = t(notice.layer === "global" ? "worldInfoVarLayerGlobal" : "worldInfoVarLayerChat");
  return t(notice.reason === "overwritten" ? "worldInfoVarNoticeOverwritten" : "worldInfoVarNoticeUnknown", { layer });
}

export async function showWorldInfoNotices(worldId: string): Promise<void> {
  let notices: unknown;
  try {
    notices = await invoke<WorldInfoNotice[]>("world_info_notices", { worldId });
  } catch {
    return;
  }
  if (!Array.isArray(notices)) return;
  for (const notice of notices as WorldInfoNotice[]) {
    const key = `${worldId}\u0000${notice.id}`;
    if (showing.has(key)) continue;
    showing.add(key);
    try {
      await message(noticeText(notice), { title: t("worldInfoVarNoticeTitle"), kind: "warning", okLabel: t("dialogAck") });
      await invoke("ack_world_info_notice", { worldId, id: notice.id });
    } catch {
      // 確認沒送成：留著，下次再提示
    } finally {
      showing.delete(key);
    }
  }
}
