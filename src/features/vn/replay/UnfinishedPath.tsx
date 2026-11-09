// 「作者到這裡就停了」（vn-mode 包 8 §2.6）：重播桌讀到作者沒玩下去的地方（不是結局、沒有可按的選項、沒有下一幕）。
// 幕末單獨出現時外面包選項面板的定位；選項點全灰時放在選項面板裡。
import { t } from "../../../i18n";

export function UnfinishedPath({
  disabled,
  onBackToChoice,
  onTitle,
}: {
  disabled: boolean;
  /** 沒給＝本幕與前一幕都沒有選過的選項格 */
  onBackToChoice?: () => void;
  onTitle: () => void;
}) {
  return (
    <div className="vn-unfinished" role="status">
      <p className="vn-unfinished-note">{t("vnWorkUnfinished")}</p>
      {onBackToChoice && (
        <button type="button" className="btn vn-choice" disabled={disabled} onClick={onBackToChoice}>
          {t("vnWorkBackToChoice")}
        </button>
      )}
      {!onBackToChoice && (
        <button type="button" className="btn btn-ghost" onClick={onTitle}>
          {t("vnBackToTitle")}
        </button>
      )}
    </div>
  );
}
