// 章末字卡（vn-mode 包 5 §2.3）：「第 N 章 完」→ 換幕中 → 「第 N+1 章」；失敗顯示原因與重試。
import { t } from "../../i18n";
import { IconStop } from "../../shared/ui/icons";
import { AiErrorText } from "../../shared/ui/atoms";
import type { ChapterCard } from "./useChapterEnd";
import "./vn-story.css";

export function ChapterCardView({
  card,
  nextChapter,
  onRetry,
  onClose,
  onStop,
  replay = false,
  backgroundState,
}: {
  card: ChapterCard;
  backgroundState?: string;
  /** 重進後新幕首則事件的章號；與結束的章相同＝轉收尾（尾聲） */
  nextChapter: number | undefined;
  onRetry: () => void;
  onClose: () => void;
  /** 下一章首拍生成中：字卡蓋著快捷列，停止鈕放在字卡上 */
  onStop?: () => void;
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
      {!replay && card.phase === "advancing" && (
        <p className="vn-chapter-note">{t("vnSceneTidying")}</p>
      )}
      {!replay && opening && <p className="vn-chapter-note">{t(backgroundState === "queued" || backgroundState === "running" ? "vnChapterOpeningBg" : "vnChapterOpening")}</p>}
      {!replay && opening && (backgroundState === "paused" || backgroundState === "failed") && <p className="vn-chapter-note">{t(backgroundState === "paused" ? "vnBackgroundQuota" : "vnBackgroundFailed")}</p>}
      {card.phase === "reenter" && <p className="vn-chapter-note">{t("vnReenterFailed")}</p>}
      {card.phase === "failed" && card.error && (
        <p className="vn-chapter-note" role="alert">
          <AiErrorText text={card.error} />
        </p>
      )}
      {opening && onStop && (
        <button type="button" className="btn" onClick={onStop}>
          <IconStop />
          {t("stopResponse")}
        </button>
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
