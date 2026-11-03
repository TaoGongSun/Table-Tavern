// 世界編輯器的「劇本」分頁（plans/vn-mode-pkg4.md §2.6）；精靈 ② 生成完也用它顯示。
// 章數上限不算劇透，露在遮罩外；其餘整頁在劇透區內，已揭露的秘密例外照列。改動下一幕生效。
import { confirm } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { t } from "../../../i18n";
import type { Bible } from "../../../shared/contracts/backend-contracts";
import { backendText } from "../../../shared/ui/backend-text";
import { BibleFields } from "./BibleFields";
import { SpoilerMask } from "./SpoilerMask";
import { bibleErrorKey, useBible } from "./useBible";

const flagValue = (value: string) =>
  value === "true" ? t("vnFlagOn") : value === "false" ? t("vnFlagOff") : value;

export function BibleTab({
  worldId,
  onDirtyChange,
}: {
  worldId: string;
  /** 劇本草稿有沒有未存改動（世界編輯器的離開確認用） */
  onDirtyChange?: (dirty: boolean) => void;
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
  async function confirmRegenerate() {
    const ok = await confirm(t("vnBibleRegenerateConfirm"), {
      title: t("vnBibleRegenerate"),
      kind: "warning",
      okLabel: t("vnBibleRegenerate"),
      cancelLabel: t("dialogCancel"),
    });
    if (ok) await act(regenerate);
  }

  const save = (next: Bible) => act(() => bible.save(next), t("saved"));

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
      {message && <p className="vn-spoiler-message">{message}</p>}
      <label className="vn-bible-max">
        {t("vnChapterMax")}
        <input
          type="number"
          min={3}
          max={30}
          value={draft.chapter_max}
          onChange={(event) =>
            setDraft({ ...draft, chapter_max: Math.min(30, Math.max(3, Number(event.currentTarget.value) || 3)) })
          }
        />
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
      <SpoilerMask worldId={worldId} onReroll={regenerate}>
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
          {t("vnBibleRegenerate")}
        </button>
      </SpoilerMask>
    </section>
  );
}
