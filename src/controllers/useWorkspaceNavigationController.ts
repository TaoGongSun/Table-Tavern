import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// 發言對象是 GM 時 speaker 存這個代號（純前端狀態，不會寫進紀錄）；GM 以旁白回應
export const GM_TARGET = "__GM__";

export type MainViewState =
  | { kind: "scene"; n: number }
  | { kind: "character"; id: string }
  | { kind: "new-character"; id: string }
  | { kind: "player"; id: string }
  | { kind: "new-player"; id: string }
  | { kind: "world" }
  | null;

interface NavigationCharacters {
  player: { id: string } | null;
  refresh: () => Promise<unknown>;
  reloadPlayer: (playerCardId: string | null) => Promise<void>;
  noteRemoved: (removedId: string, speaker: string) => Promise<string | null>;
  remove: (id: string) => Promise<boolean>;
  removePlayer: (id: string) => Promise<boolean>;
}

interface WorkspaceNavigationControllerOptions {
  worldId: string;
  characters: NavigationCharacters;
  onError: (message: string) => void;
}

export function useWorkspaceNavigationController({
  worldId,
  characters,
  onError,
}: WorkspaceNavigationControllerOptions) {
  // 角色卡編輯器每次 render 掛上「可以離開嗎」；任何會換掉編輯畫面的入口都先問它
  const leaveGuard = useRef<(() => Promise<boolean>) | null>(null);
  // 守門有入口落在 controller 的 callback 裡（匯入路由框的「開新桌並匯入」），那些閉包不隨
  // 主欄畫面重建，走 ref 才問得到當下這個畫面（比照 currentConfigRef）
  const canLeaveRef = useRef<() => Promise<boolean>>(async () => true);
  const [speaker, setSpeaker] = useState("");
  // 主欄下半部（messages＋composer）三選一整面取代：單幕閱讀／角色卡編輯／GM 世界設定編輯
  // （使用者拍板改版：需求 4 不用 modal，與需求 3 單幕閱讀同一套「整面取代」模式）
  const [mainView, setMainView] = useState<MainViewState>(null);

  // 四種卡片編輯畫面都帶 id，先收斂成一個值，下面就只需要問「是不是玩家卡」
  const cardView =
    mainView?.kind === "character" ||
    mainView?.kind === "new-character" ||
    mainView?.kind === "player" ||
    mainView?.kind === "new-player"
      ? mainView
      : null;
  const editingPlayerCard = cardView?.kind === "player" || cardView?.kind === "new-player";
  // 發言對象可能是 GM（沒有角色卡）：送出走旁白那條，晶片的名字與顏色也另一套
  const gmTargeted = speaker === GM_TARGET;

  // 建卡或改名存檔後：名單與圖片重載；id 全程不變，只有「新卡剛存下」要轉正畫面並選為發言對象
  async function finishCardSaved(id: string) {
    const wasNew = mainView?.kind === "new-character";
    await characters.refresh();
    if (wasNew) {
      setMainView({ kind: "character", id });
      setSpeaker(id);
    }
  }

  // 玩家卡是桌的屬性：第一次存檔才把 id 掛進 state，之後存檔只重載顯示（比照角色卡留在編輯器）
  async function finishPlayerCardSaved(id: string) {
    if (mainView?.kind === "new-player") {
      await invoke("set_player_card", { worldId, cardId: id });
      setMainView({ kind: "player", id });
    }
    await characters.reloadPlayer(id);
  }

  // 角色被隱藏或刪除後的善後：名單重載（controller）、發言對象改人、關掉編輯面板
  async function finishRemoval(id: string) {
    const nextSpeaker = await characters.noteRemoved(id, speaker);
    if (nextSpeaker !== null) setSpeaker(nextSpeaker);
    setMainView(null);
  }

  // 離開目前的編輯畫面前先問未儲存：換編輯對象、選發言對象、回大廳、換桌都走這一關
  async function canLeaveEditor() {
    // 只有掛著未儲存追蹤的三種畫面要問；聊天／幕紀錄沒有暫存狀態，也避免問到已卸載編輯器留下的舊守門
    const guarded = cardView !== null || mainView?.kind === "world";
    if (!guarded) return true;
    const ok = (await leaveGuard.current?.()) ?? true;
    // 放行就清掉，下一張卡載好會重新掛上——免得載入空窗期沿用上一張的未儲存狀態
    if (ok) leaveGuard.current = null;
    return ok;
  }
  // 匯入 controller 的 callback 可能跨 render 存活；每次 render 都指向最新守門 closure。
  canLeaveRef.current = canLeaveEditor;

  async function editCard(id: string) {
    if (mainView?.kind === "character" && mainView.id === id) return;
    if (await canLeaveEditor()) setMainView({ kind: "character", id });
  }

  async function openPlayerCard() {
    if (mainView?.kind === "player" || mainView?.kind === "new-player") return;
    if (!(await canLeaveEditor())) return;
    if (characters.player) {
      setMainView({ kind: "player", id: characters.player.id });
      return;
    }
    try {
      const id = await invoke<string>("new_id");
      setMainView({ kind: "new-player", id });
    } catch (reason) {
      onError(String(reason));
    }
  }

  // 點卡＝選發言對象，編輯是另一顆鈕。遊玩畫面再點一次已選中的卡＝取消對象，
  // 讓玩家能描述動作或對全場說話；人在編輯頁或前幕閱讀時，點卡＝先過未儲存守門、
  // 回到遊玩畫面再選為對象（不是取消，也不是轉去編輯）
  async function selectSpeaker(target: string) {
    if (mainView) {
      if (!(await canLeaveEditor())) return;
      setMainView(null);
      setSpeaker(target);
      return;
    }
    setSpeaker((current) => (current === target ? "" : target));
  }

  const selectCard = (id: string) => selectSpeaker(id);
  const selectGm = () => selectSpeaker(GM_TARGET);

  async function openWorldEditor() {
    if (mainView?.kind === "world") return;
    if (await canLeaveEditor()) setMainView({ kind: "world" });
  }

  // 建卡先跟後端要一個 id：草稿期生圖就落在正確的圖庫目錄，存檔用同一個 id
  async function openNewCard() {
    if (mainView?.kind === "new-character") return;
    if (!(await canLeaveEditor())) return;
    try {
      const id = await invoke<string>("new_id");
      setMainView({ kind: "new-character", id });
    } catch (reason) {
      onError(String(reason));
    }
  }

  // 幕晶片選單點一幕＝整面換成單幕閱讀，一樣會蓋掉編輯畫面
  async function openSceneReader(n: number) {
    if (!(await canLeaveEditor())) return;
    setMainView({ kind: "scene", n });
  }

  // 隱藏區與角色卡編輯畫面共用同一條刪除路徑：確認框與刪檔在 controller，這裡接善後
  async function deleteCharacter(id: string) {
    try {
      if (await characters.remove(id)) await finishRemoval(id);
    } catch (reason) {
      onError(String(reason));
    }
  }

  async function deletePlayerCard(id: string) {
    if (await characters.removePlayer(id)) setMainView(null);
  }

  return {
    leaveGuard,
    canLeaveRef,
    speaker,
    setSpeaker,
    mainView,
    setMainView,
    cardView,
    editingPlayerCard,
    gmTargeted,
    canLeaveEditor,
    editCard,
    openPlayerCard,
    selectCard,
    selectGm,
    openWorldEditor,
    openNewCard,
    openSceneReader,
    deleteCharacter,
    deletePlayerCard,
    finishCardSaved,
    finishPlayerCardSaved,
    finishRemoval,
  };
}

export type WorkspaceNavigationController = ReturnType<typeof useWorkspaceNavigationController>;
