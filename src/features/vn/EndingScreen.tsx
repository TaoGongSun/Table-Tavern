// 完結畫面（vn-mode 包 5 §2.7、包 6 §2.8）：「完」、結局標題、已達成 x／y；從某一章重玩、開新周目、結局一覽、回標題。
import { t } from "../../i18n";
import { reachedCount, useEndings } from "./StoryPanels";
import "./vn-story.css";

export function EndingScreen({
  worldId,
  endingId,
  disabled,
  onReplay,
  onNewPlaythrough,
  onEndings,
  onTitle,
}: {
  worldId: string;
  endingId: string;
  disabled: boolean;
  onReplay: () => void;
  onNewPlaythrough: () => void;
  onEndings: () => void;
  onTitle: () => void;
}) {
  const endings = useEndings(worldId, endingId);
  const title = endings?.endings.find((ending) => ending.id === endingId)?.title;
  return (
    <div className="vn-ending-screen" role="dialog" aria-label={t("vnTheEnd")}>
      <p className="vn-chapter-heading">{t("vnTheEnd")}</p>
      {title && <p className="vn-ending-title">{title}</p>}
      {endings && (
        <p className="vn-chapter-note">
          {t("vnEndingsReached", { x: reachedCount(endings), y: endings.endings.length })}
        </p>
      )}
      <div className="vn-chapter-actions">
        <button type="button" className="btn btn-primary" disabled={disabled} onClick={onReplay}>
          {t("vnReplayChapter")}
        </button>
        <button type="button" className="btn" disabled={disabled} onClick={onNewPlaythrough}>
          {t("vnNewPlaythrough")}
        </button>
        <button type="button" className="btn" onClick={onEndings}>
          {t("vnEndingsList")}
        </button>
        <button type="button" className="btn" onClick={onTitle}>
          {t("vnBackToTitle")}
        </button>
      </div>
    </div>
  );
}
