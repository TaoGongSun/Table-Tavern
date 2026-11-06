// VN 劇本聖經的前端狀態（plans/vn-mode-pkg4.md §2.5–§2.6）：劇透鎖是桌級的，同一桌開著的遮罩（私設、
// GM 條目、劇本分頁、精靈 ②）共用同一份讀回，任何一處解鎖、重骰、存檔都通知其他處重讀。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Bible, BibleView } from "../../../shared/contracts/backend-contracts";

const views = new Map<string, BibleView>();
const listeners = new Set<(worldId: string) => void>();

/** 這次執行裡的聖經生成。不落檔：關掉 app 這份名單就沒了，被中斷的工作不會一直顯示撰寫中。 */
type BibleJob =
  | { state: "running"; promise: Promise<void> }
  | { state: "failed"; error: unknown };

const jobs = new Map<string, BibleJob>();
const jobListeners = new Set<() => void>();

function notifyJobs() {
  jobListeners.forEach((listener) => listener());
}

export type BibleDraft = { state: "running" } | { state: "failed"; error: unknown };

/** 大廳色帶與精靈 ② 共用：正在生成，或同一次執行內失敗還沒被精靈顯示過 */
export function bibleDraft(worldId: string): BibleDraft | null {
  const job = jobs.get(worldId);
  if (!job) return null;
  return job.state === "running" ? { state: "running" } : { state: "failed", error: job.error };
}

export function useBibleDraft(worldId: string): BibleDraft | null {
  const [draft, setDraft] = useState<BibleDraft | null>(() => bibleDraft(worldId));
  useEffect(() => {
    const sync = () => setDraft(bibleDraft(worldId));
    jobListeners.add(sync);
    sync();
    return () => {
      jobListeners.delete(sync);
    };
  }, [worldId]);
  return draft;
}

/** 失敗已經畫在還開著的畫面上就清掉；發起的畫面已卸載才留著給大廳 */
export function acknowledgeBibleFailure(worldId: string) {
  if (jobs.get(worldId)?.state !== "failed") return;
  jobs.delete(worldId);
  notifyJobs();
}

/**
 * 同一桌同時只發一次 `vn_bible_generate`。進行中再呼叫回傳同一個 Promise；
 * 失敗留下 `{failed, error}`，明示重試才再發。
 */
export function generateBible(worldId: string, spoilerLock?: boolean): Promise<void> {
  const current = jobs.get(worldId);
  if (current?.state === "running") return current.promise;
  const promise = invoke("vn_bible_generate", { worldId, spoilerLock: spoilerLock ?? null }).then(
    () => {
      if (jobs.get(worldId)?.state === "running") jobs.delete(worldId);
      notifyJobs();
    },
    (error: unknown) => {
      jobs.set(worldId, { state: "failed", error });
      notifyJobs();
      throw error;
    },
  );
  jobs.set(worldId, { state: "running", promise });
  notifyJobs();
  return promise;
}

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
  // 生成失敗時：這份畫面還在，玩家會在這裡看到錯誤，色帶不用再留
  const mounted = useRef(false);
  const worldRef = useRef(worldId);
  worldRef.current = worldId;
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
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
      run(async () => {
        try {
          await generateBible(worldId, spoilerLock);
        } catch (reason) {
          if (mounted.current && worldRef.current === worldId) acknowledgeBibleFailure(worldId);
          throw reason;
        }
      }),
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
