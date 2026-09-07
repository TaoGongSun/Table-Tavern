import { invoke } from "@tauri-apps/api/core";
import { confirm, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { AppConfig, SceneLabel, TranscriptEvent } from "../shared/contracts/backend-contracts";
import { t } from "../i18n";

interface SceneChatActions {
  events: TranscriptEvent[];
  busy: boolean;
  beginNarration: () => void;
  endNarration: () => void;
  noteTurnDone: () => void;
}

interface SceneActionsOptions {
  worldId: string;
  scene: number;
  sceneTitles: Record<string, string>;
  sceneLabels: Record<string, SceneLabel>;
  tableName: string;
  chat: SceneChatActions;
  config: AppConfig | null;
  canLeaveEditor: () => Promise<boolean>;
  enterTable: (id: string, loaded: AppConfig) => Promise<void>;
  closeMainView: () => void;
  onError: (message: string) => void;
}

export function useSceneActions({
  worldId,
  scene,
  sceneTitles,
  sceneLabels,
  tableName,
  chat,
  config,
  canLeaveEditor,
  enterTable,
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

  // 換場：把目前場景公開紀錄壓成一則前情提要，寫進新場景開頭，current_scene +1
  async function advanceScene() {
    // 標題列不隨主欄畫面收起，編輯角色卡時這顆鈕照樣按得到
    if (!(await canLeaveEditor())) return;
    onError("");
    chat.beginNarration();
    try {
      await invoke<number>("advance_scene", { worldId });
      await enterTable(worldId, config!);
      chat.noteTurnDone();
    } catch (reason) {
      onError(String(reason));
    } finally {
      chat.endNarration();
    }
  }

  // 從前幕分岔續玩：把那一幕的紀錄複製成新的一幕，原本的歷史原封不動。
  // 整幕複製會讓下一次生成要送的內容變多，所以先跳確認框讓玩家自己決定
  async function forkScene(from: number) {
    const accepted = await confirm(t("sceneForkConfirm"), {
      title: t("sceneForkTitle"),
      kind: "warning",
    });
    if (!accepted) return;
    onError("");
    try {
      await invoke<number>("fork_scene", { worldId, scene: from });
      closeMainView();
      await enterTable(worldId, config!);
    } catch (reason) {
      onError(String(reason));
    }
  }

  // 太早按到換幕的補救：這一幕還只有那則前情提要時，刪掉它退回上一幕接著玩。
  // 前幕紀錄從來沒被動過（換幕只是開新檔），所以退回不會掉任何內容
  async function revertScene() {
    if (!canUndoScene) return;
    onError("");
    try {
      await invoke<number>("revert_scene", { worldId });
      await enterTable(worldId, config!);
    } catch (reason) {
      onError(String(reason));
    }
  }

  // 前情提要不滿意就重寫一份。拿前幕原始紀錄重跑一次摘要，蓋掉這幕唯一那則
  async function regenerateSummary() {
    if (!canUndoScene) return;
    onError("");
    chat.beginNarration();
    try {
      await invoke("regenerate_scene_summary", { worldId });
      await enterTable(worldId, config!);
      chat.noteTurnDone();
    } catch (reason) {
      onError(String(reason));
    } finally {
      chat.endNarration();
    }
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

  // 幕的顯示標籤：有取到幕名就「第 n 幕：幕名」，沒有就沿用「第 n 幕」；n 從 1 起算，內部場號 0 起算。
  // 分岔出來的幕顯示編號跟著源頭走、後面掛版本號（第 1 幕 (2)），沒進 scene_labels 的就是原線
  const sceneDisplayLabel = (n: number) => {
    const title = sceneTitles[String(n)];
    const label = sceneLabels[String(n)];
    const shown = (label?.base ?? n) + 1;
    const v = label?.version ?? 1;
    if (v > 1) {
      return title
        ? t("sceneWithTitleVersioned", { n: shown, v, title })
        : t("sceneLabelVersioned", { n: shown, v });
    }
    return title ? t("sceneWithTitle", { n: shown, title }) : t("sceneLabel", { n: shown });
  };

  return {
    canUndoScene,
    advanceScene,
    forkScene,
    revertScene,
    regenerateSummary,
    exportTranscript,
    sceneDisplayLabel,
  };
}

export type SceneActions = ReturnType<typeof useSceneActions>;
