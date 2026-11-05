// VN 劇本聖經的前端狀態（plans/vn-mode-pkg4.md §2.5–§2.6）：劇透鎖是桌級的，同一桌開著的遮罩（私設、
// GM 條目、劇本分頁、精靈 ②）共用同一份讀回，任何一處解鎖、重骰、存檔都通知其他處重讀。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import type { Bible, BibleView } from "../../../shared/contracts/backend-contracts";

const views = new Map<string, BibleView>();
const listeners = new Set<(worldId: string) => void>();

async function reload(worldId: string): Promise<BibleView> {
  const view = await invoke<BibleView>("vn_bible_read", { worldId });
  views.set(worldId, view);
  listeners.forEach((listener) => listener(worldId));
  return view;
}

export function useBible(worldId: string) {
  const [view, setView] = useState<BibleView | null>(() => views.get(worldId) ?? null);
  const [error, setError] = useState<unknown>(null);
  // 這次掛載讀回過一次（快取可能是舊的）：要依現況決定動作的（精靈 ② 要不要生成）等它
  const [fresh, setFresh] = useState(false);
  useEffect(() => {
    setFresh(false);
    setView(views.get(worldId) ?? null);
    const listener = (changed: string) => {
      if (changed === worldId) setView(views.get(worldId) ?? null);
    };
    listeners.add(listener);
    reload(worldId)
      .then(() => setFresh(true))
      .catch(setError);
    return () => {
      listeners.delete(listener);
    };
  }, [worldId]);

  /** 跑一個會改聖經的動作，完成後（成敗都）重讀；錯誤原樣丟回呼叫端 */
  const run = useCallback(
    async (work: () => Promise<unknown>) => {
      setError(null);
      try {
        await work();
      } finally {
        await reload(worldId).catch(setError);
      }
    },
    [worldId],
  );

  return {
    view,
    fresh,
    error,
    /** 鎖住（沒讀回前也當鎖住，不先露出內容） */
    locked: view?.spoiler_lock ?? true,
    setLocked: (locked: boolean) => run(() => invoke("vn_spoiler_set", { worldId, locked })),
    generate: (spoilerLock?: boolean) =>
      run(() => invoke("vn_bible_generate", { worldId, spoilerLock: spoilerLock ?? null })),
    rerollSecrets: (characterId: string) =>
      run(() => invoke("vn_secret_reroll", { worldId, characterId })),
    save: (bible: Bible) => run(() => invoke("vn_bible_save", { worldId, bible })),
  };
}

/** 後端聖經錯誤碼 → i18n 鍵（`vn_bible.rs` 的 PLAYED／FAILED／MISSING／CHAPTER_LOCKED） */
export function bibleErrorKey(
  reason: unknown,
): "vnBiblePlayed" | "vnBibleFailed" | "vnBibleMissing" | "vnChapterLocked" | null {
  const text = String(reason);
  if (text.includes("VN_CHAPTER_LOCKED")) return "vnChapterLocked";
  if (text.includes("VN_BIBLE_PLAYED")) return "vnBiblePlayed";
  if (text.includes("VN_BIBLE_FAILED")) return "vnBibleFailed";
  if (text.includes("VN_BIBLE_MISSING")) return "vnBibleMissing";
  return null;
}
