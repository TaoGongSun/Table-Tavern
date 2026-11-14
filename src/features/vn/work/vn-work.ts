// VN 作品包（plans/vn-mode-pkg8.md §2.2–§2.3）：匯出對話框（提醒、附設定）→另存→沒封面讓玩家挑→後端出包；
// 匯入：選檔或拖進大廳→只讀 manifest→附設定的包讓玩家二選一、其餘系統確認→後端整桌建好（失敗不留半桌）→進桌。
// 整包走路徑，不經 IPC 傳位元組。
import { invoke } from "@tauri-apps/api/core";
import { confirm, message, open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { t } from "../../../i18n";
import type { VnEndings, VnWorkImport, VnWorkNotice, VnWorkProbe } from "../../../shared/contracts/backend-contracts";

export type VnWorkMode = "replay" | "continue";

export const WORK_EXTENSION = "ttvn";

/** 拖進來的路徑是作品包 */
export function isWorkPath(path: string): boolean {
  return path.toLowerCase().endsWith(`.${WORK_EXTENSION}`);
}

/** 提示一條一行；沒有回空字串 */
export function vnWorkNoticeText(notices: VnWorkNotice[]): string {
  return notices.map((notice) => t(`vnWorkNotice_${notice.kind}`, { target: notice.target })).join("\n");
}

async function tellNotices(notices: VnWorkNotice[]) {
  if (notices.length > 0) {
    await message(vnWorkNoticeText(notices), { title: t("vnWorkNoticesTitle"), okLabel: t("dialogAck") });
  }
}

/** 匯入前確認框內文：標題、章數、結局數、分級；`choice`＝附設定的包（二選一說明） */
export function vnWorkConfirmText(probe: VnWorkProbe, choice = false): string {
  const warnings = probe.rating?.warnings.filter((warning) => warning.trim() !== "") ?? [];
  const rating = probe.rating
    ? t("vnWorkRating", {
        rating: t(`vnCardRating_${probe.rating.level}`),
        warnings: warnings.length > 0 ? t("vnCardWarnings", { list: warnings.join("、") }) : "",
      })
    : "";
  const params = { title: probe.title, chapters: probe.chapters, endings: probe.endings, rating };
  return t(choice ? "vnWorkChoiceBody" : "vnWorkConfirmBody", params);
}

/** 包內附了設定、又不是變數模式桌匯出的：可以當可續寫的桌 */
export function canContinueWork(probe: VnWorkProbe): boolean {
  return probe.settings && !probe.vars_mode;
}

/** 任一路線走到過結局（保底結局也算）才能匯出：按下就先查，免得玩家選完存檔位置與封面才被後端擋 */
export async function workExportable(worldId: string): Promise<boolean> {
  const overview = await invoke<VnEndings>("vn_endings", { worldId });
  return overview.endings.some((ending) => ending.reached);
}

/** 匯出（提醒與附設定在對話框裡確認過）：另存 → 桌上沒封面時讓玩家挑一張（取消＝第一張背景）→ 出包 →
 * 在資料夾中顯示。回是否真的匯出 */
export async function exportVnWork(
  worldId: string,
  tableName: string,
  { settings, cover: hasCover }: { settings: boolean; cover: boolean },
): Promise<boolean> {
  const path = await saveDialog({
    defaultPath: `${tableName.trim() || "vn"}.${WORK_EXTENSION}`,
    filters: [{ name: t("vnWorkExportFile"), extensions: [WORK_EXTENSION] }],
  });
  if (!path) return false;
  let cover: string | null = null;
  if (!hasCover) {
    const picked = await open({ title: t("vnWorkExportCover"), filters: [{ name: "PNG", extensions: ["png"] }] });
    cover = typeof picked === "string" ? picked : null;
  }
  const exported = await invoke<{ notices: VnWorkNotice[] }>("export_vn_work", { worldId, path, cover, settings });
  await tellNotices(exported.notices);
  await revealItemInDir(path);
  return true;
}

/** 選一個作品包；取消回 null */
export async function pickVnWork(): Promise<string | null> {
  const picked = await open({
    title: t("vnWorkImport"),
    filters: [{ name: t("vnWorkExportFile"), extensions: [WORK_EXTENSION] }],
  });
  return typeof picked === "string" ? picked : null;
}

/** 匯入第一步：只讀 manifest */
export function probeVnWork(path: string): Promise<VnWorkProbe> {
  return invoke<VnWorkProbe>("probe_vn_work", { path });
}

/** 沒附設定的包：系統確認框，回玩家要不要開作品桌 */
export function confirmVnWork(probe: VnWorkProbe): Promise<boolean> {
  return confirm(vnWorkConfirmText(probe), {
    title: t("vnWorkConfirmTitle"),
    okLabel: t("vnWorkImportOk"),
    cancelLabel: t("dialogCancel"),
  });
}

/** 後端建桌 → 提示（呼叫端負責換桌互斥與進桌）。回新桌 id */
export async function importVnWork(path: string, mode: VnWorkMode, spoilerLock: boolean): Promise<string> {
  const imported = await invoke<VnWorkImport>("import_vn_work", { path, mode, spoilerLock });
  await tellNotices(imported.notices);
  return imported.world_id;
}
