// 完結畫面（vn-mode 包 5 §2.7、包 6 §2.8、包 8 §2.2）：「完」、結局標題、已達成 x／y；回標題（主鈕）、從某一章重玩、
// 開新周目、匯出成 VN 作品、結局一覽。重播桌（包 8）不出開新周目與匯出，重玩改叫章節選擇。
import { t } from "../../i18n";
import { reachedText, useEndings } from "./StoryPanels";
import "./vn-story.css";

export function EndingScreen({
  worldId,
  endingId,
  disabled,
  onReplay,
  onNewPlaythrough,
  onExport,
  onEndings,
  onTitle,
}: {
  worldId: string;
  endingId: string;
  disabled: boolean;
  onReplay: () => void;
  /** 重播桌沒給：不出開新周目、匯出，重玩鈕改叫章節選擇 */
  onNewPlaythrough?: () => void;
  onExport?: () => void;
  onEndings: () => void;
  onTitle: () => void;
}) {
  const endings = useEndings(worldId, endingId);
  const title = endings?.endings.find((ending) => ending.id === endingId)?.title;
  return (
    <div className="vn-ending-screen" role="dialog" aria-label={t("vnTheEnd")}>
      <p className="vn-chapter-heading">{t("vnTheEnd")}</p>
      {title && <p className="vn-ending-title">{title}</p>}
      {endings && reachedText(endings) && <p className="vn-chapter-note">{reachedText(endings)}</p>}
      <div className="vn-chapter-actions">
        <button type="button" className="btn btn-primary" onClick={onTitle}>
          {t("vnBackToTitle")}
        </button>
        <button type="button" className="btn" disabled={disabled} onClick={onReplay}>
          {onNewPlaythrough ? t("vnReplayChapter") : t("vnWorkChapterSelect")}
        </button>
        {onNewPlaythrough && (
          <button type="button" className="btn" disabled={disabled} onClick={onNewPlaythrough}>
            {t("vnNewPlaythrough")}
          </button>
        )}
        {onExport && (
          <button type="button" className="btn" disabled={disabled} onClick={onExport}>
            {t("vnWorkExport")}
          </button>
        )}
        <button type="button" className="btn" onClick={onEndings}>
          {t("vnEndingsList")}
        </button>
      </div>
    </div>
  );
}
