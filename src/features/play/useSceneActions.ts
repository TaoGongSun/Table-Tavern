import { invoke } from "@tauri-apps/api/core";
import { confirm, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { SceneLabel, TranscriptEvent } from "../../shared/contracts/backend-contracts";
import { t } from "../../i18n";
import { backendCode } from "../../shared/ui/backend-text";
import { showWorldInfoNotices } from "./world-info-notices";
import { askWorldInfoReset, offersWorldInfoReset } from "./world-info-reset";

interface SceneChatActions {
  events: TranscriptEvent[];
  busy: boolean;
  /** 同步版忙碌判斷：append_transcript 還沒完成時 busy（state）仍是 false，這個已是 true */
  isBusy: () => boolean;
  beginNarration: () => string;
  endNarration: () => void;
}

interface SceneActionsOptions {
  worldId: string;
  scene: number;
  sceneTitles: Record<string, string>;
  sceneLabels: Record<string, SceneLabel>;
  tableName: string;
  chat: SceneChatActions;
  canLeaveEditor: () => Promise<boolean>;
  /** entered＝已提交成目前的桌；writable＝可玩（忙碌、唯讀、待修復都不是） */
  enterTable: (id: string) => Promise<{ entered: boolean; writable: boolean }>;
  /** 進出桌互斥：會重進本桌的入口（分岔、退回）要拿到鎖才動；拿不到就什麼都不做 */
  runTableOp: <T>(fn: () => Promise<T>) => Promise<T | undefined>;
  closeMainView: () => void;
  onError: (message: string) => void;
}

/** 換幕類動作失敗：世界書觸發紀錄結算不了就先問要不要重設（重設成了不必再顯示錯誤），其餘照常顯示。 */
async function reportSceneError(worldId: string, reason: unknown, onError: (message: string) => void) {
  // 換幕、分岔、退幕前的結算可能已把沒還原的變數寫入記進待回報檔（成功時重進桌會提示）；
  // 先提示完再問重設，兩個原生對話框不同時跳
  await showWorldInfoNotices(worldId);
  if (offersWorldInfoReset(reason)) {
    try {
      if (await askWorldInfoReset(worldId)) return;
    } catch (resetError) {
      onError(String(resetError));
      return;
    }
  }
  onError(String(reason));
}

/** 卡片介面上的換幕：換幕會重進本桌（重進一律收起介面），換成而且可以接著玩才把介面打開回來 */
export async function advanceThenReopen(advance: () => Promise<boolean>, reopen: () => void): Promise<void> {
  if (await advance()) reopen();
}

export function useSceneActions({
  worldId,
  scene,
  sceneTitles,
  sceneLabels,
  tableName,
  chat,
  canLeaveEditor,
  enterTable,
  runTableOp,
  closeMainView,
  onError,
}: SceneActionsOptions) {
  // 剛換完幕、這一幕只有那則前情提要＝還沒開始玩，兩條補救路都還來得及。
  // 一有新內容就作廢（同復原疊的道理：位置已被後話蓋掉，退回會連新句子一起丟）。
  // 分岔來的幕排除掉：那一則是複製來的真實對話，重寫會直接把它蓋成摘要
  const canUndoScene =
    scene > 0 &&
    chat.events.length === 1 &&
    !chat.busy &&
    !sceneLabels[String(scene)]?.forked;

  // 換場：把目前場景公開紀錄壓成一則前情提要，寫進新場景開頭，current_scene +1。
  // 回傳 true＝換成、而且重進本桌後可以接著玩（卡片介面上的換幕鈕靠它決定要不要把介面重新打開）；
  // 重進桌被擋（忙碌）或進的是唯讀／待修復畫面都回 false
  async function advanceScene(): Promise<boolean> {
    const done = await runTableOp(async () => {
      // 標題列不隨主欄畫面收起，編輯角色卡時這顆鈕照樣按得到
      if (!(await canLeaveEditor())) return false;
      // 確認框等人作答期間對話可能已經開跑，看當下不看舊閉包
      if (chat.isBusy()) return false;
      onError("");
      const turnId = chat.beginNarration();
      try {
        await invoke<number>("advance_scene", { worldId, turnId });
        const { entered, writable } = await enterTable(worldId);
        return entered && writable;
      } catch (reason) {
        // 玩家自己按了停止：紀錄沒動，不必再跳錯誤
        if (backendCode(reason) !== "scene_summary_stopped") await reportSceneError(worldId, reason, onError);
        return false;
      } finally {
        chat.endNarration();
      }
    });
    return done === true;
  }

  // 從前幕分岔續玩：把那一幕的紀錄複製成新的一幕，原本的歷史原封不動。
  // 整幕複製會讓下一次生成要送的內容變多，所以先跳確認框讓玩家自己決定
  async function forkScene(from: number) {
    await runTableOp(async () => {
      const accepted = await confirm(t("sceneForkConfirm"), {
        title: t("sceneForkTitle"),
        kind: "warning",
        okLabel: t("sceneForkTitle"),
        cancelLabel: t("dialogCancel"),
      });
      if (!accepted || chat.isBusy()) return;
      onError("");
      try {
        await invoke<number>("fork_scene", { worldId, scene: from });
        closeMainView();
        await enterTable(worldId);
      } catch (reason) {
        await reportSceneError(worldId, reason, onError);
      }
    });
  }

  // 太早按到換幕的補救：這一幕還只有那則前情提要時，刪掉它退回上一幕接著玩。
  // 前幕紀錄從來沒被動過（換幕只是開新檔），所以退回不會掉任何內容
  async function revertScene() {
    await runTableOp(async () => {
      if (!canUndoScene || chat.isBusy()) return;
      onError("");
      try {
        await invoke<number>("revert_scene", { worldId });
        await enterTable(worldId);
      } catch (reason) {
        await reportSceneError(worldId, reason, onError);
      }
    });
  }

  // 前情提要不滿意就重寫一份。拿前幕原始紀錄重跑一次摘要，蓋掉這幕唯一那則
  async function regenerateSummary() {
    await runTableOp(async () => {
      if (!canUndoScene || chat.isBusy()) return;
      onError("");
      const turnId = chat.beginNarration();
      try {
        await invoke("regenerate_scene_summary", { worldId, turnId });
        await enterTable(worldId);
      } catch (reason) {
        if (backendCode(reason) !== "scene_summary_stopped") onError(String(reason));
      } finally {
        chat.endNarration();
      }
    });
  }

  // 存哪裡由使用者決定：跳原生「另存新檔」對話框，取消就什麼都不做
  async function exportTranscript() {
    onError("");
    try {
      const now = new Date();
      const pad = (n: number) => String(n).padStart(2, "0");
      const stamp = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}${pad(now.getMinutes())}`;
      const path = await saveDialog({
        defaultPath: `${t("exportFileName", { table: tableName, stamp })}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return;
      await invoke("export_transcript", { worldId, path });
      await revealItemInDir(path);
    } catch (reason) {
      onError(String(reason));
    }
  }

  // 幕的顯示編號：n 從 1 起算，內部場號 0 起算；分岔出來的幕編號跟著源頭走、後面掛版本號
  // （第 1 幕 (2)），沒進 scene_labels 的就是原線
  const sceneNumber = (n: number) => {
    const label = sceneLabels[String(n)];
    return { shown: (label?.base ?? n) + 1, v: label?.version ?? 1 };
  };

  // 幕的顯示標籤：有取到幕名就「第 n 幕：幕名」，沒有就沿用「第 n 幕」
  const sceneDisplayLabel = (n: number) => {
    const title = sceneTitles[String(n)];
    const { shown, v } = sceneNumber(n);
    if (v > 1) {
      return title
        ? t("sceneWithTitleVersioned", { n: shown, v, title })
        : t("sceneLabelVersioned", { n: shown, v });
    }
    return title ? t("sceneWithTitle", { n: shown, title }) : t("sceneLabel", { n: shown });
  };

  // 工具列幕晶片：同一套編號與版本，但不帶幕名——幕名留給故事欄的書籤，工具列要省寬度
  const sceneChipLabel = (n: number) => {
    const { shown, v } = sceneNumber(n);
    return v > 1 ? t("sceneLabelVersioned", { n: shown, v }) : t("sceneLabel", { n: shown });
  };

  return {
    canUndoScene,
    advanceScene,
    forkScene,
    revertScene,
    regenerateSummary,
    exportTranscript,
    sceneDisplayLabel,
    sceneChipLabel,
  };
}

export type SceneActions = ReturnType<typeof useSceneActions>;
