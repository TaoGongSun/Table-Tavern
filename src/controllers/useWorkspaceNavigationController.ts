import { useCallback, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { EntryChanges, WorldChange } from "../features/worldbook/worldbook-model";

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

/** 這個畫面是不是正在顯示這張卡（四種卡片編輯畫面） */
function showsCard(view: MainViewState, id: string) {
  return (
    (view?.kind === "character" ||
      view?.kind === "new-character" ||
      view?.kind === "player" ||
      view?.kind === "new-player") &&
    view.id === id
  );
}

interface NavigationCharacters {
  player: { id: string } | null;
  refresh: () => Promise<unknown>;
  reloadPlayer: (playerCardId: string | null) => Promise<void>;
  noteRemoved: (removedId: string, speaker: string) => Promise<string | null>;
  remove: (id: string) => Promise<"cancelled" | "deleted" | "failed">;
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
  // 確認框、刪檔都是 await：關不關編輯器要看當下的畫面，不吃舊閉包
  const mainViewRef = useRef(mainView);
  mainViewRef.current = mainView;
  // 進桌世代：每換一次桌（含 w1→w2→w1）遞增。這一輪 render 的閉包記下當時的世代，刪除、善後的
  // 非同步續程回來時世代已變＝玩家換桌了，不得再動目前這桌的發言對象、畫面與通知
  const entry = useRef({ worldId, generation: 0 });
  if (entry.current.worldId !== worldId) {
    entry.current = { worldId, generation: entry.current.generation + 1 };
  }
  const enteredAt = entry.current.generation;
  const stillHere = () => entry.current.generation === enteredAt;
  const [worldChange, setWorldChange] = useState<WorldChange | null>(null);

  /** 世界設定通知：同桌就累積到被取走為止，換桌重來。只用 setter，身分穩定（匯入 controller 的 callback 吃它） */
  const postWorldChange = useCallback(
    (target: string, ids: string[], entryChanges: (EntryChanges | null)[]) => {
      setWorldChange((current) => {
        const same = current?.worldId === target;
        return {
          worldId: target,
          generation: (current?.generation ?? 0) + 1,
          ids: same ? [...current.ids, ...ids] : ids,
          entryChanges: same ? [...current.entryChanges, ...entryChanges] : entryChanges,
        };
      });
    },
    [],
  );

  /** 匯入寫完世界書：通知世界設定重讀清單；擴大了可見度的既有條目帶 visibility 組（開著的草稿據此定基）。
   * 一律用傳入的桌 id——開新桌並匯入時 controller 手上是舊閉包，閉包裡的桌 id 是原桌 */
  const noteImported = useCallback(
    (target: string, widened: number[]) => {
      postWorldChange(
        target,
        [],
        widened.length > 0
          ? [{ changed: widened.map((uid) => ({ uid, groups: ["visibility"] })), deleted: [] }]
          : [],
      );
    },
    [postWorldChange],
  );

  /** 只關正在顯示其中一張卡的編輯器；實際關了才清守門 */
  function closeCardViews(ids: string[]) {
    if (ids.some((id) => showsCard(mainViewRef.current, id))) leaveGuard.current = null;
    setMainView((current) => (ids.some((id) => showsCard(current, id)) ? null : current));
  }

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

  // 角色被隱藏、刪除或轉成條目後的善後：名單重載、發言對象改人；只關正在顯示這張卡的編輯器，
  // 別的畫面（世界設定、別張卡、新卡草稿）原樣留著〔作者裁決 2026-10-11〕。卡確定已不在才發刪除通知，
  // 開著的世界設定靠它重讀。名單善後失敗不擋關卡與通知。
  async function settleRemoval(id: string, deleted: boolean) {
    if (!stillHere()) return;
    try {
      const nextSpeaker = await characters.noteRemoved(id, speaker);
      if (!stillHere()) return;
      if (nextSpeaker !== null) setSpeaker(nextSpeaker);
    } catch (reason) {
      if (!stillHere()) return;
      onError(String(reason));
    }
    closeCardViews([id]);
    if (deleted) postWorldChange(worldId, [id], []);
  }

  /** 撤銷上次匯入完成：被撤銷刪掉的卡若正在編輯就關、不另問〔作者裁決 2026-10-11〕，別的畫面不動；
   * 世界設定一律收到通知（撤銷可能只改條目）。換桌後晚到就不動 */
  function finishUndoImport(removedIds: string[], entryChanges: EntryChanges | null) {
    if (!stillHere()) return;
    closeCardViews(removedIds);
    postWorldChange(worldId, removedIds, [entryChanges]);
  }

  const finishArchiveToggled = (id: string) => settleRemoval(id, false);
  const finishConverted = (id: string) => settleRemoval(id, true);

  /** 世界設定取走通知：同桌同世代才清累積，取走後又來的新通知不受影響 */
  function takeWorldChange(takenWorldId: string, generation: number) {
    setWorldChange((current) =>
      current?.worldId === takenWorldId && current.generation === generation
        ? { ...current, ids: [], entryChanges: [] }
        : current,
    );
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

  /** 回傳世界設定是否開著（玩家在未存確認選了留下就是 false） */
  async function openWorldEditor(): Promise<boolean> {
    if (mainView?.kind === "world") return true;
    if (!(await canLeaveEditor())) return false;
    setMainView({ kind: "world" });
    return true;
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
      if ((await characters.remove(id)) === "deleted") await settleRemoval(id, true);
    } catch (reason) {
      if (stillHere()) onError(String(reason));
    }
  }

  async function deletePlayerCard(id: string) {
    if ((await characters.removePlayer(id)) && stillHere()) setMainView(null);
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
    finishArchiveToggled,
    finishConverted,
    finishUndoImport,
    noteImported,
    worldChange,
    takeWorldChange,
  };
}

export type WorkspaceNavigationController = ReturnType<typeof useWorkspaceNavigationController>;
