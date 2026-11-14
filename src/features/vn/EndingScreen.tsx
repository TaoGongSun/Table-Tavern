// 完結畫面（vn-mode 包 5 §2.7、包 6 §2.8、包 8 §2.2）：「完」、結局標題、已達成 x／y、音訊署名；回標題（主鈕）、從某一章重玩、
// 開新周目、匯出成 VN 作品、結局一覽。重播桌（包 8）不出開新周目與匯出，重玩改叫章節選擇。
import { t } from "../../i18n";
import { creditLines, useCreditEntries, type CreditLine } from "./audio/credits";
import { DEFAULT_ENDING, reachedText, useEndings } from "./StoryPanels";
import "./vn-story.css";

function creditText(line: CreditLine): string {
  const join = t("vnCreditsJoin");
  const authors = line.authors.join(join) + (line.more > 0 ? t("vnCreditsMore", { n: line.more }) : "");
  return t(line.kind === "bgm" ? "vnCreditsLineMusic" : "vnCreditsLineSfx", {
    authors,
    hosts: line.hosts.join(join),
    licenses: line.licenses.join(join),
  });
}

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
  const creditEntries = useCreditEntries(worldId);
  const lines = creditLines(creditEntries);
  const title = endings?.endings.find((ending) => ending.id === endingId)?.title;
  return (
    <div className="vn-ending-screen" role="dialog" aria-label={t("vnTheEnd")}>
      <p className="vn-chapter-heading">{t("vnTheEnd")}</p>
      {title && <p className="vn-ending-title">{title}</p>}
      {/* 保底結局不計入 x／y〔作者裁決 2026-11-02〕：剛看完保底結局時不出這行，免得讀成「一個都沒達成」 */}
      {endingId !== DEFAULT_ENDING && endings && reachedText(endings) && (
        <p className="vn-chapter-note">{reachedText(endings)}</p>
      )}
      {lines.length > 0 && (
        <div className="vn-credits-lines">
          {lines.map((line) => (
            <p key={line.kind}>{creditText(line)}</p>
          ))}
        </div>
      )}
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
