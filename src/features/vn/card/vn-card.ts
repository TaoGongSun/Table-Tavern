// 初始 VN 卡（plans/vn-mode-pkg9.md §2.2–§2.3、§2.7）：匯入前的分級確認、夾斷提示與錯誤碼文案、
// 匯入建桌的開桌那一半（照網頁存檔的模式：後端整桌建好 → 照開新桌的流程進去）、VN 桌匯出成卡。
import { invoke } from "@tauri-apps/api/core";
import { confirm, open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { t } from "../../../i18n";
import type { BibleView, VnCardImport, VnCardProbe } from "../../../shared/contracts/backend-contracts";
import { openImportTable, type EnterResult } from "../../lobby/open-import-table";

/** 劇本分頁的重骰改成「恢復卡內原版」：與後端 `vn_bible::generate` 同條件（卡上存了原版聖經才恢復，否則照常生成） */
export function restoresCardBible(view: Pick<BibleView, "card"> | null): boolean {
  return Boolean(view?.card?.original_bible);
}

/** 分級確認框的內文：作品名、分級與警示、登場、明文提醒 */
export function vnCardConfirmText(probe: VnCardProbe): string {
  const warnings = probe.rating.warnings.filter((warning) => warning.trim() !== "");
  return t("vnCardConfirmBody", {
    title: probe.title,
    rating: t(`vnCardRating_${probe.rating.level}`),
    warnings: warnings.length > 0 ? t("vnCardWarnings", { list: warnings.join("、") }) : "",
    cast: probe.cast.join("、"),
  });
}

export function confirmVnCard(probe: VnCardProbe): Promise<boolean> {
  return confirm(vnCardConfirmText(probe), {
    title: t("vnCardConfirmTitle"),
    kind: "warning",
    okLabel: t("vnCardImportOk"),
    cancelLabel: t("dialogCancel"),
  });
}

/** 匯入時的夾斷與丟棄提示，一條一行；沒有回空字串 */
export function vnCardNoticeText(notices: VnCardImport["notices"]): string {
  return notices.map((notice) => t(`vnCardNotice_${notice.kind}`, { target: notice.target })).join("\n");
}

/** 後端錯誤碼（`import/vn_card/schema.rs`）換成文案；其他錯原樣回 null */
export function vnCardErrorText(reason: unknown): string | null {
  const text = String(reason);
  if (text.includes("VN_CARD_TOO_LARGE")) return t("vnCardTooLarge");
  if (text.includes("VN_CARD_NO_CAST")) return t("vnCardNoCast");
  return null;
}

/** 後端整桌建好（失敗不留半桌）→ 重讀清單 → 進桌；null＝新桌進不去或不可寫（同 openImportTable） */
export async function openVnCardTable(
  io: {
    importCard: (data: number[], spoilerLock: boolean) => Promise<VnCardImport>;
    refreshWorlds: () => Promise<void>;
    enterTable: (id: string) => Promise<EnterResult>;
  },
  data: number[],
  spoilerLock: boolean,
): Promise<VnCardImport | null> {
  let imported: VnCardImport | null = null;
  const id = await openImportTable(
    {
      createWorld: async () => {
        imported = await io.importCard(data, spoilerLock);
        return imported.world_id;
      },
      refreshWorlds: io.refreshWorlds,
      enterTable: io.enterTable,
    },
    "",
  );
  return id === null ? null : imported;
}

/** VN 桌匯出成卡：明文提醒 → 存檔對話框 → PNG 且桌上沒封面時讓玩家挑一張（可取消）→ 後端組卡 → 在資料夾中顯示。
 * 回傳是否真的匯出。 */
export async function exportVnCard(worldId: string, tableName: string): Promise<boolean> {
  const ok = await confirm(t("vnCardExportWarn"), {
    title: t("vnCardExport"),
    kind: "warning",
    okLabel: t("vnCardExport"),
    cancelLabel: t("dialogCancel"),
  });
  if (!ok) return false;
  const path = await saveDialog({
    defaultPath: `${tableName.trim() || "vn"}.png`,
    filters: [
      { name: t("vnCardExportPng"), extensions: ["png"] },
      { name: t("vnCardExportJson"), extensions: ["json"] },
    ],
  });
  if (!path) return false;
  let cover: string | null = null;
  if (!path.toLowerCase().endsWith(".json") && !(await invoke<BibleView>("vn_bible_read", { worldId })).cover) {
    const picked = await open({ title: t("vnCardExportCover"), filters: [{ name: "PNG", extensions: ["png"] }] });
    cover = typeof picked === "string" ? picked : null;
  }
  await invoke("export_vn_card", { worldId, path, cover });
  await revealItemInDir(path);
  return true;
}
