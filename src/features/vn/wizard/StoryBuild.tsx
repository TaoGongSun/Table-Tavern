// 精靈 ② 建立之後：進度列「建立世界 → 撰寫劇本」，聖經寫完露出劇本規格層（StorySpec）；寫不出來可重試或直接往下。
// 先等聖經讀回來再決定要不要生成：重開精靈回到 ② 時已有聖經就不重生（plans/vn-setup-wizard.md §2 上一步保存契約）。
import { useEffect, useRef, useState, type MutableRefObject } from "react";
import { t } from "../../../i18n";
import { backendText } from "../../../shared/ui/backend-text";
import { acknowledgeBibleFailure, bibleDraft, bibleErrorKey, useBible } from "../bible/useBible";
import { StorySpec } from "./StorySpec";

export type BuildState = "running" | "done" | "failed";

/** 「建立世界 → 撰寫劇本」：world＝展開建桌中、bible＝聖經生成中、done＝都好了 */
export function BuildProgress({ phase }: { phase: "world" | "bible" | "done" }) {
  const steps = [
    ["world", "vnBuildWorld"],
    ["bible", "vnBuildBible"],
  ] as const;
  const at = phase === "world" ? 0 : phase === "bible" ? 1 : 2;
  return (
    <ol className="vn-build-progress" aria-live="polite">
      {steps.map(([id, key], index) => (
        <li key={id} data-state={index < at ? "done" : index === at ? "running" : "pending"}>
          {t(key)}
        </li>
      ))}
    </ol>
  );
}

export function StoryBuild({
  worldId,
  spoilerLock,
  onState,
  onDirtyChange,
  flushRef,
}: {
  worldId: string;
  spoilerLock: boolean;
  onState: (state: BuildState) => void;
  /** 劇本規格層的未存改動（精靈換步、開始、關閉前先存） */
  onDirtyChange: (dirty: boolean) => void;
  flushRef: MutableRefObject<(() => Promise<boolean>) | null>;
}) {
  const { generate, view, fresh, error } = useBible(worldId);
  const [state, setState] = useState<BuildState>("running");
  const [message, setMessage] = useState("");
  const decided = useRef(false);
  // 關掉精靈後這次生成還在跑：失敗要留在名單上給大廳，不能在已卸載的畫面裡清掉
  const live = useRef(true);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);
  useEffect(() => onState(state), [state, onState]);

  function showFailure(reason: unknown) {
    const key = bibleErrorKey(reason);
    setMessage(key ? t(key) : backendText(reason));
    setState("failed");
    acknowledgeBibleFailure(worldId);
  }

  async function run() {
    setState("running");
    try {
      await generate(spoilerLock);
      if (live.current) setState("done");
    } catch (reason) {
      if (live.current) showFailure(reason);
    }
  }
  useEffect(() => {
    if (decided.current || !fresh || view === null) return;
    decided.current = true;
    const draft = bibleDraft(worldId);
    // 先處理這桌還在跑或還沒被看過的失敗；舊聖經留著也不算完成
    if (draft?.state === "failed") {
      showFailure(draft.error);
      return;
    }
    if (draft?.state === "running" || !view.bible) {
      void run();
      return;
    }
    setState("done");
  }, [fresh, view]);

  if (!fresh && error !== null) {
    return <p className="field-warn">{backendText(error)}</p>;
  }
  return (
    <div className="vn-story-build">
      <BuildProgress phase={state === "running" ? "bible" : "done"} />
      {state === "failed" && (
        <div className="vn-bible-failed">
          {message && <p>{message}</p>}
          <p>{t(view?.bible ? "vnBibleFailedKeep" : "vnBibleFailedStart")}</p>
          <button type="button" className="btn" onClick={() => void run()}>
            {t("vnBibleRetry")}
          </button>
        </div>
      )}
      {state === "done" && <StorySpec worldId={worldId} onDirtyChange={onDirtyChange} flushRef={flushRef} />}
    </div>
  );
}
