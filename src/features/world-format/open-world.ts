// open_world 的分流與唯讀紀錄對應。畫面怎麼畫在 FormatNotice，這裡只留可單測的判斷。
import { t } from "../../i18n";
import type { RepairReason } from "../../i18n/features/backend-msg";
import { TranscriptEvent, WorldMeta } from "../../shared/contracts/backend-contracts";

export type TableGate = "play" | "readonly" | "repair";

/** 需修復頁要的資料：原因代碼、io 時的系統錯誤原文、要打開的資料夾。 */
export interface RepairNotice {
  reason: RepairReason;
  error: string | null;
  directory: string;
}

/** 對應後端 OpenWorld（serde tag = status）。 */
export type OpenWorld =
  | { status: "ready" }
  | { status: "migrated"; from: number; to: number }
  | {
      status: "read_only";
      format_version: number | null;
      app_version: string | null;
      backup_available: boolean;
    }
  | ({ status: "needs_repair" } & RepairNotice)
  | { status: "busy" };

export interface LooseLine {
  speaker_name: string;
  text: string;
  kind: string;
}

/** read_world_readonly 的回傳。幕號與略過行數由後端算好。 */
export interface LooseWorld {
  scene: number;
  events: LooseLine[];
  skipped: number;
}

const TRANSCRIPT_KINDS = new Set<TranscriptEvent["kind"]>([
  "dialogue",
  "narration",
  "player",
  "system",
]);

/** ready／migrated 進可玩；唯讀與修復不走嚴格讀取；busy 維持原桌。 */
export function gateOf(opened: OpenWorld): TableGate | "busy" {
  switch (opened.status) {
    case "ready":
    case "migrated":
      return "play";
    case "read_only":
      return "readonly";
    case "needs_repair":
      return "repair";
    case "busy":
      return "busy";
  }
}

/**
 * 橫幅裡的 X。只認 app_version；缺了、空白、或只有格式數字，都當版本不明（省略 X）。
 */
export function readOnlyBannerVersion(opened: {
  app_version?: string | null;
  format_version?: number | null;
}): string | null {
  const version = opened.app_version?.trim() ?? "";
  return version === "" ? null : version;
}

/** 寬鬆行補成畫面用的逐字稿。認不得的 kind 當成系統訊息，不帶 id 與時間。 */
export function looseTranscript(lines: LooseLine[]): TranscriptEvent[] {
  return lines.map((line) => ({
    ts: "",
    speaker_id: "",
    speaker_name: line.speaker_name,
    kind: TRANSCRIPT_KINDS.has(line.kind as TranscriptEvent["kind"])
      ? (line.kind as TranscriptEvent["kind"])
      : "system",
    text: line.text,
  }));
}

/** 兩邊都成立時標「需要修復」。徽章是說明，不是按鈕。 */
export function listBadge(world: {
  read_only: boolean;
  needs_repair: boolean;
}): "repair" | "readonly" | null {
  if (world.needs_repair) return "repair";
  if (world.read_only) return "readonly";
  return null;
}

/** 刪桌確認窗內文。唯讀桌（紀錄比目前版本新）多一句：刪掉後更新回新版也找不回來。 */
export function deleteTableMessage(world: WorldMeta | undefined, fallbackName: string): string {
  const base = t("deleteTableConfirm", { name: world?.name ?? fallbackName });
  return world?.read_only ? `${base}\n\n${t("deleteTableReadOnlyWarn")}` : base;
}
