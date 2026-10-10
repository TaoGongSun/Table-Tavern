// 世界書觸發紀錄結算不了（後端 `world_info_settle_failed`）時的出路：這一幕的紀錄改名留備份、重新算。
// 修好前這桌送不出、換不了幕，所以聊天失敗的彈窗與換幕／分岔／退幕的錯誤都接這裡。
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { t } from "../../i18n";
import { backendCode } from "../../shared/ui/backend-text";

export const offersWorldInfoReset = (raw: unknown): boolean => backendCode(raw) === "world_info_settle_failed";

export async function resetWorldInfoTiming(worldId: string): Promise<void> {
  await invoke<string[]>("reset_world_info_timing", { worldId });
}

/** 換幕、分岔、退幕撞到時：問玩家要不要重設，答應就重設。回傳有沒有重設（重設後玩家再按一次原本的動作）。 */
export async function askWorldInfoReset(worldId: string): Promise<boolean> {
  const accepted = await confirm(t("worldInfoResetConfirm"), {
    title: t("worldInfoReset"),
    kind: "warning",
    okLabel: t("worldInfoReset"),
    cancelLabel: t("dialogCancel"),
  });
  if (!accepted) return false;
  await resetWorldInfoTiming(worldId);
  return true;
}
