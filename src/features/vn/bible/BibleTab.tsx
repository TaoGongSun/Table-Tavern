// 世界編輯器的「劇本」分頁（plans/vn-mode-pkg4.md §2.6）；精靈 ② 的劇本規格層也用它。
// 章數上限、選擇程度與結局數不算劇透，露在遮罩外；其餘整頁在劇透區內，已揭露的秘密例外照列。改動下一幕生效。
import { confirm } from "@tauri-apps/plugin-dialog";
import { useEffect, useState, type MutableRefObject } from "react";
import { t } from "../../../i18n";
import type { Bible } from "../../../shared/contracts/backend-contracts";
import { backendText } from "../../../shared/ui/backend-text";
import { restoresCardBible } from "../card/vn-card";
import { BibleFields } from "./BibleFields";
import { SpoilerMask } from "./SpoilerMask";
import { bibleErrorKey, useBible } from "./useBible";

const flagValue = (value: string) =>
  value === "true" ? t("vnFlagOn") : value === "false" ? t("vnFlagOff") : value;

const CHOICES = [
  ["none", "vnChoiceNone"],
  ["light", "vnChoiceLight"],
  ["full", "vnChoiceFull"],
] as const;

export function BibleTab({
  worldId,
  onDirtyChange,
  setup = false,
  flushRef,
}: {
  worldId: string;
  /** 劇本草稿有沒有未存改動（世界編輯器的離開確認用） */
  onDirtyChange?: (dirty: boolean) => void;
  /** 精靈 ② 的劇本規格層：沒有儲存鈕（精靈換步、開始、關閉時經 flushRef 自動存） */
  setup?: boolean;
  /** 精靈換步、開始、關閉前呼叫：有未存改動就存，回存成功沒（失敗時草稿留著、訊息顯示在這裡） */
  flushRef?: MutableRefObject<(() => Promise<boolean>) | null>;
}) {
  const bible = useBible(worldId);
  const view = bible.view;
  const [draft, setDraft] = useState<Bible | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  // 以內容為鍵重設草稿：開關鎖、別處重讀換了物件但聖經沒變時不洗掉未存的改動
  const savedJson = JSON.stringify(view?.bible ?? null);
  useEffect(() => setDraft(JSON.parse(savedJson) as Bible | null), [savedJson]);
  const dirty = draft !== null && JSON.stringify(draft) !== savedJson;
  useEffect(() => onDirtyChange?.(dirty), [dirty, onDirtyChange]);

  async function act(work: () => Promise<void>, done?: string) {
    setBusy(true);
    setMessage(null);
    try {
      await work();
      if (done) setMessage(done);
    } catch (reason) {
      const key = bibleErrorKey(reason);
      setMessage(key ? t(key) : backendText(reason));
    } finally {
      setBusy(false);
    }
  }
  const regenerate = () =>
    view?.played ? Promise.reject(new Error("VN_BIBLE_PLAYED")) : bible.generate();
  // VN 卡建的桌：重骰改成恢復卡內原版（後端 vn_bible_generate 同一支，不呼叫模型）
  const restore = restoresCardBible(view);
  async function confirmRegenerate() {
    const label = t(restore ? "vnBibleRestore" : "vnBibleRegenerate");
    const ok = await confirm(t(restore ? "vnBibleRestoreConfirm" : "vnBibleRegenerateConfirm"), {
      title: label,
      kind: "warning",
      okLabel: label,
      cancelLabel: t("dialogCancel"),
    });
    if (ok) await act(regenerate);
  }

  const save = (next: Bible) => act(() => bible.save(next), t("saved"));
  if (flushRef) {
    flushRef.current = async () => {
      if (!dirty || !draft) return true;
      try {
        await bible.save(draft);
        return true;
      } catch (reason) {
        const key = bibleErrorKey(reason);
        setMessage(key ? t(key) : backendText(reason));
        return false;
      }
    };
  }

  if (!view) return <p>{t("vnBibleLoading")}</p>;
  if (!view.bible || !draft) {
    return (
      <section className="vn-bible-tab">
        <p>{t("vnBibleNone")}</p>
        <button type="button" className="btn" disabled={busy} onClick={() => void act(regenerate)}>
          {busy ? t("vnBibleGenerating") : t("vnBibleGenerate")}
        </button>
        {message && <p className="vn-spoiler-message">{message}</p>}
      </section>
    );
  }
  const titleOf = (id: string) => view.bible?.secrets.find((secret) => secret.id === id)?.title ?? id;
  return (
    <section className="vn-bible-tab">
      {!setup && (
        <div className="vn-bible-actions">
          <button
            type="button"
            className="btn btn-primary"
            disabled={!dirty || busy}
            onClick={() => void save(draft)}
          >
            {t("vnBibleSave")}
          </button>
          <span className="vn-bible-note">{t("vnBibleNextScene")}</span>
        </div>
      )}
      {message && <p className="vn-spoiler-message">{message}</p>}
      <label className="vn-bible-max">
        {t("vnChapterMax")}
        <input
          type="number"
          min={3}
          max={30}
          readOnly={draft.chapter_lock}
          value={draft.chapter_max}
          onChange={(event) =>
            !draft.chapter_lock &&
            setDraft({ ...draft, chapter_max: Math.min(30, Math.max(3, Number(event.currentTarget.value) || 3)) })
          }
        />
      </label>
      {draft.chapter_lock && <p className="vn-bible-note">{t("vnChapterLocked")}</p>}
      <label className="vn-bible-branching">
        {t("vnChoices")}
        <span className="vn-bible-branching-row">
          <select
            value={draft.branching}
            onChange={(event) => setDraft({ ...draft, branching: event.currentTarget.value as Bible["branching"] })}
          >
            {CHOICES.map(([id, key]) => (
              <option key={id} value={id}>
                {t(key)}
              </option>
            ))}
          </select>
          {/* 結局是照存檔時的程度寫的：改了程度就提示要重寫劇本，不拿舊結局數誤導 */}
          {draft.branching !== view.bible.branching
            ? t("vnBranchingRewriteHint")
            : draft.branching !== "none" &&
              t("vnEndingCount", { n: Math.max(1, draft.endings.filter((ending) => ending.id !== "default").length) })}
        </span>
      </label>
      {!bible.locked && draft.chapter_reason && <p className="vn-bible-note">{draft.chapter_reason}</p>}
      {view.revealed.length > 0 && (
        <div className="vn-bible-list">
          <h3>{t("vnRevealedList")}</h3>
          <ul>
            {view.revealed.map((item) => (
              <li key={item.id}>
                {titleOf(item.id)}
                {item.chapter !== null && `（${t("vnRevealedAt", { n: item.chapter })}）`}
              </li>
            ))}
          </ul>
        </div>
      )}
      <SpoilerMask
        worldId={worldId}
        onReroll={restore ? undefined : regenerate}
        title="vnBibleMaskTitle"
        rerollLabel="vnBibleMaskReroll"
      >
        <div className="vn-bible-list">
          <h3>{t("vnFlagValues")}</h3>
          <ul>
            {view.bible.flags.map((flag) => (
              <li key={flag.id}>
                {flag.label || flag.id}：{flagValue(view.flagValues[flag.id] ?? flag.initial)}
              </li>
            ))}
          </ul>
        </div>
        <BibleFields draft={draft} names={view.names} onChange={setDraft} />
        <button type="button" className="btn" disabled={busy} onClick={() => void confirmRegenerate()}>
          {t(restore ? "vnBibleRestore" : "vnBibleRegenerate")}
        </button>
      </SpoilerMask>
    </section>
  );
}
