// 結局一覽、章節選擇與進度頁：蓋在播放器上的暗幕面板。
// 進度頁收章節重玩、旗標、已揭露秘密與結局；章節選擇與結局一覽本身留給完結畫面與重播桌。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import type { VnEndings } from "../../shared/contracts/backend-contracts";
import { flagValueText, useBible } from "./bible/useBible";
import "./vn-story.css";

/** 沒讀到聖經時的章數上限，與後端 `CHAPTER_FALLBACK` 一致 */
const CHAPTER_FALLBACK = 8;

/** 結局表＋本桌任一分支達成過的（`vn_endings`）；`key` 變了就重讀 */
export function useEndings(worldId: string, key: string) {
  const [endings, setEndings] = useState<VnEndings | null>(null);
  useEffect(() => {
    let live = true;
    invoke<VnEndings>("vn_endings", { worldId })
      .then((value) => live && setEndings(value))
      .catch(() => live && setEndings(null));
    return () => {
      live = false;
    };
  }, [worldId, key]);
  return endings;
}

/** 已達成 x／y 與結局一覽只算正式結局：保底結局（`default`）不算〔作者裁決 2026-11-02〕 */
export const countedEndings = (endings: VnEndings) => endings.endings.filter((ending) => ending.id !== "default");

/** 「已達成 x／y」；沒有正式結局（沒有聖經）回 null 不顯示 */
export function reachedText(endings: VnEndings): string | null {
  const counted = countedEndings(endings);
  if (counted.length === 0) return null;
  const x = counted.filter((ending) => ending.reached).length;
  return t("vnEndingsReached", { x, y: counted.length });
}

export function Panel({
  label,
  children,
  onClose,
  className,
  closeFirst,
}: {
  label: string;
  children: React.ReactNode;
  onClose: () => void;
  className?: string;
  /** 內容會長到要捲動的面板：關閉鈕放標題正下方，不必捲到底 */
  closeFirst?: boolean;
}) {
  const close = (
    <div className="vn-chapter-actions">
      <button type="button" className="btn" onClick={onClose}>
        {t("vnChapterClose")}
      </button>
    </div>
  );
  return (
    <div className={`vn-story-panel${className ? ` ${className}` : ""}`} role="dialog" aria-label={label}>
      <p className="vn-chapter-heading">{label}</p>
      {closeFirst && close}
      {children}
      {!closeFirst && close}
    </div>
  );
}

/**
 * 遊戲選單的進度：章節重玩、關鍵事件、已揭露秘密、結局。
 * 旗標說明與現值不算劇透（格邊提示鎖住時本來就顯示），不列觸發情境與未揭露的秘密，所以不包遮罩。
 * 沒有聖經就不出關鍵事件；章數分母沿用後端 fallback，讀回聖經後改用 `chapter_max`。
 */
export function ProgressPanel({
  worldId,
  chapter,
  disabled,
  onPick,
  onClose,
}: {
  worldId: string;
  chapter: number;
  disabled: boolean;
  onPick: (chapter: number) => void;
  onClose: () => void;
}) {
  const bibleState = useBible(worldId);
  const view = bibleState.view;
  const script = view?.bible ?? null;
  const endings = useEndings(worldId, "");
  const titleOf = (id: string) => script?.secrets.find((secret) => secret.id === id)?.title ?? id;
  const reached = endings ? reachedText(endings) : null;
  return (
    <Panel label={t("vnProgressPanel")} onClose={onClose}>
      <div>
        <p className="vn-story-heading">
          {t("vnStoryChapter", { n: chapter, m: script?.chapter_max || CHAPTER_FALLBACK })}
        </p>
        <div className="vn-story-list">
          {Array.from({ length: chapter }, (_, index) => index + 1).map((n) => (
            <div key={n} className="vn-progress-row">
              <span>{t("vnChapterStart", { n })}</span>
              <button type="button" className="btn" disabled={disabled} onClick={() => onPick(n)}>
                {t("vnReplayFromHere")}
              </button>
            </div>
          ))}
        </div>
      </div>
      {script && (
        <div>
          <p className="vn-story-heading">{t("vnKeyEvents")}</p>
          <ul className="vn-story-list">
            {script.flags.map((flag) => (
              <li key={flag.id}>
                {flag.label || flag.id}：{flagValueText(view?.flagValues[flag.id] ?? flag.initial)}
              </li>
            ))}
          </ul>
        </div>
      )}
      {view && view.revealed.length > 0 && (
        <div>
          <p className="vn-story-heading">{t("vnRevealedList")}</p>
          <ul className="vn-story-list">
            {view.revealed.map((item) => (
              <li key={item.id}>
                {titleOf(item.id)}
                {item.chapter !== null && `（${t("vnRevealedAt", { n: item.chapter })}）`}
              </li>
            ))}
          </ul>
        </div>
      )}
      {endings && countedEndings(endings).length > 0 && (
        <div>
          <p className="vn-story-heading">{t("vnEndingsList")}</p>
          {reached && <p className="vn-chapter-note">{reached}</p>}
          <ol className="vn-story-list">
            {countedEndings(endings).map((ending) => (
              <li key={ending.id} className={ending.reached ? undefined : "vn-unreached"}>
                {ending.reached ? ending.title || t("vnTheEnd") : "???"}
              </li>
            ))}
          </ol>
        </div>
      )}
    </Panel>
  );
}

/** 已達成顯示標題（沒標題時顯示「完」），未達成「???」；劇透鎖住時也一樣 */
export function EndingsPanel({ worldId, onClose }: { worldId: string; onClose: () => void }) {
  const endings = useEndings(worldId, "");
  return (
    <Panel label={t("vnEndingsList")} onClose={onClose}>
      {endings && (
        <>
          {reachedText(endings) && <p className="vn-chapter-note">{reachedText(endings)}</p>}
          <ol className="vn-story-list">
            {countedEndings(endings).map((ending) => (
              <li key={ending.id} className={ending.reached ? undefined : "vn-unreached"}>
                {ending.reached ? ending.title || t("vnTheEnd") : "???"}
              </li>
            ))}
          </ol>
        </>
      )}
    </Panel>
  );
}

/** 第 1 章到目前這章；選了就交給 `onPick` */
export function ChapterPanel({
  label = t("vnReplayPick"),
  chapter,
  disabled,
  onPick,
  onClose,
}: {
  /** 重播桌換成「從哪一章看起」 */
  label?: string;
  chapter: number;
  disabled: boolean;
  onPick: (chapter: number) => void;
  onClose: () => void;
}) {
  return (
    <Panel label={label} onClose={onClose}>
      <div className="vn-story-list">
        {Array.from({ length: chapter }, (_, index) => index + 1).map((n) => (
          <button key={n} type="button" className="btn" disabled={disabled} onClick={() => onPick(n)}>
            {t("vnChapterStart", { n })}
          </button>
        ))}
      </div>
    </Panel>
  );
}
