// 章末字卡（vn-mode 包 5 §2.3）：「第 N 章 完」→ 換幕中 → 「第 N+1 章」；失敗顯示原因與重試。
import { t } from "../../i18n";
import { backendText } from "../../shared/ui/backend-text";
import type { ChapterCard } from "./useChapterEnd";
import "./vn-story.css";

export function ChapterCardView({
  card,
  nextChapter,
  onRetry,
  onClose,
  replay = false,
}: {
  card: ChapterCard;
  /** 重進後新幕首則事件的章號；與結束的章相同＝轉收尾（尾聲） */
  nextChapter: number | undefined;
  onRetry: () => void;
  onClose: () => void;
  /** 重播桌只是換幕、沒有在整理，不顯示「整理中」 */
  replay?: boolean;
}) {
  const opening = card.phase === "opening";
  const heading = opening
    ? card.chapterEnd && nextChapter !== undefined
      ? nextChapter > card.chapter
        ? t("vnChapterStart", { n: nextChapter })
        : t("vnFinalBeat")
      : null
    : card.chapterEnd
      ? t("vnChapterDone", { n: card.chapter })
      : null;
  const stuck = card.phase === "failed" || card.phase === "reenter";
  return (
    <div className="vn-chapter-card" role="status">
      {heading && <p className="vn-chapter-heading">{heading}</p>}
      {!replay && (card.phase === "advancing" || (opening && !heading)) && (
        <p className="vn-chapter-note">{t("vnSceneTidying")}</p>
      )}
      {card.phase === "reenter" && <p className="vn-chapter-note">{t("vnReenterFailed")}</p>}
      {card.phase === "failed" && card.error && (
        <p className="vn-chapter-note" role="alert">
          {backendText(card.error)}
        </p>
      )}
      {stuck && (
        <div className="vn-chapter-actions">
          <button type="button" className="btn btn-primary" onClick={onRetry}>
            {t("vnChapterRetry")}
          </button>
          <button type="button" className="btn" onClick={onClose}>
            {t("vnChapterClose")}
          </button>
        </div>
      )}
    </div>
  );
}
